# KT-241 — Review độc lập (E): phân loại hợp đồng, ma trận tấn công, điều kiện đóng

**Vai**: reviewer độc lập **E** (khác tác giả A/B/C/D; D2-190 §5, D2-192).
**Worktree**: `/private/tmp/kt241-E-audit`, nhánh `kt241/E-audit`, base `c5a83552`.
**Phạm vi đã ghi**: chỉ `v2/agent/JOURNAL/kt241-review.md` (tệp này) + `v2/agent/JOURNAL/kt241-attack/`.
**Không** sửa mã sản phẩm, checker, `v2/bin/v2`, `spec/**`, `QUEUE/STATE/DECISIONS`,
`CHECKER-WAKE-CONTRACT.md`. **Không** commit.

**Trạng thái lúc soạn**: API `NonReadyTarget` + `state_write` **chưa có** trong cây (A/B/C/D đang
chạy song song). Vì vậy ma trận **đã soạn + tự kiểm bằng mock**, **chưa chạy thật** trên cây;
`run.sh --dry-run` **PASS**, `run.sh --preflight` **FAIL 5 mục đúng dự kiến** (API chưa hợp nhất).
Không chạy tấn công thật khi API chưa có (sẽ chỉ ra lỗi biên dịch vô nghĩa).

---

## 0. TL;DR (có số)

| Hạng mục | Kết quả |
|---|---|
| 5 mục D2-194 §4 | 2 lệch → **1 A + 1 B**; 3 mục chưa nguồn → **2 C + 1 B** (N3 là lỗi trích dẫn — §1.4) |
| Ma trận | **27 ô** = 20 ô KT-240 §3 (A4 + B6 + C5 + D5) + **7 ô nhóm E** (ranh giới API mới) |
| Tệp tấn công | `cells/` có **67 tệp**: 33 tệp tấn công/phụ + **34 đối chứng dương**; `manifest.tsv` 27 dòng dữ liệu; `run.sh` 675 dòng |
| Tự kiểm | `--dry-run` **PASS**; `--preflight` **FAIL 5** (đúng dự kiến); `mock/check.sh` **18/18 PASS** (mã lỗi cargo của 18 ô core/core2) |
| Checklist 7 mục | **0/7 đo được trên cây tích hợp**; 3 mục đo được ngay sau hợp nhất (1, 2, 4); 4 mục cần ma trận/gate (3, 5, 6, 7) |
| Lệnh lead | `run.sh --preflight` → `run.sh --all --with-depinfo` → `run.sh --cell E7` → `./v2/bin/v2 gate --fast` (§4) |

**Kết luận một dòng**: bộ tấn công **sẵn sàng chạy**; hai việc **phải xử lý trước nghiệm thu** là
**L2** (lệch phạm vi bảo vệ giữa hai lớp) và **các ô E1/E2/E5** (luật checker mới cho `From`/
`TryFrom`/`write_state` — nếu C không thêm, ranh giới kiểu vẫn bị vòng qua ở mức hợp đồng).

---

## 1. Phân loại 2 lệch hợp đồng + 3 mục chưa nguồn (D2-194 §4)

Nguồn danh sách: [`kt240-audit-matrix.md`](kt240-audit-matrix.md) §2.2 (L1, L2, N1, N2, N3).
Nhóm: **A** = ảnh hưởng luật wake/phạm vi bảo vệ ⇒ xử lý **trước** nghiệm thu; **B** = thuần sổ
sách ⇒ sửa tài liệu hiện trạng; **C** = chưa có nguồn ⇒ ghi **"chưa được chứng minh"**, **không**
tính là đạt.

### 1.1 Bảng quyết định

