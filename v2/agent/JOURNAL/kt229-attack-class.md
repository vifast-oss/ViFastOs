# KT-229 — tấn công checker `wake` theo **LỚP** (reviewer L54)

- **Vai**: reviewer tấn công theo lớp (không chọn mẫu từ danh sách 13 đường lọt cũ).
- **Worktree**: `/private/tmp/night3-L54-kt229`, nhánh `night3/L54-kt229`, gốc `7ab09871`.
- **Kết quả**: **2 đường lọt SỐNG** (cả ba lớp XANH + `cargo check` thành công):
  - **#14** — `macro_rules!` **lồng** che tên macro "an toàn" ⇒ `wake()` thật. **Đã vá ở KT-230**
    (commit `3e928e0b`, journal `kt230-nested-macro.md`).
  - **#15** — alias `TcbState` ⇒ `transition_tcb(…, Ready)` ngoài cửa wake. **CHƯA VÁ** — bàn giao.
- **Ghi chú thời gian**: lead chuyển vai sang tác giả KT-230 sau khi nhận báo #14 ⇒ ma trận **chưa
  chạy hết ≥40 tổ hợp**; bảng dưới là các tổ hợp **đã đo**, phần còn lại ghi ở §4.

## 1. Bảng tổ hợp đã đo

Mỗi ca: dựng mã **biên dịch được** trong worktree, chạy **cả ba** lớp (lớp một thường, lớp một
`--strict`, lớp hai `check-wake-depinfo.py --root .`), rồi hoàn nguyên. Log: `kt229-logs/`,
`kt230-logs/`.

| # | Tổ hợp (chiều × chiều) | cargo | lớp 1 | lớp 1 `--strict` | lớp 2 | Kết luận |
|---|---|---|---|---|---|---|
| B1 | macro che tên an toàn trong tệp **`.txt`** `include!` vào `v2/lib` + `let wake` + `matches!(m, wake, t)` | RC=0 | **XANH** | **XANH** | **ĐỎ** | lỗ hổng **riêng lớp 1** (lớp 2 dùng danh sách tệp rustc nạp) — **đã vá ở KT-230** (extra_files + lib→extra) |
| B2 | macro che tên an toàn **lồng trong thân `macro_rules!`** (`.rs` trong `v2/lib`) + `let wake` + `matches!(m, wake, t)` | RC=0 | **XANH** | **XANH** | **XANH** | **ĐƯỜNG LỌT SỐNG #14** — đã vá KT-230 |
| B2n | B2 bỏ `let wake = 0u32;` (đối chứng âm) | RC=0 | **ĐỎ** | **ĐỎ** | **ĐỎ** | đúng — chứng minh #14 phụ thuộc chuỗi miễn |
| A1 | `use crate::TcbState::Ready as R;` + `m.transition_tcb(t, R)` | RC=0 | XANH | **ĐỎ** | **ĐỎ** | bị bắt qua fail-closed "`TcbState::Ready` ngoài hàm" (dòng `use`) |
| A2 | `use crate::TcbState as S;` + `m.transition_tcb(t, S::Ready)` | RC=0 | **XANH** | **XANH** | **XANH** | **ĐƯỜNG LỌT SỐNG #15** — luật R2/E_STRICT_TRANSITION khớp **theo tên** `TcbState::Ready` trên **một dòng**; alias che cả hai |
| A2n | A2 dùng thẳng `TcbState::Ready` (đối chứng âm) | RC=0 | XANH | **ĐỎ** | **ĐỎ** | đúng — `E_STRICT_TRANSITION` + `E_DEPINFO_TRANSITION` |

## 2. Đường lọt SỐNG #14 (đã vá)

Xem `kt230-nested-macro.md`. Tóm tắt: `_shadowed_safe_macros` chỉ thấy `macro_rules!` mức mục
(`_walk_items` bỏ qua thân macro) ⇒ macro lồng che tên `matches` không bị phát hiện ⇒ token `wake`
là đối số macro "an toàn" + binding cục bộ `wake` ⇒ được miễn ⇒ macro mở rộng `$m.$x($t)` thành
`m.wake(t)` thật. **Đã vá**: đệ quy mọi tầng + `extra_files` + đích `v2/lib` vào `extra`.

## 3. Đường lọt SỐNG #15 (CHƯA VÁ — bàn giao)

### Repro (đã đo trên HEAD sau KT-230, commit `3e928e0b`)

`v2/kernel/vifast-core/src/kt229_a1.rs`:

```rust
use crate::TcbState as S;
use crate::{Machine, ObjId};

pub fn kt229_a2(m: &mut Machine, t: ObjId) {
    let _ = m.transition_tcb(t, S::Ready);
}
```

`v2/kernel/vifast-core/src/lib.rs`: `mod kt229_a1;`

### Bằng chứng

| Lớp | Kết quả | Log |
|---|---|---|
| cargo (`cargo check --manifest-path v2/kernel/Cargo.toml --workspace --target riscv64gc-unknown-none-elf`) | **thành công** | — |
| lớp 1 thường | **XANH** (`errors=0`) | `kt229-logs/a2/a2-l1.log` |
| lớp 1 `--strict` | **XANH** (`errors=0`) | `kt229-logs/a2/a2-l1s.log` |
| lớp 2 | **XANH** (`errors=0`) | `kt229-logs/a2/a2-l2.log` |

