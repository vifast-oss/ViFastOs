# Review thiết kế ViFastOs v2 + ViScript (trước khi giao Agent 2)

4 subagent song song + đối chiếu trực tiếp của người điều phối.
Nguồn: `review/src/ViFastOs-dossier.md` (v1, 2.220 dòng), `review/src/ViFastOs-v2-thiet-ke.md` (986 dòng), `review/src/ViScript-thiet-ke.md` (984 dòng).
Báo cáo chi tiết: `A-fidelity.md` (272 dòng), `B-viscript.md` (394), `C-handoff.md` (415), `D-technical.md` (765).
Quy ước: `V:` = dòng trong tài liệu v2; `S:` = dòng trong tài liệu ViScript; `D:` = dòng trong dossier; `dos:` như A dùng.

## 0. Phán quyết

**Hướng kiến trúc: ổn. Giao nguyên trạng: không.**

Bản v2 giữ được chất lượng dossier: mô hình seL4 nhất quán, mọi tuyên bố khó có nhãn `[ƯL]`/`[GT]`, cổng và tiêu chí xong đo được, và sửa đúng ba điểm yếu của v1 (con trỏ user, pool tĩnh, test bằng grep marker). ViScript là đặc tả định hướng tốt, không phải bản phác.

Nhưng có **7 lỗi chặn**: 4 ở dòng OS, 3 ở dòng ViScript. Ba lỗi là **lỗi cấu trúc**, không phải thiếu chi tiết:

1. **Job control / vòng đời tiến trình không biểu diễn được** trên máy trạng thái TCB và ABI hiện tại (không có Exit, không trạng thái "bị dừng", Suspend-khi-chờ chưa định nghĩa, Resume có thể phát lại `Call`).
2. **"ABI chốt ở K0" thực chất là danh sách tên**: không chữ ký, không quy ước thanh ghi, không version, thiếu `SetReplyObject`, `ReadState`/`Wait`, `Exit`; T04 một task 3 h cho ~80 entry point.
3. **Hai sản phẩm, một Agent 2, 3 h/task**: MCS đầy đủ + objmem/Miri + CSpace 2 cấp + 3 arch + Rust std + VFS + một ngôn ngữ mới (lexer 2 chế độ + typechecker + VM + LSP) — không khớp năng lực đã tuyên bố.

Riêng ViScript có thêm ba quyết định kiến trúc đang thiếu (mô hình parse chế độ lệnh, EBNF tự chứa, đặc tả từ vựng) nằm đúng trên đường tới hạn VT04–VT09, tức toàn bộ vòng 1.

Lời nhắn của agent0 nói "đã chốt luôn các câu hỏi còn mở" nhưng tài liệu không có bảng đối chiếu Q1–Q12; Q10 và Q11 không được trả lời ở đâu cả.

## 1. Lỗi chặn — dòng ViFastOs

### O1. Không có cơ chế kết thúc tiến trình / mã thoát (chặn T04)

- `V:387-406`: 14 syscall (0–13) **không có Exit**; `V:414`: `Tcb.*` không có Exit/Terminate/Wait; `V:222-235`: `Dead` chỉ đạt khi bị xoá.
- v1 **đã có** `SyscallId::Exit = 34` (`crates/vifast-abi/src/lib.rs:1171`) — v2 bỏ mà không có bảng ánh xạ.
- `V:510`: procsrv "theo dõi thoát; cấp `wait`, `kill`"; `V:578`: "procsrv có `kill/wait/list`"; `S:721`: ViShell `& jobs fg bg kill`.
- `V:696`: kịch bản test có bước `Exit` — đã giả định có cơ chế; `S:358`: `main` lỗi → mã thoát 1; `S:469`: mã thoát khác 0 thành lỗi pipeline.

**Sửa:** chọn một, ghi vào §4.21 + §6.4: (a) syscall 14 `Exit(code)` như v1 + procsrv bind Notification/fault_ep để nhận sự kiện thoát; (b) `Tcb.Exit` + `Tcb.BindExit`; (c) giao thức user-space `Call(procsrv.exit_ep)` + finalise, có IDL. Kèm đặc tả `Suspend` khi TCB đang chờ, và `wait` trả gì khi bị kill.

### O2. "Freeze ABI" chưa đủ để freeze (chặn T04)

