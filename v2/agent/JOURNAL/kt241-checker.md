# KT-241 — luồng C: sửa **hai checker** cho khớp API `NonReadyTarget` (D2-194 §3)

**Vai**: sub-agent C (checker). **Lead**: `lead`. **Nhánh**: `kt241/C-checker`.
**Worktree**: `/private/tmp/kt241-C-checker`. **Base**: `c5a83552`.
**Thời gian**: 2026-11-20 ~11:37–12:20 JST.
**Nguồn đặc tả**: `v2/agent/JOURNAL/kt241-spec/API-SPEC.md` (nguồn **duy nhất** cho tên
tệp/hàm/điều kiện), `kt240-audit-matrix.md`, `kt237-api-inventory.md` §4.4.

**Kết luận một dòng**: hai checker đã đổi từ "suy luận đối số thứ hai có phải `Ready`"
(allowlist + dataflow-lite) sang **mỏ neo kiểu + luật `write_state` + luật cấm phép chuyển
`TcbState` ⇄ `NonReadyTarget`**; selftest lớp 1 **331 → 384**, lớp 2 **52 → 59**, cả hai
**RC=0**; cây thật **ĐỎ có chủ đích** (API chưa đổi — A/B đang làm), cây fixture hậu hợp nhất
**XANH** cả hai lớp/chế độ.

**Mã lỗi chốt cho reviewer E (cập nhật `manifest.tsv`)**: `E_STRICT_NONREADY_CONV` (E1/E2),
`E_ANCHOR_API` (E5), `E_ANCHOR_TYPE` (E7 + biến thể `Ready` + cửa cũ), `E_STRICT_STATE_WRITE`
(luật `write_state`), `E_STRICT_TRANSITION` (chốt chống đảo kiểu). Lớp 2: `E_DEPINFO_NONREADY_CONV`,
`E_DEPINFO_STATE_WRITE`, `E_DEPINFO_TRANSITION`.

---

## 1. Tệp đã sửa

| Tệp | Thêm | Bớt | Nội dung |
|---|---|---|---|
| `v2/tools/check-wake-paths.py` | ~1.500 | ~330 | hằng số `state_write`, `_transition_calls` viết lại, xoá `_var_may_be_ready`, thêm `_walk_nodes`/`_fn_defs`/`_fn_visibility`/`_param_types`/`_type_has`/`_find_enum`/`_check_type_anchors`/`_check_nonready_conv(_file)`/`_check_state_write`, luật 5a/5b, fixture `state_write.rs`, 53 ca selftest mới/đổi |
| `v2/tools/check-wake-depinfo.py` | ~330 | ~90 | R2 viết lại (cùng `_transition_calls`), **R3 mới** (`E_DEPINFO_STATE_WRITE`, dùng chung `_check_state_write`), **R4 mới** (`E_DEPINFO_NONREADY_CONV`, dùng chung `_check_nonready_conv_file`), fixture `state_write.rs`, ca (5)(6)(14)(15)(16)(16b–h)(36)(37) cập nhật |
| `v2/agent/JOURNAL/kt241-logs/` | 2 log | — | `n1-wake-ngoai-mau.log`, `n2-include-inline-module.log` (đo lại N1/N2 của KT-240) |
| `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` | ~140 | ~20 | §1 phạm vi, **§2b hợp đồng KT-241**, §3 mã lỗi mới, §4 mục KT-241, **§4.1 phân loại 2 lệch + 3 mục chưa nguồn của KT-240**, §6 thuật ngữ "ba lượt / hai cơ chế", §7 bảo trì |
| `v2/agent/JOURNAL/kt241-checker.md` | mới | — | tệp này |

**Ghi chú đường dẫn**: đề bài ghi `v2/docs/CHECKER-WAKE-CONTRACT.md`; **tệp đó không tồn tại**
trong repo. Hợp đồng thật nằm ở `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` (đã kiểm bằng
`find`). Đã sửa **tệp thật**; **không** tạo bản sao ở `v2/docs/` (tránh hai nguồn sự thật).

---

## 2. Thiết kế luật mới — bảng **cũ → mới**

