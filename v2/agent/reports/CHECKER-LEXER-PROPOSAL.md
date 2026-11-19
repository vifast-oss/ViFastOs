# Phương án thay lớp từ vựng của `check-wake-paths.py` — trình A0 (D2-165 §2e)

**Trạng thái**: đề xuất, **chưa làm**. A0 yêu cầu: nếu vòng review thứ tư còn tìm được đường lọt
**cùng bản chất** thì **dừng vá cục bộ** và trình phương án lexer/token-tree kèm phạm vi thay thế,
**không** tự mở viết lại lớn.

**Căn cứ**: reviewer L30 (KT-192) tìm được ba đường lọt **cùng một bản chất** — checker đang so
**văn bản thô** trong khi rustc làm việc trên **token**:

| Đường lọt | Vì sao bản regex không thấy |
|---|---|
| `#[r#path = "t.rs"]` | tên thuộc tính là **raw identifier**; rustc coi là `#[path]` |
| `#[path = "evil_t\x2ers"]` | chuỗi có **escape**; rustc giải mã `\x2e` → `.` |
| `cfg_attr` **lồng** | cần phân tích cú pháp thuộc tính, không phải khớp chuỗi |

Cộng với sáu vòng trước (H4 → B1/B2/B3 → B2c/B3a/B2d/T3c → Unicode/`mod $n`/`r#x`/`tt`/chú thích),
tổng **bảy vòng** cùng một lớp lỗi. Vá thêm regex chỉ dịch chuyển điểm mù.

---

## 1. Phạm vi thay thế

**Chỉ thay lớp từ vựng** (đọc tệp → token), **giữ nguyên**:

- CLI (`--root`, `--strict`, `--json`, `--selftest`).
- Mã lỗi (`E_STRICT_WAKE_INCLUDE`, `E_STRICT_MACRO_INCLUDE`, `E_STRICT_MACRO_MOD`,
  `E_STRICT_MACRO_TT`, `E_STRICT_TEST_ONLY_WAKE`, …).
- Toàn bộ luật nghiệp vụ (đồ thị module, `test_only_modules`, fixpoint `include!`, allowlist).
- Toàn bộ ca `--selftest` hiện có (phải xanh lại hết).

**Không** thay: `check-test-teeth.py`, `verify-numbers.py`, `v2/bin/v2`, bất kỳ mã sản phẩm nào.

## 2. Thiết kế

### 2.1 Tokenizer (khoảng hai trăm dòng)

Một hàm `tokenize(text) -> List[Token]` thuần Python, không phụ thuộc crate:

| Token | Xử lý |
|---|---|
| `Ident(name)` | ASCII + Unicode; **raw** `r#name` giữ cờ `raw=True` |
| `Str(value)` | chuỗi thường **giải escape** (`\n`, `\t`, `\xNN`, `\u{…}`, `\\`, `\"`); raw string `r"…"`, `r#"…"#` giữ nguyên nội dung |
| `Punct(ch)` | `#`, `[`, `]`, `(`, `)`, `{`, `}`, `!`, `;`, `,`, `=`, `::`, `$` |
| `Comment` | bỏ (không sinh token) |
| `Lifetime`/`Char` | nhận đúng để không nhầm với `'` |

Điểm mấu chốt: **`#[r#path]` cho `Ident("path", raw=True)`**, và **`"a\x2eb"` cho `Str("a.b")`** —
hai thứ bản regex không làm được.

### 2.2 Duyệt cấu trúc (khoảng một trăm năm mươi dòng)

Trên dòng token, thay các regex hiện tại bằng **máy trạng thái nhỏ**:

- **Thuộc tính**: gặp `#` `[` … `]` ⇒ đọc tên thuộc tính (`path`, `cfg_attr`, `cfg`, …), kể cả raw.
  - `path = Str` ⇒ cạnh `#[path]`.
  - `cfg_attr ( … , path = Str … )` ⇒ cạnh `#[cfg_attr(path)]`; **lồng** ⇒ đệ quy có giới hạn độ sâu
    (quá giới hạn ⇒ fail-closed).
- **Macro**: `Ident("macro_rules")` `!` `Ident(name)` + delimiter bất kỳ ⇒ quét **token** trong thân:
  `include`/`path`/`mod` (kể cả raw) ⇒ ĐỎ; `$` … `:` `tt` ⇒ ĐỎ.
- **`include!`**: `Ident("include")` `!` delimiter `Str` ⇒ cạnh; `Str` không có ⇒ fail-closed.
- **`mod`**: `Ident("mod")` `Ident(name)` `;` ⇒ cạnh đồ thị module.

### 2.3 Giữ nguyên phần "nghiệp vụ"

`test_only_modules`, `resolve_include_targets` (fixpoint), `scan_v2` — **không đổi**, chỉ nhận
đầu vào là **cạnh đã phân giải** thay vì tự regex.

## 3. Ca hồi quy

- **Toàn bộ ca `--selftest` hiện có** phải xanh (không đổi kỳ vọng).
- **Thêm** ca cho đúng ba đường lọt của L30: `#[r#path]`, escape `\x2e`/`\u{2e}`, `cfg_attr` lồng —
  mỗi ca có **đối chứng dương** (dạng hợp lệ tương ứng phải xanh).
- **Thêm** ca đối chứng dương độc lập cho các nhóm L30 đã kiểm (P1–P12 trong journal KT-192).

## 4. Ngân sách và rủi ro

| Mục | Ước lượng |
|---|---|
| Tokenizer + duyệt cấu trúc | khoảng ba trăm dòng, một task trong ngân sách |
| Chuyển ca selftest | một hai giờ (cơ học) |
| Review độc lập | một vòng (D2-159) |

**Rủi ro**:

- **Tokenizer tự viết cũng có thể sai** — nhưng sai ở tầng **token** thì dễ phát hiện hơn (so với
   regex), và có thể đối chiếu với `rustc -Zunpretty=expanded` trên cây thật.
- **Không phủ proc-macro** — vẫn ngoài phạm vi, giữ fail-closed.
- **Hiệu năng** — toàn bộ tệp trong cây; tokenize ước dưới hai giây (đo sau khi làm).

## 5. Đề nghị

- **Làm thành task riêng** (`KT-194`), **không** gộp vào KT-184.
- KT-184 **giữ REVIEW** cho tới khi có bản lexer + review độc lập.
- Trong lúc chờ: **không** vá thêm regex cho ba đường lọt của L30 — chấp nhận **giới hạn đã công bố**
  trong hợp đồng (§2/§3 đã ghi rõ ba dạng bị từ chối).

---

**Ký**: Agent 1, 2026-11-19.
