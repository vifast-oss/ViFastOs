# KT-232 — đường lọt **fn-pointer**: `transition_tcb` dùng làm **giá trị**

- **Người kiểm**: L55 (reviewer, task KT-232).
- **Worktree**: `/private/tmp/night3-L55-kt232`, nhánh `night3/L55-kt232`, gốc `cbc2c2af`.
- **Nợ kiểm**: D2-181 §4 — `_transition_calls` chỉ thấy **lời gọi** `transition_tcb(...)`;
  nếu ai lấy **fn-pointer** rồi gọi, luật có thể không thấy.
- **Trạng thái**: **XÁC NHẬN NỢ — 6/7 dạng SỐNG** (7 biến thể sống). **Chưa vá**: task này
  **cấm** sửa `v2/tools/check-wake-paths.py`, `v2/tools/check-wake-depinfo.py`, `v2/bin/v2`.
  Đề xuất ở §7, bàn giao task sau.
- **Kết luận một dòng**: `_transition_calls` chỉ khớp định danh `transition_tcb` **theo sau
  bởi nhóm `(`**; mọi cách **dùng làm giá trị** (fn-pointer, generic, trait object, UFCS)
  đều **vô hình** với cả ba lớp. Riêng **UFCS** `M::transition_tcb(m, t, Ready)` còn bị
  **đọc nhầm đối số** (lấy `t` làm trạng thái) nên **XANH** dù văn bản có `transition_tcb(`.

---

## 1. Cơ chế

`_transition_calls` (KT-231, token-tree) duyệt cây token, tìm **định danh** `transition_tcb`
rồi đòi **token kế tiếp** là nhóm `(`:

```python
if not (node.kind == "ident" and node.val == "transition_tcb"):
    continue
if (i + 1 >= len(nodes)
        or not isinstance(nodes[i + 1], Group)
        or nodes[i + 1].open.val != "("):
    continue          # ← mọi cách dùng làm GIÁ TRỊ rơi ở đây
```

Hệ quả:

1. `Machine::transition_tcb` đứng trước `;` / `,` / `}` ⇒ **không** vào nhánh xử lý ⇒ không
   có mục nào trong `out` ⇒ **không** kiểm cửa wake/allowlist.
2. Lời gọi qua biến (`tt(m, t, Ready)`, `f(m, t, Ready)`, `g(m, t, Ready)`) có định danh
   `tt`/`f`/`g`, **không** phải `transition_tcb` ⇒ không thấy.
3. **UFCS** `M::transition_tcb(m, t, TcbState::Ready)` **có** `transition_tcb(` nên vào nhánh
   xử lý, nhưng `_split_commas` lấy **đối số thứ hai** = `t` (ObjId) — vì checker giả định
   cú pháp method `m.transition_tcb(t, state)`. `t` là định danh trần ⇒ dạng `"var"`;
   `_var_may_be_ready` chỉ ĐỎ khi thân hàm **gán** `t = … TcbState::Ready` ⇒ **XANH**.

Ba lớp dùng **cùng** bộ quét (`check-wake-depinfo.py` R2 `import CWP._transition_calls` —
KT-231 §3.4), nên cả ba cùng mù.

## 2. Bảng kết quả (7 dạng / 8 biến thể)

Mỗi dạng: tệp sản phẩm `v2/kernel/vifast-core/src/kt232_x.rs` + `mod kt232_x;` trong
`v2/kernel/vifast-core/src/lib.rs` (đúng cách repro KT-231), chạy **bốn** phép đo rồi hoàn
nguyên. `cargo check` = `cargo check --manifest-path v2/kernel/Cargo.toml --workspace
--target riscv64gc-unknown-none-elf`.