**Đối chứng âm** (dùng thẳng `TcbState::Ready`): lớp 1 `--strict` **ĐỎ** `E_STRICT_TRANSITION`,
lớp 2 **ĐỎ** `E_DEPINFO_TRANSITION` — `kt229-logs/a2/a2n-l1s.log`, `a2n-l2.log`.

### Nguyên nhân

`RE_READY_TRANSITION = transition_tcb\s*\([^;]*?TcbState::Ready` — khớp **văn bản** trên **một
dòng**. Alias `TcbState as S` làm lời gọi thành `transition_tcb(t, S::Ready)`:
- dòng gọi **không** chứa `TcbState::Ready` ⇒ regex không khớp;
- nhánh fail-closed "`transition_tcb` nhiều dòng" chỉ kiểm khi **có** `transition_tcb` mà **không**
  khớp regex — nhưng nó chỉ tìm `TcbState::Ready` trong **cửa sổ 8 dòng** quanh lời gọi; alias
  không có chuỗi đó ⇒ không bắt;
- dòng `use crate::TcbState as S;` có `TcbState` nhưng **không** có `TcbState::Ready` ⇒ không bắt.

### Đề xuất vá (KT-231)

1. **Fail-closed theo cấu trúc**: mọi lời gọi `transition_tcb(` trong mã sản phẩm **ngoài**
   `WAKE_DOORS` phải có **đối số thứ hai literal đúng** `TcbState::Ready` (token-tree, không regex);
   mọi dạng khác (alias, biến, `S::Ready`, `let r = …`) ⇒ **ĐỎ**. Đây là hướng chắc nhất: không cần
   phân giải kiểu, chỉ cần "không chứng minh được là `TcbState::Ready` ⇒ ĐỎ".
2. Hoặc: cấm `use …::TcbState as …` / `use …::TcbState::Ready as …` trong mã sản phẩm (fail-closed),
   và cấm `let <x> = TcbState::Ready` ngoài cửa wake.
3. Áp **cả hai lớp** (lớp 2 dùng lại `RE_READY_TRANSITION`/`WAKE_DOORS` — sửa một chỗ, chạy cả hai).

## 4. Phạm vi đã phủ / chưa phủ

**Đã đi (đo được)**:

- Chiều 1 (cơ chế đưa mã vào): `macro_rules!` (định nghĩa + gọi), `include!` (tệp `.txt`), `#[path]`
  (qua biến thể `.txt`), macro lồng nhiều tầng.
- Chiều 3 (`#[cfg]`): `cfg(test)` (KT-219 đã vá — không thử lại), `cfg(feature="ktest")` (KT-227 P0-4
  đã vá — không thử lại).
- Chiều 5 (phạm vi quét): `v2/lib` (đích `include!` không `.rs`), `v2/kernel/<crate>/src/**`.
- Chiều 6 (hình dạng `wake`): `m.wake(t)` qua macro; `transition_tcb(…, Ready)` qua alias.

**Chưa đi (bàn giao cho reviewer sau — ưu tiên theo nghi vấn)**:

- **Chiều 6, ưu tiên cao**: `let tt = Machine::transition_tcb;` + gọi cách xa >8 dòng (nghi vấn:
  cửa sổ fail-closed 8 dòng không phủ); `wake` qua trait method/Deref/closure (token `wake` vẫn
  hiện nên khó lọt, nhưng chưa đo).
- **Chiều 1**: proc-macro sinh `wake` từ **chuỗi** (cây **không** có crate `proc-macro = true` —
  đo `grep proc-macro` trên mọi `Cargo.toml` = 0; nếu thêm thì `_shadowed_safe_macros` **không**
  thấy — rủi ro còn lại ghi ở `kt230-nested-macro.md` §5); `build.rs` sinh `$OUT_DIR` + `include!`
  **ẩn trong macro** (nghi vấn: lớp 2 bỏ tệp trong `target/` chỉ **cảnh báo**, lớp 1 không thấy
  `include!` trong macro của tệp ngoài `MACRO_SCAN_ROOTS` — **chưa đo**).
- **Chiều 2** (vị trí token): mới đo qua macro-arg + local-binding; chưa đo `match` guard, `if let`,
  const generic, array length, `where` clause.
- **Chiều 4** (gốc phân giải): mới đo alias `TcbState`; chưa đo symlink/`..`/tuyệt đối/Unicode cho
  **lớp 1** sau KT-230 (KT-224 đã đo trước bản vá).
- **Chiều 5**: `v2/kernel/<crate>/init/**`, `tests/**`, `benches/**`, `examples/**`, tệp gốc crate
  — chưa đo trong phiên này.

## 5. Kết luận

- **CÒN LỌT**: #14 (đã vá KT-230) + **#15 (CHƯA VÁ)**.
- **Báo oan**: 0 trên cây thật sau KT-230 (lớp 1 thường + `--strict` `errors=0`; `v2 check` PASS).
- **Không** tuyên bố "ĐÓNG ĐƯỢC" — còn #15 sống và ma trận chưa phủ hết.