`V:408-427` chỉ là bảng tên; thiếu chữ ký, quy ước thanh ghi, version, và các method sống còn:
- **Renumber v1→v2**: v1 `0=Send, 1=Call` (D:1140-1152); v2 `0=Call, 1=Send` — mọi tham chiếu số syscall cũ sai.
- **KError**: dossier 22 giá trị (D:1078-1104); v2 20 tên (`V:383`), gần như đổi hết tên.
- **Message**: v1 8 word + 1 cap (D:1121-1126); v2 64 MR + 4 cap (`V:256-258`) — thay đổi ABI lớn, không lý do/chi phí.
- **Thiếu method**: `VCpu` không có Configure (`V:424`), `IOSpace` không có Attach (`V:423`), `VSpace` không có tạo/khách (`V:418`) ⇒ H1/D3 không cài được sau K0.
- **Call/Reply**: `Call` trên object khác là invocation nhưng không nói **trả ngay** (`V:394`); TCB không có trường Reply object (`V:241`) và không có `SetReplyObject` ⇒ đường nóng IPC không đặc tả được (D F3).
- **Receive window** `V:218-220` "CNode + index + depth" trong IPC buffer: nếu là ObjRef là đường vòng quyền; phải là **CPTR**.
- T04 một task 3 h cho ~80 entry point, trong khi `V:979` nói "dùng thử ABI bằng kịch bản sim trước khi lên QEMU" mà T04 lại chặn T12 (`V:955,963`).

**Sửa:** freeze 5 thứ (số syscall, MessageInfo, mã lỗi, quy ước gọi, "ID không tái dùng") + `abi_version` + golden gen-check; bảng ánh xạ "58 v1 → 14 v2 + method" và "22 KError → 20"; tách T04a (bảng), T04b (chữ ký/thanh ghi), T04c (20 kịch bản sim **chỉ bằng ABI**), T04d (duyệt); T04c phải xong trước khi duyệt.

### O3. Queue §13 không chạy được (chặn vòng 1)

- **Không có task cho K1a/b/c**: `V:782-784` đòi finalise khung, `vifast-sync`, scheduler, MCS, IPC/Reply/Notification/WaitAddr/fault — queue T01–T16 không có task nào cho các mảng này (`V:950-967`).
- **Phụ thuộc vòng/sai**: T10↔T13 (`V:961,964`); T12 cần "dump trace" nhưng traceformat là T14 phụ thuộc T12 (`V:963-965`); T15 thiếu T14; T11 cần `vifast-abi2` của T05 nhưng chỉ phụ thuộc T08 (`V:962`); T16 phụ thuộc "tất cả" trong khi T04 chờ duyệt (`V:967`).
- **Vượt ngân sách 3 h**: T02 (~33 crate), T04 (~80 entry point), T12 (scenario + sim + scheduler + 4 checker) — `V:887,896`.
- **T04 không có trạng thái REVIEW**: `agentctl next` sẽ trả T04 mãi (`V:955,857`).
- **Không có task** cho `v2 gate` đầy đủ / `nightly` / `mutants` (`V:763-766`), dù K1c đòi "mọi đột biến chuẩn bị bắt" (`V:784`).

**Sửa:** thêm task finalise, sync/scheduler, MCS, IPC; tách traceformat trước T12; T16 = "R1 phủ task đã mở + blocked[]"; chia T02/T04/T12; cột Trạng thái `READY|BLOCKED|REVIEW`; thêm T17 (gate/nightly/mutants).

### O4. Mất ràng buộc cứng, lỗi mở, hợp đồng đo của v1

- **D-01..D-08 biến mất** (`D:2012-2023`), v2 chỉ có D-09..D-27 (`V:918-942`); **D-11/D-18 trùng mã khác nghĩa** với dossier D11/D18 (`D:2027-2029`). D-02 (đóng băng tính năng) và D-08 (cấm đồ hoạ/ViFastKey) bị đảo ngược không ghi lý do.
- **Lỗi mở mất**: T26, T31, T-IPC, audit Q1-A/P1-5 không tồn tại trong v2; T33a chỉ còn triệu chứng, tiêu chí đóng bị hạ còn "có báo cáo" (`V:34,780,952` vs `D:1667-1669`).
- **Marker**: `V:64` nói "giữ hợp đồng marker có case âm" nhưng §9 không có marker/`harness_spec.py`/FORBIDDEN/677 case âm (`D:1398-1431`), chỉ có TAP (`V:700`).
- **Đo lường**: 750 boot (`V:780,952`) không đủ cho p=0,3–0,5 % (mâu thuẫn chính `V:755` `n≥3/p`); mất cấu hình smp/`MAX_IO`/T-BUILD; `V:899` ghi "3 QEMU song song [DOSSIER]" trong khi dossier nói 3 **luồng soak**.
- **Object-in-Untyped** (`V:158`) trình bày như đã chốt, bỏ qua việc dossier ghi tách pool **đã thử 3 lần hỏng AArch64 vì layout .data** (`D:2095-2098`); không có ngân sách static mới/headroom.

