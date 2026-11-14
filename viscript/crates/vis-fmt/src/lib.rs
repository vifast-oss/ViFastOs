// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis-fmt` — định dạng mã ViScript từ CST (ST-07; tài liệu thiết kế §14.2, §18.4, VD-39).
//!
//! Phạm vi **vòng 1** (VD-39): chỉ chuẩn hoá **thụt lề** — mỗi cấp khối = đúng 4 dấu cách (suy từ
//! cây CST qua `INDENT`/`DEDENT`, không đếm theo tệp gốc), tab → 4 dấu cách, bỏ khoảng trắng cuối
//! dòng, đúng **một** newline cuối tệp, giữ nguyên văn trong dòng (khoảng trắng giữa token, chú
//! thích, thứ tự token), đầu ra **NFC** (lấy từ [`vis_syntax::LexOutput::text`]).
//!
//! **Không** định dạng lại biểu thức/danh sách/ngắt dòng dài — vòng 2.
//!
//! # Bốn tiêu chí §18.4
//!
//! (a) **lossless**: tệp "đã đúng chuẩn" là điểm bất động (`fmt(x) == x` từng byte); (b) **idempotent**
//! `fmt(fmt(x)) == fmt(x)`; (c) tệp có tab ⇒ đầu ra không còn `\t`, thụt = bội số 4; (d) tệp **lỗi cú
//! pháp** ⇒ [`format`] trả [`FmtError`], **không** sửa gì.
//!
//! # Vì sao dùng `Cst::tokens`
//!
//! Khoảng trắng không phải token (SPEC-lex §1): nó là **gap** giữa hai token = `text[prev_end..start]`.
//! Formatter giữ nguyên gap **trong dòng**, chỉ **tái tạo** phần thụt lề ở đầu dòng theo độ sâu khối
//! ⇒ không mất/đổi token nào (kể cả chú thích), khớp bất biến lossless của ST-05.

use vis_syntax::{Kind, Mode, ParseOutput};

/// Lỗi cú pháp/từ vựng chặn định dạng (ST-07 (d)): `fmt` **không sửa** tệp lỗi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmtError {
    pub code: &'static str,
    pub line: u32,
    pub col: u32,
    pub message: String,
}

/// Mã `VS0107` = tab (cảnh báo-lỗi) — **không** chặn format vì `vis fmt` chính là chỗ sửa tab.
const VS0107: u16 = 107;

/// Định dạng `text` (mã nguồn ViScript, chế độ script). Trả về bản đã chuẩn hoá hoặc [`FmtError`].
///
/// Chọn lỗi "đầu tiên" theo **tầng** (VD-38 mục 8): lỗi từ vựng `01xx` (trừ tab `VS0107`) trước, rồi
/// lỗi cú pháp `02xx`; tab không tính là lỗi ở đây vì chính `fmt` sửa nó.
pub fn format(text: &str) -> Result<String, FmtError> {
    format_mode(text, Mode::Script)
}

/// **ST-77 (VD-108)**: như [`format`] nhưng nhận **chế độ** — `.vish` dùng [`Mode::Command`].
/// Cùng một bộ luật lossless/idempotent, không đổi token; chỉ tái tạo thụt lề/CRLF/newline cuối.
pub fn format_mode(text: &str, mode: Mode) -> Result<String, FmtError> {
    let out = vis_syntax::parse(text, mode);
    // **ST-77**: chế độ lệnh có thể phát mã **không** phải 01xx/02xx (vd `VS0602` cờ lệnh sai);
    // mọi chẩn đoán **khác tab** đều chặn format — nếu không, render một CST dở dang sẽ **mất
    // token** (đo được: `.vish` sai cú pháp từng cho ra chuỗi rỗng). Tab `VS0107` vẫn được sửa.
    if let Some(d) = out.diagnostics.iter().find(|d| d.code.id != VS0107) {
        return Err(FmtError {
            code: d.code.short,
            line: d.line,
            col: d.col,
            message: d.message.clone(),
        });
    }
    if mode == Mode::Command {
        // **ST-77**: CmdCST **không** phát INDENT/DEDENT (khối `cmd` giữ thụt lề gốc trong CST),
        // nên chế độ lệnh chỉ chuẩn hoá **an toàn**: NFC, CRLF→LF, tab ở phần thụt lề → 4 dấu
        // cách, bỏ khoảng trắng cuối dòng, đúng một newline cuối — **không** tự đổi độ sâu khối
        // (sẽ làm lệch 50 fixture `.vish` đang đúng). Vẫn lossless token + idempotent.
        return Ok(render_command(&out));
    }
    Ok(render(&out))
}

