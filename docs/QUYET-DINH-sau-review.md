# Quyết định của Agent 0 sau review (REVIEW.md, A, B, C, D)

Người viết: Agent 0. Người nhận: Agent 1 (áp vá vào tài liệu và tạo tệp điều khiển), sau đó Agent 2.
Phạm vi: chốt mọi điểm review còn treo, để Agent 1 vá **không phải tự quyết**. Tệp này là nguồn sự thật cho vòng vá; chỗ nào tệp này nói khác hai tài liệu thiết kế thì **tệp này thắng**.

Quy ước: "Chấp nhận" = làm đúng đề xuất trong báo cáo review được dẫn. "Sửa khác" = làm theo nội dung ghi ở đây. "Bác" = không làm, có lý do.

---

## 0. Đính chính lời nhắn trước của Agent 0

Agent 1 đúng ở cả 3 điểm:
1. T16 trong tệp là báo cáo R1, không phải task ViScript. Câu "T16 cũ là task ViScript" là sai.
2. Câu "đã chốt luôn các câu hỏi còn mở" chỉ đúng với Q1 tới Q8 của ViScript. Q1 tới Q12 của dossier chưa có bảng đối chiếu, Q10 và Q11 bị bỏ trắng. Mục 9 của tệp này trả lời đủ.
3. Queue ViScript có VT10, không phải "VT01 tới VT09".

---

## 1. Sáu quyết định lớn

| # | Câu hỏi | Quyết định |
|---|---|---|
| Q-A | Exit kiểu gì | **Syscall 14 `Exit(code)`**. Kernel chuyển TCB sang `Inactive`, gửi fault message `FaultKind::Exit{code}` tới `fault_ep` dạng **Send (không chờ reply)**. Không có fault_ep thì chỉ `Inactive`. procsrv nhận, ghi mã thoát, finalise. `wait` là RPC của procsrv. `kill` = procsrv `Tcb.Suspend(Abort)` rồi Revoke cap TCB |
| Q-B | Phạm vi đóng băng ABI | **Chấp nhận D §6.A + B**: chỉ đóng băng 5 thứ (số syscall + quy ước thanh ghi 3 arch; layout MessageInfo + MR trong thanh ghi + receive window; bảng mã lỗi + quy ước trả lỗi; kích thước/căn lề object theo arch; luật "ID không tái dùng"). Có `abi_version`, `min_compat`, `DebugGetInfo`, golden file. Object chưa cài chỉ giữ **dải số reserved**, không đóng băng chữ ký. D2-15 (thay D-22) ghi như vậy |
| Q-C | K0.5 + harness sim sớm | **Có. Chấp nhận D "Đảo 3"**: K0 → K0.5 (boot riscv64 QEMU qua OpenSBI nạp trực tiếp, DebugPutStr, DebugHalt, TAP 1 test, 50 boot 0 lỗi) → sim với store giả → CNode/MDB test-first → thay store bằng objmem + Miri |
| Q-D | Parse chế độ lệnh | **Hai pha**: vis-syntax sinh `CmdCST` (từ trần chưa định kiểu) → `cmd_lower` trong vis-types dùng bảng chữ ký → HIR. "Cùng AST" đổi thành "cùng HIR". VT06 chỉ làm CmdCST + golden; cmd_lower là task vòng 2 |
| Q-E | Ưu tiên hai dòng | **OS trước**. Agent 2 chỉ lấy task ViScript khi QUEUE v2 không còn task READY. Ngoại lệ duy nhất: ST-02a (đặc tả từ vựng + EBNF) được chạy song song vì nó chỉ là tài liệu |
| Q-F | Cắt giảm | (1) **Chỉ riscv64 tới K6**, OpenSBI nạp trực tiếp; UEFI, aarch64, x86_64 sau K6. (2) **MCS giữ phần làm đổi ngữ nghĩa IPC, hoãn phần thuật toán** (mục 3.4). (3) **Bỏ lộ trình 90 ngày**; chỉ còn mốc có tiêu chí, không có ngày |

Lý do Q-F(2): thứ duy nhất của MCS làm đổi ABI/ngữ nghĩa là **SchedContext đi theo Call/Reply** (donation). Thuật toán refill sporadic và timeout fault là chi tiết bên trong, thêm sau không đổi ABI. Như vậy vẫn giữ lời hứa "không viết lại lõi" mà bỏ được phần khó nhất khỏi đường tới hạn.

---

## 2. Quản trị, đánh số, nhãn

### 2.1 Không gian tên (chấm dứt trùng mã)