| Mục | Nhóm | Lý do | Hành động | Ai làm | Đo bằng gì |
|---|---|---|---|---|---|
| **L1** — §6 dòng 217: "Cả ba chế độ chạy trong `./v2/bin/v2 check` và `gate --fast`" | **B** | Hành vi **đúng** (`v2/bin/v2:344–361` = 3 `run_step` + 2 selftest); chỉ sai **thuật ngữ** "ba chế độ" — D2-189 §3 chốt "ba lượt kiểm thuộc **hai cơ chế**". Không đổi luật wake/phạm vi. | Sửa **một câu** ở §6 dòng 217: "Ba lượt kiểm (thường, `--strict`, dep-info) thuộc **hai cơ chế** (lớp 1 tĩnh + lớp 2 dep-info) chạy trong `./v2/bin/v2 check` và `gate --fast`; xem D2-189 §3." Giữ nguyên §1–§5. | **C** (chủ `CHECKER-WAKE-CONTRACT.md`) | `grep -c "ba chế độ"` = 0; câu mới chứa "hai cơ chế"; `v2 check` không đổi (doc-only). |
| **L2** — §4 dòng 176 (`:expr`) + lệch mẫu phạm vi `in_test_only_wake_scope` (`paths.py:176`) vs `RE_CRATE_SRC` (`depinfo:145`) vs `MACRO_SCAN_ROOTS` (`paths.py:2030`) | **A** | Hai lớp định nghĩa **phạm vi khác nhau**: lớp 1 chỉ `v2/kernel|sim` (+ `v2/lib` macro), `MACRO_SCAN_ROOTS` **không** có `v2/tools`; lớp 2 `RE_CRATE_SRC` **có** `v2/tools`; hợp đồng §1 lại ghi `v2/tools/**` ngoài phạm vi. Đây là lệch **phạm vi bảo vệ**, không phải mô tả thuần; D2-188 chốt phạm vi từ `cargo metadata`, D2-194 §3 đòi checker bảo vệ "phạm vi metadata". | (i) **C** cập nhật §1 thành bảng phạm vi **theo từng lớp** (lớp 1: ba gốc + `build.rs`; lớp 2: `cargo metadata` + bao đóng path-dep; `v2/tools` chỉ "từ chối rõ" khi bao đóng **không** có crate bảo vệ — KT-238 §2.2); (ii) **B/KT-238** đồng nhất `RE_CRATE_SRC` với phạm vi metadata; (iii) thêm **selftest chéo** dựng fixture `v2/tools/x.rs` và assert hai lớp **cùng kết luận** (cùng ĐỎ hoặc cùng XANH kèm lý do); (iv) `:expr`: giữ là **giới hạn đã công bố**, thêm repro "gọi từ tệp ngoài `MACRO_SCAN_ROOTS`" hoặc hạ câu "chỉ còn hở khi…" thành "chưa đo hết". | **C** (hợp đồng + selftest lớp 1) + **B** (KT-238, lớp 2) + **lead** (quyết `v2/tools` trong/ngoài) | (a) selftest chéo mới trong `check-wake-paths.py --selftest` **và** `check-wake-depinfo.py --selftest`; (b) `grep` bảng §1 khớp hằng số code; (c) `v2 check` PASS. |
| **N1** — §1 dòng 25 (KT-209/L2): "đo 2026-11-20: **không** có token `wake`" ở `v2/kernel/vifast-plat-user/lib.rs` + `v2/kernel/vifast-kbin-riscv64/init/*.rs` | **C** | Không tìm thấy log đo trong `JOURNAL/kt209-logs/`; kỹ thuật đúng (hai đường ngoài mẫu quét) nhưng khẳng định "không có token" **chưa được chứng minh**. | Chạy `grep -n "wake"` trên đúng hai đường + `check-wake-paths.py --verbose` (chứng minh ngoài phạm vi), lưu `JOURNAL/kt241-logs/n1-grep.log` kèm commit + lệnh + thời điểm; trích log vào §1. **Chưa có log ⇒ ghi "chưa được chứng minh", không tính là đạt.** | **C** | Log tồn tại, có lệnh + output + commit; `grep -c` = 0; §1 dẫn đường dẫn log. |
| **N2** — §2 dòng 107–108: "`include!` phân giải theo **thư mục tệp chứa** (đúng rustc: `mod m { include!("x.rs") }` ⇒ `x.rs` cạnh tệp gốc)" | **C** | `kt212-attack-path-mod.md` §3 chỉ đo `#[path]`, **không** đo riêng `include!` trong inline module; hành vi code đúng hướng nhưng thiếu log đối chứng. | Dựng fixture rustc: `src/foo.rs` + `mod m { include!("x.rs") }` + `x.rs` ở `src/x.rs` (không phải `src/foo/x.rs`), `cargo check` RC=0 + checker; lặp cho tệp mod-rs; lưu `JOURNAL/kt241-logs/n2-include-inline.log`. **Chưa có log ⇒ "chưa được chứng minh".** | **C** | Log có lệnh rustc/cargo + RC + output checker; fixture chứng minh `x.rs` được nạp từ `src/`, không phải `src/foo/`. |
| **N3** — §4 dòng 183 (F3 `v2/lib`): "… §1 **đã ghi**" | **B** *(audit xếp N — xem §1.4)* | Nội dung kỹ thuật (không quét cấu trúc `v2/lib`) **đã có nguồn** (code + §1 thực tế chỉ ghi "thân macro + include/path"); lỗi là **trích dẫn vòng sai**: §1 **không** ghi trường hợp `cfg_attr` lồng ngoài macro. Đây là lỗi biên tập, không phải khẳng định kỹ thuật chưa đo — sửa bằng một bullet, không cần đo mới. | Thêm vào §1 bullet: "`v2/lib`: `cfg_attr` lồng **ngoài** thân macro **không** được quét cấu trúc ⇒ XANH có chủ đích (hệ quả phạm vi, không phải lỗi)"; khi đó câu "§1 đã ghi" ở §4 thành đúng. Hoặc sửa §4 thành "§1 chỉ ghi thân macro + include/path; trường hợp này là hệ quả, ghi bổ sung". | **C** | `grep "cfg_attr lồng"` trong §1 khớp; §4 dẫn đúng bullet §1; không cần log mới. |

**Tổng**: **A = 1** (L2), **B = 2** (L1, N3), **C = 2** (N1, N2). Không có mục nào được tính là
đạt khi chưa có nguồn.

### 1.2 Vì sao L2 là **A**, không phải B

L2 không chỉ là câu chữ: nó là **ba định nghĩa phạm vi khác nhau** đang cùng tồn tại —

