# KT-239 — đóng gói repro bền WB-16/WB-17 + vá UFCS đọc đúng `args[2]`

- **Người làm**: sub-agent E (tiếp quản từ agent C, đã dừng) · **Luồng**: C · **D2-192**.
- **Worktree**: `/private/tmp/night4-C-kt239`, nhánh `night4/C-kt239`, gốc `298b92c5`.
- **Tệp sửa duy nhất**: `v2/tools/check-wake-paths.py` (**+435/−4**: C để lại +341/−4, E làm
  chặt thêm +94/−0 — xem §4). Không sửa `spec/`, `QUEUE.md`, `DECISIONS.md`, `STATE.md`, `docs/**`,
  `AGENTS.md`, `v2/AGENTS.md`, journal của agent khác, `v2/tools/check-wake-depinfo.py`.
- **Không** nới ngưỡng, **không** thêm allowlist, **không** `#[ignore]`, **không** bỏ ca.
- **Nguồn sự thật**: [`kt234-interfn.md`](kt234-interfn.md) (WB-16),
  [`kt232-fnptr.md`](kt232-fnptr.md) (WB-17), D2-190 §2, D2-192.

---

## 1. Vì sao

`KT-236` (D2-190 §2) khoá **WB-16** (Ready truyền qua **tham số liên hàm**) và **WB-17**
(`transition_tcb` dùng làm **giá trị** / fn-pointer) là **P0 chặn nghiệm thu**. Hai journal gốc
chứng minh đường lọt **bằng tay** (dựng tệp, `cargo check`, chạy ba lớp, hoàn nguyên) — không có
ca tự động nào giữ chúng. Nếu không đóng gói, một bản vá sau có thể **làm mất** đúng hành vi đã
đo mà không ai biết.

KT-239 làm hai việc:

1. **Vá một lỗi thật trong `_transition_calls`** — dạng UFCS (WB-17 f6) bị **đọc nhầm đối số**,
   nên XANH dù văn bản có `transition_tcb(` và `TcbState::Ready`.
2. **Đóng gói 36 ca tự động** (6 dạng WB-16 + 7 biến thể WB-17 + đối chứng âm/dương), chạy
   trong `check-wake-paths.py --selftest` — **một lệnh, không cần cargo, không cần QEMU**.

## 2. UFCS đã sửa gì

`_transition_calls` tìm **định danh** `transition_tcb` rồi đòi token kế tiếp là nhóm `(`. Với
method `.transition_tcb(t, state)` thì đối số thứ hai (`args[1]`) **là** trạng thái. Nhưng với
UFCS `M::transition_tcb(m, t, state)` — dạng WB-17 f6 — **receiver chiếm `args[0]`**, nên trạng
thái nằm ở **`args[2]`**; `args[1]` là `ObjId`.

Trước vá, mã đọc `segs_of(args[1])` cho **mọi** lời gọi:

```rust
use crate::Machine as M2;
let _ = M2::transition_tcb(self, t, TcbState::Ready);   // args[1] = `t` ⇒ ("var", "t")
```

`("var", "t")` ⇒ `_var_may_be_ready(text, fn, "t")` = `False` (thân hàm không gán `t` từ
`TcbState::Ready`) ⇒ **XANH**. Đúng đường lọt: `cargo check` RC=0, văn bản có đủ
`transition_tcb(` + `TcbState::Ready`, mà cả ba lớp đều im.

Sau vá (nhận diện tiền tố `::` bằng token, không bằng regex văn bản):

```python
is_ufcs = i >= 2 and _is_punct(nodes[i - 1], ":") and _is_punct(nodes[i - 2], ":")
args = _split_commas(nodes[i + 1].children)
if is_ufcs:
    if len(args) < 3:                       # fail-closed, không đoán
        out.append((node.line, "unknown", "UFCS thiếu đối số trạng thái (args[2])"))
        continue
    segs = segs_of(args[2])                 # ← trạng thái thật
else:
    if len(args) < 2:
        out.append((node.line, "unknown", "thiếu đối số thứ hai"))
        continue
    segs = segs_of(args[1])
```

