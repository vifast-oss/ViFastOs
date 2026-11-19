# KT-234 — đường lọt #16: `TcbState::Ready` truyền qua **tham số liên hàm**

- **Người kiểm**: L57 (task KT-234, nhánh `night3/L57-kt234`, từ `cbc2c2af`).
- **Nợ nguồn**: D2-181 §4; dòng "Truyền `Ready` qua **tham số liên hàm** — **Mở**" trong
  [`kt231-transition-alias.md`](kt231-transition-alias.md) §5.
- **Trạng thái**: **PHÁT HIỆN ĐƯỜNG LỌT SỐNG** — **6/10** dạng XANH **cả bốn** lớp
  (cargo RC=0 + lớp 1 thường + lớp 1 `--strict` + lớp 2). Task này **chỉ kiểm**, **không vá**.
- **Nguồn sự thật**: `v2/tools/check-wake-paths.py` (`_transition_calls`,
  `_var_may_be_ready`) + `v2/tools/check-wake-depinfo.py` (R2 dùng lại đúng hai hàm đó).

---

## 1. Cơ chế

`_transition_calls` chỉ phân tích **đối số thứ hai của lời gọi `transition_tcb` tại chỗ**:

| Dạng đối số thứ hai | Phân loại | Xử lý |
|---|---|---|
| `TcbState::Ready` (literal) | `ready` | đòi cửa wake `WAKE_DOORS` hoặc `NON_WAKE_READY_SITES` |
| `TcbState::<BiếnThểKhác>` | `other` | chứng minh không Ready ⇒ XANH |
| `<X>::Ready` với `X ≠ TcbState` | `alias` | ĐỎ (fail-closed) |
| một định danh trần `v` | `var` | ĐỎ **chỉ khi** `_var_may_be_ready` thấy `v = … TcbState::Ready` **trong thân hàm chứa lời gọi** |
| dạng khác (call, `[…]`, `*…`, …) | `unknown` | ĐỎ (fail-closed) |

`_var_may_be_ready` là dataflow **một hàm**: nó đọc đúng dải dòng `fn.start..fn.end` của hàm
chứa lời gọi và tìm regex gán `var = … TcbState::Ready`. **Không có bước lan truyền liên hàm**,
không có bảng const/static, không có phân tích closure.

Hệ quả: khi `Ready` **không xuất hiện ở đối số thứ hai tại chỗ gọi** — nó ở một call site khác,
trong một hàm khác, trong một `const`/`static`, hoặc trong closure — thì luật **không thấy**.
Đây đúng là nợ D2-181 §4.

## 2. Repro (10 dạng)

Tất cả dưới `v2/kernel/vifast-core/src/`, khai báo `mod` trong `lib.rs` (mã sản phẩm, không
phải test-only). Bản đầy đủ: [`kt234-logs/repro-files.txt`](kt234-logs/repro-files.txt).

