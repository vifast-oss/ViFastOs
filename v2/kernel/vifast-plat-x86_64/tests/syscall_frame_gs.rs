// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho hotfix audit `KT-417c` — X1 (bố cục frame cửa `syscall`) + X2 (cặp `swapgs`
//! phải khép ở **mọi** đường rời kernel về user) + X3 (đo "đã vào từ CPL3" ở cửa nhanh bằng RIP).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test syscall_frame_gs
//! ```
//!
//! # Vì sao test này tồn tại
//!
//! Audit W8b (`/tmp/audits/code-audit.md` §X1/§X2/§X3) chỉ ra ba lỗi **không** cổng nào bắt:
//!
//! 1. **X1** — frame cửa `syscall` **136 B** (`err` + `vector` + 15 GPR) nhưng handler nhánh
//!    `Switched` ghi `frame + SYSCALL_FRAME_RIP` = `frame + 136` cho **cả hai** cửa ⇒ trên cửa
//!    nhanh đó **không** phải một ô của frame mà **đúng** `rsp0` = đỉnh `__syscall_stack`, tức
//!    8 B đầu của `__ist1_stacks` (stack `#DF` hart 0 — `kbin/bootasm.rs` đặt hai mảng **cùng VA
//!    liền nhau**). Bản vá: `resume_rip_offset(via_syscall)` trả ô `RCX` (104) cho cửa nhanh — ô
//!    **có thật** mà `sysretq` đọc lại — và ô `RIP` (136) chỉ dùng cho `int 0x80`.
//! 2. **X2** — cặp `swapgs` chỉ nằm trong thân stub `syscall`; đường `Exit` (không quay lại đuôi
//!    stub) để `GS.base` lật **vĩnh viễn** ⇒ `syscall` kế tiếp `mov gs:[8], rsp` ghi vào VA 8 ⇒
//!    `#PF` CPL0 trên stack user (SMAP ⇒ `#DF`). Bản vá: `enter_user` — nơi **duy nhất** mọi
//!    `iretq` về CPL3 đi qua — chuẩn hoá cặp GS bằng **đúng một** `swapgs` **có điều kiện**.
//! 3. **X3** — `cs` của cửa nhanh là **hằng** `sysret_cs()` ⇒ `user=1` là tautology. Bản vá:
//!    cửa nhanh truyền sentinel [`SYSCALL_FAST_NO_CS`]; `user` suy từ **RIP đo được** (`RCX` do
//!    CPU ghi) nằm trong cửa sổ mã user.
//!
//! # Đỏ-trước (bằng chứng, `v2/agent/JOURNAL/kt417c-fix/`)
//!
//! * `red-frame-gs.log` — chạy **trước** bản vá: `fast_frame_resume_slot_must_be_inside_the_136_byte_frame`
//!   và `canary_at_rsp0_must_survive_the_resume_write` ĐỎ (ghi `frame+136` đè `rsp0`), và
//!   `enter_user_must_restore_gs_before_iretq` ĐỎ (`enter_user` chưa có `swapgs` nào).
//! * `red-check-syscall.log` — `check_syscall` của runner XANH với `cs`/`rip` giả (audit X3).

#![allow(clippy::assertions_on_constants)]

use vifast_plat_x86_64::syscall_fast::{
    gs_is_kernel, gs_scratch, needs_swap_before_user, user_gs_ok, user_gs_pair_ok_with,
    USER_GS_BASE,
};
use vifast_plat_x86_64::umode::{
    is_user_cs, resume_rip_offset, user_from_entry, user_rip_ok, KERNEL_CS,
    SYSCALL_FAST_FRAME_BYTES, SYSCALL_FAST_NO_CS, SYSCALL_FRAME_RCX, SYSCALL_FRAME_RIP,
    USER_CODE_BYTES, USER_CODE_VA, USER_CS, VSPACE_CODE_VA,
};

/// **Số byte frame của cửa `syscall`**, viết tay từ thân stub (`err` 8 + `vector` 8 + 15 GPR × 8
/// = 17 qword). Cố ý **không** lấy từ hằng của crate: đây là con số của **stub asm**, thứ mà
/// test phải đối chiếu.
const FAST_FRAME_BYTES: usize = 136;

/// Vùng scratch **giả lập** của hart 0 cho state machine `swapgs` (`gs_scratch` thật là địa chỉ
/// của `plat-x86_64`; test host chỉ cần một vùng 32 B phân biệt với `USER_GS_BASE = 0`).
const FAKE_KERNEL_GS: u64 = 0x0000_0000_0015_01a0;