Ba tính chất của bản vá:

- **Chỉ đổi chỉ số khi có `::`** — `.transition_tcb(` (method) giữ nguyên `args[1]`; không có
  nguy cơ đổi hành vi trên 100% lời gọi method đang có.
- **Fail-closed**: UFCS thiếu `args[2]` ⇒ `unknown` ⇒ ĐỎ, không suy diễn.
- **Không dùng regex văn bản** — vẫn là token-tree (KT-231), nên alias kiểu
  `use crate::Machine as M2;` không đánh lừa được.

## 3. 36 ca mới kiểm gì

Tất cả trong `selftest()` của `v2/tools/check-wake-paths.py`, dựng bằng `_fixture_v2` (cây v2
tối thiểu, **không** cần cargo), chạy **cả hai** chế độ (`thường` + `--strict`) ⇒ 18 kịch bản ×
2 = **36 ca**. Tổng selftest: **295 → 331**.

| Nhóm | Số ca | Kỳ vọng | Ý nghĩa |
|---|---|---|---|
| WB-16 f1..f6 | 12 | **ghi nhận sống** (XANH) | 6 dạng đang sống, ghi đúng thực trạng để luồng A (KT-237) đóng |
| WB-16 f2 đủ 2 tệp | 2 | **ghi nhận sống** (XANH) | dựng đúng cấu trúc journal: call site 1 tệp, lời gọi ở tệp helper |
| WB-17 f1/f2/f3/f5a/f5b/f7 | 12 | **ghi nhận sống** (XANH) | 6 biến thể fn-pointer: checker **không thấy** lời gọi |
| WB-17 f6 (UFCS) | 2 | **ĐỎ** `E_STRICT_TRANSITION` | **bản vá**: đọc `args[2]` ⇒ bắt được |
| Đối chứng âm (thẳng `Ready`) | 2 | **ĐỎ** `E_STRICT_TRANSITION` | chứng minh cùng tệp/cùng fixture **không** bị miễn |
| Đối chứng dương | 6 | **XANH** (không báo oan) | cửa wake, `block_current(to)`, biến thể khác (gồm UFCS `Blocked`) |

**"Ghi nhận sống" là ca có chủ đích**: nó assert checker **XANH** đúng thực trạng đã đo, kèm
assert **cơ chế** (xem §4) để XANH là do **đúng đường lọt đã ghi**, không phải do tệp bị bỏ
qua. Khi luồng A (KT-237) đóng WB-16/WB-17, **đổi kỳ vọng sang ĐỎ tại đúng chỗ này** — ca sẽ
báo động, không im lặng.

## 4. Mỗi ca assert **nguyên nhân**, không chỉ mã lỗi

Đây là phần **làm chặt** của E trên bản của C (C chỉ assert `rep.ok` / `any("E_STRICT_TRANSITION")`).
`_kt239_case` nay bắt buộc thêm:

| Tham số | Bắt buộc điều gì | Chặn cái gì |
|---|---|---|
| `calls` | `_transition_calls(văn bản, relp)` trả **đúng** `[(kind, desc)]` | ca XANH vì tệp bị bỏ qua / fixture rỗng; ca ĐỎ vì lý do khác |
| `need` | chuỗi phải có trong văn bản | ca **rỗng nghĩa** (không có gì để kiểm mà vẫn "xanh") |
| `site` | thông điệp lỗi phải nêu đúng `relp:dòng (tên hàm)` | ĐỎ do chỗ khác trong cây |
| (ĐỎ) `all(e.startswith("E_STRICT_TRANSITION"))` | ĐỎ **chỉ** vì luật transition | `E_ANCHOR_MISSING`/`E_LEX_*`/`E_STRICT_STALE`/cây rỗng chen vào làm "bằng chứng" |
| (ĐỎ) `"ngoài cửa wake" in e` | thông điệp nêu **đúng nguyên nhân** | ĐỎ vì "thiếu đối số"/"alias" — nguyên nhân khác |

