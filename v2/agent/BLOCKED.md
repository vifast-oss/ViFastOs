# BLOCKED — ViFastOs v2

Người ghi: **Agent 2**. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

## 1. REVIEW — chờ Agent 0 duyệt (không phải lỗi kỹ thuật)

| Task | Trạng thái | Nội dung chờ | Cần gì để mở |
|---|---|---|---|
| ~~KT-04d~~ | **APPROVED 2026-11-12 Agent 0** | Gói bản 2 + C1/C2; D2-35..39; `KT-05` mở | — |

**O1 (Exit) — ĐÃ CHỐT, không còn là câu hỏi mở.** Nguồn: **QD §1 Q-A + D2-25** (câu chữ QD ghi gọn "chuyển `Inactive` rồi gửi fault"; bản chốt đầy đủ là D2-25 + thiết kế §4.4 — `docs/v2/ViFastOs-v2-thiet-ke.md:285` và `:469`): `Exit` là **syscall 14 `Exit(code)`**; kernel lưu `exit_code`, TCB thành **`BlockedOnSend { ep: fault_ep, call: false }`** mang `FaultKind::Exit{code}` — giao qua **hàng gửi của `fault_ep`, không có Reply**; procsrv `Recv` xong TCB **mới** thành `Inactive`; không có `fault_ep` thì `Inactive` ngay. `Resume`/`Configure` xoá `exit_code`. `wait` là RPC của procsrv; `kill` = procsrv `Tcb.Suspend(Abort)` rồi Revoke cap TCB. Dải syscall **16..31 reserved** (15 = DebugGetInfo, D2-29). Không mở lại O1; **không** chép nguyên văn QD:23 vào `abi.toml` khi làm KT-04a.

## 2. BLOCKED theo bảng `review/PATCH-BRIEF.md` §1

| Task | Chặn bởi | Cần gì để mở |
|---|---|---|
| ~~KT-03~~ | **đã DONE** (`3222602`, `d4d6bf2`) — Agent 0 đã lật trạng thái ở `545f3b4` | — |
| ~~KT-04b~~ | **ĐÃ TRẢ LỜI** (D2-32): được ghi `spec/` một lần, SchedControl lên lõi, VSpace khách = cờ `GUEST` | Agent 2 áp theo D2-32; không hỏi lại |
| ~~KT-04c~~ | **ĐÃ TRẢ LỜI** (D2-33): draft chữ ký hẹp + enum TCB đầy đủ; ca 18 hết blocked | Agent 2 làm theo D2-33; không hỏi lại |
| ~~KT-05~~ | **DONE** `845df91`+`c6487a9`; gen-check ON | — |
| ~~KT-07~~ | **đã DONE** (`bf61901`, `e01a81f`): 50/50 boot, `.data+.bss` 0 B | — |
| KT-08 | KT-02 (đã DONE) — **DONE `cf433b7`**, sửa theo review #11 | — |
| ~~KT-09~~ | **DONE** `3d578c0` + `da7d30f`; 29 test, C5 D2-41 | — |
| ~~KT-10~~ | **DONE** `c78e814`; 20 test, reviewer APPROVE | — |
| ~~KT-11~~ | **READY** — KT-10 DONE; `vifast-model` + F3 quyền | — |
| ~~KT-12~~ | **DONE** `e750847`..`d3dfe31` (host/format); no_std sang **KT-12b** (D2-42) | — |

Không có phụ thuộc vòng. Không có task nào bị chặn vì thiếu spec/thiếu quyết định: QD đã trả lời Q1..Q12 (QD §9).

Danh sách trên đã dọn theo trạng thái thật ngày 2026-11-12: KT-03/KT-07 **DONE**, KT-12 thôi ghi sai dep KT-08.

## 3. Câu hỏi cho Agent 0

> **Cập nhật 23:35:** KT-10 DONE; KT-11 READY. YC-06..17 đã chốt; không hỏi lại.



> **Cập nhật 18:20:** YC-02..05 đã được Agent 0 trả lời trong **D2-29..D2-33**; các mục dưới giữ làm lịch sử. **Không hỏi lại.**

### Q-01 — KT-04c là **20** hay **21** kịch bản? — **ĐÃ TRẢ LỜI 2026-11-12 (D2-27)**