- `in_test_only_wake_scope` (`check-wake-paths.py:176`): `^v2/(kernel|sim)/[^/]+/(src/.*\.rs|build\.rs)$`
  — **không** có `v2/lib`, **không** có `v2/tools`;
- `MACRO_SCAN_ROOTS` (`check-wake-paths.py:2030`): `("v2/lib", "v2/kernel", "v2/sim")` — **không**
  có `v2/tools`;
- `RE_CRATE_SRC` (`check-wake-depinfo.py:145`): `^v2/(?:lib|kernel|sim|tools)/[^/]+/src/` — **có**
  `v2/tools`.

Hợp đồng §1 lại ghi `v2/tools/**` **ngoài phạm vi**. Ba mẫu này đã từng gây lệch thật
(B3b/B12–B15, KT-224/227 — xem `kt240-audit-matrix.md` S7). D2-188 chốt phạm vi phải sinh từ
`cargo metadata`; D2-194 §3 đòi hai checker bảo vệ "nguồn sinh/crate mới/**phạm vi metadata**".
Vì vậy L2 **phải xử lý trước nghiệm thu**: nếu không, một tệp ở `v2/tools` (hoặc crate ngoài ba
gốc) có thể được một lớp bảo vệ, lớp kia bỏ qua — đúng lớp lỗi WB-18.

**Cách kiểm sau khi sửa** (cụ thể):
1. Thêm vào `check-wake-paths.py --selftest` một ca dựng fixture `v2/tools/x.rs` chứa `wake` và
   assert **cùng kết luận** với lớp 2 trên cùng fixture (ĐỎ cả hai, hoặc XANH cả hai kèm lý do
   "từ chối rõ").
2. Thêm vào `check-wake-depinfo.py --selftest` ca tương ứng (đã có 52 ca; thêm 1–2).
3. `grep` bảng §1 của hợp đồng với ba hằng số trên — mọi dòng phải khớp.
4. `./v2/bin/v2 check` PASS trên cây thật.

### 1.3 Vì sao L1 là **B**, không phải A

L1 chỉ là **thuật ngữ**: `v2/bin/v2` thực tế chạy 3 `run_step` (paths thường, `--strict`, dep-info)
+ 2 selftest; D2-189 §3 đính chính "ba lượt kiểm thuộc **hai cơ chế**", không phải ba lớp độc lập.
Không có hành vi nào thay đổi nếu sửa câu chữ. Tuy nhiên, **hệ quả nhận thức** của câu sai là
thật: người đọc tưởng ba lớp độc lập nên đánh giá thấp **mù chung** (S1/S2/S3 của KT-240). Vì vậy
sửa **ngay trong đợt này** (B), không để nợ.

### 1.4 Vì sao N3 là **B** (khác nhãn N của audit)

Audit KT-240 xếp N3 vào cột **N** (không nguồn) vì câu "§1 đã ghi" không có nguồn. Nhưng:
- **nội dung kỹ thuật** ("`v2/lib` không được quét cấu trúc ngoài thân macro") **đã có nguồn**:
  code `scan_structure`/`V2_SCAN` + §1 thực tế;
- cái **sai** là **trích dẫn**: §1 không ghi trường hợp `cfg_attr` lồng ngoài macro;
- sửa bằng **một bullet trong §1** (hoặc sửa câu §4) — **không cần đo mới**.

Vì vậy tôi phân loại **B**. Nếu lead muốn giữ đúng ánh xạ D2-194 ("chưa có nguồn ⇒ C") thì N3
rơi vào **C** với "bằng chứng cần" = **đoạn §1 phải chứa câu đó**; nhưng hành động vẫn là **sửa
§1** (B), không phải đo mới. Đây là điểm duy nhất tôi phân loại khác nhãn audit; ghi ở §5.

---

## 2. Ma trận tấn công — 27 ô

### 2.1 Tổng quan

- **20 ô theo KT-240 §3**: A4 + B6 + C5 + D5 = 20. ("4 nhóm × 5 ô" là cách nói tắt; **B có 6**
  vì WB-16 có 6 dạng — B1..B6.)
- **7 ô nhóm E** (mới, do API đổi): E1 `From`, E2 `TryFrom`, E3 `into_state`, E4 wrapper,
  E5 `write_state`, E6 `transmute`, E7 mutation anchor.
- Mỗi ô: **tệp tấn công** `.rs.txt` + **đối chứng dương**; `run.sh` chèn → `cargo check` →
  lớp 1 → lớp 1 `--strict` → (lớp 2) → **khôi phục** → đối chứng dương → **khôi phục**.
- **Tiêu chí sau thu hẹp API**:
  - ô **compiler chặn được** (A1–A4, B1–B5, E3, E4, E6): `cargo` **phải** từ chối **đúng mã**
    (`E0308`/`E0631`/`E0624`/`unsafe`); checker `any` (ĐỎ là phòng thủ nhiều lớp, XANH chấp nhận
    vì ranh giới kiểu đã chặn; nếu ĐỎ phải đúng mã);
  - ô **compiler KHÔNG chặn được, hoặc chỉ chặn một phần** (C1–C5, D1–D5, E1, E2, E5, E7):
    checker **phải** ĐỎ **đúng mã**. E5 là ca "chặn một phần": compiler chặn **gọi trực tiếp**
    `write_state` (E0624), nhưng nếu visibility bị nới thì phải có luật checker bảo vệ primitive;
  - **đối chứng dương** phải XANH (chứng minh ĐỎ không do tệp bị bỏ qua/báo oan).

