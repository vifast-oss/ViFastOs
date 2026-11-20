// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
#![forbid(unsafe_code)]
//! `vifast-alloc` — allocator **Frame trên Untyped** của K6 (KT-67, D2-95).
//!
//! # Vì sao có crate này (một nguồn sự thật byte)
//!
//! `Frame` của v2 **không** phải vùng nhớ cấp phát: `pa = pa_base + offset` là **nhãn PA** trong
//! cửa sổ của Untyped (D2-66) — Frame tiêu **0 byte arena**. Việc cấp/thu hồi Frame vì vậy là
//! **số học trên cửa sổ PA** (watermark + free-list hole), không phải allocator heap.
//!
//! KT-52 cài số học đó **bên trong** `vifast-plat-common::obj::arena`; KT-67 phải dùng lại đúng
//! số học ấy cho `memsrv`/cầu nối D2-83. Nếu chép sang một bản thứ hai thì hai bên có thể lệch
//! nhau (hai nguồn sự thật byte) ⇒ crate này là **bản duy nhất**: object layer
//! (`vifast_plat_common::obj`) **bỏ** `pa_used`/`pa_holes` của mình và giữ một [`FrameArena`],
//! nên `UntypedStats`/`check_objmem` của obj layer và mọi caller (memsrv, TAP) đọc **cùng một**
//! trạng thái. `check_objmem` gọi [`FrameArena::check_accounting`] thay vì tự soát lại hole.
//!
//! # Ngữ nghĩa (giữ nguyên KT-52/D2-66)
//!
//! - Cỡ Frame: `size_bits ∈ {12, 21, 30}` (4 KiB / 2 MiB / 1 GiB), như `vifast_pte::FrameSize`.
//!   `size_bits` khác ⇒ [`AllocError::InvalidSize`] (obj layer đổi thành `InvalidArgument`).
//! - **Không đệm căn lề ngầm**: cửa sổ căn cỡ lớn nhất muốn cấp; muốn trộn cỡ thì cấp **giảm
//!   dần**. Bump mà phải đệm căn lề ⇒ [`AllocError::Alignment`], **không** tiêu byte (K2 KT-36).
//! - Cấp: quét hole (first-fit, có kiểm căn **tuyệt đối** `(base+off) % size == 0`) rồi mới bump.
//! - Trả: thêm hole + gộp hole dính nhau + **rewind** watermark nếu hole chạm đúng mép, nên
//!   `Σ Frame sống + Σ hole == used` luôn đúng — đây là bất biến [`FrameArena::check_accounting`].
//! - Mọi lỗi **nguyên tử**: `alloc`/`free` không đổi trạng thái khi trả `Err`.
//! - Không panic, không `unsafe`, không cấp phát động, chạy được cả `no_std` (kernel) và host
//!   (test/sim) — luật D2-17/R5 giữ `unsafe` ra ngoài crate này.
//!
//! # Giới hạn của mô hình
//!
//! [`FrameArena`] **không** biết Frame nào đang bị map, cũng không giữ cây cap: chủ sở hữu Frame
//! là object layer (D2-83), và nó tự kiểm "Frame còn mapping ⇒ `HasChildren`". Crate chỉ kế toán
//! **byte cửa sổ** + số Frame sống theo cỡ; `check_accounting` nhận `live_bytes` do chủ sở hữu
//! tính từ bảng object của nó để đối chiếu (một chiều, không lưu bản sao thứ hai).

/// Cửa sổ PA của một Untyped: **1 GiB** `[TK]` (D2-66) — `Frame.pa = base + offset`.
pub const WINDOW_BYTES: u32 = 1 << 30;

/// Số hole tối đa của một cửa sổ (bảng tĩnh, không cấp phát động).
pub const MAX_HOLES: usize = 12;

/// Ba cỡ Frame hợp lệ theo `size_bits` (4 KiB / 2 MiB / 1 GiB) — cùng bộ với `vifast-pte`.
pub const FRAME_SIZE_BITS: [u8; 3] = [12, 21, 30];

/// Một khoảng trống của cửa sổ: `[off, off + len)` (byte, so với đáy cửa sổ).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hole {
    /// Offset đầu khoảng (byte).
    pub off: u32,
    /// Cỡ khoảng (byte).
    pub len: u32,
}