/// Dựng lại `.vish` (chế độ lệnh) từ văn bản đã chuẩn hoá: giữ nguyên thụt lề gốc (chỉ tab → 4
/// dấu cách ở phần đầu dòng), bỏ khoảng trắng cuối mỗi dòng, đúng một newline cuối tệp.
fn render_command(out: &ParseOutput) -> String {
    let src = &out.text;
    let mut dst = String::with_capacity(src.len() + 1);
    for line in src.split('\n') {
        let mut ind = String::new();
        let mut rest = line;
        loop {
            if let Some(r) = rest.strip_prefix('\t') {
                ind.push_str("    ");
                rest = r;
            } else if let Some(r) = rest.strip_prefix(' ') {
                ind.push(' ');
                rest = r;
            } else {
                break;
            }
        }
        dst.push_str(&ind);
        dst.push_str(rest.trim_end());
        dst.push('\n');
    }
    while dst.ends_with('\n') {
        dst.pop();
    }
    dst.push('\n');
    dst
}

/// Dựng lại văn bản từ token + độ sâu khối của CST.
///
/// Phân biệt **dòng câu lệnh** (thụt lề = 4 × độ sâu khối) và **dòng nối** (giữ thụt lề gốc, chỉ đổi
/// tab ở phần thụt lề): dòng nối là dòng nằm **trong ngoặc** (`() [] {}`, nơi SPEC-lex §4 bỏ NEWLINE)
/// hoặc bắt đầu bằng `|>` (§4.5 nối với dòng trước). Mọi chỗ khác là dòng câu lệnh — kể cả dòng
/// **chỉ có chú thích** (khi đó độ sâu lấy theo token thật kế tiếp, xem `dong_truoc` bên dưới).
fn render(out: &ParseOutput) -> String {
    let src = &out.text; // đã NFC + CRLF→LF (SPEC-lex §1/§4)
    let toks = &out.cst.tokens;

    // Độ sâu khối **trước** mỗi token (INDENT/DEDENT là token rỗng, không in).
    let mut depth_truoc = Vec::with_capacity(toks.len());
    let mut d = 0usize;
    for t in toks {
        depth_truoc.push(d);
        match t.kind {
            Kind::Indent => d += 1,
            Kind::Dedent => d = d.saturating_sub(1),
            _ => {}
        }
    }
    // Token "thật" kế tiếp (bỏ trivia/INDENT/DEDENT/EOF) — dùng cho thụt lề dòng chỉ có chú thích.
    let mut sau = vec![usize::MAX; toks.len()];
    let mut next = usize::MAX;
    for i in (0..toks.len()).rev() {
        sau[i] = next;
        if !matches!(
            toks[i].kind,
            Kind::Comment
                | Kind::DocComment
                | Kind::Newline
                | Kind::Indent
                | Kind::Dedent
                | Kind::Eof
        ) {
            next = i;
        }
    }

    let mut dst = String::with_capacity(src.len() + 16);
    let mut last_end = 0usize;
    let mut at_line_start = true;
    let mut bracket_depth = 0usize;

    for (i, t) in toks.iter().enumerate() {
        match t.kind {
            Kind::Eof => break,
            Kind::Indent | Kind::Dedent => continue,
            _ => {}
        }
        let gap = if last_end <= t.span.start {
            &src[last_end..t.span.start]
        } else {
            ""
        };
        if t.kind == Kind::Newline {
            // Bỏ khoảng trắng trước NEWLINE = "khoảng trắng cuối dòng".
            dst.push('\n');
            at_line_start = true;
            last_end = t.span.end;
            continue;
        }
        let la_chu_thich = matches!(t.kind, Kind::Comment | Kind::DocComment);
        // Dòng chỉ có chú thích đứng **trước** dòng mã đầu tiên của khối: INDENT chưa phát ⇒ lấy
        // độ sâu của token thật kế tiếp (nếu sâu hơn) để chú thích nằm đúng cấp khối.
        let mut dong = depth_truoc[i];
        if la_chu_thich && sau[i] != usize::MAX {
            dong = dong.max(depth_truoc[sau[i]]);
        }
        let n = gap.bytes().filter(|b| *b == b'\n').count();
        if n > 0 {
            for _ in 0..n {
                dst.push('\n');
            }
            // Dòng nối: trong ngoặc hoặc bắt đầu bằng `|>` ⇒ giữ thụt lề gốc (chỉ đổi tab ở
            // phần thụt lề, VD-39 không canh biểu thức ở vòng 1).
            if bracket_depth > 0 || t.text == "|>" {
                let tail = &gap[gap.rfind('\n').map_or(0, |k| k + 1)..];
                dst.push_str(&tail.replace('\t', "    "));
            } else {
                push_indent(&mut dst, dong);
            }
        } else if at_line_start {
            push_indent(&mut dst, dong);
        } else {
            // Khoảng trắng **trong dòng**: giữ nguyên văn, kể cả tab (VD-39 chỉ sửa tab thụt lề).
            dst.push_str(gap);
        }
        if la_chu_thich {
            // Chú thích: giữ nguyên văn **thân** (kể cả tab bên trong), chỉ bỏ khoảng trắng cuối.
            dst.push_str(t.text.trim_end());
        } else {
            dst.push_str(&t.text);
        }
        if t.kind == Kind::Punct {
            match t.text.as_str() {
                "(" | "[" | "{" => bracket_depth += 1,
                ")" | "]" | "}" => bracket_depth = bracket_depth.saturating_sub(1),
                _ => {}
            }
        }
        at_line_start = false;
        last_end = t.span.end;
    }

    // Đúng **một** newline cuối tệp (kể cả tệp rỗng ⇒ "\n").
    while dst.ends_with('\n') {
        dst.pop();
    }
    dst.push('\n');
    dst
}