| # | Luật cũ | Luật mới (KT-241) | Vì sao |
|---|---|---|---|
| 1 | `E_STRICT_TRANSITION`: mọi `transition_tcb(…, TcbState::Ready)` phải nằm trong cửa wake (`WAKE_DOORS` = `wake_step`/`wake_from`/`wake_from_unlink` ở `core_sched.rs`/`kobj.rs`) hoặc có mục trong `NON_WAKE_READY_SITES` (15 mục, khoá `(tệp, hàm, thứ tự)`) | `E_STRICT_TRANSITION`: mọi lời gọi `transition_tcb(…)` bị **ĐỎ** nếu đối số thứ hai là/chứa `TcbState` **hoặc** kết thúc bằng/chứa `…::Ready` (alias `S::Ready`, `Self::Ready`, `NonReadyTarget::Ready`). Không cửa, không allowlist, không thứ tự. | Dưới API `to: NonReadyTarget` (enum **không có** `Ready`), đối số thứ hai **không thể** là `Ready` ⇒ câu hỏi "có phải Ready không" **không còn nghĩa**. Luật còn lại là **chốt chống đảo kiểu**: dạng `TcbState::*`/`…::Ready` **không biên dịch được** ⇒ có người đã đảo chữ ký. |
| 2 | `NON_WAKE_READY_SITES` (15 mục allowlist) | **XOÁ** | Allowlist chỉ tồn tại để miễn những chỗ `transition_tcb(…, Ready)` **hợp lệ ngoài cửa wake**. Dưới API mới **không còn chỗ nào như vậy** (Ready chỉ vào qua 5 cửa trong `state_write.rs`). Giữ allowlist rỗng/cũ = **bề mặt tấn công** (mỗi mục là một chỗ được miễn). |
| 3 | `_var_may_be_ready(text, fn, var)` — dataflow-lite: ĐỎ nếu thân hàm gán `var = … TcbState::Ready` | **XOÁ** (xem §3) | Câu hỏi "biến này có thể là Ready không" không còn nghĩa; giữ lại là mô hình **không đầy đủ** (không mô hình hoá call/const/closure/liên hàm) làm người đọc tưởng checker chứng minh nhiều hơn thực tế. |
| 4 | (không có) | **`E_STRICT_STATE_WRITE`**: `write_state`/`into_state` **chỉ** được xuất hiện trong `state_write.rs`; trong đó `write_state` **chỉ** được **gọi** bởi `STATE_WRITE_CALLERS` = `transition_tcb` (cửa 0) + `wake_step` + `preempt_current` + `yield_to_ready` + `start_tcb` + `suspend_to_ready`; dùng làm **giá trị** ⇒ ĐỎ. `tcb_yield` **không** được gọi trực tiếp (phải qua `yield_to_ready`). | **Luật thay thế trực tiếp** cho allowlist: primitive ghi thô là private, nên "chỗ chuyển Ready hợp lệ" bị **loại bỏ về mặt cấu trúc**, không cần miễn. `tcb_yield` là ca cụ thể: gọi thẳng `write_state` là **bỏ kiểm điều kiện nguồn**. |
| 5 | (không có) | **Mỏ neo kiểu** `E_ANCHOR_MISSING`/`E_ANCHOR_TYPE` — xem §4 | D2-194 §3: "đơn giản hoá theo kiểu, **không tin tên hàm**" **nhưng** không được thay bằng "thấy `transition_tcb(` là an toàn". Nếu ai đó **đảo kiểu**, đường lọt cũ sống dậy mà checker vẫn XANH ⇒ phải đọc **định nghĩa** và **ĐỎ khi thiếu/sai**. |
| 6 | `WAKE_DOORS` = `{(core_sched.rs, wake_step), (core_sched.rs, wake_from), …}` | `WAKE_DOORS` = `{(state_write.rs, <6 hàm ở STATE_WRITE_CALLERS>)}` cho **cả hai** crate | `wake_step` **rời** `core_sched.rs`/`kobj.rs` sang `state_write.rs` (API-SPEC §3). Khoá theo **tệp `state_write.rs`** để hàm trùng tên ở tệp khác không lọt. Tên hằng giữ nguyên vì lớp 2 `import`. |
| 7 | Mỏ neo `KERNEL_SCHED: ("fn wake(", ("fn wake_step(",))`, `SIM_KOBJ` như trên | `KERNEL_SCHED: ("fn wake(", ())`, `SIM_KOBJ: ("fn wake(", ())`; mỏ neo cửa chuyển sang `_check_type_anchors` | Giữ `"fn wake_step("` ở tệp cũ sẽ **ĐỎ oan** sau khi A/B hợp nhất (lead đã cảnh báo). |
| 8 | `_check_wake_text(relp, text, tr, safe_macros, r)` — `tr` **chết** | `_check_wake_text(relp, text, safe_macros, r)` — **bỏ** `tr` khỏi chữ ký + **cả 4** call site (lớp 1 ×3, lớp 2 ×1) | Xem §5. |
| 9 | `scan_v2(root, strict, allowlist=…)` | `scan_v2(root, strict, extra_doors=…)` — `extra_doors` là tập `(tệp, hàm)` **thêm** (chỉ selftest dùng); mục không khớp ⇒ `E_STRICT_STALE` | Allowlist cũ (3-tuple + lý do) không còn đối tượng. Giữ `E_STRICT_STALE` cho `extra_doors` để không xoá luật "mục allowlist mục ⇒ ĐỎ". |
| 10 | (không có) | **`E_ANCHOR_API`** (E5): `write_state`/`into_state` phải **private** — `pub`, `pub(crate)`, `pub(super)`, `pub(in …)` ⇒ ĐỎ | Reviewer E: nới visibility ⇒ `cargo check` RC=0 ⇒ mọi module trong crate gọi được primitive thô. Tách khỏi `E_ANCHOR_TYPE` để ma trận E ánh xạ đúng ô. |
| 11 | (không có) | **`E_STRICT_NONREADY_CONV`** (E1/E2): cấm mọi phép chuyển `TcbState` ⇄ `NonReadyTarget` ngoài `into_state` private — `impl From`/`TryFrom`/`Into`, inherent impl `NonReadyTarget { fn …(TcbState) -> Self }`, inherent impl `TcbState { fn … -> NonReadyTarget }`, `pub fn` nhận `TcbState` trả `NonReadyTarget`. Phạm vi `v2/kernel` + `v2/sim` + **`v2/lib`**. | Reviewer E: orphan rule **cho phép** impl **bên trong** crate ⇒ `cargo check` RC=0 ⇒ `NonReadyTarget::from(TcbState::Ready)` biên dịch được ⇒ đường vòng tái tạo qua ranh giới kiểu mà **không** đụng `transition_tcb`. API-SPEC §1 cấm tường minh. |