/// Một Frame đã cấp: offset trong cửa sổ + cỡ.
///
/// `pa` thật là `base + off` — **nhãn** PA của D2-66, không phải địa chỉ dereference được ở host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    /// Offset trong cửa sổ (byte).
    pub off: u32,
    /// `size_bits` (`12`/`21`/`30`).
    pub size_bits: u8,
}

impl Frame {
    /// Cỡ Frame (byte) — `None` nếu `size_bits` lạ (không xảy ra với Frame do [`FrameArena`] cấp).
    pub const fn size_bytes(self) -> Option<u32> {
        match self.size_bits {
            12 => Some(1 << 12),
            21 => Some(1 << 21),
            30 => Some(1 << 30),
            _ => None,
        }
    }
}

/// Lỗi của allocator — **cục bộ**, không phải mã ABI; object layer ánh xạ sang `ObjError` tại
/// chỗ gọi (một bảng ánh xạ, không có bảng mã thứ hai).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllocError {
    /// `size_bits` ngoài `{12, 21, 30}` (hoặc cỡ > cửa sổ).
    InvalidSize,
    /// Cửa sổ hết chỗ (bump chạm trần hoặc không hole nào đủ lớn).
    NotEnoughMemory,
    /// Bump phải đệm căn lề — K2 KT-36: cấp **giảm dần**, không đệm ngầm.
    Alignment,
    /// `free` một Frame chưa từng cấp (ngoài watermark hoặc hết Frame sống cùng cỡ).
    NotAllocated,
    /// Hết slot hole (bảng tĩnh [`MAX_HOLES`]) — caller phải xử lý, không im lặng.
    NoHoles,
    /// Trạng thái nội bộ hỏng (hole chồng/vượt watermark, kế toán lệch).
    Corrupt,
}

/// Allocator **một cửa sổ PA** của Untyped: watermark `used` + free-list `holes`.
///
/// `const fn new` để object layer dựng được `Untyped::EMPTY` trong `static` (không cấp phát).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameArena {
    base: u64,
    capacity: u32,
    used: u32,
    holes: [Hole; MAX_HOLES],
    n_holes: u8,
    live: [u16; 3],
    allocs: u32,
    frees: u32,
}

impl FrameArena {
    /// Cửa sổ rỗng (chưa cấp) — dùng cho `Untyped::EMPTY`.
    pub const EMPTY: Self = Self {
        base: 0,
        capacity: 0,
        used: 0,
        holes: [Hole { off: 0, len: 0 }; MAX_HOLES],
        n_holes: 0,
        live: [0; 3],
        allocs: 0,
        frees: 0,
    };

    /// Cửa sổ `capacity` byte bắt đầu ở nhãn PA `base`.
    ///
    /// `capacity == 0` (chưa dựng) hoặc `> WINDOW_BYTES` bị coi là hỏng ở
    /// [`FrameArena::check_accounting`]/`alloc`; constructor **không panic** (D2-17).
    pub const fn new(base: u64, capacity: u32) -> Self {
        Self {
            base,
            capacity,
            used: 0,
            holes: [Hole { off: 0, len: 0 }; MAX_HOLES],
            n_holes: 0,
            live: [0; 3],
            allocs: 0,
            frees: 0,
        }
    }

    /// Đáy cửa sổ (nhãn PA).
    pub const fn base(&self) -> u64 {
        self.base
    }

    /// Cỡ cửa sổ (byte).
    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Watermark: byte cửa sổ đã chạm (Frame sống + hole) — **không** phải "byte đã cấp".
    pub const fn used_bytes(&self) -> u32 {
        self.used
    }

    /// Byte chưa từng chạm (`capacity - used`).
    pub const fn unused_bytes(&self) -> u32 {
        self.capacity.saturating_sub(self.used)
    }

    /// Tổng byte trong các hole.
    pub fn hole_bytes(&self) -> u32 {
        let mut sum = 0u32;
        for h in self.holes.iter().take(self.n_holes as usize) {
            sum = sum.saturating_add(h.len);
        }
        sum
    }

    /// Byte **có thể cấp lại** = `capacity - Σ Frame sống` = `unused + hole_bytes`.
    ///
    /// (Một hole nhỏ hơn cỡ yêu cầu vẫn không cấp được — API này là tổng byte tự do, không phải
    /// bảo đảm cấp được một Frame cụ thể.)
    pub fn free_bytes(&self) -> u32 {
        self.capacity
            .saturating_sub(self.used)
            .saturating_add(self.hole_bytes())
    }

