// SPDX-License-Identifier: MIT OR Apache-2.0
//! `gpu_console` — **console chữ trên framebuffer** virtio-gpu: con trỏ `(col, row)` theo ô ký tự
//! 8×8, `putc`/`puts`/`newline`, **cuộn** khi hết dòng, và **vùng bẩn** để flush theo lô — KT-271.
//!
//! # Vì sao logic nằm ở crate **thuần**
//!
//! Cùng lý do `gpu.rs` (KT-263): phần dễ sai **im lặng** của console là **hình học** — con trỏ lệch
//! một cột, cuộn lệch một hàng pixel, hộp bao vùng bẩn thiếu một dòng. Thiết bị vẫn trả
//! `RESP_OK_NODATA` cho mọi lệnh, nên chỉ ảnh chụp màn hình mới thấy sai. Đặt ở crate `no_std`
//! không `unsafe` nghĩa là **mọi** ca biên chạy được trên host trước khi chạm QEMU (D2-05), và
//! kernel chỉ còn phần gọi `TRANSFER_TO_HOST_2D`/`RESOURCE_FLUSH`.
//!
//! # Vì sao **không** flush từng ký tự
//!
//! Mỗi lần flush là **hai** lệnh virtqueue (`TRANSFER_TO_HOST_2D` + `RESOURCE_FLUSH`). Flush mỗi
//! ký tự ⇒ 29 ký tự = 58 lệnh cho một khối chữ 3 dòng, và mỗi lệnh là một vòng poll `used->idx`.
//! Console ở đây **tích** vùng đã đổi vào một hộp bao ([`Console::dirty`]) rồi người gọi flush
//! **một** lần ([`Console::take_dirty`] + `virtio_gpu::flush_rect`) ⇒ 2 lệnh cho cả khối. Số lệnh
//! là số **đo được** (bộ đếm `commands` của driver), không phải lời hứa — xem `# gpu console:`
//! trong ảnh `ktest-gpu`.
//!
//! # Vì sao cuộn bằng `copy_within`
//!
//! `slice::copy_within` là `memmove` (xử lý chồng lấn đúng), nên dời cả vùng chữ lên một dòng là
//! **một** lời gọi cho mỗi hàng pixel — không cấp phát, không đệm phụ, không `unsafe`. Vùng cuộn
//! chỉ là **cột của console** (`cols × 8` pixel), không phải cả khung: pixel ngoài vùng console
//! (ví dụ hình chữ nhật đỏ của TAP `gpu draw`) **không** bị chạm.

use crate::gpu::{
    check_rect, fill_rect, glyph, glyph_pixel, glyph_supported, DrawError, Rect, BYTES_PER_PIXEL,
    GLYPH_H, GLYPH_W,
};

/// Bộ đếm của console — in nguyên văn vào evidence (D2-102: số đo, không phải số trang trí).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConsoleStats {
    /// Số ký tự **đã vẽ** (không tính `\n`/`\r`/ký tự điều khiển).
    pub chars: u64,
    /// Số lần xuống dòng (kể cả tự xuống dòng khi hết cột).
    pub newlines: u64,
    /// Số lần **cuộn** (mỗi lần dời cả vùng chữ lên một dòng).
    pub scrolls: u64,
    /// Số lần người gọi báo đã flush một lô ([`Console::note_flush`]).
    pub flushes: u64,
    /// Cột hiện tại (ô ký tự).
    pub col: usize,
    /// Dòng hiện tại (ô ký tự).
    pub row: usize,
}

/// Console chữ trên một framebuffer `width × height` (`stride` byte/hàng), gốc tại
/// `(origin_x, origin_y)` tính bằng **pixel**.
///
/// Console **không** sở hữu framebuffer: mỗi lệnh nhận `fb: &mut [u8]` (cùng khuôn `draw_text`),
/// nên nó dùng được cho cả framebuffer thật (physmap) lẫn đệm test trên host.
#[derive(Clone, Copy, Debug)]
pub struct Console {
    width: usize,
    height: usize,
    stride: usize,
    origin_x: usize,
    origin_y: usize,
    cols: usize,
    rows: usize,
    col: usize,
    row: usize,
    dirty: Option<Rect>,
    stats: ConsoleStats,
}