fn push_indent(dst: &mut String, depth: usize) {
    for _ in 0..depth {
        dst.push_str("    ");
    }
}

#[cfg(test)]
mod tests {
    use super::format;

    #[test]
    fn tab_mot_cap_thanh_bon() {
        let f = format("fn f():\n\t1\n").expect("format được");
        assert_eq!(f, "fn f():\n    1\n");
    }

    #[test]
    fn trong_dong_giu_nguyen() {
        let f = format("let x  =  1   # c\n").expect("format được");
        assert_eq!(f, "let x  =  1   # c\n");
    }

    #[test]
    fn loi_tra_ve_ma() {
        let e = format("let x =\n").expect_err("phải lỗi");
        assert_eq!(e.code, "VS0201");
    }

    #[test]
    fn tep_rong_ra_mot_newline() {
        // Tệp rỗng chưa có định nghĩa "đã chuẩn"; `fmt` chọn "\n" (đúng một newline cuối tệp)
        // và "\n" là điểm bất động — ghi ở JOURNAL ST-07 (MINOR review vòng 1).
        assert_eq!(format("").expect("format được"), "\n");
        assert_eq!(format("\n").expect("format được"), "\n");
    }

    #[test]
    fn idempotent() {
        let f = format("fn f():\n\t1\n").expect("format được");
        assert_eq!(format(&f).expect("format lại được"), f);
    }
}
