# KT-202 — vá đường lọt SỐNG: `include!` trong đối số lời gọi macro + `$…:<frag>` passthrough

**Ngày**: 2026-11-20 · **Agent 1 (lead) → sub-agent L37** · nhánh `night3/L37-kt181` ·
worktree `/private/tmp/night3-L37-kt181` · HEAD `6793cc20` (KT-181, giữ nguyên).
**Nguồn**: reviewer độc lập **L40** (vòng 4), lead đã tự tái lập trên `8f5a1189`.
**Ngân sách**: 2 h. Không merge, không push, không `gate --fast`, không QEMU.

## 0. Kết luận trước

| Câu hỏi | Trả lời | Căn cứ |
|---|---|---|
| Đường lọt L40 có thật? | **CÓ** — tái lập được: `cargo check` biên dịch được, checker **xanh** cả hai chế độ | §1 |
| Còn biến thể lọt nào khác? | **CÓ** — lời gọi macro lồng trong `( … )`; chỉ sửa `_walk_items` **không** bắt được | §1.2 |
| Đã vá? | **CÓ** — hai vá: (1) `_scan_macro_calls` quét **toàn cây token**; (2) `$…:<frag>` passthrough | §2 |
| Repro sau vá | **ĐỎ** ở **cả** chế độ thường **và** `--strict` (cả dạng item-level lẫn dạng lồng) | §3 |
| Đối chứng dương | `assert_eq!`/`vec!`/`format!`/`matches!`/`stringify!(mod)` ⇒ **XANH** | §3 |
| Selftest | **128 → 144** ca (16 ca mới; 128 ca cũ **không mất ca nào**) | §4 |
| `./v2/bin/v2 check` | **PASS** trên cây thật sau vá | `kt181-logs/kt202-after-v2-check.log` |
| Sai lệch so với đề | **`:expr` KHÔNG bị từ chối** — lý do + bằng chứng ở §5 | `kt181-logs/kt202-expr-conflict.log` |

## 1. Đường lọt (tái lập, nguyên văn)

### 1.1 Dạng item-level (repro của L40)

```rust
// v2/lib/evil_kt201.rs
#[allow(dead_code)]
pub fn evil_kt201(m: &mut crate::Machine, t: crate::ObjId) { let _ = m.wake(t); }

// v2/kernel/vifast-core/src/kt201_evil.rs   (+ `mod kt201_evil;` trong lib.rs)
macro_rules! kt201_pass { ($i:item) => { $i }; }
kt201_pass!(include! {"../../../lib/evil_kt201.rs"});
```

Đo **trước** vá (HEAD `6793cc20`):

| Lệnh | Kết quả | Log |
|---|---|---|
| `cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-core` | **RC=0** (biên dịch được) | `kt181-logs/kt202-repro-cargo.log` |
| `python3 v2/tools/check-wake-paths.py` | **XANH** (`errors=0`) | `kt181-logs/kt202-repro-normal-before.log` |
| `python3 v2/tools/check-wake-paths.py --strict` | **XANH** (`errors=0`) | `kt181-logs/kt202-repro-strict-before.log` |

### 1.2 Dạng lồng trong `( … )` — **phát hiện thêm khi vá**

```rust
// v2/lib/evil_expr.rs
m.wake(t)

// v2/kernel/vifast-core/src/kt201_nested.rs
macro_rules! pass_expr { ($e:expr) => { $e }; }
fn kt201_nested(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = (pass_expr!(include! {"../../../lib/evil_expr.rs"}));
}
```

Đo **trước** vá: `cargo check` **RC=0**, checker `--strict` **XANH**
(`kt181-logs/kt202-nested-cargo.log`, `kt181-logs/kt202-nested-checker.log`). Lý do: `_walk_items`
chỉ đệ quy vào nhóm `{ … }`; lời gọi macro trong `( … )` **không** bao giờ được thấy. Vì vậy
**chỉ sửa `_walk_items` như đề là chưa đủ** — xem §2.1.