impl Console {
    /// Console trên `width × height` (`stride` byte/hàng), gốc `(origin_x, origin_y)`.
    ///
    /// `None` (fail-closed) khi hình học vô lý: rộng/cao bằng 0, `stride < width * 4`, hoặc gốc
    /// lệch tới mức **không còn một ô 8×8 nào**. Không kẹp im lặng — kẹp sẽ vẽ ra ngoài vùng đã
    /// khai và ghi đè pixel của vùng khác.
    pub fn new(
        width: usize,
        height: usize,
        stride: usize,
        origin_x: usize,
        origin_y: usize,
    ) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        if stride < width.checked_mul(BYTES_PER_PIXEL)? {
            return None;
        }
        if origin_x >= width || origin_y >= height {
            return None;
        }
        let cols = (width - origin_x) / GLYPH_W;
        let rows = (height - origin_y) / GLYPH_H;
        if cols == 0 || rows == 0 {
            return None;
        }
        Some(Self {
            width,
            height,
            stride,
            origin_x,
            origin_y,
            cols,
            rows,
            col: 0,
            row: 0,
            dirty: None,
            stats: ConsoleStats::default(),
        })
    }

    /// Số cột ký tự của console.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Số dòng ký tự của console.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Con trỏ hiện tại `(col, row)` theo ô ký tự.
    pub fn cursor(&self) -> (usize, usize) {
        (self.col, self.row)
    }

    /// Bộ đếm (ký tự/xuống dòng/cuộn/flush + con trỏ).
    pub fn stats(&self) -> ConsoleStats {
        ConsoleStats {
            col: self.col,
            row: self.row,
            ..self.stats
        }
    }

    /// Hộp bao **vùng đã đổi** kể từ lần [`Self::take_dirty`] gần nhất. `None` ⇔ không có pixel
    /// nào đổi (ví dụ chỉ in `\n`) ⇒ **không** cần flush.
    pub fn dirty(&self) -> Option<Rect> {
        self.dirty
    }

    /// Lấy **và xoá** vùng bẩn — người gọi flush đúng hộp bao này rồi gọi [`Self::note_flush`].
    pub fn take_dirty(&mut self) -> Option<Rect> {
        self.dirty.take()
    }

    /// Ghi nhận người gọi đã flush **một** lô (một cặp `TRANSFER_TO_HOST_2D` + `RESOURCE_FLUSH`).
    pub fn note_flush(&mut self) {
        self.stats.flushes += 1;
    }

    /// Vẽ **một** ký tự tại con trỏ rồi tiến con trỏ.
    ///
    /// `\n` = xuống dòng, `\r` = về đầu dòng, ký tự điều khiển khác (`< 0x20`) **bỏ qua** (không
    /// vẽ, không tiến). Byte không có glyph (ngoài ASCII 32..126) vẽ **ô nền** nhưng **vẫn** tiến
    /// con trỏ — bỏ qua sẽ làm lệch mọi cột phía sau.
    pub fn putc(&mut self, fb: &mut [u8], c: u8, fg: u32, bg: u32) -> Result<(), DrawError> {
        self.check_fb(fb)?;
        match c {
            b'\n' => return self.newline(fb, bg),
            b'\r' => {
                self.col = 0;
                return Ok(());
            }
            0x00..=0x1f => return Ok(()),
            _ => {}
        }
        let x = self.origin_x + self.col * GLYPH_W;
        let y = self.origin_y + self.row * GLYPH_H;
        let cell = Rect::new(x as u32, y as u32, GLYPH_W as u32, GLYPH_H as u32);
        if glyph_supported(c) {
            // Một ký tự ASCII ⇒ `from_utf8` luôn thành công; nhánh `Err` chỉ để **không** panic
            // nếu ai đó đổi điều kiện `glyph_supported` sau này.
            let one = [c];
            match core::str::from_utf8(&one) {
                Ok(text) => crate::gpu::draw_text(
                    fb,
                    self.width,
                    self.height,
                    self.stride,
                    x,
                    y,
                    text,
                    fg,
                    bg,
                )?,
                Err(_) => fill_rect(fb, self.width, self.height, self.stride, cell, bg)?,
            }
        } else {
            fill_rect(fb, self.width, self.height, self.stride, cell, bg)?;
        }
        self.mark_dirty(cell);
        self.stats.chars += 1;
        self.col += 1;
        if self.col >= self.cols {
            self.newline(fb, bg)?;
        }
        Ok(())
    }

    /// Vẽ chuỗi `s` (mỗi byte một ô) rồi để con trỏ sau ký tự cuối.
    pub fn puts(&mut self, fb: &mut [u8], s: &str, fg: u32, bg: u32) -> Result<(), DrawError> {
        for byte in s.as_bytes() {
            self.putc(fb, *byte, fg, bg)?;
        }
        Ok(())
    }

    /// Xuống dòng: về cột 0, xuống một dòng; hết dòng cuối ⇒ **cuộn**.
    pub fn newline(&mut self, fb: &mut [u8], bg: u32) -> Result<(), DrawError> {
        self.check_fb(fb)?;
        self.col = 0;
        self.stats.newlines += 1;
        if self.row + 1 >= self.rows {
            self.scroll(fb, bg)?;
        } else {
            self.row += 1;
        }
        Ok(())
    }

    /// Xoá vùng chữ về `bg` và đưa con trỏ về `(0, 0)`.
    pub fn clear(&mut self, fb: &mut [u8], bg: u32) -> Result<(), DrawError> {
        self.check_fb(fb)?;
        let area = self.text_area();
        fill_rect(fb, self.width, self.height, self.stride, area, bg)?;
        self.col = 0;
        self.row = 0;
        self.mark_dirty(area);
        Ok(())
    }

    /// Vùng chữ của console (pixel) — `cols × 8` rộng, `rows × 8` cao.
    pub fn text_area(&self) -> Rect {
        Rect::new(
            self.origin_x as u32,
            self.origin_y as u32,
            (self.cols * GLYPH_W) as u32,
            (self.rows * GLYPH_H) as u32,
        )
    }

    /// Dời cả vùng chữ lên **một** dòng (8 hàng pixel) bằng `copy_within` (memmove), rồi xoá dòng
    /// cuối. Chỉ chạm cột của console — pixel ngoài vùng giữ nguyên.
    fn scroll(&mut self, fb: &mut [u8], bg: u32) -> Result<(), DrawError> {
        self.check_fb(fb)?;
        let line_bytes = self.cols * GLYPH_W * BYTES_PER_PIXEL;
        let x_off = self.origin_x * BYTES_PER_PIXEL;
        let top = self.origin_y;
        let moved_rows = (self.rows - 1) * GLYPH_H;
        let mut row = 0;
        while row < moved_rows {
            let dst = (top + row) * self.stride + x_off;
            let src = (top + row + GLYPH_H) * self.stride + x_off;
            fb.copy_within(src..src + line_bytes, dst);
            row += 1;
        }
        let last = Rect::new(
            self.origin_x as u32,
            (top + moved_rows) as u32,
            (self.cols * GLYPH_W) as u32,
            GLYPH_H as u32,
        );
        fill_rect(fb, self.width, self.height, self.stride, last, bg)?;
        self.row = self.rows - 1;
        self.stats.scrolls += 1;
        self.mark_dirty(self.text_area());
        Ok(())
    }

    /// `Err(BadBuffer)` nếu `fb` ngắn hơn `stride * height` — kiểm **một lần** trước mọi lệnh ghi
    /// (để `copy_within` không bao giờ panic vì chỉ số ngoài đệm).
    fn check_fb(&self, fb: &[u8]) -> Result<(), DrawError> {
        let need = self
            .stride
            .checked_mul(self.height)
            .ok_or(DrawError::Overflow)?;
        if fb.len() < need {
            return Err(DrawError::BadBuffer);
        }
        Ok(())
    }

    /// Hợp `rect` vào hộp bao vùng bẩn.
    fn mark_dirty(&mut self, rect: Rect) {
        self.dirty = Some(match self.dirty {
            None => rect,
            Some(current) => {
                let x = current.x.min(rect.x);
                let y = current.y.min(rect.y);
                let right = current.right().max(rect.right());
                let bottom = current.bottom().max(rect.bottom());
                Rect::new(x, y, right - x, bottom - y)
            }
        });
    }
}