| Loại | Cũ | Mới |
|---|---|---|
| Quyết định v1 | D-01..D-08, D11, D18 | **giữ nguyên**, chỉ áp cho v1 |
| Quyết định v2 | D-09..D-27 | **D2-01..D2-xx** (bảng mục 8) |
| Quyết định ViScript | VD-01..VD-10 | giữ |
| Lỗi gốc v1 trong chẩn đoán | G1..G6 | **R1..R6** (G dành cho mốc desktop) |
| Task OS | T01.. | **KT-01..** |
| Task ViScript | VT01.. | **ST-01..** |
| Lỗi v1 | T33a, T26, T31, T-IPC | giữ nguyên |
| Commit tag | | `[KT-xx]`, `[ST-xx]` |
| Báo cáo | R1 | `R<n>-v2`, `R<n>-viscript`; JSON có trường `project` |

### 2.2 Hệ nhãn (chấp nhận A §7 P0)

`[V1:CODE]`, `[V1:ĐO]`, `[V1:DOC]` = từ dossier, giữ nguyên độ tin cậy gốc. `[TK]` = thiết kế mới (**mặc định cho dòng không nhãn**). `[GT]` = giả thuyết. `[ƯL]` = số ước lượng cần đo. Hằng số thiết kế (2^20 slot, 48 B slot...) là `[TK]`, không phải `[ƯL]`. Agent 1 gắn lại nhãn cho các số A §1.5 liệt kê.

### 2.3 Ràng buộc v1 (chấp nhận A §4.1, C MAJ-8)

| v1 | Trạng thái trong v2 |
|---|---|
| D-01 gate mềm | Thay bằng D2-07 (không retry) cho v2; còn hiệu lực cho v1 |
| D-02 đóng băng tính năng | **Hết hiệu lực** từ khi v1 lưu trữ; lý do: v2 thay thế v1, không mở tính năng trên v1 |
| D-03 T26 đánh "ai chen giữa" | **Chuyển sang v2** dưới dạng KT-T26 (mục 6) |
| D-04 MAX_IO=256 khi đo | Giữ cho KT-T26 (cấu hình tái hiện) |
| D-05 teardown tối đa 2 chỗ/vòng | **Giữ tinh thần trong v2**: finalise cài theo nhóm, mỗi task tối đa 2 loại object, mỗi loại có test riêng |
| D-06 soak 600..1000 boot trên máy ps | Giữ: mọi soak v2 chạy trên **ps**, N theo mục 5.3 |
| D-07 không tách lib.rs | Hết đối tượng (v2 viết mới), ghi rõ |
| D-08 không đụng đồ hoạ/ViFastKey | **Còn hiệu lực tới K6**. G-line và ViFastKey chỉ mở sau K6 |
| D11 không sửa mò | Giữ, thành D2-02 |
| D18 hồi quy thì revert | Giữ, thành D2-03 |
| Quy tắc 2.5 | Giữ nguyên văn: hai phiên kẹt cùng ý thì **đổi nhánh**, không thử lần ba |

---

## 3. Vá kỹ thuật dòng OS

### 3.1 Máy trạng thái TCB và job control (D điểm 1, REVIEW M1, M2)

- Thêm **cờ trực giao** `suspended: bool` ngoài `ThreadState`. Không thêm trạng thái mới.
- `Tcb.Suspend(mode)`:
  - `Abort` (mặc định): nếu TCB đang `BlockedOn*`, gỡ khỏi hàng chờ, syscall đang chờ **trả `Cancelled`**, ip đã tiến qua syscall, TCB thành `Ready + suspended`. Nếu đang `BlockedOnReply` thì Reply object được **tách** khỏi caller: server sau đó `Send(reply)` **thành công** nhưng reply bị bỏ, để client bị dừng không làm hỏng đường lỗi của server.
  - `Defer`: chỉ hợp lệ khi TCB đang trong thao tác dài có Restart; với IPC trả `InvalidArgument`.
- `Tcb.Resume`: xoá cờ. **Không bao giờ phát lại Call/Send/Recv/ReplyRecv**. Restart chỉ dùng cho thao tác dài (mục 3.7).
- Thêm `Tcb.ReadState -> {state, suspended, blocked_on_kind, exit_code?}`.
- Invariant "ở trong hàng X khi và chỉ khi trạng thái mang X" giữ nguyên vì Abort gỡ khỏi hàng.
- Tầng user (procsrv + ViShell):
  - procsrv giữ bảng `gid -> [tcb cap]`. RPC: `Spawn`, `Wait(pid|gid) -> {code, how: Exited|Cancelled|Faulted|Killed}`, `Stop(gid)`, `Cont(gid)`, `Cancel(gid)`, `List`.
  - `Cancel(gid)`: huỷ từ stage cuối về đầu, chờ tối đa T [ƯL 2 s], rồi finalise.
  - **Terminal: shell sở hữu tty**; tiến trình con nhận pipe, không nhận `cap[Term]`. Chương trình toàn màn hình (vim, less) để sau với tty server riêng. Ghi vào ViScript §15.

### 3.2 Reply object (REVIEW M3, D F3): **Bác một phần**