**Nguyên nhân gốc — hai chỗ cộng lại**:
1. `_walk_items` gặp nhóm sau `!` (lời gọi macro) thì bỏ **mù** toàn bộ nhóm, nên
   `include!`/`#[path]`/`mod` trong **đối số lời gọi** không bao giờ được
   `resolve_include_targets` thấy.
2. `_check_macro_body` chỉ từ chối `$…:tt`; `:item` (và `:stmt`/`:block`/`:meta`) chở được y hệt
   nhưng **không** bị từ chối.

## 2. Đã sửa gì

Chỉ **siết**: không nới ngưỡng, không thêm allowlist, không `#[ignore]`/`#[should_panic]`,
không đổi kỳ vọng của ca cũ. Toàn bộ thay đổi logic nằm trong `v2/tools/check-wake-paths.py`.

### 2.1 Vá (1) — quét **đối số lời gọi macro** ở **mọi** tầng nhóm

- Tách logic của `_check_macro_body` thành hàm dùng chung `_check_macro_passthrough(relp, where,
  nodes, r)` — nhận diện `include!`, `#[path]`/`#[r#path]`/`cfg_attr(…, path = …)`, `mod NAME;`/
  `mod NAME {…}`/`mod r#…;`/`mod $…;` ⇒ `E_STRICT_MACRO_INCLUDE`/`E_STRICT_MACRO_MOD`.
- Thêm `_scan_macro_calls(nodes, relp, r)`: đệ quy **mọi** nhóm (kể cả `( … )`/`[…]`), khi gặp
  nhóm sau `!` thì gọi `_check_macro_passthrough` trên **đối số**; **không** đệ quy tiếp vào nhóm
  lời gọi (hàm kia đã quét đệ quy toàn nhóm ⇒ tránh báo trùng).
- `scan_macro_bodies` gọi `_scan_macro_calls(tree, relp, r)` cho **mọi** tệp
  `v2/lib`+`v2/kernel`+`v2/sim` (cùng phạm vi KT-181).
- **Vì sao không chỉ sửa `_walk_items`**: `_walk_items` chỉ đệ quy `{ … }` (để không nhầm `mod`
  trong biểu thức thành khai báo module). Dạng lồng trong `( … )` (§1.2) **lọt** nếu chỉ sửa
  `_walk_items` — đã đo. `_scan_macro_calls` quét **toàn cây token** nên phủ cả hai dạng.

### 2.2 Vá (2) — `$…:<frag>` passthrough

- `MACRO_PASSTHROUGH_FRAGS = {tt, item, stmt, block, meta}`; `_check_macro_body` từ chối
  `$…:<frag>` với `frag` thuộc tập này ⇒ `E_STRICT_MACRO_TT` (giữ mã lỗi, đổi thông điệp).
- **`:expr` KHÔNG nằm trong tập** — lý do + bằng chứng ở §5.

### 2.3 Hợp đồng

`v2/agent/reports/CHECKER-WAKE-CONTRACT.md`: §1 (đích ra ngoài chỉ ĐỎ **khi cạnh phân giải được**),
§2 (mục KT-202), §3 (hai dòng: `$…:<frag>`; `include!`/`#[path]`/`mod` trong đối số lời gọi),
§4 (mục "Macro invocation" → **đã vá**, nêu cả dạng lồng; thêm dòng `:expr` không bị từ chối).

## 3. Đột biến đỏ/xanh (bảng đầy đủ)

Mỗi đột biến: thêm tệp/nối mã, chạy checker, rồi `git checkout --` + xoá tệp mới.

