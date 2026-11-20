# KT-242 — vá bốn lệch hợp đồng ảnh hưởng luật `wake` (P0-1, P0-2, P0-3, P1-1)

**Ngày**: 2026-11-20. **Nhánh**: `task/wake-review` (worktree `/tmp/kt242-s2`).
**Tiền đề**: `JOURNAL/kt242-wake-review.md` (review độc lập, commit `5546baa0`) — sáu đường lọt
N1–N6, verdict KT-184/KT-194 **CHƯA ĐÓNG**. Lead chấp nhận verdict và giao nhóm C của
`KE-HOACH-CUOI-TUAN-2026-11-20.md`: "khép lệch hợp đồng ảnh hưởng luật wake".

**Phạm vi**: chỉ `v2/tools/check-wake-paths.py`, `v2/tools/check-wake-depinfo.py`,
`v2/agent/reports/CHECKER-WAKE-CONTRACT.md`, `v2/agent/JOURNAL/kt242-wake-repro/**`. **Không**
sửa mã sản phẩm, **không** sửa `spec/`, **không** sửa `QUEUE.md`/`DECISIONS.md`, **không** nới
ngưỡng, **không** tắt ca.

---

## 0. Số đo trước/sau

| Hạng mục | Trước | Sau |
|---|---|---|
| selftest lớp tĩnh (`check-wake-paths.py --selftest`) | **384/384 ok** (38.3 s) | **406/406 ok** (50.9 s) |
| selftest lớp dep-info (`check-wake-depinfo.py --selftest`) | **62/62 ok** (1.6 s) | **65/65 ok** (1.7 s) |
| lớp tĩnh, cây thật, chế độ thường | `errors=0` (67 s) | `errors=0` (**36.7 s**) |
| lớp tĩnh, cây thật, `--strict` | `errors=0` (67 s) | `errors=0` (**51.4 s**) |
| lớp dep-info, cây thật | `errors=0` (17.6 s) | `errors=0` (23.5 s) |
| `verify-numbers.py` (luật I) | `ok=5 fail=0` | `ok=5 fail=0` |
| `v2 check` (toàn bộ) | PASS | **PASS** (279.6 s) |

Lớp tĩnh **nhanh hơn** dù thêm việc, nhờ **hai** thay đổi:

1. `_iter_rs_ci` **cắt sớm** thư mục build (`target/`) bằng `os.scandir`, trong khi
   `rglob("*.rs")` đi hết rồi mới lọc;
2. `_test_char_ranges` có **cache** theo `(relp, độ dài, hash)` — trước bản vá mỗi tệp bị token
   hoá **hai** lần (`scan_v2` cho luật kênh/consumer/arch + `_check_wake_text` cho R1). Đo được
   khi **chưa** có cache: thường 48.6 s, `--strict` **104.9 s** (chậm hơn cả trước bản vá); sau
   khi thêm cache: 36.7 s / 51.4 s.

Lớp dep-info chậm hơn (17.6 → 23.5 s) vì R2/R3 nay dùng `_test_char_ranges` (token-tree) thay
`test_line_ranges` (regex theo dòng) — đây là **giá của tính đúng**, không phải hồi quy.

---

## 1. P0-1 — `write_state` phải quy cho **hàm trong cùng** (N1/N2)

### Chọn hướng (a): token-tree, không phải (b) cấm hàm lồng

Đề xuất trong review có hai hướng:

- **(a)** quy `write_state` cho **hàm trong cùng theo token-tree**;
- **(b)** **cấm** `fn`/`impl`/`trait`/`mod` lồng trong `state_write.rs`.

**Chọn (a)**, vì:

1. **(a) sửa gốc, (b) sửa triệu chứng.** Gốc của N1/N2 là **quy sai hàm**:
   `_check_state_write` dùng `enclosing(fns, n.line)`, mà `find_functions` **nhảy qua** hàm lồng
   (`i = end + 1`) ⇒ `write_state` trong hàm lồng bị quy cho **cửa ngoài** ⇒ thuộc
   `STATE_WRITE_CALLERS` ⇒ XANH. (b) chỉ chặn **một** hình dạng của lỗi quy sai; một hình dạng
   khác (ví dụ `mod` lồng chứa `fn` trùng tên cửa) vẫn lọt.
