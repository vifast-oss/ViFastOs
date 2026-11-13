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
    /// Thông điệp tiếng Việt — **nguồn chuẩn** (SPEC-lex §7; test `so_khop_spec_lex_that` khoá).
    pub message_vi: &'static str,
    /// Thông điệp tiếng Anh (ST-03).
    pub message_en: &'static str,
    /// Thông điệp tiếng Nhật (ST-03).
    pub message_ja: &'static str,
    pub severity: Severity,
}

/// Ngôn ngữ thông điệp (ST-03): mặc định `Vi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Vi,
    En,
    Ja,
}

impl Lang {
    /// `vi`/`en`/`ja` (không phân biệt hoa thường); sai ⇒ `None`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "vi" => Some(Self::Vi),
            "en" => Some(Self::En),
            "ja" => Some(Self::Ja),
            _ => None,
        }
    }
}

impl Code {
    /// Thông điệp theo ngôn ngữ; `message_vi` vẫn là chuẩn của SPEC-lex §7.
    pub fn message(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Vi => self.message_vi,
            Lang::En => self.message_en,
            Lang::Ja => self.message_ja,
        }
    }
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
        message_en: "unterminated string",
        message_ja: "文字列が閉じられていません",
        severity: Severity::Error,
    },
    Code {
        id: 102,
        short: "VS0102",
        message_vi: "escape không hợp lệ",
        message_en: "invalid escape",
        message_ja: "無効なエスケープです",
        severity: Severity::Error,
    },
    Code {
        id: 103,
        short: "VS0103",
        message_vi: "ký tự lạ ngoài bảng token",
        message_en: "unexpected character outside the token table",
        message_ja: "トークン表にない不正な文字です",
        severity: Severity::Error,
    },
    Code {
        id: 104,
        short: "VS0104",
        message_vi: "literal số sai/tràn i64",
        message_en: "invalid numeric literal / i64 overflow",
        message_ja: "数値リテラルが不正、または i64 を超過しています",
        severity: Severity::Error,
    },
    Code {
        id: 105,
        short: "VS0105",
        message_vi: "char literal không đúng 1 scalar",
        message_en: "char literal must be exactly one scalar",
        message_ja: "char リテラルはちょうど 1 スカラーである必要があります",
        severity: Severity::Error,
    },
    Code {
        id: 106,
        short: "VS0106",
        message_vi: "byte string có ký tự > 0x7F",
        message_en: "byte string contains a character > 0x7F",
        message_ja: "バイト文字列に 0x7F を超える文字があります",
        severity: Severity::Error,
    },
    Code {
        id: 107,
        short: "VS0107",
        message_vi: "tab (cảnh báo-lỗi; tab = 4)",
        message_en: "tab (warning-error; tab = 4)",
        message_ja: "タブ（警告エラー、タブ = 4）",
        severity: Severity::Warning,
    },
    Code {
        id: 108,
        short: "VS0108",
        message_vi: "nội suy `{` không đóng",
        message_en: "unclosed `{` interpolation",
        message_ja: "補間 `{` が閉じられていません",
        severity: Severity::Error,
    },
    Code {
        id: 109,
        short: "VS0109",
        message_vi: "thụt lề không khớp đơn vị/DEDENT sai",
        message_en: "inconsistent indentation / invalid DEDENT",
        message_ja: "インデントの不一致／DEDENT が不正です",
        severity: Severity::Error,
    },
    Code {
        id: 110,
        short: "VS0110",
        message_vi: "`^` sai ngữ cảnh (lexer chế độ lệnh biết đầu stage)",
        message_en: "`^` out of context (command-mode lexer knows stage start)",
        message_ja: "`^` の文脈が不正です（コマンドモードの字句解析器が stage 先頭を判定）",
        severity: Severity::Error,
    },
];

/// Sổ **mã cú pháp** `02xx` (VD-35) — tách khỏi [`CODES`] để test "CODES khớp SPEC-lex §7" vẫn
/// là hàng rào thật (không nới kỳ vọng cũ). Quy tắc cấp: `VS0201` lỗi cú pháp chung; `VS0202`
/// `%` dùng như phần trăm (thông điệp phải gợi ý `50pct`, L3 của `docs/DUYET-AGENT0-vong-va.md`).
pub const SYNTAX_CODES: [Code; 2] = [
    Code {
        id: 201,
        short: "VS0201",
        message_vi: "lỗi cú pháp: token không mong đợi/thiếu vế",
        message_en: "syntax error: unexpected token / missing operand",
        message_ja: "構文エラー: 予期しないトークン／項が不足しています",
        severity: Severity::Error,
    },
    Code {
        id: 202,
        short: "VS0202",
        message_vi: "`%` dùng như phần trăm — viết `50pct`",
        message_en: "`%` used as percent — write `50pct`",
        message_ja: "`%` がパーセントとして使われています — `50pct` と書いてください",
        severity: Severity::Error,
    },
];