### 2.2 Nhóm A — alias / UFCS / fn-pointer (4 ô)

| Ô | Dạng tấn công | Tệp sẽ chèn | Nội dung | Kỳ vọng | Đối chứng dương |
|---|---|---|---|---|---|
| **A1** | alias kiểu | `cells/A1.rs.txt` → `core/src/kt241_a1.rs` + `mod` | `use crate::tcb::TcbState as S; m.transition_tcb(t, S::Ready)` | cargo **E0308** (expected `NonReadyTarget`, found `TcbState`); checker any (nếu ĐỎ phải `E_STRICT_TRANSITION`) | `A1.positive.rs.txt`: `NonReadyTarget::Inactive` ⇒ cargo OK + l1/l1s XANH |
| **A2** | UFCS | `cells/A2.rs.txt` → `core/src/kt241_a2.rs` + `mod` | `Machine::transition_tcb(m,t,Ready)`; `<Machine>::transition_tcb(m,t,Ready)` | cargo **E0308** | `A2.positive`: UFCS với `NonReadyTarget` ⇒ XANH |
| **A3** | fn-pointer / generic / trait-object | `cells/A3.rs.txt` → `core/src/kt241_a3.rs` + `mod` | `let f = Machine::transition_tcb; f(m,t,Ready)`; `fn(..) -> TcbState`; `F: Fn(.., TcbState)`; `&dyn Fn(.., TcbState)` | cargo **E0308/E0631** | `A3.positive`: fn-pointer/generic/trait-object với `NonReadyTarget` ⇒ XANH |
| **A4** | `Self`/đường dẫn lạ | `cells/A4.rs.txt` → `core/src/kt241_a4.rs` + `mod` | `<S>::Ready`; `crate::tcb::TcbState::Ready` | cargo **E0308** | `A4.positive`: path-qualified `NonReadyTarget` ⇒ XANH |

### 2.3 Nhóm B — helper liên hàm / closure / const / static (6 ô)

| Ô | Dạng tấn công | Tệp sẽ chèn | Nội dung | Kỳ vọng | Đối chứng dương |
|---|---|---|---|---|---|
| **B1** | helper cùng tệp | `cells/B1.rs.txt` → `core/src/kt241_b1.rs` + `mod` | `fn helper(m,t,s: TcbState){ m.transition_tcb(t,s) }` + `helper(m,t,Ready)` | cargo **E0308** tại thân helper | `B1.positive`: helper nhận `NonReadyTarget` ⇒ XANH |
| **B2** | helper tệp khác | `B2.rs.txt` + `B2.helper.rs.txt` → `kt241_b2.rs` + `kt241_b2_helper.rs` + 2 `mod` | caller `crate::kt241_b2_helper::helper(m,t,Ready)`; helper nhận `TcbState` | cargo **E0308** | `B2.positive` + `B2.positive.helper` ⇒ XANH |
| **B3** | hàm trả về `Ready` | `cells/B3.rs.txt` → `core/src/kt241_b3.rs` + `mod` | `fn ready_state() -> TcbState { Ready }` + `m.transition_tcb(t, ready_state())` | cargo **E0308** | `B3.positive`: hàm trả `NonReadyTarget` ⇒ XANH |
| **B4** | `const`/`static` | `cells/B4.rs.txt` → `core/src/kt241_b4.rs` + `mod` | `const R: TcbState = Ready; static S: TcbState = Ready;` + truyền | cargo **E0308** | `B4.positive`: const/static `NonReadyTarget` ⇒ XANH |
| **B5** | closure | `cells/B5.rs.txt` → `core/src/kt241_b5.rs` + `mod` | `(|s: TcbState| m.transition_tcb(t,s))(Ready)` | cargo **E0308** | `B5.positive`: closure `NonReadyTarget` ⇒ XANH |
| **B6** | **đối chứng dương** | `cells/B6.rs.txt` → `core/src/kt241_b6.rs` + `mod` | `block_current(hart, to: NonReadyTarget)`; `transition_tcb(t, BlockedOnReply{..})`; `Sleeping{Budget}` | cargo OK + l1/l1s/l2 **XANH** | (chính là đối chứng dương) |

### 2.4 Nhóm C — macro / cfg / nguồn sinh (5 ô)