- **Chốt**: **21** kịch bản = 20 theo QD §4 + ca **"Exit khi procsrv chưa Recv"** (D2-27, Agent 0). KT-04c/KT-04d dùng 21, không hỏi lại, không tự đổi.
- Bối cảnh mâu thuẫn (giữ lại để tra cứu): **20** ở `docs/QUYET-DINH-sau-review.md:176` (QD §4) và `v2/agent/DECISIONS.md` Phần IV; **21** ở `v2/agent/QUEUE.md:104` + `docs/v2/ViFastOs-v2-thiet-ke.md:1068`; `review/PATCH-BRIEF.md` §1 ghi 20.
- Đóng cùng ngày với **D2-28** (KT-02 chỉ port tối đa `vifast-fs`, `vifast-fsutil`, `vifast-elf`; `cargo build` xanh + mỗi tệp ≤ 1.500 dòng).

### YC-01 — Nhờ Agent 0 lật trạng thái QUEUE (mở 2026-11-12, Agent 2)

- **Hiện trạng sổ**: `QUEUE.md` vẫn ghi `KT-02 … READY` (`:100`), `KT-03 … BLOCKED` (`:101`), `KT-07 … BLOCKED` (`:108`); `v2/AGENTS.md` §3 vẫn viết "`check`/`gate --fast` **chưa có**".
- **Thực tế đã kiểm**: KT-01 `19659ee` (tag `v1-archive`), KT-02 `b4a8693` (build xanh 3 crate, 52/52 test), KT-03 `3222602` (`gate --fast` PASS), KT-07 (50/50 boot riscv64 + `.data+.bss` 0 B ≤ 256 KiB) — **phụ thuộc đã DONE**.
- **Vì sao Agent 2 không tự lật**: `AGENTS.md` §3 — Agent 2 không sửa `QUEUE.md`; bước 7 "`vifast-agentctl done`" chưa dùng được vì KT-06 chưa có.
- **Đề nghị**: Agent 0 ghi `KT-01/02/03/07 = DONE`, `KT-08 = READY` (dep KT-07 xong), và cập nhật `v2/AGENTS.md` §3. Không chặn Agent 2: các task đang chạy theo điều phối miệng.
- **ĐÃ XỬ LÝ 2026-11-12**: QUEUE đã lật `KT-01/02/03/07 = DONE`, `KT-06/08/12 = READY`; `v2/AGENTS.md` §3 đã sửa. Không cần chờ.


### YC-02 — DebugGetInfo: chưa có số syscall (mở 2026-11-12, Agent 2)

- D2-15/Q-B buộc ABI có `DebugGetInfo` (thiết kế:492), nhưng dải syscall đã đóng 0..14 và **15..31 reserved**.
- Agent 2 **không tự chiếm số**; trong `spec/abi.toml` ghi `[debug_info] status = "REVIEW"`.
- Đề nghị Agent 0 chốt: `DebugGetInfo` lấy **số 15** (đầu dải reserved) kèm một mục D2 mới, hay là một method của object (object nào?).
- Không chặn KT-04a (spec ghi REVIEW); chặn phần "đóng băng đủ 5 thứ" ở KT-04d.

### YC-03 — Hai câu trong QD/thiết kế không khớp code v1 (mở 2026-11-12, Agent 2)

1. QD §4 "0..13 như cũ": **sai** với code v1 (`crates/vifast-abi/src/lib.rs`): v1 `14 = CapRevoke`, v2 `14 = Exit`;
   v1 `0=Send,1=Call`, v2 `0=Call,1=Send`. Bảng ánh xạ của KT-04a ghi rõ đây là **đánh số lại** (không giữ số).
   Đề nghị Agent 0 sửa câu chữ trong QD (tệp cấm với Agent 2) hoặc xác nhận cách hiểu.
2. Thiết kế §8 (dòng ~680) vẽ `v2/spec/` trong khi `AGENTS.md` §1/§4.1, `v2/AGENTS.md` §1, QUEUE (KT-04a),
   BLOCKED và `AGENT2-PROMPT` §2 đều dùng **`spec/` ở gốc**; ngay thiết kế §… cũng gọi `spec/abi.toml`.
   Agent 2 đã tạo **`spec/` ở gốc** (bậc nguồn cao hơn) và đưa vào phạm vi quét R3. Đề nghị sửa cây §8 cho khớp.


### YC-04 — KT-04b không được ghi `spec/`; xin Agent 0 quyết (mở 2026-11-12, Agent 2)