/// Sổ **mã tên** `03xx` (VD-42, ST-11) — sổ **thứ ba**, tách khỏi [`CODES`]/[`SYNTAX_CODES`] vì
/// cùng lý do VD-35: sổ từ vựng vẫn đúng bằng bảng SPEC-lex §7.
///
/// Cấp tối thiểu theo VD-42: `VS0301` tên không tồn tại (ST-11 dùng cho **tên giá trị**; tên kiểu
/// để ST-12), `VS0302` khai báo trùng trong **cùng** phạm vi.
pub const NAME_CODES: [Code; 2] = [
    Code {
        id: 301,
        short: "VS0301",
        message_vi: "tên không tồn tại",
        message_en: "name does not exist",
        message_ja: "名前が見つかりません",
        severity: Severity::Error,
    },
    Code {
        id: 302,
        short: "VS0302",
        message_vi: "khai báo trùng trong cùng phạm vi",
        message_en: "duplicate declaration in the same scope",
        message_ja: "同じスコープで重複した宣言です",
        severity: Severity::Error,
    },
];

/// Sổ **mã kiểu** `04xx` (VD-42, ST-12) — sổ thứ tư. `VS0412` giữ đúng nghĩa "kiểu không khớp"
/// của thiết kế §9; `VS0401` sai số/thứ tự đối số; `VS0402` `match` không xét hết; `VS0403`
/// trường/kiểu không tồn tại.
pub const TYPE_CODES: [Code; 4] = [
    Code {
        id: 401,
        short: "VS0401",
        message_vi: "sai số lượng hoặc thứ tự đối số",
        message_en: "wrong number or order of arguments",
        message_ja: "引数の数または順序が正しくありません",
        severity: Severity::Error,
    },
    Code {
        id: 402,
        short: "VS0402",
        message_vi: "`match` không xét hết trường hợp",
        message_en: "`match` is not exhaustive",
        message_ja: "`match` が網羅的ではありません",
        severity: Severity::Error,
    },
    Code {
        id: 403,
        short: "VS0403",
        // **SỬA KÈM LÝ DO (ST-29)**: mở rộng nghĩa sang **phương thức** (trait/impl) — Agent 0 chốt
        // giữ mã, không cấp `VS0404`; mỗi ca có thông điệp riêng nêu cụ thể.
        message_vi: "trường, kiểu hoặc phương thức không tồn tại",
        message_en: "field, type or method does not exist",
        message_ja: "フィールド・型・メソッドが存在しません",
        severity: Severity::Error,
    },
    Code {
        id: 412,
        short: "VS0412",
        message_vi: "kiểu không khớp",
        message_en: "type mismatch",
        message_ja: "型が一致しません",
        severity: Severity::Error,
    },
];

/// Sổ **mã runtime** `05xx` (VD-42, ST-13) — sổ thứ năm: `VS0501` lỗi runtime chung (tràn `i64`,
/// phép chưa hỗ trợ ở VS0), `VS0502` chia/lấy dư cho 0, `VS0503` chỉ số ngoài phạm vi.
/// Khối `06xx` — **chế độ lệnh** (ST-18, VD-52). Bốn mã, đúng phạm vi VS0:
/// lệnh không có trong bảng chữ ký, sai đối số, pipeline không nối được, tính năng chưa hỗ trợ.
pub const COMMAND_CODES: [Code; 4] = [
    Code {
        id: 601,
        short: "VS0601",
        message_vi: "không có lệnh này: tên không có trong bảng chữ ký (cmd/builtin/extern)",
        message_en: "no such command: name not found in the signature table (cmd/builtin/extern)",
        message_ja: "そのコマンドはありません: シグネチャ表（cmd/builtin/extern）にありません",
        severity: Severity::Error,
    },
    Code {
        id: 602,
        short: "VS0602",
        message_vi: "sai đối số lệnh: thiếu, thừa hoặc sai kiểu",
        message_en: "bad command argument: missing, extra or wrong type",
        message_ja: "コマンド引数が不正です: 不足・過剰・型違い",
        severity: Severity::Error,
    },
    Code {
        id: 603,
        short: "VS0603",
        message_vi: "pipeline không nối được: kiểu ra của stage trước ≠ kiểu vào của stage sau",
        message_en: "pipeline not connectable: previous stage output type ≠ next stage input type",
        message_ja: "パイプが接続できません: 前段の出力型と次段の入力型が異なります",
        severity: Severity::Error,
    },
    Code {
        id: 604,
        short: "VS0604",
        message_vi: "tính năng chưa hỗ trợ ở VS0 (ví dụ chạy nền `&`)",
        message_en: "feature not supported in VS0 (e.g. background `&`)",
        message_ja: "VS0 では未対応の機能です（例: バックグラウンド `&`）",
        severity: Severity::Error,
    },
];

