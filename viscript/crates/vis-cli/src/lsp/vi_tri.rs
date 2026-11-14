// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-68/ST-69 (VD-102 (4))** — vị trí LSP **0-based UTF-16**, **tab = 1 cột**.
//!
//! Tính trên **văn bản đã chuẩn hoá** (`lexed.text`: NFC + CRLF→LF) — mọi `span` của lex/parse/
//! lower/tyck trỏ vào chuỗi đó. **Không** dùng `modules::vi_tri` (1-based/scalar/tab=4).

use vis_syntax::Span;

/// Byte offset → `(line0, char0)` với `char0` tính bằng **UTF-16 code unit** (tab = 1).
pub fn vi_tri_utf16(text: &str, byte: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut ch = 0u32;
    for (i, c) in text.char_indices() {
        if i >= byte {
            break;
        }
        if c == '\n' {
            line += 1;
            ch = 0;
        } else {
            ch += c.len_utf16() as u32;
        }
    }
    (line, ch)
}

/// `(line0, char0)` UTF-16 → byte offset (đầu đơn vị UTF-16) — **ST-69** sẽ dùng (hiện mới có unit test).
#[allow(dead_code)]
/// Toạ độ vượt dòng ⇒ trả **byte cuối dòng** (hoặc `text.len()` nếu vượt tệp).
pub fn byte_tu_vi_tri(text: &str, line: u32, character: u32) -> usize {
    let mut l = 0u32;
    let mut c = 0u32;
    for (i, ch) in text.char_indices() {
        if l == line && c >= character {
            return i;
        }
        if ch == '\n' {
            if l == line {
                return i;
            }
            l += 1;
            c = 0;
        } else if l == line {
            c += ch.len_utf16() as u32;
        }
    }
    text.len()
}

/// Độ dài **UTF-16** của dòng (không tính `\n`) — dùng để kẹp `end`.
fn do_dai_dong(text: &str, line: u32) -> u32 {
    let mut l = 0u32;
    let mut n = 0u32;
    for ch in text.chars() {
        if l == line {
            if ch == '\n' {
                break;
            }
            n += ch.len_utf16() as u32;
        } else if ch == '\n' {
            l += 1;
        }
    }
    n
}

/// `span` (byte, trên văn bản **chuẩn hoá**) → cặp vị trí LSP 0-based UTF-16.
///
/// Span vắt dòng ⇒ kẹp `end = (line bắt đầu, min(char+1, độ dài dòng))`; span rỗng ⇒ rộng **1**
/// code unit (trừ cuối dòng ⇒ giữ nguyên, range rỗng hợp lệ).
pub fn khoang(text: &str, span: Span) -> ((u32, u32), (u32, u32)) {
    let s = vi_tri_utf16(text, span.start);
    let mut e = vi_tri_utf16(text, span.end);
    // Span **vắt dòng** hoặc **rỗng/ngược** ⇒ cùng một cách kẹp về dòng bắt đầu.
    if e.0 != s.0 || span.end <= span.start {
        e = (s.0, (s.1 + 1).min(do_dai_dong(text, s.0)));
    }
    (s, e)
}

#[cfg(test)]
mod tests {
    use super::{byte_tu_vi_tri, khoang, vi_tri_utf16};
    use vis_syntax::Span;

    /// Emoji: 1 scalar nhưng **2** code unit UTF-16 ⇒ `character` đúng 13 (scalar sẽ 12).
    #[test]
    fn vi_tri_emoji_utf16() {
        let t = "let s = \"😀\" 1\n";
        assert_eq!(vi_tri_utf16(t, 15), (0, 13));
        assert_eq!(byte_tu_vi_tri(t, 0, 13), 15);
    }

    /// Tab = **1** cột (không phải 4 như `modules::vi_tri`).
    #[test]
    fn vi_tri_tab_mot_cot() {
        assert_eq!(vi_tri_utf16("\tx", 1), (0, 1));
        assert_eq!(byte_tu_vi_tri("\tx", 0, 1), 1);
    }

    /// CRLF: `lex` chuẩn hoá CRLF→LF; mốc span là văn bản **đã chuẩn hoá**.
    #[test]
    fn vi_tri_crlf_da_chuan_hoa() {
        let lexed = vis_syntax::lex("a\r\nb", vis_syntax::Mode::Script);
        assert_eq!(lexed.text, "a\nb");
        assert_eq!(vi_tri_utf16(&lexed.text, 3), (1, 1));
        assert_eq!(byte_tu_vi_tri(&lexed.text, 1, 1), 3);
    }

    /// Span rỗng ở cuối dòng ⇒ kẹp `character` về độ dài dòng (không vượt `\n`).
    #[test]
    fn khop_cuoi_dong() {
        let t = "let x = \n";
        assert_eq!(khoang(t, Span::new(8, 8)), ((0, 8), (0, 8)));
    }

    /// Biên 0 và biên cuối tệp.
    #[test]
    fn bien_khong_va_cuoi() {
        let t = "abc\n";
        assert_eq!(vi_tri_utf16(t, 0), (0, 0));
        assert_eq!(byte_tu_vi_tri(t, 0, 0), 0);
        assert_eq!(vi_tri_utf16(t, t.len()), (1, 0));
        assert_eq!(byte_tu_vi_tri(t, 9, 9), t.len());
    }

    /// Nhiều dòng: dòng 2 bắt đầu sau `\n`; `character` vượt dòng ⇒ byte cuối dòng.
    #[test]
    fn nhieu_dong() {
        let t = "a\nbb\nc";
        assert_eq!(vi_tri_utf16(t, 5), (2, 0));
        assert_eq!(byte_tu_vi_tri(t, 2, 0), 5);
        assert_eq!(byte_tu_vi_tri(t, 1, 9), 4);
    }
}
