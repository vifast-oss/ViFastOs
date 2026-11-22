// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-338c** (D2-08) — nhánh **ảnh sản phẩm** của `vifast-kbin-aarch64`.
//!
//! # Vì sao tách bằng **feature `ktest`** (và vì sao nhánh này trả `!`)
//!
//! Trước KT-338c, `vifast-kbin-aarch64` là **một** ảnh vừa sản phẩm vừa harness: `kmain` in
//! `TAP version 13` + `1..35` rồi chạy 35 ca kiểm — trái D2-08 ("test kernel không nằm trong boot
//! sản phẩm"). Ảnh sản phẩm ở đây build **không** feature `ktest`; `kmain` rẽ vào [`boot`]
//! **trước** dòng TAP đầu tiên. Vì [`boot`] trả `!`, toàn bộ mã kiểm phía sau `kmain` là **không
//! thể tới** trong ảnh sản phẩm ⇒ bị MIR/LLVM loại khỏi ảnh. Đây không phải lời hứa: runner
//! `v2/test/boot/run-aarch64.sh` quét **bằng máy** (`strings | grep -c`) và có **ca âm** (ảnh
//! `ktest` phải đếm > 0), cộng đột biến feature `mut-harness-marker` chèn **một** marker vào ảnh
//! sản phẩm ⇒ bộ quét phải ĐỎ.
//!
//! # Phạm vi ảnh sản phẩm ở bước này (ghi thẳng, không tô hồng)
//!
//! **Có**: `_start` (stack + zero `.bss`, asm trong `main.rs`), PL011 115200 8N1, bảng **16
//! vector** cài thật (`VBAR_EL1`, kiểm căn `0x800`), neo ảnh D2-80 (`# conf image:`), marker
//! **một lần**, halt qua PSCI `SYSTEM_OFF` (QEMU exit 0).
//!
//! **Chưa** (thuộc `KT-338b`): MMU/`VSpace`/physmap, GICv3/SMP, U-mode, PCI/virtio, W^X. Các pha
//! ấy hiện **chỉ** chạy trong ảnh `ktest` vì chúng gắn với phép đo TAP; `KT-338b` (vifast-core +
//! IPC EL0) sẽ dựng nhánh sản phẩm thật và chuyển dần phần khởi tạo thuần sang đây. Ghi rõ để
//! không ai đọc báo cáo thành "sản phẩm đã đủ".

use core::fmt::Write;

use vifast_plat_aarch64::{debug_halt, image, pl011, vector};

use crate::{vector_base, Console, BOOT_MARKER};

/// `true` ⇔ ảnh này là ảnh **kiểm** (`--features ktest`). Ảnh sản phẩm = `false`.
///
/// `cfg!` là hằng biên dịch: nhánh còn lại bị loại ngay ở MIR, nên ảnh sản phẩm **không** mang
/// chuỗi/mã harness (không phải "tắt lúc chạy").
pub(crate) const KTEST: bool = cfg!(feature = "ktest");

/// Dòng halt của ảnh sản phẩm — runner đòi **đúng** dòng này sau marker (khuôn `x86_64` của
/// KT-340a, để hai arch có cùng định nghĩa "boot sản phẩm xong").
pub(crate) const HALT_LINE: &str = "# viFastOs aarch64: halt";

/// Khởi tạo **sản phẩm** tới marker rồi halt — không TAP, không gọi hàm kiểm nào.
///
/// Trả `!` là **hợp đồng** với `kmain`: mọi dòng sau điểm gọi (toàn bộ harness) là unreachable
/// trong ảnh sản phẩm. Đổi kiểu trả về thành `()` sẽ làm harness quay lại ảnh — bộ quét `strings`
/// của runner bắt được ngay.
pub(crate) fn boot(_dtb: u64, console: &mut Console) -> ! {
    // **Đột biến (i) của KT-338c** — chèn **một** dòng marker harness vào ảnh sản phẩm: bộ quét
    // `harness-scan` của runner phải ĐỎ. Chỉ dùng cho đột biến, **không** phải feature sản phẩm.
    #[cfg(feature = "mut-harness-marker")]
    let _ = writeln!(console, "TAP version 13");
    // Ghim mảng lớn vào `.bss` (đột biến ii) — `black_box` giữ nó khỏi bị `--gc-sections` cắt.
    #[cfg(feature = "mut-big-bss")]
    let _ = core::hint::black_box(MUT_BIG_BSS[0].load(core::sync::atomic::Ordering::Relaxed));

    // PL011: cùng chuỗi khởi tạo DDI 0183 như ảnh kiểm — nếu tắt/chia tần sai thì không in được
    // gì, nên đây là điều kiện **đầu tiên** của một boot sản phẩm.
    let steps = pl011::init();
    // D2-80 — neo ảnh: FNV-1a 64 trên `.text` đã nạp + độ dài, cùng định dạng với ảnh ktest để
    // `soak-aarch64.sh` dùng **một** bộ đọc log cho cả hai ảnh.
    image::report_image_digest();

    // Bảng vector **thật** — nền sản phẩm, không phải phép kiểm: trap sau đây vào handler của
    // kernel thay vì "không có gì".
    let base = vector_base();
    let vbar = vector::install(base);
    let aligned = vector::is_aligned(base);
    let _ = writeln!(
        console,
        "# viFastOs aarch64: product pl011={steps}/{} vbar=0x{vbar:x} slots={} aligned={}",
        pl011::INIT_SEQUENCE.len(),
        vector::VECTOR_COUNT,
        aligned as u8,
    );

    if steps != pl011::INIT_SEQUENCE.len() || vbar != base || !aligned {
        let _ = writeln!(
            console,
            "PANIC: product: pl011/vector chưa sẵn sàng (init={steps} vbar=0x{vbar:x} base=0x{base:x} aligned={})",
            aligned as u8,
        );
        debug_halt(1)
    }

    // Marker đúng **một** lần + halt sạch (PSCI SYSTEM_OFF ⇒ QEMU exit 0).
    let _ = console.write_str(BOOT_MARKER);
    let _ = writeln!(console, "{HALT_LINE}");
    debug_halt(0)
}

/// **KT-338c** (D2-08) — handler trap của ảnh **sản phẩm**: chẩn đoán rồi halt, **không** nuôi
/// đường kiểm. Handler đầy đủ (BRK/IRQ GICv3/fault được chờ/U-mode) chỉ có trong ảnh `ktest`
/// (`trap.rs`): nhờ vậy **mọi** mã harness mất **gốc tham chiếu** ở ảnh sản phẩm và bị loại.
#[cfg(not(feature = "ktest"))]
#[no_mangle]
pub extern "C" fn vec_handler(
    slot: u64,
    esr_raw: u64,
    far: u64,
    elr: u64,
    spsr: u64,
    _frame: u64,
) -> u64 {
    let mut console = Console;
    let _ = writeln!(
        console,
        "# exc: slot={slot} esr=0x{esr_raw:016x} far=0x{far:x} elr=0x{elr:x} spsr=0x{spsr:x}"
    );
    debug_halt(2)
}

/// **Đột biến (ii) của KT-338c** — `.bss` cố ý vượt trần **64 KiB** của ảnh sản phẩm để chứng
/// minh `budget-aarch64.sh --product` **có răng** (12.288 × 8 B = 96 KiB). Chỉ dùng cho đột biến.
#[cfg(feature = "mut-big-bss")]
#[used]
static MUT_BIG_BSS: [core::sync::atomic::AtomicU64; 12_288] =
    [const { core::sync::atomic::AtomicU64::new(0) }; 12_288];