```rust
// f1 — helper cùng tệp
pub fn kt234_f1(m: &mut Machine, t: ObjId) { helper(m, t, TcbState::Ready); }
fn helper(m: &mut Machine, t: ObjId, s: TcbState) { let _ = m.transition_tcb(t, s); }

// f2 — helper ở TỆP KHÁC (kt234_f2_helper.rs)
pub fn kt234_f2(m: &mut Machine, t: ObjId) {
    crate::kt234_f2_helper::helper(m, t, TcbState::Ready);
}
// kt234_f2_helper.rs: pub(crate) fn helper(m, t, s: TcbState) { let _ = m.transition_tcb(t, s); }

// f3 — helper TRẢ VỀ Ready
fn ready_state() -> TcbState { TcbState::Ready }
pub fn kt234_f3(m: &mut Machine, t: ObjId) { let _ = m.transition_tcb(t, ready_state()); }

// f4 — const
const R: TcbState = TcbState::Ready;
pub fn kt234_f4(m: &mut Machine, t: ObjId) { let _ = m.transition_tcb(t, R); }

// f5 — static
static R: TcbState = TcbState::Ready;
pub fn kt234_f5(m: &mut Machine, t: ObjId) { let _ = m.transition_tcb(t, R); }

// f6 — closure
pub fn kt234_f6(m: &mut Machine, t: ObjId) {
    let _ = (|s: TcbState| m.transition_tcb(t, s))(TcbState::Ready);
}

// f7 — if/else gán biến (bị bắt)
pub fn kt234_f7(m: &mut Machine, t: ObjId, cond: bool) {
    let r = if cond { TcbState::Ready } else { TcbState::Inactive };
    let _ = m.transition_tcb(t, r);
}

// f8 — [Ready][0] (bị bắt)
pub fn kt234_f8(m: &mut Machine, t: ObjId) { let _ = m.transition_tcb(t, [TcbState::Ready][0]); }

// f9 — *&Ready (bị bắt)
pub fn kt234_f9(m: &mut Machine, t: ObjId) { let _ = m.transition_tcb(t, *&TcbState::Ready); }

// f10 — Ready /* comment */ (bị bắt)
pub fn kt234_f10(m: &mut Machine, t: ObjId) {
    let _ = m.transition_tcb(t, TcbState::Ready /* comment */);
}
```

## 3. Đo

Lệnh (toolchain ghim `1.99.1` theo `v2/rust-toolchain.toml`):

```bash
cargo check --manifest-path v2/kernel/Cargo.toml --workspace \
    --target riscv64gc-unknown-none-elf
python3 v2/tools/check-wake-paths.py            # lớp 1 thường
python3 v2/tools/check-wake-paths.py --strict   # lớp 1 --strict
python3 v2/tools/check-wake-depinfo.py --root . # lớp 2 (dep-info)
```

### 3.1 Cây có **đủ 10** dạng

| Lớp | Kết quả | Log |
|---|---|---|
| cargo check (kernel workspace, riscv64) | **RC=0** (5,4 s) | `kt234-logs/kt234-cargo-all.log` |
| lớp 1 thường | **RC=1**, `errors=5` — chỉ f7, f8, f9, f10 | `kt234-logs/kt234-l1-all.log` |
| lớp 1 `--strict` | **RC=1**, `errors=5` — chỉ f7, f8, f9, f10 | `kt234-logs/kt234-l1s-all.log` |
| lớp 2 | **RC=1**, `errors=4` — chỉ f7, f8, f9, f10 | `kt234-logs/kt234-l2-all.log` |

**Không có lỗi nào cho f1..f6** ở cả ba lớp.

### 3.2 Cây chỉ còn **f1..f6** (đường lọt) — XANH toàn phần

| Lớp | Kết quả | Log |
|---|---|---|
| cargo check (kernel workspace, riscv64) | **RC=0** (3,2 s) | `kt234-logs/kt234-cargo-green.log` |
| lớp 1 thường | **RC=0**, `errors=0` | `kt234-logs/kt234-l1-green.log` |
| lớp 1 `--strict` | **RC=0**, `errors=0` | `kt234-logs/kt234-l1s-green.log` |
| lớp 2 | **RC=0**, `errors=0` (152 tệp rustc nạp, 2 unit build RC=0) | `kt234-logs/kt234-l2-green.log` |

⇒ **SÁU đường lọt SỐNG**: f1, f2, f3, f4, f5, f6.

### 3.3 Phân loại từng dạng (gọi thẳng `_transition_calls` + `_var_may_be_ready`)

| Dạng | Đối số thứ hai | Hàm chứa lời gọi | `_var_may_be_ready` | Kết luận |
|---|---|---|---|---|
| f1 | `var s` | `helper` (cùng tệp) | `False` | **SỐNG** |
| f2 | `var s` | `helper` (tệp khác) | `False` | **SỐNG** |
| f3 | `var ready_state` | `kt234_f3` | `False` | **SỐNG** |
| f4 | `var R` | `kt234_f4` | `False` | **SỐNG** |
| f5 | `var R` | `kt234_f5` | `False` | **SỐNG** |
| f6 | `var s` | `kt234_f6` | `False` | **SỐNG** |
| f7 | `var r` | `kt234_f7` | `True` | ĐỎ (đúng) |
| f8 | `unknown` (`[…]`) | `kt234_f8` | — | ĐỎ (đúng) |
| f9 | `unknown` (`*…`) | `kt234_f9` | — | ĐỎ (đúng) |
| f10 | `ready` | `kt234_f10` | — | ĐỎ (đúng) |

