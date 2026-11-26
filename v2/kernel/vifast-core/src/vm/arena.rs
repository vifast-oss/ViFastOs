// SPDX-License-Identifier: MIT OR Apache-2.0
//! Arena byte + cửa sổ PA của một Untyped (bump + hole free-list) — KT-52.
//!
//! Tách khỏi `obj/mod.rs` để giữ R1 ≤ 1.500 dòng/tệp; toàn bộ ngữ nghĩa object layer ở
//! [`crate::vm`] (dời từ `vifast-plat-common::obj` ở `KT-417b`; shim plat xoá ở `KT-417d`).

use vifast_alloc::{Frame as PaFrame, FrameArena};

use super::{align_up_u32, ObjError, MAX_HOLES, OBJ_NONE, PAGE_TABLE_BYTES};

// ---------------------------------------------------------------------------------------------
// Dữ liệu tĩnh (Copy, không cấp phát)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
pub(crate) struct Hole {
    pub(crate) off: u32,
    pub(crate) len: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct Untyped {
    pub(crate) live: bool,
    pub(crate) object: u16,
    pub(crate) arena_base: u64,
    pub(crate) capacity: u32,
    pub(crate) bump: u32,
    pub(crate) holes: [Hole; MAX_HOLES],
    pub(crate) n_holes: u8,
    /// Cửa sổ PA (Frame) — **cùng** allocator với `vifast_alloc` (KT-67): object layer không
    /// giữ `pa_used`/`pa_holes` riêng nữa (một nguồn sự thật byte).
    pub(crate) pa: FrameArena,
}

impl Untyped {
    pub(crate) const EMPTY: Self = Self {
        live: false,
        object: OBJ_NONE,
        arena_base: 0,
        capacity: 0,
        bump: 0,
        holes: [Hole { off: 0, len: 0 }; MAX_HOLES],
        n_holes: 0,
        pa: FrameArena::EMPTY,
    };

    fn insert(list: &mut [Hole; MAX_HOLES], n: &mut u8, hole: Hole) -> bool {
        if hole.len == 0 {
            return true;
        }
        let count = *n as usize;
        if count >= MAX_HOLES {
            return false;
        }
        let mut at = count;
        for (i, h) in list.iter().take(count).enumerate() {
            if h.off > hole.off {
                at = i;
                break;
            }
        }
        list.copy_within(at..count, at + 1);
        list[at] = hole;
        *n += 1;
        true
    }

    fn remove(list: &mut [Hole; MAX_HOLES], n: &mut u8, index: usize) -> Hole {
        let count = *n as usize;
        let hole = list[index];
        list.copy_within(index + 1..count, index);
        *n -= 1;
        list[count - 1] = Hole { off: 0, len: 0 };
        hole
    }

    /// Gộp mọi hole dính nhau/trùng (sắp theo `off` rồi merge).
    fn merge_holes(holes: &mut [Hole; MAX_HOLES], n: &mut u8) {
        let count = *n as usize;
        for i in 1..count {
            let mut j = i;
            while j > 0 && holes[j - 1].off > holes[j].off {
                holes.swap(j - 1, j);
                j -= 1;
            }
        }
        let mut i = 0usize;
        while i + 1 < *n as usize {
            let a = holes[i];
            let b = holes[i + 1];
            if a.off as u64 + a.len as u64 >= b.off as u64 {
                let end = (b.off as u64 + b.len as u64).max(a.off as u64 + a.len as u64);
                let start = a.off.min(b.off);
                holes[i] = Hole {
                    off: start,
                    len: (end - start as u64) as u32,
                };
                let _ = Self::remove(holes, n, i + 1);
            } else {
                i += 1;
            }
        }
    }

    /// Rewind watermark nếu hole cuối chạm đúng `watermark` (LIFO thường gặp).
    fn rewind_holes(holes: &mut [Hole; MAX_HOLES], n: &mut u8, watermark: &mut u32) {
        loop {
            let mut found = None;
            for (i, h) in holes.iter().take(*n as usize).enumerate() {
                if h.off as u64 + h.len as u64 == *watermark as u64 {
                    found = Some(i);
                    break;
                }
            }
            match found {
                Some(i) => {
                    let h = Self::remove(holes, n, i);
                    *watermark = h.off;
                }
                None => break,
            }
        }
    }

    /// Gộp hole + rewind watermark của **arena** (`bump`).
    fn normalize_holes(holes: &mut [Hole; MAX_HOLES], n: &mut u8, bump: &mut u32) {
        Self::merge_holes(holes, n);
        Self::rewind_holes(holes, n, bump);
    }