/// Đếm số ký tự của `s` tại `(x, y)` có **đúng** bitmap 8×8 trong `fb` (so với [`glyph`]).
///
/// Trả số ký tự khớp (0..=`s.len()`), `Err` khi hình học sai — **không** kẹp. Đây là phép kiểm
/// **đọc lại framebuffer** của ảnh `ktest-gpu`: nó chứng minh `putc` đã vẽ đúng **trước** khi
/// `TRANSFER_TO_HOST_2D`, độc lập với ảnh chụp PPM của runner (hai đường kiểm khác nhau).
///
/// **Chín tham số là chủ ý** — cùng lý do `draw_text`: hình học framebuffer phải tường minh.
#[allow(clippy::too_many_arguments)]
pub fn verify_text(
    fb: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    x: usize,
    y: usize,
    s: &str,
    fg: u32,
    bg: u32,
) -> Result<usize, DrawError> {
    let count = s.len();
    let block_w = count.checked_mul(GLYPH_W).ok_or(DrawError::Overflow)?;
    check_rect(
        fb.len(),
        width,
        height,
        stride,
        Rect::new(x as u32, y as u32, block_w as u32, GLYPH_H as u32),
    )?;
    let fg_bytes = fg.to_le_bytes();
    let bg_bytes = bg.to_le_bytes();
    let mut matched = 0;
    for (index, byte) in s.as_bytes().iter().enumerate() {
        let bits = glyph(*byte);
        let gx = x + index * GLYPH_W;
        let mut same = true;
        let mut row = 0;
        while row < GLYPH_H && same {
            let base = (y + row) * stride + gx * BYTES_PER_PIXEL;
            let mut col = 0;
            while col < GLYPH_W {
                let at = base + col * BYTES_PER_PIXEL;
                let want = if glyph_pixel(&bits, col, row) {
                    fg_bytes
                } else {
                    bg_bytes
                };
                if fb[at..at + BYTES_PER_PIXEL] != want {
                    same = false;
                    break;
                }
                col += 1;
            }
            row += 1;
        }
        if same {
            matched += 1;
        }
    }
    Ok(matched)
}