    /// Số hole hiện có.
    pub const fn hole_count(&self) -> u8 {
        self.n_holes
    }

    /// Hole thứ `i` ( `None` nếu ngoài dải).
    pub const fn hole(&self, i: usize) -> Option<Hole> {
        if i < self.n_holes as usize {
            Some(self.holes[i])
        } else {
            None
        }
    }

    /// Số Frame sống của một cỡ.
    pub const fn live_frames(&self, size_bits: u8) -> u16 {
        match index_of(size_bits) {
            Some(i) => self.live[i],
            None => 0,
        }
    }

    /// Tổng số Frame sống (mọi cỡ).
    pub const fn live_total(&self) -> u16 {
        self.live[0] + self.live[1] + self.live[2]
    }

    /// Tổng số lần cấp thành công (kể từ lần cuối [`FrameArena::reset`]).
    pub const fn alloc_count(&self) -> u32 {
        self.allocs
    }

    /// Tổng số lần trả thành công.
    pub const fn free_count(&self) -> u32 {
        self.frees
    }

    /// PA nhãn của một Frame đã cấp.
    pub fn pa(&self, frame: Frame) -> u64 {
        self.base + u64::from(frame.off)
    }

    /// Về trạng thái rỗng (dùng khi `Revoke` Untyped — byte về 0 như sim).
    pub fn reset(&mut self) {
        self.used = 0;
        self.n_holes = 0;
        self.holes = [Hole { off: 0, len: 0 }; MAX_HOLES];
        self.live = [0; 3];
        self.allocs = 0;
        self.frees = 0;
    }

    /// Cấp một Frame `size_bits`: first-fit hole (căn tuyệt đối) rồi bump.
    ///
    /// Lỗi **nguyên tử**; `Alignment` khi đệm căn lề ngầm sẽ phải tiêu byte (K2 KT-36).
    pub fn alloc(&mut self, size_bits: u8) -> Result<Frame, AllocError> {
        let idx = index_of(size_bits).ok_or(AllocError::InvalidSize)?;
        let size = 1u32 << size_bits;
        if u64::from(size) > u64::from(self.capacity) {
            return Err(AllocError::NotEnoughMemory);
        }
        // 1) Hole: first-fit, kiểm **căn tuyệt đối** (như KT-52: cửa sổ 1 GiB nhưng chỉ cửa sổ
        //    căn đúng cỡ mới cấp được Frame lớn ở offset lẻ không căn).
        let mut i = 0usize;
        while i < self.n_holes as usize {
            let hole = self.holes[i];
            let start = match align_up_u32(hole.off, size) {
                Some(v) => v,
                None => return Err(AllocError::Corrupt),
            };
            let end = u64::from(hole.off) + u64::from(hole.len);
            let fits = u64::from(start) + u64::from(size) <= end;
            let aligned = (self.base + u64::from(start)) & u64::from(size - 1) == 0;
            if fits && aligned {
                // **Nguyên tử**: bảng hole phải đủ chỗ cho phần còn lại của hole bị tách
                // (`left` + `right`), sau khi bỏ chính hole đó. Thiếu chỗ ⇒ `NoHoles`, KHÔNG
                // đổi byte/watermark và **không** được "thành công" để rồi mất hole im lặng
                // (reviewer-kt67 #5: crate tự hứa lỗi nguyên tử).
                let left = start > hole.off;
                let right = u64::from(start) + u64::from(size) < end;
                let need = usize::from(left) + usize::from(right);
                if (self.n_holes as usize).saturating_sub(1) + need > MAX_HOLES {
                    return Err(AllocError::NoHoles);
                }
                self.remove_hole_at(i);
                if left
                    && !Self::insert(
                        &mut self.holes,
                        &mut self.n_holes,
                        Hole {
                            off: hole.off,
                            len: start - hole.off,
                        },
                    )
                {
                    return Err(AllocError::NoHoles);
                }
                let right_off = u64::from(start) + u64::from(size);
                if right
                    && !Self::insert(
                        &mut self.holes,
                        &mut self.n_holes,
                        Hole {
                            off: right_off as u32,
                            len: (end - right_off) as u32,
                        },
                    )
                {
                    return Err(AllocError::NoHoles);
                }
                self.live[idx] = self.live[idx].saturating_add(1);
                self.allocs = self.allocs.saturating_add(1);
                return Ok(Frame {
                    off: start,
                    size_bits,
                });
            }
            i += 1;
        }
        // 2) Bump: `align_up(base + used)`; đệm căn lề **không** được tiêu ngầm ⇒ `Alignment`.
        let base = match self.base.checked_add(u64::from(self.used)) {
            Some(v) => v,
            None => return Err(AllocError::Corrupt),
        };
        let want = match align_up_u64(base, u64::from(size)).checked_sub(self.base) {
            Some(v) => v,
            None => return Err(AllocError::Corrupt),
        };
        if want != u64::from(self.used) {
            return Err(AllocError::Alignment);
        }
        let end = match want.checked_add(u64::from(size)) {
            Some(v) => v,
            None => return Err(AllocError::Corrupt),
        };
        if end > u64::from(self.capacity) {
            return Err(AllocError::NotEnoughMemory);
        }
        self.used = end as u32;
        self.live[idx] = self.live[idx].saturating_add(1);
        self.allocs = self.allocs.saturating_add(1);
        Ok(Frame {
            off: want as u32,
            size_bits,
        })
    }

