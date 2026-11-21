// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-271** — test host cho **console chữ trên framebuffer** virtio-gpu (`gpu_console`).
//!
//! Năm nhóm: (A) hình học + con trỏ; (B) `putc`/`puts`/`newline`/`\r`/tự xuống dòng; (C) **cuộn**
//! (memmove vùng framebuffer) và biên vùng cuộn; (D) **vùng bẩn** (flush theo lô) + bộ đếm; (E)
//! `verify_text` — đọc lại framebuffer và so **bitmap 8×8** với [`glyph`], kèm ca âm.
//!
//! Mọi kỳ vọng pixel ở đây suy từ **bảng font** (`vifast_virtio::gpu::glyph`), không chép lại
//! bitmap bằng tay: một glyph sai trong bảng thì test đỏ, chứ không phải "test khớp bản sao".

use vifast_virtio::gpu::{
    get_pixel, put_pixel, DrawError, Rect, COLOR_BLACK, COLOR_RED, COLOR_WHITE, GLYPH_H, GLYPH_W,
};
use vifast_virtio::gpu_console::{verify_text, Console};

const W: usize = 64; // 8 cột ký tự
const H: usize = 32; // 4 dòng ký tự
const STRIDE: usize = W * 4;
const FG: u32 = COLOR_WHITE;
const BG: u32 = COLOR_BLACK;

/// Framebuffer khởi tạo **đúng màu nền** (không phải 0 = đen trong suốt): `COLOR_BLACK` là
/// `0xff000000` (alpha đặc), nên đệm 0 sẽ khác nền và mọi ca "pixel chưa vẽ" sẽ so sai.
fn fb() -> Vec<u8> {
    let mut buf = vec![0u8; STRIDE * H];
    let mut at = 0;
    while at < buf.len() {
        buf[at..at + 4].copy_from_slice(&BG.to_le_bytes());
        at += 4;
    }
    buf
}

fn pixel(fb: &[u8], x: usize, y: usize) -> u32 {
    get_pixel(fb, W, H, STRIDE, x, y).expect("pixel trong biên")
}

// ---------------------------------------------------------------- A) hình học + con trỏ

#[test]
fn console_new_tu_choi_hinh_hoc_sai() {
    // Rộng/cao bằng 0.
    assert!(Console::new(0, H, STRIDE, 0, 0).is_none());
    assert!(Console::new(W, 0, STRIDE, 0, 0).is_none());
    // `stride` hẹp hơn `width * 4` ⇒ mỗi hàng không đủ pixel.
    assert!(Console::new(W, H, W * 4 - 1, 0, 0).is_none());
    // Gốc lệch tới mức không còn **một** ô 8×8 nào.
    assert!(Console::new(W, H, STRIDE, W - 7, 0).is_none());
    assert!(Console::new(W, H, STRIDE, 0, H - 7).is_none());
    // Hình học hợp lệ ⇒ số cột/dòng suy từ `(width - origin) / 8`.
    let console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    assert_eq!(console.cols(), 8);
    assert_eq!(console.rows(), 4);
    assert_eq!(console.cursor(), (0, 0));
    // Gốc lệch 8 px ⇒ mất đúng một cột và một dòng.
    let offset = Console::new(W, H, STRIDE, 8, 8).expect("hình học hợp lệ");
    assert_eq!(offset.cols(), 7);
    assert_eq!(offset.rows(), 3);
}

// ---------------------------------------------------------------- B) putc/puts/newline

#[test]
fn console_putc_ve_glyph_va_tien_con_tro() {
    let mut fb = fb();
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    console.putc(&mut fb, b'V', FG, BG).expect("vẽ được");
    assert_eq!(console.cursor(), (1, 0));
    // 'V' = `0x33` ở hàng 0 ⇒ bit 0 **bật**, bit 2 **tắt** (đọc theo `glyph_pixel`).
    assert_eq!(pixel(&fb, 0, 0), FG);
    assert_eq!(pixel(&fb, 2, 0), BG);
    // Ô kế tiếp chưa vẽ ⇒ vẫn là nền.
    assert_eq!(pixel(&fb, GLYPH_W, 0), BG);
    assert_eq!(console.stats().chars, 1);
    assert_eq!(console.stats().newlines, 0);
}

#[test]
fn console_puts_nhieu_ky_tu_va_newline() {
    let mut fb = fb();
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    console.puts(&mut fb, "ab", FG, BG).expect("vẽ được");
    assert_eq!(console.cursor(), (2, 0));
    console.putc(&mut fb, b'\r', FG, BG).expect("CR");
    assert_eq!(console.cursor(), (0, 0));
    console.putc(&mut fb, b'\n', FG, BG).expect("LF");
    assert_eq!(console.cursor(), (0, 1));
    assert_eq!(console.stats().newlines, 1);
    // 'b' = `0x07` hàng 0 ⇒ bit 0 bật: đã vẽ ở cột 1 hàng 0.
    assert_eq!(pixel(&fb, GLYPH_W, 0), FG);
    // 'a' = `0x00` hàng 0 ⇒ hàng 0 của 'a' là nền.
    assert_eq!(pixel(&fb, 0, 0), BG);
}