**Sửa:** phụ lục "ràng buộc đang hiệu lực" chép D-01..D-08 + ghi cái nào hết hiệu lực; sổ lỗi mở kèm tiêu chí đóng; khôi phục hợp đồng marker hoặc nói rõ thay thế; `[GT]` + kế hoạch đo headroom cho objmem.

## 2. Lỗi chặn — dòng ViScript

(Subagent B kiểm bằng máy: 31 khối mã, đếm tab/thụt lề, đối chiếu từng luật; chi tiết `B-viscript.md` §5.)

### V1. Chế độ lệnh yêu cầu parse phụ thuộc kiểu, nhưng kiến trúc đặt parser trước typechecker

`S:443`: tham số hiểu theo **chữ ký lệnh**; `S:446`: `where size > 5mb` cần biết `size` là trường của `FileEntry`. Nhưng `S:625-631` xếp lexer→parser→**phân giải tên+kiểm kiểu**; `S:644` `vis-types` phụ thuộc `vis-syntax`; `S:848` giao VT06 "hai chế độ ra cùng AST" ở vòng 1, còn typechecker hoãn sang vòng 2 (`S:854`).

**Sửa:** chốt hai pha: `vis-syntax` sinh `CmdCST` chưa định kiểu → `cmd_lower` **trong `vis-types`** dùng bảng chữ ký → HIR; sửa "cùng AST" thành "cùng HIR"; VT06 = CST + golden test, `cmd_lower` là task vòng 2.

### V2. Phụ lục A không sinh ra được chính ví dụ của tài liệu

`S:906-935` có 12 luật, thân tài liệu dùng thêm ~30: không có `pattern` (dù `S:916` dùng), `if_expr`/`match_expr`, `lambda`, `args`, `field_updates`, `types`; `ref[T]` và `()` không phải kiểu hợp lệ; `xs[i]` nhập nhằng generic `[types]`; thiếu `elif`/`assign`/`use`/`impl`/`trait`/`test`/`extern_cmd`/`const`; `NEWLINE/INDENT/DEDENT` chưa từng định nghĩa.

**Sửa:** viết lại thành EBNF tự chứa (~60–70 luật) + cổng "mọi khối mã trong tài liệu parse được". Đây là VT02 nhưng phải xong **trước VT05**.

### V3. Đặc tả từ vựng thiếu, lexer buộc phải đoán

`S:113-133` chỉ có ví dụ, không có luật token: `..` vs float; `%` (percent/modulo/jobspec); danh sách đơn vị `size`/`duration`; tiền tố `p"`/`b"`/`r"`/`"""`; **không định nghĩa escape nào**; nội suy `}}`/format spec; `'ố'` NFD; `#` cắt URL; maximal munch `?`/`?.`/`??`; `|>` vs `|`; `set{...}` vs method `set`; **đơn vị cột chẩn đoán** cho VT03/LSP.

**Sửa:** `SPEC-lex.md` 2–3 trang + golden token test trước VT04.

## 3. Lỗi lớn

### Nhóm kỹ thuật kernel / ABI

