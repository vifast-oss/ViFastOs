// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-415** — pha đo **tranh khoá Big Kernel Lock** trên N hart (feature `bench-bkl`).
//!
//! # Vì sao có tệp này
//!
//! `D2-259` mục 6 ghi BKL toàn cục (`IrqMcsLock<Machine>`) là **nút cổ chai** của SMP: mọi hart
//! phải xếp hàng một khoá cho mọi thao tác core. Quyết định mở khoá theo hart / đa cụm `S3` cần
//! **số đo**, không phải cảm nhận — nên pha này chạy **cùng một vòng việc** trên N hart thật
//! (QEMU `-smp N`) rồi in bộ đếm `acquired`/`contended` của chính khoá.
//!
//! # Số đo, không phải hằng
//!
//! `IrqMcsLock` (KT-411) đã đếm `acquired` (lấy được khoá) và `contended` (try nhanh thất bại ⇒
//! phải xếp hàng MCS). Tỷ lệ chờ = `contended / acquired` — in dạng **phần triệu** (số nguyên,
//! `no_std`, so được bằng máy). Một hart đơn lẻ cho `contended = 0`; càng nhiều hart cùng vào
//! vùng găng thì tỷ lệ càng cao — đó là **đại lượng** dùng làm tiêu chuẩn kích hoạt khoá Per-Hart.
//!
//! # Vì sao feature riêng
//!
//! Ảnh sản phẩm **không** feature phải giữ nguyên từng byte (D2-08/D2-109): pha bench chỉ tồn tại
//! khi bật `bench-bkl`, và `v2/tools/bench-bkl.sh` là nơi duy nhất bật nó.
//!
//! # Không treo máy
//!
//! Chờ hart khác xong có **trần** vòng quay; hết trần thì vẫn in số đo (kèm `done=` để biết hart
//! nào chưa xong) — bench **không** được biến thành deadlock.

use core::fmt::Write;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::{core_bridge, smp};

/// Số vòng mỗi hart — đủ để 4 hart chồng nhau thật mà vẫn xong trong vài giây.
///
/// **Vì sao 2.000 chứ không 20.000**: dưới `-icount` QEMU chạy các vCPU **luân phiên** và đồng hồ
/// ảo tiến theo **số lệnh**, nên mỗi vòng quay chờ khoá bị **tính** vào ngân sách lệnh — 20.000
/// vòng × 4 hart làm boot vượt trần 30 s (đo được: treo ở `ok 1 - k3 boot`). 2.000 vòng vẫn đủ
/// để mọi hart chồng nhau (tỷ lệ chờ bão hoà từ vài trăm vòng) và xong trong trần.
pub const OPS: u32 = 2_000;

/// Trần vòng quay chờ hart khác xong (không treo máy).
pub const WAIT_LIMIT: u64 = 400_000_000;

/// Bitmask hart đã **sẵn sàng** vào pha đo (BSP đặt sau khi mọi hart online).
static START: AtomicU32 = AtomicU32::new(0);
/// Bitmask hart đã **xong** vòng việc.
static DONE: AtomicU32 = AtomicU32::new(0);
/// Tổng số vòng đã chạy (mọi hart) — bằng chứng "hart thật sự làm việc", không phải hằng.
static OPS_DONE: AtomicU64 = AtomicU64::new(0);

/// Vòng việc của **một** hart: `OPS` lần vào vùng găng của Big Kernel Lock.
///
/// Vùng găng **ngắn có chủ đích** (một phép đọc `hart_count`) để phép đo nói về **khoá**, không
/// về lượng việc bên trong khoá.
fn work() {
    let mut n = 0u64;
    for _ in 0..OPS {
        let _ = core_bridge::with_machine(|m| m.hart_count());
        n += 1;
    }
    OPS_DONE.fetch_add(n, Ordering::Relaxed);
}

/// Entry hart **thứ cấp** trong pha bench: chờ `START`, chạy vòng việc, báo `DONE`, rồi park.
///
/// Trả `!` — hart thứ cấp không bao giờ quay lại đường boot sản phẩm.
pub fn secondary(hart: usize) -> ! {
    let bit = 1u32 << (hart & 31);
    while START.load(Ordering::Acquire) & bit == 0 {
        core::hint::spin_loop();
    }
    work();
    DONE.fetch_or(bit, Ordering::Release);
    loop {
        core::hint::spin_loop();
    }
}

/// Chạy pha đo trên `harts` hart **online**; in `# bench: kind=bkl …` rồi trả `true` ⇔ mọi hart
/// đã xong trong trần.
///
/// `harts` bị kẹp theo `smp::online_mask()` — không bịa hart chưa online (số đo sẽ sai).
pub fn run<P: Platform>(plat: &P, harts: usize) -> bool {
    let online = smp::online_mask() as u32;
    let want = harts.clamp(1, vifast_plat_common::smp::MAX_HARTS);
    let mask = if want >= 32 {
        u32::MAX
    } else {
        (1u32 << want) - 1
    } & online;
    let boot = smp::boot_hart();
    let boot_bit = 1u32 << (boot & 31);

    // Phát START cho **mọi** hart trừ BSP (BSP chạy vòng việc ngay dưới).
    START.store(mask & !boot_bit, Ordering::Release);
    work();
    DONE.fetch_or(boot_bit, Ordering::Release);

    let mut spins = 0u64;
    while DONE.load(Ordering::Acquire) & mask != mask {
        spins += 1;
        if spins >= WAIT_LIMIT {
            break;
        }
        core::hint::spin_loop();
    }
    let done = DONE.load(Ordering::Acquire) & mask;
    let stats = core_bridge::lock_stats();
    let ops = OPS_DONE.load(Ordering::Relaxed);
    let mut w = PlatformWriter(plat);
    let _ = writeln!(
        w,
        "# bench: kind=bkl harts={} ops={} acquired={} contended={} in_cs={} overlap={} ratio_ppm={} done=0x{:x} mask=0x{:x}",
        mask.count_ones(),
        ops,
        stats.acquired,
        stats.contended,
        stats.in_cs,
        stats.overlap,
        stats.wait_ratio_ppm(),
        done,
        mask
    );
    done == mask
}