- **Quyền ghi**: 4 nguồn chỉ cho **KT-04a** ghi `spec/` (`AGENTS.md` §3 dòng 39, §4.1 dòng 48; `QUEUE.md` Khối A #1; `AGENT2-PROMPT.md` §2; D2-22). KT-04b **không** có tên ⇒ Agent 2 **không** chép số reserved vào `spec/abi.toml` trong task này (làm vậy là UNSAFE, không phải DRIFT).
- **Đã làm thay** (sản phẩm hợp lệ): draft `v2/docs/abi-reserved-objects.toml` (`status = "REVIEW"`) + máy kiểm `v2/test/abi/check_reserved.py` gắn vào `v2/bin/v2 check` (bước "abi reserved"), tự báo khi spec chưa hợp nhất. Máy kiểm bắt: thiếu object, id trùng, id ngoài dải 8..31, method id trùng/không liền mạch, thiếu `unsupported = true`.
- **Đề xuất số** (theo đúng thứ tự bảng thiết kế §4.21 sau Reply=7, dải 8..31 còn 24 ô nên thừa chỗ):
  `8 SchedControl, 9 VSpace, 10 PageTable, 11 Frame, 12 IrqControl, 13 IrqHandler, 14 IOSpace, 15 VCpu, 16 PowerControl`; `17..31` để trống.
- **Method reserved theo thiết kế**: `VCpu.Configure`, `IOSpace.Attach`, `VSpace` tạo/khách (đề xuất `VSpace.Configure`); **câu hỏi**: QD §4 ghi "VSpace **khách**" là *số reserved* còn thiết kế §4.21 ghi là *method* — Agent 0 chốt: object riêng `VSpaceGuest` (lấy ô 17) hay method `VSpace.Configure`?
- **Xin Agent 0 chọn 1 trong 2**:
  (a) thêm `KT-04b` vào danh sách được ghi `spec/` (sửa `AGENTS.md` §3/§4.1 + QUEUE Khối A + `AGENT2-PROMPT` §2) ⇒ Agent 2 chép draft vào `spec/abi.toml`; **hoặc**
  (b) Agent 0 tự chép draft vào `spec/abi.toml` (một mục D2 mới) và đánh dấu KT-04b DONE.
- **Xin lật sổ**: `QUEUE:102` KT-04a = **DONE**, `QUEUE:103` KT-04b = **BLOCKED** (chờ YC-04), `QUEUE:104` KT-04c = **READY** (độc lập với reserved).
- **Cảnh báo cho KT-04d**: sau dòng `APPROVED`, `spec/abi.toml` phải đổi `status = "REVIEW"` → `"APPROVED"` trong **cùng** cửa sổ; `v2/test/abi/check_abi.py` nay chấp nhận cả hai (tránh cổng đỏ ngay sau duyệt).


### YC-05 — Chữ ký method + từ vựng trạng thái cho `.scn` (mở 2026-11-12, Agent 2)

- **Việc đã xong** (không vi phạm quyền ghi): `v2/docs/scn-format.md` (chiếu §9.3, `[TK]`, `version = 1`),
  `v2/test/scn/check_scn.py` + 3 fixture âm, `v2/test/scn/*.scn` **21 tệp** (1 tệp `blocked = "thiếu ABI"`:
  `18-schedcontext-donation` vì `SchedControl.Configure` chỉ có trong draft, chưa trong `spec/`), gắn vào
  `v2/bin/v2 check` (bước "scn" + "scn fixtures").
- **Phần KHÔNG đạt trung thực**: DoD KT-04c đòi kiểm "**đủ tham số**". `spec/abi.toml` chỉ có `args` dạng văn xuôi
  cho syscall và **một** chữ ký method (`Tcb.Suspend`); `CNode.Copy/Revoke`, `Untyped.Retype`, `Tcb.Configure`…
  **không có chữ ký** ⇒ kiểm arity lúc này sẽ là **tự đặt ABI** (D2-22, `AGENTS.md` §4.1).
- **Xin Agent 0 chọn 1 trong 2**:
  (a) cho **KT-04c** ghi `args/ret` cho từng method vào `spec/abi.toml` (như KT-04a), hoặc
  (b) duyệt draft `v2/docs/abi-call-signatures.toml` để Agent 2 soạn chữ ký ở trạng thái REVIEW, hợp nhất sau.
- **Cùng lý do**: `BlockedOnSend{FaultKind::Exit}` chỉ là *note* trong `spec/abi.toml`; chưa có **từ vựng trạng thái TCB**
  để kịch bản khẳng định "procsrv chưa Recv ⇒ TCB còn `BlockedOnSend`". Đề nghị đưa enum trạng thái (§4.4) vào `spec/`.
- Không chặn KT-06/KT-08/KT-12 (đều `READY`).


### YC-06 — `vifast-fs` không thể tái hiện T26 ở host; xin Agent 0 chốt phạm vi (mở 2026-11-12, Agent 2)

- **Đo được**: `commit()` của fs gọi `dev.flush()` **ngay trong đường ghi** (`v2/lib/vifast-fs/src/tx.rs:81`)
  ⇒ sau mỗi `create`/`write` chỉ còn **một** khối bẩn (commit record LBA 29); 1.000 seed với kẻ thứ ba ghi
  xen + xoá cache đều **không** tạo được lệch cache/đĩa. Crate cũng **không** có tầng VFS/`read_dir` (T26 v1
  nằm ở đó) và không có `smp`.
- **Đã làm thay** (đèn host, không claim đóng T26): `v2/lib/vifast-fs/tests/t26_read_your_own_write.rs` —
  `ryow_1000_seeds`, `ryow_across_remount_1000_seeds` (đọc RAW sau `fsync`), ca âm
  `harness_catches_silent_write_loss` (thiết bị **mất ghi im lặng** ⇒ phép đo phải đỏ; đã dán log đỏ khi đảo
  kỳ vọng); dùng `CachedDev` đang ship + đèn `lost_writes()`; `MAX_IO = 256` cục bộ trong test.
- **Xin Agent 0 chọn**:
  (a) KT-T26 = **đèn** + ca âm, DoD đổi thành "đèn chứng minh bắt được mất ghi im lặng" (không tuyên bố đóng
      T26); phần tái hiện đầy đủ chuyển sang **KT-08** (sim, `-smp 2`, 50 boot theo D-04) hoặc **K3.5** (VFS);
  **hoặc**
  (b) giữ DoD cũ ⇒ KT-T26 **BLOCKED tới KT-08**, và ghi rõ T26 **chưa** đóng ở v2.
- Nhắc lại hai con số đúng: `MAX_IO` **v1 = 128** (r262 hạ từ 256 vì 256: 12/49 hỏng); `USER_COPY_MAX = 256`
  là hằng số khác. 256 ở KT-T26 là **cấu hình tái hiện** (D-04), không phải giá trị sản phẩm.

### KT-04d — gói duyệt đã gửi (2026-11-12, **không phải blocker**)

- Tệp: [`reports/KT-04d-goi-duyet.md`](reports/KT-04d-goi-duyet.md) — `spec/abi.toml`, 21 `.scn`, draft chữ ký D2-33,
  checklist 5 mục/13 method, mapping đã sửa (16 syscall / 26 method / 23 đích / 20 bỏ), D2-26..28.
- **Bản 2 (sau review #9, 18:40)**: gói đã được sửa (chữ ký theo thiết kế, `.scn` hết ca sai ngữ nghĩa,
  `expect_states` có trường, fixture âm arity, cross-check spec↔draft). Agent 0 nên đọc **bản 2**, không duyệt bản 18:29.
- Chờ **một dòng** `APPROVED <ngày> Agent 0` trong `v2/agent/QUEUE.md`; dòng đó mở **KT-05**. Agent 2 **không** ghi APPROVED.
- Còn mở đúng **một** câu hỏi kỹ thuật: **YC-06** (phạm vi T26).


### YC-11 — Sổ và `spec/` cần Agent 0 xử lý (mở 2026-11-12 sau review #10, Agent 2)

1. **Lật trạng thái**: `QUEUE:105` `KT-04c` vẫn `READY` dù đã DONE `5454834`; `QUEUE:150` vẫn liệt `KT-04b/04c` là READY. `agentctl next` vì thế trả `KT-04c` thay vì `KT-06`/`KT-08`.
2. **`QUEUE` Khối A bước 7** bảo chạy `` `vifast-agentctl done <id>` `` — đây là lệnh **ghi** QUEUE, trái `AGENTS.md` §3 (sổ thuộc Agent 0). Đề nghị: bỏ khỏi bước 7, hoặc ghi rõ agentctl là đường ghi được phép và ai chạy.
3. **Comment `spec/abi.toml` còn cũ** (`:8` "0..13 + 14 Exit", `:24` "0..14", `:105` "15 syscall", `:139` "Reserved 8..31"). Agent 2 **đã revert** bản sửa (không có quyền sửa `spec/` ngoài D2-29/D2-32); đề nghị Agent 0 commit phần comment trong cùng cửa sổ APPROVED.
4. **D2-21 chưa có máy kiểm**: không có `deny.toml`/`cargo-deny` trong repo — hiện chỉ dựa vào việc "không có `[dependencies]`".


### YC-12 — Thứ tự/định dạng trace: KT-12 hay KT-08 sở hữu? (mở 2026-11-12 sau review #11, Agent 2)

- **Hiện trạng**: KT-08 (`cf433b7`) dump trace **text** `# scn seed=… steps=… tasks=… cap=…` + `step=… kind=… task=… used=… value=…` + `# trace-hash=` (FNV-1a).
  Thiết kế §9.11 và QD:350 nói **KT-12** sở hữu traceformat (bản ghi 32 B `{t_ns,hart,kind,tcb,a,b}`, "trước sim dump").
- **Vấn đề**: Agent 2 đã cho KT-08 dump trước khi KT-12 mở ⇒ KT-12 sẽ phải phá hoặc chiều theo định dạng tự chế. Không tự quyết (D2-11/D2-22).
- **Xin Agent 0 chọn**:
  (a) KT-08 **bỏ dump text**, chỉ giữ `Event` + hash; **KT-12** sở hữu định dạng (đúng QD:350). Golden hiện có chuyển thành của KT-12.
  (b) KT-12 **nhận** định dạng text hiện tại làm **legacy** và thêm bản ghi nhị phân 32 B song song (máy kiểm so byte cả hai).
- Trong lúc chờ, KT-08 đã có: golden + so **hai tiến trình** + ca âm "lệch 1 byte" + test ngữ nghĩa "2 seed phải khác quỹ đạo" (trước sửa là **đỏ**, đã dán log).


### YC-13 — Khối E: "8 khoá" hay 11, "task đã mở" nghĩa là gì (mở 2026-11-12 sau review #12, Agent 2)

1. **Số khoá**: `QUEUE:74` ghi "8 khoá" nhưng ví dụ ngay dưới (`:76-90`) và thiết kế §11.7 có **11** khoá
   (`project, round, head, build_id, tasks, gates, sim, coverage, mutants, perf, blocked`). Báo cáo `R1-v2.json`
   chọn **11** và máy kiểm `check_report.py` ép đủ 11. Đề nghị Agent 0 sửa câu chữ Khối E thành **11**.
2. **"Phủ task đã mở"**: hiện máy kiểm ép phủ **đủ 17/17 hàng QUEUE** (mạnh hơn "đã mở"); đề nghị chốt định nghĩa
   để `CHECK_REPORT` và báo cáo khớp nhau.
3. **Khoá `next`**: `agentctl report` có `next`; báo cáo Khối E không có. Có thêm vào JSON chuẩn không?


### YC-14 — Dòng ViScript: duyệt ST-02a, xung đột `04xx`, lật sổ (mở 2026-11-12 sau review #13/#14, Agent 2)

1. **Duyệt ST-02a cho đúng mẫu**: chỉ có chữ trong ô bảng `viscript/agent/QUEUE.md:98` + ghi chú `:111`;
   mẫu bắt buộc là **dòng riêng** `APPROVED <YYYY-MM-DD> Agent 0` ngay dưới mục task. Cũng cần sửa 3 nơi còn `REVIEW`:
   `docs/viscript/SPEC-lex.md:3`, `viscript/AGENTS.md:28`, `viscript/agent/DECISIONS.md:58`.
2. **Xung đột quy tắc cấp**: thiết kế dùng `VS0412` = "kiểu không khớp" (`ViScript-thiet-ke.md:902,909,1006`).
   Agent 2 đã sửa *đề xuất* trong `vis-diag` thành `01xx` từ vựng / `02xx` cú pháp / `03xx` tên / **`04xx` kiểu** / `05xx` runtime
   và ghi rõ là **đề xuất chờ duyệt**; cần Agent 0 chốt vào SPEC (ST-02b được phép sửa SPEC hay không:
   `AGENTS.md` §4.1 chỉ liệt ST-02a, còn `viscript/agent/QUEUE.md:25` lại cho ST-02b — **hai luật mâu thuẫn**).
3. **Lật sổ**: `ST-02b` → DONE (đã xong `a48857b`); `ST-03` → READY (dep `ST-01` DONE); `v2/agent/QUEUE.md:152` còn liệt
   6 task OS READY (thực tế 0); `viscript/agent/BLOCKED.md` mục 4 còn "4 task READY" (đã sửa thành 0).
4. **`3d5539a [ST-00]`**: xác nhận ai viết commit lật sổ (nếu Agent 2 viết QUEUE thì trái AGENTS §3).


### YC-15 — **ĐÃ CHỐT 2026-11-12** — Dọn luật/đường dẫn sau KT-05 (mở 2026-11-12 sau review #16, Agent 2)

1. **`AGENTS.md` §3/§4** chỉ cho KT-04a/04b/ST-02a/ST-02b ghi `spec/`, trong khi `QUEUE:132` + D2-33 đã cho KT-05 ghi
   `[method_signature]` + `[debug_info].return_channel` ⇒ văn bản luật chưa khớp hành vi đã duyệt; đề nghị commit `[KT-00]`.
2. **`golden` treo**: `spec/abi.toml:18` khai `test/abi/abi-v1.golden` nhưng tệp không tồn tại và gen không sinh;
   D2-15 đòi golden — chọn: bỏ khoá, hoặc sinh golden từ spec trong KT-05b.
3. **Đường dẫn `docs/abi.md`**: DoD (`QUEUE:107`, thiết kế) ghi `docs/abi.md`; nay gen sinh đúng đường đó (đã bỏ `docs/v2/abi.md`).
4. **Lật sổ**: `QUEUE:107` KT-05 = DONE (commit `845df91` + sửa review #16); `QUEUE:151` còn liệt ST-02b/ST-10.


### YC-16 — **ĐÃ CHỐT 2026-11-12** — `[trace]` cần thành spec trước khi "dùng chung QEMU" (mở 2026-11-12 sau review #17)

D2-38 ghim **32 B + 6 tên trường** nhưng chưa có: offset/endianness từng trường, bảng `kind`, ngữ nghĩa `t_ns`,
header container (magic/version/count/hash). Golden nhị phân hiện do Python sinh độc lập từ golden text (200×32 B) nhưng
**layout vẫn là lựa chọn của Agent 2**; crate còn dùng `Vec` nên chưa link được vào kernel `no_std`.
Đề nghị Agent 0: (a) duyệt `[trace]` trong `spec/abi.toml` hoặc D2 mới; (b) khôi phục mệnh đề "cùng định dạng QEMU"
ở `QUEUE:114`; (c) quyết `Ring<const N>` no-alloc thuộc KT-12 hay KT-12b.

### YC-17 — **ĐÃ CHỐT 2026-11-12** — gộp: lật sổ + `[trace]` + C5 (mở 2026-11-12 sau review #18)

1. Lật `QUEUE:107` KT-05 = DONE và `QUEUE:114` KT-12 = DONE ⇒ mở **KT-09**.
2. Duyệt `[trace]` (YC-16a): offset/endianness/bảng kind/header trong `spec/abi.toml` hoặc D2 mới; khôi phục mệnh đề
   "cùng định dạng QEMU" (thiết kế `:835` vs `QUEUE:114`).
3. **Định nghĩa C5**: "ngữ nghĩa thời điểm `expect_states`" (DoD KT-09) chưa có ở đâu trong repo — cần chốt trước khi KT-09 chạy được.
4. Chủ sở hữu `#![no_std]`/feature `alloc` của `vifast-traceformat` (KT-12b hay KT-12 tiếp).

Không có câu hỏi mở nào khác tại thời điểm này. Khi phát sinh: ghi ở đây kèm dẫn chứng (`tệp:dòng`), đề xuất cụ thể, và task kế sẽ làm thay.

## 4. Dòng ViScript **không** bị chặn bởi mục 1–2

`ST-01`, `ST-02b`, `ST-10` đang `READY` và độc lập với KT-04d/KT-03. Khi `v2/agent/QUEUE.md` hết task `READY` thì lấy task ViScript (D2-20); ngoại lệ duy nhất được chạy song song là `ST-02a` (chỉ là tài liệu).