| Ô | Dạng tấn công | Tệp sẽ chèn | Nội dung | Kỳ vọng | Đối chứng dương |
|---|---|---|---|---|---|
| **C1** | `macro_rules!` che tên | `cells/C1.rs.txt` → `core/src/kt241_c1.rs` + `mod` | `macro_rules! matches { ($m,$x,$t) => { $m.$x($t) } }` + `let wake=0u32; matches!(m,wake,t)` | cargo OK; l1 **ĐỎ** `E_STRICT_TEST_ONLY_WAKE`; l2 **ĐỎ** `E_DEPINFO_WAKE` | `C1.positive`: `matches!` thật + `let wake` ⇒ XANH |
| **C2** | `include!` | `C2.rs.txt` + `C2.evil.rs.txt` → `kt241_c2.rs` + `kt241_c2_evil.rs` + `mod` | `include!("kt241_c2_evil.rs")`; tệp chứa `m.wake(t)` | cargo OK; l1/l2 **ĐỎ** | `C2.positive` + `C2.positive.ok` (include lành) ⇒ XANH |
| **C3** | `#[path]` non-mod-rs | `C3.rs.txt` + `C3.evil.rs.txt` → `kt241_c3.rs` + `evil.rs`; tạo `kt241_c3/inner/notes.txt` | `mod inner { #[path="../../../evil.rs"] mod baz; }` | cargo OK; l1 **ĐỎ** `E_STRICT_WAKE_INCLUDE`; l2 **ĐỎ** | `C3.positive` + `C3.positive.ok` (path lành trong `src/`) ⇒ XANH |
| **C4** | `build.rs` + `$OUT_DIR` | `C4.build.rs.txt` → `core/build.rs`; `lib.rs` += `include!(concat!(env!("OUT_DIR"), "/evil.rs"))` | build.rs ghi `evil.rs` chứa `m.wake(t)` | cargo OK; l1 **ĐỎ** `E_STRICT_WAKE_INCLUDE`; l2 **ĐỎ** `E_DEPINFO_WAKE` | `C4.positive` + `C4.positive.ok` (include literal lành) ⇒ XANH |
| **C5** | `#[cfg(any())] mod` | `C5.rs.txt` → `core/src/kt241_c5.rs`; `lib.rs` += `#[cfg(any())] mod kt241_c5;` | module chứa `m.wake(t)` **không** được biên dịch | cargo OK; l1 **ĐỎ** `E_STRICT_TEST_ONLY_WAKE`; l2 **XANH** (giới hạn: lớp 2 chỉ thấy tệp rustc nạp) | `C5.positive` (module lành) ⇒ XANH |

### 2.5 Nhóm D — crate mới / dependency mới / đường dẫn đặc biệt (5 ô)

| Ô | Dạng tấn công | Tệp sẽ chèn | Nội dung | Kỳ vọng | Đối chứng dương |
|---|---|---|---|---|---|
| **D1** | crate ngoài ba gốc | `D1.crate.{toml,build.rs,lib.rs}.txt` → `v2/extra/evil/`; `v2/Cargo.toml` += member | build.rs ghi `$OUT_DIR/evil.rs` chứa `m.wake(t)`; lib.rs include | cargo OK; l1 **XANH** (ngoài ba gốc); l2 **ĐỎ** `E_DEPINFO_WAKE` | `D1.positive.crate.*` → `v2/extra/ok` (mã sinh lành) ⇒ l2 XANH |
| **D2** | tên tệp có dấu cách | `D2.rs.txt` → `core/src/kt241 space.rs`; `lib.rs` += `#[path="kt241 space.rs"] mod kt241_space;` | file chứa `m.wake(t)` | cargo OK; l1/l2 **ĐỎ** | `D2.positive` + `D2.positive.ok` ⇒ XANH |
| **D3** | thư mục `target` | `D3.rs.txt` → `core/src/target/mod.rs`; `D3.evil.rs.txt` → `core/src/target/evil.rs`; `lib.rs` += `mod target;` | `src/target/evil.rs` chứa `m.wake(t)` | cargo OK; l1/l2 **ĐỎ** | `D3.positive` + `D3.positive.ok` ⇒ XANH |
| **D4** | dependency mới | `D4.rs.txt` → `vifast-agentctl/src/kt241_d4.rs`; `Cargo.toml` += `vifast-sim`; `lib.rs` += `mod` | crate công cụ ngoài ba gốc gọi `m.wake(t)` | cargo OK; l1 **XANH**; l2 **ĐỎ** (KT-238 nâng lên bảo vệ) | `D4.positive` (cùng dep, lành) ⇒ l2 XANH |
| **D5** | `#[path]` mức mục | `D5.rs.txt` → `core/src/kt241_d5.rs`; `D5.evil.rs.txt` → `core/evil.rs`; `lib.rs` += `mod` | `#[path="../evil.rs"] mod evil;` | cargo OK; l1 **ĐỎ** `E_STRICT_WAKE_INCLUDE`; l2 **ĐỎ** | `D5.positive` + `D5.positive.ok` ⇒ XANH |

### 2.6 Nhóm E — đường vòng qua chính ranh giới mới (7 ô)

