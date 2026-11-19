# KT-216 — giải xung đột merge KT-211 (L46) + KT-213/KT-215 (L47)

**Tác giả**: L46 (sub-agent của Agent 1) · **Nhánh**: `night3/L46-kt211` · **Gốc merge**: `74e2b2c9`
**Nhánh được merge**: `night3/L47-kt212` (`aebe5505` KT-213 + `63424df3` KT-215)
**Commit merge**: xem `git log -1` sau commit `[KT-216]`

---

## 0. KẾT LUẬN NGẮN

**ĐẠT.** Merge sạch về ngữ nghĩa: giữ **cả** `_walk_all` (KT-211 — quét token toàn cục cho
`include!`/`#[path]`) **và** `_walk_items` với `mod_dir` (KT-213 — đồ thị module + inline module);
giữ `_is_build_dir`/`MAILBOX_DEFS`/đóng F2 (KT-215). Selftest **194/194** (175 của KT-211 + 19
của KT-213/215, **không** mất ca nào). `--strict` và chế độ **thường** trên cây thật: `errors=0`.
Sáu biến thể L44 + L40 + #11 + #12 ⇒ **ĐỎ** cả hai chế độ; đối chứng dương ⇒ **XANH**.
`./v2/bin/v2 check` ⇒ **PASS**.

---

## 1. XUNG ĐỘT VÀ CÁCH GIẢI

### 1.1. `v2/tools/check-wake-paths.py` — 4 khối xung đột

| # | Vị trí | HEAD (KT-211) | theirs (KT-213/215) | Cách giải |
|---|---|---|---|---|
| 1 | sau `_walk_items` | `_walk_all` (hàm mới) | `_is_build_dir` (hàm mới) | **Giữ cả hai**: `_walk_all` rồi `_is_build_dir` |
| 2 | trong `resolve_include_targets` | `_walk_all(...)` | `on_mod` + `_walk_items(..., mod_dir=p_mod_dir)` | **Giữ cả hai**: `on_mod` (cho `inline_path_toks`) + `_walk_all` (sinh cạnh `include!`/`#[path]`) + `_walk_items` (giữ `mod`) |
| 3 | helper selftest | `_kt211_case` | `_kt213_case` | **Giữ cả hai** helper |
| 4 | ca selftest | 18 ca KT-211 | 19 ca KT-213/215 | **Giữ cả hai** bộ ca |