Chốt chống hồi quy mạnh nhất là `calls` của ca UFCS: `[("ready", "TcbState::Ready")]`. Hoàn
nguyên bản vá (`segs_of(args[1])`) ⇒ phân loại thành `("var", "t")` ⇒ ca **ĐỎ ngay** (đo ở §6,
MUT-A). Nói cách khác: **không thể revert bản vá mà selftest còn xanh**.

Ngoài ra đối chứng âm dùng **cùng đường dẫn** `v2/kernel/vifast-core/src/evil.rs` như 12 ca
"ghi nhận sống" — cùng tệp, cùng fixture: Ready-literal ⇒ ĐỎ, dạng đã ghi ⇒ XANH. Đó là bằng
chứng "tệp có được quét" ngay trong selftest.

## 5. Bảng WB-16 / WB-17 → ca tương ứng

### WB-16 — `TcbState::Ready` truyền qua **tham số liên hàm** ([`kt234-interfn.md`](kt234-interfn.md) §2)

| Dạng | Mã journal | Ca trong selftest | `_transition_calls` | Kỳ vọng |
|---|---|---|---|---|
| f1 | helper **cùng tệp** | `KT-239 wb16-f1-helper-cung-tep` ×2 | `[("var","s")]` | XANH (sống) |
| f2 | helper **tệp khác** | `KT-239 wb16-f2-helper-tep-khac` ×2 + `… (đủ 2 tệp)` ×2 | `[]` ở call site, `[("var","s")]` ở helper | XANH (sống) |
| f3 | helper **trả về** `Ready` | `KT-239 wb16-f3-ham-tra-ve-ready` ×2 | `[("var","ready_state")]` | XANH (sống) |
| f4 | `const R = TcbState::Ready` | `KT-239 wb16-f4-const` ×2 | `[("var","R")]` | XANH (sống) |
| f5 | `static R = TcbState::Ready` | `KT-239 wb16-f5-static` ×2 | `[("var","R")]` | XANH (sống) |
| f6 | **closure** IIFE | `KT-239 wb16-f6-closure` ×2 | `[("var","s")]` | XANH (sống) |
| f7..f10 | if/else gán, `[Ready][0]`, `*…`, literal | — (đã ĐỎ đúng, không cần ca sống) | — | — |

### WB-17 — `transition_tcb` dùng làm **giá trị** ([`kt232-fnptr.md`](kt232-fnptr.md) §2)

| Dạng | Mã journal | Ca trong selftest | `_transition_calls` | Kỳ vọng |
|---|---|---|---|---|
| 1 | `let tt = Machine::transition_tcb;` | `KT-239 wb17-f1-fn-pointer-tran` ×2 | `[]` | XANH (sống) |
| 2 | fn-pointer **có kiểu** tường minh | `KT-239 wb17-f2-fn-pointer-kieu` ×2 | `[]` | XANH (sống) |
| 3 | truyền qua **tham số hàm** trợ giúp | `KT-239 wb17-f3-qua-tham-so-ham` ×2 | `[]` | XANH (sống) |
| 4 | closure `\|m, t\| m.transition_tcb(t, Ready)` | — (đã ĐỎ đúng) | — | — |
| 5a | generic `F: Fn(…)` | `KT-239 wb17-f5a-generic` ×2 | `[]` | XANH (sống) |
| 5b | trait object `&dyn Fn(…)` | `KT-239 wb17-f5b-trait-object` ×2 | `[]` | XANH (sống) |
| **6** | **UFCS** `M::transition_tcb(m, t, Ready)` | `KT-239 wb17-f6-UFCS-doc-dung-args2` ×2 | `[("ready","TcbState::Ready")]` | **ĐỎ** (đã vá) |
| 7 | `let f = <Machine>::transition_tcb;` | `KT-239 wb17-f7-turbofish` ×2 | `[]` | XANH (sống) |

Đối chứng: `KT-239 doi-chung-am-thang-Ready` ×2 (ĐỎ, `site=(3,"am")`) ·
`KT-239 duong-cua-wake` ×2 · `KT-239 duong-block-current-tham-so` ×2 ·
`KT-239 duong-bien-the-khac` ×2 (gồm **UFCS `Blocked`** — chứng minh nhánh UFCS đọc `args[2]`
mà **không** biến mọi lời gọi UFCS thành ĐỎ).

