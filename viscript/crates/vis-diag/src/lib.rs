// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis-diag` — khung chẩn đoán của dòng ViScript (ST-01).
//!
//! Vòng 1 mới có **định dạng mã lỗi** `VS` + 4 chữ số (sổ mã đầy đủ thuộc ST-02b).
//! Không `unsafe` (luật dòng ViScript), không phụ thuộc ngoài.

/// Định dạng một mã lỗi ViScript: `VS` + **đúng 4 chữ số** (VD: `VS0101`).
///
/// # Panics
/// `n > 9999` — SPEC hứa 4 chữ số; cấp quá là lỗi lập trình, không được lặng lẽ thành 5 chữ số.
pub fn code(n: u16) -> String {
    assert!(n <= 9999, "mã lỗi phải 4 chữ số (0..=9999), nhận {n}");
    format!("VS{n:04}")
}

/// Mức của một mã: lỗi hay cảnh báo-lỗi (tab vẫn lex được nhưng phải báo — SPEC-lex §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Một dòng trong sổ mã lỗi. Số **không tái dùng** (quy tắc cấp, xem `docs` của crate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Code {
    pub id: u16,
    pub short: &'static str,
    pub message_vi: &'static str,
    pub severity: Severity,
}

/// Sổ mã lỗi **vòng 1**: đúng 10 mã `VS0101..VS0110` của `docs/viscript/SPEC-lex.md` §7.
///
/// **ĐỀ XUẤT quy tắc cấp** (chưa phải chuẩn — chờ Agent 0 duyệt qua YC-14; SPEC-lex là nơi
/// *định nghĩa* chuẩn, crate này chỉ *thực thi*):
/// - mã 4 chữ số thập phân, khối theo tầng: `01xx` từ vựng (lexer), `02xx` cú pháp/parser,
///   `03xx` tên/phân giải, `04xx` **kiểu** (`VS0412` = "kiểu không khớp" đã có trong thiết kế),
///   `05xx` runtime; cấp tăng dần, **không tái dùng** số đã cấp;
/// - mọi mã phải có mặt trong SPEC-lex/errata trước khi vào sổ; `message_vi` ở đây là **placeholder
///   vòng 1** — bảng vi/en/ja thuộc ST-03;
/// - đổi nghĩa/đánh số lại phải qua Agent 0 (VD-08), không tự sửa.
pub const CODES: [Code; 10] = [
    Code {
        id: 101,
        short: "VS0101",
        message_vi: "chuỗi chưa đóng",
        severity: Severity::Error,
    },
    Code {
        id: 102,
        short: "VS0102",
        message_vi: "escape không hợp lệ",
        severity: Severity::Error,
    },
    Code {
        id: 103,
        short: "VS0103",
        message_vi: "ký tự lạ ngoài bảng token",
        severity: Severity::Error,
    },
    Code {
        id: 104,
        short: "VS0104",
        message_vi: "literal số sai/tràn i64",
        severity: Severity::Error,
    },
    Code {
        id: 105,
        short: "VS0105",
        message_vi: "char literal không đúng 1 scalar",
        severity: Severity::Error,
    },
    Code {
        id: 106,
        short: "VS0106",
        message_vi: "byte string có ký tự > 0x7F",
        severity: Severity::Error,
    },
    Code {
        id: 107,
        short: "VS0107",
        message_vi: "tab (cảnh báo-lỗi; tab = 4)",
        severity: Severity::Warning,
    },
    Code {
        id: 108,
        short: "VS0108",
        message_vi: "nội suy `{` không đóng",
        severity: Severity::Error,
    },
    Code {
        id: 109,
        short: "VS0109",
        message_vi: "thụt lề không khớp đơn vị/DEDENT sai",
        severity: Severity::Error,
    },
    Code {
        id: 110,
        short: "VS0110",
        message_vi: "`^` sai ngữ cảnh (lexer chế độ lệnh biết đầu stage)",
        severity: Severity::Error,
    },
];