| Ô | Dạng tấn công | Tệp sẽ chèn | Nội dung | Kỳ vọng | Đối chứng dương |
|---|---|---|---|---|---|
| **E1** | `From` | `E1.rs.txt` → `core/src/kt241_e1.rs` + `mod` | `impl From<TcbState> for NonReadyTarget` (Ready→Running) + `m.transition_tcb(t, Ready.into())` | cargo OK; l1 **PHẢI ĐỎ** (luật mới của C); l2 any | `E1.positive` (không `From`) ⇒ XANH |
| **E2** | `TryFrom` | `E2.rs.txt` → `core/src/kt241_e2.rs` + `mod` | `impl TryFrom<TcbState> for NonReadyTarget` + `try_from(Ready).unwrap()` | cargo OK; l1 **PHẢI ĐỎ**; l2 any | `E2.positive` ⇒ XANH |
| **E3** | `into_state` | `E3.rs.txt` → `core/src/kt241_e3.rs` + `mod` | `NonReadyTarget::Inactive.into_state()` từ module ngoài `state_write` | cargo **E0624**; checker any | `E3.positive` ⇒ XANH |
| **E4** | wrapper | `E4.rs.txt` → `core/src/kt241_e4.rs` + `mod` | `fn wrap(m,t,s: TcbState){ m.transition_tcb(t,s) }` | cargo **E0308** | `E4.positive` (wrapper `NonReadyTarget`) ⇒ XANH |
| **E5** | `write_state` | `E5.rs.txt` → `core/src/kt241_e5.rs` + `mod` | `m.write_state(t, TcbState::Ready)` từ module anh em | cargo **E0624**; l1 **PHẢI ĐỎ** (bảo vệ primitive — D2-194 §3) | `E5.positive` (cửa thường) ⇒ XANH |
| **E6** | `unsafe transmute` | `E6.rs.txt` → `core/src/kt241_e6.rs` + `mod` | `unsafe { transmute::<NonReadyTarget, TcbState>(Running{hart:0}) }` | cargo **unsafe** (`forbid(unsafe_code)`) | `E6.positive` (không unsafe) ⇒ XANH |
| **E7** | mutation anchor | không có tệp; `run.sh` sửa `state_write.rs` | `transition_tcb(... to: NonReadyTarget)` → `to: TcbState` | checker **PHẢI ĐỎ** (`E_STRICT_*`/`E_ANCHOR_MISSING`); cargo skip | checker trên cây sạch ⇒ XANH |

**Vì sao E1/E2/E5 là P0 tiềm năng**: ranh giới kiểu **không** chặn được `impl From<TcbState> for
NonReadyTarget` (orphan rule cho phép vì cả hai kiểu đều cục bộ) và **không** chặn được việc một
module sản phẩm thêm `impl`/gọi `write_state` nếu visibility bị nới. Compiler chỉ chặn **gọi trực
tiếp** `write_state`/`into_state` (E0624) và **truyền** `TcbState` vào `transition_tcb` (E0308).
Vì vậy C **phải** thêm luật checker cho `From`/`TryFrom`/`write_state` ngoài `state_write`; nếu
không, E1/E2/E5 sẽ FAIL và đó là **finding**, không phải lỗi của ma trận.

### 2.7 Cách chạy + kết quả tự kiểm

```bash
A=v2/agent/JOURNAL/kt241-attack
$A/run.sh --dry-run          # PASS (logs/dry-run.log)
$A/run.sh --preflight        # FAIL 5 mục đúng dự kiến (API chưa hợp nhất)
$A/run.sh --list             # 27 ô
$A/mock/check.sh             # 18/18 PASS (mã lỗi cargo trên mock API ngoài cây)
```

- `logs/dry-run.log`: 168 dòng, in đủ 27 ô + kỳ vọng + tệp chèn + đối chứng dương.
- `logs/preflight.log`: 14 dòng, FAIL đúng 5 mục API (`state_write.rs`, re-export core, sim
  `NonReadyTarget`, `write_state`, `into_state`); PASS checker/toolchain.
- `logs/mock-validation.log`: 20 dòng, **PASS=18 FAIL=0** — xác nhận A1–A4/B1–B5/E3–E6 ra đúng
  `E0308`/`E0631`/`E0624`/`unsafe`, B6/C1/C2/E1/E2 ra RC=0.
- `logs/list.log`: bảng 27 ô.
- `logs/runsh-mechanics.log`: 47 dòng — kiểm **cơ chế** chèn/khôi phục trên **fake repo**
  `/tmp/kt241-fake` (mock API + checker stub): `--group A` **4/4 PASS**; **`--all` 27 ô:
  PASS=15 FAIL=12 ERROR=0** — 12 FAIL đúng là các ô cần checker ĐỎ (stub luôn XANH), **0 ERROR**;
  restore sạch (`v2/extra` xoá, `src/` không còn thư mục tạm, `Cargo.toml` không còn member/dep
  tạm). Log này **không** phải nghiệm thu — chỉ chứng minh `run.sh` không hỏng.

---

## 3. Checklist 7 điều kiện đóng (D2-190 §1–§7)