/// Đếm `push` **dựng frame** trong stub `syscall` (từ nhãn tới `mov rbx, rsp`), bỏ comment.
///
/// Đây là phép đối chiếu **chéo**: `FAST_FRAME_BYTES` viết tay ở trên phải bằng `số push × 8`,
/// nếu không thì một `push` thêm/bớt trong stub sẽ không còn khớp `SYSCALL_FAST_FRAME_BYTES`.
fn stub_frame_pushes() -> usize {
    let src = include_str!("../src/asm/syscall.rs");
    let start = src
        .find("vifast_isr_syscall_fast:")
        .expect("không thấy nhãn `vifast_isr_syscall_fast`");
    let body = &src[start..];
    let end = body
        .find("\"mov rbx, rsp\"")
        .expect("không thấy `mov rbx, rsp` (mốc hết phần dựng frame)");
    body[..end]
        .lines()
        .filter(|l| !l.trim_start().starts_with("//") && l.contains("\"push "))
        .count()
}

// =============================================================================================
// X1 — frame 136 B: ô resume của cửa nhanh phải nằm trong frame thật
// =============================================================================================

/// **Đỏ-trước**: bản KT-417c dùng `SYSCALL_FRAME_RIP = 136` cho **cả hai** cửa ⇒ trên cửa nhanh
/// ghi **ngoài** frame (vào `rsp0`). Sau vá: cửa nhanh ghi ô `RCX` (104); ô `RIP` = 136 đúng
/// bằng `rsp0` là **cấm** cho cửa nhanh.
#[test]
fn fast_frame_resume_slot_must_be_inside_the_136_byte_frame() {
    // Hằng của crate == số byte stub thật đẩy == số viết tay (hai chiều, không tautology).
    assert_eq!(
        SYSCALL_FAST_FRAME_BYTES, FAST_FRAME_BYTES,
        "SYSCALL_FAST_FRAME_BYTES phải bằng số byte stub `syscall` đẩy (17 qword)"
    );
    let pushes = stub_frame_pushes();
    assert_eq!(pushes, 17, "stub `syscall` phải đẩy 17 qword dựng frame");
    assert_eq!(
        pushes * 8,
        FAST_FRAME_BYTES,
        "17 qword × 8 = {FAST_FRAME_BYTES} B"
    );

    // Ô handler ghi để resume của cửa nhanh phải **có thật** trong 136 B…
    let fast_off = resume_rip_offset(true);
    assert_eq!(
        fast_off, SYSCALL_FRAME_RCX,
        "cửa `syscall` resume qua `RCX` — ô mà `sysretq` đọc lại"
    );
    assert!(
        fast_off + 8 <= FAST_FRAME_BYTES,
        "ô resume cửa nhanh +{fast_off} nằm ngoài frame {FAST_FRAME_BYTES} B"
    );

    // …và ô `RIP` của `int 0x80` **đúng bằng** `rsp0` của cửa nhanh ⇒ tuyệt đối không dùng.
    assert_eq!(
        SYSCALL_FRAME_RIP, FAST_FRAME_BYTES,
        "ô RIP của int 0x80 đúng bằng rsp0 — dùng cho cửa nhanh là ghi vào __ist1_stacks"
    );
    assert_eq!(resume_rip_offset(false), SYSCALL_FRAME_RIP);
    // Bốn ô CPU đẩy còn lại cũng **chỉ** có ở frame `int 0x80` (176 B).
    for (name, off) in [
        ("CS", vifast_plat_x86_64::umode::SYSCALL_FRAME_CS),
        ("RFLAGS", vifast_plat_x86_64::umode::SYSCALL_FRAME_RFLAGS),
        ("RSP", vifast_plat_x86_64::umode::SYSCALL_FRAME_RSP),
        ("SS", vifast_plat_x86_64::umode::SYSCALL_FRAME_SS),
    ] {
        assert!(
            off >= FAST_FRAME_BYTES,
            "ô {name} (+{off}) không tồn tại trong frame 136 B của cửa nhanh"
        );
    }
}

