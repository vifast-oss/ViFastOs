// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-340d** — ca kiểm **TLB shootdown đa hart** trên `x86_64` (D2-238/D2-244/D2-247, D2-68,
//! D2-77; đóng lỗi v1 **N5**).
//!
//! # Vì sao ca kiểm này chạy **ngay sau TAP 21** (và vì sao số hiệu là 37)
//!
//! Shootdown cần **hai** hart cùng dùng **một** `VSpace`: hart A đổi bảng trang, hart B phải thấy
//! hiệu lực. Vì vậy hart phụ phải **nạp `CR3`** của `VSpace` ấy — điều đó chỉ an toàn khi hart phụ
//! đang ở **kernel context** (vòng `hlt` của `kmain_secondary`). Sau `sched::run`, một hart phụ có
//! thể đang chạy tác vụ **ring 3** (pha nhiều hart của `KT-310` chọn cả `TASK_U`); đổi `CR3` lúc ấy
//! làm mã user không còn map ⇒ `#PF` thật, đỏ oan và làm bẩn bất biến đếm `# exc:` của runner.
//!
//! Vì vậy ca kiểm chạy **giữa TAP 21 và TAP 22**, và phát TAP **37** (số hiệu cuối dải) — cùng
//! tinh thần "chạy sau TAP 21, không đổi số hiệu 1..36" của `KT-326`/`KT-336a`/`KT-340a`. TAP 13
//! cho phép số hiệu **không theo thứ tự** miễn đủ/không trùng và khớp plan `1..N`, nên không phải
//! dịch lại toàn bộ TAP 22..36 (cùng mọi comment + grep của runner đang gắn với chúng).
//!
//! # Hai chiều shootdown, hai bằng chứng khác nhau
//!
//! ```text
//!  (a) BSP → hart 1   : đường **sản phẩm** — `VSpace::map`/`unmap` (`VSpace::flush_va`)
//!      BSP map VA→frameA; hart 1 nạp `CR3` của VSpace rồi đọc VA (TLB = A); BSP unmap rồi map
//!      VA→frameB ⇒ tự động `TlbFlush(VA)` tới hart 1; handler hart 1 `invlpg` + **đọc lại** VA
//!      ⇒ phải ra `B`. Bằng chứng: `remap_obs = B` **và** ACK mang **đúng** `seq` của `map`.
//!      Đột biến (i) `mut-no-shootdown` ⇒ không IPI ⇒ `obs` = A (dữ liệu **cũ**) ⇒ **ĐỎ**.
//!      Đột biến (ii) `mut-no-ipi-invlpg` ⇒ ACK vẫn 1 nhưng `obs` = A ⇒ **ĐỎ**.
//!
//!  (b) hart 1 → BSP   : chiều của lỗi v1 **N5** — yêu cầu `MODE_REMAP|MODE_CHAIN`
//!      hart 1 **ghi ô lá** của VA2 → frame D rồi tự gửi `TlbFlush(VA2)` cho BSP; BSP đang chờ ACK
//!      trong vùng `IF = 0` nên **chỉ** vòng chờ (`drain`) của nó mới phục vụ được yêu cầu ấy.
//!      Bằng chứng: `chain_ack == chain_seq`, `chain_obs = D` **và** BSP đọc VA2 ra `D`.
//!      Đột biến (iii) `mut-no-wait-drain` ⇒ vòng chờ **không** `drain` ⇒ `chain_ack = 0` ⇒ **ĐỎ**.
//! ```
//!
//! # Vì sao `obs` (hart nhận **đọc lại** VA) là bằng chứng, không phải trang trí
//!
//! "Đã gửi IPI" và "đã ACK" đều **không** chứng minh hart kia thấy mapping mới — đột biến (ii) cho
//! ACK = 1 mà dữ liệu vẫn cũ. Vì vậy yêu cầu của `map` mang bit `MODE_OBSERVE`: hart nhận **đọc
//! lại chính VA** ngay sau `invlpg`, ghi giá trị vào ô yêu cầu, **rồi** mới ACK. Cùng lý do với
//! `KT-340b` (probe `INVLPG` đọc lại cùng VA trên **một** hart): phép đo phải **quan sát được**.
//!
//! # Ngân sách
//!
//! Không hart mới, không khung hình: 4 frame dữ liệu + 2 bảng trang từ **cùng** arena
//! `bootpt::__boot_arena` (32 frame) mà `KT-340b` đã dựng; mọi vòng chờ có **trần số vòng**
//! (`ipi::ACK_SPIN_LIMIT`) nên một hart chết chỉ làm ca kiểm ĐỎ, không treo máy.