**Giữ nguyên (không xoá)**: token `wake` (`E_STRICT_TEST_ONLY_WAKE`), `include!`/`#[path]`/`mod`/
macro (`E_STRICT_WAKE_INCLUDE`/`E_STRICT_MACRO_*`), phạm vi `cargo metadata` (KT-238), `MailboxMsg`,
`unsafe`, luật arch, `E_LEX_*`, `E_ANCHOR_MISSING` cho mailbox/`fn wake`/`SYNC_TEST`.

---

## 3. Vì sao **bỏ** `_var_may_be_ready` (và cả phân tích đối số thứ hai)

`_var_may_be_ready` ra đời ở KT-231 để trả lời: *"biến `var` (đối số thứ hai của
`transition_tcb`) có được gán từ `TcbState::Ready` trong thân hàm không?"* — cần vì
`transition_tcb` nhận `TcbState`, nên một biến **có thể** là `Ready`; nếu coi **mọi** biến là ĐỎ
thì báo oan `block_current(cur, to)` (hợp lệ).

Dưới API KT-241, `to: NonReadyTarget` — enum **10 biến thể, không có `Ready`**. Câu hỏi trên
**không còn nghĩa**: một biến kiểu `NonReadyTarget` **không thể** là `Ready`, và một biến kiểu
`TcbState` truyền vào **không biên dịch được** (`E0308`). Giữ `_var_may_be_ready` sẽ:

1. **Duy trì một mô hình không đầy đủ** — nó không mô hình hoá call/const/static/closure/liên hàm
   (đúng 6 dạng WB-16 còn "sống" ở KT-234), nên nó **không** phải là bảo đảm; người đọc dễ tưởng
   checker chứng minh được nhiều hơn thực tế.
2. **Che mất bảo đảm thật** — bảo đảm thật là **kiểu** + **mỏ neo**. Nếu ai đó đảo chữ ký,
   `_var_may_be_ready` vẫn XANH (vì thân hàm không gán `Ready`) trong khi đường lọt sống dậy.
3. **Tạo báo oan mới** — mọi biến `NonReadyTarget` (đường hợp lệ duy nhất) đều là "var"; chỉ cần
   một phép gán `Ready` ở đâu đó trong hàm là ĐỎ oan.