/// Bố cục **thật** của bộ nhớ quanh frame: 136 B frame nằm sát dưới `rsp0`, và **ngay trên**
/// `rsp0` là đầu `__ist1_stacks` của hart 0 (`kbin/bootasm.rs`: `__syscall_stack` + `.align 4096`
/// ⇒ cùng VA liền nhau). Canary = 8 B đầu của stack `#DF`; ghi resume **cửa nhanh** không được
/// chạm nó. Đối chứng âm: nếu ai quay lại ghi ô `RIP` (offset 136) thì canary **phải** bị đè —
/// nếu không, canary đặt sai chỗ và test này là dấu cao su.
#[test]
fn canary_at_rsp0_must_survive_the_resume_write() {
    const CANARY: u64 = 0x0000_1571_c0de;
    let entry = VSPACE_CODE_VA + 0x40;

    // (1) Hành vi **sau vá**: ghi ô resume của cửa nhanh (RCX).
    let mut mem = [0u8; FAST_FRAME_BYTES + 8];
    mem[FAST_FRAME_BYTES..].copy_from_slice(&CANARY.to_le_bytes());
    let off = resume_rip_offset(true);
    mem[off..off + 8].copy_from_slice(&entry.to_le_bytes());
    let canary = u64::from_le_bytes(mem[FAST_FRAME_BYTES..].try_into().unwrap());
    assert_eq!(
        canary, CANARY,
        "ghi frame+{off} (ô RCX) không được chạm 8 B đầu `__ist1_stacks` (rsp0)"
    );

    // (2) Đối chứng âm: đúng lỗi X1 (ghi `frame + SYSCALL_FRAME_RIP` = +136) **phải** đè canary.
    let mut bug = [0u8; FAST_FRAME_BYTES + 8];
    bug[FAST_FRAME_BYTES..].copy_from_slice(&CANARY.to_le_bytes());
    bug[SYSCALL_FRAME_RIP..SYSCALL_FRAME_RIP + 8].copy_from_slice(&entry.to_le_bytes());
    let bug_canary = u64::from_le_bytes(bug[FAST_FRAME_BYTES..].try_into().unwrap());
    assert_ne!(
        bug_canary, CANARY,
        "đối chứng âm: ghi frame+{SYSCALL_FRAME_RIP} phải đè canary (nếu không, canary sai chỗ)"
    );
}

// =============================================================================================
// X2 — cặp `swapgs` phải khép ở mọi đường về user
// =============================================================================================

/// Lọc **dòng mã** (bỏ comment) để đếm lệnh thật, không đếm chữ trong bình luận.
fn code_lines(src: &str) -> Vec<&str> {
    src.lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("//!"))
        })
        .collect()
}

/// `enter_user` là **nơi duy nhất** `iretq` về CPL3 (đường `Exit` → `start_core_phase` → đây,
/// đường `shell`, đường pha core). Nếu nó không khép cặp `swapgs` thì `GS.base` ở lại scratch
/// của kernel ⇒ `syscall` kế tiếp `mov gs:[8], rsp` ghi vào **VA 8** ⇒ `#PF` CPL0 **trên stack
/// user** (SMAP ⇒ `#DF`).
#[test]
fn enter_user_must_restore_gs_before_iretq() {
    let src = include_str!("../src/asm/trap.rs");
    let start = src
        .find("pub fn enter_user(entry")
        .expect("không thấy `pub fn enter_user` trong asm/trap.rs");
    let rest = &src[start..];
    let end = rest
        .find("Stub host của [`enter_user`]")
        .unwrap_or(rest.len());
    let body = code_lines(&rest[..end]);

    let swaps: Vec<usize> = body
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("\"swapgs\""))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        swaps.len(),
        1,
        "`enter_user` phải có **đúng một** `swapgs` (đường Exit/suspend-resume không đi qua đuôi stub); thấy {}",
        swaps.len()
    );
    let iret = body
        .iter()
        .position(|l| l.contains("\"iretq\""))
        .expect("không thấy `iretq` trong enter_user");
    assert!(
        swaps[0] < iret,
        "`swapgs` phải đứng TRƯỚC `iretq` (sau `iretq` là CPL3, không còn đặc quyền)"
    );
    assert!(
        body.iter()
            .any(|l| l.contains("needs_swap_before_user") || l.contains("user_gs_ok")),
        "`swapgs` của `enter_user` phải **có điều kiện** (`int 0x80` không lật GS ⇒ swap lần hai là lật sai chiều)"
    );
    // Bằng chứng đo được: `GS.base` phải được **đọc lại** (không tin trạng thái suy đoán).
    assert!(
        body.iter().any(|l| l.contains("MSR_IA32_GS_BASE")),
        "`enter_user` phải đọc `IA32_GS_BASE` để biết cặp GS đang lệch hay không"
    );
}