- **M1. ThreadState không có trạng thái "bị dừng"** (`V:222-235`); `Suspend` khi `BlockedOn*` phá invariant "ở trong hàng X ⟺ trạng thái mang X" (`V:239`) — được test ở `V:714` mà không định nghĩa; không có `ReadState`/`Wait` (`TcbInfo`/`TcbWait` bị bỏ) ⇒ `jobs`/`wait`/`fg` không cài được.
- **M2. Resume phát lại syscall** (`V:323`): `Call` bị gửi lặp — đúng lớp lỗi T31 của v1 (`D:1694`); không có "huỷ IPC". Cần `Suspend(mode=Abort)` trả `Cancelled`, `Defer` chỉ cho thao tác dài đã có `Restart`.
- **M3. Reply object không có chỗ trong TCB** (`V:229,241,260`): thêm trường + `SetReplyObject`, freeze kèm.
- **M4. `get2_mut` không đủ cho IPC** (`V:174`): cần ≥4 object (2 TCB + EP + Reply + IPC buffer + slot); `place` + `&mut` vào `Vec<u8>`/physmap sẽ đụng Miri/Stacked Borrows — đổi shape API ngay K1a.
- **M5. Hàng chờ WaitAddr "trong header Frame"** (`V:272`): `ObjHeader` 24 B (`V:168`) không chứa được; nếu là header trang thì user sửa được con trỏ kernel. Cần bảng băm phía kernel theo `(frame_id, offset)`.
- **M6. Push migration đọc trạng thái hart khác** (`V:280-281`) — đúng thứ §4.8 từ chối ở work-stealing, vi phạm luật 3 (`V:352`).
- **M7. Slot cap 32 B** (`V:212-213`): ObjRef 16 + badge 8 + 2 link MDB 16 + kind/rights/depth → ~59 B. Nếu chốt 32 B thì phải nén ObjRef/MDB index; ảnh hưởng "hàng triệu cap" × 2.
- **M8. `id` "tăng đơn điệu toàn cục"** (`V:180`) mâu thuẫn luật đường nóng (`V:341-352`) và cụm S3 (`V:345`).
- **M9. U1 hứa `net`** (`V:578`) nhưng net server ở N1 sau U1 (`V:796,805`); VSH2 phụ thuộc thiếu N1 (`V:797`).

### Nhóm phạm vi / quy trình

- **M10. Hai sản phẩm, một agent**: ViScript 16 crate, 4 backend, LSP, gói, plugin ABI (`S:638-658`); OS 3 arch, sim, server. Chỉ có rủi ro "tranh tài nguyên" (`V:985`), không có luật ưu tiên. Đề xuất D-28: tới K3 ViScript chỉ VS0/VSH1 tối thiểu.
- **M11. Không có mốc chạy được sớm**: sim T12 phụ thuộc T09/T10; QEMU ở K3; 8–10 task đầu chỉ biên dịch (xem O3 + D:479-522).
- **M12. Không có task cho gate/nightly/mutants**, `v2`/`vis-dev` chưa định nghĩa; `vifast-agentctl` ở `v2/tools` nhưng ViScript cần từ ngày đầu.
- **M13. Bàn giao giả định `v2/agent/`, `viscript/agent/` đã có** nhưng chưa; T02/VT01 lại là task tạo chúng ⇒ nguy cơ ghi đè; T04/VT02 không có trạng thái REVIEW; commit tag `[VT##]`, R JSON `project` chưa có; hai `R1` trùng tên.
- **M14. Chiều phụ thuộc workspace mâu thuẫn**: `vis-host-vifast` (`S:649`) phụ thuộc `vifast-urt` của v2 (`V:572`), trong khi `V:610` nói v2 phụ thuộc `vis-host-vifast` từ VSH2 ⇒ VT01 có thể không build.
- **M15. Thẩm quyền SPEC**: VD-08 "chỉ Agent 0 đổi SPEC" (`S:884`) vs VT02 do Agent 2 viết (`S:844`); `V:872` "không sửa spec" vs T04 ghi spec (`V:955`).

### Nhóm ViScript (ngoài 3 blocker)

- **M16. Ngữ nghĩa lõi thiếu**: không bảng ưu tiên (`**` kết hợp phải ở Phụ lục A khác §4.5 nhóm với `* / %`; `??` giữa so sánh và range; `not` dưới `cmp`); `|` vừa or-pattern (`S:248`) vừa bị cấm ở script (`S:452`) vừa "giải phóng cho pipe mọi chế độ" (`S:869`) trong khi pipe script là `|>`; Optional/Result thiếu thăng cấp `T→T?`, `T??`, `?` trên `T?`, pattern `none/ok/err`; exhaustiveness với guard/range/kiểu vô hạn; `%` âm/`%0`/NaN/`**` int; chuỗi `s[i]`/`len()` byte/scalar/grapheme, NFC/NFD, `upper()` tiếng Việt; không luật shadowing/CoW/short-circuit/thứ tự đánh giá; ba tên lỗi `ParseError/Error/VisError`; chế độ lệnh không có trích dẫn/thoát (`#`, `|`, dấu cách), không `$HOME`; API std không tồn tại (`FileEntry`, `Ctx`, `GitSub`, method `cap[*]`); `path = "."` chuyển ngầm `str→path`; `test` là từ khoá nhưng có module `test`; async không có `Task[T]`.
- **M17. Quy trình ViScript tự mâu thuẫn**: VT02 "không chặn" (`S:844`) vs gate VS-SPEC (`S:817`) và VD-08; VT09 "≥100 tệp từ ví dụ" bất khả (~10 dùng được; 24 fence, 6 khối `...`, 7 dòng `>`); VT07 yếu hơn §18.2; VT08 "backend giả" khi interpreter bị hoãn (`S:854`) → VT09 treo; **Phụ lục B không parse** (xuống dòng treo `|>` ngoài ngoặc `S:954-959` trái `S:105`; `str(e.path)`; `now` trần; `is_file()` vs `is_file`; `"..." * times`; `./backup` trong biểu thức; closure `fn(...)`).
- **M18. VS1 và typechecker là dự án con**: generic + trait + coherence + bytecode + VM + cache + std + CLI + vi sai 100.000 chương trình, trong một mốc (`S:818`), không ước lượng công sức.