#[test]
fn console_tu_dong_xuong_dong_khi_het_cot() {
    let mut fb = fb();
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    let mut index = 0;
    while index < 8 {
        console.putc(&mut fb, b'V', FG, BG).expect("vẽ được");
        index += 1;
    }
    // Đầy 8 cột ⇒ tự xuống dòng (đếm là một `newline`).
    assert_eq!(console.cursor(), (0, 1));
    assert_eq!(console.stats().newlines, 1);
    assert_eq!(console.stats().chars, 8);
}

#[test]
fn console_ky_tu_dieu_khien_bi_bo_qua() {
    let mut fb = fb();
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    console.putc(&mut fb, 0x07, FG, BG).expect("BEL");
    console.putc(&mut fb, b'\t', FG, BG).expect("TAB");
    // Không vẽ, không tiến con trỏ, không bẩn.
    assert_eq!(console.cursor(), (0, 0));
    assert_eq!(console.stats().chars, 0);
    assert_eq!(console.dirty(), None);
}

#[test]
fn console_byte_ngoai_ascii_thanh_o_nen() {
    let mut fb = fb();
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    console.putc(&mut fb, 0x80, FG, BG).expect("vẽ được");
    // Byte không có glyph ⇒ ô 8×8 toàn nền, con trỏ **vẫn** tiến (không lệch cột).
    assert_eq!(console.cursor(), (1, 0));
    assert_eq!(console.stats().chars, 1);
    let mut row = 0;
    while row < GLYPH_H {
        let mut col = 0;
        while col < GLYPH_W {
            assert_eq!(pixel(&fb, col, row), BG, "({col},{row})");
            col += 1;
        }
        row += 1;
    }
}

// ---------------------------------------------------------------- C) cuộn

#[test]
fn console_cuon_day_noi_dung_len() {
    let mut fb = fb();
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    for ch in *b"ABCD" {
        console.putc(&mut fb, ch, FG, BG).expect("vẽ được");
        console.putc(&mut fb, b'\n', FG, BG).expect("LF");
    }
    // Bốn dòng trên `rows = 4`: '\n' sau 'D' ở dòng cuối ⇒ **đã** cuộn một lần.
    assert_eq!(console.cursor(), (0, 3));
    assert_eq!(console.stats().scrolls, 1);
    // Nội dung dời lên đúng một dòng: 'B' dòng 0, 'C' dòng 1, 'D' dòng 2, dòng 3 trống.
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 0, "B", FG, BG), Ok(1));
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 8, "C", FG, BG), Ok(1));
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 16, "D", FG, BG), Ok(1));
    // 'A' đã bị đẩy khỏi màn hình.
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 0, "A", FG, BG), Ok(0));
    // Dòng cuối đã bị xoá về nền (dấu cách = glyph toàn 0 ⇒ khớp nền).
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 24, " ", FG, BG), Ok(1));
    // Thêm một '\n' nữa ⇒ cuộn lần hai: 'C' dòng 0, 'D' dòng 1, hai dòng cuối trống.
    console.putc(&mut fb, b'\n', FG, BG).expect("LF");
    assert_eq!(console.stats().scrolls, 2);
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 0, "C", FG, BG), Ok(1));
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 8, "D", FG, BG), Ok(1));
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 16, " ", FG, BG), Ok(1));
}

#[test]
fn console_cuon_khong_dung_ngoai_vung() {
    let mut fb = fb();
    // Mốc **ngoài** vùng console (gốc 8,8) — cuộn không được chạm tới.
    put_pixel(&mut fb, W, H, STRIDE, 0, 0, COLOR_RED).expect("đặt mốc");
    let mut console = Console::new(W, H, STRIDE, 8, 8).expect("hình học hợp lệ");
    assert_eq!(console.rows(), 3);
    console.puts(&mut fb, "A\nB\nC\n", FG, BG).expect("vẽ được");
    assert_eq!(console.stats().scrolls, 1);
    assert_eq!(pixel(&fb, 0, 0), COLOR_RED);
    // Nội dung đã dời lên đúng một dòng trong vùng console.
    assert_eq!(verify_text(&fb, W, H, STRIDE, 8, 8, "B", FG, BG), Ok(1));
    assert_eq!(verify_text(&fb, W, H, STRIDE, 8, 16, "C", FG, BG), Ok(1));
}

#[test]
fn console_cuon_hai_lan_lien_tiep() {
    let mut fb = fb();
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    console
        .puts(&mut fb, "A\nB\nC\nD\nE\n", FG, BG)
        .expect("vẽ được");
    // 5 dòng trên 4 hàng ⇒ cuộn 2 lần: còn 'C','D','E' + một dòng trống.
    assert_eq!(console.stats().scrolls, 2);
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 0, "C", FG, BG), Ok(1));
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 8, "D", FG, BG), Ok(1));
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 16, "E", FG, BG), Ok(1));
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 24, " ", FG, BG), Ok(1));
}