## 6. Bằng chứng

Log trong [`kt239-logs/`](kt239-logs/):

| Tệp | Lệnh | Kết quả |
|---|---|---|
| `l1-selftest.log` | `python3 v2/tools/check-wake-paths.py --selftest` | **331/331 ok**, RC=0 (36 ca KT-239) |
| `l2-depinfo-selftest.log` | `python3 v2/tools/check-wake-depinfo.py --selftest` | **37/37 ok**, RC=0 — xem ghi chú §7 |
| `l1-strict.log` | `python3 v2/tools/check-wake-paths.py --strict` | `errors=0 warnings=0`, RC=0 |
| `l1-thuong.log` | `python3 v2/tools/check-wake-paths.py` | `errors=0 warnings=0`, RC=0 |
| `v2-check.log` | `./v2/bin/v2 check` | **PASS**, RC=0 |
| `mutation-mutA.log` | hoàn nguyên bản vá UFCS (`args[2]` → `args[1]`) | **327/331** — 4 ca ĐỎ: `wb17-f6-UFCS…` ×2 + `duong-bien-the-khac` ×2 |
| `mutation-mutB.log` | ca `wb16-f1` thành **rỗng nghĩa** | **329/331** — 2 ca ĐỎ (bị `need` bắt) |
| `mutation-mutC.log` | `site=(4,…)` → `(5,…)` | **329/331** — 2 ca ĐỎ (bị `site` bắt) |

Ba đột biến chứng minh các assert **có răng**: bản vá bị revert, ca rỗng, và "đúng mã nhưng sai
chỗ" đều **không** lọt. (Đột biến thứ tư — đổi tên tệp ca sang `ktest_evil.rs` — **không** làm
ca đỏ: `is_test_file` dựa **đồ thị module** từ KT-172, không dựa tên; ghi lại để người sau không
tưởng nhầm là lỗ hổng.)

## 7. Ghi chú / rủi ro còn lại

1. **Lớp 2 trên nhánh này là 37/37, không phải 52/52.** Luồng B (KT-238) đã nâng
   `check-wake-depinfo.py` lên **52/52** trên nhánh `night4/B-kt238` (đo lại: chạy tệp của B
   trong thư mục tạm cùng `check-wake-paths.py` ⇒ `# selftest: 52/52 ok`), nhưng nhánh đó
   **chưa merge** vào gốc `298b92c5`. Trên `night4/C-kt239`, tệp depinfo vẫn là bản `e09b634f`
   ⇒ **37/37**. Task cấm sửa tệp này và cấm merge — nên ghi nhận, không "sửa cho đủ số".
2. **12 dạng WB-16 + 6 biến thể WB-17 vẫn SỐNG** — đúng thực trạng, không phải hồi quy của
   KT-239. Đóng chúng là việc của luồng A (KT-237, ranh giới API). Ca "ghi nhận sống" ở đây là
   **đèn báo**: khi luồng A vá, 26 ca này sẽ ĐỎ và buộc đổi kỳ vọng — không thể vá im lặng.
3. **Không** nới ngưỡng, **không** allowlist: `scan_v2(..., allowlist={})` trong **mọi** ca
   KT-239 (allowlist rỗng ⇒ không có mục nào để "xin xanh").
4. **Không** chạy `gate --fast` (D2-192: gate độc quyền ở lead), **không** push.

## 8. Lệnh tái lập

```bash
cd /private/tmp/night4-C-kt239
export VIFAST_ROLE=lead; export PATH="$HOME/.cargo/bin:$PATH"
python3 v2/tools/check-wake-paths.py --selftest      # 331/331
python3 v2/tools/check-wake-depinfo.py --selftest    # 37/37 trên nhánh này (52/52 sau khi merge B)
python3 v2/tools/check-wake-paths.py --strict        # errors=0
python3 v2/tools/check-wake-paths.py                 # errors=0
./v2/bin/v2 check                                    # PASS
```