## 4. Lỗi nhỏ / nit

- **SPDX chưa thao tác được**: `V:881` chỉ đúng cho Rust; `.vis` dùng `#`, `.toml/.vidl/.json` không comment; chưa có công cụ (cargo-deny/cargo-about), `NOTICE`/`THIRD-PARTY`; chưa nói ai kiểm phụ thuộc.
- **`v1-final` khi T33a mở** (`V:952`; dossier dòng 107: 0,3–0,5 % treo) — đổi `v1-archive` + known-issue, hoặc vá C (nửa ngày) trước khi tag.
- **750 vs 600** cho cùng phép đo; `soak.py` **tuần tự** (`scripts/soak.py:132-179`), không có script 3 luồng; T01 "commit r323" đã xong (`ca2eaf5`); `pkill -f qemu-system` giết nhánh khác.
- **Lộ trình 90 ngày** (`V:834`) không có giả định năng suất/nguồn lực; K6 "tương đương v1" (`V:789`) không định nghĩa.
- **Q10/Q11** của dossier không được trả lời; không có bảng Q1–Q12 → quyết định.
- **Định dạng**: `V:387-407` cột "Cài ở" K1 lệch K1a/b/c; `V:213` `[ƯL]` cho hằng số cấu trúc; `V:948` "mục 20" thay vì 20.2; `V:969` "VT01..VT09" thiếu VT10; `S:434-441` ngữ pháp trộn văn xuôi; `S:448-456` hàng "Ghi tệp" không phải ký hiệu; 24 fence gắn `python`; `S:262` `str * int` không có trong bảng toán tử.
- **Hai chú thích `##` ở `S:426`/`S:951` KHÔNG phải lỗi** — nằm trong fence, là chú thích tài liệu hợp lệ; đừng sửa.

## 5. Checklist bàn giao (bản rút gọn; đầy đủ ở `C-handoff.md` §7)

**Định nghĩa "bàn giao xong":** Agent 2 mở máy, đọc một thư mục `agent/` + hai tài liệu là chạy được vòng lặp; mọi task có tiêu chí đo được, ngân sách, lệnh cổng, trạng thái `READY|BLOCKED|REVIEW`.

1. Chốt chỗ đặt tài liệu: `docs/v2/`, `docs/viscript/` (không dùng `review/`); cập nhật `README.md` + `docs/README.md`.
2. Vá tài liệu hoặc phát hành `ERRATA.md` cho O1–O4, V1–V3, M1–M6, M13–M15; mỗi phát hiện có quyết định "vá / chấp nhận + lý do".
3. Người điều phối tạo trước 12 mục: `v2/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md` + `JOURNAL/` + `reports/`, tương tự `viscript/agent/`. Sửa T02/VT01 thành "không ghi đè nếu đã tồn tại".
4. `v2/agent/QUEUE.md` = §13 + 5 khối inline (luật, 3 h, DoD, lệnh cổng, schema R) + cột Trạng thái/Ngân sách + T01a/b/c, T03 tách gen-check, T10 mô hình tối giản, T16 "task đã mở", T17 gate/nightly/mutants.
5. `v2/agent/DECISIONS.md` = D-01..D-08 + D-09..D-27 + D-28/D-29 (commit tag, APPROVED T04).
6. `v2/agent/STATE.md` HEAD `ca2eaf5`; `BLOCKED.md` T04 REVIEW kèm O1; VT02 REVIEW không chặn.
7. `viscript/agent/QUEUE.md` = §20.2 + 5 khối inline + VT01 (SPDX + luật cấu trúc + stub `vis-host-vifast`), VT02a (từ vựng + EBNF, **chặn** VT04–VT09), VT09 tiêu chí mới, VT10.
8. `viscript/agent/DECISIONS.md` = VD-01..VD-10 + Q1..Q8 + commit `[VT##]`.
9. `AGENTS.md` root + `v2/` + `viscript/`: thứ tự nguồn sự thật (code+harness > spec > DECISIONS > thiết kế), cấm sửa spec/`accept()`/`#[ignore]`/nới ngưỡng/retry, lệnh cổng, commit tag, "v1 đóng băng", cấm `pkill` toàn cục, `docs/decisions/*` chỉ là v1.
10. Quyết git: tag `v1-final` trên `ca2eaf5` sau T01b (kèm văn bản "T33a mở"), một bản `LICENSE`/`NOTICE`/`THIRD-PARTY` ở root, `.gitignore` đã đủ.
11. Gửi **một gói** (2 tài liệu + 2 QUEUE + 2 DECISIONS + 2 STATE + 2 BLOCKED + AGENTS.md); Agent 2 ack bằng nội dung STATE.md/BLOCKED.md, không bằng chat.
12. Trước khi mở T04: duyệt theo O2 (bảng ánh xạ + 20 kịch bản sim), không duyệt bằng mắt.

