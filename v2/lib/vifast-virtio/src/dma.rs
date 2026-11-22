// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Vùng DMA** cho transport VirtIO-PCI — phần **thuần** (KT-334, D2-239(1)).
//!
//! # Vì sao có trait này
//!
//! Thiết bị virtio DMA bằng **địa chỉ vật lý** vào ring/đệm mà driver dựng trong RAM. Ba
//! `vifast-plat-*` trước KT-334 mỗi bên tự giữ một trang `.bss` 4 KiB, tự đổi VA → PA (riscv64/
//! x86_64 identity; aarch64 alias nửa cao) và tự chèn cache maintenance — rồi chép cùng một
//! chuỗi khởi tạo. Trait này gom **cấp phát** vùng vật lý liên tục + **hai hook cache** về một
//! chỗ, để transport chung không cần biết kiến trúc.
//!
//! # `sync_for_device` / `sync_for_cpu` là **bắt buộc**
//!
//! D2-239(1) chốt: `DmaPool` **phải** có đúng hai hook này để giữ nguyên bảo trì cache của
//! aarch64 (KT-277): `dc civac` trước khi bàn giao, `dc ivac` sau khi thiết bị báo xong. Trên
//! riscv64/x86_64 chúng là no-op (không có cache không nhất quán với DMA), nhưng **không** được
//! bỏ khỏi trait — bỏ là mất đường duy nhất để aarch64 cài đúng, và test host đếm lời gọi sẽ
//! không còn chỗ bám.
//!
//! # Hợp đồng offset
//!
//! `off` của [`DmaPool::sync_for_device`]/[`DmaPool::sync_for_cpu`] là offset **tính từ đầu
//! pool** (không phải từ đầu `DmaBuf`). Transport cộng `DmaBuf::off` trước khi gọi, nên pool
//! nhiều vùng vẫn đúng.

/// Cỡ trang DMA mà virtio-blk dùng cho ring + request + data + status.
pub const DMA_PAGE_BYTES: usize = 4096;

/// Một vùng DMA **vật lý liên tục** đã cấp từ [`DmaPool`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DmaBuf {
    /// Offset trong pool (byte).
    pub off: usize,
    /// Cỡ vùng (byte).
    pub len: usize,
    /// Địa chỉ vật lý của byte đầu vùng (= `pool.base_pa() + off`).
    pub pa: u64,
}

impl DmaBuf {
    /// Offset byte cuối (không gồm).
    pub const fn end(&self) -> usize {
        self.off + self.len
    }
}

/// Pool cấp vùng DMA vật lý liên tục + hai hook bảo trì cache (D2-239(1)).
pub trait DmaPool {
    /// PA của byte đầu pool.
    fn base_pa(&self) -> u64;

    /// Tổng cỡ pool (byte).
    fn capacity(&self) -> usize;

    /// Toàn bộ pool dưới dạng lát byte ghi được (driver chép request/đọc data ở đây).
    fn bytes(&mut self) -> &mut [u8];

    /// Cấp `len` byte căn lề `align` (luỹ thừa 2). `None` nếu pool không đủ — fail-closed.
    fn alloc(&mut self, len: usize, align: usize) -> Option<DmaBuf>;

    /// Thu hồi vùng **vừa cấp sau cùng** (LIFO). `false` nếu không phải vùng đỉnh.
    fn free(&mut self, buf: DmaBuf) -> bool;

    /// Bàn giao `[off, off+len)` cho thiết bị: làm sạch cache (aarch64 `dc civac` + barrier).
    fn sync_for_device(&mut self, off: usize, len: usize);

    /// Nhận lại `[off, off+len)` từ thiết bị: bỏ cache (aarch64 `dc ivac` + barrier).
    fn sync_for_cpu(&mut self, off: usize, len: usize);

    /// Đọc 16 bit **volatile** tại offset `off` trong pool (driver đọc `used->idx` qua DMA).
    fn read_u16(&self, off: usize) -> u16;

    /// Đọc 8 bit **volatile** tại offset `off` trong pool (byte status do thiết bị ghi).
    fn read_u8(&self, off: usize) -> u8;

    /// Zero một vùng đã cấp (dùng trước khi bàn giao ring lần đầu).
    fn zero(&mut self, buf: DmaBuf) {
        let at = buf.off;
        let end = buf.end().min(self.bytes().len());
        if at <= end {
            self.bytes()[at..end].fill(0);
        }
    }
}

/// Bộ cấp phát **bump** cho một vùng liên tục — dùng trong adapter `vifast-plat-*` (một trang
/// `.bss`) và trong test host (mảng `Vec`).
///
/// `free` chỉ thu hồi được vùng **đỉnh** (LIFO). Đây là đủ cho driver một request một lúc; nếu
/// sau này cần cấp/thu xen kẽ, phải thay bằng allocator thật — **không** nới hợp đồng này.
#[derive(Clone, Copy, Debug, Default)]
pub struct DmaArena {
    cursor: usize,
    capacity: usize,
}

impl DmaArena {
    /// Arena rỗng trên `capacity` byte.
    pub const fn new(capacity: usize) -> Self {
        Self {
            cursor: 0,
            capacity,
        }
    }

    /// Cấp `len` byte căn `align`; PA = `base_pa + off`.
    pub fn alloc(&mut self, base_pa: u64, len: usize, align: usize) -> Option<DmaBuf> {
        if len == 0 || align == 0 || !align.is_power_of_two() {
            return None;
        }
        let off = self.cursor.checked_add(align - 1)? & !(align - 1);
        let end = off.checked_add(len)?;
        if end > self.capacity {
            return None;
        }
        self.cursor = end;
        Some(DmaBuf {
            off,
            len,
            pa: base_pa + off as u64,
        })
    }

    /// Thu hồi nếu `buf` là vùng đỉnh; ngược lại `false` (không đoán).
    pub fn free(&mut self, buf: DmaBuf) -> bool {
        if buf.end() == self.cursor && buf.end() <= self.capacity {
            self.cursor = buf.off;
            true
        } else {
            false
        }
    }

    /// Xoá toàn bộ cấp phát (giữ nguyên cỡ pool).
    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    /// Số byte đã cấp.
    pub const fn used(&self) -> usize {
        self.cursor
    }

    /// Tổng cỡ pool.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}
