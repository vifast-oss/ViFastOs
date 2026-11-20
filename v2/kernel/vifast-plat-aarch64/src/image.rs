// SPDX-License-Identifier: MIT OR Apache-2.0
//! BUILD_ID của ảnh **aarch64** — FNV-1a 64 trên `.text` **đã nạp** (KT-265, khuôn riscv64 D2-80).
//!
//! # Vì sao cần
//!
//! `soak.sh` riscv64 từ chối mọi log không có dòng `# conf image: build=<16 hex> text=<len>` do
//! **kernel tự in** (D2-80): sha256 của tệp ảnh không đủ, vì nó không chứng minh được **đoạn mã
//! đang chạy** là đoạn mã trong tệp. Ảnh aarch64 (KT-247..KT-262) chưa in dòng ấy — KT-265 thêm
//! vào để `soak-aarch64.sh` có cùng bất biến với `soak.sh`.
//!
//! # Cách đo
//!
//! ```text
//! __text_start ──► đọc từng byte bằng asm::read_u8 ──► FNV-1a 64 ──► build=<16 hex>
//! __text_end   ──► len = end - start ─────────────────────────────► text=<len>
//! ```
//!
//! `test/conformance/image_digest.py` băm **cùng** section `.text` từ tệp ELF bằng **cùng** hằng
//! FNV ⇒ runner/soak so được hai phía mà không cần tin nhau.
//!
//! # Vì sao đọc từng **byte** (không phải `u64`)
//!
//! `__text_end` không nhất thiết căn 8 (đo được: `text=37044` ⇒ `end = 0x4008_90B4`). Đọc `u64`
//! ở đoạn đuôi sẽ vượt qua `__text_end` và băm cả byte của section kế tiếp ⇒ hash **khác** bản
//! băm từ ELF. Đọc byte là cách duy nhất khớp tuyệt đối.
//!
//! # `unsafe`
//!
//! Tệp này **không** có `unsafe`: phép đọc nằm ở [`crate::asm::read_u8`] — mô-đun duy nhất của
//! crate được `#![allow(unsafe_code)]` (D2-17 / luật R5). Lấy **địa chỉ** symbol linker bằng
//! `core::ptr::addr_of!` là an toàn (không tạo reference, không đọc giá trị).
//!
//! # Host
//!
//! Trên host không có bản đồ của kernel và không có symbol linker ⇒ [`image_digest`] **panic**
//! (fail-loud, cùng khuôn [`crate::asm`]) thay vì trả một hash giả. Phần **thuần** ([`fnv1a64`])
//! test được trên host với vector chuẩn của FNV.

/// Offset basis của FNV-1a 64 (đặc tả FNV, cùng hằng với `image_digest.py`).
pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// Prime của FNV-1a 64 (đặc tả FNV, cùng hằng với `image_digest.py`).
pub const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Một bước FNV-1a 64 trên **một** byte — tách ra để vòng đọc bộ nhớ và phép băm không lệch nhau.
#[inline]
pub fn fnv1a64_update(hash: u64, byte: u8) -> u64 {
    (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
}

/// FNV-1a 64 của một lát byte (hàm **thuần**, test host được với vector chuẩn).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for &byte in bytes {
        hash = fnv1a64_update(hash, byte);
    }
    hash
}

/// In `# conf image: build=<16 hex> text=<len>` — dòng **D2-80** buộc runner/soak đối chiếu với
/// ảnh (khuôn `vifast_plat_riscv64::report_image_digest`).
///
/// Gọi **một lần**, ngay sau `1..N` của TAP: dòng này là **bằng chứng neo ảnh**, không phải một
/// ca kiểm — nó phải có mặt kể cả khi boot hỏng ở ca sau.
pub fn report_image_digest() {
    let (hash, len) = image_digest();
    let mut buf = [0u8; 96];
    let mut n = 0usize;
    push(&mut buf, &mut n, b"# conf image: build=");
    let mut hex = [0u8; 16];
    for (i, h) in hex.iter_mut().enumerate() {
        let nib = ((hash >> ((15 - i) * 4)) & 0xf) as u8;
        *h = if nib < 10 {
            b'0' + nib
        } else {
            b'a' + nib - 10
        };
    }
    push(&mut buf, &mut n, &hex);
    push(&mut buf, &mut n, b" text=");
    let mut dec = [0u8; 20];
    let mut t = 0usize;
    let mut v = len;
    if v == 0 {
        dec[t] = b'0';
        t += 1;
    }
    while v > 0 {
        dec[t] = b'0' + (v % 10) as u8;
        v /= 10;
        t += 1;
    }
    while t > 0 {
        t -= 1;
        push(&mut buf, &mut n, &[dec[t]]);
    }
    push(&mut buf, &mut n, b"\n");
    crate::pl011::write_str(core::str::from_utf8(&buf[..n]).unwrap_or(""));
}

/// Chép `bytes` vào `buf` (không bao giờ vượt `buf.len()` — dòng in ra ngắn hơn 96 byte).
fn push(buf: &mut [u8; 96], n: &mut usize, bytes: &[u8]) {
    for &byte in bytes {
        if *n < buf.len() {
            buf[*n] = byte;
            *n += 1;
        }
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
mod imp {
    extern "C" {
        /// Đầu output section `.text` (`linker.ld`) — **địa chỉ**, không phải giá trị.
        static __text_start: u8;
        /// Cuối output section `.text` (`linker.ld`) — **địa chỉ**, không phải giá trị.
        static __text_end: u8;
    }

    /// FNV-1a 64 trên `[__text_start, __text_end)` của **chính ảnh đang chạy** + độ dài `.text`.
    ///
    /// Trả `(hash, len)`; `len` là số byte **đã băm** (khớp `text=` mà `image_digest.py` tính từ
    /// section `.text` của tệp ELF).
    pub fn image_digest() -> (u64, usize) {
        // `addr_of!` **không** tạo reference và **không** đọc bộ nhớ ⇒ an toàn (không cần
        // `unsafe`); giá trị đọc được là địa chỉ do linker gán.
        let start = core::ptr::addr_of!(__text_start) as usize;
        let end = core::ptr::addr_of!(__text_end) as usize;
        let mut hash = super::FNV_OFFSET;
        let mut p = start;
        while p < end {
            hash = super::fnv1a64_update(hash, crate::asm::read_u8(p as u64));
            p += 1;
        }
        (hash, end.saturating_sub(start))
    }
}

#[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
mod imp {
    /// Stub host — **panic**: trên host không có bản đồ của kernel và không có symbol linker.
    pub fn image_digest() -> (u64, usize) {
        panic!("vifast-plat-aarch64: image_digest chỉ chạy trên aarch64 bare-metal (host stub, KT-265)")
    }
}

pub use imp::image_digest;