Vì sao f1/f2 lọt: lời gọi `helper(m, t, TcbState::Ready)` **không chứa token `transition_tcb`**;
lời gọi `transition_tcb(t, s)` nằm trong `helper`, nơi `s` là **tham số** — `_var_may_be_ready`
chỉ tìm phép gán trong thân `helper`, không thấy (và không có bước kiểm call site của `helper`).
f3/f4/f5/f6 lọt vì đối số thứ hai là **biểu thức** (`ready_state()`, `R`, `R`, closure) mà
`_var_may_be_ready` không mô hình hoá.

### 3.4 Đo **từng dạng một** (cây chỉ có đúng một dạng)

Mỗi dạng được dựng **riêng** (chỉ `mod` dạng đó trong `lib.rs`), rồi chạy đủ bốn lớp:

| Dạng | cargo check | lớp 1 thường | lớp 1 `--strict` | lớp 2 | Kết luận |
|---|---|---|---|---|---|
| f1 | **RC=0** | **RC=0** `errors=0` | **RC=0** `errors=0` | **RC=0** `errors=0` | **SỐNG** |
| f2 | **RC=0** | **RC=0** `errors=0` | **RC=0** `errors=0` | **RC=0** `errors=0` | **SỐNG** |
| f3 | **RC=0** | **RC=0** `errors=0` | **RC=0** `errors=0` | **RC=0** `errors=0` | **SỐNG** |
| f4 | **RC=0** | **RC=0** `errors=0` | **RC=0** `errors=0` | **RC=0** `errors=0` | **SỐNG** |
| f5 | **RC=0** | **RC=0** `errors=0` | **RC=0** `errors=0` | **RC=0** `errors=0` | **SỐNG** |
| f6 | **RC=0** | **RC=0** `errors=0` | **RC=0** `errors=0` | **RC=0** `errors=0` | **SỐNG** |

Log: `kt234-logs/per-form/f<N>-{l1,l1s,cargo,l2}.log`. Cây thật (đối chứng dương) cũng
`errors=0` trong **mỗi** lượt ⇒ 0 báo oan.

## 4. Đối chứng

### 4.1 Âm — dùng thẳng `TcbState::Ready` ⇒ **ĐỎ**

`kt234_neg.rs`:

```rust
pub fn kt234_neg(m: &mut Machine, t: ObjId) {
    let _ = m.transition_tcb(t, TcbState::Ready);
}
```

| Lớp | Kết quả | Log |
|---|---|---|
| cargo check | **RC=0** (biên dịch được) | `kt234-logs/kt234-cargo-neg.log` |
| lớp 1 thường | **RC=1**, `errors=2` — `kt234_neg.rs:7` | `kt234-logs/kt234-l1-neg.log` |
| lớp 1 `--strict` | **RC=1**, `errors=2` — `kt234_neg.rs:7` | `kt234-logs/kt234-l1s-neg.log` |
| lớp 2 | **ĐỎ** — dạng f10 (literal `TcbState::Ready` + comment) bị `E_DEPINFO_TRANSITION` | `kt234-logs/kt234-l2-all.log` |

### 4.2 Dương — cây thật ⇒ **XANH**, 0 báo oan

Trong **cả ba** lượt XANH ở §3.2, cây thật (gồm các đối chứng dương) đều `errors=0`:

- `block_current(cur, to)` — biến **tham số** (`to`), luôn là trạng thái *blocked* ⇒ XANH;
- `wake_step` / `wake_from` / `wake_from_unlink` — cửa wake `WAKE_DOORS` ⇒ XANH;
- các mục `NON_WAKE_READY_SITES` (`dispatch_step`, `yield_current`, `reply_with`,
  `ensure_started`, `tcb_suspend`, `transition_tcb`, `boot_smoke`, `build_rig`, `turn`) ⇒ XANH;