## 6. Quyết định cần chốt trước khi chạy

1. **Exit** (O1): syscall 14, `Tcb.Exit`, hay giao thức procsrv?
2. **Phạm vi freeze ABI** (O2): 5 thứ + version, hay giữ D-22 nguyên trạng?
3. **Thứ tự thi công** (O3): thêm K0.5/T12a vào vòng 1, hay giữ nguyên?
4. **Parse chế độ lệnh** (V1): hai pha `CmdCST→cmd_lower→HIR`, hay type-directed parse?
5. **Ưu tiên hai dòng** (M10): OS-first tới K3, hay chia đôi?
6. **Cắt giảm** (D đề xuất): MCS đầy đủ lùi sau K6; chỉ riscv64 tới K6; vẫn giữ 3 arch? — cần quyết vì ảnh hưởng lộ trình 90 ngày.

## 7. Những điểm agent0 làm đúng (giữ nguyên)

- Mô hình seL4, object trong Untyped, `forbid(unsafe_code)`, W^X theo Frame, `check_invariants` + mutation, test đỏ-trước.
- Sim tất định + mô hình tham chiếu + trace ring + shrink + scenario chạy hai nơi — đúng hướng dossier §16.4.
- MCS/Reply/passive server/WaitOnAddr/preemptible ops: lựa chọn hợp lý, có lý do; shootdown/ASID tốt hơn v1, có đột biến AP→BSP.
- D-27 + SPDX trong cổng; T04 cố ý chờ duyệt; tách ViScript QUEUE riêng; §7.3 chỉ còn tích hợp; bảng §21 có cột lý do.
- `xs[i]` panic + `get → T?` + panic bắt ở ranh giới shell; `|` để dành cho pipe (cần sửa or-pattern).
- Nhãn `[ƯL]`/`[GT]`, "không số liệu viết tay" (nhưng tự vi phạm), `p95 ≈ 3/n`.
- Đã kiểm bằng máy: Q1, Q2, Q3 (thụt lề/tab), giấy phép, decimal, bit-func **đúng**; mục lục/số mục khớp; `##` trong fence hợp lệ.

## 8. Kết luận

Không cần viết lại hai tài liệu. Cần **một vòng vá 2–3 ngày người**: (1) Exit + thu hẹp freeze ABI + tiêu chí T04; (2) queue chạy được + K0.5/T12a; (3) viết lại Phụ lục A + SPEC-lex + chốt hai pha parse; (4) job control đủ 3 tầng; (5) gộp D-01..D-08/Q1–Q8 vào DECISIONS; (6) checklist bàn giao + cổng "doc-examples parse". Sau đó thiết kế đủ chín để Agent 2 chạy qua đêm.

## 9. Nguồn chi tiết

- `A-fidelity.md` — dossier ↔ v2: nhãn, số liệu, D-01..D-08, lỗi mở, marker, COPY_WINDOWS, IOMMU.
- `B-viscript.md` — 3 BLOCKER + 21 MAJOR + 16 MINOR + 7 NIT, dẫn chứng `Lnnn`, bảng "không phải lỗi".
- `C-handoff.md` — tự chứa queue, phụ thuộc vòng, T01, chiều workspace, checklist B1–B16.
- `D-technical.md` — 15 phát hiện kỹ thuật, phụ lục ABI tối thiểu, 3 việc nên cắt/đảo.