pub const RUNTIME_CODES: [Code; 3] = [
    Code {
        id: 501,
        short: "VS0501",
        message_vi: "lỗi runtime: phép toán hoặc tính năng chưa hỗ trợ ở VS0",
        message_en: "runtime error: operation or feature not supported in VS0",
        message_ja: "実行時エラー: VS0 では未対応の演算または機能です",
        severity: Severity::Error,
    },
    Code {
        id: 502,
        short: "VS0502",
        message_vi: "chia hoặc lấy dư cho 0",
        message_en: "division or remainder by zero",
        message_ja: "0 による除算または剰余",
        severity: Severity::Error,
    },
    Code {
        id: 503,
        short: "VS0503",
        message_vi: "chỉ số ngoài phạm vi",
        message_en: "index out of range",
        message_ja: "インデックスが範囲外です",
        severity: Severity::Error,
    },
];

/// **Sổ thứ bảy** — `07xx` module (ST-25, VD-57): `use` không thấy tệp, vòng phụ thuộc, `use` trùng.
///
/// `07xx` được cấp ở **ST-25** cho khung module; `std.*` vẫn **hoãn** (ST-26) và `pkg.*` hoãn (VS2)
/// — cả hai báo `VS0701` với thông điệp riêng (VD-57).
pub const MODULE_CODES: [Code; 3] = [
    Code {
        id: 701,
        short: "VS0701",
        message_vi: "không thấy tệp module (hoặc `std`/`pkg` chưa hỗ trợ)",
        message_en: "module file not found (or `std`/`pkg` not supported)",
        message_ja: "モジュールファイルが見つかりません (または `std`/`pkg` は未対応)",
        severity: Severity::Error,
    },
    Code {
        id: 702,
        short: "VS0702",
        message_vi: "vòng phụ thuộc giữa các tệp module",
        message_en: "dependency cycle between module files",
        message_ja: "モジュールファイル間の循環依存",
        severity: Severity::Error,
    },
    Code {
        id: 703,
        short: "VS0703",
        message_vi: "`use` trùng hoặc mơ hồ (hai đường dẫn cùng một module)",
        message_en: "duplicate or ambiguous `use` (two paths to the same module)",
        message_ja: "重複または曖昧な `use` (同じモジュールへの2つの経路)",
        severity: Severity::Error,
    },
];

/// **Năm sổ** mã lỗi đã cấp ở ST-13: từ vựng `01xx`, cú pháp `02xx`, tên `03xx`, kiểu `04xx`,
/// runtime `05xx` (VD-27/VD-35/VD-42). Nay **mọi** khối dành trước đều đã có mã; `06xx` là khối
/// kế tiếp chưa cấp nên [`next_block`] vẫn trả **601**.
pub const BOOKS: [&[Code]; 7] = [
    &CODES,
    &SYNTAX_CODES,
    &NAME_CODES,
    &TYPE_CODES,
    &RUNTIME_CODES,
    &COMMAND_CODES,
    &MODULE_CODES,
];

/// Khối mã **đã dành trước** theo sơ đồ VD-27 (`01xx`..`06xx`); VD-42 cấp `04xx` ở ST-12, `05xx` ở
/// ST-13, ST-18 cấp `06xx`. Dùng cho [`next_block`] để khối kế không rơi vào khối đã dành.
pub const BLOCKS_RESERVED: [u16; 7] = [1, 2, 3, 4, 5, 6, 7];

/// Duyệt **cả bốn sổ** đã cấp, theo đúng thứ tự khối.
pub fn all_codes() -> impl Iterator<Item = &'static Code> {
    BOOKS.iter().flat_map(|b| b.iter())
}