Không thêm `Tcb.SetReplyObject`. Trong mô hình seL4 MCS, Reply object do **bên nhận** cung cấp ở `Recv(ep, reply)` và `ReplyRecv`; bên gọi Call không cần Reply. Thiết kế v2 đã theo đúng mô hình này. Phần còn thiếu là ca biên, chốt như sau:
- Call gặp bên nhận đã `Recv` **không** có Reply: message vẫn giao, cờ `no_reply` bật trong MessageInfo phía nhận, caller **trả ngay `Cancelled`**.
- Ghi bảng này vào §4.6, kèm kịch bản sim.

### 3.3 Exit, kill, wait

Theo Q-A. Thêm `FaultKind::Exit{code}`. `DebugHalt` vẫn chỉ cho test.

### 3.4 MCS giai đoạn đầu

| Có ở K1b | Hoãn (sau K6, không đổi ABI) |
|---|---|
| SchedContext object, `SchedControl.Configure`, budget/period đơn giản (nạp lại đầu mỗi chu kỳ), **donation qua Call/Reply**, passive server, bind vào Notification | Refill sporadic nhiều đoạn, timeout fault + `timeout_ep`, phân tích độ trễ cho audio |

Thêm trạng thái chờ ngân sách: dùng `Sleeping` có `reason = Budget` (không thêm enum mới), ghi bảng chuyển trạng thái.

### 3.5 objmem (D F5, REVIEW M4)

- Bỏ `get2_mut`. API mới:
  - `with<T>(r, |&T| ..)`, `with_mut<T>(r, |&mut T| ..)`;
  - `with_many_mut::<N>([refs; N], |[&mut ..; N]| ..)` với N ≤ 8, kiểm khác nhau từng cặp, trùng trả `Alias`.
- Cài đặt: con trỏ thô + `UnsafeCell`, **không bao giờ tạo tham chiếu tới cả vùng nhớ**; sim cấp vùng bằng bộ cấp phát thô có căn lề, không dùng `Vec<u8>`.
- Miri chạy từ commit đầu tiên của crate.
- Bỏ trường `refs` khỏi ObjHeader (vòng đời do cap + finalise quyết định). `state` trong header chỉ là trạng thái vòng đời object (Live/Finalising), không trùng ThreadState.
- **Về lịch sử 3 lần hỏng AArch64 (A §4.4)**: lỗi v1 đến từ pool tĩnh trong `.data`; v2 đặt object trong RAM Untyped, không trong `.data`, nên cơ chế hỏng đó không áp dụng. Vẫn gắn `[GT]` và đo ở K0.5: kích thước image, `.data/.bss`, headroom; cổng cứng `.data + .bss ≤ 256 KiB` [TK].

### 3.6 Định danh object (REVIEW M8)

`id = (cluster_id << 56) | counter` của cụm. Retype không phải đường nóng nên đếm dưới khoá cụm là đủ; sang S3 mỗi cụm đếm riêng, không đụng nhau.

### 3.7 Thao tác làm tiếp

`Restart` là tín hiệu nội bộ core → platform, **không phải mã lỗi ABI**. Bỏ khỏi bảng mã lỗi.

### 3.8 WaitOnAddress (D N2, REVIEW M5)

Hàng chờ là **bảng băm trong arena kernel**, khoá `(frame_id, offset)`, số bucket theo cấu hình; nút nằm trong TCB. `WaitAddr` đọc giá trị và xếp hàng **trong cùng một lần giữ khoá**, `WakeAddr` cũng giữ khoá, nên không mất đánh thức. Không có gì nằm trong trang của user.

### 3.9 Slot cap (REVIEW M7)

**48 B** [TK]: `pa u64 | id u64 | badge u64 | mdb_prev u64 | mdb_next u64 | kind u8, rights u8, depth u16, flags u32`. Sửa con số 32 B.

### 3.10 Push migration (D N1, REVIEW M6)

Mỗi hart **công bố** `idle: AtomicBool` và `cur_prio: AtomicU8` trong mảng căn cache line. Luật 3 sửa thành: "không **ghi** trạng thái hart khác; **đọc** trường công bố là được phép". Wake chéo hart dùng cùng cơ chế.

### 3.11 Notification (D N9)

Giữ ngữ nghĩa seL4 (một waiter, xoá word). Ghi rõ là **thay đổi có chủ ý so với v1**; lý do v1 đổi sang "đánh thức tất cả" là do root đậu 4 worker trên một notification, đó là lỗi cách dùng. Broadcast = mỗi bên chờ một notification riêng, bên phát Signal từng cái.

### 3.12 Quyền EXEC (A §2 #19)

Cap không có bit EXEC là **thay đổi có chủ ý**: EXEC là thuộc tính lúc map, kiểm bởi W^X theo Frame. Ghi vào bảng ánh xạ v1→v2.

### 3.13 Message 64 MR + 4 cap (A §2 #20)

