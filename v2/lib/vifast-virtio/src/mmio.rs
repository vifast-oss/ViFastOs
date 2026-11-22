// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Cổng MMIO** cho transport VirtIO-PCI — phần **thuần** (KT-334).
//!
//! # Vì sao có trait này
//!
//! Trước KT-334, mỗi `vifast-plat-*` tự viết `read8/read16/read32/write8/…` bằng con trỏ thô rồi
//! chép **nguyên** chuỗi lập trình thanh ghi common config của virtio-pci vào `virtio_blk.rs`.
//! Ba bản sao của cùng một chuỗi là ba chỗ để lệch nhau (thứ tự status, feature hai trang, …
//! — đúng lớp lỗi D2-110/CAO-1). Transport chung vì thế chỉ nói chuyện với thiết bị **qua**
//! trait này; crate `vifast-virtio` vẫn `#![forbid(unsafe_code)]`, còn `unsafe` (con trỏ thô +
//! asm rào) nằm ở adapter của từng `kernel/vifast-plat-*` (D2-17).
//!
//! # Hợp đồng
//!
//! - Mọi hàm nhận **địa chỉ tuyệt đối** (BAR base + offset) do transport tính.
//! - `read*`/`write*` phải là truy cập **volatile** ở phía adapter (thiết bị MMIO, không phải RAM).
//! - [`MmioBus::fence`] là hàng rào thứ tự **trước khi bàn giao ring** (spec virtio §2.7.13).
//!   Mặc định no-op để adapter không bắt buộc cài; adapter thật **phải** cài (riscv `fence rw,rw`,
//!   aarch64 `dsb sy`, x86 `mfence`).
//! - [`MmioBus::notify`] là cửa thông báo: mặc định ghi 16 bit index queue — đúng layout
//!   virtio-pci §4.1.4.5. Tách khỏi `write16` để adapter có thể chèn rào/thống kê tại **một** chỗ.

/// Truy cập MMIO của thiết bị virtio-pci (BAR đã map) — do `kernel/vifast-plat-*` cài.
pub trait MmioBus {
    /// Đọc 8 bit (thanh ghi `device_status`, `config_generation`, byte device config).
    fn read8(&self, addr: u64) -> u8;
    /// Đọc 16 bit (`num_queues`, `queue_size`, `queue_notify_off`, `used->idx`).
    fn read16(&self, addr: u64) -> u16;
    /// Đọc 32 bit (`device/queue feature`, `queue_select` trả về).
    fn read32(&self, addr: u64) -> u32;
    /// Ghi 8 bit.
    fn write8(&mut self, addr: u64, value: u8);
    /// Ghi 16 bit.
    fn write16(&mut self, addr: u64, value: u16);
    /// Ghi 32 bit.
    fn write32(&mut self, addr: u64, value: u32);
    /// Ghi 64 bit (`queue_desc`/`queue_driver`/`queue_device`).
    fn write64(&mut self, addr: u64, value: u64);

    /// Hàng rào thứ tự bộ nhớ trước khi bàn giao ring cho thiết bị.
    ///
    /// Adapter **phải** cài: thiếu rào, thiết bị có thể thấy `avail->idx` trước khi ring xong
    /// (spec §2.7.13). Mặc định no-op chỉ để test host không phải cài.
    fn fence(&self) {}

    /// Ghi index queue vào cửa thông báo (doorbell).
    fn notify(&mut self, addr: u64, queue: u16) {
        self.write16(addr, queue);
    }

    /// Đọc `out.len()` byte liên tiếp từ `base` — device config của virtio-blk là chuỗi byte.
    fn read_bytes(&self, base: u64, out: &mut [u8]) {
        for (index, slot) in out.iter_mut().enumerate() {
            *slot = self.read8(base + index as u64);
        }
    }
}