    /// Cấp `len` byte căn `align` trong arena; trả offset. Không panic.
    pub(crate) fn alloc(&mut self, len: u32, align: u32) -> Option<u32> {
        if len == 0 || align == 0 || !align.is_power_of_two() || align > PAGE_TABLE_BYTES {
            return None;
        }
        let mut i = 0usize;
        while i < self.n_holes as usize {
            let hole = self.holes[i];
            let start = align_up_u32(hole.off, align)?;
            let end = (hole.off as u64).checked_add(hole.len as u64)?;
            if start as u64 + len as u64 <= end {
                self.remove_hole_at(i);
                if start > hole.off {
                    Self::insert(
                        &mut self.holes,
                        &mut self.n_holes,
                        Hole {
                            off: hole.off,
                            len: start - hole.off,
                        },
                    );
                }
                let right = start + len;
                if (right as u64) < end {
                    Self::insert(
                        &mut self.holes,
                        &mut self.n_holes,
                        Hole {
                            off: right,
                            len: (end - right as u64) as u32,
                        },
                    );
                }
                return Some(start);
            }
            i += 1;
        }
        let start = align_up_u32(self.bump, align)?;
        if start as u64 + len as u64 > self.capacity as u64 {
            return None;
        }
        // Đệm căn lề là **byte tự do** (hole), không phải "gap" tự khai: bất biến
        // `Σ live + Σ hole == bump` phủ **mọi** byte trong watermark.
        if start > self.bump
            && !Self::insert(
                &mut self.holes,
                &mut self.n_holes,
                Hole {
                    off: self.bump,
                    len: start - self.bump,
                },
            )
        {
            return None;
        }
        self.bump = start + len;
        Some(start)
    }

    /// Trả block về arena (hole + merge + rewind).
    pub(crate) fn free(&mut self, off: u32, len: u32) -> bool {
        if len == 0 {
            return true;
        }
        if off as u64 + len as u64 > self.bump as u64 {
            return false;
        }
        if !Self::insert(&mut self.holes, &mut self.n_holes, Hole { off, len }) {
            return false;
        }
        Self::normalize_holes(&mut self.holes, &mut self.n_holes, &mut self.bump);
        true
    }

    fn remove_hole_at(&mut self, index: usize) {
        let _ = Self::remove(&mut self.holes, &mut self.n_holes, index);
    }

    /// Đáy cửa sổ PA (nhãn, D2-66) — `Frame.pa = pa_base() + offset`.
    pub(crate) fn pa_base(&self) -> u64 {
        self.pa.base()
    }

    /// Cấp PA nhãn `size` byte trong cửa sổ qua [`FrameArena`] (KT-67): **không** đệm căn lề
    /// ngầm (K2 KT-36 `Alignment`) — muốn trộn cỡ thì cấp **giảm dần**.
    pub(crate) fn pa_alloc(&mut self, size: u64) -> Result<u32, ObjError> {
        let bits = pa_size_bits(size).ok_or(ObjError::InvalidArgument)?;
        match self.pa.alloc(bits) {
            Ok(frame) => Ok(frame.off),
            Err(error) => Err(map_alloc_error(error)),
        }
    }

    /// Trả PA nhãn (cửa sổ) — `false` khi Frame chưa từng cấp / bảng hole đầy.
    pub(crate) fn pa_free(&mut self, off: u32, size: u64) -> bool {
        let Some(bits) = pa_size_bits(size) else {
            return false;
        };
        self.pa
            .free(PaFrame {
                off,
                size_bits: bits,
            })
            .is_ok()
    }

    /// Bất biến kế toán cửa sổ PA: caller (object layer) **liệt kê** `(off, len)` của mọi Frame
    /// đang sống từ bảng object của nó rồi uỷ quyền [`FrameArena::check_accounting`] — allocator
    /// **không** giữ bản sao danh sách Frame, nên phép kiểm không phải tautology.
    pub(crate) fn pa_check(&self, live: &[(u32, u32)]) -> bool {
        self.pa.check_accounting(live).is_ok()
    }

    /// Về rỗng (Revoke Untyped): byte arena + watermark PA + hole + đếm Frame.
    pub(crate) fn reset_pa(&mut self) {
        self.pa.reset();
    }
}

/// `size_bits` của một cỡ PA byte (`4096 → 12`, `2 MiB → 21`, `1 GiB → 30`) — **cùng bộ** cỡ
/// Frame của `vifast_pte`/`vifast_alloc`; cỡ khác ⇒ `None` (caller trả `InvalidArgument`).
pub(crate) fn pa_size_bits(size: u64) -> Option<u8> {
    match size {
        4096 => Some(12),
        2_097_152 => Some(21),
        1_073_741_824 => Some(30),
        _ => None,
    }
}

/// Ánh xạ lỗi allocator → lỗi object layer (một bảng, không thêm bảng mã thứ hai).
pub(crate) fn map_alloc_error(error: vifast_alloc::AllocError) -> ObjError {
    use vifast_alloc::AllocError as E;
    match error {
        E::InvalidSize => ObjError::InvalidArgument,
        E::NotEnoughMemory => ObjError::NotEnoughMemory,
        E::Alignment => ObjError::Alignment,
        E::NotAllocated | E::NoHoles | E::Corrupt => ObjError::Corrupt,
    }
}
