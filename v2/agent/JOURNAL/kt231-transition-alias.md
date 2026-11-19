# KT-231 — đường lọt #15: alias kiểu che `transition_tcb(…, Ready)`

- **Người tìm**: reviewer L54 (task KT-229, tấn công theo **lớp**).
- **Người vá**: L54 (task KT-231, cùng nhánh `night3/L54-kt229`).
- **Trạng thái**: **ĐÃ VÁ** — repro #15 ĐỎ cả ba; cây thật xanh; selftest xanh.
- **Nguồn sự thật**: `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` §4 (dòng KT-231).

---

## 1. Cơ chế

Luật KT-155 ("mọi `transition_tcb(…, TcbState::Ready)` trong mã sản phẩm phải nằm trong **cửa
wake** `WAKE_DOORS` hoặc có tên trong `NON_WAKE_READY_SITES`") được cài bằng **regex văn bản**
trên **một dòng**:

```python
RE_READY_TRANSITION = re.compile(r"transition_tcb\s*\([^;]*?TcbState::Ready")
```

Hai lớp dùng chung regex này (lớp 2 import `CWP.RE_READY_TRANSITION`). Alias **kiểu** che cả hai:

```rust
use crate::TcbState as S;
pub fn kt231_a(m: &mut Machine, t: ObjId) {
    let _ = m.transition_tcb(t, S::Ready);   // văn bản KHÔNG có `TcbState::Ready`
}
```

- dòng gọi không chứa `TcbState::Ready` ⇒ regex không khớp;
- nhánh fail-closed "nhiều dòng" chỉ kiểm khi **có** `transition_tcb` mà **không** khớp regex —
  nó tìm `TcbState::Ready` trong cửa sổ tám dòng quanh lời gọi; alias không có chuỗi đó ⇒ không bắt;
- dòng `use crate::TcbState as S;` có `TcbState` nhưng **không** có `TcbState::Ready` ⇒ không bắt.

Biến thể `use crate::TcbState::Ready as R;` thì **bị bắt** (fail-closed "`TcbState::Ready` ngoài
hàm" trên dòng `use`) — nên chỉ alias **kiểu** mới lọt.

## 2. Repro (đã đo trên HEAD sau KT-230, commit `3e928e0b`)

`v2/kernel/vifast-core/src/kt231_a.rs`:

```rust
use crate::TcbState as S;
use crate::{Machine, ObjId};

pub fn kt231_a(m: &mut Machine, t: ObjId) {
    let _ = m.transition_tcb(t, S::Ready);
}
```

`v2/kernel/vifast-core/src/lib.rs`: `mod kt231_a;`

### Bằng chứng TRƯỚC khi vá (XANH cả ba)

| Lớp | Kết quả | Log |
|---|---|---|
| cargo (`cargo check --manifest-path v2/kernel/Cargo.toml --workspace --target riscv64gc-unknown-none-elf`) | **thành công** | — |
| lớp 1 thường | **XANH** (`errors=0`) | `kt229-logs/a2/a2-l1.log` |
| lớp 1 `--strict` | **XANH** (`errors=0`) | `kt229-logs/a2/a2-l1s.log` |
| lớp 2 | **XANH** (`errors=0`) | `kt229-logs/a2/a2-l2.log` |

### Đối chứng âm (dùng thẳng `TcbState::Ready`)

`--strict` **ĐỎ** `E_STRICT_TRANSITION`, lớp 2 **ĐỎ** `E_DEPINFO_TRANSITION` —
`kt229-logs/a2/a2n-l1s.log`, `a2n-l2.log`.

## 3. Bản vá

### 3.1 `_transition_calls` — token-tree, không regex văn bản

Quét **toàn cây token** tìm định danh `transition_tcb` + nhóm `(…)` (bỏ định nghĩa `fn
transition_tcb(`), tách đối số theo dấu phẩy mức ngoài, lấy **đối số thứ hai**, đọc các **đoạn
định danh** của đường dẫn (`a::b::C` ⇒ `[a, b, C]`):

| Dạng | Điều kiện | Xử lý |
|---|---|---|
| `ready` | hai đoạn cuối là `TcbState::Ready` (kể cả path-qualified `vifast_core::TcbState::Ready`) | cửa wake / allowlist như cũ |
| `other` | hai đoạn cuối là `TcbState::<BiếnThểKhác>` (kể cả có `{ … }` sau) | **chứng minh không Ready** ⇒ XANH |
| `alias` | đoạn cuối `Ready` nhưng đoạn trước **khác** `TcbState` (`S::Ready`, `Self::Ready`, đường dẫn lạ) | **ĐỎ** (fail-closed) |
| `var` | một định danh trần (biến) | **ĐỎ** nếu hàm gán biến từ `TcbState::Ready` (dataflow-lite) |
| `unknown` | dạng khác (thiếu đối số, không phải đường dẫn biến thể) | **ĐỎ** (fail-closed) |

### 3.2 `_var_may_be_ready` — dataflow-lite cho biến

Vì sao không "mọi biến ⇒ ĐỎ": cây thật có `block_current(&mut self, to: TcbState)` gọi
`self.transition_tcb(cur, to)` với `to` là **tham số** (luôn là trạng thái *blocked* — hợp lệ).
Nếu coi mọi biến là ĐỎ ⇒ **báo oan** cây thật. Nay chỉ ĐỎ khi thân hàm có phép gán
`var = … TcbState::Ready` (bắt `let r = TcbState::Ready; m.transition_tcb(t, r)`).
**Rủi ro còn lại**: truyền `Ready` qua **tham số liên hàm** (`fn f(to: TcbState)` gọi từ nơi
truyền `TcbState::Ready`) — không phát hiện được bằng phân tích cục bộ; ghi rõ, **không** tuyên
bố đã đóng.

### 3.3 Chạy ở **mọi** chế độ (lớp 1)

Lead KT-231 yêu cầu repro #15 ĐỎ **cả lớp 1 thường** (trước chỉ `--strict`). Nay luật transition
chạy **ngoài** `if strict:`; `--strict` chỉ còn thêm luật token `wake` (mục 6) và kiểm
`E_STRICT_STALE` (giữ hành vi cũ — cây fixture selftest không đủ tệp repo thật nên kiểm stale ở
chế độ thường sẽ báo oan; `v2 check` chạy **cả** `--strict` nên không mất).

### 3.4 Lớp 2 dùng **cùng** bộ quét

`check-wake-depinfo.py` R2 gọi `CWP._transition_calls` + `CWP._var_may_be_ready` (import, không
chép — KT-214). Sửa **một chỗ**, áp **cả hai lớp**.

## 4. Đo sau khi vá

### 4.1 Repro #15 ⇒ ĐỎ cả ba

| Lớp | Kết quả | Log |
|---|---|---|
| lớp 1 thường | **RC=1**, `E_STRICT_TRANSITION … S::Ready … alias/đường dẫn lạ` | `kt231-logs/k231-l1.log` |
| lớp 1 `--strict` | **RC=1**, cùng lỗi | `kt231-logs/k231-l1s.log` |
| lớp 2 | **RC=1**, `E_DEPINFO_TRANSITION …` | `kt231-logs/k231-l2.log` |

### 4.2 Đối chứng âm (dùng thẳng `TcbState::Ready`) ⇒ vẫn ĐỎ

- lớp 1 thường: **RC=1** `E_STRICT_TRANSITION` — `kt231-logs/k231n-l1.log`
- lớp 1 `--strict`: **RC=1** — `kt231-logs/k231n-l1s.log`
- lớp 2: **RC=1** `E_DEPINFO_TRANSITION` — `kt231-logs/k231n-l2.log`

### 4.3 Cây thật ⇒ XANH (0 báo oan)

- lớp 1 thường: `errors=0` — `kt231-logs/k231c-l1.log`
- lớp 1 `--strict`: `errors=0` — `kt231-logs/k231c-l1s.log`
- lớp 2: `errors=0` — `kt231-logs/k231c-l2.log`

Đối chứng dương trên cây thật (đều XANH): `wake_step`/`wake_from`/`wake_from_unlink` (cửa wake),
các mục `NON_WAKE_READY_SITES` (`dispatch_step`, `yield_current`, `reply_with`, `ensure_started`,
`tcb_suspend`, `transition_tcb`, `boot_smoke`, `build_rig`, `turn`), mọi lời gọi
`transition_tcb(…, TcbState::<BiếnThểKhác>)` (`Inactive`/`BlockedOnReply`/`BlockedOnSend`/
`BlockedOnRecv`/`Sleeping`/`Running`/`Dead`), và `block_current(cur, to)` (biến tham số).

### 4.4 Selftest

| Bộ | Trước | Sau | Thêm |
|---|---|---|---|
| lớp 1 (`check-wake-paths.py --selftest`) | **281/281** | **295/295** | **+14** ca KT-231 |
| lớp 2 (`check-wake-depinfo.py --selftest`) | **34/34** | **37/37** | **+3** ca KT-231 |

Ca lớp 1 (mỗi ca chạy **cả** chế độ thường và `--strict`):

1. repro #15 alias `TcbState as S` + `S::Ready` ⇒ **ĐỎ** (×2)
2. `Self::Ready` ⇒ **ĐỎ** (×2)
3. biến gán từ `TcbState::Ready` rồi truyền ⇒ **ĐỎ** (×2)
4. đối chứng dương: `TcbState::Blocked` ngoài cửa ⇒ **XANH** (×2)
5. đối chứng dương: `TcbState::Ready` trong hàm có mục allowlist ⇒ **XANH** (×2)
6. đối chứng dương: biến **tham số** `to` (`block_current`) ⇒ **XANH** (×2)
7. đối chứng dương: path-qualified `vifast_core::TcbState::Ready` ngoài cửa ⇒ **ĐỎ** (×2)

Ca lớp 2:

- (35) alias `TcbState as S` + `S::Ready` ⇒ **ĐỎ** `E_DEPINFO_TRANSITION`
- (36) đối chứng dương: `TcbState::Blocked` ⇒ **XANH**
- (37) đối chứng dương: biến tham số `to` ⇒ **XANH**

## 5. Rủi ro còn lại (ghi rõ, **không** tuyên bố đã đóng)

| Dạng | Trạng thái | Ghi chú |
|---|---|---|
| Truyền `Ready` qua **tham số liên hàm** | **Mở** | `fn f(to: TcbState) { m.transition_tcb(t, to) }` gọi từ nơi truyền `TcbState::Ready` — phân tích cục bộ không thấy. Cần dataflow liên hàm (ngoài phạm vi). |
| `transition_tcb` dùng làm **giá trị** (fn-pointer) | **Mở** | `let tt = Machine::transition_tcb; tt(m, t, TcbState::Ready)` — `_transition_calls` chỉ thấy **lời gọi**; chưa có luật "dùng làm giá trị". Đề nghị reviewer sau thử. |
| Alias của **biến thể khác** (`S::Blocked`) | **Đỏ có chủ đích** | `segs = [S, Blocked]` ⇒ `unknown` ⇒ ĐỎ (fail-closed). Cây thật không có. |

## 6. Việc **chưa** làm (bàn giao)

- Ma trận ≥40 tổ hợp của KT-229 vẫn **chưa chạy hết** (xem `kt229-attack-class.md` §4).
- Chưa chạy `gate --fast` (bị cấm); đã chạy `./v2/bin/v2 check` — xem `kt231-logs/v2-check.log`.