2. **(b) tạo báo oan.** `impl` lồng trong thân hàm là **hợp lệ** trong Rust và **có** dùng thật
   (định nghĩa helper cục bộ, `impl` cho kiểu cục bộ). Cấm nó là **thêm** một luật mới mà hợp
   đồng chưa có, và làm đỏ mã sản phẩm **không** liên quan tới `wake`.
3. **(a) không nới gì.** Sáu cửa thật vẫn XANH — có **đối chứng dương** trong selftest
   (`KT-242 N1 đối chứng dương: 6 cửa thật vẫn được nhận đúng cửa`), chứng minh bản vá **siết**
   chứ không phải "luôn đỏ".

### Thay đổi

- Thêm `_fn_body_groups(nodes, out)` — thu `(tên, nhóm thân)` của **mọi** `fn` có thân, **mọi**
  tầng token-tree (đệ quy **mọi** nhóm, kể cả `( … )`/`[…]`).
- Thêm `_innermost_fn_tree(tree, tok)` — hàm **trong cùng** chứa `tok`; **khoảng nhỏ nhất
  thắng** (`body.close.end - body.open.start`), nên hàm lồng luôn thắng cửa ngoài.
- `_check_state_write`: **bỏ** tham số `fns` (nguồn của lỗi quy sai), thay `enclosing(fns, n.line)`
  bằng `_innermost_fn_tree(tree, n)`. Thông điệp lỗi nêu rõ "hàm **lồng trong thân** một cửa
  **không** phải cửa".
- Cập nhật **hai** call site: `scan_v2` (lớp tĩnh) và `scan()` (lớp dep-info, R3).

### Đo

| Ca | Trước | Sau |
|---|---|---|
| N1 — `impl Machine { pub fn kt242_door }` lồng trong `wake_step`, sản phẩm gọi `m.kt242_door(t)` | lớp tĩnh thường **XANH**, `--strict` **XANH**, dep-info **XANH** | **ĐỎ** `E_STRICT_STATE_WRITE` / `E_DEPINFO_STATE_WRITE`, quy đúng `kt242_door` |
| N2 — `fn` lồng trong `wake_step` gọi `write_state` | **XANH** cả ba | **ĐỎ** cả ba |
| Sáu cửa thật | XANH | **XANH** (đối chứng dương) |

Log: `JOURNAL/kt242-wake-repro/logs/r3a/` (`cargo RC=0` — mã **biên dịch được**, nên đây là
đường lọt thật, không phải ca giả).

---

## 2. P0-2 — vùng test theo **ký tự** cho **mọi** luật (N5)

### Thay đổi

- `_transition_calls` trả **bốn** phần tử `(dòng, dạng, mô_tả, offset_ký_tự)` — thêm offset để
  caller lọc vùng test theo **ký tự**.
- `scan_v2`: `test_ranges[str(p)] = _test_char_ranges(strip_rust_noise(text), rel(root, p))`
  (thay `test_line_ranges(text)`), thêm `line_starts` để đổi `(dòng, cột)` → offset.
- **Luật kênh** (producer `MailboxMsg`), **luật consumer**, **luật arch**: miễn theo **vị trí của
  match** (`ls[lineno-1] + m.start()`), không theo cả dòng.