/// Stub `syscall` phải giữ **đúng một** `swapgs` vào + **đúng một** ra (bất biến 1 của
/// `syscall_fast.rs` — trước đây **0 test** nào kiểm, audit G10).
#[test]
fn fast_stub_has_exactly_one_swapgs_pair() {
    let src = include_str!("../src/asm/syscall.rs");
    let start = src
        .find("vifast_isr_syscall_fast:")
        .expect("không thấy nhãn `vifast_isr_syscall_fast`");
    let body = code_lines(&src[start..]);
    let swaps: Vec<usize> = body
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("\"swapgs\""))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        swaps.len(),
        2,
        "stub `syscall` phải có đúng 2 `swapgs` (một vào, một ra); thấy {}",
        swaps.len()
    );
    let sysret = body
        .iter()
        .position(|l| l.contains("\"sysretq\""))
        .expect("không thấy `sysretq`");
    assert!(
        swaps[0] < swaps[1] && swaps[1] < sysret,
        "thứ tự phải là: swapgs vào … swapgs ra … sysretq"
    );
}

/// **State machine của cặp MSR** (`swapgs` đảo `GS.base` ↔ `KERNEL_GS_BASE`) qua **mọi** đường
/// rời kernel. Ca âm ở cuối mô phỏng đúng X2: bỏ bước chuẩn hoá của `enter_user` ⇒ lần `syscall`
/// kế có `GS.base = 0` và `mov gs:[8], rsp` ghi vào **VA 8** (ngoài vùng scratch).
#[test]
fn gs_pair_survives_every_way_out() {
    /// Hai MSR mà `swapgs` đảo (SDM Vol 2A "SWAPGS").
    struct GsPair {
        gs_base: u64,
        kernel_gs_base: u64,
    }
    fn swapgs(m: &mut GsPair) {
        core::mem::swap(&mut m.gs_base, &mut m.kernel_gs_base);
    }

    // `syscall_fast::init`: ring 3 chạy với `GS.base = 0`, scratch của kernel ở `KERNEL_GS_BASE`.
    let mut m = GsPair {
        gs_base: USER_GS_BASE,
        kernel_gs_base: FAKE_KERNEL_GS,
    };
    assert!(user_gs_ok(m.gs_base));

    // (1) `syscall` vào: một `swapgs` ⇒ kernel dùng `gs:[0]`/`gs:[8]` **trong vùng scratch**.
    swapgs(&mut m);
    assert_eq!(m.gs_base, FAKE_KERNEL_GS);
    let user_rsp_slot = m.gs_base + 8;
    assert!(
        (FAKE_KERNEL_GS..FAKE_KERNEL_GS + 32).contains(&user_rsp_slot),
        "`gs:[8]` của kernel phải trỏ trong scratch, không phải VA 8"
    );

    // (2) Đường `Return`: đuôi stub `swapgs` một lần ⇒ về đúng base ring 3 trước `sysretq`.
    swapgs(&mut m);
    assert!(user_gs_ok(m.gs_base));

    // (3) Đường `Exit`: đi vào bằng `syscall` rồi **không** quay lại đuôi stub ⇒ GS còn scratch.
    swapgs(&mut m);
    assert_eq!(m.gs_base, FAKE_KERNEL_GS, "Exit để lại GS.base của kernel");

    // `enter_user` (nơi duy nhất mọi `iretq` về CPL3 đi qua) phải chuẩn hoá **có điều kiện**.
    assert!(
        needs_swap_before_user(m.gs_base),
        "GS.base còn là scratch ⇒ enter_user phải swapgs bù"
    );
    swapgs(&mut m);
    assert_eq!(m.gs_base, USER_GS_BASE);
    assert!(user_gs_ok(m.gs_base));

    // (4) `syscall` kế tiếp: một `swapgs` ⇒ GS lại là scratch, `mov gs:[8], rsp` ghi **trong**
    // vùng scratch. Đây là điều X2 bảo đảm: lần vào thứ hai **không** ghi vào VA 8.
    swapgs(&mut m);
    assert_eq!(m.gs_base, FAKE_KERNEL_GS);
    assert!((FAKE_KERNEL_GS..FAKE_KERNEL_GS + 32).contains(&(m.gs_base + 8)));

    // ---- Đối chứng âm: **bỏ** bước chuẩn hoá của `enter_user` (đúng bug X2).
    let mut bug = GsPair {
        gs_base: FAKE_KERNEL_GS, // Exit không khép cặp
        kernel_gs_base: USER_GS_BASE,
    };
    swapgs(&mut bug); // `syscall` kế: swapgs đảo ⇒ GS.base = 0
    assert_eq!(bug.gs_base, USER_GS_BASE, "thiếu swapgs bù ⇒ GS.base = 0");
    assert_eq!(
        bug.gs_base + 8,
        8,
        "thiếu swapgs bù ⇒ `mov gs:[8], rsp` ghi VA 8 (kernel text RO) ⇒ #PF CPL0 trên stack user"
    );
    assert!(
        !(FAKE_KERNEL_GS..FAKE_KERNEL_GS + 32).contains(&(bug.gs_base + 8)),
        "đối chứng âm: VA 8 phải ngoài vùng scratch"
    );
}