/// Tra một mã theo số (ví dụ `103` ⇒ `VS0103`). `None` nếu chưa cấp.
///
/// Tra **cả bốn sổ** đã cấp (VD-42): từ vựng `01xx`, cú pháp `02xx`, tên `03xx`, kiểu `04xx`.
/// Nay đủ **sáu sổ** (`01xx`..`06xx`); `lookup(701)` mới là `None`.
/// [`CODES`]/[`SYNTAX_CODES`] vẫn nguyên vẹn nên test "khớp SPEC-lex §7" không đổi.
pub fn lookup(id: u16) -> Option<&'static Code> {
    all_codes().find(|c| c.id == id)
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
///
/// Suy ra từ **khối** đã cấp ([`all_codes`]: `01xx`..`05xx`) **và** khối đã **dành trước**
/// ([`BLOCKS_RESERVED`]: `01xx`..`06xx`) ⇒ `max(3, 6) + 1 = 07` ⇒ **`701`** (VD-42, ST-18). Không
/// hardcode `701`; thêm sổ mới thì số tự nhảy.
pub fn next_block() -> u16 {
    // Suy ra từ **khối** đã cấp (mã lớn nhất / 100) và khối đã dành trước — không hardcode số.
    let issued = all_codes().map(|c| c.id / 100).max().unwrap_or(0);
    let reserved = BLOCKS_RESERVED.iter().copied().max().unwrap_or(0);
    let block = issued.max(reserved) + 1;
    block * 100 + 1
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

/// Một chẩn đoán đã có vị trí **theo Unicode scalar** (1-based) để [`render`] hiển thị.
///
/// `col`/`line` do lexer/parser tính (lexer đếm `char`, tab = 4 — SPEC-lex §4); ST-03 chỉ
/// **render** chứ không tự đoán lại vị trí.
#[derive(Debug, Clone, Copy)]
pub struct DiagLine<'a> {
    pub path: &'a str,
    pub line: u32,
    pub col: u32,
    pub severity: Severity,
    pub code: &'a str,
    pub message: &'a str,
    /// Dòng nguồn (chưa có `\n`) để in con trỏ `^`; `None` ⇒ chỉ in dòng tiêu đề.
    pub source: Option<&'a str>,
}

/// Nhãn mức theo ngôn ngữ.
pub fn severity_label(sev: Severity, lang: Lang) -> &'static str {
    match (sev, lang) {
        (Severity::Error, Lang::Vi) => "lỗi",
        (Severity::Warning, Lang::Vi) => "cảnh báo",
        (Severity::Error, Lang::En) => "error",
        (Severity::Warning, Lang::En) => "warning",
        (Severity::Error, Lang::Ja) => "エラー",
        (Severity::Warning, Lang::Ja) => "警告",
    }
}