| Dạng | Mã | cargo | lớp 1 | lớp 1 `--strict` | lớp 2 | Kết luận |
|---|---|---|---|---|---|---|
| 1 | `let tt = Machine::transition_tcb; tt(m, t, TcbState::Ready)` | RC=0 | **XANH** | **XANH** | **XANH** | **SỐNG** |
| 2 | fn-pointer có kiểu tường minh | RC=0 | **XANH** | **XANH** | **XANH** | **SỐNG** |
| 3 | truyền qua tham số hàm trợ giúp `fn call(f: fn(...), ...)` | RC=0 | **XANH** | **XANH** | **XANH** | **SỐNG** |
| 4 | closure `\|m, t\| m.transition_tcb(t, TcbState::Ready)` | RC=0 | **ĐỎ** | **ĐỎ** | **ĐỎ** | bị bắt (đúng) |
| 5a | generic `F: Fn(&mut Machine, ObjId, TcbState)` | RC=0 | **XANH** | **XANH** | **XANH** | **SỐNG** |
| 5b | trait object `&dyn Fn(&mut Machine, ObjId, TcbState)` | RC=0 | **XANH** | **XANH** | **XANH** | **SỐNG** |
| 6 | alias **kiểu** `use crate::Machine as M;` + `M::transition_tcb(m, t, TcbState::Ready)` | RC=0 | **XANH** | **XANH** | **XANH** | **SỐNG** (UFCS) |
| 7 | `let f = <Machine>::transition_tcb; f(m, t, TcbState::Ready)` | RC=0 | **XANH** | **XANH** | **XANH** | **SỐNG** |
| — | **đối chứng âm**: `m.transition_tcb(t, TcbState::Ready)` | RC=0 | **ĐỎ** | **ĐỎ** | **ĐỎ** | đúng |

**Đường lọt SỐNG: 6 dạng / 7 biến thể** — dạng 1, 2, 3, 5a, 5b, 6, 7.
**Báo oan: 0** (dạng 4 bị bắt là **đúng** — thân closure chứa lời gọi thẳng).

Bằng chứng "tệp có được quét": lớp 2 báo `rustc nạp: 146 tệp` khi có exploit, `145 tệp` trên
cây sạch (`base-l2.log`) — đúng +1 tệp; `.d` của `vifast-core` chứa `kt232_x.rs` (đã kiểm
bằng `grep`). Đối chứng âm cùng tệp/cùng dòng bị **ĐỎ** cả ba ⇒ không phải miễn trừ.

## 3. Bằng chứng từng dạng

Nguồn đầy đủ: `v2/agent/JOURNAL/kt232-logs/src/fN.rs`; log: `kt232-logs/fN-{cargo,l1,l1s,l2}.log`.

### Dạng 1 — `let tt = Machine::transition_tcb;`

```rust
use crate::{KError, Machine, ObjId, TcbState};

pub fn kt232_f1(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let tt = Machine::transition_tcb;
    tt(m, t, TcbState::Ready)
}
```

| Phép đo | RC | Log |
|---|---|---|
| cargo check kernel riscv64 | **0** | `f1-cargo.log` |
| lớp 1 thường | **0** (`errors=0`) | `f1-l1.log` |
| lớp 1 `--strict` | **0** (`errors=0`) | `f1-l1s.log` |
| lớp 2 dep-info | **0** (`errors=0`, 146 tệp) | `f1-l2.log` |

### Dạng 2 — fn-pointer có kiểu tường minh

```rust
pub fn kt232_f2(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let tt: fn(&mut Machine, ObjId, TcbState) -> Result<(), KError> = Machine::transition_tcb;
    tt(m, t, TcbState::Ready)
}
```

cargo **0** · lớp 1 **0** · `--strict` **0** · lớp 2 **0** — `f2-*.log`.

### Dạng 3 — truyền qua tham số hàm trợ giúp

```rust
fn call(
    f: fn(&mut Machine, ObjId, TcbState) -> Result<(), KError>,
    m: &mut Machine,
    t: ObjId,
) -> Result<(), KError> {
    f(m, t, TcbState::Ready)
}

pub fn kt232_f3(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    call(Machine::transition_tcb, m, t)
}
```

cargo **0** · lớp 1 **0** · `--strict` **0** · lớp 2 **0** — `f3-*.log`.

### Dạng 4 — closure (BỊ BẮT, đúng)

```rust
pub fn kt232_f4(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let f = |m: &mut Machine, t: ObjId| m.transition_tcb(t, TcbState::Ready);
    f(m, t)
}
```