// ---------------------------------------------------------------- D) vùng bẩn + bộ đếm

#[test]
fn console_vung_ban_hop_nhat_va_lay_ra() {
    let mut fb = fb();
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    assert_eq!(console.dirty(), None);
    console.puts(&mut fb, "ab\ncd", FG, BG).expect("vẽ được");
    // Hộp bao hai dòng: rộng 2 ô, cao 2 dòng.
    assert_eq!(console.dirty(), Some(Rect::new(0, 0, 16, 16)));
    assert_eq!(console.take_dirty(), Some(Rect::new(0, 0, 16, 16)));
    assert_eq!(console.dirty(), None);
    assert_eq!(console.take_dirty(), None);
}

#[test]
fn console_vung_ban_chi_rong_khi_co_ve() {
    let mut fb = fb();
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    console.putc(&mut fb, b'\n', FG, BG).expect("LF");
    // Xuống dòng **không** vẽ pixel nào ⇒ không có gì để flush.
    assert_eq!(console.dirty(), None);
    console.putc(&mut fb, b'V', FG, BG).expect("vẽ được");
    assert_eq!(console.dirty(), Some(Rect::new(0, 8, 8, 8)));
}

#[test]
fn console_dem_flush() {
    let console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    assert_eq!(console.stats().flushes, 0);
    let mut console = console;
    console.note_flush();
    console.note_flush();
    assert_eq!(console.stats().flushes, 2);
}

#[test]
fn console_clear_xoa_vung_va_ve_goc() {
    let mut fb = fb();
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    console.puts(&mut fb, "AB\nCD", FG, BG).expect("vẽ được");
    console.take_dirty();
    console.clear(&mut fb, BG).expect("xoá được");
    assert_eq!(console.cursor(), (0, 0));
    assert_eq!(console.dirty(), Some(Rect::new(0, 0, 64, 32)));
    assert_eq!(verify_text(&fb, W, H, STRIDE, 0, 0, "AB", FG, BG), Ok(0));
}

#[test]
fn console_dem_ngan_tra_badbuffer() {
    let mut short = vec![0u8; STRIDE * H - 1];
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    assert_eq!(
        console.putc(&mut short, b'A', FG, BG),
        Err(DrawError::BadBuffer)
    );
    assert_eq!(console.newline(&mut short, BG), Err(DrawError::BadBuffer));
    assert_eq!(console.clear(&mut short, BG), Err(DrawError::BadBuffer));
    // Không có gì bị đánh dấu bẩn khi lệnh hỏng.
    assert_eq!(console.dirty(), None);
}

// ---------------------------------------------------------------- E) verify_text

#[test]
fn verify_text_dung_va_sai() {
    let mut fb = fb();
    let mut console = Console::new(W, H, STRIDE, 0, 0).expect("hình học hợp lệ");
    console.puts(&mut fb, "ViFast", FG, BG).expect("vẽ được");
    // "ViFast" = **sáu** ký tự (V,i,F,a,s,t) — cùng chuỗi ảnh `ktest-gpu` vẽ.
    assert_eq!(
        verify_text(&fb, W, H, STRIDE, 0, 0, "ViFast", FG, BG),
        Ok(6)
    );
    // Đổi **một** pixel của glyph đầu ⇒ đúng ký tự đó không khớp.
    put_pixel(&mut fb, W, H, STRIDE, 0, 0, BG).expect("đổi pixel");
    assert_eq!(
        verify_text(&fb, W, H, STRIDE, 0, 0, "ViFast", FG, BG),
        Ok(5)
    );
    // Chuỗi khác hoàn toàn ⇒ 0 ký tự khớp.
    assert_eq!(
        verify_text(&fb, W, H, STRIDE, 0, 0, "XXXXXX", FG, BG),
        Ok(0)
    );
}

#[test]
fn verify_text_vuot_bien_bi_tu_choi() {
    let fb = vec![0u8; STRIDE * H];
    assert_eq!(
        verify_text(&fb, W, H, STRIDE, W - 8, 0, "AB", FG, BG),
        Err(DrawError::OutOfBounds)
    );
    assert_eq!(
        verify_text(&fb, W, H, STRIDE, 0, H - 4, "A", FG, BG),
        Err(DrawError::OutOfBounds)
    );
    assert_eq!(
        verify_text(&fb, W, H, STRIDE - 1, 0, 0, "A", FG, BG),
        Err(DrawError::BadStride)
    );
    assert_eq!(
        verify_text(&fb[..STRIDE * H - 1], W, H, STRIDE, 0, 0, "A", FG, BG),
        Err(DrawError::BadBuffer)
    );
}