/// Render một chẩn đoán: `<tệp>:<dòng>:<cột>: <mức> <mã>: <thông điệp>`; có `source` thì in
/// thêm dòng nguồn và con trỏ `^` **canh theo scalar** (tab tính 4 cột như lexer).
pub fn render(d: &DiagLine<'_>, lang: Lang) -> String {
    let mut ra = format!(
        "{}:{}:{}: {} {}: {}",
        d.path,
        d.line,
        d.col,
        severity_label(d.severity, lang),
        d.code,
        d.message
    );
    if let Some(src) = d.source {
        let src = src.trim_end_matches(['\n', '\r']);
        ra.push('\n');
        ra.push_str(src);
        // Con trỏ canh theo **scalar**, tab = 4 cột (khớp cách lexer đếm).
        let want = d.col.saturating_sub(1) as usize;
        let mut width = 0usize;
        for ch in src.chars() {
            if width >= want {
                break;
            }
            width += if ch == '\t' { 4 } else { 1 };
        }
        ra.push('\n');
        ra.push_str(&" ".repeat(width));
        ra.push('^');
    }
    ra
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
        // 01xx (từ vựng) + 02xx (cú pháp) + 03xx (tên, VD-42) đã cấp, và 04xx/05xx đã **dành
        // trước** (VD-27) ⇒ khối kế là 06xx. Sửa kỳ vọng cũ 301 kèm lý do: VD-42 cấp thêm `03xx`
        // và dành trước `04xx`/`05xx`; `next_block` nay suy ra từ khối, không đếm mã.
        assert_eq!(
            next_block(),
            801,
            // **SỬA KÈM LÝ DO (ST-25)**: `07xx` đã cấp 3 mã module (`VS0701..0703`) ⇒ khối kế `08xx` (801).
            "01xx..07xx đã cấp/dành trước ⇒ khối kế 08xx (801)"
        );
    }

    #[test]
    fn so_runtime_05xx_khoa_theo_vd42() {
        let ids: Vec<u16> = RUNTIME_CODES.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![501, 502, 503], "sổ runtime đúng thứ tự tăng dần");
        for c in RUNTIME_CODES.iter() {
            assert_eq!(lookup(c.id).map(|x| x.short), Some(c.short));
            assert_eq!(lookup_str(c.short).map(|x| x.id), Some(c.id));
            for lang in [Lang::Vi, Lang::En, Lang::Ja] {
                assert!(!c.message(lang).is_empty(), "{} thiếu {:?}", c.short, lang);
            }
        }
        assert!(
            RUNTIME_CODES[2].message_vi.contains("ngoài phạm vi"),
            "VS0503 nói 'ngoài phạm vi'"
        );
    }

    #[test]
    fn so_kieu_04xx_khoa_theo_vd42() {
        let ids: Vec<u16> = TYPE_CODES.iter().map(|c| c.id).collect();
        assert_eq!(
            ids,
            vec![401, 402, 403, 412],
            "sổ kiểu đúng thứ tự tăng dần"
        );
        assert_eq!(TYPE_CODES[3].short, "VS0412");
        assert_eq!(
            TYPE_CODES[3].message_vi, "kiểu không khớp",
            "VS0412 giữ đúng nghĩa thiết kế §9"
        );
        for c in TYPE_CODES.iter() {
            assert_eq!(lookup(c.id).map(|x| x.short), Some(c.short));
            assert_eq!(lookup_str(c.short).map(|x| x.id), Some(c.id));
            for lang in [Lang::Vi, Lang::En, Lang::Ja] {
                assert!(!c.message(lang).is_empty(), "{} thiếu {:?}", c.short, lang);
            }
        }
        assert!(
            TYPE_CODES[1].message_vi.contains("xét hết"),
            "VS0402 phải nói 'xét hết'"
        );
    }

    #[test]
    fn so_ten_03xx_khoa_theo_vd42() {
        let ids: Vec<u16> = NAME_CODES.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![301, 302]);
        assert_eq!(NAME_CODES[0].short, "VS0301");
        assert_eq!(NAME_CODES[1].short, "VS0302");
        // **SỬA KÈM LÝ DO (ST-18)**: ST-18 cấp sổ thứ **sáu** `06xx` (4 mã chế độ lệnh) ⇒
        // 10 + 2 + 2 + 4 + 3 + 4 = **25** mã, **6** sổ; `lookup(601)` nay **có**, mã chưa cấp là `701`.
        // **SỬA KÈM LÝ DO (ST-25)**: sổ thứ bảy `07xx` (module) đã cấp ⇒ 25 + 3 = **28** mã,
        // `BOOKS` = **7**; khối kế tiếp là `801` (xem `next_block` bên dưới).
        assert_eq!(all_codes().count(), 28);
        assert_eq!(BOOKS.len(), 7);
        for id in [601u16, 602, 603, 604] {
            assert!(
                lookup(id).is_some(),
                "mã {id} phải có ở ST-18 (khối `06xx`)"
            );
        }
        // **SỬA KÈM LÝ DO (ST-25)**: `07xx` nay đã cấp cho module (`VS0701/0702/0703`).
        for id in [701u16, 702, 703] {
            assert!(
                lookup(id).is_some(),
                "mã {id} phải có ở ST-25 (khối `07xx` module)"
            );
        }
        assert!(
            lookup(801).is_none(),
            "mã 801 chưa cấp — khối `08xx` là của vòng sau"
        );
        // Khối đã dành trước vẫn chặn `next_block` rơi vào 01xx..07xx.
        assert_eq!(BLOCKS_RESERVED, [1, 2, 3, 4, 5, 6, 7]);
        for c in NAME_CODES.iter() {
            for lang in [Lang::Vi, Lang::En, Lang::Ja] {
                assert!(!c.message(lang).is_empty(), "{} thiếu {:?}", c.short, lang);
            }
        }
    }

    #[test]
    fn so_cu_phap_02xx_khoa_theo_vd35() {
        // Sổ cú pháp riêng: 2 mã, đúng thứ tự, đúng nghĩa, và `CODES` KHÔNG bị đụng.
        let ids: Vec<u16> = SYNTAX_CODES.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![201, 202]);
        assert_eq!(SYNTAX_CODES[0].short, "VS0201");
        assert_eq!(SYNTAX_CODES[1].short, "VS0202");
        assert!(
            SYNTAX_CODES[1].message_vi.contains("50pct"),
            "VS0202 phải gợi ý `50pct` (L3)"
        );
        assert_eq!(CODES.len(), 10, "sổ từ vựng vẫn đúng 10 mã của §7");
        // Tra xuyên sổ: `vis explain VS0201` phải chạy.
        for (i, c) in SYNTAX_CODES.iter().enumerate() {
            assert_eq!(lookup(201 + i as u16).map(|x| x.short), Some(c.short));
            assert_eq!(lookup_str(c.short).map(|x| x.id), Some(c.id));
        }
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