- **R2** (`transition_tcb`): miễn theo `_start` của token.
- **R3** (`write_state`): miễn theo `n.start` của token.
- Lớp dep-info: R2 và R3 dùng **cùng** `_test_char_ranges` (import từ lớp tĩnh, không chép luật).
- **Bỏ** `tr = test_line_ranges(text)` **chết** trong `_mods_of` (nợ KT-194: "còn chỗ trông như
  `test_line_ranges` còn quyết định luật").

### Đo

| Ca | Trước | Sau |
|---|---|---|
| `#[cfg(test)]\nfn helper() {} fn evil(…) { post_mail(…, MailboxMsg::Wake) }` | **XANH** (vùng `(1,2)` phủ cả dòng 2) | **ĐỎ** `E_CHANNEL_PRODUCER` |
| … consumer `MailboxMsg::Wake` cùng dòng | **XANH** | **ĐỎ** `E_CONSUMER` |
| … arch `IpiKind::Wake` cùng dòng | **XANH** | **ĐỎ** `E_ARCH_DOORBELL` |
| Đối chứng âm (bỏ `#[cfg(test)]`) | ĐỎ | **ĐỎ** (không đổi) |
| Đối chứng dương `#[cfg(test)] mod t { … }` | XANH | **XANH** |
| Đối chứng dương R1 cùng dòng `#[cfg(test)]` | ĐỎ | **ĐỎ** (không hồi quy) |

**Chốt chống tái diễn**: ca selftest quét **mã nguồn** hai checker, khẳng định
`test_line_ranges` **không** còn call site sản xuất (chỉ còn trong chính `selftest`, nơi chứng
minh hai cách phân vùng khác nhau).

---

## 3. P0-3 — phạm vi = **mẫu ∪ crate root thật**, đuôi `.rs` không phân biệt hoa/thường (N3/N4)

### N3 — `[[bin]] path` / `[lib] path` ngoài `src/`

`RE_TEST_ONLY_WAKE_SCOPE` chỉ khớp `src/**` + `build.rs`. Bin/lib khai báo **ngoài `src/`** là bố
cục Rust hợp lệ, `cargo` biên dịch thật, nhưng **không** khớp mẫu ⇒ luật token `wake` **không**
chạy.

- `in_test_only_wake_scope(relp, root=None)` — nhận thêm `root`; phạm vi = **mẫu** ∪
  `_crate_root_relps(root)` (đọc `[lib] path`/`[[bin]] path` từ `Cargo.toml` qua `crate_roots`).
  Cache theo gốc cây (selftest dựng nhiều cây tạm).
- Cập nhật **hai** call site trong `scan_v2` và **một** trong lớp dep-info
  (`outside_static_scope`) để truyền `root`.

### N4 — tệp `.RS` (hoa)

`rglob("*.rs")` so khớp **phân biệt** hoa/thường, nhưng rustc trên macOS/Windows nạp `src/x.RS`
cho `mod x;` (đo: `cargo check` biên dịch được) ⇒ tệp **sản phẩm** không được quét.

- Thêm `_iter_rs_ci(base, root)` — duyệt `os.scandir`, **cắt sớm** thư mục build, so
  `suffix.lower() == ".rs"`, có `seen` theo `(st_dev, st_ino)` chống symlink vòng, trả danh sách
  **đã sắp xếp** (thứ tự ổn định như `sorted(rglob(...))` trước đây).
- Thay **năm** chỗ `sorted(base.rglob("*.rs"))`: `_protected_files`, `scan_structure`, `iter_rs`,
  `scan_macro_bodies`, `_shadowed_safe_macros`.
- `RE_TEST_ONLY_WAKE_SCOPE` thêm `re.IGNORECASE` (chỉ **mở rộng** phạm vi — fail-closed).

### Đo

| Ca | Trước | Sau |
|---|---|---|
| `[[bin]] path = "bin/kt242_bin.rs"` chứa `m.wake(0)` | lớp tĩnh thường **XANH**, `--strict` **XANH**, dep-info ĐỎ | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` cả hai chế độ |
| `src/kt242_rs.RS` + `mod kt242_rs;` chứa `m.wake(t)` | lớp tĩnh **XANH** cả hai, dep-info ĐỎ | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` cả hai |
| Đối chứng dương `tests/*.rs` cấp crate | XANH | **XANH** |

Log: `JOURNAL/kt242-wake-repro/logs/binpath/`, `.../rs-case/` (`cargo RC=0` cả hai).

---

## 4. P1-1 — đường dẫn không phân giải được ⇒ **lỗi cổng rõ** (N6)

`Path.resolve()` ném `ValueError` khi đường dẫn chứa NUL (`include!("\0")`) và `OSError` khi quá
dài / vòng symlink. Trước bản này lỗi đó **thoát ra ngoài** thành traceback ⇒ vi phạm D2-166 §3C
("**không âm thầm bỏ qua** … ⇒ lỗi **rõ** + exit khác 0"): exit khác 0 nhưng **không rõ** và
không có mã lỗi.

- Thêm `_safe_resolve(base, raw)` — trả `None` thay vì ném.
- Dùng ở **bốn** chỗ: `resolve_include_targets` (⇒ `E_STRICT_WAKE_INCLUDE` có vị trí nguồn),
  `_mods_of`, `_walk_items`, `_walk_all` (ba chỗ sau giữ đường dẫn dự phòng, lỗi cổng do
  `resolve_include_targets` phát).

### Đo

| Ca | Trước | Sau |
|---|---|---|
| `include!("\0")` | `ValueError: embedded null byte` + traceback, exit 1 **không mã lỗi** | **ĐỎ** `E_STRICT_WAKE_INCLUDE` có `relp:dòng`, exit 1 |

---

## 5. Hợp đồng — sửa bốn chỗ lệch

`v2/agent/reports/CHECKER-WAKE-CONTRACT.md`:

1. **§1 bảng phạm vi** — thêm hai hàng: **crate root thật** (bin/lib ngoài `src/`) và **đuôi `.rs`
   không phân biệt hoa/thường**.
2. **§1 `cfg_attr` lồng trong `v2/lib`** — sửa câu chữ cho khớp **hành vi đo được**: đích **trong**
   `v2/lib` ⇒ XANH; đích **ra ngoài** ⇒ **ĐỎ** `E_STRICT_WAKE_INCLUDE`. Câu cũ ("⇒ XANH") chỉ
   đúng khi đích ở trong. Phần thật sự còn hở là **DEBT-KT227-1**, **không** phải `cfg_attr`.
3. **§2 ngữ cảnh test** — nói rõ **mọi** luật (kênh/consumer/arch/R1/R2/R3) dùng **khoảng ký tự**;
   `test_line_ranges` **không** còn call site sản xuất.
4. **§2b.2** — thêm hàng **hàm lồng**: "hàm gọi" = **hàm trong cùng theo token-tree**, không theo
   dòng/thụt lề; hàm lồng **không** phải cửa ⇒ ĐỎ.
5. **§4 F3** — đổi "CHƯA ĐƯỢC CHỨNG MINH" thành **ĐÃ ĐO** (hai ca đối chứng).

**Luật I (số phải có nguồn)**: `CHECKER-WAKE-CONTRACT.md` nằm trong `[targets]` của
`numbers-policy.toml` ⇒ **không** đưa số đo viết tay vào tệp đó. Số đo nằm ở **journal này**;
hợp đồng chỉ nêu **định tính** ("số ca selftest tăng — số đo đầy đủ ở journal"). Đã chạy
`verify-numbers.py`: `ok=5 fail=0`.

---

## 6. Ca selftest mới

### Lớp tĩnh: 384 → **406** (+22)

| Nhóm | Số ca | Nội dung |
|---|---|---|
| P0-1 | 5 | `impl` lồng ⇒ ĐỎ (thường + `--strict`); `fn` lồng ⇒ ĐỎ (thường + `--strict`); **đối chứng dương** sáu cửa thật vẫn đúng cửa |
| P0-2 | 8 | luật kênh/consumer/arch cùng dòng `#[cfg(test)]` ⇒ ĐỎ (mỗi luật × hai chế độ); đối chứng dương `#[cfg(test)] mod t` ⇒ XANH; đối chứng dương R1 cùng dòng ⇒ vẫn ĐỎ |
| P0-3 | 5 | `[[bin]] path` ngoài `src/` ⇒ ĐỎ (hai chế độ); đối chứng dương `tests/*.rs` ⇒ XANH; `.RS` hoa ⇒ ĐỎ (hai chế độ) |
| P1-1 | 1 | `include!("\0")` ⇒ ĐỎ `E_STRICT_WAKE_INCLUDE`, **không** crash |
| Chốt | 3 | `test_line_ranges` không còn call site sản xuất; `_check_state_write` không còn tham số `fns`; `in_test_only_wake_scope` có tham số `root` |

### Lớp dep-info: 62 → **65** (+3)

| Mã | Nội dung |
|---|---|
| (16k) | `impl` lồng trong cửa gọi `write_state` ⇒ ĐỎ `E_DEPINFO_STATE_WRITE`, quy đúng `kt242_door` |
| (16l) | R2 cùng dòng `#[cfg(test)]` ⇒ ĐỎ `E_DEPINFO_TRANSITION` |
| (16m) | đối chứng dương `#[cfg(test)] mod t { … }` ⇒ XANH |

---

## 7. Hồi quy — chạy lại **toàn bộ** repro cũ

| Repro | Trước (review) | Sau (bản vá) |
|---|---|---|
| N1 `impl` lồng (`logs/r3a/`) | XANH cả ba | **ĐỎ** cả ba |
| N2 `fn` lồng (fixture) | XANH | **ĐỎ** |
| N3 `[[bin]] path` (`logs/binpath/`) | lớp tĩnh XANH, dep-info ĐỎ | **ĐỎ** cả ba |
| N4 `.RS` hoa (`logs/rs-case/`) | lớp tĩnh XANH, dep-info ĐỎ | **ĐỎ** cả ba |
| N5 cùng dòng `#[cfg(test)]` | XANH (luật kênh/consumer/arch) | **ĐỎ** cả ba |
| N6 `include!("\0")` | traceback | **ĐỎ** có mã lỗi |
| R1 cùng dòng `#[cfg(test)]` | ĐỎ | **ĐỎ** (không hồi quy) |
| R1 trong `#[cfg(test)] mod t` | XANH | **XANH** |
| `cfg_attr` lồng trong `v2/lib` trỏ ra ngoài | ĐỎ | **ĐỎ** |
| `cfg_attr` lồng trong `v2/lib` trỏ trong | XANH | **XANH** |
| DEBT-KT227-1 (token `wake` trong `v2/lib`) | XANH | **XANH** (ngoài phạm vi bản vá — xem §8) |
| Crate mới dưới `v2/kernel` | ĐỎ | **ĐỎ** |
| `include!`/`#[path]` không literal / ra ngoài cây | ĐỎ | **ĐỎ** |
| H4 fixpoint | giữ | **giữ** |
| `macro_rules!` lồng / `:tt` / `:item` | ĐỎ | **ĐỎ** |
| `From`/`TryFrom` chuyển `TcbState` | ĐỎ | **ĐỎ** |
| Mỏ neo kiểu | giữ | **giữ** |
| `init/*.rs` không gọi được `wake` | giữ | **giữ** |

---

## 8. Việc **chưa** làm (và vì sao)

1. **DEBT-KT227-1** — `v2/lib` ngoài luật token `wake`. **Không** thuộc bốn mục P0/P1; sửa nó là
   **mở rộng phạm vi** (thêm `v2/lib` vào luật token `wake`), cần lead quyết vì có thể làm đỏ mã
   `v2/lib` hiện có. Hiện **latent**: không crate `v2/lib` nào khai `vifast-core`.
2. **L2** (ba mẫu phạm vi lệch nhau: `in_test_only_wake_scope` / `RE_CRATE_SRC` /
   `MACRO_SCAN_ROOTS`) — **không** có đường lọt sống; hợp nhất ba mẫu là việc tái cấu trúc, không
   phải vá lỗ. Bản vá này **giảm** lệch (thêm crate root vào mẫu thứ nhất) nhưng **không** gộp.
3. **`find_functions`/`enclosing` vẫn còn** — dùng cho luật kênh/consumer/arch (quy hàm theo
   dòng). Ở đó hàm lồng **không** tạo đường lọt (luật chỉ cần **tên** hàm phát/phát hiện, không
   cần "cửa"), nên **không** đổi. R3 đã bỏ hẳn.
4. **`v2 check` clippy/fmt chỉ phủ workspace host** (`v2/bin/v2` dùng `MANIFEST="$V2_ROOT/Cargo.toml"`)
   ⇒ thay đổi chỉ trong `v2/kernel` **không** được lint. Không phải việc của bản vá này.

---

## 9. Rủi ro còn lại

1. **Closure trong thân một cửa** vẫn bị quy cho **cửa** (closure không phải `fn`, nên
   `_fn_body_groups` không thấy). Không thoát được: cửa trả `Result<(), KError>`, closure bắt
   `&mut self` trong thân cửa vẫn phải gọi `write_state` **trong** cửa — tức vẫn là mã của cửa.
   Nếu mai kia có closure **trả** `Result` và được **truyền ra ngoài**, cần siết tiếp.
2. **`_iter_rs_ci` theo `suffix.lower()`** — trên Linux (phân biệt hoa/thường) một tệp `x.RS`
   **không** được rustc nạp, nhưng checker vẫn quét ⇒ **báo oan** (fail-closed, chấp nhận được).
3. **`re.IGNORECASE` trên cả mẫu phạm vi** — `v2/KERNEL/...` cũng khớp. Chỉ **mở rộng**, không
   thu hẹp.
4. **`_crate_root_relps` cache theo `str(root)`** — nếu một tiến trình dựng **hai** cây khác nhau
   ở **cùng** đường dẫn (ghi đè tại chỗ), cache có thể cũ. Selftest dựng cây **mới** mỗi ca nên
   không lộ; `main` chỉ chạy một cây.
5. **`--cfg test` trong build sản phẩm** vẫn là giả định chưa kiểm (như review đã ghi): nếu ai
   build sản phẩm với `--cfg test`, mã `#[cfg(test)]` vào ảnh và **mọi** luật miễn nó.

---

## 10. Cần lead review tay

1. **Hướng P0-1 (a)** — bản vá **thêm** khả năng đọc token-tree vào `_check_state_write`. Đúng
   tinh thần "sửa gốc", nhưng là thay đổi **cơ chế** (không chỉ thêm ca). Đề nghị lead đọc
   `_fn_body_groups`/`_innermost_fn_tree` và **đối chứng dương** sáu cửa.
2. **`in_test_only_wake_scope` đổi chữ ký** (thêm `root`). Caller cũ **không** truyền `root` vẫn
   chạy (chỉ mẫu) — cố ý giữ tương thích, nhưng nghĩa là **quên** truyền `root` sẽ **âm thầm**
   quay lại phạm vi hẹp. Đã có ca selftest chốt chữ ký; **không** có cách chặn cứng ở runtime.
3. **`test_line_ranges` vẫn còn trong tệp** (chỉ selftest dùng). Nếu lead muốn **xoá hẳn**, cần
   bỏ ca selftest chứng minh hai cách phân vùng khác nhau — đánh đổi: mất bằng chứng "vì sao đổi".
4. **Số ca selftest tăng 22 + 3** — cần lead xác nhận ngân sách thời gian cổng (lớp tĩnh 38.3 →
   41 s; `--strict` cây thật 67 → 105 s, do `_test_char_ranges` chạy thêm một lượt token hoá cho
   **mọi** tệp trong phạm vi).

---

## 11. Cam kết phạm vi

- **Không** sửa mã sản phẩm (`v2/kernel/**`, `v2/sim/**`, `v2/lib/**`).
- **Không** sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
  `docs/v1/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`.
- **Không** nới ngưỡng, **không** tắt ca, **không** sửa checker để repro xanh.
- Repro cây thật dùng `trap` khôi phục **ngay**; `run-repro.sh` chỉ khôi phục
  `v2/kernel`/`v2/sim`/`v2/lib` (bản cũ `git checkout -- .` **xoá luôn** bản vá checker — đã sửa
  và ghi lại ở đây để người sau không lặp).