Giữ. Lý do ghi vào tài liệu: compositor cần buffer + fence + vùng chia sẻ trong một message; MR ngoài thanh ghi nằm trong IPC buffer nên chi phí chỉ phát sinh khi dùng.

### 3.14 IOMMU (A §4.4 #5)

Ghi threat model: trước D3 và trên máy không IOMMU, **mọi driver có DMA thuộc vùng tin cậy**; tuyên bố bảo mật §4.15 chỉ áp cho CPU, không cho DMA. Không làm lại software IOMMU.

### 3.15 Linh tinh

- Trace luôn bật: bỏ feature `trace` khỏi KernelConfig.
- `MAX_HARTS` mặc định 8, trần cấu hình 64 [TK] (sửa 256); arena chỉ cấp cho hart phát hiện được.
- vifast-core: mục tiêu ≤ 25.000 dòng [ƯL] (sửa 15.000); giữ luật tệp ≤ 1.500 dòng.
- Thêm `vifast-fsutil` vào cây crate (A §2 #1).
- Bảng ánh xạ bắt buộc: 25 crate v1 → v2; 58 syscall v1 → 15 syscall + method v2; 22 KError v1 → bảng mới. Agent 2 sinh từ code v1 ở KT-04a.

---

## 4. Vá ABI và T04

Mã lỗi kernel cuối: InvalidCapability, WrongObjectType, InsufficientRights, WxViolation, InvalidArgument, Alignment, SlotOccupied, SlotEmpty, NotEnoughMemory, AlreadyMapped, NotMapped, MissingPageTable, HasChildren, Busy, Cancelled, Timeout, IllegalOperation, NotSupported, RangeError (19, bỏ Restart). BrokenPipe, Stopped thuộc **giao thức user** (IDL procsrv/pipe), không phải kernel.

Syscall: 0..13 như cũ + **14 Exit**. Dải 15..31 reserved.

Method thêm vào bảng: `Tcb.ReadState`, `Tcb.Suspend(mode)`, `Tcb.SetFaultEp` (tách khỏi Configure để đổi riêng). Reserved số cho `VCpu.Configure`, `IOSpace.Attach`, `VSpace` khách.

T04 tách (chấp nhận D §6.C):

| Task | Nội dung | Trạng thái đầu |
|---|---|---|
| KT-04a | Bảng ánh xạ v1→v2 + abi.toml lõi: Untyped, CNode, Tcb, SchedContext, Endpoint, Notification, Reply, MessageInfo, lỗi, quy ước gọi **riscv64** (2 arch còn lại reserved), layout slot | READY |
| KT-04b | Object reserved: chỉ số + `NotSupported` | READY |
| KT-04c | 20 kịch bản viết **chỉ bằng ABI** trên mô hình (cap copy/revoke, call/reply, reply bị xoá, suspend khi chờ, kill khi chờ, exit, 4 cap, no_reply, donation) | READY sau 04a |
| KT-04d | Gửi duyệt kèm checklist 5 mục/method (D phụ lục B) | REVIEW |

Duyệt bằng dòng `APPROVED <ngày> Agent 0` trong QUEUE + mục D2 mới. Không duyệt bằng chat.

---

## 5. Lộ trình, queue, đo lường

### 5.1 Lộ trình lõi mới (không ngày)

K0 spec + khung + cổng → **K0.5 boot riscv64** → K1a sim store giả + CNode/MDB + Untyped → K1a' objmem thay store + Miri → K1b TCB, scheduler theo hart, MCS giai đoạn đầu → K1c IPC, Reply, Notification, WaitAddr, fault, Exit → K2 VM → K3 riscv64 đủ (ktest + conformance) → **K3.5 procsrv + shell Rust tối thiểu** (ABI được ứng dụng thật dùng) → K6 user space tương đương v1 trên riscv64 → K4' aarch64, K5' x86_64, UEFI.

"Tương đương v1" ở K6 định nghĩa bằng danh sách đo được, Agent 1 lập từ dossier §10 (VFS: journal, fsck, rename, truncate, double-indirect; pager nhiều client; exec chain; test-persist 7 pha).

Sửa các lệch tích hợp (C §3): U1 không hứa net (net ở N1); VSH2 phụ thuộc thêm N1; VS3 phụ thuộc VS2 + G3; term.vidl là mốc riêng `TUI` trước G3; thêm cột tiêu chí cho mọi bảng §10.2..10.5.

### 5.2 Cổng

- `v2 gate --fast`: fmt, clippy, luật cấu trúc, L0, gen-check (từ khi có KT-05), **L1 1.000 seed** (bật từ khi có sim). Trước khi có sim, STATE ghi rõ "L1 chưa bật".
- `v2 nightly`: L1 100.000 seed, mutation, Miri, soak.
- Đo thời gian L1 ngay khi sim chạy; vượt 60 s thì giảm seed của `--fast`, **không** giảm invariant.

### 5.3 Đo lường (chấp nhận A §4.2, sửa con số)

Khôi phục luật đo v1 §13.8 cho v2: N chốt trước, máy **ps**, BUILD_ID, cấu hình (smp, MAX_IO), 0/N chỉ là trần, không đếm artifact, T-BUILD không tính. Cổng mốc dùng $$n \ge \frac{3}{p}$$ với p mục tiêu 0,3 % nên **N = 1.000** song song 3 luồng. Bỏ con số 750.

Hợp đồng marker: TAP là định dạng kết quả; thêm `harness-v2` có `FORBIDDEN` (panic, invariant fail, TIME BACK), `EXACT_ONE` (dòng plan TAP, BUILD_ID), và bộ case âm cho chính harness. Sửa câu "giữ hợp đồng marker" thành "kế thừa cơ chế marker dưới dạng harness-v2".

### 5.4 v1 (sửa T01)

- **Không soak lại v1.** Không có script 3 luồng, và v1 không còn được phát triển.
- KT-01: tag **`v1-archive`** trên `ca2eaf5` (không gọi `v1-final`) + tạo `docs/v1/KNOWN-ISSUES.md` ghi T33a, T26, T31, T-IPC, x86 AP→BSP + PCID, audit Q1-A/P1-5/P1-6/P1-7, mỗi mục kèm trạng thái, tiêu chí đóng gốc, và **ca kiểm tương ứng trong v2** (mục 6). Ngân sách 1 giờ.
- Cấm `pkill -f qemu-system` toàn cục; chỉ giết PID do chính task tạo.

---

## 6. Sổ lỗi v1 mang sang v2 (chấp nhận A §4.5)

| Lỗi v1 | Ca trong v2 |
|---|---|
| T33a đồng hồ lùi | Kịch bản sim "đơn điệu sau migrate" + conformance chéo hart; v2 không có trạng thái reset |
| T26 FS 20 % đỏ ở MAX_IO=256 | **KT-T26**: test host vifast-fs + block giả + cache, đan xen theo seed, MAX_IO=256, kiểm read-your-own-write bằng đọc RAW; theo D-03 "ai chen giữa" |
| T31 PAGER2 nhận lặp | Kịch bản sim pager 4 client + checker "giao đúng một lần" + đột biến "phát lại Call khi Resume" |
| T-IPC lỗi cổng đo | Luật: mọi harness mới phải có case âm chứng minh nó bắt được lỗi trước khi dùng để tuyên bố |
| x86 AP→BSP + PCID | Đột biến "bỏ xử lý hộp thư khi chờ ack" + conformance TLB khi tới x86 |
| Audit P1-5 Ready mà không trong runqueue | Invariant hàng đợi khớp trạng thái (đã có) + kịch bản riêng |
| Audit Q1-A kênh wake lệch 3 port | vifast-sync dùng chung cho mọi arch |

---

## 7. Vá dòng ViScript

### 7.1 Chốt từ vựng (đầu vào cho SPEC-lex.md)

| Mục | Quyết định |
|---|---|
| Chuẩn hoá | **Toàn bộ tệp nguồn được chuẩn hoá NFC trước khi lex.** 'ố' gõ kiểu nào cũng là một scalar |
| Thụt lề | Đầu vào: thụt lề nhất quán bất kỳ, đơn vị suy từ cấp đầu; đầu ra `vis fmt` luôn 4. Tab: lexer báo cảnh báo-lỗi VS0107, coi tab = 4, **vẫn lex tiếp**; `vis check` coi là lỗi, `vis fmt` sửa |
| Ca biên thụt lề | Dòng trống/chỉ chú thích không sinh INDENT/DEDENT; chuỗi nhiều dòng nuốt trước khi tính cột; trong `() [] {}` tắt INDENT; DEDENT ngầm ở EOF và cuối mỗi input REPL |
| Số | Float phải có chữ số sau `.` (`1.` sai); `1..2` là range; `_` chỉ giữa hai chữ số; literal tràn i64 là lỗi biên dịch |
| Đơn vị size | `b kb mb gb tb kib mib gib tib`, không dấu cách |
| Đơn vị duration | `ns us ms s m h d w`; ghép giảm dần là một token (`2h30m`) |
| Phần trăm | **Bỏ literal `50%`**, dùng `50pct`. `%` chỉ là modulo. Job spec dùng `job 1` |
| Chuỗi | `"..."` có escape + nội suy; `r"..."` không escape, không nội suy; `"""..."""` có cả hai; `r"""..."""` thô; `b"..."` chỉ ASCII + `\xNN`; `p"..."` là path |
| Escape | `\n \t \r \0 \\ \" \' \{ \} \u{X..X}`; `\xNN` chỉ trong `b""` |
| Nội suy | `{expr}` hoặc `{expr:spec}` (spec kiểu Rust format); `{{` `}}` là ngoặc thật |
| Chú thích | Script: `#` ngoài chuỗi luôn là chú thích. Lệnh: `#` chỉ là chú thích khi đứng đầu từ (sau khoảng trắng hoặc đầu dòng); `https://x/y#frag` giữ nguyên |
| `?` | Maximal munch: `??`, `?.`, `?`; `x??y` là `x ?? y` |
| `\|` | Biểu thức: không có nghĩa. Pattern: or-pattern. Lệnh: pipe ở độ sâu ngoặc 0. `\|>` pipe script |
| `set{` | `set` dính liền `{` là set literal; có khoảng trắng là định danh |
| Cột chẩn đoán | Đếm theo Unicode scalar, bắt đầu từ 1; LSP đổi sang UTF-16 ở biên |

### 7.2 Chế độ lệnh

- Metachar: khoảng trắng, `( ) [ ] { } ; & $ ' " |`, `#` ở đầu từ. Từ trần là chuỗi liên tục không chứa metachar.
- `"..."` có nội suy, `'...'` thô. `$x` biến phiên, `$env.NAME` biến môi trường.
- `;` tách pipeline; `&` nền chỉ khi là token cuối có khoảng trắng phía trước.
- `--` kết thúc cờ; `-5` là số nếu tham số kiểu số, ngược lại là cờ.
- Thân closure-arg kéo tới `|` kế tiếp ở độ sâu 0; or-pattern trong một stage phải đặt trong ngoặc.
- `.vish` và REPL: `shell_item := use | let | fn | cmd | pipeline`.
- `from json` không tham số trả `Json`; `where` trên `Json` là lỗi kiểu có gợi ý `from json[T]`.
- SIGPIPE/huỷ sớm do `take` không phải lỗi. `| complete` chặn lỗi pipeline và trả `{stdout, stderr, code, how}`.

### 7.3 Ngữ nghĩa lõi

| Mục | Quyết định |
|---|---|
| `\|` or-pattern | Giữ `\|` trong pattern; sửa Q8 thành "`\|` không bao giờ là toán tử nhị phân trong biểu thức" |
| Bit | Thêm `bit_not`, `ushr`; `shr` là dịch số học; số đếm ≥ 64 panic; có `checked_shl/shr` |
| `%` | Dấu theo số bị chia; `%0` panic; có `rem_euclid`, `mod_floor` |
| `**` | Kết hợp phải, cao hơn `*`; mũ âm với int panic; có `checked_pow` |
| Bảng ưu tiên (cao xuống thấp) | postfix (`. ?. () [] ? with`) → `**` → `-` một ngôi → `* / %` → `+ -` → `.. ..=` → `??` → so sánh (không kết hợp) → `not` → `and` → `or` → `\|>` |
| Float | NaN: `==` trả false; float không có Eq/Ord/Hash; kiểu chứa float không derive được Eq/Hash; dùng `total_cmp` |
| Số hệ thống | Chấp nhận bảng D điểm 3 (size, duration, datetime, percent) |
| `percent` | So sánh được với float theo tỉ lệ (`50pct == 0.5`); `ps.cpu` có kiểu `percent` |
| Optional | Tự nâng `T → T?` ở gán, trả, đối số; `T??` là lỗi kiểu; `?` dùng được trên `T?` trong hàm trả `U?`; pattern `none`, `some(x)`, `ok(v)`, `err(e)` |
| Xét hết | Nhánh có guard không tính; kiểu vô hạn bắt buộc `_` |
| Lỗi | `trait Error: Show { message, source, span }`; `AnyError` là kiểu xoá; mọi `E: Error` tự chuyển sang `AnyError` qua `?`; `main -> Result[(), AnyError]`; tên phía plugin Rust là `vis::Error` |
| Mã thoát | 0 ok, 1 lỗi từ main, 2 lỗi dùng/kiểu, 101 panic, 126 không chạy được, 127 không tìm thấy, 130 bị huỷ |
| Scope | Che tên được ở scope trong; khai lại cùng scope là lỗi; `and/or` ngắn mạch; đánh giá trái sang phải |
| Chuỗi | **Cấm `s[i]`**; dùng `.chars() .graphemes() .bytes() .byte_at(i)?`; `byte_len()`, `char_count()`, `grapheme_count()`; so sánh theo dãy scalar |
| Index | Thêm `xs[i]?` trả `T?`; `xs[-1]` panic có thông điệp |
| panic | Payload là `AnyError`; ViShell bọc **mỗi lệnh** trong task có arena riêng; panic giải phóng arena, đặt mã 101, giữ biến/cwd/job/lịch sử |
| Tài nguyên | Tài nguyên do lệnh mở là cap của phiên, đăng ký vào arena lệnh, tự đóng khi arena giải phóng; không thêm `defer` ở bản đầu |
| `print` | stdio của chính tiến trình không phải capability; `sh "..."` cần `cap[Proc]` |
| Plugin | Chế độ hạn chế: plugin động tắt; plugin tĩnh chỉ trong bản `vis` người dùng tự build |
| Consent | Nhớ theo **hash closure** (nguồn + phụ thuộc + phiên bản compiler), dùng chung hash cache |
| async | `Future[T]`, `Task[T]`; gọi `async fn` tạo Future lười; `s.spawn(fut) -> Task[T]`; panic trong scope huỷ anh em rồi lan lên |
| Generic vs index | `name[types](args)`: chỉ thử danh sách kiểu khi ngay sau `]` là `(` và tên là hàm generic đã biết; ngược lại là index |
| Từ khoá | Thêm `const`; bỏ `as`; `test` là từ khoá theo ngữ cảnh; module đổi tên `std.testing` |
| Chuyển đổi | `str(x)` = `x.show()`; path từ chuỗi phải tường minh `path("...")` hoặc `p"..."`; tham số mặc định `path = p"."` |
| `str * int` | Bỏ khỏi ví dụ; dùng `s.repeat(n)` |

### 7.4 Phụ lục A, Phụ lục B, VT09

- **Phụ lục A viết lại thành EBNF tự chứa** theo B §9 + quyết định trên (ST-02a). Cổng "**mọi khối mã trong tài liệu parse được**" (ngoại lệ phải khai báo).
- Sửa Phụ lục B: xuống dòng `|>` bọc trong ngoặc hoặc cho phép `|>` đầu dòng tiếp nối (**chốt: cho phép dòng bắt đầu bằng `|>` nối với dòng trước**), `str(e.path)` hợp lệ theo 7.3, `time.now()` thay `now`, thống nhất `is_file()`, đổi nhãn fence `python` thành `vis`, bỏ `>` đầu dòng (dùng nhãn fence `vish-session`).
- VT09 mới: 1 fixture cho mỗi khối mã trong tài liệu + fixture viết tay cho mỗi quyết định ở 7.1..7.3; cấm `...`.

### 7.5 Queue ViScript (sửa theo B §11, C)

| Task | Thay đổi |
|---|---|
| ST-01 | Workspace + agent/ (không ghi đè) + SPDX theo loại tệp + luật 1.500 dòng/unsafe có case âm + `vis-host-vifast` **không nằm trong viscript/** |
| ST-02a | SPEC-lex.md + EBNF tự chứa + bảng ưu tiên + 60 golden token + 30 golden biểu thức. **Chặn ST-04..ST-09**. Trạng thái cuối REVIEW |
| ST-02b | Sổ mã lỗi (VS + 4 chữ số, quy tắc cấp) |
| ST-03 | vis-diag (cột theo scalar) |
| ST-04 | Lexer theo SPEC-lex (sau duyệt) |
| ST-05 | Parser + CST lossless; tiêu chí: 100 % fixture tài liệu parse, mỗi tệp lỗi cho đúng lỗi đầu ở dòng ghi trong fixture |
| ST-06 | CmdCST + golden cho từng hàng bảng 9.4 + 10 ca mơ hồ (B §10.2) |
| ST-07 | fmt: lossless, idempotent, sửa tab |
| ST-08 | Runner chỉ `--check` cú pháp (chưa interpreter) |
| ST-09 | Fixture theo 7.4 |
| ST-10 | Báo cáo `R1-viscript` |

---

## 8. Danh sách D2 (thay D-09..D-27)

D2-01 v1 lưu trữ ở `v1-archive`, sổ lỗi mang sang (mục 6). D2-02 không sửa mò (D11 cũ). D2-03 hồi quy thì revert (D18 cũ). D2-04 core không chứa FS/fd/block/console/args/demo; thay đổi ABI qua Agent 0. D2-05 commit chạm `kernel/`, `lib/` phải xanh `--fast`; lỗi logic tái hiện trên sim, kèm `.scn` đỏ trước. D2-06 luật cấu trúc là cổng cứng. D2-07 không retry ở mọi cổng; flake là T-INFRA. D2-08 test kernel không nằm trong boot sản phẩm. D2-09 trace luôn bật, không có cờ tắt. D2-10 chỉ riscv64 tới K6. D2-11 một nguồn sự thật trong `spec/`. D2-12 đồng thời theo S1..S4 theo số đo; đọc trường công bố của hart khác được phép. D2-13 kịch bản `.scn` chạy cả sim và QEMU. D2-14 object trong Untyped qua objmem (API closure, Miri từ đầu). D2-15 ABI đóng băng 5 thứ + version + golden (mục 4). D2-16 MCS: donation từ K1b, thuật toán refill sau K6. D2-17 unsafe chỉ trong danh sách crate đóng. D2-18 ứng dụng hệ thống viết bằng Rust std, crate thuần Rust; libc, wasm để sau. D2-19 ViScript là ngôn ngữ script và shell chính; Linux qua máy ảo. D2-20 OS trước, ViScript khi không còn task OS READY. D2-21 giấy phép MIT OR Apache-2.0, kiểm bằng `cargo-deny` (allowlist mục 9.3), `THIRD-PARTY.md` sinh tự động, license nhúng vào ảnh, không tạo NOTICE. D2-22 duyệt spec bằng dòng APPROVED; Agent 2 soạn nháp spec ở trạng thái REVIEW, sau duyệt chỉ Agent 0 sửa. D2-23 D-08 còn hiệu lực tới K6. D2-24 mọi soak trên máy ps, N theo $$n \ge 3/p$$.

---

## 9. Câu hỏi mở dossier Q1..Q12

| Câu | Trả lời |
|---|---|
| Q1 mục tiêu gần | K0 → K0.5 → K1a; không hứa ngày |
| Q2 chiến lược lỗi hiếm | Sim tất định + mô hình; soak chỉ xác nhận |
| Q3 T33a | Không sửa trên v1; mang sang v2 bằng kịch bản (mục 6) |
| Q4 worker C | Thành kịch bản đơn điệu đồng hồ |
| Q5 T26 | KT-T26 trên host với vifast-fs |
| Q6 --strict | Thay bằng D2-07 |
| Q7 tách lib.rs | Không; v2 viết mới |
| Q8 trần tĩnh | Object trong Untyped; cổng `.data+.bss ≤ 256 KiB`; `[GT]` đo ở K0.5 |
| Q9 đóng băng x86 | x86 sau K6 (D2-10) |
| Q10 archive tài liệu | Có: `docs/agent2-report-*`, `docs/overnight-log.md`, `docs/decisions/*` chuyển vào `docs/v1/`; tài liệu v2 ở `docs/v2/`, ViScript ở `docs/viscript/` |
| Q11 định nghĩa xong | Mỗi mốc có cột tiêu chí; "tương đương v1" có danh sách đo được (mục 5.1) |
| Q12 nguồn sự thật số liệu | `STATUS.md` sinh bởi nightly; tài liệu thiết kế không chứa số đo viết tay, chỉ số `[TK]` hoặc `[ƯL]` |

### 9.3 Giấy phép (chấp nhận D điểm 8)

Allowlist: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, 0BSD, Zlib, Unicode-3.0, Unicode-DFS-2016. Cấm MPL, GPL, LGPL. SPDX: `//` cho Rust/asm, `#` cho `.vis .vish .vidl .toml`, tệp không có chú thích dùng sidecar `.license`, tệp sinh do generator phát header. Thêm `license.workspace = true` cho mọi crate (sửa vifast-maint). LICENSE-MIT/APACHE chỉ một bản ở gốc repo.

---

## 10. Việc giao Agent 1 (vòng vá)

Agent 1 có quyền đọc repo và đã có số dòng, nên làm phần cơ học:

1. Vá hai tài liệu theo tệp này, chuyển vào `docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/viscript/ViScript-thiet-ke.md`; ghi mục "Thay đổi sau review" ở đầu mỗi tài liệu.
2. Viết `docs/viscript/SPEC-lex.md` và Phụ lục A mới theo mục 7 (có thể giao làm ST-02a cho Agent 2 nếu Agent 1 thấy quá dài; khi đó ST-02a chặn ST-04..09).
3. Tạo tệp điều khiển theo checklist C §7 (B1..B16) với các sửa ở tệp này: `AGENTS.md` (gốc, v2/, viscript/), hai bộ `agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md`, `JOURNAL/`, `reports/`. QUEUE tự chứa 5 khối (luật, ngân sách, DoD, lệnh cổng, schema báo cáo), cột Trạng thái `READY|BLOCKED|REVIEW|DONE`.
4. QUEUE v2 vòng 1 tối thiểu: KT-01 (v1-archive + KNOWN-ISSUES), KT-02 (khung, không ghi đè), KT-03 (cổng `--fast` khung, chưa gen-check), KT-04a..d, KT-05 (gen + gen-check), KT-06 (agentctl), **KT-07 K0.5 boot riscv64**, KT-08 sim harness store giả + scheduler tối thiểu một hart, KT-09 Untyped trên sim, KT-10 CNode/MDB test-first với mô hình tối giản trong test, KT-11 mô hình tham chiếu Untyped/CNode, KT-12 traceformat (trước sim dump), KT-T26, KT-R1 (báo cáo phủ task đã mở + `blocked[]`). Mỗi task ≤ 3 giờ; task lớn hơn tách.
5. Gửi lại Agent 0 bản diff tài liệu + hai QUEUE trước khi giao Agent 2.

Nếu khi vá Agent 1 thấy một quyết định ở đây mâu thuẫn với code hoặc dossier, ghi vào `BLOCKED.md` của vòng vá kèm dẫn chứng, **không tự chọn**.
