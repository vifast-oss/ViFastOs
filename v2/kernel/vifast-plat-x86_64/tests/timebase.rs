// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast-plat-x86_64::timebase` — KT-274 (K5' bước 5).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test timebase
//! ```
//!
//! # Vì sao cần một đồng hồ riêng cho bước mạng
//!
//! Driver virtio-net phải **fail-closed** khi không có reply (không treo tới hết ngân sách của
//! runner). Nguồn tick của ảnh là PIT kênh 0 → IOAPIC → LAPIC, nhưng đường đó cần **ngắt bật**;
//! bước mạng chạy sau khi `cli` và trước U-mode nên không dựa vào nó được. PIT **kênh 2** đã được
//! `measure_timer` lập trình mode 2 divisor `0xFFFF` và **không** phát ngắt ⇒ dùng nó làm đồng hồ
//! đếm-lùi tự do: mỗi lần đọc là một ảnh 16 bit, cộng dồn chênh lệch (xử lý bọc) ra số count đã
//! trôi qua, rồi đổi sang µs bằng tần số PIT thật (`1_193_182 Hz`).
//!
//! Test này chốt **số học** của phép cộng dồn + đổi đơn vị; phần chạm cổng I/O do QEMU đo.

use vifast_plat_x86_64::timebase::{
    elapsed_us, PitClock, PIT_CH2_DIVISOR, PIT_CH2_WRAP_COUNTS, PIT_FREQUENCY_HZ,
};

#[test]
fn pit_clock_accumulates_countdown_and_wraps() {
    let mut clock = PitClock::new();
    clock.reset(0xffff);
    // Đếm lùi bình thường: 0xffff → 0xff00 = 0xff count.
    assert_eq!(clock.update(0xff00), 0x00ff);
    // 0xff00 → 0xfe00 = 0x100 count nữa ⇒ tổng 0x1ff.
    assert_eq!(clock.update(0xfe00), 0x01ff);
    // Đọc **cùng** giá trị hai lần ⇒ 0 count (không đếm thừa).
    assert_eq!(clock.update(0xfe00), 0x01ff);
    // 0xfe00 → 0x0100 = 0xfd00 count (vẫn đếm lùi, chưa bọc).
    assert_eq!(clock.update(0x0100), 0x01ff + 0xfd00);
    // 0x0100 → 0xfff0: counter **bọc** (nạp lại 0xffff) ⇒ 0x100 + 0x10 = 0x110 count.
    assert_eq!(clock.update(0xfff0), 0x01ff + 0xfd00 + 0x110);
    // `reset` xoá bộ đếm nhưng giữ ảnh hiện tại làm mốc.
    clock.reset(0x8000);
    assert_eq!(clock.update(0x7fff), 1);
}

#[test]
fn pit_clock_converts_counts_to_microseconds() {
    // Một giây = đúng tần số PIT (1_193_182 count) ⇒ 1_000_000 µs.
    assert_eq!(elapsed_us(PIT_FREQUENCY_HZ as u64), 1_000_000);
    // 0 count ⇒ 0 µs (không có sai số âm).
    assert_eq!(elapsed_us(0), 0);
    // 100 Hz = 11_932 count ⇒ ~10 ms (sai số làm tròn < 1 µs).
    let ten_ms = elapsed_us(11_932);
    assert!((9_999..=10_001).contains(&ten_ms), "ten_ms={ten_ms}");
    // Một chu kỳ đầy của kênh 2 (65_536 count) ≈ 54,9 ms.
    let period = elapsed_us(PIT_CH2_WRAP_COUNTS);
    assert!((54_900..=54_950).contains(&period), "period={period}");
    // Đơn điệu: count lớn hơn ⇒ µs không nhỏ hơn.
    assert!(elapsed_us(2 * PIT_FREQUENCY_HZ as u64) > elapsed_us(PIT_FREQUENCY_HZ as u64));
}

#[test]
// Hằng số **phải** được chốt bằng giá trị tuyệt đối (đổi hằng là đổi hợp đồng phần cứng) — đó
// chính là mục đích của ca này, nên `assertions_on_constants` là cảnh báo sai ở đây.
#[allow(clippy::assertions_on_constants)]
fn pit_channel2_constants_match_programmed_divisor() {
    // `measure_timer` lập trình kênh 2 với divisor lớn nhất ⇒ một chu kỳ = 0x1_0000 count.
    assert_eq!(PIT_CH2_DIVISOR, 0xffff);
    assert_eq!(PIT_CH2_WRAP_COUNTS, 0x1_0000);
    // Tần số PIT là hằng của phần cứng PC (14,31818 MHz / 12), không phải số đo.
    assert_eq!(PIT_FREQUENCY_HZ, 1_193_182);
    // Chu kỳ kênh 2 phải **lớn hơn** khoảng cách hai lần đọc điển hình (một vòng poll) — nếu
    // không, bộ đếm mất vòng và timeout thành vô nghĩa.
    assert!(PIT_CH2_WRAP_COUNTS > 1_000);
}