cargo **0** · lớp 1 **RC=1** `E_STRICT_TRANSITION` tại `kt232_x.rs:6 (kt232_f4)` ·
`--strict` **RC=1** · lớp 2 **RC=1** `E_DEPINFO_TRANSITION` — `f4-*.log`.
Lý do: thân closure **chứa lời gọi thẳng** `m.transition_tcb(t, TcbState::Ready)` nên
token-tree thấy. Bản lách tương đương là **dạng 1** (closure gọi qua fn-pointer).

### Dạng 5 — generic `F: Fn(...)` + trait object `&dyn Fn(...)`

```rust
fn call_generic<F>(f: F, m: &mut Machine, t: ObjId) -> Result<(), KError>
where
    F: Fn(&mut Machine, ObjId, TcbState) -> Result<(), KError>,
{
    f(m, t, TcbState::Ready)
}

pub fn kt232_f5(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    call_generic(Machine::transition_tcb, m, t)
}

pub fn kt232_f5b(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let f: fn(&mut Machine, ObjId, TcbState) -> Result<(), KError> = Machine::transition_tcb;
    let g: &dyn Fn(&mut Machine, ObjId, TcbState) -> Result<(), KError> = &f;
    g(m, t, TcbState::Ready)
}
```

cargo **0** · lớp 1 **0** · `--strict` **0** · lớp 2 **0** — `f5-*.log` (cả hai biến thể
nằm trong cùng tệp, cả tệp XANH).

### Dạng 6 — alias **kiểu** `Machine as M` + UFCS

```rust
use crate::Machine as M;
use crate::{KError, ObjId, TcbState};

pub fn kt232_f6(m: &mut M, t: ObjId) -> Result<(), KError> {
    M::transition_tcb(m, t, TcbState::Ready)
}
```

cargo **0** · lớp 1 **0** · `--strict` **0** · lớp 2 **0** — `f6-*.log`.

**Đây là lỗi khác dạng 1–5**: văn bản **có** `transition_tcb(`, nhưng checker đọc
`args[1] = t` (ObjId) làm trạng thái. Chẩn đoán trực tiếp bằng `_transition_calls`:

```
f6 -> [(7, 'var', 't')]      # "var" + _var_may_be_ready("t") = False ⇒ XANH
```

Đúng ra phải đọc `args[2]` cho UFCS (hoặc nhận diện `Type::method(` là gọi associated
function, không phải method).

### Dạng 7 — `let f = <Machine>::transition_tcb;`

```rust
pub fn kt232_f7(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let f = <Machine>::transition_tcb;
    f(m, t, TcbState::Ready)
}
```

cargo **0** · lớp 1 **0** · `--strict` **0** · lớp 2 **0** — `f7-*.log`.

## 4. Chẩn đoán: luật nhìn thấy gì (import trực tiếp `_transition_calls`)

```
f1 -> []                              # không thấy gì
f2 -> []                              # không thấy gì
f3 -> []                              # không thấy gì
f4 -> [(6, 'ready', 'TcbState::Ready')]   # thấy ⇒ ĐỎ (đúng)
f5 -> []                              # không thấy gì
f6 -> [(7, 'var', 't')]               # thấy NHẦM đối số ⇒ XANH
f7 -> []                              # không thấy gì
```

## 5. Đối chứng âm

Cùng tệp `kt232_x.rs`, cùng vị trí, chỉ đổi thành lời gọi thẳng:

```rust
pub fn kt232_neg(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.transition_tcb(t, TcbState::Ready)
}
```

| Phép đo | RC | Mã | Log |
|---|---|---|---|
| cargo check | 0 | — | `neg-cargo.log` |
| lớp 1 thường | **1** | `E_STRICT_TRANSITION` `kt232_x.rs:6 (kt232_neg)` | `neg-l1.log` |
| lớp 1 `--strict` | **1** | `E_STRICT_TRANSITION` | `neg-l1s.log` |
| lớp 2 | **1** | `E_DEPINFO_TRANSITION` | `neg-l2.log` |