- mọi `transition_tcb(…, TcbState::<BiếnThểKhác>)` ⇒ XANH.

Baseline cây sạch (trước khi thêm repro): lớp 1 thường `errors=0` —
`kt234-logs/kt234-base-l1.log`.

**Báo oan: 0.**

## 5. Vì sao nguy hiểm

Bất biến mà KT-151/KT-155/KT-217 bảo vệ là "**một** cơ chế đánh thức TCB": mọi chuyển
`blocked/sleeping → Ready` phải đi qua cửa wake (`wake_step`/`wake_from`) hoặc có tên trong
allowlist kèm lý do. Sáu dạng trên cho phép mã sản phẩm chuyển `Ready` **ngoài** cửa wake mà
**cả bốn lớp đều xanh** — đúng lớp lỗi mà các bản vá KT-171/177/184/231 vừa đóng cho các dạng
khác (alias kiểu, `include!`, `#[path]`, macro). Một helper dùng chung kiểu
`fn mark_ready(m, t, s: TcbState)` hoặc một `const READY: TcbState = TcbState::Ready;` là đủ
để lách toàn bộ allowlist.

## 6. Đề xuất vá (không làm trong task này)

1. **Luật token mạnh (rẻ, fail-closed)** — mọi token `TcbState::Ready` trong mã sản phẩm phải
   nằm trong `WAKE_DOORS` hoặc trong một **ngữ cảnh so sánh** được allowlist (`==`, `!=`,
   `matches!`, `if let`), hoặc trong `NON_WAKE_READY_SITES`. Cách này bắt **cả sáu** dạng vì
   literal `TcbState::Ready` xuất hiện ở đâu đó trong mã sản phẩm (call site f1/f2, thân
   `ready_state` f3, `const`/`static` f4/f5, closure f6). Cần allowlist ngữ cảnh để tránh báo
   oan chỗ so sánh thật (`wake_step`: `if state == TcbState::Ready`) và test.
2. **Dataflow liên hàm (đúng ngữ nghĩa)** — dựng đồ thị "hàm nào dùng tham số thứ *k* làm đối
   số thứ hai của `transition_tcb`", rồi kiểm **call site** truyền literal `Ready` vào tham số
   đó; xử lý đệ quy/closure/const. Đắt hơn nhưng đóng được cả f1/f2 mà không cần allowlist ngữ
   cảnh rộng.
3. **Tối thiểu** — mở rộng `_var_may_be_ready` thành `_expr_may_be_ready` xử lý call/const/
   static/closure (đóng f3–f6). **Không** đóng được f1/f2 (vẫn là phân tích cục bộ) ⇒ chỉ là
   vá tạm, phải ghi rõ.

Đề xuất: (1) làm hàng rào chính (bắt mọi dạng, kể cả dạng chưa biết), (2) làm kiểm ngữ nghĩa
bổ sung; (3) không đủ.

## 7. Việc **chưa** làm (bàn giao)

- **Chưa vá** — task KT-234 chỉ kiểm; cây đã được **revert** về `cbc2c2af` trước commit.
- Chưa thử các dạng qua **crate khác**, `macro_rules!`, hay `fn`-pointer (KT-231 §5 đã ghi
  fn-pointer là **mở**; cùng họ "không thấy đối số tại chỗ").
- Chưa chạy `gate --fast` (bị cấm trong task này).
- `./v2/bin/v2 check` trên cây sau revert: xem kết quả ghi ở cuối tệp này (nếu chạy).

---

## 8. `./v2/bin/v2 check` trên cây sau revert

```
v2 check: PASS (L1 chạy trong gate --fast)
```

RC=0, 3m29s — log `kt234-logs/kt234-v2check.log`. Cây sau revert **không** còn tệp repro
(`git status` chỉ có journal + log của task này).