| Tên | Lệnh | Trước vá | Sau vá | Log |
|---|---|---|---|---|
| Repro L40 (`:item` + `include!` trong lời gọi) | checker thường | **XANH** | **ĐỎ** `E_STRICT_MACRO_TT` + `E_STRICT_MACRO_INCLUDE` | `kt202-repro-normal-before.log` / `kt202-repro-normal-after.log` |
| Repro L40 | checker `--strict` | **XANH** | **ĐỎ** (cùng hai mã) | `kt202-repro-strict-before.log` / `kt202-repro-strict-after.log` |
| Biến thể lồng `( … )` | `cargo check` + checker `--strict` | cargo RC=0, checker **XANH** | cargo RC=0, checker **ĐỎ** `E_STRICT_MACRO_INCLUDE` | `kt202-nested-cargo.log` / `kt202-nested-checker.log` / `kt202-nested-after.log` |
| `:item` → `:tt` | checker `--strict` | — | **ĐỎ** `E_STRICT_MACRO_TT` + `E_STRICT_MACRO_INCLUDE` (không hồi quy) | `kt202-mut-tt.log` |
| `include!` trần (không macro) | checker `--strict` | — | **ĐỎ** `E_STRICT_WAKE_INCLUDE` (không hồi quy) | `kt202-mut-bare-include.log` |
| Đối chứng dương: `assert_eq!`/`vec!`/`format!`/`matches!`/`stringify!(mod)` | checker `--strict` | — | **XANH** (không báo oan) | `kt202-mut-positive-control.log` |
| Cây thật (không đột biến) | checker `--strict` | XANH | **XANH** | `kt202-clean-tree.log` |

## 4. Selftest — trước/sau

| | Số ca | Kết quả |
|---|---|---|
| Trước KT-202 (HEAD `6793cc20`) | **128** | 128/128 ok |
| Sau KT-202 | **144** | 144/144 ok (`kt181-logs/kt202-after-selftest.log`) |

**16 ca mới** (đều trong `--selftest`):

| # | Ca | Trước vá | Sau vá |
|---|---|---|---|
| 1 | repro `:item` + `include!` trong lời gọi (chế độ thường) ⇒ đỏ | **FAIL** | PASS |
| 2 | repro `:item` + `include!` trong lời gọi (`--strict`) ⇒ đỏ | **FAIL** | PASS |
| 3 | `:item` → `:tt` vẫn đỏ (không hồi quy) | PASS | PASS |
| 4 | `include!` trần (không macro) vẫn đỏ | PASS | PASS |
| 5 | `:item` passthrough trong thân (không lời gọi) ⇒ đỏ | **FAIL** | PASS |
| 6 | `include!` trong lời gọi macro (thân không passthrough) ⇒ đỏ | **FAIL** | PASS |
| 7 | `#[path]` trong lời gọi macro ⇒ đỏ | **FAIL** | PASS |
| 8 | `mod` trong lời gọi macro ⇒ đỏ | **FAIL** | PASS |
| 9 | `:stmt` passthrough ⇒ đỏ | **FAIL** | PASS |
| 10 | `:block` passthrough ⇒ đỏ | **FAIL** | PASS |
| 11 | `:meta` passthrough ⇒ đỏ | **FAIL** | PASS |
| 12 | `:expr` passthrough ⇒ xanh (lý do §5) | PASS | PASS |
| 13 | đối chứng dương: `assert_eq!`/`vec!`/`format!` ⇒ xanh | PASS | PASS |
| 14 | đối chứng dương: `#[cfg(test)]` trong lời gọi macro ⇒ xanh | PASS | PASS |
| 15 | biến thể lồng trong `( … )` ⇒ đỏ | **FAIL** | PASS |
| 16 | đối chứng dương: lời gọi macro lồng trong `( … )` không include/path/mod ⇒ xanh | PASS | PASS |

**Bằng chứng "có răng"**: chạy **bộ ca mới** trên **checker cũ** `6793cc20` (chèn nguyên khối ca
KT-202) ⇒ `# selftest: 134/144 ok`, đúng **10 ca ĐỎ** fail
(`kt181-logs/kt202-selftest-new-cases-on-old-checker.log`). Trên checker mới ⇒ 144/144.
**128 ca cũ không mất ca nào** (so khớp tên ca: `comm` rỗng).