/// Tra một mã theo số (ví dụ `103` ⇒ `VS0103`). `None` nếu chưa cấp.
pub fn lookup(id: u16) -> Option<&'static Code> {
    CODES.iter().find(|c| c.id == id)
}

/// Tra theo chuỗi `VSxxxx` (chấp nhận `vs0103`); `None` nếu sai định dạng hoặc chưa cấp.
pub fn lookup_str(s: &str) -> Option<&'static Code> {
    let t = s.trim();
    let digits = t.strip_prefix("VS").or_else(|| t.strip_prefix("vs"))?;
    if digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    lookup(digits.parse().ok()?)
}

/// Khối mã kế tiếp còn trống (quy tắc cấp): trả số đầu của khối mới, không tái dùng.
pub fn next_block() -> u16 {
    let phase = CODES.iter().map(|c| c.id / 100).max().unwrap_or(0) + 1;
    phase * 100 + 1
}

/// Chẩn đoán tối thiểu: mã **đã cấp trong sổ** + thông điệp + vị trí byte (0-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub code: &'static str,
    pub msg: String,
    pub offset: usize,
}

impl Diag {
    /// Chỉ nhận `Code` lấy từ [`lookup`]/[`CODES`] — không có đường tạo mã ngoài sổ.
    pub fn new(code: &'static Code, msg: impl Into<String>, offset: usize) -> Self {
        Self {
            code: code.short,
            msg: msg.into(),
            offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ma_co_bon_chu_so() {
        assert_eq!(code(1), "VS0001");
        assert_eq!(code(9999), "VS9999");
    }

    #[test]
    fn so_ma_lien_mach_va_dung_spec_lex() {
        let ids: Vec<u16> = CODES.iter().map(|c| c.id).collect();
        assert_eq!(ids.first(), Some(&101));
        assert_eq!(ids.last(), Some(&110));
        assert_eq!(ids.len(), 10);
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(CODES[i].id, 101 + i as u16, "sổ phải liền mạch 0101..0110");
            assert_eq!(CODES[i].short, format!("VS{id:04}"));
            assert!(!CODES[i].message_vi.is_empty());
        }
    }

    #[test]
    fn tra_ma_va_ma_la() {
        assert_eq!(lookup(103).map(|c| c.short), Some("VS0103"));
        assert!(lookup(111).is_none());
        assert_eq!(
            lookup_str("VS0107").map(|c| c.severity),
            Some(Severity::Warning)
        );
        assert_eq!(lookup_str("vs0107").map(|c| c.id), Some(107));
        assert!(lookup_str("VS010").is_none());
        assert!(lookup_str("XX0101").is_none());
    }

    #[test]
    fn khoi_ke_tiep_khong_tai_dung() {
        assert_eq!(
            next_block(),
            201,
            "khối từ vựng đã dùng 01xx ⇒ khối kế 02xx"
        );
    }

    #[test]
    fn so_khop_spec_lex_that() {
        // Đọc thẳng bảng §7 của SPEC-lex: sổ trong code không được lệch nguồn chuẩn.
        let spec = std::fs::read_to_string("../../../docs/viscript/SPEC-lex.md")
            .expect("phải đọc được docs/viscript/SPEC-lex.md");
        let mut rows: Vec<(String, String)> = Vec::new();
        for line in spec.lines() {
            let t = line.trim();
            if t.starts_with("| VS") {
                let cells: Vec<&str> = t.trim_matches('|').split('|').map(|c| c.trim()).collect();
                if cells.len() >= 2 {
                    rows.push((cells[0].to_string(), cells[1].to_string()));
                }
            }
        }
        assert_eq!(
            rows.len(),
            CODES.len(),
            "SPEC-lex §7 phải có {} hàng",
            CODES.len()
        );
        for (row, code) in rows.iter().zip(CODES.iter()) {
            assert_eq!(row.0, code.short);
            assert_eq!(row.1, code.message_vi);
        }
    }

    #[test]
    fn chan_doan_giu_vi_tri() {
        let d = Diag::new(&CODES[2], "token lạ", 7);
        assert_eq!(d.msg, "token lạ");
        assert_eq!(d.offset, 7);
    }
}