| # | Điều kiện (D2-190) | Đo bằng gì | Hiện trạng |
|---|---|---|---|
| **1** | **Hợp đồng bảo vệ rõ**: crate, target, feature, nguồn sinh, phần hỗ trợ và phần từ chối | `CHECKER-WAKE-CONTRACT.md` §1–§5 + danh sách phạm vi `cargo metadata` in ở đầu `check-wake-depinfo.py` + bảng crate/target/feature/nguồn sinh/từ chối | **Chưa đo — chờ hợp nhất.** Hợp đồng có nhưng **L2** (lệch phạm vi) + **N1/N2/N3** chưa xử lý; §1 chưa có bảng phạm vi theo lớp. |
| **2** | **#16–#18 được chặn**, gồm lỗi đọc sai vị trí đối số khi gọi UFCS | 6 dạng WB-16 + 7 biến thể WB-17 + WB-18; `cargo` **E0308/E0631** hoặc checker ĐỎ; selftest UFCS `args[2]` (KT-239) vẫn ĐỎ | **Chưa đo — API chưa có.** Ma trận phủ: B1–B6 (WB-16), A2/A3 (WB-17), D1/D4 (WB-18). Mock xác nhận mã lỗi cargo. |
| **3** | **Toàn bộ repro cũ được giữ bền** + đối chứng hợp lệ | `check-wake-paths.py --selftest` (KT-240: 295; KT-239 +36 ⇒ **331**), `check-wake-depinfo.py --selftest` (KT-240: 37; KT-238 +15 ⇒ **52**), 18 WB repro, 34 đối chứng dương của ma trận | **Chưa đo trên commit tích hợp.** Trên base: 295/37 (KT-240) + 331/52 sau KT-239/238 (theo journal). Ma trận có 34 đối chứng dương. |
| **4** | **Ranh giới API được compiler cưỡng chế**, không chỉ checker tự xác nhận | **13 ô compile-fail** (A1–A4, B1–B5, E3–E6) **đúng mã** (`E0308`/`E0631`/`E0624`/`unsafe`), **không** nhận lỗi khác; 18 ô đã tự kiểm bằng mock (13 compile-fail + B6/C1/C2/E1/E2 cargo OK) + đối chứng dương compile OK | **Chưa đo trên cây thật — API chưa có.** Mock: **18/18 PASS** đúng mã. Đây là mục **mạnh nhất** của thiết kế mới. |
| **5** | **Reviewer khác tác giả** tấn công theo 4 nhóm | Ma trận 20 ô + 7 ô nhóm E, chạy bởi **E** (không phải A/B/C/D), có bảng PASS/FAIL + log từng ô | **Đã soạn, chưa chạy** (API chưa có). `run.sh --all --with-depinfo` là phép đo. |
| **6** | **Gate trên commit tích hợp cuối PASS**, có log + danh tính công cụ | `./v2/bin/v2 gate --fast` trên commit cuối; log có commit + toolchain `1.99.1` + thời điểm; `git status` sạch | **Chưa đo.** D2-194 §5 ghi gate PASS tại `a2e18cdf`; HEAD sau đó chỉ đổi tài liệu — **không** phải gate trên commit tích hợp KT-241. |
| **7** | **Không còn đường lọt đã biết trong hợp đồng** | `BYPASS-REGISTRY.md` WB-01..18: WB-16/17/18 phải **CLOSED** (repro ĐỎ + đối chứng); hợp đồng §4 không còn P0; E1/E2/E5/E7 **ĐỎ** | **Chưa đo.** WB-16/17 **OPEN**, WB-18 **CLOSED** (KT-238). E1/E2/E5 là **đường vòng mới** của ranh giới API — chưa có luật checker cho tới khi C thêm. |

**Tóm tắt**: 0/7 đo được trên cây tích hợp; 3 mục (1, 2, 4) đo được **ngay sau hợp nhất** bằng
`run.sh --all --with-depinfo`; 3 mục (3, 5, 7) đo được bằng ma trận + selftest; 1 mục (6) cần
`gate --fast` trên commit cuối.

---

## 4. Lệnh **chính xác** lead phải chạy sau hợp nhất

```bash
cd /private/tmp/kt241-E-audit          # hoặc worktree tích hợp cuối
A=v2/agent/JOURNAL/kt241-attack

# 0. API đã hợp nhất chưa? (exit 0 = đủ; exit 2 = còn thiếu)
$A/run.sh --preflight

# 1. Nền phải XANH — nếu ĐỎ, DỪNG (ma trận vô nghĩa trên nền đỏ)
python3 v2/tools/check-wake-paths.py --root .
python3 v2/tools/check-wake-paths.py --root . --strict
python3 v2/tools/check-wake-depinfo.py --root .

# 2. Toàn ma trận: 27 ô + 27 bước đối chứng dương (34 tệp), gồm lớp 2 (~40–60 phút)
$A/run.sh --all --with-depinfo

# 3. Nếu cần chạy từng nhóm / ô (khi ô nào FAIL)
$A/run.sh --group A --with-depinfo
$A/run.sh --group E
$A/run.sh --cell E7

# 4. Gate cuối trên commit tích hợp (ghi commit + toolchain + thời điểm)
./v2/bin/v2 gate --fast
```

**Kỳ vọng**: `TỔNG: 27 ô | PASS=27 FAIL=0 ERROR=0`. Nếu FAIL, xem `logs/<ID>/` (cargo/l1/l1s/l2/
positive-*). Nếu `ERROR`, xem `setup.err` / `positive-setup.err`.

**Lưu ý vận hành**:
- **Không chạy song song** với `gate --fast`/`nightly` (D2-192: đo nặng độc quyền).
- `run.sh` **tự khôi phục** bằng trap (`EXIT/INT/TERM`); nếu bị **SIGKILL**, cây có thể còn tệp
  tạm — kiểm `git status` và xoá theo `logs/<ID>/` + thư mục backup in ra ở đầu ra.