    /// Trả một Frame: hole + gộp + rewind watermark; giảm đếm Frame sống.
    ///
    /// Không kiểm "Frame có bị map" — chủ sở hữu (object layer) kiểm `maps == 0` trước khi gọi
    /// (D2-83).
    pub fn free(&mut self, frame: Frame) -> Result<(), AllocError> {
        let idx = index_of(frame.size_bits).ok_or(AllocError::InvalidSize)?;
        let size = 1u32 << frame.size_bits;
        let end = u64::from(frame.off) + u64::from(size);
        if end > u64::from(self.used) {
            return Err(AllocError::NotAllocated);
        }
        if self.live[idx] == 0 {
            return Err(AllocError::NotAllocated);
        }
        if !Self::insert(
            &mut self.holes,
            &mut self.n_holes,
            Hole {
                off: frame.off,
                len: size,
            },
        ) {
            return Err(AllocError::NoHoles);
        }
        self.live[idx] -= 1;
        self.frees = self.frees.saturating_add(1);
        self.merge_holes();
        self.rewind();
        Ok(())
    }

    /// Bất biến kế toán — **không tautology**: danh sách Frame sống do **caller liệt kê** từ
    /// bảng object của nó (KHÔNG lưu bản sao trong allocator), rồi allocator kiểm:
    ///
    /// 1. mọi hole: `len > 0`, nằm trong `[0, used)`, sắp theo `off`, **không chồng** nhau;
    /// 2. mọi Frame sống caller đưa: nằm trong `[0, used)`, **không chồng** nhau và **không
    ///    chồng** hole nào;
    /// 3. `Σ cỡ Frame sống + Σ hole == used` **và** `used ≤ capacity`.
    ///
    /// Ba điều kiện này **chứng minh phủ kín** `[0, used)` chứ không phải định nghĩa vòng tròn:
    /// các khoảng (Frame ∪ hole) rời nhau, mỗi khoảng nằm trong `[0, used)`, tổng độ dài bằng
    /// `used` ⇒ không byte nào trong watermark bị "bỏ quên" (một gap sẽ làm tổng < `used`).
    /// `hole_bytes()` là **tổng độ dài các hole được lưu**, không phải `used - live`; `used` là
    /// watermark do `alloc` đẩy, không phải tổng suy ra từ danh sách.
    ///
    /// Nếu quên liệt kê một Frame, hoặc liệt kê chồng/ngoài watermark ⇒ `Corrupt` (có test host
    /// chứng minh: `kiem_ke_toan_bat_duoc_khai_gian`).
    pub fn check_accounting(&self, live: &[(u32, u32)]) -> Result<(), AllocError> {
        if self.capacity > WINDOW_BYTES || self.used > self.capacity {
            return Err(AllocError::Corrupt);
        }
        // 1) Hole: trong dải, sắp, không chồng; cộng độ dài **được lưu**.
        let mut prev_end = 0u64;
        let mut hole_sum = 0u64;
        for i in 0..self.n_holes as usize {
            let h = self.holes[i];
            if h.len == 0 || u64::from(h.off) + u64::from(h.len) > u64::from(self.used) {
                return Err(AllocError::Corrupt);
            }
            if u64::from(h.off) < prev_end {
                return Err(AllocError::Corrupt);
            }
            prev_end = u64::from(h.off) + u64::from(h.len);
            hole_sum += u64::from(h.len);
        }
        // 2) Frame sống do caller liệt kê: trong dải, không chồng nhau/không chồng hole.
        let mut live_sum = 0u64;
        for (i, &(off, len)) in live.iter().enumerate() {
            if len == 0 || u64::from(off) + u64::from(len) > u64::from(self.used) {
                return Err(AllocError::Corrupt);
            }
            live_sum += u64::from(len);
            for &(o2, l2) in live.iter().skip(i + 1) {
                if u64::from(off) < u64::from(o2) + u64::from(l2)
                    && u64::from(o2) < u64::from(off) + u64::from(len)
                {
                    return Err(AllocError::Corrupt);
                }
            }
            // KT-242: slicing giữ **đúng** hành vi cũ — nếu `n_holes > MAX_HOLES` thì panic
            // như indexing, không âm thầm cắt ngắn.
            for &h in &self.holes[..self.n_holes as usize] {
                if u64::from(off) < u64::from(h.off) + u64::from(h.len)
                    && u64::from(h.off) < u64::from(off) + u64::from(len)
                {
                    return Err(AllocError::Corrupt);
                }
            }
        }
        // 3) Phủ kín [0, used): rời nhau + trong dải + tổng bằng `used`.
        if live_sum + hole_sum != u64::from(self.used) {
            return Err(AllocError::Corrupt);
        }
        Ok(())
    }