Đối chứng âm **giống hệt** cho cả 6 dạng sống (cùng tệp, cùng module, cùng dòng) — một lần
đo đủ chứng minh tệp **không** bị miễn; không chạy lặp 6 lần vô nghĩa.

## 6. Đối chứng dương (cây sạch — 0 báo oan)

| Phép đo | RC | Log |
|---|---|---|
| lớp 1 thường | **0** (`errors=0`) | `base-l1.log` |
| lớp 1 `--strict` | **0** (`errors=0`) | `base-l1s.log` |
| lớp 2 | **0** (`errors=0`, 145 tệp) | `base-l2.log` |

17 mục được miễn **đúng** trên cây thật (đều XANH): 2 **cửa wake** `wake_step`
(`core_sched.rs:307`, `kobj.rs:5794`) + 15 mục `NON_WAKE_READY_SITES`
(`ensure_started`, `reply_with`, `dispatch_step`, `yield_current`, `transition_tcb`,
`tcb_suspend`, `build_rig`, `turn`, `boot_smoke` — cả kernel lẫn sim). `wake_from`/
`wake_from_unlink` gọi `wake_step`; `wake_addr` (`core_sync.rs:544`) gọi `wake_step(tcb, true)`
— đều nằm trong đường XANH của cây thật.

## 7. Nguyên nhân gốc + đề xuất (KHÔNG thực hiện trong task này)

Hai lỗi **độc lập**, cần hai bản vá:

1. **Dùng làm giá trị** (dạng 1, 2, 3, 5a, 5b, 7): `_transition_calls` bỏ qua mọi
   `transition_tcb` **không** theo sau bởi `(`. Đề xuất: thêm luật "**tham chiếu**
   `transition_tcb` ngoài vị trí định nghĩa `fn transition_tcb(`" ⇒ ĐỎ fail-closed (mọi
   fn-pointer/generic/trait-object/`<Machine>::` đều lộ), trừ khi nằm trong cửa wake/allowlist.
   Đây là luật **hẹp** (chỉ một định danh), không báo oan cây thật (cây thật chỉ có lời gọi
   method + định nghĩa).
2. **UFCS đọc nhầm đối số** (dạng 6): khi định danh đứng sau `::` (đường dẫn
   `Type::transition_tcb(`), đối số thứ hai **không phải** trạng thái — trạng thái là
   `args[2]` (nếu `args[0]` là receiver) hoặc `args[1]` (nếu gọi kiểu `Machine::transition_tcb`
   không receiver — không hợp lệ với `&mut self`). Đề xuất: nhận diện tiền tố `::` và dịch
   chỉ số; hoặc fail-closed với mọi lời gọi có tiền tố `::` trừ khi chứng minh được.

**Rủi ro còn lại** (không thuộc 7 dạng, ghi để bàn giao): truyền `Ready` qua **tham số liên
hàm** (đã ghi ở KT-231 §5) vẫn mở; dạng 6 chứng minh rằng ngay cả khi **có** `transition_tcb(`
trong văn bản, luật vẫn có thể XANH nếu đối số bị đọc lệch.

## 8. Tệp / log

- Journal này: `v2/agent/JOURNAL/kt232-fnptr.md`.
- Log: `v2/agent/JOURNAL/kt232-logs/` — `base-{l1,l1s,l2}.log`, `fN-{cargo,l1,l1s,l2}.log`
  (N=1..7), `neg-{cargo,l1,l1s,l2}.log`, script `run_form.sh`, nguồn `src/fN.rs`, `src/neg.rs`.
- Không sửa: `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `docs/**`, `AGENTS.md`,
  `v2/AGENTS.md`, journal agent khác, ba tệp công cụ bị cấm.
- Không chạy `gate --fast`; chỉ `./v2/bin/v2 check` (xem §9).

## 9. `./v2/bin/v2 check` trên cây sạch (sau khi hoàn nguyên)

`./v2/bin/v2 check` → **RC=0**, `v2 check: PASS` (`kt232-logs/v2-check.log`, 797 dòng).
Không chạy `gate --fast` (bị cấm trong task này).