## 5. Sai lệch so với đề: `:expr` **không** bị từ chối

Đề của lead liệt kê `frag ∈ {tt, item, expr, stmt, block, meta}`. Tôi **không** đưa `:expr` vào
tập bị từ chối, vì:

1. **Cây thật sẽ ĐỎ**: `macro_rules! bail` ở `v2/kernel/vifast-plat-riscv64/src/user.rs:476`
   dùng `$pages:expr, $e:expr` **hợp lệ**. Đo bằng cách mô phỏng đúng danh sách của lead (có
   `expr`) trên cây thật: **ĐỎ**, các lỗi `E_STRICT_MACRO_TT` tại `user.rs:477`
   (`kt181-logs/kt202-expr-conflict.log`). Như vậy `v2 check` sẽ **FAIL** — trái yêu cầu
   "không hồi quy".
2. **`:expr` không chở được `mod`/`#[path]`**: `mod` là item, `#[path]` là thuộc tính — `:expr`
   chỉ nhận biểu thức. Nó chỉ chở được `include!` ở **vị trí biểu thức**.
3. **`include!` trong đối số lời gọi đã bị vá (1) bắt ở tệp gọi** — kể cả khi macro dùng `:expr`.
   Vá (1) quét token trong **mọi** nhóm đối số lời gọi ở cả ba gốc, nên `m!(include!("…"))` là ĐỎ
   bất kể fragment của macro.
4. **Không thêm allowlist** (đúng lệnh lead) — nếu thêm `bail` vào allowlist thì đó là "thêm
   allowlist để lấy xanh", bị cấm.

**Hệ quả còn hở** (ghi rõ, không tuyên bố đã đóng): `:expr` chỉ còn hở khi macro được **gọi từ
tệp ngoài `MACRO_SCAN_ROOTS`** (ví dụ `v2/tools/**`, `v2/test/fixtures/**`) — không phải mã sản
phẩm. Nếu lead muốn đóng cả `:expr`, cần **một trong hai**: (a) viết lại `bail` để không dùng
`:expr` (đổi call site — ngoài phạm vi KT-202), hoặc (b) cho phép allowlist `bail` có lý do.

## 6. Giới hạn còn lại

1. **`:expr`** — xem §5. Không tuyên bố đã đóng.
2. **`v2/tools/**` + `v2/test/fixtures/**`** — ngoài `MACRO_SCAN_ROOTS` (lý do ở hợp đồng §4).
3. **`#[path]`/`include!` ngoài thân macro trong `v2/lib`** — sai lệch **có trước**, ghi ở hợp
   đồng §4; ngoài phạm vi KT-202.
4. **Lời gọi macro lồng** — `_scan_macro_calls` đệ quy **mọi** nhóm, nên
   `m!(n!(include!("…")))` và `let _ = (m!(include!("…")));` đều ĐỎ. Không còn giới hạn ở đây.
5. **Báo trùng**: `_scan_macro_calls` **bỏ qua** thân `macro_rules!` (đã được `_check_macro_body`
   quét đệ quy) nên không báo trùng khi thân chứa lời gọi macro có `include!`/`#[path]`/`mod`.
6. Không chạy `gate --fast` (luật task); chỉ `./v2/bin/v2 check`.

## 7. Luật đã giữ

- Không sửa `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `docs/QUYET-DINH-sau-review.md`,
  `docs/v2/*`, `docs/v1/*`, `AGENTS.md`, `v2/AGENTS.md`.
- Không `#[ignore]`/`#[should_panic]`, không nới ngưỡng, không retry để lấy xanh, không allowlist.
- Không `pkill -f qemu-system`; task không dùng QEMU.
- Không push, không merge, không tạo worktree mới; commit `6793cc20` (KT-181) giữ nguyên.
- Mọi đột biến đã `git checkout --` + xoá tệp mới; cây sạch trước commit.