use core::fmt::Write;

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::bootmem::{BootArena, FRAME_BYTES};
use vifast_plat_x86_64::format::{Perms, PTE_ADDR_MASK};
use vifast_plat_x86_64::ipi::{self, AckOutcome, FlushRequest, MODE_CHAIN, MODE_REMAP};
use vifast_plat_x86_64::mmu::{TableMemory, WalkOutcome};
use vifast_plat_x86_64::smp::SMP_HARTS;
use vifast_plat_x86_64::umode::VSPACE_BASE;
use vifast_plat_x86_64::vspace::VSpace;

use crate::smp;
use crate::{check, Console};

/// VA của trang **được shootdown** (cửa sổ 64 GiB của `VSpace` động — ngoài identity/MMIO).
///
/// `KT-340b` dùng `base+0x0000..base+0x4000`; ca này dùng hai trang **kế tiếp** để không đụng bất
/// kỳ VA nào của phép đo cũ (mỗi `VSpace` có cây riêng nên đây chỉ là quy ước dễ đọc).
pub(crate) const TLB_VA: u64 = VSPACE_BASE + 0x6000;
/// VA của trang mà **hart 1** sửa ô lá (chiều AP→BSP của v1 N5).
pub(crate) const CHAIN_VA: u64 = VSPACE_BASE + 0x7000;

/// Hart phụ tham gia ca kiểm — hart 1, cùng hart `KT-278` đã dùng cho TAP 20.
const PEER: usize = 1;

/// Dữ liệu bốn frame — khác nhau **từng bit** để một lần đọc sai không thể trùng ngẫu nhiên.
const WORD_A: u64 = 0xA1A1_1111_2222_3333;
const WORD_B: u64 = 0xB2B2_4444_5555_6666;
const WORD_C: u64 = 0xC3C3_7777_8888_9999;
const WORD_D: u64 = 0xD4D4_AAAA_BBBB_CCCC;

/// `seq` kế tiếp — lấy từ **một** nguồn toàn ảnh (`ipi::next_seq`); **không** giữ bộ đếm riêng:
/// `seq` trùng giữa hai nguồn làm ACK cũ thoả yêu cầu mới (xem `ipi::SEQ_SOURCE`).
fn next_seq() -> u64 {
    ipi::next_seq()
}

/// Kết quả một pha — đọc ra để in **một** dòng bằng chứng.
#[derive(Clone, Copy, Debug, Default)]
struct Phase {
    /// `seq` mà hart nhận đã ACK.
    acked: u64,
    /// Giá trị hart nhận đọc lại `va` (`0` = không đọc).
    obs: u64,
}

/// Cấp một frame dữ liệu từ arena bảng trang và ghi `word` vào đó.
fn alloc_data(console: &mut Console, arena: &mut BootArena, what: &str, word: u64) -> Option<u64> {
    match arena.alloc_frame() {
        Some(pa) => {
            asm::write_u64(pa, word);
            Some(pa)
        }
        None => {
            let _ = writeln!(
                console,
                "PANIC: tlb: arena hết frame khi cấp '{what}' ({} / {} frame)",
                arena.used(),
                arena.frames()
            );
            None
        }
    }
}

/// Địa chỉ (PA) của **ô lá** phủ `va` trong `vs` — để hart khác ghi đúng ô ấy.
///
/// `WalkStep.table` là PA bảng đang đọc, `index` là chỉ số entry ⇒ `table + index * 8`. Chỉ nhận
/// walk kết thúc ở **lá 4 KiB** (`Block`/`Table` ⇒ `None`, fail-closed).
fn leaf_entry_pa(vs: &VSpace<BootArena>, va: u64) -> Option<u64> {
    let walk = vs.walk(va);
    if !matches!(walk.outcome, WalkOutcome::Leaf { .. }) || walk.len == 0 {
        return None;
    }
    let step = walk.steps[walk.len - 1]?;
    Some(step.table + (step.index as u64) * 8)
}