// =============================================================================================
// X3 — cửa nhanh không có CS: "đã vào từ CPL3" phải suy từ RIP đo được
// =============================================================================================

/// **Đỏ-trước**: `cs` của cửa nhanh là hằng `sysret_cs()` ⇒ `is_user_cs(cs)` **luôn** đúng ⇒
/// `user=1` không đo gì. Sau vá: `cs` là sentinel [`SYSCALL_FAST_NO_CS`]; `user` suy từ `rip`.
#[test]
fn fast_gate_has_no_cs_so_user_must_come_from_rip() {
    // Sentinel **không** phải selector user (0 = null descriptor) — nếu ai truyền `USER_CS` vào
    // cửa nhanh thì đó là **tự khai**, không phải bằng chứng.
    assert_eq!(SYSCALL_FAST_NO_CS, 0);
    assert!(!is_user_cs(SYSCALL_FAST_NO_CS));

    let static_rip = USER_CODE_VA + 0x25;
    let vspace_rip = VSPACE_CODE_VA + 0x40;
    assert!(user_rip_ok(static_rip), "RIP trong cửa sổ mã tĩnh");
    assert!(user_rip_ok(vspace_rip), "RIP trong cửa sổ VSpace");
    assert!(!user_rip_ok(VSPACE_CODE_VA + USER_CODE_BYTES));
    assert!(!user_rip_ok(USER_CODE_VA - 8));
    // Canonical: nửa cao (kernel) **không** phải user, dù rơi đúng số học của cửa sổ.
    assert!(!user_rip_ok(0xffff_ffff_8000_0000));
    assert!(!user_rip_ok(0x10_0000)); // identity map của ảnh kernel

    // Cửa nhanh: không CS + RIP **đo được** trong cửa sổ user ⇒ user.
    assert!(user_from_entry(true, SYSCALL_FAST_NO_CS, vspace_rip));
    assert!(user_from_entry(true, SYSCALL_FAST_NO_CS, static_rip));
    // Đúng bug X3: truyền `sysret_cs()` (selector user) làm "bằng chứng" ⇒ **không** được coi là user.
    assert!(
        !user_from_entry(true, USER_CS as u64, vspace_rip),
        "cs tự khai của cửa nhanh không phải bằng chứng đã vào từ CPL3"
    );
    // RIP ngoài cửa sổ user (kernel lỡ `syscall` ở CPL0) ⇒ không phải user.
    assert!(!user_from_entry(true, SYSCALL_FAST_NO_CS, 0x10_0000));

    // Cửa `int 0x80`: `cs` do **CPU** đẩy ⇒ đo thật; RIP không tham gia.
    assert!(user_from_entry(false, USER_CS as u64, 0));
    assert!(!user_from_entry(false, KERNEL_CS as u64, vspace_rip));
    assert!(!user_from_entry(false, SYSCALL_FAST_NO_CS, vspace_rip));
}

// =============================================================================================
// X2 vòng 2 — cặp MSR phải đúng **cả hai** phía; và mã kbin phải thật sự dùng hàm của crate
// =============================================================================================

