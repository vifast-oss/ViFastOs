# Hợp đồng của `check-wake-paths.py` — KT-184 (D2-160 + D2-161)

**Trạng thái**: bản do Agent 1 soạn theo yêu cầu **A0 2026-11-19** ("ghi rõ hợp đồng của checker").
**Chưa được reviewer độc lập xác nhận** — KT-184 vẫn `REVIEW`.

Máy kiểm này **không** phải bộ phân tích Rust tổng quát. Nó là **máy kiểm từ vựng** (lexical) trên
cây nguồn, chạy được **không cần biên dịch**, và **fail-closed** ở mọi chỗ nó không phân giải được.

---

## 1. Phạm vi cây nguồn được bảo vệ

| Cây | Luật áp |
|---|---|
| `v2/kernel/**` | `include!`/`#[path]`/`cfg_attr(path)` literal + trong cây; `wake` chỉ-dùng-test; macro body |
| `v2/sim/**` | như trên |
| `v2/lib/**` | thân `macro_rules!` (token `wake`, `include!`, `#[path]`, `mod`, `:tt`) |
| `v2/test/**`, `v2/tools/**`, `v2/agent/**` | **không** bảo vệ (ngoài phạm vi) |

Đích của `include!`/`#[path]` phải nằm trong `v2/kernel` hoặc `v2/sim`; trỏ ra ngoài ⇒ **ĐỎ**.

## 2. Dạng **được hỗ trợ** (phân giải được)

- `include!("x.rs")`, `include!["x.rs"]`, `include!{"x.rs"}`, `include ! ( "x.rs" )`.
- Chuỗi thường `"x.rs"`, chuỗi thô `r"x.rs"`, `r#"x.rs"#`, `r##"x.rs"##`.
- `#[path = "x.rs"]`, `#[ path = "x.rs" ]`, `#[\npath = "x.rs"]`.
- `#[cfg_attr(not(test), path = "x.rs")]` (một tầng `cfg_attr`).
- **KHÔNG** hỗ trợ: `#[r#path = "…"]` (raw identifier ở tên thuộc tính — rustc coi là `#[path]`),
  chuỗi có **escape** (`"a\x2eb"`, `"a\u{2e}b"`), `cfg_attr` **lồng** nhiều tầng. Ba dạng này
  **bị từ chối** (xem §3) — reviewer L30 (KT-192) đã chứng minh chúng lọt bản trước.
- `mod NAME;` khai báo thường (đồ thị module, kể cả `#[cfg(test)]`/`#[cfg(feature = "ktest")]`).
- `macro_rules!` với **mọi** tên (kể cả Unicode) và **cả ba** dấu thân `{ … }`, `( … )`, `[ … ]`.
- Chú thích chen giữa `#`/`[`/`path` (khử chú thích trước khi khớp).

## 3. Dạng **bị từ chối có chủ đích** (fail-closed)

| Dạng | Mã lỗi | Lý do |
|---|---|---|
| `include!`/`#[path]`/`cfg_attr(path)` **không literal** (`concat!`, `env!`, biến) | `E_STRICT_WAKE_INCLUDE` | không phân giải được đích |
| Đích trỏ **ra ngoài** `v2/kernel`+`v2/sim` | `E_STRICT_WAKE_INCLUDE` | không quét được |
| Đích **không tồn tại** | `E_STRICT_WAKE_INCLUDE` | fail-closed |
| Thân `macro_rules!` chứa `include`/`path`/`mod` (bất kỳ dạng) | `E_STRICT_MACRO_INCLUDE` | rustc giải theo **tệp gọi**, không phân giải tĩnh được |
| Thân `macro_rules!` chứa `$…:tt` (passthrough) | `E_STRICT_MACRO_TT` | có thể chở `mod`/`include!` từ **lời gọi** |
| Thân `macro_rules!` khai `mod …;` | `E_STRICT_MACRO_MOD` (literal) / `E_STRICT_MACRO_INCLUDE` (`mod $n;`, `mod r#x;`) | đồ thị module không thấy |
| `#[r#path = "…"]` (raw identifier) | `E_STRICT_WAKE_INCLUDE` | rustc coi là `#[path]`; checker không phân giải |
| Chuỗi có **escape** (`\x2e`, `\u{2e}`) trong `include!`/`#[path]` | `E_STRICT_WAKE_INCLUDE` | checker lấy literal thô, rustc giải mã |
| `cfg_attr` **lồng** nhiều tầng | `E_STRICT_WAKE_INCLUDE` | không phân giải tĩnh an toàn |
| `wake(` trong mã sản phẩm **thuộc `TEST_ONLY_WAKE_SCOPE`** | `E_STRICT_TEST_ONLY_WAKE` | D2-144; `wake` trong `v2/lib` sản phẩm **không** bị luật này phủ |

## 4. Điều checker **KHÔNG** bảo đảm

- **Ngữ nghĩa mở rộng macro** — không mô phỏng `macro_rules!`; chỉ từ chối thân macro có dấu hiệu.
- **Proc-macro** (`paste!`, `concat_idents!`) — không có trong allowlist phụ thuộc; nếu thêm, phải
   mở rộng hợp đồng này.
- **Mã sinh lúc build** (`build.rs`, `OUT_DIR`) — ngoài phạm vi; `include!(concat!(env!("OUT_DIR"), …))`
   sẽ **bị từ chối** (fail-closed), không phải báo oan.
- **`#[cfg]` phức tạp** — chỉ hiểu `#[cfg(test)]` và `#[cfg(feature = "ktest")]`; `cfg_attr` lồng
   nhiều tầng có thể không phân giải.
- **Đường dẫn qua symlink** — phân giải theo `Path::resolve`, không theo symlink của rustc.
- **Code nằm ngoài `v2/kernel`/`v2/sim`** — không quét.

## 5. Cách chạy

```bash
python3 v2/tools/check-wake-paths.py            # chế độ thường (include!/#[path] + macro)
python3 v2/tools/check-wake-paths.py --strict   # + luật wake chỉ-dùng-test + allowlist
python3 v2/tools/check-wake-paths.py --selftest # số ca hiện tại
```

Cả ba chế độ chạy trong `./v2/bin/v2 check` và `gate --fast`.

## 6. Bảo trì

- Mọi dạng mới **phải** thêm vào §2 (hỗ trợ) hoặc §3 (từ chối) **trước** khi vá regex.
- Mọi ca hồi quy nằm trong `--selftest`, mỗi ca có **đối chứng dương**.
- Nếu vòng review thứ tư còn tìm được đường lọt **cùng bản chất**, A0 yêu cầu **dừng vá cục bộ** và
  trình phương án lexer/token-tree — xem quyết định A0 2026-11-19 §2.