    // ------------------------------------------------------------------ free-list nội bộ

    /// Chèn hole theo thứ tự `off`; `false` khi bảng đầy (**caller phải xử lý**).
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

    fn remove_hole_at(&mut self, index: usize) {
        let _ = Self::remove(&mut self.holes, &mut self.n_holes, index);
    }

    /// Gộp hole dính nhau/trùng (sắp theo `off` rồi merge).
    fn merge_holes(&mut self) {
        let holes = &mut self.holes;
        let n = &mut self.n_holes;
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
            if u64::from(a.off) + u64::from(a.len) >= u64::from(b.off) {
                let end =
                    (u64::from(b.off) + u64::from(b.len)).max(u64::from(a.off) + u64::from(a.len));
                let start = a.off.min(b.off);
                holes[i] = Hole {
                    off: start,
                    len: (end - u64::from(start)) as u32,
                };
                let _ = Self::remove(holes, n, i + 1);
            } else {
                i += 1;
            }
        }
    }

    /// Rewind watermark khi hole chạm đúng mép (giữ `Σ live + Σ hole == used`).
    fn rewind(&mut self) {
        loop {
            let mut found = None;
            for (i, h) in self.holes.iter().take(self.n_holes as usize).enumerate() {
                if u64::from(h.off) + u64::from(h.len) == u64::from(self.used) {
                    found = Some(i);
                    break;
                }
            }
            match found {
                Some(i) => {
                    let h = Self::remove(&mut self.holes, &mut self.n_holes, i);
                    self.used = h.off;
                }
                None => break,
            }
        }
    }
}

/// Chỉ số cỡ trong [`FRAME_SIZE_BITS`].
const fn index_of(size_bits: u8) -> Option<usize> {
    match size_bits {
        12 => Some(0),
        21 => Some(1),
        30 => Some(2),
        _ => None,
    }
}

/// `align_up` tràn ⇒ `None` (fail-closed, không panic).
fn align_up_u32(value: u32, align: u32) -> Option<u32> {
    value.checked_add(align - 1).map(|v| v & !(align - 1))
}

/// `align_up` u64, **không panic**: `align` là luỹ thừa 2; tràn (caller đưa giá trị lạ) thì trả
/// `u64::MAX` để phép so sánh phía sau thất bại thay vì panic trong build debug.
fn align_up_u64(value: u64, align: u64) -> u64 {
    let mask = align - 1;
    value
        .checked_add(mask)
        .map(|v| v & !mask)
        .unwrap_or(u64::MAX)
}