/// `enter_user` chỉ kiểm `GS.base == 0` là **thiếu**: `KERNEL_GS_BASE` sai (scratch hart khác)
/// vẫn qua chốt và lần `syscall` sau lấy `gs:[0]`/`gs:[8]` của hart khác. Test **nhân thuần**
/// `user_gs_pair_ok_with` để kiểm cả hai chiều mà không cần `init` (host không có MSR).
#[test]
fn gs_pair_must_match_user_base_and_this_hart_scratch() {
    let scratch = FAKE_KERNEL_GS;
    assert!(
        user_gs_pair_ok_with(0, scratch, scratch),
        "cặp đúng: GS=0, KGS=scratch hart"
    );
    // Ảnh **chưa** khởi tạo cửa `syscall` (product-shell) có `KERNEL_GS_BASE = 0`: trạng thái boot
    // hợp lệ, không được `halt(5)` oan lúc shell xuống CPL3 (tái hiện 2026-11-28).
    assert!(
        user_gs_pair_ok_with(0, 0, scratch),
        "KGS=0 là trạng thái boot của ảnh không dùng syscall"
    );
    // Sai `GS.base`: ring 3 chạy với GS của kernel (đúng lớp X2).
    assert!(!user_gs_pair_ok_with(scratch, scratch, scratch));
    // Sai `KERNEL_GS_BASE`: scratch của hart **khác** ⇒ `syscall` sau lấy stack của hart khác.
    assert!(!user_gs_pair_ok_with(0, scratch + 0x1000, scratch));
    // `gs_is_kernel` là **hàm duy nhất** kiểm được phía kernel. Host test khác
    // (`tests/syscall_fast.rs`) có thể đã `set_kernel_stack` nên **không** giả định scratch = 0:
    // đọc chính giá trị đó rồi kiểm quan hệ, và một giá trị chắc chắn khác.
    assert!(gs_is_kernel(0, gs_scratch(0)));
    assert!(!gs_is_kernel(0, 0xdead_beef_0000));
}

/// **Chốt chống revert** ở tầng mã `kbin` (crate không host-testable): handler **phải** gọi
/// `resume_rip_offset(via_syscall)` (X1) và kiểm `gs_is_kernel` tại cửa vào (X2 vòng 2). Nếu ai
/// quay lại ghi `frame + SYSCALL_FRAME_RIP` cho cả hai cửa, hoặc bỏ phép kiểm GS, test này ĐỎ.
/// **KT-424 #0** — `Switched` phải được sinh từ cửa `syscall` (audit MAJOR-2) và là syscall
/// **sau** `Exit` (MAJOR-1b). Chương trình ring 3 pha core gọi `Tcb.Suspend(Defer)` bằng
/// `syscall`; nếu ai đổi lại `int 0x80` thì nhánh `resume_rip_offset(via_syscall)` lại thành code
/// chết và test này ĐỎ (bằng chứng chạy ở `run-x86_64.sh` ca âm (mm) cũng sẽ bắt).
#[test]
fn core_phase_must_switch_via_syscall() {
    let core = include_str!("../../vifast-kbin-x86_64/src/coretest.rs");
    assert!(
        core.contains("\"mov r10, {defer}\",\n    \"syscall\","),
        "pha core phải gọi Tcb.Suspend(Defer) qua `syscall` (KT-424 #0 / audit MAJOR-2)"
    );
    assert!(
        !core.contains("\"mov r10, {defer}\",\n    \"int 0x80\","),
        "Suspend(Defer) không được quay lại `int 0x80` — nhánh resume cửa nhanh sẽ thành code chết"
    );
}

#[test]
fn kbin_must_use_the_shared_resume_and_gs_checks() {
    let kbin = include_str!("../../vifast-kbin-x86_64/src/umode.rs");
    assert!(
        kbin.contains("resume_rip_offset(via_syscall)"),
        "kbin phải dùng resume_rip_offset(via_syscall) — không tự chọn ô RIP/RCX"
    );
    assert!(
        kbin.contains("gs_is_kernel(hart_in, gs_base_in)"),
        "kbin phải kiểm GS.base tại cửa vào `syscall` bằng gs_is_kernel"
    );
    assert!(
        !kbin.contains("frame + SYSCALL_FRAME_RIP as u64,\n                entry"),
        "không được quay lại ghi ô RIP 136 cho cửa nhanh (X1)"
    );
    let trap = include_str!("../src/asm/trap.rs");
    assert!(
        trap.contains("user_gs_pair_ok(gs_base, kernel_gs_base, hart)"),
        "enter_user phải đòi **cả cặp** MSR, không chỉ GS.base"
    );
}