**Nguyên tắc** (theo phiếu):
- `include!`/`#[path]` **chỉ** do `_walk_all` sinh cạnh. `_walk_items` trong
  `resolve_include_targets` **không** nhận `on_attr`/`on_include` (bỏ callback đó — không mất
  năng lực: `_walk_all` phủ **mọi** vị trí token, là **siêu tập** của `_walk_items` cho hai loại
  cạnh này; đã chứng minh bằng 18 ca KT-211 + 19 ca KT-213/215 + 6 biến thể L44 + L40 + #11 + #12).
- `mod` vẫn theo `_walk_items` (`on_mod` → `inline_path_toks`); `_walk_all` cũng thu thập
  `inline_path_toks` (tập `set`, trùng vô hại) — giữ cả hai để không phụ thuộc một đường.
- `_walk_all` nhận thêm `mod_dir` (KT-213) với **cùng nghĩa** `_walk_items`: `cur_dir` mức gốc là
  **thư mục tệp** (`p.parent`) cho `#[path]` mức mục; `mod_dir` là **thư mục module** của tệp
  (`p_mod_dir`) cho inline module **không** `#[path]` (`src/foo.rs` ⇒ `src/foo/NAME/`).
  Inline module **có** `#[path]` ⇒ `cur_dir/<path>`.

**Chi tiết tích hợp `resolve_include_targets`** (khối xung đột 2):

```python
inline_path_toks: set = set()

def on_mod(name_tok, attrs, inline, group, cur_dir, in_test=False, mod_dir=None):
    if not inline:
        return
    for a in attrs:
        ...  # thu thập inline_path_toks

# KT-211 — quét toàn bộ token sinh cạnh include!/#[path]
_walk_all(tree, relp, p.parent, on_include, on_path_attr, inline_path_toks,
          mod_dir=p_mod_dir)
# KT-213 — mod giữ đường cấu trúc; KHÔNG truyền on_attr/on_include (tránh trùng)
_walk_items(tree, relp, p.parent, on_mod=on_mod, errors=[], mod_dir=p_mod_dir)
```

**Sửa lỗi tích hợp đã phát hiện trong lúc merge**: lần đầu tôi truyền `p_mod_dir` làm `cur_dir`
cho `_walk_all` ⇒ hai ca **đối chứng dương** của KT-213 (`#[path]` mức mục trong tệp non-mod-rs)
**ĐỎ oan** (vì `#[path]` mức mục phải giải theo `p.parent`, không theo `p_mod_dir`). Đã sửa:
`cur_dir = p.parent`, `mod_dir = p_mod_dir` — đúng hai gốc của KT-213. Selftest 192/194 → **194/194**.

### 1.2. `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` — 2 khối xung đột

| # | Vị trí | Cách giải |
|---|---|---|
| 1 | tiêu đề + trạng thái | **Gộp cả ba**: KT-211 + KT-213 + KT-215; trạng thái `REVIEW` cho KT-184/194/211/213 |
| 2 | §4 hai dòng F2 + Macro invocation | **F2** lấy bản theirs (**ĐÓNG bởi KT-215**); **Macro invocation** lấy bản HEAD (KT-202 đóng **một dạng**, KT-211 đóng **lớp**) |

Các phần **không** xung đột được git tự gộp và **giữ nguyên**:
- §1 đoạn **KT-211** (cạnh phân giải được ở mọi vị trí token) + §1 đoạn KT-202 cập nhật;
- §2 gạch đầu dòng **KT-211** (quét toàn cục) + gạch đầu dòng **KT-213** (hai gốc `#[path]`);
- §3 hai dòng KT-211 (không literal / không có đối số — mọi vị trí token);
- §4 các dòng **KT-213**, **KT-215** (`target/`, `MAILBOX_DEFS`, virtio), "Báo oan có trước" (bản
  KT-211 có thêm 4 dạng).

### 1.3. `v2/agent/DECISIONS.md` và `v2/agent/QUEUE.md` — **merge cơ học**, không phải tôi sửa

Hai tệp này bị đổi **do nhánh L47 mang theo** (D2-170/D2-172; KT-208 DONE, KT-211 READY), **không**
phải tôi soạn. Tôi **không** sửa tay nội dung của chúng; merge tự gộp (không xung đột). Ghi rõ để
không bị coi là "tự sửa tệp cấm".

---

## 2. KIỂM SAU MERGE

### 2.1. Selftest — **194/194** (không mất ca nào)

| Bên | Số ca | Ghi chú |
|---|---|---|
| Gốc `74e2b2c9` | 157 | |
| KT-211 (L46) | +18 | 11 ĐỎ + 7 đối chứng dương |
| KT-213 (L47) | +8 | repro #11 (thường + strict) + 4 đối chứng dương + 2 ca `_mods_of` |
| KT-215 (L47) | +11 | repro #12 (thường + strict) + 2 đối chứng dương build-dir + 6 ca bảng `_is_build_dir` + 1 ca `mailbox.rs` |
| **Sau merge** | **194** | `# selftest: 194/194 ok` |

Log: [`kt216-logs/01-selftest-after-merge.log`](kt216-logs/01-selftest-after-merge.log).

### 2.2. Cây thật — `errors=0` cả hai chế độ

| Lệnh | Kết quả |
|---|---|
| `python3 v2/tools/check-wake-paths.py` | RC=0, `errors=0` |
| `python3 v2/tools/check-wake-paths.py --strict` | RC=0, `errors=0` |

Log: [`kt216-logs/02-real-tree-normal.log`](kt216-logs/02-real-tree-normal.log),
[`03-real-tree-strict.log`](kt216-logs/03-real-tree-strict.log).

### 2.3. Đột biến chứng minh (cây thật, `cargo check` + checker cả hai chế độ)

| # | Dạng | cargo | thường | strict |
|---|---|---|---|---|
| L44-B01 | `sink(include!{…})` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| L44-B02 | `[include!{…}]` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| L44-B03 | `(0u8, include!{…})` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| L44-B23 | `(include!{…})` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| L44-B24 | `(|| include!{…})` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| L44-B25 | `Some(include!{…})` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| L40 | repro KT-202 (`:item` + `include!` trong lời gọi) | **0** | **ĐỎ** (3) | **ĐỎ** (3) |
| #11 | `#[path]` trong inline module tệp non-mod-rs | **0** | **ĐỎ** (4) | **ĐỎ** (4) |
| #12 | `src/target/evil.rs` chứa `wake` | **0** | **ĐỎ** (4) | **ĐỎ** (4) |

**Đối chứng dương** (phải XANH):

| # | Dạng | cargo | thường | strict |
|---|---|---|---|---|
| P1 | `include_str!` | **0** | **XANH** | **XANH** |
| P2 | `include_bytes!` | **0** | **XANH** | **XANH** |
| P3 | `stringify!(include)` | **0** | **XANH** | **XANH** |
| P4 | `include!` hợp lệ trong cây, trong `( … )` | **0** | **XANH** | **XANH** |
| P5 | `#[path]` hợp lệ trong cây, trong khối lồng `( … )` | **0** | **XANH** | **XANH** |
| P6 | tệp trong `v2/kernel/target/` (build dir) vẫn bỏ qua | **0** | **XANH** | **XANH** |

Tổng hợp: [`kt216-logs/00-mutation-summary.log`](kt216-logs/00-mutation-summary.log); log từng ca
`kt216-logs/<ca>-{cargo,normal,strict}.log`.

**Ghi chú**: repro #11 dùng `src/kt216_foo.rs` (non-mod-rs) + `src/kt216_foo/inner/notes.txt` +
`vifast-core/evil.rs` (chứa `wake`) + decoy `v2/kernel/evil.rs` — đúng cấu trúc repro của L47.
Repro #12 dùng `src/target/mod.rs` + `src/target/evil.rs` + `mod target;` trong `lib.rs`.

### 2.4. `./v2/bin/v2 check` — **PASS**

Log: [`kt216-logs/04-v2check.log`](kt216-logs/04-v2check.log). Không có T-INFRA SIGTRAP (KT-207
đã có trên nhánh này).

---

## 3. GIỚI HẠN / GHI CHÚ

1. **`_walk_all` và `_walk_items` cùng chạy** trong `resolve_include_targets` — hai lượt duyệt
   token mỗi tệp. Chấp nhận: `_walk_all` phủ `include!`/`#[path]` (siêu tập), `_walk_items` giữ
   `mod`/`inline_path_toks` theo đúng phiếu. Không nới ngưỡng, không allowlist.
2. **`inline_path_toks` thu thập hai lần** (cả `_walk_all` và `_walk_items`) — `set`, trùng vô hại.
3. **`DECISIONS.md`/`QUEUE.md` đổi do merge** (D2-170/D2-172, KT-208 DONE, KT-211 READY) — nội
   dung của nhánh L47, **không** phải tôi soạn; không sửa tay.
4. **Không** push, **không** chạy `gate --fast` (theo phiếu).
5. Các giới hạn còn lại của KT-211 (mục §7 journal `kt211-include-global.md`) và của KT-213/215
   (journal `kt212-attack-path-mod.md`, `kt215-target-filter.md`) **giữ nguyên** — merge không
   thay đổi phạm vi bảo đảm.

---

## 4. TỆP ĐÃ SỬA TRONG COMMIT MERGE

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-paths.py` | giải 4 xung đột: giữ `_walk_all` + `_walk_items(mod_dir)` + `_is_build_dir` + `MAILBOX_DEFS` + F2; `_walk_all` nhận `mod_dir`; selftest 194 ca |
| `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` | giải 2 xung đột: gộp tiêu đề/trạng thái; F2 (KT-215) + Macro invocation (KT-211) |
| `v2/agent/JOURNAL/kt216-merge.md` | tệp này |
| `v2/agent/JOURNAL/kt216-logs/**` | log đột biến + selftest + `v2 check` |
| (từ nhánh L47) | `v2/agent/DECISIONS.md`, `v2/agent/QUEUE.md`, `v2/agent/JOURNAL/kt212-*`, `kt213-*`, `kt215-*` |