Vì vậy: **bỏ** `_var_may_be_ready`, **bỏ** phân tích "Ready/không-Ready" của đối số thứ hai; thay
bằng **một** kiểm tối thiểu (`TcbState::*`/`…::Ready` ⇒ ĐỎ, chốt chống đảo kiểu) + **mỏ neo kiểu**
(§4) + **luật `write_state`** (§2 #4). Đây đúng D2-194 §3: *đơn giản hoá theo kiểu, không tin tên
hàm* — nhưng **không** thay bằng "thấy `transition_tcb(` là an toàn".

---

## 4. Mỏ neo kiểu — `_check_type_anchors` (fail-closed)

Đọc **định nghĩa** trong `state_write.rs` của **cả hai** crate (core + sim). Thiếu ⇒
`E_ANCHOR_MISSING`; sai ⇒ `E_ANCHOR_TYPE`.

| # | Mỏ neo | Cách kiểm |
|---|---|---|
| 1 | `state_write.rs` tồn tại | `Path.is_file()` |
| 2 | `enum NonReadyTarget` tồn tại, **không** biến thể `Ready` | `_find_enum` + duyệt con trực tiếp của thân enum tìm ident `Ready` |
| 3 | `transition_tcb` tham số thứ hai kiểu `NonReadyTarget` | `_fn_defs` + `_param_types` (bỏ `self`) + `_type_has` |
| 4 | `block_current` tham số thứ hai kiểu `NonReadyTarget` | như trên, tìm trong **mọi** tệp của crate (có thể ở `state_write.rs` hoặc `core_sched.rs`/`kobj.rs`) |
| 5 | `write_state`, `into_state` là `fn` **private** | `_fn_visibility` — `pub`, `pub(crate)`, `pub(super)`, `pub(in …)` đều là "pub" |
| 6 | `state_write.rs` có đủ 5 cửa `Ready` + `tcb_yield` + `transition_tcb` | `by_name` |
| 7 | `core_tcb.rs`/`kobj.rs` **không** còn `fn transition_tcb(` | `_fn_defs` trên tệp cũ |
| 8 | không `impl From/TryFrom/Into<TcbState> for NonReadyTarget`; không `pub fn` nhận `TcbState` trả `NonReadyTarget` | duyệt `impl` head + `_fn_defs` trên **mọi** tệp của crate |

**"Tham số thứ hai"** = tham số thứ hai **của lời gọi** (`transition_tcb(tcb, to)`), tức tham số
thứ ba của **method** (`&mut self, tcb, to`). `_param_types` **loại** `self`/`&self`/`&mut self`/
`self: …` khỏi danh sách — nếu không, mỏ neo đọc `tcb: ObjId` và **ĐỎ oan** mọi method (đã đo:
đây đúng là lỗi đầu tiên khi chạy thử, sửa bằng cách lọc `self`).

**Vì sao `transition_tcb` nằm trong `STATE_WRITE_CALLERS`** dù đề bài liệt kê "5 cửa": API-SPEC §3
hàng 0 định nghĩa `transition_tcb` là **cửa thông thường duy nhất** và §3.1 nói thân `write_state`
là thân `transition_tcb` cũ ⇒ `transition_tcb` **phải** gọi `write_state(tcb, to.into_state())`.
Nếu không cho nó vào allowlist thì **cây thật sau hợp nhất sẽ ĐỎ oan**. Vậy allowlist có **6** hàm
(1 cửa thông thường + 5 cửa `Ready`); `tcb_yield` **không** có mặt (đúng như đề bài lưu ý).

---

## 5. Tham số `tr` chết — đã bỏ

**Trước**: `_check_wake_text(relp, text, tr, safe_macros, r)` nhận `tr = test_line_ranges(text)`
nhưng **bỏ qua** và tự tính `_test_char_ranges(code, relp)` (token-tree). Caller (lớp 1 ×3, lớp 2
×1) tưởng mình điều khiển vùng test; thực tế **không**. Đây là "mồi lệch tương lai" KT-240 ghi.

**Sau**: bỏ `tr` khỏi **chữ ký** và khỏi **cả 4** call site. **Một** nơi sở hữu phân vùng:
`_check_wake_text` tự tính `_test_char_ranges`.

**Test phân vùng** (3 ca mới):
1. **Hai cách phân vùng khác nhau thật** — văn bản
   `#[cfg(test)] mod tests { fn h() {} }` (dòng 1) + `pub fn evil(m: &mut M, t: u32) { m.wake(t); }`
   (dòng 2):
   - `test_line_ranges` (theo **dòng**) trả `[(1, 2)]` ⇒ **phủ** dòng 2 ⇒ nếu caller điều khiển
     vùng test thì `m.wake(t)` được **miễn** ⇒ XANH (sai);
   - `_test_char_ranges` (theo **token-tree**) chỉ phủ mục `mod tests` ⇒ `m.wake(t)` là **sản
     phẩm** ⇒ ĐỎ (đúng).
   Ca khẳng định `line_covers=True`, `char_covers=False`, và kết quả `scan_v2` **ĐỎ**
   `E_STRICT_TEST_ONLY_WAKE` — tức kết quả theo cách của **callee**.
2. **Chữ ký** `_check_wake_text` có đúng 4 tham số `["relp", "text", "safe_macros", "r"]`
   (`inspect.signature`) — không còn `tr`.
3. **Call site**: quét **nguồn** của cả hai checker, đếm đúng **4** call site
   (`check-wake-paths.py` ×3 + `check-wake-depinfo.py` ×1) và **không** call site nào truyền
   `test_line_ranges` (bỏ dòng chú thích). Ca này bắt được cả trường hợp lớp 2 lệch lớp 1.

---

## 6. Selftest — số ca **cũ → mới**

| Checker | Cũ | Mới | RC |
|---|---|---|---|
| `check-wake-paths.py --selftest` | **331** | **384** (+53) | **0** |
| `check-wake-depinfo.py --selftest` | **52** | **59** (+7) | **0** |

### 6.1 Bảng ca cũ → mới (lớp 1)

| Nhóm ca cũ | Số | Kỳ vọng cũ | Ca mới | Kỳ vọng mới |
|---|---|---|---|---|
| (4) transition ngoài cửa wake | 1 | ĐỎ `E_STRICT_TRANSITION` | giữ nguyên văn bản | **ĐỎ** (đối số `TcbState::Ready` = đảo kiểu) |
| (4b) allowlist khớp ⇒ xanh | 1 | XANH | **thay**: `extra_doors` khớp cửa `dispatch_step` gọi `write_state` | XANH (chứng minh allowlist được **tư vấn**) |
| (4c) đột biến trong hàm allowlist | 1 | ĐỎ | **thay**: `tcb_yield` gọi thẳng `write_state` | ĐỎ `E_STRICT_STATE_WRITE` |
| (4d) allowlist mục | 1 | ĐỎ `E_STRICT_STALE` | **thay**: `extra_doors` mục | ĐỎ `E_STRICT_STALE` |
| (4e) transition nhiều dòng | 1 | ĐỎ | giữ nguyên | ĐỎ (token-tree vẫn đọc đúng) |
| KT-231 alias `S::Ready` ×2 chế độ | 2 | ĐỎ | giữ nguyên | **ĐỎ** (`ready_alias`) |
| KT-231 `Self::Ready` ×2 | 2 | ĐỎ | giữ nguyên | **ĐỎ** (`ready_alias`) |
| KT-231 biến gán từ `TcbState::Ready` ×2 | 2 | ĐỎ | **đổi kỳ vọng** | **XANH** (kiểu đã loại trừ) — **cặp** với ca mỏ neo dưới |
| KT-231 `TcbState::Blocked` ×2 | 2 | XANH | **đảo kỳ vọng** | **ĐỎ** (mọi `TcbState::*` = đảo kiểu) |
| KT-231 `TcbState::Ready` trong hàm từng allowlist ×2 | 2 | XANH | **đảo kỳ vọng** | **ĐỎ** (allowlist đã xoá; `extra_doors` không miễn transition) |
| KT-231 tham số `to` ×2 | 2 | XANH | đổi `to: TcbState` → `to: NonReadyTarget` | XANH |
| KT-231 path-qualified `vifast_core::TcbState::Ready` ×2 | 2 | ĐỎ | giữ nguyên | ĐỎ (`tcbstate`) |
| KT-239 WB-16 f1..f6 ×2 | 12 | "ghi nhận sống" (XANH) | **giữ văn bản**, kỳ vọng **XANH** (kiểu đã loại trừ) **+ ca cặp `-anchor`** (đảo chữ ký) | XANH / **ĐỎ `E_ANCHOR_TYPE`** |
| KT-239 WB-16 f2 đủ 2 tệp ×2 | 2 | XANH | giữ nguyên (phân loại `("ok","s")`) | XANH |
| KT-239 WB-17 f1/f2/f3/f5a/f5b/f7 ×2 | 12 | "ghi nhận sống" | **giữ văn bản**, XANH **+ ca cặp `-anchor`** | XANH / **ĐỎ `E_ANCHOR_TYPE`** |
| KT-239 WB-17 f6 UFCS ×2 | 2 | ĐỎ | giữ nguyên (phân loại `("tcbstate","TcbState::Ready")`) | ĐỎ |
| KT-239 đối chứng âm thẳng `Ready` ×2 | 2 | ĐỎ | giữ nguyên | ĐỎ |
| KT-239 đối chứng dương cửa wake / block_current / biến thể khác ×2 | 6 | XANH | **đổi văn bản**: cửa thông thường `NonReadyTarget::*`, `to: NonReadyTarget`, `NonReadyTarget::Stopping/Sleeping/Dead` | XANH |
| **MỚI** KT-241 mỏ neo cơ bản (6 ca) | 6 | — | thiếu tệp `state_write.rs`, `Ready` variant, `transition_tcb` sai kiểu, `block_current` sai kiểu, `core_tcb.rs` còn cửa cũ, thiếu `suspend_to_ready` | ĐỎ `E_ANCHOR_MISSING`/`E_ANCHOR_TYPE` |
| **MỚI** KT-241 luật `write_state` (4 ca) | 4 | — | ngoài `state_write.rs` (×2: `write_state`, `into_state`), `tcb_yield` gọi thẳng, dùng làm giá trị | ĐỎ `E_STRICT_STATE_WRITE` |
| **MỚI** KT-241 đối chứng dương (3 ca) | 3 | — | `state_write.rs` hợp lệ; `write_state` trong `start_tcb`; `into_state` trong `transition_tcb` | XANH |
| **MỚI** KT-241 cặp mỏ neo (1 ca) | 1 | — | biến gán từ `TcbState::Ready` + chữ ký bị đảo | ĐỎ `E_ANCHOR_TYPE` |
| **MỚI** KT-241 phân vùng `tr` (3 ca) | 3 | — | xem §5 | ĐỎ / đúng chữ ký / đúng call site |
| **MỚI** E1/E2 phép chuyển (7 ca) | 7 | — | `impl From`, `impl TryFrom`, `impl Into<NonReadyTarget> for TcbState`, inherent `NonReadyTarget { fn …(TcbState) -> Self }`, inherent `TcbState { fn … -> NonReadyTarget }`, `pub fn` nhận `TcbState` trả `NonReadyTarget`, và một ca trong **`v2/lib`** (phạm vi rộng) | ĐỎ `E_STRICT_NONREADY_CONV` |
| **MỚI** E5 visibility (4 ca) | 4 | — | `pub(crate) fn write_state`, `pub fn into_state`, `pub(super) fn write_state`, `pub(crate) fn into_state` | ĐỎ `E_ANCHOR_API` |
| **MỚI** E7 đột biến mỏ neo (1 ca) | 1 | — | `transition_tcb` về `TcbState` | ĐỎ `E_ANCHOR_TYPE` |

**Không ca nào bị xoá trắng**: 14 ca KT-231 và 36 ca KT-239 đều được **giữ văn bản** và **đổi
kỳ vọng sang luật mới**; 24 ca `-anchor` là **cặp** chứng minh mỏ neo thay dataflow-lite.

### 6.2 Bảng ca cũ → mới (lớp 2)

| Ca cũ | Kỳ vọng cũ | Ca mới | Kỳ vọng mới |
|---|---|---|---|
| (5) cây sạch | XANH, `scanned=3` | thêm `state_write.rs` vào `loaded` | XANH, `scanned=4` |
| (6) tệp producer vẫn quét | `scanned=3` | như trên | `scanned=4` |
| (14) `transition_tcb(…, Ready)` ngoài cửa | ĐỎ | giữ văn bản | ĐỎ (đảo kiểu) |
| (15) `wake_step` trong `WAKE_DOORS` | XANH | **thay**: `state_write.rs` hợp lệ (5 cửa gọi `write_state`) | XANH |
| (16) mục `NON_WAKE_READY_SITES` | XANH | **thay**: `NonReadyTarget::*`/biến `to` | XANH |
| — | — | **MỚI** (16b) `TcbState::Blocked` | ĐỎ |
| — | — | **MỚI** (16c) `write_state` ngoài `state_write.rs` | ĐỎ `E_DEPINFO_STATE_WRITE` |
| — | — | **MỚI** (16d) `into_state` ngoài `state_write.rs` | ĐỎ `E_DEPINFO_STATE_WRITE` |
| — | — | **MỚI** (16e) bản sao `state_write` ngoài đường dẫn chuẩn | ĐỎ (không miễn theo tên) |
| — | — | **MỚI** (16f) `write_state` trong `tcb_yield` | ĐỎ |
| — | — | **MỚI** (16g) `impl From<TcbState> for NonReadyTarget` | ĐỎ `E_DEPINFO_NONREADY_CONV` |
| — | — | **MỚI** (16h) `impl TryFrom<TcbState> for NonReadyTarget` | ĐỎ `E_DEPINFO_NONREADY_CONV` |
| (35) alias `S::Ready` | ĐỎ | giữ nguyên | ĐỎ |
| (36) `TcbState::Blocked` | XANH | **đảo kỳ vọng** | ĐỎ |
| (37) tham số `to` | XANH | đổi `to: TcbState` → `to: NonReadyTarget` | XANH |

---

## 7. Phân loại 2 lệch + 3 mục chưa nguồn của KT-240

Chi tiết đầy đủ ở hợp đồng **§4.1**. Tóm tắt:

| Mục | Loại | Xử lý |
|---|---|---|
| **L1** — "Cả ba chế độ" gây hiểu ba lớp độc lập | **thuần sổ sách** (thuật ngữ) | **Sửa tài liệu ngay**: §6 ghi "ba **lượt** kiểm thuộc **hai** cơ chế" (lớp tĩnh ×2 lượt + lớp dep-info ×1), đúng D2-189 §3 và `v2/bin/v2:344-361`. |
| **L2** — §4 `:expr` nói "ngoài `MACRO_SCAN_ROOTS`" nhưng §1 ghi `v2/tools` ngoài phạm vi trong khi depinfo `RE_CRATE_SRC` gồm `v2/tools` | **ảnh hưởng phạm vi bảo vệ** — **thuộc nhóm A** (reviewer E chỉ định), phải xử lý **trước nghiệm thu** | **Sửa tài liệu ngay trong đợt này**: §1 tách "ngoài phạm vi **tĩnh**" và ghi rõ lớp dep-info áp R1 cho **mọi** tệp rustc nạp + R2/R3/R4 cho **mọi** crate trong bao đóng `cargo metadata`, crate `v2/tools/` phụ thuộc crate bảo vệ được **nâng** lên bảo vệ (`_classify_scope`, KT-238). **Không đổi code** (code đã fail-closed). **Việc thống nhất ba mẫu phạm vi** (`in_test_only_wake_scope` vs `RE_CRATE_SRC` vs `MACRO_SCAN_ROOTS`) **là của nhóm A** — **không** tính là đóng. |
| **N1** — KT-209/L2 "đo 2026-11-20: không có token `wake`" ở `vifast-plat-user/lib.rs` + `init/*.rs` | **chưa có nguồn → đã đo lại** | **Đã đo** trên HEAD `c5a83552`: `grep` token `wake` trên `vifast-plat-user/lib.rs` + **25** tệp `init/*.rs` ⇒ **0 dòng khớp**; đối chứng trên `core_sched.rs` **có** khớp (lệnh đúng). Log: `kt241-logs/n1-wake-ngoai-mau.log`. **Còn phải đo lại trên HEAD cuối** sau hợp nhất. |
| **N2** — `include!` trong inline module giải theo thư mục tệp chứa | **chưa có nguồn → đã đo** | **Đã đo** bằng `rustc 1.100.0`: `src/foo.rs` = `mod inner { include!("x.rs"); }` — `src/x.rs` ⇒ **RC=0**; `src/foo/x.rs` ⇒ **RC=1** `couldn't find file src/x.rs` ⇒ giải theo **thư mục tệp chứa**, khớp khẳng định. Log: `kt241-logs/n2-include-inline-module.log`. **Đóng N2**. |
| **N3** — F3 "§1 đã ghi" | **chưa có nguồn** (tham chiếu vòng) | Ghi **"CHƯA ĐƯỢC CHỨNG MINH"** + sửa tham chiếu: §1 **không** ghi; đây là **hệ quả** của phạm vi `v2/lib`, không phải khẳng định đã đo. **Đã thêm bullet §1** nói rõ `cfg_attr` lồng ngoài thân macro trong `v2/lib` **không** được quét cấu trúc. |

---

## 7b. Đáp ứng reviewer E (3 lỗ hổng P0 + 1 đột biến mỏ neo)

E dựng 7 ô "đường vòng qua ranh giới mới" (nhóm E). Bốn ô liên quan checker:

| Ô | Kịch bản | Trước KT-241 (checker cũ) | Sau (checker này) | Mã |
|---|---|---|---|---|
| **E1** | `impl From<TcbState> for NonReadyTarget` **bên trong** `vifast-core` — orphan rule cho phép, `cargo check` RC=0 | **XANH** (không có luật) | **ĐỎ** | `E_STRICT_NONREADY_CONV` |
| **E2** | `impl TryFrom<TcbState> for NonReadyTarget` | **XANH** | **ĐỎ** | `E_STRICT_NONREADY_CONV` |
| **E5** | nới `write_state` thành `pub(crate)`/`pub`/`pub(super)` | **XANH** | **ĐỎ** | `E_ANCHOR_API` |
| **E7** | đột biến mỏ neo: `transition_tcb` về `TcbState` | **XANH** (không có mỏ neo) | **ĐỎ** | `E_ANCHOR_TYPE` |

**E1/E2 — luật `E_STRICT_NONREADY_CONV`** (mới, `_check_nonready_conv_file`): bắt **bốn** dạng
chuyển `TcbState` ⇄ `NonReadyTarget` ngoài `into_state` private — (1) trait impl `From`/`TryFrom`/
`Into` có cả hai kiểu trong đầu `impl`; (2) inherent impl `NonReadyTarget { fn …(TcbState) -> Self }`;
(3) inherent impl `TcbState { fn … -> NonReadyTarget }`; (4) `pub fn` nhận `TcbState` trả
`NonReadyTarget`. **Phạm vi rộng hơn luật `wake`**: `v2/kernel/**` + `v2/sim/**` + **`v2/lib/**`**
(E yêu cầu; `v2/lib` hiện không có hai kiểu này nên không báo oan). Lớp 2 áp **cùng** hàm, mã
`E_DEPINFO_NONREADY_CONV`.

**E5 — mỏ neo visibility `E_ANCHOR_API`** (tách khỏi `E_ANCHOR_TYPE` cho ma trận E): `write_state`
và `into_state` phải là `fn` **private**; `pub`, `pub(crate)`, `pub(super)`, `pub(in …)` ⇒ ĐỎ.
`_fn_visibility` xử lý cả modifier (`async`/`unsafe`/`const`/`extern`/`default`) chen giữa.

**E7 — mỏ neo chữ ký `E_ANCHOR_TYPE`**: `transition_tcb`/`block_current` tham số thứ hai phải là
`NonReadyTarget`; `NonReadyTarget` không có biến thể `Ready`; `core_tcb.rs`/`kobj.rs` không còn
`fn transition_tcb(`.

**Mã lỗi chốt (báo lead cập nhật `manifest.tsv`)**:

| Mã | Ô E | Điều kiện |
|---|---|---|
| `E_STRICT_NONREADY_CONV` | E1, E2 | phép chuyển `TcbState` ⇄ `NonReadyTarget` ngoài `into_state` private |
| `E_ANCHOR_API` | E5 | `write_state`/`into_state` không private |
| `E_ANCHOR_TYPE` | E7 (+ biến thể `Ready`, cửa cũ) | chữ ký sai kiểu / `Ready` trong enum / `core_tcb.rs`/`kobj.rs` còn cửa cũ |
| `E_STRICT_STATE_WRITE` | — | `write_state`/`into_state` ngoài `state_write.rs` hoặc sai cửa |
| `E_STRICT_TRANSITION` | — | `transition_tcb` nhận `TcbState::*`/`…::Ready` (chốt chống đảo kiểu) |
| `E_DEPINFO_NONREADY_CONV` / `E_DEPINFO_STATE_WRITE` / `E_DEPINFO_TRANSITION` | lớp 2 | tương ứng ba luật trên |

**Kiểm chứng E trên cây fixture hậu hợp nhất** (không sửa cây thật): 6/6 ca E1/E2 (gồm một ca
trong `v2/lib`), 2/2 ca E5 (`pub(super)`, `pub(crate)`), 1/1 ca E7 ⇒ **ĐỎ đúng mã**; cây fixture
sạch vẫn **XANH** cả hai lớp/chế độ.

**Phân loại lại theo E**: **L1** (sổ sách) và **N3** (thêm bullet §1) — **của luồng C**, đã làm;
**L2** (ba mẫu phạm vi `in_test_only_wake_scope` vs `RE_CRATE_SRC` vs `MACRO_SCAN_ROOTS`) —
**của nhóm A**, phải xử lý trước nghiệm thu (C chỉ sửa **tài liệu**, không đổi code);
**N1/N2** — đã **chạy đo thật + lưu log** (`kt241-logs/`), N2 **đóng**, N1 còn phải đo lại trên
HEAD cuối sau hợp nhất.

## 8. Kiểm chứng

### 8.1 Selftest (bắt buộc)

```bash
cd /private/tmp/kt241-C-checker
python3 v2/tools/check-wake-paths.py --selftest    # RC=0, 384/384
python3 v2/tools/check-wake-depinfo.py --selftest  # RC=0, 59/59
```

### 8.2 Cây thật (dự kiến ĐỎ — API chưa đổi)

`python3 v2/tools/check-wake-paths.py --strict` trên HEAD `c5a83552` **ĐỎ** với đúng ba nhóm lỗi
dự kiến: `E_ANCHOR_MISSING` (chưa có `state_write.rs`), `E_ANCHOR_TYPE` (`core_tcb.rs:389` /
`kobj.rs:5079` còn `fn transition_tcb`), `E_STRICT_TRANSITION` (mọi call-site còn `TcbState::*`).
**Đây là dự kiến** — A/B đang hiện thực API; không phải hồi quy của checker.

### 8.3 Cây fixture hậu hợp nhất (`/tmp/kt241-fixture`, **không** sửa cây thật)

Dựng cây mô phỏng API mới: `state_write.rs` (core + sim) đủ enum/cửa/`write_state` private;
`core_tcb.rs` chỉ còn `TcbState`; `core_sched.rs`/`kobj.rs` chỉ còn `wake`/`wake_from`/
`wake_from_unlink`/`apply_mail`/`push_step` + gọi cửa; `core_ipc.rs` gọi `transition_tcb` với
`NonReadyTarget::*`; `proc_rt.rs` gọi `start_tcb`/`tcb_yield`/`transition_tcb`.

| Kiểm | Kết quả |
|---|---|
| lớp 1 chế độ thường | **XANH** (`errors=0`) |
| lớp 1 `--strict` | **XANH** (`errors=0`) |
| lớp 2 (`scan` với `loaded` = mọi `.rs`) | **XANH** (`errors=0`, `scanned=11`) |
| 15 đột biến âm (write_state ngoài, into_state ngoài, `TcbState::Ready`, `S::Ready`, đảo chữ ký, `impl From`, `impl TryFrom`, `impl Into`, inherent `NonReadyTarget`, inherent `TcbState`, `write_state pub(crate)`, `write_state pub(super)`, `tcb_yield` gọi thẳng, `NonReadyTarget` có `Ready`, `core_tcb` còn cửa cũ, `block_current` sai kiểu) | **ĐỎ đúng mã** từng ca (lớp 1); các ca tương ứng ở lớp 2 **ĐỎ đúng mã** |

---

## 9. Điều **không** làm được / còn nghi ngờ

1. **Không chạy `v2 check`/`gate --fast` trên cây thật** — cây thật ĐỎ có chủ đích (API chưa
   đổi). Theo đề bài, trách nhiệm của luồng C là **selftest hai checker xanh**; cổng cuối thuộc
   lead sau khi A/B hợp nhất (D2-191 §3).
2. **Rủi ro báo oan sau hợp nhất** (đã đo trên fixture, nhưng fixture không phải cây thật):
   - **`write_state` gọi từ hàm thứ bảy** trong `state_write.rs` (ví dụ A/B tách helper
     `fn write_ready(&mut self, tcb)` rồi các cửa gọi nó). Theo API-SPEC §3 "**Không** tạo wrapper
     tổng quát nhận `TcbState` rồi gọi `write_state` (D2-194 §2)" ⇒ **ĐỎ là đúng**, nhưng nếu A/B
     làm khác đặc tả thì sẽ ĐỎ. Cần lead xác nhận với A/B.
   - **`into_state` trong `block_current`**: API-SPEC §3.2 nói `block_current` **không** được gọi
     `into_state`; luật của tôi **cho phép** (vì `into_state` không thể sinh `Ready`). Nếu A/B
     gọi, checker vẫn XANH — đây là **nới có chủ đích** so với câu chữ đặc tả (không phải lỗ
     hổng: `into_state` không sinh được `Ready`).
   - **`transition_tcb` nằm trong `STATE_WRITE_CALLERS`** (6 hàm, không phải 5): bắt buộc theo
     API-SPEC §3 hàng 0 + §3.1; nếu lead muốn đúng **5** thì phải đổi đặc tả (và cây thật sẽ ĐỎ).
   - **Test trong `#[cfg(test)]` của tệp sản phẩm**: luật `write_state` **miễn** theo
     `test_line_ranges` (cùng quy ước mọi luật khác). Test không thể gọi `write_state` (private)
     nên miễn là an toàn; nhưng nếu A/B để **compile-fail test** trong `src/` (không phải
     `tests/`) có nhắc `write_state`, nó sẽ **không** bị bắt (và cũng không cần bắt).
3. **`E_STRICT_STALE` chỉ còn cho `extra_doors`** (tham số selftest). Trên cây thật,
   `extra_doors=None` ⇒ luật này **không chạy**. Đây là **giảm** so với trước (allowlist 15 mục
   được kiểm mục mỗi lượt `--strict`), nhưng đúng bản chất: không còn allowlist sản phẩm.
4. **Mỏ neo đọc tệp trên đĩa, không đọc "tệp rustc nạp"**: nếu `state_write.rs` tồn tại nhưng
   **không** được `mod` (tệp chết), mỏ neo vẫn XANH. Đã cân nhắc thêm kiểm "`state_write.rs` phải
   nằm trong `loaded` của lớp 2" nhưng **không làm** (ngoài yêu cầu, và lớp 1 vẫn bắt được đảo
   kiểu qua `E_STRICT_TRANSITION` nếu call-site đổi về `TcbState`). Ghi nợ có tên.
5. **Đường dẫn hợp đồng trong đề bài sai** (`v2/docs/…` không tồn tại) — đã sửa tệp thật ở
   `v2/agent/reports/…`; lead nên xác nhận đây là tệp đúng.
6. **Không commit** (lead commit). `git status` chỉ có 3 tệp M + 1 tệp mới (journal này).