/// Ca kiểm TLB shootdown — phát **một** TAP (`tap`) và các dòng `# tlb:`/`# ipi:`.
///
/// Gọi **giữa** TAP 21 (`smp::run`) và TAP 22 (`sched::run`): hart phụ còn ở vòng `hlt` kernel
/// context nên `CR3` của nó đổi được an toàn (xem đầu tệp).
pub fn run(console: &mut Console, tap: usize) {
    // ---- 0. Công bố `LAPIC_ID` cho crate arch: từ đây `ipi::current_hart()` biết "hart đang chạy".
    let mut published = 0u32;
    let mut hart = 0usize;
    while hart < SMP_HARTS {
        if let Some(id) = smp::lapic_id_of_hart(hart) {
            if ipi::set_lapic_id(hart, id) {
                published |= 1u32 << hart;
            }
        }
        hart += 1;
    }
    let pcid = ipi::pcid_status();
    let boot_root = asm::read_cr3() & !(FRAME_BYTES - 1);
    let mut ok = published & (1u32 << PEER) != 0 && pcid.full_flush_ok();
    let mut reason: &str = "";

    // ---- 1. `VSpace` **riêng** cho ca kiểm + bốn frame dữ liệu (cùng arena của `KT-340b`).
    let Some(arena) = BootArena::new(
        crate::bootpt::arena_base(),
        crate::bootpt::arena_frames(),
        crate::bootpt::arena_next(),
    ) else {
        let _ = writeln!(console, "PANIC: tlb: arena bảng trang không hợp lệ");
        asm::halt(3)
    };
    let mut data = arena;
    let (Some(pa_a), Some(pa_b), Some(pa_c), Some(pa_d)) = (
        alloc_data(console, &mut data, "tlb-a", WORD_A),
        alloc_data(console, &mut data, "tlb-b", WORD_B),
        alloc_data(console, &mut data, "tlb-c", WORD_C),
        alloc_data(console, &mut data, "tlb-d", WORD_D),
    ) else {
        asm::halt(3)
    };

    let mut vs = match VSpace::create(3, arena) {
        Ok(vs) => vs,
        Err(err) => {
            let _ = writeln!(console, "PANIC: tlb: tạo VSpace lỗi {err:?}");
            asm::halt(3)
        }
    };
    for (va, pa, what) in [(TLB_VA, pa_a, "tlb1"), (CHAIN_VA, pa_c, "tlb2")] {
        if let Err(err) = vs.map(va, pa, Perms::USER_RW) {
            let _ = writeln!(console, "PANIC: tlb: map {what} lỗi {err:?}");
            asm::halt(3)
        }
    }
    // **Sản phẩm** của ca kiểm: `VSpace` do hart 0 kích hoạt, hart 1 nhận qua IPI (mục 2).
    vs.activate();
    // Nạp TLB **trước** khi đổi mapping: hai entry "cũ" nằm trong TLB của hart 0.
    let stale_tlb = asm::read_u64(TLB_VA);
    let stale_chain = asm::read_u64(CHAIN_VA);

    // ---- 2. Chiều **BSP → hart 1**: `TlbFlush(TLB_VA)` mang `root` + `MODE_OBSERVE`; handler
    // hart 1 nạp `CR3` của VSpace rồi `invlpg` + đọc lại ⇒ `obs` phải là `WORD_A` (và phép đọc ấy
    // **nạp** entry "cũ" A vào TLB của hart 1 — nguyên liệu cho pha 3).
    let mut adopt = Phase::default();
    if ok {
        let seq = next_seq();
        let mut req = FlushRequest::observe(seq, TLB_VA);
        req.root = vs.root();
        if ipi::issue(PEER, &req) {
            match ipi::wait_ack(PEER, seq) {
                AckOutcome::Acked => {
                    let snap = ipi::flush_snapshot(PEER);
                    adopt = Phase {
                        acked: snap.acked,
                        obs: snap.obs,
                    };
                    // Hart 1 **đang chạy** VSpace này từ giờ (nó vừa nạp `CR3` của nó).
                    vs.mark_active(PEER);
                }
                _ => reason = "hart 1 không ACK khi nhận VSpace",
            }
            ipi::release_slot(PEER);
        } else {
            reason = "không gửi được TlbFlush(VSpace) cho hart 1";
        }
        ok &= ipi::ack_ok(adopt.acked, seq) && adopt.obs == WORD_A;
    }

    // ---- 3. Chiều (a): `VSpace::map`/`unmap` — đường **sản phẩm** (`VSpace::flush_va`) tự
    // shootdown mọi hart khác trong `active_on` (ở đây: hart 1) và chờ ACK.
    let mut remap = Phase::default();
    let mut remap_ok = false;
    if ok {
        // `unmap` (chỉ xoá TLB) rồi `map` sang frame **B** (xoá + đọc lại) — hai yêu cầu, hai `seq`.
        if vs.unmap(TLB_VA).is_err() {
            reason = "unmap TLB_VA lỗi";
        } else if vs.map(TLB_VA, pa_b, Perms::USER_RW).is_err() {
            reason = "map TLB_VA→B lỗi";
        } else {
            let snap = ipi::flush_snapshot(PEER);
            remap = Phase {
                acked: snap.acked,
                obs: snap.obs,
            };
            // ACK phải mang **đúng** `seq` của `map` (yêu cầu cuối) **và** dữ liệu đọc lại là B.
            remap_ok = ipi::ack_ok(remap.acked, vs.flush_seq()) && remap.obs == WORD_B;
            if !remap_ok {
                reason = "hart 1 không thấy mapping mới sau shootdown";
            }
        }
        ok &= remap_ok;
    }

    // ---- 4. Chiều (b) — **v1 N5**: hart 1 **sửa ô lá** của `CHAIN_VA` (→ frame D) rồi gửi
    // `TlbFlush(CHAIN_VA)` cho BSP. BSP chờ ACK trong `IF = 0` ⇒ **chỉ** `drain` trong vòng chờ
    // mới phục vụ được yêu cầu ấy (đột biến (iii) bỏ `drain` ⇒ `chain_ack = 0`).
    let mut chain = Phase::default();
    let mut chain_seq = 0u64;
    let mut chain_read = 0u64;
    let mut own_wait = AckOutcome::NotSent;
    let mut peer_wait = AckOutcome::NotSent;
    let mut chain_ok = false;
    if ok {
        match leaf_entry_pa(&vs, CHAIN_VA) {
            None => reason = "không tìm được ô lá của CHAIN_VA",
            Some(leaf_pa) => {
                let old = asm::read_u64(leaf_pa);
                let new = (old & !PTE_ADDR_MASK) | (pa_d & PTE_ADDR_MASK);
                let seq = next_seq();
                chain_seq = next_seq();
                let mut req = FlushRequest::observe(seq, TLB_VA);
                req.mode = MODE_REMAP | MODE_CHAIN;
                req.remap_pa = leaf_pa;
                req.remap_val = new;
                req.chain = 0;
                req.chain_seq = chain_seq;
                req.chain_va = CHAIN_VA;
                asm::cli();
                let issued = ipi::issue(PEER, &req);
                if issued {
                    peer_wait = ipi::wait_ack(PEER, seq);
                    // Vòng chờ **thứ hai** mới là chỗ phục vụ hộp thư của chính BSP (N5).
                    own_wait = ipi::wait_ack(0, chain_seq);
                }
                asm::sti();
                // Ô của hart 0 do **hart 1** giữ chỗ (nó là người gửi yêu cầu nối) — người nhận
                // biết yêu cầu đã xong khi thấy ACK, nên trả chỗ ở đây là đúng chủ sở hữu *dữ liệu*
                // của ô (một yêu cầu đang bay, người gửi thật không còn chờ).
                ipi::release_slot(PEER);
                ipi::release_slot(0);
                let own = ipi::flush_snapshot(0);
                chain = Phase {
                    acked: own.acked,
                    obs: own.obs,
                };
                // Đọc **trước** `vs.restore()`: `CHAIN_VA` chỉ map trong `VSpace` này, không map
                // trong cây gốc của ảnh — đọc sau khi trả `CR3` là `#PF` thật (không phải ca kiểm).
                chain_read = asm::read_u64(CHAIN_VA);
                let seen = chain_read;
                chain_ok = issued
                    && peer_wait == AckOutcome::Acked
                    && own_wait == AckOutcome::Acked
                    && ipi::ack_ok(chain.acked, chain_seq)
                    && chain.obs == WORD_D
                    && seen == WORD_D;
                if !chain_ok {
                    reason = "BSP không phục vụ TlbFlush(CHAIN_VA) trong lúc chờ ACK (N5)";
                }
            }
        }
        ok &= chain_ok;
    }

    // ---- 5. Trả `CR3` của hart 1 rồi của hart 0 về cây gốc (hart 1 **phải** rời VSpace trước,
    // nếu không nó chạy với `CR3` của một cây mà `active_on` không còn phản ánh).
    if vs.active_on() & (1u32 << PEER) != 0 {
        let seq = next_seq();
        let mut req = FlushRequest::flush(seq, 0);
        req.root = boot_root;
        if ipi::issue(PEER, &req) {
            let _ = ipi::wait_ack(PEER, seq);
        }
        ipi::release_slot(PEER);
        vs.mark_inactive(PEER);
    }
    vs.restore();
    let restored = asm::read_cr3() & !(FRAME_BYTES - 1) == boot_root;

    // ---- 6. Bằng chứng + TAP.
    let stats = ipi::stats();
    // Phép đo chỉ **chứng minh** được khi dữ liệu cũ/mới thật sự khác nhau và mọi yêu cầu đều tới
    // đích (không `invalid`, không `timeout`) — fail-closed.
    // Sáu yêu cầu của ca kiểm: `adopt` + `unmap` + `map` + `chain` (hart 1) + `chain` (hart 0) +
    // `restore`. `empty` **không** phải điều kiện: vòng chờ gọi `drain` mỗi bước nên hộp thư rỗng
    // ở phần lớn bước lặp là bình thường (chỉ handler mới đếm `empty` — xem `ipi::drain_waiting`).
    let stats_ok = stats.sent >= 6
        && stats.acked >= 6
        && stats.drained >= 6
        && stats.invalid == 0
        && stats.timeouts == 0;
    let ok = ok && restored && stats_ok && stale_tlb == WORD_A && stale_chain == WORD_C;
    let _ = writeln!(
        console,
        "# tlb: shootdown va=0x{TLB_VA:x} stale=0x{stale_tlb:016x} remap_obs=0x{:016x} expect=(0x{WORD_A:016x},0x{WORD_B:016x}) adopt_ack={} remap_ack={} remap_seq={} chain_va=0x{CHAIN_VA:x} chain_obs=0x{:016x} chain_ack={} chain_seq={} chain_read=0x{:016x} expect_chain=(0x{WORD_C:016x},0x{WORD_D:016x}) peer_wait={} own_wait={} remap_ok={} chain_ok={} restored={} ok={}",
        remap.obs,
        adopt.acked,
        remap.acked,
        vs.flush_seq(),
        chain.obs,
        chain.acked,
        chain_seq,
        chain_read,
        peer_wait.name(),
        own_wait.name(),
        remap_ok as u8,
        chain_ok as u8,
        restored as u8,
        ok as u8,
    );
    let _ = writeln!(
        console,
        "# ipi: mailbox posted={} coalesced={} sent={} drained={} empty={} acked={} invalid={} local={} timeouts={} busy={} vector=0x{:02x} target={PEER} online=0x{published:x} ok={}",
        stats.posted,
        stats.coalesced,
        stats.sent,
        stats.drained,
        stats.empty,
        stats.acked,
        stats.invalid,
        stats.local,
        stats.timeouts,
        stats.busy,
        ipi::IPI_VECTOR,
        stats_ok as u8,
    );
    let _ = writeln!(
        console,
        "# ipi: pcid supported={} pge_supported={} cr4_pcid={} cr4_pge={} full_flush={} reason=no-pcid-per-va-invlpg",
        pcid.cpuid_pcid as u8,
        pcid.cpuid_pge as u8,
        pcid.cr4_pcid as u8,
        pcid.cr4_pge as u8,
        pcid.full_flush_ok() as u8,
    );
    if !reason.is_empty() {
        let _ = writeln!(console, "# tlb: lý do ĐỎ — {reason}");
    }
    check(console, tap, "tlb: shootdown ap->bsp", ok);
}