- `--keep` chỉ dùng khi debug **một ô** (`--cell`), không dùng với `--all`.
- Sau khi C chốt mã checker cho E1/E2/E5/E7, **cập nhật** cột `expect_codes_checker` trong
  `manifest.tsv` (hiện là tạm định) — nếu không, các ô đó FAIL vì **sai mã**, không phải lỗ hổng.

---

## 5. Điều tôi **không** làm được / còn nghi ngờ

1. **Không chạy tấn công thật** trên cây: API `NonReadyTarget`/`state_write` chưa có lúc soạn
   (đúng yêu cầu). Bằng chứng hiện có: `--dry-run` PASS + `mock/check.sh` 18/18 PASS (mock API
   **ngoài cây**, không phải nghiệm thu).
2. **Mã checker của E1/E2/E5/E7 là tạm định** (`E_STRICT_TRANSITION|E_STRICT_NONREADY|...`).
   C có thể dùng mã khác; khi đó cập nhật `manifest.tsv`. Đây là **điểm mềm** lớn nhất của ma
   trận — nhưng nó **không** che lỗ hổng: nếu checker XANH, ô vẫn FAIL (vì `expect_l1=red`).
3. **N3 phân loại B, khác nhãn N của audit** — lý do ở §1.4. Nếu lead giữ ánh xạ D2-194
   ("chưa nguồn ⇒ C") thì đổi nhãn N3 thành C; hành động vẫn là sửa §1.
4. **A1–A4/B1–B5 dùng checker `any`** (không bắt buộc ĐỎ) vì compiler đã chặn. Nếu lead muốn
   **cả hai** cùng ĐỎ (phòng thủ nhiều lớp), đổi `expect_l1/l1s` thành `red` — nhưng khi đó các ô
   này sẽ FAIL nếu C bỏ luật cũ theo D2-194 §3. Đây là **quyết định chính sách**, không phải lỗi
   kỹ thuật; tôi chọn `any` vì D2-194 §5 nói "compiler/checker từ chối đúng nguyên nhân".
5. **C5 lớp 2 XANH** là giới hạn đã biết (lớp 2 chỉ thấy tệp rustc nạp; `#[cfg(any())]` không
   được biên dịch). Không phải hồi quy; đừng "sửa" bằng cách bắt lớp 2 quét mọi nhánh cfg.
6. **D1/D4 lớp 1 XANH** là đúng phạm vi ba gốc hiện hành; nếu hợp nhất đưa lớp 1 sang
   `cargo metadata`, đổi `expect_l1` thành `red` và cập nhật manifest.
7. **E7 regex** giả định chữ ký `pub fn transition_tcb(... to: NonReadyTarget)` trong
   `state_write.rs`; nếu A đặt `transition_tcb` ở tệp khác hoặc chữ ký khác, `run.sh` báo `ERROR`
   (không phải FAIL) — cập nhật `setup_mutate` theo vị trí thật.
8. **Chưa kiểm** `v2/agent/JOURNAL/kt241-attack/` có bị `cargo metadata` nhặt không: thư mục
   không có `Cargo.toml` (mock nằm ở `/tmp`), nên không. Nếu lead muốn giữ mock trong repo, phải
   đặt ngoài `v2/` hoặc thêm `exclude`.
9. **Không sửa** `CHECKER-WAKE-CONTRACT.md` (quyền C) — các đề xuất L1/L2/N1/N2/N3 ở §1 chỉ là
   **phiếu**; C/lead phải thực thi.

---

## 6. Bằng chứng trong worktree

| Tệp | Nội dung |
|---|---|
| [`kt241-attack/README.md`](kt241-attack/README.md) | bố cục, cách chạy, 27 ô, tiêu chí, giới hạn |
| [`kt241-attack/manifest.tsv`](kt241-attack/manifest.tsv) | 27 dòng dữ liệu, 12 cột |
| [`kt241-attack/run.sh`](kt241-attack/run.sh) | 675 dòng; `--dry-run/--preflight/--list/--all/--group/--cell/--keep` |
| [`kt241-attack/cells/`](kt241-attack/cells/) | 67 tệp: 33 tấn công/phụ + 34 đối chứng dương |
| [`kt241-attack/mock/check.sh`](kt241-attack/mock/check.sh) | tự kiểm mã lỗi cargo trên mock API ngoài cây |
| [`kt241-attack/logs/dry-run.log`](kt241-attack/logs/dry-run.log) | 168 dòng, kế hoạch 27 ô |
| [`kt241-attack/logs/preflight.log`](kt241-attack/logs/preflight.log) | FAIL 5 mục API đúng dự kiến |
| [`kt241-attack/logs/mock-validation.log`](kt241-attack/logs/mock-validation.log) | PASS=18 FAIL=0 |
| [`kt241-attack/logs/list.log`](kt241-attack/logs/list.log) | bảng 27 ô |
| [`kt241-attack/logs/runsh-mechanics.log`](kt241-attack/logs/runsh-mechanics.log) | kiểm cơ chế chèn/khôi phục trên fake repo |

**Trạng thái git**: chỉ thêm `v2/agent/JOURNAL/kt241-review.md` + `v2/agent/JOURNAL/kt241-attack/`;
không sửa tệp nào khác; **không commit**.
