# GÓI TỔNG HỢP GỬI AGENT 0 — vòng vá 2026-11-12

Tệp này gộp toàn bộ thứ cần duyệt vào **một** Markdown. Không cần mở tệp khác.
Người gửi: Agent 1. Người duyệt: Agent 0. Nguồn sự thật: `docs/QUYET-DINH-sau-review.md` (QD).

## 0. Cách đọc (30 giây)

1. Đọc §1 (đã làm gì) và §2 (3 điểm cần xác nhận) — đây là phần cần anh quyết.
2. Muốn xem thay đổi: PHẦN A (ViFastOs) và PHẦN B (ViScript) là **unified diff** so với bản anh gửi (`review/src/`), có 3 dòng ngữ cảnh mỗi hunk.
3. Đặc tả mới: PHẦN C (`SPEC-lex.md` toàn văn) + Phụ lục A nằm trong diff ViScript.
4. Task/duyệt: PHẦN D, E (2 QUEUE) — nơi anh ghi `APPROVED`.
5. Quyết định: PHẦN F, G. Sổ lỗi v1: PHẦN H. Kiểm tra: PHẦN I. Luật chơi: PHẦN J.

## 1. Đã làm gì

| Hạng mục | Kết quả |
|---|---|
| `docs/v2/ViFastOs-v2-thiet-ke.md` | 1.131 dòng; D2-01..24; 17 task KT; **syscall 14 Exit(code)**; 19 KError; slot cap 48 B; objmem API closure; WaitAddr bảng băm; id theo cụm; roadmap K0→K0.5→K1a→K1a'→K1b→K1c→K2→K3→K3.5→K6→K4'/K5'; Phụ lục A (sổ lỗi) + B (Q1–Q12) |
| `docs/viscript/ViScript-thiet-ke.md` | 1.457 dòng; NFC toàn tệp; bỏ `50%`→`50pct`; bảng ưu tiên 12 mức; hai pha `CmdCST→cmd_lower→HIR`; **Phụ lục A EBNF tự chứa ~122 luật**; ST-01..10; VD-01..22; Phụ lục B sửa ví dụ |
| `docs/viscript/SPEC-lex.md` | 180 dòng: bảng token, escape/nội suy, đơn vị size/duration, maximal munch, thụt lề, chế độ lệnh, 10 mã VS01xx, golden test |
| `v2/agent/QUEUE.md` | 17 dòng: KT-01..KT-12, KT-T26, KT-R1; 5 khối tự chứa; trạng thái `READY/BLOCKED/REVIEW/DONE` |
| `viscript/agent/QUEUE.md` | 11 dòng ST-01..ST-10 (ST-02a tách); ST-02a REVIEW chặn ST-04..ST-09 |
| Điều khiển | `AGENTS.md` + `v2/AGENTS.md` + `viscript/AGENTS.md`; 2 bộ `agent/{QUEUE,STATE,BLOCKED,DECISIONS}`; `JOURNAL/`, `reports/` |
| Lưu trữ v1 | tag `v1-archive` (KT-01); 104 tệp `docs/agent2-report-*`, `overnight-log.md`, `decisions/` đã `git mv` sang `docs/v1/`; `KNOWN-ISSUES.md` 7 mục |
| Tài liệu | `docs/README.md` thêm mục 2a trỏ v2/viscript; `docs/QUYET-DINH-sau-review.md` giữ nguyên |

**Chưa commit.** Diff thô cũng có ở `review/work/*.diff` nếu cần bản riêng.

## 2. Ba điểm cần Agent 0 xác nhận (mình không tự chọn)

1. **ST-02a đã đổi nghĩa**: `SPEC-lex.md` + Phụ lục A do **Agent 1 soạn** (2 subagent phụ trách hết output token). Vì vậy ST-02a không còn "viết từ đầu" mà là **duyệt + hoàn thiện + 60 golden token/30 golden biểu thức**, vẫn `REVIEW` và vẫn chặn ST-04..ST-09. Đã sửa đồng bộ trong tài liệu ViScript (§19, §20.2) và `viscript/agent/QUEUE.md`.
2. **KT-04c cần mô hình nhưng mô hình (KT-11) ở sau**: đã chốt trong QUEUE là 20 kịch bản `.scn` + **5 ca chạy trên mock CNode/Untyped tối giản trong test**; KT-11 chạy lại đủ 20 ca. Nếu anh muốn đảo KT-11 trước KT-04c thì báo.
3. **`Tcb.ReadState.exit_code`**: hiểu là kernel lưu mã thoát khi `Exit` để procsrv đọc. Nếu anh muốn procsrv tự lưu thì bỏ field này khỏi ABI; không ảnh hưởng chỗ khác.

Hai điểm nhỏ đã ghi trong tài liệu, không cần quyết nếu anh đồng ý: (a) Exit fault dùng **Send, không Reply** (khác Fault thường); (b) `cargo-deny` mới là **kế hoạch** trong KT-03/ST-01, chưa cài.

## 3. Đề nghị duyệt

- Duyệt nội dung 2 tài liệu + `SPEC-lex`/Phụ lục A.
- Ghi đúng một dòng vào mỗi QUEUE:
  - `v2/agent/QUEUE.md` (dưới KT-04d): `APPROVED <YYYY-MM-DD> Agent 0`
  - `viscript/agent/QUEUE.md` (dưới ST-02a): `APPROVED <YYYY-MM-DD> Agent 0`
- Sau APPROVED mới mở KT-05 và ST-04..ST-09. Trước đó chỉ chạy task READY: KT-01, KT-02, KT-04a, KT-T26, ST-01, ST-02b, ST-10.

## 4. Kết quả kiểm tự động

- OS: 24/24 D2, 17/17 KT, 15/15 syscall + Exit, 19 mã lỗi, slot 48 B, K0.5/K3.5, 0 task T cũ, phụ lục Q1–Q12 + sổ lỗi.
- ViScript: 11 ST, 22 VD, 8 Q, 122 luật EBNF, `50pct`, CmdCST, SPEC-lex 180 dòng/10 mã.
- Điều khiển: 17/17 + 11/11 task, 24 D2 + 8 D-0x, KNOWN-ISSUES 7/7, 105 tệp lưu trữ, 3 AGENTS.

---


# PHẦN A — UNIFIED DIFF: ViFastOs v2

So với bản Agent 0 gửi. Dòng `+` thêm, `-` bỏ, `@@` là vị trí. Toàn bộ nằm trong khối `text` để không phá Markdown.

````text
--- review/src/ViFastOs-v2-thiet-ke.md	2026-11-12 14:34:15
+++ docs/v2/ViFastOs-v2-thiet-ke.md	2026-11-12 15:45:08
@@ -5,8 +5,20 @@
 Tài liệu liên quan: `ViScript-thiet-ke.md` (ngôn ngữ script và shell, sản phẩm độc lập chạy đa nền tảng).
 Giấy phép mã nguồn: **MIT OR Apache-2.0** (giống Rust).
 
-Nhãn: [DOSSIER] lấy từ hồ sơ v1; [GT] giả thuyết cần kiểm; [ƯL] con số ước lượng, phải đo lại.
+Nhãn (QD §2.2): `[V1:CODE]`, `[V1:ĐO]`, `[V1:DOC]` = lấy từ dossier, giữ độ tin cậy gốc; `[TK]` = thiết kế mới (**mặc định** cho dòng không nhãn); `[GT]` = giả thuyết cần kiểm; `[ƯL]` = số ước lượng phải đo lại. Hằng số thiết kế là `[TK]`, không phải `[ƯL]`.
 
+## Thay đổi sau review (2026-11-12)
+
+Bản này đã vá theo `docs/QUYET-DINH-sau-review.md` (QD). Các nhóm thay đổi:
+1. Không gian tên: D-09..D-27 → **D2-01..D2-24**; G1..G6 → **R1..R6**; task T01.. → **KT-##**; báo cáo `R<n>-v2`.
+2. ABI: syscall 0..13 + **14 `Exit(code)`**; dải 15..31 reserved; 19 mã lỗi; thêm `Tcb.ReadState/Suspend(mode)/SetFaultEp`; object chưa cài giữ số reserved; `abi_version`, `min_compat`, golden file.
+3. Kernel: cờ `suspended` + `Suspend(Abort/Defer)`; `Resume` không phát lại IPC; `FaultKind::Exit`; objmem API closure; slot cap 48 B; WaitAddr bảng băm; id theo cụm; MCS giai đoạn đầu.
+4. Tích hợp: U1 không hứa net (net ở N1); VSH2 phụ thuộc N1; shell sở hữu tty; `term.vidl` thành mốc `TUI`.
+5. Lộ trình không ngày: K0 → K0.5 → K1a → K1a' → K1b → K1c → K2 → K3 → K3.5 → K6 → K4'/K5'; chỉ riscv64 tới K6.
+6. Đo lường: N=1.000 trên máy `ps`, 3 luồng; bỏ 750; harness-v2; `--fast` L1 1.000 seed.
+7. v1: tag `v1-archive`, không soak lại; sổ lỗi mang sang v2 (Phụ lục A).
+8. Quyết định/Q&A: D2-01..D2-24 (mục 12); Q1–Q12 của dossier (Phụ lục B).
+
 ---
 
 ## Mục lục
@@ -27,6 +39,9 @@
 13. Giao việc vòng 1
 14. Rủi ro
 
+Phụ lục A. Sổ lỗi v1 mang sang v2
+Phụ lục B. Q1–Q12 của dossier
+
 ---
 
 ## 0. Kết luận chính
@@ -48,21 +63,23 @@
 
 ---
 
-## 1. Chẩn đoán v1 [DOSSIER]
+## 1. Chẩn đoán v1 [V1:DOC]
 
 | # | Lỗi gốc | Triệu chứng | Hướng v2 |
 |---|---|---|---|
-| G1 | 58 syscall, ramfs, fd, block, console, args trong kernel; kernel dựng VSpace cho server | lib.rs 17.715 dòng | 14 syscall + invocation; dịch vụ ra user |
-| G2 | Trạng thái static reset hai lần; đồng hồ nằm trong trạng thái reset được | T33a: thời gian lùi 49 về 8 | Đồng hồ phần cứng; `init` một lần |
-| G3 | Trạng thái TCB tách khỏi hàng đợi; message chờ trong ô `resume` | Mất message r319; hàng 8 phần tử đầy; O(n) | Trạng thái mang object chờ; danh sách xâm nhập; ghi thẳng context |
-| G4 | COPY_WINDOWS, read_user/write_user | Lỗi exec vòng 112 tới 145 | Không con trỏ user trong kernel |
-| G5 | Identity map nhỏ, pool trong .data | Trần 566 KB, 24 TCB, tràn .data AArch64 | Nửa cao + physmap; object trong Untyped |
-| G6 | Test = boot demo + grep marker | 11 vòng cho một lỗi | Sim tất định, mô hình, QEMU TAP |
+| R1 | 58 syscall [V1:CODE], ramfs, fd, block, console, args trong kernel; kernel dựng VSpace cho server | lib.rs 17.715 dòng [V1:CODE] | 15 syscall (0..14) + invocation; dịch vụ ra user |
+| R2 | Trạng thái static reset hai lần; đồng hồ nằm trong trạng thái reset được | T33a: thời gian lùi 49 về 8 | Đồng hồ phần cứng; `init` một lần |
+| R3 | Trạng thái TCB tách khỏi hàng đợi; message chờ trong ô `resume` | Mất message r319; hàng 8 phần tử đầy; O(n) | Trạng thái mang object chờ; danh sách xâm nhập; ghi thẳng context |
+| R4 | COPY_WINDOWS, read_user/write_user | Lỗi exec vòng 112 tới 145 [V1:ĐO] | Không con trỏ user trong kernel |
+| R5 | Identity map nhỏ, pool trong .data | Trần 566 KB, 24 TCB, tràn .data AArch64 [V1:ĐO] | Nửa cao + physmap; object trong Untyped |
+| R6 | Test = boot demo + grep marker | 11 vòng cho một lỗi [V1:DOC] | Sim tất định, mô hình, QEMU TAP |
 
 Phụ: Untyped chỉ là quota (object thật nằm trong pool static, hai trần chồng nhau); SMP không có giao thức thống nhất (kênh wake lệch 3 port, poll_maintenance khác nhau, lỗ shootdown AP->BSP).
 
-Giữ từ v1: mô hình seL4, `forbid(unsafe_code)` cho core, check_invariants + mutation, hợp đồng marker có case âm, BUILD_ID, W^X theo Frame, văn hoá test đỏ trước; thư viện vifast-pte, vifast-elf, vifast-acpi, vifast-fs, vifast-fsutil, vifast-virtio, ý tưởng hộp thư của vifast-maint.
+Mã lỗi gốc đổi **G1..G6 → R1..R6** (QD §2.1): chữ **G** để dành cho mốc desktop (G1..G5, mục 10.4), tránh trùng mã khi grep.
 
+Giữ từ v1: mô hình seL4, `forbid(unsafe_code)` cho core, check_invariants + mutation, **cơ chế marker (kế thừa dưới dạng `harness-v2`, mục 9.12)**, BUILD_ID, W^X theo Frame, văn hoá test đỏ trước; thư viện vifast-pte, vifast-elf, vifast-acpi, vifast-fs, vifast-fsutil, vifast-virtio, ý tưởng hộp thư của vifast-maint.
+
 ---
 
 ## 2. Nguyên tắc
@@ -91,10 +108,26 @@
 
 | Làm ngay | Khai báo ABI, cài sau | Để sau |
 |---|---|---|
-| Object trong Untyped, CSpace 2 cấp, IPC 4 cap, Reply, MCS, Frame nhiều cỡ + attrs, WaitOnAddress, thao tác làm tiếp, FPU eager, đồng hồ user, hộp thư, đẩy cân bằng tải | IOSpace, MSI, VCpu + VSpace khách, KPTI, power hook, SetTls (cài ở K6) | IPC fastpath, cụm (S3), khoá mịn (S4), libc, wasm |
+| Object trong Untyped, CSpace 2 cấp, IPC 4 cap [TK], Reply, MCS phần **donation** (mục 4.5), Frame nhiều cỡ + attrs, WaitOnAddress, thao tác làm tiếp, FPU eager, đồng hồ user, hộp thư, đẩy cân bằng tải | IOSpace, MSI, VCpu + VSpace khách, KPTI, power hook, SetTls (cài ở K6), phần hoãn của MCS (refill nhiều đoạn, timeout fault) | IPC fastpath, cụm (S3), khoá mịn (S4), libc, wasm |
 
 ---
 
+### 2.3 Ràng buộc v1 còn hiệu lực (QD §2.3)
+
+| v1 | Trạng thái trong v2 |
+|---|---|
+| D-01 gate mềm | thay bằng D2-07 (không retry) cho v2; còn hiệu lực cho v1 |
+| D-02 đóng băng tính năng | **hết hiệu lực** từ khi v1 lưu trữ (`v1-archive`) |
+| D-03 T26 "ai chen giữa" | chuyển sang v2: **KT-T26** |
+| D-04 `MAX_IO=256` khi đo | giữ cho KT-T26 (cấu hình tái hiện) |
+| D-05 teardown ≤2 chỗ/vòng | giữ tinh thần: finalise theo nhóm, mỗi task ≤2 loại object, mỗi loại có test |
+| D-06 soak 600..1000 trên máy `ps` | giữ: mọi soak v2 trên `ps`, N theo mục 9.12 |
+| D-07 không tách `lib.rs` | hết đối tượng (v2 viết mới) |
+| D-08 không đụng đồ hoạ/ViFastKey | **còn hiệu lực tới K6** |
+| D11 không sửa mò | D2-02 |
+| D18 hồi quy thì revert | D2-03 |
+| Quy tắc 2.5 | giữ nguyên văn: hai phiên kẹt cùng ý thì đổi nhánh, không thử lần ba |
+
 ## 3. Kiến trúc tổng thể
 
 ```
@@ -162,29 +195,30 @@
                                                                  └ mọi object kernel nằm ở đây
 ```
 
-**vifast-objmem** (crate duy nhất có unsafe liên quan object, mục tiêu dưới 800 dòng):
+**vifast-objmem** (crate duy nhất có unsafe liên quan object, mục tiêu dưới 800 dòng [ƯL]):
 
 ```rust
-#[repr(C)] pub struct ObjHeader { kind: ObjKind, state: u8, _pad: u16, refs: u32, id: u64 }
+#[repr(C)] pub struct ObjHeader { kind: ObjKind, state: LifeState, _pad: u16, id: u64 } // không còn `refs`
 pub struct ObjRef<T> { pa: PAddr, id: u64, _t: PhantomData<T> }
 
 pub trait ObjStore {
-    fn get<T: KObj>(&self, r: ObjRef<T>) -> Result<&T, Dead>;
-    fn get_mut<T: KObj>(&mut self, r: ObjRef<T>) -> Result<&mut T, Dead>;
-    fn get2_mut<A: KObj, B: KObj>(&mut self, a: ObjRef<A>, b: ObjRef<B>) -> Result<(&mut A, &mut B), Alias>;
+    fn with<T: KObj, R>(&self, r: ObjRef<T>, f: impl FnOnce(&T) -> R) -> Result<R, Dead>;
+    fn with_mut<T: KObj, R>(&mut self, r: ObjRef<T>, f: impl FnOnce(&mut T) -> R) -> Result<R, Dead>;
+    fn with_many_mut<const N: usize, R>(&mut self, rs: [ObjRef<dyn KObj>; N],
+        f: impl FnOnce([&mut dyn KObj; N]) -> R) -> Result<R, Alias>; // N ≤ 8, kiểm từng cặp
     unsafe fn place<T: KObj>(&mut self, pa: PAddr, v: T) -> ObjRef<T>;   // chỉ Retype gọi
     unsafe fn destroy<T: KObj>(&mut self, r: ObjRef<T>);                  // chỉ finalise gọi
 }
 ```
 
-- `id` u64 tăng đơn điệu toàn cục, không lặp lại. Truy cập bằng `ObjRef` có `id` khác header là `Dead`.
-- `get2_mut` cho thao tác chạm hai object (IPC giữa hai TCB) mà không vi phạm aliasing; cùng địa chỉ trả `Alias`.
-- Trên máy thật đi qua physmap; trên sim đi qua `Vec<u8>` căn lề. **Cùng một code unsafe**, kiểm bằng Miri.
+- `id = (cluster_id << 56) | counter` của cụm; duy nhất, không tái dùng. Truy cập bằng `ObjRef` có `id` khác header là `Dead`. Retype không phải đường nóng nên đếm dưới khoá cụm là đủ; sang S3 mỗi cụm đếm riêng.
+- `with_many_mut` thay `get2_mut`: IPC cần chạm ≥4 object (2 TCB + Endpoint + Reply + IPC buffer); trùng địa chỉ trả `Alias`. Cài đặt bằng con trỏ thô + `UnsafeCell`, **không** tạo tham chiếu tới cả vùng nhớ; sim cấp vùng bằng bộ cấp phát thô căn lề, không dùng `Vec<u8>`; Miri từ commit đầu.
+- Trên máy thật đi qua physmap; trên sim đi qua **bộ cấp phát thô có căn lề** (không `Vec<u8>` — QD §3.5). **Cùng một code unsafe**, kiểm bằng Miri.
 - Kích thước object là hằng số theo arch lúc build, riêng TCB = phần cố định + vùng FPU/vector tính lúc boot theo tính năng CPU (XSAVE size, SVE VL, RISC-V VLEN).
 
 **Untyped**:
 - Mỗi Untyped có `watermark` (bump). Retype lấy từ watermark, căn lề theo kích thước object.
-- Chỉ thu hồi khi **Revoke Untyped**: finalise mọi con, đặt watermark về 0, xoá trắng (làm tiếp được, mục 4.13).
+- Thu hồi bằng **`CNode.Revoke` trên cap Untyped** (không có method `Untyped.Revoke` riêng): finalise mọi con, đặt watermark về 0, xoá trắng (làm tiếp được, mục 4.13).
 - Untyped con: Retype Untyped thành Untyped nhỏ hơn để chia quyền sở hữu (memory server dùng).
 - Device Untyped: chỉ Retype thành Frame với attrs Device/UC; không chứa object kernel.
 
@@ -207,12 +241,14 @@
 
 Invariant sim: sau mỗi event, **không trường nào trong bất kỳ object sống nào trỏ tới object chết**.
 
+**Trần tĩnh [GT]**: lỗi v1 khi tách pool ra khỏi `.data` (3 lần, hỏng AArch64) đến từ pool tĩnh trong `.data`; v2 đặt object trong RAM Untyped nên cơ chế đó không áp dụng, **nhưng vẫn phải đo** ở K0.5 (kích thước image, `.data`/`.bss`, headroom) và giữ **cổng cứng `.data + .bss ≤ 256 KiB` [TK]** (mục 8).
+
 ### 4.3 CSpace và capability
 
-- **Hai cấp cố định**: địa chỉ cap 64 bit = `[L1 index | L2 index]`, kích thước mỗi cấp do CNode quyết định (2^n slot, n do người tạo chọn, tối đa 2^20 [ƯL]). CNode gốc chứa cap tới CNode lá. Đủ cho hàng triệu cap, không có guard.
-- **Slot cap** 32 byte [ƯL]: `{ objref, kind, rights, badge, mdb_prev, mdb_next, mdb_depth }`.
+- **Hai cấp cố định**: địa chỉ cap 64 bit = `[L1 index | L2 index]`, kích thước mỗi cấp do CNode quyết định (2^n slot, n do người tạo chọn, tối đa **2^20 slot [TK]**). CNode gốc chứa cap tới CNode lá. Đủ cho hàng triệu cap, không có guard.
+- **Slot cap 48 B** [TK]: `pa u64 | id u64 | badge u64 | mdb_prev u64 | mdb_next u64 | kind u8, rights u8, depth u16, flags u32`. Thêm test `size_of::<Slot>() == 48`.
 - **MDB** kiểu seL4: danh sách liên kết theo thứ tự cây dẫn xuất; Revoke xoá con theo thứ tự, làm tiếp được.
-- **Rights**: READ, WRITE, GRANT, GRANT_REPLY. EXEC và attrs là thuộc tính lúc map.
+- **Rights**: READ, WRITE, GRANT, GRANT_REPLY. **Cap không có bit EXEC — thay đổi có chủ ý so với v1** (QD §3.12): EXEC là **thuộc tính lúc map** (mục 4.10), kiểm bởi W^X theo Frame; attrs cũng là thuộc tính lúc map.
 - **Badge**: Mint endpoint/notification với badge 64 bit.
 - **Thao tác**: Copy, Mint, Move, Mutate, Delete, Revoke, Rotate (Rotate để server đổi Reply hiệu quả).
 - **Cap allocator ở user** (`vifast-capalloc`): quản lý slot trống trong CSpace của tiến trình, tự xin CNode lá mới từ memory server khi cần.
@@ -222,7 +258,7 @@
 ```rust
 pub enum ThreadState {
     Inactive,
-    Ready,                                         // trong ready queue của hart
+    Ready,                                         // trong ready queue của hart, chỉ khi !suspended
     Running { hart: HartId },
     BlockedOnSend  { ep: ObjRef<Endpoint>, call: bool },
     BlockedOnRecv  { ep: ObjRef<Endpoint>, reply: Option<ObjRef<Reply>> },
@@ -236,39 +272,63 @@
 ```
 
 - `transition(tcb, to)` duy nhất, tra `const ALLOWED`. Bảng xuất ra tài liệu và dùng sinh case âm.
-- Liên kết xâm nhập bằng `ObjRef` (crate `vifast-ilist`, không unsafe vì đi qua `ObjStore`). Invariant: TCB ở trong hàng của X khi và chỉ khi trạng thái mang X.
+- Liên kết xâm nhập bằng `ObjRef` (crate `vifast-ilist`, không unsafe vì đi qua `ObjStore`).
+- **Invariant (sửa)**: TCB ở trong hàng của X **khi và chỉ khi** trạng thái mang X **và `!suspended`**. `Ready + suspended` **không** nằm trong ready queue; `Abort` gỡ khỏi hàng **trước** khi bật cờ, nên bất biến vẫn kiểm được sau mỗi event.
 - Không ô `resume`: message ghi thẳng vào context + IPC buffer rồi mới chuyển Ready. Đường mất message kiểu r319 không biểu diễn được.
-- Trường TCB chính: context, FPU area, cspace_root, vspace, ipc_buffer (Frame), fault_ep, timeout_ep, sched_context, bound_notification, priority, max_controlled_priority, affinity (HartMask), home hart, tls.
+- **Cờ `suspended: bool` trực giao** với `ThreadState` (không thêm trạng thái mới). `Tcb.Suspend(mode)`:
+  - `Abort` (mặc định): nếu đang `BlockedOn*`, gỡ khỏi hàng chờ, syscall đang chờ trả `Cancelled`, ip đã tiến qua syscall, TCB thành `Ready + suspended`. Nếu đang `BlockedOnReply`, Reply object được tách khỏi caller: server `Send(reply)` sau đó **thành công nhưng reply bị bỏ**, để client bị dừng không làm hỏng đường lỗi của server.
+  - `Defer`: chỉ hợp lệ khi TCB đang trong thao tác dài có `Restart`; với IPC trả `InvalidArgument`.
+- `Tcb.Resume` xoá cờ. **Không bao giờ phát lại Call/Send/Recv/ReplyRecv**; `Restart` chỉ dùng cho thao tác dài (mục 4.13).
+- `Tcb.ReadState -> {state, suspended, blocked_on_kind, exit_code?}`; `Tcb.SetFaultEp` tách khỏi `Configure`.
+- `FaultKind::Exit{code}`: syscall `Exit` chuyển TCB `Inactive` rồi gửi fault tới `fault_ep` dạng **Send, không chờ Reply**; không có `fault_ep` thì chỉ `Inactive` (QD §1 Q-A).
+- Trường TCB chính: context, FPU area, cspace_root, vspace, ipc_buffer (Frame), fault_ep, timeout_ep, sched_context, bound_notification, priority, max_controlled_priority, affinity (HartMask), home hart, tls, **suspended**.
 
 ### 4.5 MCS: SchedContext
 
-- **SchedContext** object: `budget`, `period`, danh sách refill (sporadic server, tối đa K refill [ƯL K=8]), `badge`, hart.
-- TCB chỉ chạy khi có SchedContext còn budget. Hết budget: TCB vào hàng chờ refill; nếu có `timeout_ep` thì kernel gửi timeout fault.
+- **SchedContext** object: `budget`, `period`, danh sách refill là chi tiết cài đặt (tối đa **K = 8 refill [TK]**), `badge`, hart.
+- TCB chỉ chạy khi có SchedContext còn budget. Hết budget: TCB vào **`Sleeping` với `reason = Budget`** — **không thêm enum mới** (QD §3.4); `timeout_ep` và timeout fault thuộc phần **hoãn sau K6**.
 - **SchedControl** cap (mỗi hart một cái, chỉ init có): `Configure(sc, budget, period, refills)`.
 - **Passive server**: server không có SchedContext riêng. Khi client Call, SchedContext của client **đi theo** sang server; Reply trả về. Thời gian phục vụ tính cho client. Đảo ưu tiên được xử lý vì server chạy bằng SchedContext của client (MCP kiểm soát mức ưu tiên).
 - Notification có thể bind SchedContext để đánh thức passive driver.
-- Ưu tiên: 256 mức; `max_controlled_priority` ngăn tự nâng ưu tiên.
+- Ưu tiên: **256 mức [TK]**; `max_controlled_priority` ngăn tự nâng ưu tiên.
 - Kiểm sim: tổng thời gian chạy của mỗi SchedContext trong mọi cửa sổ `period` không vượt `budget` (cộng sai số tick); không TCB nào chạy khi SchedContext hết budget.
 
+**Giai đoạn đầu (QD §3.4):**
+
+| Có ở K1b | Hoãn sau K6 (không đổi ABI) |
+|---|---|
+| SchedContext object, `SchedControl.Configure`, budget/period đơn giản (nạp lại đầu mỗi chu kỳ), **donation qua Call/Reply**, passive server, bind Notification | Refill sporadic nhiều đoạn, timeout fault + `timeout_ep`, phân tích độ trễ audio |
+
+Trạng thái chờ ngân sách dùng `Sleeping` với `reason = Budget` (không thêm enum, QD §3.4). Bảng chuyển trạng thái liên quan ngân sách:
+
+| Từ | Sự kiện | Đến |
+|---|---|---|
+| `Running` | hết budget, còn refill kế | `Sleeping{reason = Budget}` |
+| `Sleeping{reason = Budget}` | timer refill kế tới hạn | `Ready` |
+| `Sleeping{reason = Budget}` | `Tcb.Suspend(Abort)` | `Sleeping{reason = Budget}` + `suspended` (không ở hàng chờ) |
+| `Sleeping{reason = Budget} + suspended` | `Tcb.Resume` | `Sleeping{reason = Budget}` (chờ refill; **không phát lại** gì) |
+
 ### 4.6 IPC
 
 - **Syscall**: Call, Send, NBSend, Recv, NBRecv, ReplyRecv, NBSendRecv, NBSendWait (hai cái cuối cho passive server và driver).
-- **MessageInfo** (một thanh ghi): `label: u20`, `caps: u3 (≤4)`, `extra_caps_unwrapped: u4`, `len: u7 (≤64 MR)`.
-- MR0 tới MR3 trong thanh ghi (riscv64/aarch64 có thể 6 [ƯL]), còn lại trong IPC buffer.
-- **IPC buffer** là Frame bind qua `Tcb.Configure`; kernel copy bằng `PhysMem`. Header chứa receive window (CNode + chỉ số + độ sâu cho tối đa 4 cap).
+- **MessageInfo** (một thanh ghi): `label: u20`, `caps: u3 (≤4)`, `no_reply: u1`, `extra_caps_unwrapped: u4`, `len: u7 (≤64 MR)`.
+- MR0 tới MR3 trong thanh ghi (riscv64/aarch64 có thể 6 [ƯL]), còn lại trong IPC buffer. 64 MR + 4 cap là thay đổi có chủ ý so với v1: compositor cần buffer + fence + vùng chia sẻ trong **một** message; MR ngoài thanh ghi nằm trong IPC buffer nên chi phí chỉ phát sinh khi dùng.
+- **IPC buffer** là Frame bind qua `Tcb.Configure`; kernel copy bằng `PhysMem`. Header chứa **receive window là CPTR** (địa chỉ cap giải qua CSpace của chính thread, không phải `ObjRef<CNode>`) cho tối đa 4 cap; kernel kiểm slot trống và quyền ghi trước khi cài.
 - **Chuyển cap**: cần GRANT; cap nhận bỏ GRANT; cap cùng endpoint với badge được "unwrap" thành badge (kiểu seL4). Chuyển thất bại không mất message, đặt cờ trong MessageInfo.
-- **Reply object**: `Recv(ep, reply)`; Call làm caller sang `BlockedOnReply{reply}`; server trả lời bằng `Send(reply)` hoặc `ReplyRecv`. Server giữ nhiều Reply, phục vụ xen kẽ nhiều client. Generation trong `id` thay cho reply_epoch của v1. MCS: SchedContext gắn với Reply khi server passive.
-- **Fault**: Call tới fault endpoint với Reply; trả lời Reply để tiếp tục, có thể kèm thanh ghi mới. Loại fault: VM, unknown syscall, user exception, timeout (MCS), VM exit (H1).
+- **Reply object do bên nhận cấp** (QD §3.2): `Recv(ep, reply)` và `ReplyRecv`; **bên gọi Call không cần và không cấp Reply**. Call làm caller sang `BlockedOnReply{reply}`; server trả lời bằng `Send(reply)` hoặc `ReplyRecv`. Server giữ nhiều Reply, phục vụ xen kẽ nhiều client. Generation trong `id` thay cho reply_epoch của v1. MCS: SchedContext gắn với Reply khi server passive. **Không có `Tcb.SetReplyObject`.**
+- **Fault**: Call tới fault endpoint với Reply; trả lời Reply để tiếp tục, có thể kèm thanh ghi mới. Loại fault: VM, unknown syscall, user exception, timeout (MCS), **Exit (Send, không Reply)**, VM exit (H1).
+- **Ca biên `no_reply`** (QD §3.2): Call gặp bên nhận đã `Recv` **không** có Reply ⇒ message vẫn giao, cờ `no_reply` bật trong MessageInfo phía nhận, caller **trả ngay `Cancelled`**. Kèm kịch bản sim.
+- **Invocation trả ngay**: Call tới object **không phải Endpoint** là invocation (method = label) — **không chặn, không sinh Reply**, kết quả trả trong MR (mục 4.21). Chỉ Endpoint mới đi đường IPC chặn.
 - **Một hàm lõi** `ipc::transfer`. Fastpath sau này phải gọi cùng hàm copy cap/MR hoặc được sim so vi sai.
 
 ### 4.7 Notification và WaitOnAddress
 
-**Notification**: word 64 bit; Signal OR badge, đánh thức một waiter, giao word, xoá word; Wait/Poll; bound notification với TCB và SchedContext; IrqHandler signal vào notification. Broadcast là việc của user space.
+**Notification** (giữ **nguyên ngữ nghĩa seL4**, QD §3.11): word 64 bit; Signal OR badge, đánh thức **một** waiter, giao word, **xoá word**; Wait/Poll; bound notification với TCB và SchedContext; IrqHandler signal vào notification. **Đây là thay đổi có chủ ý so với v1** (v1 đổi sang "đánh thức tất cả"): lý do v1 phải đổi là root đậu 4 worker trên **một** notification — **lỗi cách dùng**, không phải yêu cầu của giao diện. Broadcast = mỗi bên chờ một notification riêng, bên phát Signal từng cái (việc của user space).
 
-**WaitOnAddress** (futex):
-- `WaitAddr(frame_cap, offset, expected: u32|u64, timeout)`: kernel đọc giá trị tại (Frame, offset) qua physmap; bằng `expected` thì chặn, khác thì trả ngay.
-- `WakeAddr(frame_cap, offset, count)`: đánh thức tối đa `count` người.
-- Khoá theo **địa chỉ vật lý** (Frame + offset), nên hoạt động với bộ nhớ chia sẻ giữa tiến trình.
+**WaitOnAddress** (futex, QD §3.8):
+- Hàng chờ là **bảng băm trong arena kernel**, khoá `(frame_id, offset)`, số bucket theo `kernel.toml`; nút nằm trong TCB. Không có gì nằm trong trang của user.
+- `WaitAddr(frame_cap, offset, expected, timeout)`: đọc giá trị và xếp hàng **trong cùng một lần giữ khoá**; `WakeAddr` cũng giữ khoá, nên không mất đánh thức.
+- Khoá theo **địa chỉ vật lý** (Frame + offset), hoạt động với bộ nhớ chia sẻ giữa tiến trình.
 - Hàng chờ nằm trong **header Frame** (danh sách waiter sắp xếp theo offset; nâng lên cây khi có số đo). [GT: chi phí với Frame 2 MiB nhiều địa chỉ cần đo.]
 - Kiểm: không mất đánh thức (Wake sau khi giá trị đổi luôn đánh thức người đã Wait với giá trị cũ).
 
@@ -277,8 +337,8 @@
 - Mỗi hart: 256 mức `ilist` + bitmap; refill queue (MCS); timer heap theo deadline.
 - `pick` O(1). Lượng thời gian do SchedContext quyết định.
 - **Affinity** là HartMask; **home** là hart hiện tại.
-- **Đẩy cân bằng (push migration)**: khi hart X có > 1 TCB Ready cùng mức cao nhất và có hart Y trong affinity đang Idle, X đẩy một TCB sang Y qua hộp thư `Migrate`. Quyết định tất định (hart rảnh có chỉ số nhỏ nhất). Không work stealing (kéo) vì phải đọc trạng thái hart khác.
-- Wake chéo hart: hộp thư `Wake` + IPI nếu ưu tiên cao hơn current của đích.
+- **Đẩy cân bằng (push migration)**: mỗi hart **công bố** `idle: AtomicBool` và `cur_prio: AtomicU8` trong mảng căn cache line (QD §3.10). Khi hart X có > 1 TCB Ready cùng mức cao nhất và có hart Y trong affinity đang `idle`, X đẩy một TCB sang Y qua hộp thư `Migrate`. Quyết định tất định (hart rảnh có chỉ số nhỏ nhất). Không work stealing (kéo) vì phải **ghi** trạng thái hart khác; **đọc** trường công bố thì được phép (mục 4.16 luật 3).
+- Wake chéo hart: hộp thư `Wake` + IPI nếu ưu tiên cao hơn **`cur_prio` công bố** của đích.
 - Idle: `Decision::Idle(hint)`, platform chọn mức idle sâu theo deadline kế tiếp.
 - Đo: thời gian chờ khoá, độ dài ready queue, số migrate.
 
@@ -292,7 +352,7 @@
 ### 4.10 Bộ nhớ ảo
 
 - Object: VSpace, PageTable, Frame (`size_bits` 12/21/30), Device Frame.
-- `Frame.Map(vspace, va, rights, attrs)` với attrs: `WB`, `WC`, `UC`, `Device`, `NX`. Kernel chọn cấp lá theo kích thước Frame. Kernel không tự cấp page table.
+- `Frame.Map(vspace, va, rights, attrs)`: `rights` chỉ còn READ/WRITE; **EXEC là thuộc tính lúc map** (trong `attrs`), **không phải bit trong cap** (QD §3.12). attrs: `WB`, `WC`, `UC`, `Device`, `NX`, `EXEC`. Kernel chọn cấp lá theo kích thước Frame. Kernel không tự cấp page table.
 - W^X: một Frame vật lý không được map vừa W vừa X ở bất kỳ VSpace nào (bộ đếm mapped_w/mapped_x, giữ từ v1). JIT (nếu có sau này) phải unmap W rồi map X.
 - **Không gian kernel**: nửa cao + physmap (trang lớn, NX). RISC-V copy entry nửa cao vào root mới; AArch64 TTBR1; x86 copy PML4[256..511], hoặc KPTI. Logic copy nửa cao nằm trong `PageTableFormat` của vifast-pte, dùng chung.
 - **ASID** có generation (hết ASID thì tăng generation và flush toàn bộ), module chung; độ rộng theo `Mmu`.
@@ -316,11 +376,12 @@
 - Object `IOSpace` gắn với một thiết bị (PCI BDF hoặc stream ID). `Frame.MapIo(iospace, iova, rights)`.
 - Khai báo trong ABI từ K0; cài ở mốc D3: Intel VT-d trên QEMU `intel-iommu`, virtio-iommu, ARM SMMUv3; RISC-V IOMMU khi QEMU hỗ trợ ổn định [GT].
 - Máy không có IOMMU: memory server chỉ cấp Frame cho DMA từ một vùng riêng, driver DMA được đánh dấu tin cậy trong manifest. Đây là chính sách user.
+- **Threat model (QD §3.14)**: trước mốc **D3** và trên máy **không** có IOMMU, **mọi driver có DMA thuộc vùng tin cậy**; tuyên bố bảo mật ở mục 4.15 chỉ áp cho **CPU, không áp cho DMA**. Không làm lại software IOMMU (v1 đã bỏ).
 
 ### 4.13 Thao tác dài làm tiếp được
 
 - Áp dụng cho: Retype lớn (xoá trắng), Revoke, Delete CNode lớn, finalise VSpace, unmap vùng lớn.
-- Cơ chế chung `Preempt`: mỗi thao tác chia thành bước có cận; sau mỗi bước kiểm `p.pending_interrupt()`; nếu có, lưu tiến độ **trong object** (ví dụ `untyped.zero_cursor`, `cnode.delete_cursor`), trả `Restart` để platform quay lại chính lệnh syscall (đặt lại ip).
+- Cơ chế chung `Preempt`: mỗi thao tác chia thành bước có cận; sau mỗi bước kiểm `p.pending_interrupt()`; nếu có, lưu tiến độ **trong object** (ví dụ `untyped.zero_cursor`, `cnode.delete_cursor`), trả `Restart` để platform quay lại chính lệnh syscall (đặt lại ip). **`Restart` là tín hiệu nội bộ core → platform, KHÔNG phải mã lỗi ABI** (QD §3.7): nó không bao giờ tới user và **đã bị bỏ khỏi bảng mã lỗi** (mục 4.20).
 - Trạng thái trung gian phải thoả mọi invariant (sim kiểm ở mọi điểm preempt).
 - Ở sim: bộ lập lịch có thể ép preempt tại mọi bước.
 
@@ -342,21 +403,21 @@
 
 | Bậc | Nội dung | Khi nào |
 |---|---|---|
-| S1 | BKL dạng **queue lock (MCS lock)** | K1 |
-| S2 | Đường không khoá: hộp thư, ack TLB, IPI, timer riêng hart, idle | K1 |
-| S3 | Cụm: mỗi cụm một khoá + một `Kernel` riêng; chéo cụm qua hộp thư | Khi chờ khoá > ngưỡng (mục 9.9) |
+| S1 | BKL dạng **queue lock (MCS lock)** | K1a |
+| S2 | Đường không khoá: hộp thư, ack TLB, IPI, timer riêng hart, idle | K1a |
+| S3 | Cụm: mỗi cụm một khoá + một `Kernel` riêng; chéo cụm qua hộp thư | Khi chờ khoá > ngưỡng (mục 9.10) |
 | S4 | Khoá mịn cục bộ | Sau S3, chỉ đúng đường nóng |
 
-Năm luật bắt buộc từ K1:
+Năm luật bắt buộc từ **K1a**:
 1. vifast-core không biết khoá; platform lấy khoá rồi gọi `handle`.
 2. Đường nóng chỉ ghi `PerHart` của hart đang chạy + object mà thao tác chạm tới; không bộ đếm global trên đường nóng.
-3. Không sửa `PerHart` của hart khác; đi qua hộp thư + IPI.
+3. Không **ghi** trạng thái hart khác; đi qua hộp thư + IPI. **Đọc** trường công bố (`idle`, `cur_prio` — mục 4.8) là **được phép** (QD §3.10).
 4. Mỗi object có `home`.
 5. Không giữ khoá khi chờ; đang chờ khoá hay chờ ack đều phải xử lý hộp thư của mình.
 
-Build sim có guard ghi lại hart truy cập `PerHart`; vi phạm luật 2, 3 là panic.
+Build sim có guard ghi lại hart truy cập `PerHart`; vi phạm luật 2, 3 là panic (đọc trường công bố không tính là vi phạm).
 
-`MAX_HARTS` cấu hình tối đa 256; HartMask kích thước theo số hart lúc boot (bitmap trong arena).
+`MAX_HARTS` **mặc định 8, trần cấu hình 64 [TK]** (sửa 256); HartMask kích thước theo số hart lúc boot (bitmap trong arena). **Arena chỉ cấp cho hart phát hiện được** lúc boot, không cấp đủ `MAX_HARTS` [TK].
 
 ### 4.17 Hộp thư theo hart (`vifast-sync`)
 
@@ -380,18 +441,18 @@
 
 ### 4.20 Mã lỗi
 
-InvalidCapability, WrongObjectType, InsufficientRights, WxViolation, InvalidArgument, Alignment, SlotOccupied, SlotEmpty, NotEnoughMemory, AlreadyMapped, NotMapped, MissingPageTable, HasChildren, Busy, Cancelled, Timeout, IllegalOperation, NotSupported, RangeError, Restart (nội bộ, không bao giờ tới user).
+**19 mã** (QD §4): InvalidCapability, WrongObjectType, InsufficientRights, WxViolation, InvalidArgument, Alignment, SlotOccupied, SlotEmpty, NotEnoughMemory, AlreadyMapped, NotMapped, MissingPageTable, HasChildren, Busy, Cancelled, Timeout, IllegalOperation, NotSupported, RangeError.
 
-Mỗi mã một nghĩa. Kèm một word chi tiết (chỉ số tham số sai, cấp bảng trang thiếu, số byte thiếu). Tất cả nằm trong `abi.toml`.
+Mỗi mã một nghĩa; kèm một word chi tiết (chỉ số tham số sai, cấp bảng trang thiếu, số byte thiếu). Tất cả nằm trong `abi.toml`. `Restart` **không phải mã lỗi ABI** — là tín hiệu nội bộ core → platform (mục 4.13). `BrokenPipe`, `Stopped` thuộc giao thức user (IDL procsrv/pipe), không phải kernel.
 
 ### 4.21 Bảng ABI
 
-**Syscall**
+**Syscall** — 0..13 như cũ, thêm **14 Exit**; dải **15..31 reserved** (QD §4).
 
 | # | Tên | Ghi chú |
 |---|---|---|
-| 0 | Call | Endpoint: IPC; object khác: invocation (method = label) |
-| 1 | Send | Notification: Signal; Reply: trả lời |
+| 0 | Call | Endpoint: IPC; object khác: invocation (method = label), **trả ngay** |
+| 1 | Send | Notification: Signal; Reply: trả lời; fault `Exit`: gửi không chờ |
 | 2 | NBSend | |
 | 3 | Recv | Endpoint hoặc Notification |
 | 4 | NBRecv | Poll |
@@ -404,33 +465,46 @@
 | 11 | ClockRead | dự phòng |
 | 12 | DebugPutStr | feature debug |
 | 13 | DebugHalt | feature test, thoát QEMU với mã |
+| 14 | Exit(code) | TCB → `Inactive`, gửi `FaultKind::Exit{code}` tới `fault_ep` dạng Send; không có `fault_ep` thì chỉ `Inactive` |
 
 **Object và method** (method id sinh từ `abi.toml`)
 
 | Object | Method | Cài ở |
 |---|---|---|
-| Untyped | Retype | K1 |
-| CNode | Copy, Mint, Move, Mutate, Delete, Revoke, Rotate | K1 |
-| Tcb | Configure, SetPriority, SetMcp, SetAffinity, SetSchedParams, SetTimeoutEp, ReadRegs, WriteRegs, CopyRegs, Resume, Suspend, BindNotification, UnbindNotification, SetTls | K1 (SetTls K6) |
-| SchedControl | Configure | K1 |
-| SchedContext | Bind, Unbind, Consumed, YieldTo | K1 |
-| Endpoint, Notification, Reply | (qua syscall) | K1 |
-| VSpace | GetAsid (debug) | K2 |
+| Untyped | Retype (Revoke qua `CNode.Revoke` trên cap Untyped) | K1a |
+| CNode | Copy, Mint, Move, Mutate, Delete, Revoke, Rotate | K1a |
+| Tcb | Configure, **SetFaultEp**, **ReadState**, **Suspend(mode)**, SetPriority, SetMcp, SetAffinity, SetSchedParams, SetTimeoutEp, ReadRegs, WriteRegs, CopyRegs, Resume, BindNotification, UnbindNotification, SetTls | K1b (SetTls K6) |
+| SchedControl | Configure | K1b |
+| SchedContext | Bind, Unbind, Consumed, YieldTo | K1b |
+| Endpoint, Notification, Reply | (qua syscall) | K1c |
+| VSpace | GetAsid (debug); **tạo/khách: reserved** | K2 |
 | PageTable | Map, Unmap | K2 |
 | Frame | Map, Unmap, GetAddress, MapIo, UnmapIo | K2 (Io: D3) |
 | IrqControl | GetLine, GetMsi | K3 (Msi: D2) |
 | IrqHandler | SetNotification, Ack, Clear, SetAffinity | K3 |
-| IOSpace | (Map qua Frame), Flush | D3 |
-| VCpu | Run, ReadRegs, WriteRegs, InjectIrq, SetTsc | H1 |
+| IOSpace | Flush; **Attach: reserved** | D3 |
+| VCpu | Run, ReadRegs, WriteRegs, InjectIrq, SetTsc; **Configure: reserved** | H1 |
 | PowerControl | HartOffline, HartOnline, Suspend | P1 |
 
-Method chưa cài trả `NotSupported`; conformance test kiểm điều đó.
+Object chưa cài chỉ giữ **số reserved**, không đóng băng chữ ký; method chưa cài trả `NotSupported`; conformance test kiểm điều đó (QD §1 Q-B).
 
-Bỏ hẳn so với v1: Open, Read, Write, Close, Stat, Seek, Block*, ConsoleRead, TcbSetArgs, ModuleRelease, MemInfo/TcbInfo/TimerInfo, software IOMMU.
+**Đóng băng ABI (D2-15):** 5 thứ — (1) số syscall + quy ước thanh ghi 3 arch; (2) layout MessageInfo + MR trong thanh ghi + receive window; (3) bảng mã lỗi + quy ước trả lỗi; (4) kích thước/căn lề object theo arch; (5) luật "ID không tái dùng". Có `abi_version`, `min_compat`, `DebugGetInfo`, golden file.
 
+Bỏ hẳn so với v1: Open, Read, Write, Close, Stat, Seek, Block*, ConsoleRead, TcbSetArgs, ModuleRelease, MemInfo/TcbInfo/TimerInfo, software IOMMU. Bảng ánh xạ 58 syscall v1 → 15 + method, 22 KError → 19 là việc KT-04a.
+
+**Exit, kill, wait** (QD §1 Q-A, §3.3):
+
+| Việc | Ai làm | Ngữ nghĩa |
+|---|---|---|
+| `exit(code)` | syscall **14** | Kernel chuyển TCB `Inactive`; nếu có `fault_ep`, gửi fault `FaultKind::Exit{code}` **dạng Send, không chờ Reply**; không có `fault_ep` thì chỉ `Inactive`. |
+| `wait` | RPC procsrv | Trả `{code, how}` với `how ∈ Exited | Cancelled | Faulted | Killed` (mục 6.4). |
+| `kill` | procsrv | `Tcb.Suspend(Abort)` rồi Revoke cap TCB; ghi `how = Killed`. |
+
+`DebugHalt` vẫn chỉ dùng cho test.
+
 ### 4.22 KernelConfig
 
-Khai báo trong `spec/kernel.toml`, sinh thành hằng số Rust: `MAX_HARTS`, kích thước trace ring, kích thước hộp thư, số refill tối đa, định nghĩa cụm, feature (debug, test, trace, kpti). Sim và cả 3 platform đọc cùng tệp; kịch bản sim có thể ghi đè.
+Khai báo trong `spec/kernel.toml`, sinh thành hằng số Rust: `MAX_HARTS` (mặc định **8**, trần **64** [TK]), kích thước trace ring, kích thước hộp thư, số refill tối đa, định nghĩa cụm, feature (debug, test, kpti). **Trace luôn bật, không có cờ tắt** (D2-09). Sim và cả 3 platform đọc cùng tệp; kịch bản sim có thể ghi đè.
 
 ---
 
@@ -441,6 +515,7 @@
 - Ứng dụng UEFI cho cả 3 arch (QEMU: OVMF cho x86, AAVMF cho aarch64, EDK2/U-Boot cho riscv64 [GT: kiểm EDK2 riscv64 trên QEMU đang dùng]). Đường dự phòng riscv64: OpenSBI + nạp trực tiếp.
 - Đọc bản đồ RAM, ACPI (vifast-acpi) hoặc DTB (vifast-dtb), nạp kernel ELF + init + module archive, dựng bảng trang ban đầu (nửa cao + physmap), điền **BootDesc** (vifast-bootdesc) rồi nhảy vào kernel.
 - Kết quả: điểm vào kernel giống nhau ở mọi arch.
+- **Tới K6 chỉ riscv64, OpenSBI nạp trực tiếp**; UEFI và aarch64/x86_64 chỉ sau K6 (D2-10, QD §1 Q-F(1)).
 
 ### 5.2 Chia RAM lúc boot
 
@@ -468,7 +543,7 @@
 | IPI | SBI IPI / IMSIC | SGI | LAPIC IPI |
 | Hart phụ | SBI HSM | PSCI | INIT-SIPI trampoline (tham khảo v1) |
 | Ảo hoá | H extension | EL2 | VT-x |
-| Mục tiêu | < 2.500 dòng Rust + asm riêng | như trái | như trái |
+| Mục tiêu | < 2.500 dòng Rust + asm riêng [ƯL] | như trái | như trái |
 
 ---
 
@@ -507,8 +582,18 @@
 
 ### 6.4 process server
 
-Tạo tiến trình từ ELF (vifast-elf): CSpace, VSpace, IPC buffer, stack với argv/env, trang thời gian, TLS; SchedContext theo chính sách; theo dõi thoát; cung cấp `wait`, `kill`, danh sách tiến trình; **job control không dùng signal** (huỷ, tạm dừng, tiếp tục qua IPC) cho ViShell.
+Tạo tiến trình từ ELF (vifast-elf): CSpace, VSpace, IPC buffer, stack với argv/env, trang thời gian, TLS; SchedContext theo chính sách. **Job control không dùng signal** (QD §3.1, §3.3):
 
+| RPC (IDL) | Ngữ nghĩa |
+|---|---|
+| `Spawn(spec) -> pid` | tạo tiến trình; trả cap/pid |
+| `Wait(pid\|gid) -> {code, how}` | `how ∈ Exited \| Cancelled \| Faulted \| Killed` |
+| `Stop(gid)` / `Cont(gid)` | `Tcb.Suspend(Abort)` / `Tcb.Resume`; Resume **không** phát lại IPC |
+| `Cancel(gid)` | huỷ từ stage cuối về đầu, chờ tối đa **2 s** [ƯL], rồi finalise TCB |
+| `List() -> [...]` | danh sách tiến trình/nhóm |
+
+procsrv giữ bảng `gid -> [tcb cap]`. **Terminal: shell sở hữu tty**; tiến trình con nhận pipe, **không** nhận `cap[Term]`; chương trình toàn màn hình (vim, less) để sau với tty server riêng. Exit: syscall 14 → `FaultKind::Exit{code}` → procsrv ghi mã thoát rồi finalise; `kill` = `Suspend(Abort)` + Revoke cap TCB.
+
 ### 6.5 VFS
 
 - Nhiều luồng worker, mỗi worker giữ Reply object riêng.
@@ -569,13 +654,13 @@
 
 | Việc | Nơi | Mốc |
 |---|---|---|
-| `vis-host-vifast`: cài trait Host bằng IPC tới VFS, process server, net server; quyền là cap thật | `viscript/` dùng `vifast-urt` | VSH2 |
+| `vis-host-vifast`: cài trait Host bằng IPC tới VFS, process server, net server; quyền là cap thật | workspace ngoài `viscript/`, dùng `vifast-urt` | VSH2 |
 | Binding ViScript sinh từ IDL (vifast-gen thêm đầu ra binding) | `tools/vifast-gen` | VSH2 |
 | ViShell làm shell mặc định, thay shell tạm của K6 | đóng gói trong `user/` | VSH2 |
-| Giao thức hiển thị có cấu trúc ViShell và ViTerm | `term.vidl` | VS3 + G3 |
+| Giao thức hiển thị có cấu trúc ViShell và ViTerm | `term.vidl` | mốc riêng **TUI** (trước G3) |
 | Job control không signal trong process server | `procsrv` | U1 |
 
-Yêu cầu từ ViScript lên OS: Rust std đầy đủ (thread, time, fs, net), WaitAddr cho Mutex, trang thời gian, process server có `kill/wait/list`. Tất cả nằm trong U1.
+Yêu cầu từ ViScript lên OS: Rust std phần **thread, time, fs, process** trong U1; **net để ở N1** (U1 không hứa net); WaitAddr cho Mutex; trang thời gian; procsrv có `kill/wait/list`. VSH2 phụ thuộc **U1 + N1 + VS2** (sửa lệch C §3); VS3 phụ thuộc **VS2 + G3**.
 
 ### 7.4 Linux qua máy ảo
 
@@ -596,7 +681,7 @@
   lib/           (no_std, chạy được trên host)
     vifast-abi2  vifast-ilist  vifast-pte  vifast-elf  vifast-dtb  vifast-acpi
     vifast-bootdesc  vifast-sync(*)  vifast-objmem(*)  vifast-traceformat
-    vifast-wire  vifast-fs  vifast-virtio  vifast-nvme  vifast-pci  vifast-capalloc
+    vifast-wire  vifast-fs  vifast-fsutil  vifast-virtio  vifast-nvme  vifast-pci  vifast-capalloc
   kernel/        vifast-core  vifast-plat-common  vifast-plat-{riscv64,aarch64,x86_64}  vifast-kbin-*
   boot/          vifast-boot
   sim/           vifast-sim  vifast-model
@@ -611,12 +696,14 @@
 ```
 
 **Luật cấu trúc (cổng cứng)**:
-- Tệp trong `kernel/`, `lib/` không quá 1.500 dòng; vifast-core dưới 15.000 dòng không tính test [ƯL].
+- Tệp trong `kernel/`, `lib/` không quá **1.500 dòng [TK]**; `vifast-core` dưới **25.000 dòng** không tính test [ƯL].
 - `cfg(target_arch)` cấm trong core và lib.
 - `unsafe` chỉ trong danh sách crate đóng + `kernel/vifast-plat-*` + `boot/`.
 - Đồ thị phụ thuộc theo `deps.toml`.
 - Code sinh trùng spec.
-- Mỗi platform dưới 2.500 dòng không tính asm.
+- Mỗi platform dưới **2.500 dòng [ƯL]** không tính asm.
+- **Bảng ánh xạ 25 crate v1 → v2 là việc của KT-04a** (cùng bảng 58 syscall → 15 + method, 22 KError → 19); tài liệu không chép tay bảng đó.
+- Ngân sách tĩnh: `.data + .bss ≤ 256 KiB` [TK], đo ở K0.5; arena chỉ cấp cho hart phát hiện được (QD §3.5, §3.15).
 
 **Ma trận tái sử dụng**:
 
@@ -641,12 +728,12 @@
 | Tầng | Nội dung | Mục tiêu | Khi chạy |
 |---|---|---|---|
 | L0 | Unit + property test từng crate | < 15 s | mỗi lần lưu, mỗi bước vòng lặp |
-| L1 | Sim tất định, invariant sau mỗi event | 10.000 seed < 60 s | mỗi commit |
+| L1 | Sim tất định, invariant sau mỗi event | `--fast`: 1.000 seed; nightly: 100.000; đo lại khi có sim | mỗi commit (sau KT-08) |
 | L2 | So mô hình + kịch bản có tên + hồi quy `.scn` | < 5 phút | mỗi commit chạm core |
 | L3 | QEMU: ktest + conformance, 3 arch, smp 1/2/4 | < 2 phút/arch | commit chạm platform/user; cổng ngày |
 | L4 | Sim triệu seed, mutation đầy đủ, Miri, L3 lặp, soak, đo hiệu năng | hàng giờ | đêm |
 
-Vòng lặp hằng ngày khi sửa logic kernel: L0 + L1 trên host, không cần QEMU.
+Vòng lặp hằng ngày khi sửa logic kernel: L0 + L1 trên host, không cần QEMU. **L1 chưa bật tới KT-08**; trước đó `STATE.md` ghi rõ. Đo thời gian L1 ngay khi sim chạy; vượt 60 s thì giảm seed của `--fast`, **không** giảm invariant (QD §5.2).
 
 ### 9.2 Kiểm thử theo từng phần
 
@@ -707,7 +794,7 @@
 ### 9.5 SimPlatform
 
 - N hart (1 tới 8 trong test thường, tới 64 trong nightly), đồng hồ ảo ns, deadline theo hart, IPI.
-- RAM là `Vec<u8>` căn lề; bảng trang thật ghi vào đó bằng vifast-pte; object thật đặt bằng vifast-objmem.
+- RAM là **vùng thô có căn lề** (bộ cấp phát thô, **không** `Vec<u8>` — QD §3.5); bảng trang thật ghi vào đó bằng vifast-pte; object thật đặt bằng vifast-objmem.
 - **Mô hình TLB theo hart**: cache (asid, va) -> (pa, rights) khi user Touch; chỉ xoá khi kernel flush. Truy cập qua TLB tới PA đã trả về Untyped hoặc quyền đã hạ = vi phạm TLB-safety.
 - Hộp thư dùng đúng vifast-sync.
 - Đi qua đúng bộ giải mã syscall sinh từ ABI.
@@ -752,17 +839,19 @@
 
 $$p_{95} \approx \frac{3}{n}$$
 
-Muốn tuyên bố tỉ lệ dưới p cần $$n \ge \frac{3}{p}$$. Chốt n trước, cùng BUILD_ID, cùng máy. Lỗi logic phải tái hiện ở L1 trước; soak chỉ xác nhận.
+Muốn tuyên bố tỉ lệ dưới p cần $$n \ge \frac{3}{p}$$. Chốt n trước, cùng **máy `ps`**, BUILD_ID, cấu hình (smp, `MAX_IO`); 0/N chỉ là trần; không đếm artifact; T-BUILD không tính. Với p mục tiêu **0,3 %**, N = **1.000** chạy **3 luồng** (QD §5.3). Bỏ con số 750. Lỗi logic phải tái hiện ở L1 trước; soak chỉ xác nhận.
 
+**Hợp đồng marker (harness-v2):** TAP là định dạng kết quả; `harness-v2` có `FORBIDDEN` (panic, invariant fail, `TIME BACK`), `EXACT_ONE` (dòng plan TAP, BUILD_ID), và bộ case âm cho chính harness (T-IPC: harness mới phải chứng minh bắt được lỗi trước khi dùng để tuyên bố).
+
 ### 9.13 Lệnh
 
 | Lệnh | Nội dung |
 |---|---|
 | `v2 check` | L0 các crate bị chạm |
-| `v2 gate --fast` | gen-check, fmt, clippy -D warnings, luật cấu trúc, L0 toàn bộ, L1 10.000 seed. < 2 phút |
-| `v2 gate` | + L2 + L3 các arch đã port |
-| `v2 nightly` | L4 + cập nhật STATUS.md + perf history |
-| `v2 repro <seed|scn> [--arch X]` | chạy lại một ca, mở trace |
+| `v2 gate --fast` | fmt, clippy -D warnings, luật cấu trúc, L0 toàn bộ; + gen-check (từ KT-05); + L1 **1.000 seed** (từ KT-08). < 2 phút |
+| `v2 gate` | + L2 + L3 arch đã port |
+| `v2 nightly` | L1 100.000, mutation, Miri, soak + cập nhật `STATUS.md` + perf history |
+| `v2 repro <seed\|scn> [--arch X]` | chạy lại một ca, mở trace |
 | `v2 mutants [tên]` | chạy đột biến |
 
 Không retry ở mọi cổng. Flake hạ tầng phân loại T-INFRA, không bao giờ tính PASS.
@@ -771,67 +860,74 @@
 
 ## 10. Lộ trình đầy đủ
 
-Quy tắc: một mốc bắt đầu khi mọi phụ thuộc đã đạt tiêu chí. Các dòng khác nhau chạy song song nếu không phụ thuộc.
+Quy tắc: một mốc bắt đầu khi mọi phụ thuộc đã đạt tiêu chí. Các dòng khác nhau chạy song song nếu không phụ thuộc. **Không có mốc theo ngày** (QD §1 Q-F, §5.1); mọi tiêu chí đo được.
 
 ### 10.1 Dòng lõi
 
 | Mốc | Nội dung | Tiêu chí xong |
 |---|---|---|
-| V1-END | Commit r323, soak n=750 không retry, tag `v1-final` | Có báo cáo |
-| K0 | Spec đầy đủ (`abi.toml` mọi object/method mục 4.20, 4.21; `kernel.toml`; `deps.toml`), bảng trạng thái, vifast-gen, khung workspace, luật cấu trúc, vifast-agentctl | `v2 gate --fast` chạy; Agent 0 duyệt spec |
-| K1a | objmem, Untyped, CSpace 2 cấp + MDB, finalise khung, preempt khung | Miri xanh; sim kịch bản cap/untyped 100.000 seed; khớp mô hình |
-| K1b | TCB + transition, scheduler theo hart, hộp thư, MCS | Bao phủ cặp 100%; kiểm budget; push migration |
-| K1c | IPC + Reply + Notification + WaitAddr + fault | Bảo toàn message, không mất đánh thức; mọi đột biến chuẩn bị bắt; 100.000 seed x smp 1/2/4/8 |
-| K2 | VSpace/PageTable/Frame 3 cỡ + attrs, ASID, shootdown, W^X, mô hình TLB | 0 vi phạm TLB-safety 100.000 seed; đột biến AP->BSP bị bắt |
-| K3 | vifast-boot + plat-common + plat-riscv64; init tối thiểu; ktest + conformance | TAP 100% smp 1/2/4; 750 boot 0 lỗi; perf đầu tiên |
-| K4 | plat-aarch64 | Như K3 |
-| K5 | plat-x86_64 (+ KPTI tuỳ chọn) | Như K3, + `-cpu max` |
-| K6 | memsrv, procsrv, vifast-alloc, capalloc, IDL + ring, VFS (pager, page cache, journal), virtio-blk/console, SetTls, shell tạm (Rust) | Tương đương v1; test-persist 7 pha x 3 arch; harness sản phẩm có case âm |
+| K0 | Spec đầy đủ (`abi.toml` 15 syscall + method + 19 lỗi + reserved; `kernel.toml`; `deps.toml`), bảng trạng thái, vifast-gen, khung workspace, luật cấu trúc, vifast-agentctl | `v2 gate --fast` chạy; Agent 0 duyệt spec (KT-04d APPROVED) |
+| **K0.5** | boot riscv64 QEMU qua OpenSBI nạp trực tiếp; `DebugPutStr`/`DebugHalt`; TAP 1 test; đo image/`.data`/`.bss` | **50 boot 0 lỗi**; `.data + .bss ≤ 256 KiB` [TK] |
+| K1a | sim harness store giả + scheduler tối thiểu 1 hart; Untyped; CNode/MDB test-first (mô hình tối giản trong test) | 1 kịch bản end-to-end; invariant tổng byte; property test cây |
+| K1a' | thay store giả bằng objmem (API closure) + Miri | Miri xanh từ commit đầu; 20 ca KT-04c chạy trên mô hình |
+| K1b | TCB + transition + `suspended`; scheduler theo hart; hộp thư; **MCS giai đoạn đầu** (donation) | Bao phủ cặp 100%; kiểm budget; push migration |
+| K1c | IPC + Reply + Notification + WaitAddr + fault + **Exit** | Bảo toàn message; không mất đánh thức; mọi đột biến chuẩn bị bắt; 100.000 seed × smp 1/2/4/8 |
+| K2 | VSpace/PageTable/Frame 3 cỡ + attrs, ASID, shootdown, W^X, mô hình TLB | 0 vi phạm TLB-safety 100.000 seed; đột biến AP→BSP bị bắt |
+| K3 | plat-common + plat-riscv64; init tối thiểu; ktest + conformance | TAP 100% smp 1/2/4 [TK]; soak boot **0 lỗi / N = 1.000** trên máy `ps` [TK] (D2-24); perf đầu tiên |
+| **K3.5** | procsrv + shell Rust tối thiểu (ABI được ứng dụng thật dùng) | spawn/wait/exit/kill chạy; shell chạy một lệnh |
+| K6 | memsrv, vifast-alloc, capalloc, IDL + ring, VFS (pager, page cache, journal), virtio-blk/console, SetTls | "Tương đương v1" theo danh sách đo được (VFS journal/fsck/rename/truncate/double-indirect; pager nhiều client; exec chain; test-persist 7 pha) |
+| K4' | plat-aarch64 | như K3 |
+| K5' | plat-x86_64 (+ KPTI tuỳ chọn) | như K3 + `-cpu max` |
+| UEFI | `vifast-boot` UEFI cho 3 arch (sau K5') | boot 3 arch qua UEFI, hoặc ghi quyết định giữ OpenSBI cho riscv64 |
 
 ### 10.2 Dòng runtime và ViScript
 
-| Mốc | Nội dung | Phụ thuộc |
-|---|---|---|
-| VS-SPEC, VS0, VS1, VSH1, VS2 | Ngôn ngữ, VM, ViShell trên Mac/Linux/Windows/wasm (tài liệu ViScript mục 19) | Không phụ thuộc ViFastOs, chạy song song dòng lõi |
-| U1 | Rust std cho ViFastOs + job control trong procsrv | K6 |
-| VSH2 | vis-host-vifast, binding IDL, ViShell thành shell mặc định | U1, VS2 |
-| VS3 | Plugin, binding toolkit, ViTerm có cấu trúc | VSH2, G3 |
+| Mốc | Nội dung | Phụ thuộc | Tiêu chí |
+|---|---|---|---|
+| VS-SPEC..VS2 | Ngôn ngữ, VM, ViShell trên Mac/Linux/Windows/wasm (tài liệu ViScript mục 19) | không phụ thuộc ViFastOs | theo ViScript §19 |
+| U1 | Rust std phần thread/time/fs/process + job control trong procsrv | K6 | test con std; spawn/wait/kill |
+| **N1** | Net server, socket qua IDL, DHCP/DNS | D1, U1 | ping/DNS qua socket |
+| VSH2 | vis-host-vifast, binding IDL, ViShell thành shell mặc định | **U1, N1**, VS2 | chạy được ViShell trên ViFastOs |
+| VS3 | Plugin, binding toolkit, ViTerm có cấu trúc | VSH2, G3 | ứng dụng desktop nhỏ |
 
 ### 10.3 Dòng thiết bị và mạng
 
-| Mốc | Nội dung | Phụ thuộc |
-|---|---|---|
-| D1 | virtio-net, virtio-gpu, virtio-input | K6 |
-| N1 | Net server, socket qua IDL, DHCP/DNS | D1, U1 |
-| D2 | PCIe enumeration, MSI-X, NVMe, AHCI | K5 |
-| D3 | IOSpace: VT-d, SMMUv3, virtio-iommu | D2 |
-| D4 | xHCI + USB HID + mass storage | D2 |
-| D5 | Âm thanh | D2, số đo MCS |
+| Mốc | Nội dung | Phụ thuộc | Tiêu chí |
+|---|---|---|---|
+| D1 | virtio-net, virtio-gpu, virtio-input | K6 | thiết bị giả + QEMU |
+| N1 | Net server, socket qua IDL, DHCP/DNS | D1, U1 | xem §10.2 |
+| D2 | PCIe enumeration, MSI-X, NVMe, AHCI | K5' | enumerate + IRQ |
+| D3 | IOSpace: VT-d, SMMUv3, virtio-iommu | D2 | DMA được cô lập |
+| D4 | xHCI + USB HID + mass storage | D2 | HID hoạt động |
+| D5 | Âm thanh | D2, số đo MCS | phát 1 tệp |
 
 ### 10.4 Dòng desktop
 
-| Mốc | Nội dung | Phụ thuộc |
-|---|---|---|
-| G1 | Framebuffer + console đồ hoạ, font tiếng Việt/Nhật | D1 |
-| G2 | Compositor phần mềm, input server, cửa sổ | G1, U1 |
-| G3 | Toolkit Rust, ứng dụng mẫu (ViTerm chạy ViShell, trình quản lý tệp) | G2 |
-| G4 | ViFastKey IME server | G2 |
-| G5 | Audio + trình phát | D5, G3 |
+| Mốc | Nội dung | Phụ thuộc | Tiêu chí |
+|---|---|---|---|
+| **TUI** | `term.vidl` — giao thức hiển thị có cấu trúc ViShell/ViTerm | VS2, K6 | ViShell chạy trong ViTerm |
+| G1 | Framebuffer + console đồ hoạ, font Việt/Nhật | D1 | hiện chữ có dấu |
+| G2 | Compositor phần mềm, input server, cửa sổ | G1, U1 | 2 cửa sổ |
+| G3 | Toolkit Rust, ứng dụng mẫu (ViTerm, quản lý tệp) | G2 | chạy ViShell |
+| G4 | ViFastKey IME server | G2 | gõ tiếng Việt |
+| G5 | Audio + trình phát | D5, G3 | phát 1 tệp |
 
+**D-08 còn hiệu lực tới K6** (D2-23): dòng G và ViFastKey (G4) chỉ mở **sau K6**, không chạy song song dòng lõi.
+
 ### 10.5 Dòng nâng cao
 
-| Mốc | Nội dung | Phụ thuộc |
-|---|---|---|
-| P1 | Power: idle sâu, hart offline/online, suspend/resume | K5 |
-| H1 | VCpu + VMM (riscv64 H trước), Linux khách tới shell | K6, D1 |
-| H2 | Linux khách trên aarch64, x86_64; chia sẻ tệp, cửa sổ khách vào compositor | H1, G2 |
-| S3 | Cụm, khi số đo vượt ngưỡng | perf |
-| F1 | IPC fastpath | perf |
-| HW1 | Chạy trên máy thật (một board riscv64/aarch64 + một PC x86) | K5, D2 |
+| Mốc | Nội dung | Phụ thuộc | Tiêu chí |
+|---|---|---|---|
+| P1 | Power: idle sâu, hart offline/online, suspend/resume | K5' | đo tiêu thụ |
+| H1 | VCpu + VMM (riscv64 H trước), Linux khách tới shell | K6, D1 | shell khách |
+| H2 | Linux khách trên aarch64/x86_64; chia sẻ tệp, cửa sổ khách vào compositor | H1, G2 | chia sẻ tệp |
+| S3 | Cụm, khi số đo vượt ngưỡng | perf §9.10 | chờ khoá > 10 % |
+| F1 | IPC fastpath | perf §9.10 | p99 giảm |
+| HW1 | Máy thật (board riscv64/aarch64 + PC x86) | K5', D2 | boot tới shell |
 
-### 10.6 Định hướng thời gian [ƯL]
+### 10.6 Thứ tự ưu tiên (không ngày)
 
-90 ngày đầu: V1-END, K0, K1a/b/c, K2; ViScript VS-SPEC, VS0 song song. 90 ngày tiếp: K3, K4, K5; ViScript VS1, VSH1. Sau đó K6, U1, D1, VS2, VSH2. Các dòng G, N, H mở dần.
+K0 → K0.5 → K1a → K1a' → K1b → K1c → K2 → K3 → K3.5 → K6. Dòng ViScript VS-SPEC..VS2 chạy song song nhưng **OS trước**: Agent 2 chỉ lấy task ViScript khi QUEUE v2 không còn task READY (D2-20), ngoại lệ ST-02a (tài liệu). Không cam kết ngày; tiến độ đo bằng mốc đạt.
 
 ---
 
@@ -847,14 +943,14 @@
 
 | Tệp | Ai ghi | Nội dung |
 |---|---|---|
-| `QUEUE.md` | Agent 0 (qua người điều phối) | Task có ID, phụ thuộc, tiêu chí xong, ngân sách |
+| `QUEUE.md` | Agent 0 (qua người điều phối) | Task có ID, phụ thuộc, ngân sách, **trạng thái `READY\|BLOCKED\|REVIEW\|DONE`**, tiêu chí xong, dòng `APPROVED` khi duyệt spec |
 | `STATE.md` | Agent 2 | Task đang làm, bước, HEAD, cổng gần nhất, việc dở. **Cập nhật sau mỗi bước** để nén ngữ cảnh bất cứ lúc nào |
 | `JOURNAL/YYYY-MM-DD.md` | Agent 2 | Nhật ký ngắn: làm gì, kết quả, quyết định nhỏ |
 | `BLOCKED.md` | Agent 2 | Task bị chặn + lý do + câu hỏi cho Agent 0 |
-| `reports/R<n>.md` + `R<n>.json` | Agent 2 | Báo cáo cuối vòng; JSON máy đọc được |
+| `reports/R<n>-v2.md` + `R<n>-v2.json` | Agent 2 | Báo cáo cuối vòng; JSON có `project` |
 | `DECISIONS.md` | Agent 0 | Danh sách quyết định có hiệu lực (chép từ mục 12) |
 
-`vifast-agentctl`: `next` (task kế có phụ thuộc đã xong), `start <id>`, `done <id>`, `block <id> <lý do>`, `report` (sinh R<n>.json từ git + kết quả cổng). Nhận `--project v2|viscript` để dùng chung cho hai dòng.
+`vifast-agentctl`: `next` (task kế có phụ thuộc đã DONE, **bỏ qua REVIEW**), `start <id>`, `done <id>`, `block <id> <lý do>`, `report` (sinh `R<n>-v2.json` từ git + kết quả cổng). Nhận `--project v2|viscript` để dùng chung cho hai dòng.
 
 ### 11.3 Vòng lặp chuẩn cho mỗi task
 
@@ -863,13 +959,13 @@
 3. **Test đỏ trước**: viết test/kịch bản thể hiện yêu cầu; chạy, xác nhận đỏ đúng lý do.
 4. **Cài đặt** theo bước nhỏ; sau mỗi bước `v2 check`.
 5. **Cổng**: `v2 gate --fast` (luôn); `v2 gate` nếu chạm platform/user.
-6. **Commit**: `[T<id>] <tóm tắt>` + kết quả cổng + BUILD_ID.
+6. **Commit**: `[KT-##] <tóm tắt>` + kết quả cổng + BUILD_ID.
 7. **Cập nhật** STATE.md, JOURNAL; `agentctl done`.
 8. **Nén ngữ cảnh** nếu lớn, rồi quay lại bước 1 với task kế.
 
 ### 11.4 Luật bắt buộc
 
-- Không sửa `spec/` (trừ khi task cho phép rõ), DECISIONS.md, ngưỡng cổng, `accept()` của harness; danh sách đột biến chỉ được thêm.
+- Không sửa `spec/` trừ task được ghi rõ (`KT-04a`), không sửa DECISIONS.md, ngưỡng cổng, `accept()` của harness; danh sách đột biến chỉ được thêm. **Duyệt chỉ bằng dòng `APPROVED <ngày> Agent 0` trong `QUEUE.md` + một mục D2**, không duyệt bằng chat (D2-22); Agent 2 soạn nháp spec ở `REVIEW`, sau duyệt chỉ Agent 0 sửa.
 - Không tắt test, không `#[ignore]`, không nới invariant để qua cổng.
 - Không retry để qua cổng. Flake: ghi T-INFRA kèm log, chạy lại tối đa một lần **chỉ để phân loại**.
 - Hồi quy (cổng từng xanh nay đỏ): revert trước, điều tra sau.
@@ -878,7 +974,7 @@
 - Thêm phụ thuộc crate ngoài: thuần Rust, giấy phép tương thích MIT/Apache-2.0, ghi lý do trong JOURNAL, kiểm `deps.toml`.
 - Không đưa số liệu viết tay vào tài liệu; số liệu do `v2 nightly` sinh.
 - `cargo fmt` trước commit; clippy sạch.
-- Mọi tệp mã nguồn mới có header SPDX `// SPDX-License-Identifier: MIT OR Apache-2.0`.
+- SPDX theo loại tệp (D2-21, QD §9.3): `//` cho Rust/asm, `#` cho `.vis .vish .vidl .toml`; tệp không có chú thích dùng sidecar `.license`; tệp sinh do generator phát header. Thêm `license.workspace = true` cho mọi crate; LICENSE-MIT/APACHE một bản ở gốc; `THIRD-PARTY.md` sinh tự động; **không** tạo NOTICE.
 
 ### 11.5 Ngân sách và điều kiện dừng
 
@@ -889,20 +985,20 @@
 | Spec mâu thuẫn hoặc thiếu | BLOCKED với đề xuất cụ thể, không tự đổi spec |
 | Cổng đỏ do lỗi không thuộc task | Mở mục lỗi trong JOURNAL + `.scn` nếu có; không sửa lan |
 | Hết task khả dụng | Chạy `v2 nightly`, viết báo cáo, dừng |
-| Máy bận/QEMU treo | `pkill -f qemu-system`, ghi T-INFRA, tiếp |
+| Máy bận/QEMU treo | giết **đúng PID do task tạo** (cấm `pkill -f qemu-system` toàn cục), ghi T-INFRA, tiếp |
 
 ### 11.6 Chạy qua đêm
 
-- Trước khi rời máy: QUEUE.md có ít nhất 8 task chia nhỏ (1 tới 3 giờ mỗi task), phụ thuộc rõ, ít nhất 3 nhánh độc lập để khi một nhánh BLOCKED vẫn còn việc. Dòng ViScript là nguồn nhánh độc lập tốt.
+- Trước khi rời máy: QUEUE.md có ít nhất 8 task READY chia nhỏ (1 tới 3 giờ mỗi task), phụ thuộc rõ, ít nhất 3 nhánh độc lập để khi một nhánh BLOCKED vẫn còn việc. Dòng ViScript chỉ mở khi OS hết task READY (D2-20).
 - Agent 2 chạy vòng 11.3 liên tục; sau mỗi task chạy `v2 gate --fast` toàn bộ để phát hiện hồi quy chéo.
 - Cuối phiên hoặc khi hết task: `v2 nightly`, `agentctl report`.
-- Tài nguyên: tối đa 3 QEMU song song trên máy hiện tại [DOSSIER], sim dùng tối đa số core - 1.
+- Tài nguyên: tối đa **3 QEMU** song song (QD §5.3), sim dùng tối đa `số core − 1`.
 
 ### 11.7 Định dạng báo cáo
 
-`R<n>.md`: tóm tắt 5 dòng, bảng task (ID, trạng thái, commit, cổng), lỗi tìm thấy (`.scn`), số đo thay đổi, đột biến sống, BLOCKED + câu hỏi, đề xuất task kế.
+`R<n>-v2.md`: tóm tắt 5 dòng, bảng task (ID, trạng thái, commit, cổng), lỗi tìm thấy (`.scn`), số đo thay đổi, đột biến sống, BLOCKED + câu hỏi, đề xuất task kế.
 
-`R<n>.json`: `{round, head, build_id, tasks:[{id,status,commits,gates}], gates:{fast,full,nightly}, sim:{seeds, failures}, coverage:{transitions, error_paths}, mutants:{killed, alive}, perf:{...}, blocked:[...]}`.
+`R<n>-v2.json`: `{project:"v2", round, head, build_id, tasks:[{id,status,commits,gates}], gates:{fast,full,nightly}, sim:{seeds, failures}, coverage:{transitions, error_paths}, mutants:{killed, alive}, perf:{...}, blocked:[...]}`.
 
 Agent 0 đọc JSON trước, MD sau.
 
@@ -917,56 +1013,70 @@
 
 ## 12. Quyết định
 
+Quyết định v1 (D-01..D-08, D11, D18) giữ nguyên mã, **chỉ áp cho v1** (xem §2.3). Quyết định v2 dùng **D2-xx** (QD §2.1, §8), không trùng mã v1.
+
 | Mã | Nội dung |
 |---|---|
-| D-09 | v1 đóng băng sau r323 + một soak; v2 là nhánh chính |
-| D-10 | Core không chứa FS, fd, block, console, args, demo; syscall/method/object mới phải vào `abi.toml` qua Agent 0 |
-| D-11 | Commit chạm `kernel/` hoặc `lib/` phải xanh `v2 gate --fast`; lỗi logic tái hiện trên sim trước, kèm `.scn` |
-| D-12 | Luật cấu trúc mục 8 là cổng cứng |
-| D-13 | Test kernel không nằm trong boot sản phẩm |
-| D-14 | Trace ring luôn bật; cấm đèn chẩn đoán ad-hoc trên đường nóng |
-| D-15 | Không retry ở mọi cổng; flake phân loại T-INFRA |
-| D-16 | Port riscv64, aarch64, x86_64 theo thứ tự |
-| D-17 | Một nguồn sự thật trong `spec/`; code sinh không sửa tay |
-| D-18 | Đồng thời theo thang S1 tới S4; nâng bậc theo số đo |
-| D-19 | Kịch bản test viết dạng `.scn`/Scenario chạy cả sim và QEMU |
-| D-20 | Object kernel nằm trong Untyped của người tạo (vifast-objmem); không pool cố định |
-| D-21 | MCS là mô hình lập lịch duy nhất từ K1 |
-| D-22 | ABI lõi chốt ở K0 gồm cả object chưa cài (IOSpace, VCpu, PowerControl); sau K0 chỉ thêm, không đổi |
-| D-23 | `unsafe` chỉ trong danh sách crate đóng; mỗi crate unsafe có Miri hoặc loom/stress trong nightly |
-| D-24 | Ứng dụng hệ thống viết bằng Rust std, chỉ crate thuần Rust; libc và wasm để sau |
-| D-25 | ViScript (kiểu tĩnh, đa nền tảng, một ngôn ngữ cho script và shell) là ngôn ngữ script và shell chính, thiết kế trong tài liệu riêng; tương thích Linux qua máy ảo |
-| D-26 | Agent 2 làm việc theo mục 11; STATE.md cập nhật sau mỗi bước |
-| D-27 | Giấy phép MIT OR Apache-2.0 cho toàn bộ mã nguồn; phụ thuộc phải tương thích |
+| D2-01 | v1 lưu trữ ở `v1-archive`, sổ lỗi mang sang v2 (Phụ lục A) |
+| D2-02 | Không sửa mò (D11 cũ): cần đường tái hiện hoặc chứng minh cơ chế |
+| D2-03 | Hồi quy thì revert trước (D18 cũ) |
+| D2-04 | Core không chứa FS/fd/block/console/args/demo; thay đổi ABI/ngữ nghĩa qua Agent 0 |
+| D2-05 | Commit chạm `kernel/`, `lib/` phải xanh `--fast`; lỗi logic tái hiện trên sim, kèm `.scn` đỏ trước |
+| D2-06 | Luật cấu trúc mục 8 là cổng cứng |
+| D2-07 | Không retry ở mọi cổng; flake là T-INFRA |
+| D2-08 | Test kernel không nằm trong boot sản phẩm |
+| D2-09 | Trace luôn bật, không có cờ tắt |
+| D2-10 | Chỉ riscv64 tới K6; aarch64/x86 sau K6 |
+| D2-11 | Một nguồn sự thật trong `spec/`; code sinh không sửa tay |
+| D2-12 | Đồng thời theo S1..S4 theo số đo; đọc trường công bố của hart khác được phép |
+| D2-13 | Kịch bản `.scn` chạy cả sim và QEMU |
+| D2-14 | Object trong Untyped qua objmem (API closure, Miri từ đầu) |
+| D2-15 | ABI đóng băng 5 thứ + `abi_version` + golden (mục 4.21) |
+| D2-16 | MCS: donation từ K1b, thuật toán refill sau K6 |
+| D2-17 | `unsafe` chỉ trong danh sách crate đóng |
+| D2-18 | Ứng dụng hệ thống viết bằng Rust std, crate thuần Rust; libc, wasm để sau |
+| D2-19 | ViScript là ngôn ngữ script và shell chính; Linux qua máy ảo |
+| D2-20 | OS trước; ViScript khi không còn task OS READY (ngoại lệ ST-02a) |
+| D2-21 | Giấy phép MIT OR Apache-2.0; `cargo-deny` allowlist; `THIRD-PARTY.md` tự sinh; license nhúng ảnh; không NOTICE |
+| D2-22 | Duyệt spec bằng dòng APPROVED; Agent 2 soạn nháp ở REVIEW, sau duyệt chỉ Agent 0 sửa |
+| D2-23 | D-08 (không đụng đồ hoạ/ViFastKey) còn hiệu lực tới K6 |
+| D2-24 | Mọi soak trên máy `ps`, N theo `n ≥ 3/p` |
 
-Giữ từ v1: hồi quy thì revert; hai phiên kẹt cùng ý thì đổi nhánh; không nới ngưỡng; không sửa `accept()`; fmt trước commit.
+Ba ghi chú bắt buộc (QD §2.1, §2.3, §5.3):
 
+| Ghi chú | Nội dung |
+|---|---|
+| D-01..D-08, D11, D18 | **Vẫn hiệu lực cho v1**; v2 dùng D2-xx, không dùng lại mã D-xx (chi tiết mục 2.3). |
+| Duyệt | bằng dòng `APPROVED <ngày> Agent 0` trong `QUEUE.md` + một mục D2 — **không duyệt bằng chat**. |
+| Soak | mọi soak trên máy `ps`; **N = 1.000** với p mục tiêu 0,3 % (mục 9.12). |
+
 ---
 
 ## 13. Giao việc vòng 1 cho Agent 2
 
-Đưa vào `v2/agent/QUEUE.md`. Ngân sách mặc định 3 giờ/task. Dòng ViScript có QUEUE riêng (tài liệu ViScript mục 20).
+Đưa vào `v2/agent/QUEUE.md`; bảng dưới là bản rút gọn, QUEUE có đủ 5 khối tự chứa. Ngân sách **3 h/task**; task lớn tách. `agentctl next` bỏ qua `REVIEW`.
 
-| ID | Task | Phụ thuộc | Tiêu chí xong |
-|---|---|---|---|
-| T01 | Đóng v1: commit r323; `pkill -f qemu-system`; soak x86_64 3 luồng `VIFAST_NO_RETRY=1` n=750; tag `v1-final` | | Báo cáo n, số treo, số `TIME MONOTONIC BAD`/`C TIME BACK`, BUILD_ID |
-| T02 | Khung workspace `v2/` theo mục 8 (crate rỗng biên dịch được), `v2/agent/` với các tệp mục 11.2, LICENSE-MIT + LICENSE-APACHE | | `cargo build` toàn workspace |
-| T03 | Luật cấu trúc: kiểm dòng, `cfg(target_arch)`, danh sách unsafe, `deps.toml`, header SPDX; lệnh `v2 check`, `v2 gate --fast` khung | T02 | Cổng chạy, có case âm cho từng luật |
-| T04 | `spec/abi.toml` đầy đủ mục 4.20, 4.21 (cả object chưa cài) + `kernel.toml` | T02 | Gửi Agent 0 duyệt (BLOCKED tới khi duyệt) |
-| T05 | vifast-gen: sinh vifast-abi2, bảng dispatch, stub urt, `docs/abi.md`; gen-check trong cổng | T04 | Code sinh biên dịch; sửa tay bị cổng bắt |
-| T06 | vifast-agentctl: next/start/done/block/report, `--project` | T02 | Dùng được cho chính vòng này |
-| T07 | vifast-objmem + Miri | T02 | Miri xanh; fuzz chuỗi place/get/destroy |
-| T08 | vifast-ilist trên ObjStore | T07 | Property test so Vec |
-| T09 | Untyped (watermark, Untyped con, device) + preempt khung + Retype làm tiếp | T05, T07 | Test ép preempt mọi bước |
-| T10 | CNode 2 cấp + slot + MDB + Copy/Mint/Move/Mutate/Delete/Revoke/Rotate | T09 | Property test so mô hình cây |
-| T11 | Bảng ThreadState + transition + xuất tài liệu | T08 | Mọi cặp cấm bị từ chối |
-| T12 | vifast-scenario + SimPlatform (chưa VM) + bộ lập lịch event + checker 1, 2, 3, 9 | T09, T10 | Seed tái hiện được; dump trace |
-| T13 | vifast-model phần Untyped/CNode | T10 | Khớp core trên 10.000 seed |
-| T14 | vifast-traceformat + ring + vifast-trace | T12 | Giải mã dump từ sim |
-| T15 | vifast-shrink + `v2 repro` + `test/regress/` | T12 | Thu nhỏ được một đột biến cố ý |
-| T16 | Báo cáo R1 (MD + JSON) | tất cả | Theo mục 11.7 |
+| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
+|---|---|---|---|---|---|
+| KT-01 | tag `v1-archive` trên `ca2eaf5`; `docs/v1/KNOWN-ISSUES.md`; chuyển tài liệu v1 vào `docs/v1/` | — | 1 h | READY | tag có message "T33a mở"; KNOWN-ISSUES đủ 7 mục |
+| KT-02 | khung `v2/` + `v2/agent/` (không ghi đè), LICENSE tham chiếu root, `vifast-fsutil` | — | 3 h | READY | `cargo build` workspace rỗng xanh |
+| KT-03 | `v2 check` + `v2 gate --fast` khung: fmt, clippy, luật cấu trúc, L0, SPDX; chưa gen-check/L1 | KT-02 | 3 h | BLOCKED | cổng chạy; 5 luật có 5 case âm |
+| KT-04a | bảng ánh xạ v1→v2 + `spec/abi.toml` lõi (15 syscall, 19 lỗi, layout 48 B, riscv64) | KT-02 | 3 h | READY | abi.toml parse; đủ bảng ánh xạ |
+| KT-04b | object reserved: số + `NotSupported` | KT-04a | 2 h | BLOCKED | số không trùng; trả NotSupported |
+| KT-04c | 20 kịch bản `.scn` chỉ bằng ABI + kỳ vọng | KT-04a | 3 h | BLOCKED | 20 tệp + 5 ca trên mock |
+| KT-04d | gửi Agent 0 duyệt (checklist + 20 kịch bản) | KT-04a,b,c | 1 h | REVIEW | `APPROVED <ngày> Agent 0` |
+| KT-05 | `vifast-gen` + gen-check + `docs/abi.md` | KT-04a | 3 h | BLOCKED | code sinh biên dịch; sửa tay bị bắt |
+| KT-06 | `vifast-agentctl --project v2\|viscript` | KT-02 | 2 h | BLOCKED | dùng được; JSON có `project` |
+| KT-07 | K0.5: boot riscv64 OpenSBI; DebugPutStr/Halt; TAP; 50 boot | KT-02, KT-03 | 3 h | BLOCKED | 50 boot 0 lỗi |
+| KT-08 | sim harness + store giả + scheduler 1 hart | KT-07 | 3 h | BLOCKED | 1 kịch bản end-to-end |
+| KT-09 | Untyped trên sim | KT-08 | 3 h | BLOCKED | invariant tổng byte |
+| KT-10 | CNode/MDB test-first | KT-09 | 3 h | BLOCKED | property test cây |
+| KT-11 | `vifast-model` Untyped/CNode | KT-10 | 2 h | BLOCKED | khớp 10.000 seed |
+| KT-12 | `vifast-traceformat` + ring | KT-08 | 2 h | BLOCKED | giải mã dump |
+| KT-T26 | host test `vifast-fs` + block giả + cache, `MAX_IO=256`, RAW | — | 3 h | READY | test đỏ trước |
+| KT-R1 | báo cáo `R1-v2` + `blocked[]` | task READY | 1 h | READY | MD + JSON có `project` |
 
-Nhánh độc lập để chạy qua đêm: (T01), (T02 → T03/T06), (T07 → T08 → T11), và toàn bộ dòng ViScript VT01 tới VT09. T04 chờ duyệt nên T05, T09, T10, T12 tới T15 có thể dừng ở BLOCKED; khi đó Agent 2 làm T07, T08, T11 và dòng ViScript.
+Nhánh chạy song song: (KT-01, KT-02, KT-04a, KT-T26), (KT-07 → KT-08 → KT-09 → KT-10 → KT-11), (KT-05/KT-06 sau KT-02/KT-04a). KT-04d **REVIEW**; APPROVED mở KT-05.
 
 ---
 
@@ -984,3 +1094,38 @@
 | Agent 2 trôi khi chạy dài | STATE.md sau mỗi bước, ngân sách, điều kiện dừng, báo cáo JSON |
 | Dòng ViScript tranh tài nguyên dòng lõi | QUEUE riêng; ViScript dùng làm nhánh độc lập khi lõi BLOCKED hoặc theo tỉ lệ người điều phối đặt |
 | Phần cứng thật (GPU, Wi-Fi) quá lớn | Ưu tiên virtio; phần cứng thật theo từng mốc HW |
+| **Cắt còn 1 arch tới K6** (D2-10) làm lỗi aarch64/x86_64 dồn về sau | OpenSBI là đường duy nhất phải đúng; `PageTableFormat`/`Mmu` dùng chung từ K2; conformance viết sẵn cho 3 arch, chạy lại ngay khi port (K4'/K5') |
+| **MCS cắt phần thuật toán** (refill nhiều đoạn, timeout fault) làm phát hiện muộn lỗi ngữ nghĩa ngân sách | Donation + `Sleeping{reason=Budget}` vào từ K1b; mô hình tham chiếu có budget từ đầu; phần hoãn là chi tiết trong, không đổi ABI (D2-16) |
+| **objmem là crate unsafe duy nhất** phải tự viết cấp phát thô + Miri | Miri từ commit đầu (D2-14); test `with_many_mut` kiểm alias từng cặp; fuzz chuỗi thao tác; cấm tạo tham chiếu tới cả vùng |
+| **Phạm vi hai dòng (OS + ViScript)** tranh ngân sách | D2-20: OS trước, ViScript chỉ khi QUEUE v2 hết task READY (ngoại lệ ST-02a); hai QUEUE độc lập, mỗi task ≤ 3 h |
+
+---
+
+## Phụ lục A. Sổ lỗi v1 mang sang v2
+
+| Lỗi v1 | Mô tả | Ca kiểm trong v2 |
+|---|---|---|
+| T33a | Đồng hồ lùi sau migrate/reset | Kịch bản sim "đơn điệu sau migrate" + conformance chéo hart |
+| T26 | FS 20 % đỏ ở `MAX_IO=256` | **KT-T26**: host test + block giả + cache, RAW read-your-own-write |
+| T31 | PAGER2 nhận message lặp | Sim pager 4 client + checker "giao đúng một lần" + đột biến "phát lại Call khi Resume" |
+| T-IPC | Lỗi cổng đo bị coi là lỗi sản phẩm | Luật: mọi harness mới phải có case âm chứng minh bắt được lỗi trước khi tuyên bố |
+| x86 AP→BSP + PCID | Shootdown chưa xử lý hộp thư khi chờ ack | Đột biến "bỏ xử lý hộp thư khi chờ ack" + conformance TLB khi tới x86 |
+| Audit P1-5 | `Ready` nhưng không trong runqueue | Invariant hàng đợi khớp trạng thái + kịch bản riêng |
+| Audit Q1-A | Kênh wake lệch 3 port | `vifast-sync` dùng chung mọi arch |
+
+## Phụ lục B. Câu hỏi mở Q1–Q12 của dossier
+
+| Câu | Trả lời |
+|---|---|
+| Q1 mục tiêu gần | K0 → K0.5 → K1a; không hứa ngày |
+| Q2 chiến lược lỗi hiếm | Sim tất định + mô hình; soak chỉ xác nhận |
+| Q3 T33a | Không sửa trên v1; mang sang v2 bằng kịch bản (Phụ lục A) |
+| Q4 worker C | Thành kịch bản đơn điệu đồng hồ |
+| Q5 T26 | KT-T26 trên host |
+| Q6 `--strict` | Thay bằng D2-07 (không retry) |
+| Q7 tách `lib.rs` | Không; v2 viết mới |
+| Q8 trần tĩnh | Object trong Untyped; `.data+.bss ≤ 256 KiB`; `[GT]` đo ở K0.5 |
+| Q9 đóng băng x86 | x86 sau K6 (D2-10) |
+| Q10 archive tài liệu | `docs/v1/` (báo cáo, decisions, overnight-log); v2 ở `docs/v2/`, ViScript ở `docs/viscript/` |
+| Q11 định nghĩa xong | Mỗi mốc có cột tiêu chí; "tương đương v1" có danh sách đo được |
+| Q12 nguồn số liệu | `STATUS.md` sinh bởi nightly; tài liệu chỉ chứa `[TK]`/`[ƯL]` |

````

---

# PHẦN B — UNIFIED DIFF: ViScript

So với bản Agent 0 gửi. Phụ lục A mới (~122 luật EBNF) nằm trong phần này.

````text
--- review/src/ViScript-thiet-ke.md	2026-11-12 14:34:15
+++ docs/viscript/ViScript-thiet-ke.md	2026-11-12 15:46:34
@@ -4,8 +4,29 @@
 Quan hệ với ViFastOs: ViScript là **sản phẩm độc lập**, chạy trên Linux, macOS, Windows, WebAssembly và ViFastOs. Tài liệu `ViFastOs-v2-thiet-ke.md` (mục 7.3) chỉ mô tả phần tích hợp; mọi quyết định về ngôn ngữ nằm ở đây.
 Giấy phép mã nguồn: **MIT OR Apache-2.0** (giống Rust).
 
-Nhãn: [ƯL] ước lượng cần đo lại; [GT] giả thuyết cần kiểm.
+Nhãn: `[V1:CODE]`, `[V1:ĐO]`, `[V1:DOC]` = lấy từ dossier v1, giữ độ tin cậy gốc; **`[TK]`** = quyết định thiết kế (**mặc định** cho dòng không nhãn); `[GT]` = giả thuyết cần kiểm; `[ƯL]` = ước lượng cần đo lại.
 
+## Thay đổi sau review (2026-11-12)
+
+Bản này đã vá theo [`docs/QUYET-DINH-sau-review.md`](../QUYET-DINH-sau-review.md) (gọi tắt **QD**) — nguồn sự thật; chỗ nào QD nói khác tài liệu này thì **QD thắng**. Đặc tả từ vựng chi tiết (bảng token, escape, đơn vị, maximal munch, golden token) nằm ở `docs/viscript/SPEC-lex.md` — **đã có**; ST-02a duyệt/bổ sung. Phụ lục A dưới đây là **EBNF tự chứa** và là chuẩn tắc cho parser.
+
+| # | Nhóm đã vá | Mục |
+|---|---|---|
+| 1 | Không gian tên: task ViScript `VT01..VT10` → **`ST-01..ST-10`**; lỗi gốc `G1..G6` → `R1..R6`; tag commit `[ST-##]` | 19, 20, 21 |
+| 2 | Từ vựng: NFC toàn tệp trước khi lex; thụt lề suy đơn vị; tab = `VS0107` nhưng **vẫn lex**; bảng literal mới; bỏ `50%` → `50pct`; escape/nội suy; cột theo Unicode scalar | 2.1–2.4, 3.5 |
+| 3 | Parse chế độ lệnh **hai pha**: `vis-syntax` → `CmdCST` → `cmd_lower` (trong `vis-types`) → HIR; "cùng AST" đổi thành "cùng **HIR**" | 9.1, 9.3, 14.1, 18.2 |
+| 4 | Ngữ nghĩa số: bảng đầy đủ `%`, `**`, bit, float/NaN, `size/duration/percent`; **bảng ưu tiên 12 mức** | 3.6, 4.5 |
+| 5 | Kiểu và optional: thêm `Future[T]`, `Task[T]`, `AnyError`; bỏ `s[i]`; tự nâng `T → T?`; pattern `none/some/ok/err` | 3, 4.3–4.4 |
+| 6 | Lỗi: `trait Error: Show`, `AnyError`, `?` tự chuyển; **bảng mã thoát**; panic payload + arena lệnh | 6 |
+| 7 | Chế độ lệnh: metachar, `$env.NAME`, `;`/`&`/`--`/`-5`, closure-arg, `shell_item`, `from json[T]`, `complete` | 9 |
+| 8 | Host/ViShell: proc suspend/resume/cancel + wait; shell sở hữu tty, con nhận pipe, pty để sau | 12, 15 |
+| 9 | Crate/plugin/cổng: `vis-host-vifast` ngoài `viscript/`; plugin động tắt ở chế độ hạn chế; cổng **doc-examples parse** | 14.2, 17, 18.4 |
+| 10 | **Phụ lục A viết lại** thành EBNF tự chứa; Phụ lục B sửa ví dụ; thêm VD-11..VD-20 | A, B, 22 |
+
+Ba quyết định lớn ảnh hưởng thi công: (a) QD §1 **Q-D** — parse chế độ lệnh hai pha; `VT06` cũ chỉ làm `CmdCST` + golden, `cmd_lower` là task vòng 2; (b) QD §7.4 — **ST-02a chặn ST-04..ST-09**; (c) QD §7.3 — **`|` không bao giờ là toán tử nhị phân trong biểu thức**, pattern vẫn dùng `|`.
+
+Chỗ QD chưa nói và tài liệu này chọn mặc định (kèm dẫn chứng): xem [`review/work/PATCH-NOTES-viscript.md`](../../review/work/PATCH-NOTES-viscript.md).
+
 ---
 
 ## Mục lục
@@ -34,7 +55,7 @@
 21. Các lựa chọn đã chốt
 22. Quyết định
 23. Rủi ro
-Phụ lục A. Ngữ pháp rút gọn
+Phụ lục A. EBNF tự chứa (chế độ script + chế độ lệnh)
 Phụ lục B. Chương trình mẫu
 
 ---
@@ -75,7 +96,7 @@
 
 ## 1. Nguyên tắc
 
-1. **Một ngôn ngữ, hai chế độ cú pháp** (script và lệnh), cùng cây cú pháp, cùng kiểm kiểu, cùng VM.
+1. **Một ngôn ngữ, hai chế độ cú pháp** (script và lệnh), cùng **HIR**, cùng kiểm kiểu, cùng VM.
 2. **Kiểu tĩnh, không lối thoát ngầm.** Không `any`. Dữ liệu chưa rõ cấu trúc có kiểu riêng `Json` và phải chuyển đổi tường minh.
 3. **Chữ ký hàm ghi kiểu, thân hàm tự suy.** Suy kiểu cục bộ hai chiều, không suy toàn chương trình.
 4. **Ngữ nghĩa giá trị.** Gán là sao chép về mặt ngữ nghĩa, cài đặt bằng chia sẻ + copy-on-write.
@@ -92,46 +113,77 @@
 
 ### 2.1 Tệp, mã hoá, định danh, từ khoá
 
-- Phần mở rộng `.vis` (script), `.vish` (tệp toàn bộ ở chế độ lệnh). UTF-8 bắt buộc. Dòng đầu tuỳ chọn `#!/usr/bin/env vis`.
-- **Từ khoá tiếng Anh** (giống Python, Rust, JS/TS).
+- Phần mở rộng `.vis` (script), `.vish` (tệp ở chế độ lệnh, xem 9.1). UTF-8 bắt buộc. Dòng đầu tuỳ chọn `#!/usr/bin/env vis`.
+- **Chuẩn hoá NFC trước khi lex** (QD §7.1): **toàn bộ** tệp nguồn được chuẩn hoá Unicode NFC **trước** khi lex. Hệ quả: định danh, literal `str`/`char` và **cột chẩn đoán** đều tính trên dạng NFC; `'ố'` gõ dạng tổ hợp hay dựng sẵn đều là **một** Unicode scalar và là một literal `char` hợp lệ; `số_lượng` viết dạng tổ hợp (NFD) và dạng dựng sẵn (NFC) là **cùng một** tên. `vis fmt` ghi ra NFC. Vị trí báo lỗi vẫn trỏ được về tệp gốc vì cả lexer và editor cùng chuẩn hoá một cách tất định.
+- **Từ khoá tiếng Anh** (giống Python, Rust, JS/TS). Danh sách:
+  `let var const fn cmd type enum trait impl alias newtype pub use if elif else match for in while loop break continue return and or not true false none self async await scope test extern derives with has dyn ref`
+  - **Thêm `const`** (dùng cho `const_decl`, Phụ lục A).
+  - **Bỏ `as`**: bản này không có cú pháp alias trong `use`; `as` là định danh bình thường.
+  - **`test` là từ khoá theo ngữ cảnh**: chỉ là từ khoá khi đứng đầu item và ngay sau là `fn` (`test fn name():`). Ở vị trí khác `test` là định danh, nên `test.run()` hợp lệ.
+  - Module kiểm thử đổi tên `test` → **`std.testing`** (xem 13), nhờ vậy `use std.testing` lex được.
 - **Định danh Unicode** theo UAX #31: được viết tiếng Việt, tiếng Nhật (`số_lượng`, `tính_tổng`). So sánh sau chuẩn hoá NFC, nên `ố` gõ bằng tổ hợp hay dựng sẵn đều là một tên.
 - Lint cảnh báo định danh trộn chữ dễ nhầm (ví dụ Latin + Cyrillic giống nhau) để tránh tấn công homoglyph.
 
-Danh sách từ khoá: `let var fn cmd type enum trait impl alias newtype pub use as if elif else match for in while loop break continue return and or not true false none self async await scope test extern derives with has dyn ref`.
-
 ### 2.2 Khối và thụt lề
 
-- Khối mở bằng `:` cuối dòng, thân thụt lề **4 dấu cách**. Tab bị cấm (lỗi từ vựng, `vis fmt` tự sửa).
-- Trong ngoặc `()`, `[]`, `{}` được xuống dòng tự do.
-- Thân ngắn một dòng sau `:` được phép: `if x > 0: return x`.
+- Khối mở bằng `:` ở cuối dòng.
+- **Đầu vào: thụt lề nhất quán bất kỳ.** Lexer **suy đơn vị thụt lề** từ **cấp thụt lề đầu tiên** trong tệp (dòng đầu tiên có thân khối); mọi mức sau phải là bội số của đơn vị đó. Trộn 2 và 4 dấu cách trong cùng tệp là lỗi (mã trong `ST-02b`). Nhờ vậy dán mã từ nơi khác không chết ngay dòng đầu, mà vẫn giữ "một kiểu duy nhất" ở đầu ra.
+- **Đầu ra: `vis fmt` luôn 4 dấu cách** [TK].
+- **Tab:** lexer báo **cảnh báo-lỗi `VS0107`**, quy ước **tab = 4 cột**, và **vẫn lex tiếp** (không dừng). `vis check` coi `VS0107` là **lỗi**; `vis fmt` **sửa** tab thành dấu cách. Chính vì "vẫn lex tiếp" mà `vis-fmt` có CST để sửa — đây là chỗ vá mâu thuẫn cũ "tab là lỗi từ vựng nhưng `vis fmt` tự sửa".
+- Thân ngắn một dòng sau `:` được phép: `if x > 0: return x`. Khối một dòng **không** sinh `INDENT`/`DEDENT`.
+- **Bốn ca biên** (luật cho lexer; golden token ở `SPEC-lex.md`):
+  1. **Dòng trống hoặc chỉ có chú thích** không sinh `INDENT`/`DEDENT` và **không đóng khối**; chú thích ở cột 0 vẫn nằm trong khối đang mở.
+  2. **Chuỗi nhiều dòng** (`"""..."""`, `r"""..."""`) được nuốt **trọn** thành một token **trước** khi tính cột, nên nội dung ở cột 0 không đóng khối và không sinh `DEDENT`.
+  3. **Trong `()`, `[]`, `{}`** tắt `INDENT`/`DEDENT`: xuống dòng tự do, không sinh `NEWLINE` có nghĩa.
+  4. **`DEDENT` ngầm** ở `EOF` và ở **cuối mỗi input REPL** (dán khối nhiều dòng vào shell rồi Enter là hết khối).
+- **Nối dòng bằng `|>`:** một dòng bắt đầu bằng `|>` ở **độ sâu ngoặc 0** được nối với dòng trước (không sinh `NEWLINE`). Đây là ngoại lệ **duy nhất** của luật "xuống dòng ngoài ngoặc chỉ sau dấu `:`" (QD §7.4).
+- CST giữ **độ rộng thụt lề gốc** (để `vis fmt` lossless); AST/HIR chỉ giữ cấu trúc khối.
 
 ### 2.3 Chú thích
 
-- `#` tới cuối dòng.
-- `##` là tài liệu, gắn với khai báo kế tiếp (dùng cho `vis doc` và LSP).
+- **Chế độ script:** `#` ngoài chuỗi **luôn** là chú thích, cắt tới cuối dòng.
+- **Chế độ lệnh:** `#` chỉ là chú thích khi **đứng đầu từ** — đầu dòng hoặc ngay sau khoảng trắng. Ở giữa từ, `#` là ký tự thường: `https://x/y#frag` giữ nguyên `#frag` (QD §7.1, §7.2).
+- `##` là tài liệu, gắn với khai báo kế tiếp (dùng cho `vis doc` và LSP). `##` chỉ có nghĩa khi đứng đầu dòng (sau thụt lề).
+- `#|` mở đầu một dòng chú thích meta của script (`deps`, xem mục 8); phần còn lại của dòng là chú thích theo luật trên, định dạng do mục 8 quy định.
 
 ### 2.4 Literal
 
-| Loại | Ví dụ | Kiểu |
+| Loại | Ví dụ | Kiểu / luật |
 |---|---|---|
-| Số nguyên | `42`, `1_000_000`, `0xff`, `0b1010` | `int` |
-| Số thực | `3.14`, `1e-9` | `float` |
+| Số nguyên | `42`, `1_000_000`, `0xff`, `0b1010` | `int` (i64). `_` **chỉ** đứng giữa hai chữ số (`1_0` đúng; `_1`, `1_`, `1__0` sai). **Literal tràn i64 là lỗi biên dịch** (khác tràn lúc chạy = panic) |
+| Số thực | `3.14`, `1e-9` | `float` (f64). **Bắt buộc có chữ số sau `.`**: `1.` sai; `1..2` là **range** (`1` `..` `2`), không phải float |
 | Bool | `true`, `false` | `bool` |
-| Chuỗi | `"xin chào {name}"`, `r"C:\raw"`, `"""nhiều dòng"""` | `str` |
-| Ký tự | `'a'`, `'ố'` | `char` |
-| Bytes | `b"\x00\x01"` | `bytes` |
-| Kích thước | `5kb`, `5kib`, `2mb`, `1gib` | `size` |
-| Thời lượng | `250ms`, `2h30m`, `1d` | `duration` |
-| Phần trăm | `50%` | `percent` |
-| Đường dẫn | `p"./data/a.txt"` (script), `./data/a.txt` (chế độ lệnh) | `path` |
+| Ký tự | `'a'`, `'ố'` | `char` (một Unicode scalar; nguồn đã NFC nên `'ố'` NFD thành một scalar) |
+| Chuỗi | `"xin chào {name}"`, `r"C:\raw"`, `"""nhiều dòng"""`, `r"""thô"""` | `str` (xem bảng dưới) |
+| Bytes | `b"\x00\x01"` | `bytes` (chỉ ASCII + `\xNN`) |
+| Kích thước | `5kb`, `5kib`, `2mb`, `1gib` | `size`; đơn vị `b kb mb gb tb kib mib gib tib`, **không dấu cách**; `kb`=1000, `kib`=1024 |
+| Thời lượng | `250ms`, `2h30m`, `1d` | `duration`; đơn vị `ns us ms s m h d w`; ghép **giảm dần** là **một token** (`2h30m` đúng, `30m2h` sai) |
+| Phần trăm | `50pct` | `percent`; **bỏ literal `50%`** (QD §7.1): `%` **chỉ** là modulo; job spec viết `job 1`, không phải `%1` |
+| Đường dẫn | `p"./data/a.txt"` (script), `./data/a.txt` (chế độ lệnh, từ trần) | `path` |
 | List | `[1, 2, 3]` | `list[int]` |
 | Map | `{"a": 1, "b": 2}` | `map[str, int]` |
 | Set | `set{1, 2}` | `set[int]` |
 | Tuple | `(1, "a")` | `(int, str)` |
 | Bản ghi ẩn danh | `{name: "An", age: 20}` | `{name: str, age: int}` |
 
-`kb` = 1000, `kib` = 1024. Chuỗi nội suy `"{expr}"`, định dạng `"{x:>8}"`, `{{` để ra `{`.
+**Năm dạng chuỗi** (QD §7.1):
 
+| Dạng | Escape | Nội suy | Nhiều dòng |
+|---|---|---|---|
+| `"..."` | có | có | không |
+| `r"..."` | **không** (thô) | **không** | không |
+| `"""..."""` | có | có | có |
+| `r"""..."""` | **không** | **không** | có |
+| `b"..."` | chỉ ASCII + `\xNN` | không | không |
+| `p"..."` | như `"..."`, kết quả là `path` | có | không |
+
+- **Escape** (chỉ trong `"..."`, `"""..."""`, `b"..."`, `p"..."`): `\n` `\t` `\r` `\0` `\\` `\"` `\'` `\{` `\}` `\u{X..X}` (1–6 chữ số hex) [TK]; `\xNN` **chỉ** trong `b"..."`; `\` khác là lỗi. Trong `r"..."`/`r"""..."""` **không** có escape: `\` là ký tự thường.
+- **Nội suy:** `{expr}` hoặc `{expr:spec}`, `spec` theo cú pháp Rust format (`{x:>8}`, `{n:.2}`); `{{` và `}}` cho ra `{` `}` thật. Biểu thức trong `{}` là biểu thức đầy đủ, được kiểm kiểu; giá trị in qua `Show` (3.5).
+- **`set{` dính liền** là set literal; có khoảng trắng (`set {`, `set (`, `counter.set(...)`) thì `set` là **định danh**.
+- **Maximal munch:** `??`, `?.`, `?` → `x??y` là `x ?? y`; `|>` dài nhất trước `|`.
+- **Cột chẩn đoán** đếm theo **Unicode scalar, bắt đầu từ 1**, trên văn bản **đã NFC**; LSP đổi sang **UTF-16 ở biên** giao thức (`Position.character` của LSP là UTF-16), nội bộ `vis-diag` luôn là scalar (QD §7.1).
+- `kb` = 1000, `kib` = 1024.
+
 ---
 
 ## 3. Hệ kiểu
@@ -142,21 +194,25 @@
 |---|---|
 | Số | `int` (i64, tràn là panic; phép quấn tường minh `wrapping_add`), `float` (f64) |
 | Logic, ký tự | `bool`, `char` |
-| Văn bản | `str` (UTF-8 bất biến), `bytes` |
+| Văn bản | `str` (UTF-8 bất biến, **không có `s[i]`**, so sánh theo dãy scalar), `bytes` |
 | Hệ thống | `path`, `size`, `duration`, `percent`, `instant`, `datetime` |
 | Tập hợp | `list[T]`, `map[K, V]`, `set[T]`, `(A, B, ...)` |
-| Rỗng, lỗi | `T?`, `Result[T, E]`, `never`, `()` |
+| Rỗng, lỗi | `T?`, `Result[T, E]`, `AnyError`, `never`, `()` |
 | Hàm | `fn(A, B) -> C` |
-| Luồng | `stream[T]` (lười, kéo) |
+| Luồng, bất đồng bộ | `stream[T]` (lười, kéo), `Future[T]` (kết quả lười), `Task[T]` (future đã spawn) |
 | Dữ liệu động | `Json` (enum: Null, Bool, Num, Str, Arr, Obj) |
 | Tham chiếu chia sẻ | `ref[T]`, `weak[T]` |
 | Quyền | `cap[Dir]`, `cap[File]`, `cap[Net]`, `cap[Proc]`, `cap[Env]`, `cap[Term]`, `cap[Service[P]]` |
 
-`bigint`, `decimal` là thư viện, không nằm trong lõi.
+- `Future[T]` là giá trị lười do gọi `async fn` sinh ra; `Task[T]` là future **đã được spawn** trong `scope`, có handle để `await`/huỷ (10.1).
+- `AnyError` là **kiểu lỗi xoá kiểu** dùng ở ranh giới (6.1).
+- `T??` là **lỗi kiểu** — optional không lồng (4.4).
+- `percent` so sánh được với `float` **theo tỉ lệ** (`50pct == 0.5`, 3.6); `ps.cpu` có kiểu `percent`.
+- `bigint`, `decimal` là thư viện, không nằm trong lõi.
 
 ### 3.2 Kiểu người dùng
 
-```python
+```vis
 type User derives Eq, Show, Json:
     name: str
     age: int
@@ -171,15 +227,16 @@
 newtype Email = str             # kiểu mới, không tự chuyển đổi với str
 ```
 
-- `derives` sinh cài đặt trait chuẩn: `Eq`, `Ord`, `Hash`, `Show`, `Json`, `Default`.
+- `derives` sinh cài đặt trait chuẩn: `Eq`, `Ord`, `Hash`, `Show`, `Json`, `Default`. Kiểu chứa `float` **không** derive được `Eq`/`Hash` (3.6).
 - Tạo bản ghi: `User(name: "An", age: 20)`; tham số có tên bắt buộc khi bản ghi có hơn 2 trường.
-- Cập nhật: `u with age: 21` trả bản ghi mới.
+- Cập nhật: `u with age: 21` trả bản ghi mới. Trường **bất biến** ở bản này (không có `var` cho trường).
+- `alias` là tên khác của cùng kiểu; `newtype` là kiểu mới, phải chuyển đổi tường minh (`Email(s)` / `e.0`).
 
 ### 3.3 Kiểu cấu trúc cho dữ liệu bảng
 
 Bản ghi ẩn danh so khớp theo cấu trúc. Hàm có thể nhận "mọi bản ghi có trường `size: size`":
 
-```python
+```vis
 fn total[R: has {size: size}](rows: list[R]) -> size:
     rows.map(r => r.size).sum()
 ```
@@ -197,19 +254,41 @@
 
 Không chuyển đổi ngầm giữa số. `float(x)`, `int.trunc(f)`, `int.parse(s) -> Result[int, ParseError]`. Ngoại lệ duy nhất: literal số nguyên ở vị trí `float`.
 
-### 3.6 Phép chia
+- **`str(x)` = `x.show()`** (QD §7.3) — cùng một đường, không có "ép kiểu chuỗi" riêng. Mọi kiểu in được đều cài `Show` (qua `derives Show` hoặc viết tay); nội suy `{expr}` dùng đúng `show()`.
+- **Path từ chuỗi phải tường minh**: `path("...")` hoặc literal `p"..."`; **không** còn chuyển ngầm `str → path`, kể cả ở tham số mặc định. Tham số mặc định kiểu `path` viết `path = p"."`.
+- `p"..."` là literal `path`; `path("...")` là hàm chuyển đổi tường minh cùng nghĩa (dùng khi chuỗi đến từ biến).
+- `str` **không có** `s[i]`: dùng `.chars()`, `.graphemes()`, `.bytes()`, `.byte_at(i)?`; `byte_len()`, `char_count()`, `grapheme_count()` (3.1, 4.5).
+- So sánh `str` theo **dãy Unicode scalar** (sau NFC), không theo grapheme.
 
-- `int / int -> int` (cắt về 0; chia 0 là panic), `float / float -> float`.
-- `int.div_floor`, `int.checked_div -> int?`.
-- Lint cảnh báo khi chia hai literal nguyên (`1 / 2`), gợi ý `1.0 / 2.0`.
+### 3.6 Ngữ nghĩa số
 
+Bảng bắt buộc (QD §7.3, chấp nhận bảng D điểm 3):
+
+| Biểu thức | Kiểu | Luật |
+|---|---|---|
+| `int / int` | `int` | cắt về 0; `/0` **panic**; có `div_floor`, `div_euclid`, `checked_div -> int?` |
+| `int % int` | `int` | dấu theo **số bị chia** (C/Rust: `-7 % 2 == -1`); `%0` **panic**; có `rem_euclid`, `mod_floor` |
+| `int ** int` | `int` | **kết hợp phải**, **cao hơn** `*`; mũ âm **panic**; tràn **panic**; có `checked_pow -> int?` |
+| `float` cơ bản | `float` | f64; `/0.0` theo IEEE 754 (±inf/NaN), **không panic** |
+| NaN | — | `==` trả `false` (kể cả `nan == nan`); float **không** có `Eq`/`Ord`/`Hash`; kiểu chứa `float` không `derives Eq`/`Hash`; sắp xếp dùng `total_cmp` |
+| Bit | `int` | `bit_and bit_or bit_xor bit_not shl shr ushr` trên 64 bit có dấu (`bit_not(0) == -1`); `shr` **số học** (giữ dấu), `ushr` **logic**; số đếm ngoài `0..63` **panic**; có `checked_shl`, `checked_shr` |
+| `size ± size` | `size` | tràn panic; `size * int`, `size / int`; `size / size -> float` |
+| `duration ± duration` | `duration` | `duration * int`, `duration / int`, `duration / duration -> float` |
+| `datetime - datetime` | `duration` | `datetime ± duration -> datetime` **theo UTC**, không theo lịch (DST để thư viện `time`) |
+| `percent * size` | `size` | `percent * int -> int` |
+| `percent` vs `float` | — | so sánh **theo tỉ lệ**: `50pct == 0.5`, `25pct < 0.3`; `percent` lưu tỉ lệ 0..1 |
+
+- `int` tràn lúc chạy = panic; literal tràn i64 = lỗi biên dịch (2.4).
+- Lint: cảnh báo khi kết quả `int/int` được so sánh hoặc dùng với `float`/`percent` (lint `percent-of-int-division`); lint chia hai literal nguyên giữ nhưng **hạ mức** cảnh báo.
+- `0 ** 0 == 1`; `float` luỹ thừa theo `f64.powf`.
+
 ---
 
 ## 4. Biểu thức, câu lệnh, pattern
 
-### 4.1 Biến
+### 4.1 Biến và scope
 
-```python
+```vis
 let x = 10            # bất biến
 var count = 0         # biến đổi được
 count += 1
@@ -217,9 +296,14 @@
 let {name, age} = user
 ```
 
+- `let` bất biến, `var` biến đổi được; `count += 1` là `count = count + 1`.
+- **Shadowing** (QD §7.3): được **che tên ở scope trong** — `let x = 1` ngoài, `let x = "a"` trong khối là hợp lệ, **kiểu được phép khác**. **Khai lại cùng một scope là lỗi** kiểm kiểu, kể cả khi kiểu giống hệt.
+- Tham số, biến `for`, binding của arm `match` và `if let` cũng là khai báo: chúng che tên ngoài, không khai lại trong cùng scope.
+- Gán cho `let` hoặc cho tham số là lỗi; `var self` là ngoại lệ duy nhất cho tham số (5.4).
+
 ### 4.2 Điều khiển
 
-```python
+```vis
 if x > 0: print("dương")
 elif x < 0: print("âm")
 else: print("không")
@@ -234,63 +318,119 @@
     if done(): break
 ```
 
-### 4.3 match
+- `if`/`match` dùng được như **biểu thức** khi có `else`; như câu lệnh thì `else` tuỳ chọn (Phụ lục A).
+- `..` không gồm cận trên, `..=` gồm cận trên.
 
-```python
+### 4.3 match và pattern
+
+```vis
 match s:
     Circle(r) if r > 100.0: "lớn"
-    Circle(r): "tròn {r}"
+    Circle(r) | Rect(r, _): "tròn {r}"
     Rect(w, h): "chữ nhật {w}x{h}"
     Empty: "rỗng"
 ```
 
 - Phải xét hết; thiếu nhánh là lỗi kiểm kiểu, liệt kê nhánh thiếu.
-- Pattern: literal, biến, `_`, tuple, bản ghi `{name, age: 18..}`, enum, list `[first, ..rest]`, khoảng `0..10`, `|`, guard `if`.
+- **Or-pattern `|` giữ nguyên** (QD §7.3): `Circle(r) | Rect(w, h)` trong một arm. `|` **không bao giờ** là toán tử nhị phân trong biểu thức (Q8 sửa); bit-or là hàm `bit_or`.
+- **Guard không tính vào coverage**: nhánh có `if` **không** chứng minh được phủ, nên `Circle(r) if r > 100.0:` không làm `Circle` được coi là đã xét; phải có nhánh `Circle(r)` không guard hoặc `_`.
+- **Kiểu vô hạn** (`int`, `float`, `str`, `path`, `datetime`...) **bắt buộc** có `_`; range pattern **không** dùng để chứng minh phủ kiểu vô hạn.
+- Pattern đầy đủ: literal (kể cả `-1`), biến, `_`, tuple `(a, b)`, bản ghi `{name, age: 18..}`, enum `Circle(r)` / `Shape.Circle(r)`, list `[first, ..rest]`, range `0..10` / `0..=10`, or `|`, guard `if`. Pattern cho optional/Result: **`none`**, **`some(x)`**, **`ok(v)`**, **`err(e)`**.
+- `if let PAT = EXPR:` là đường tắt của `match` một nhánh, **không** xét hết.
 
 ### 4.4 Optional
 
-```python
+```vis
 let e = user.email ?? "không có"
 let domain = user.email?.split("@").last()
 if let mail = user.email:
     send(mail)
 ```
 
+- **Tự nâng `T → T?`** ở ba vị trí: **gán** (`let x: int? = 5`), **trả về** (`-> int?` với `5`), **đối số** (`f(x)` với tham số `int?`). Chiều ngược lại **không** có: `T?` không tự thành `T`.
+- `T??` là **lỗi kiểu** (optional không lồng).
+- `?` dùng được trên `T?` khi hàm đang viết trả `U?`; trên `Result[T, E]` khi trả `Result[U, F]` với `E: Error` (6.1).
+- `?.` trên `T?` trả `T?`; `??` nhận toán hạng phải **cùng kiểu đã bỏ optional**.
+- `.get(i)`, `.first()`, `.last()`, `xs[i]?` trả `T?`; `xs[i]` panic (4.5, Q5).
+
 ### 4.5 Toán tử
 
-Số học `+ - * / % **`; so sánh `== != < <= > >=`; logic `and or not`; bit bằng hàm `bit_and`, `bit_or`, `bit_xor`, `shl`, `shr` (để `|` dành cho pipe); nối hàm `|>`: `xs |> filter(even) |> sum()`.
+Số học `+ - * / % **`; so sánh `== != < <= > >=`; logic `and or not`; bit bằng hàm `bit_and`, `bit_or`, `bit_xor`, `bit_not`, `shl`, `shr`, `ushr`; nối hàm `|>`: `xs |> filter(even) |> sum()`.
 
+**`|` không bao giờ là toán tử nhị phân trong biểu thức** (Q8 sửa, QD §7.3). Biểu thức chỉ có `|>`. `|` còn nghĩa ở đúng hai chỗ: **or-pattern** (4.3) và **pipe của chế độ lệnh** (9.3). Hệ quả: bit-or **phải** viết `bit_or(a, b)`.
+
+**Bảng ưu tiên 12 mức** [TK] (cao → thấp, đúng QD §7.3):
+
+| Mức | Nhóm | Toán tử | Kết hợp |
+|---|---|---|---|
+| 1 | postfix | `.` `?.` `()` `[]` `?` `with` | trái |
+| 2 | luỹ thừa | `**` | **phải** |
+| 3 | một ngôi | `-` (đổi dấu) | phải |
+| 4 | nhân | `* / %` | trái |
+| 5 | cộng | `+ -` | trái |
+| 6 | khoảng | `.. ..=` | không kết hợp |
+| 7 | coalesce | `??` | phải |
+| 8 | so sánh | `== != < <= > >=` | **không kết hợp** (`1 < x < 10` là lỗi cú pháp) |
+| 9 | phủ định | `not` | phải |
+| 10 | hội | `and` | trái, **ngắn mạch** |
+| 11 | tuyển | `or` | trái, **ngắn mạch** |
+| 12 | nối | `\|>` | trái |
+
+Hệ quả, phải có golden test ở **ST-02a**:
+
+- `-2 ** 2` = `-(2 ** 2)` = `-4` (một ngôi **thấp hơn** `**`).
+- `2 ** 3 ** 2` = `2 ** (3 ** 2)` = `512`.
+- `a ?? b == c` = `(a ?? b) == c`.
+- `0..n+1` = `0..(n+1)`.
+- `not a == b` = `not (a == b)`.
+- `x? |> f` = `(x?) |> f`; `u with age: 21 |> f` = `(u with age: 21) |> f`.
+- `a and b or c` = `(a and b) or c`.
+
+- `and`/`or` **ngắn mạch** ("short-circuit"); đánh giá **trái sang phải** cho toán hạng, đối số, phần tử literal và trường bản ghi (4.6).
+- `xs[i]` **panic** với thông điệp nêu chỉ số và độ dài; `xs[i]?` trả `T?` (checked, không panic); `xs[-1]` panic với thông điệp "chỉ số âm không hỗ trợ; dùng `xs.last()`".
+- **Không có `s[i]`** trên `str` (lỗi kiểu, gợi ý `.chars()/.graphemes()/.byte_at(i)?`).
+- `str * int` **không có**; dùng `s.repeat(n)` (QD §7.3).
+
+### 4.6 Thứ tự đánh giá
+
+- **Đánh giá trái sang phải** [TK], tất định: toán hạng nhị phân, phần tử trong literal list/map/set/tuple, trường trong bản ghi, đối số trong lời gọi (vị trí trước, tên sau, giữ nguyên thứ tự viết).
+- `and`/`or` ngắn mạch: toán hạng phải **không được đánh giá** khi trái quyết định.
+- `??` ngắn mạch theo cùng luật: toán hạng phải chỉ đánh giá khi trái là `none`.
+- Thứ tự này là một phần của đặc tả: bộ vi sai interpreter/VM (18.2) dùng nó làm oracle.
+
 ---
 
 ## 5. Hàm, closure, generic, trait
 
 ### 5.1 Hàm
 
-```python
+```vis
 pub fn greet(name: str, times: int = 1, --loud: bool = false) -> str:
-    let s = "Xin chào {name}" * times
+    let s = "Xin chào {name}".repeat(times)
     if loud: s.upper() else: s
 ```
 
-- Tham số vị trí, tham số mặc định, tham số có tên bắt đầu bằng `--` (chỉ truyền theo tên; thành cờ trong chế độ lệnh).
+- Tham số vị trí, tham số mặc định, tham số có tên bắt đầu bằng `--` (chỉ truyền theo tên; thành cờ trong chế độ lệnh). Gọi: `greet("An", 2, --loud: true)`.
+- **Thứ tự đối số:** vị trí trước, tên sau; tên **không trùng**; không được bỏ tham số vị trí bắt buộc; `...rest: T` (nếu có) đứng **cuối**.
 - Biểu thức cuối khối là giá trị trả về; `return` để trả sớm.
-- Tham số bất biến.
+- Tham số **bất biến**; chỉ `var self` (5.4) là ngoại lệ.
+- Không có quá tải theo kiểu; cùng tên trong cùng scope là lỗi (4.1).
 
 ### 5.2 Closure
 
 `x => x * 2`, `(a, b) => a + b`, hoặc nhiều dòng:
 
-```python
+```vis
 let f = fn(x: int) -> int:
     let y = x * 2
     y + 1
 ```
 
-Closure bắt biến theo giá trị. Muốn chia sẻ trạng thái biến đổi: `ref[T]`.
+Closure bắt biến theo giá trị. Muốn chia sẻ trạng thái biến đổi: `ref[T]`. Hai dạng closure (`x => expr` và `fn(...) -> T:` nhiều dòng) là **cùng một kiểu** `fn(A) -> B`.
 
 ### 5.3 Generic
 
-```python
+```vis
 fn first[T](xs: list[T]) -> T?:
     if xs.is_empty(): none else: xs[0]
 
@@ -299,9 +439,11 @@
 
 Chỉ định tường minh khi cần: `json.parse[list[User]](text)`.
 
+- **Generic vs index** (QD §7.3): `name[types](args)` chỉ được hiểu là **danh sách kiểu** khi ngay sau `]` là `(` **và** `name` là hàm generic đã biết; ngược lại là **index**. Nhờ vậy `xs[i]` và `f[T](x)` không mập mờ.
+
 ### 5.4 Trait
 
-```python
+```vis
 trait Show:
     fn show(self) -> str
 
@@ -317,29 +459,34 @@
 - Ràng buộc generic `[T: Show]` hoặc giá trị động `dyn Show`.
 - Không kế thừa cài đặt; trait có phương thức mặc định.
 - Quy tắc mồ côi: `impl Trait for Type` chỉ trong module định nghĩa Trait hoặc Type.
+- `trait Error: Show { message, source, span }` là trait lõi (6.1).
 
 ### 5.5 Phương thức
 
-`impl User:` thêm phương thức. Gọi `u.method()`. Không UFCS.
+`impl User:` thêm phương thức. Gọi `u.method()`. Không UFCS. Phương thức thắng hàm trần cùng tên; hàm trần phải `use` tường minh.
 
 ---
 
 ## 6. Xử lý lỗi
 
-### 6.1 Result và `?`
+### 6.1 Result, `?`, trait `Error`
 
-```python
-fn read_config(dir: cap[Dir]) -> Result[Config, Error]:
+```vis
+fn read_config(dir: cap[Dir]) -> Result[Config, AnyError]:
     let text = dir.read_text("config.json")?
     let cfg = json.parse[Config](text)?
     Ok(cfg)
 ```
 
-`?` cần lỗi nguồn chuyển được sang lỗi đích (trait `Into`). `Error` là kiểu lỗi chung có thông điệp, nguyên nhân lồng, vị trí.
+- `trait Error: Show { message, source, span }` (QD §7.3): mọi kiểu lỗi in được (`Show`) và cung cấp `message: str`, `source: Error?` (nguyên nhân lồng), `span: Span?` (vị trí nguồn).
+- `AnyError` là **kiểu xoá kiểu** (boxed `dyn` là chi tiết cài đặt): bọc mọi `E: Error`, dùng ở ranh giới (main, plugin, task).
+- **Mọi `E: Error` tự chuyển sang `AnyError` qua `?`** — không cần `impl Into`. `?` trên `Result[T, E]` trong hàm trả `Result[U, AnyError]` hợp lệ khi `E: Error`.
+- `?` trên `T?` chỉ trong hàm trả `U?` (4.4). `?` **không** tự bọc `Result` vào `Result` lồng; `T??` là lỗi kiểu.
+- `main -> Result[(), AnyError]`. Phía plugin Rust, tên của cùng trait là **`vis::Error`** (17).
 
 ### 6.2 `else` xử lý tại chỗ
 
-```python
+```vis
 let cfg = read_config(dir) else err:
     print("Dùng cấu hình mặc định: {err}")
     Config.default()
@@ -353,12 +500,31 @@
 
 ### 6.4 panic
 
-Cho vi phạm bất biến: `xs[i]` ngoài phạm vi (dùng `xs.get(i) -> T?` nếu không chắc), tràn số, `assert`. panic dừng task hiện tại, in vết ngăn xếp có vị trí nguồn. Không bắt được trong code thường; chỉ ranh giới task/shell bắt để báo cáo.
+Cho vi phạm bất biến, kể cả lỗi số học: tràn, chia 0, `%0`, mũ âm với `int`, chỉ số ngoài phạm vi, `assert`. **Payload của panic là `AnyError`** (QD §7.3): `panic(err)` nhận `E: Error`; `panic("chuỗi")` là đường tắt tạo `AnyError` với `message`. panic dừng task hiện tại, in vết ngăn xếp có vị trí nguồn. Không bắt được trong code thường; chỉ ranh giới task/shell bắt để báo cáo.
 
-### 6.5 Lỗi trong `main`
+### 6.5 Lỗi trong `main` và mã thoát
 
-`fn main(...) -> Result[(), Error]`: lỗi được in đẹp, mã thoát 1.
+`fn main(...) -> Result[(), AnyError]`: lỗi được in đẹp, mã thoát 1.
 
+| Mã | Nghĩa | Nguồn |
+|---|---|---|
+| 0 | Thành công | `main` trả `Ok`, hoặc lệnh shell xong bình thường |
+| 1 | Lỗi từ `main` | `Err` của `main` |
+| 2 | Lỗi dùng / lỗi kiểu / lỗi cú pháp | `vis` trước khi chạy (cờ sai, tệp sai) |
+| 101 | panic | task panic; ViShell đặt mã này cho lệnh bị panic |
+| 126 | Không chạy được (không phải tệp thực thi, thiếu quyền) | host |
+| 127 | Không tìm thấy lệnh | host |
+| 130 | Bị huỷ (Ctrl-C, `Cancel`) | tín hiệu, job control |
+
+Bảng này đúng QD §7.3 và khớp quy ước POSIX cho 126/127/130; `| complete` trả `code`/`how` theo cùng phân loại (9.5).
+
+### 6.6 Ranh giới lệnh trong ViShell
+
+- ViShell bọc **mỗi lệnh** trong **một task có heap arena riêng** (QD §7.3). Ranh giới lỗi là **lệnh**, không phải tiến trình và không phải phiên.
+- panic giải phóng arena của lệnh, in trace + span, đặt mã **101**, và **giữ** biến phiên, cwd, job, lịch sử.
+- Tài nguyên mà lệnh mở (tệp, tiến trình con, kênh) là **cap của phiên**, đăng ký vào arena của lệnh và **tự đóng** khi arena giải phóng.
+- **Chưa thêm `defer`** ở bản này (QD §7.3); nếu sau này cần là thay đổi ngữ nghĩa phải qua Agent 0 (VD-08).
+
 ---
 
 ## 7. Mô hình giá trị và bộ nhớ
@@ -367,7 +533,7 @@
 
 Mọi kiểu (trừ `ref`, `cap`, `stream`, kênh) có ngữ nghĩa giá trị:
 
-```python
+```vis
 var a = [1, 2, 3]
 var b = a
 b.push(4)        # a vẫn là [1, 2, 3]
@@ -381,7 +547,7 @@
 
 ### 7.3 `ref[T]`
 
-```python
+```vis
 let counter = ref(0)
 let inc = () => counter.set(counter.get() + 1)
 ```
@@ -398,71 +564,100 @@
 
 - Mỗi tệp là một module. `use std.fs`, `use ./util`, `use pkg.http`.
 - `pub` để xuất; mặc định riêng tư.
+- **Không có `as`** (đã bỏ khỏi từ khoá, 2.1): muốn tên ngắn thì `use` đúng mục cụ thể.
 - Cấm vòng phụ thuộc giữa module.
 - Gói: thư mục có `vis.toml` (tên, phiên bản, giấy phép, phụ thuộc theo đường dẫn, git URL + commit, hoặc registry sau này), `vis.lock` khoá phiên bản + hash nội dung.
 - Script một tệp khai phụ thuộc ở đầu tệp:
 
-```python
+```vis
 #!/usr/bin/env vis
 #| deps: http = "git+https://example.org/vis-http@a1b2c3"
 ```
 
 - Gói không có mã chạy lúc cài đặt.
+- Hash của **closure phụ thuộc** (tệp chính + mọi phụ thuộc + phiên bản compiler) là khoá của cache (14.5) **và** của consent quyền (11.3).
 
 ---
 
 ## 9. Chế độ lệnh (shell)
 
-### 9.1 Ý tưởng
+### 9.1 Ý tưởng và mô hình **hai pha**
 
-Chế độ lệnh là **cú pháp thứ hai**, được parser dịch sang **cùng cây cú pháp** với chế độ script. Dùng ở:
+Chế độ lệnh là **cú pháp thứ hai**, và được xử lý thành **hai pha** (QD §1 **Q-D**):
+
+1. **`vis-syntax` → `CmdCST`**: lex + parser chế độ lệnh sinh CST **chưa định kiểu**; đơn vị cơ bản là `Word` giữ **nguyên văn** từ trần (để báo lỗi và để `vish-edit` tô màu). Ở pha này chưa biết `5mb` là `size` hay `str`, chưa biết `size` là trường hay tên lệnh.
+2. **`cmd_lower` (trong `vis-types`) → HIR**: dùng **bảng chữ ký** để quyết định: literal theo kiểu tham số, glob theo kiểu tham số, `it` + viết tắt trường, `tên cmd` vs chương trình ngoài, binding `--flag`, `--help`.
+
+Hệ quả thuật ngữ: hai chế độ ra **cùng HIR**, không phải "cùng AST" (sửa câu cũ ở 1.1 và 18.2). Dùng ở:
 - Mỗi dòng gõ trong ViShell.
 - Khối `cmd { ... }` trong tệp `.vis`.
 - Tệp `.vish`.
 
+Bảng "luật → pha":
+
+| Luật | Pha |
+|---|---|
+| tách `word`, `"..."`, `'...'`, `$x`, `$env.NAME`, `--flag`, `--`, `;`, `&`, `\|`, `^` | `vis-syntax` (`CmdCST`) |
+| `CmdCST` + vị trí nguồn + golden test từng hàng bảng 9.4 | `vis-syntax` |
+| `5mb` → `size` theo kiểu tham số | `cmd_lower` |
+| `*.txt` → glob theo kiểu tham số (`list[path]`/`glob`) | `cmd_lower` |
+| `where size > 5mb` → `where it => it.size > 5mb` | `cmd_lower` |
+| `ls` là `cmd` nội bộ, biến hàm, hay chương trình ngoài | `cmd_lower` |
+| `--flag` binding vào tham số, sinh `--help` | `cmd_lower` |
+| `-5` là số hay cờ (theo kiểu tham số) | `cmd_lower` |
+
 ### 9.2 Khai báo lệnh
 
-```python
+```vis
 ## Liệt kê tệp
-pub cmd ls(path: path = ".", --all: bool = false, --long: bool = false) -> stream[FileEntry]:
+pub cmd ls(path: path = p".", --all: bool = false, --long: bool = false) -> stream[FileEntry]:
     ...
 ```
 
 - `cmd` là hàm bình thường, cộng thêm: tự sinh `--help`, gọi được bằng cú pháp lệnh, tham số `--x` thành cờ, `bool` thành cờ không giá trị.
 - Lệnh nhận `ctx: Ctx` ngầm (thư mục hiện tại, môi trường, terminal, quyền của phiên). Hàm thường không có `ctx`.
+- Tham số mặc định kiểu `path` viết `p"."` (3.5), không viết `"."`.
 
 ### 9.3 Phân tích một dòng lệnh
 
 ```
 pipeline   := stage ('|' stage)*  ['&']
 stage      := command arg*   |   '(' expr ')'
-command    := tên cmd | biến hàm | chương trình ngoài | '^' tên (ép gọi ngoài)
-arg        := word | '--flag' [value] | '$' ident ('.' ident)* | '(' expr ')' | string | closure-arg
+command    := ten_cmd | bien_ham | chuong_trinh_ngoai | '^' ten_cmd
+arg        := word | '--flag' [value] | '$' ident ('.' ident)* | '$env' '.' ident | '(' expr ')' | string | closure_arg
+shell_item := use | let | fn | cmd | pipeline          // .vish và REPL
 ```
 
-- **Tham số được hiểu theo chữ ký lệnh.** Từ trần `./photos` vào tham số `path` thành `path`; `5mb` vào `size` thành `size`; `*.txt` vào tham số `list[path]`/`glob` được mở rộng; vào `str` thành chuỗi. Không có tách từ và trích dẫn kiểu Bash.
-- Chương trình ngoài: mọi tham số là `str`, glob mở rộng trước khi truyền.
-- Biểu thức: `$x`, `$x.field`, `(expr)`.
-- Tham số kiểu hàm (`where`, `each`, `sort_by`...) được phân tích ở **chế độ biểu thức**, có biến ngầm `it` và **viết tắt trường**: `where size > 5mb` nghĩa là `where it => it.size > 5mb`.
+- **Metachar**: khoảng trắng, `( ) [ ] { } ; & $ ' " |`; `#` khi đứng đầu từ. Từ trần là chuỗi liên tục không chứa metachar. `"..."` có nội suy; `'...'` thô; `\` chỉ trong chuỗi.
+- `$x` biến phiên; `$env.NAME` biến môi trường. `;` tách pipeline; `&` nền chỉ khi là token cuối và có khoảng trắng trước; `--` kết thúc cờ; `-5` là số nếu tham số kiểu số, ngược lại là cờ.
+- **Tham số hiểu theo chữ ký lệnh — pha `cmd_lower` (QD §1 Q-D)**: `vis-syntax` sinh `CmdCST` với từ trần **chưa định kiểu**; `cmd_lower` trong `vis-types` dùng bảng chữ ký quyết `./photos`→`path`, `5mb`→`size`, glob theo kiểu, `it` + viết tắt trường, `--flag` binding. Không hard-code chữ ký trong parser.
+- Chương trình ngoài: mọi tham số là `str`, glob mở rộng trước khi truyền; `^ten` ép gọi ngoài.
+- Closure-arg kéo tới `|` kế tiếp ở **độ sâu ngoặc 0**; or-pattern trong một stage phải đặt trong ngoặc.
+- `from json` không tham số trả `Json`; `where` trên `Json` là lỗi kiểu, gợi ý `from json[T]`.
 
 ### 9.4 Xung đột ký hiệu và cách giải
 
 | Ký hiệu | Chế độ lệnh | Chế độ script |
 |---|---|---|
-| `\|` | Pipe | Không dùng (bit OR là `bit_or`) |
-| `>` | So sánh (trong tham số biểu thức) | So sánh |
-| Ghi tệp | `\| save out.txt`, `\| save --append log.txt` | `file.write(...)` |
-| `*` | Glob trong từ trần | Nhân |
-| `-x`, `--x` | Cờ | Trừ |
-
-Không dùng `>` để chuyển hướng nên không bao giờ mập mờ.
+| `\|` | pipe ở độ sâu 0 | or-pattern trong pattern; **không** là toán tử nhị phân |
+| `\|>` | — | pipe biểu thức |
+| `>` `<` | so sánh trong tham số biểu thức; không redirect | so sánh |
+| `#` | chú thích khi đứng đầu từ; `https://x#y` giữ nguyên | chú thích |
+| `%` | modulo; jobspec dùng `job 1` | modulo |
+| `*` `**` | glob (một/đệ quy) | nhân/luỹ thừa |
+| `-x --x --` | cờ; `--` kết thúc cờ; `-5` là số nếu kiểu số | trừ |
+| `$` | `$x`, `$env.NAME` | không dùng |
+| `[` `(` | biểu thức/list literal phải trong `( )`; glob char-class | index/gọi |
+| `;` `&` | tách pipeline / nền | không dùng |
+| `^ten` | ép gọi chương trình ngoài | không dùng |
+| Ghi tệp | `\| save out.txt` | `file.write(...)` |
 
 ### 9.5 Luồng dữ liệu
 
-- `stream[T]` lười, kéo, có backpressure, huỷ khi bị bỏ. `take 10` dừng nguồn sớm.
-- Chương trình ngoài: stdout là `stream[bytes]`; chuyển đổi bằng `| lines`, `| from json`, `| from csv`, `| from toml`; ngược lại `| to json`.
-- Mã thoát khác 0 thành lỗi pipeline; `| complete` để nhận `{stdout, stderr, code}`.
-- Khai chữ ký cho chương trình ngoài để có gợi ý và kiểm tham số:
+- `stream[T]` lười, kéo, có backpressure, huỷ khi bị bỏ. `take 10` dừng nguồn sớm; SIGPIPE/early-close do `take` **không phải lỗi** pipeline.
+- Chương trình ngoài: stdout là `stream[bytes]`; chuyển đổi bằng `| lines`, `| from json[T]`, `| from csv`, `| from toml`; ngược lại `| to json`.
+- Mã thoát khác 0 thành lỗi pipeline; `| complete` **chặn** lỗi pipeline và trả `{stdout, stderr, code, how}`.
+- Khai chữ ký cho chương trình ngoài:
 
 ```python
 extern cmd git(sub: GitSub, --oneline: bool, ...rest: str)
@@ -474,20 +669,20 @@
 
 ### 9.7 Ví dụ
 
-```python
-> ls ./photos | where size > 5mb | sort_by modified --desc | take 10
-> let big = ls ./photos | where size > 5mb
-> big | each f => f.copy_to(./backup)
-> open users.json | where age >= 18 | get name
-> git log --oneline | lines | take 5
-> ps | where cpu > 50% | each p => p.kill()
-> http get https://api.example.com/items | from json | to csv | save items.csv
+```vish-session
+ls ./photos | where size > 5mb | sort_by modified --desc | take 10
+let big = ls ./photos | where size > 5mb
+big | each f => f.copy_to(p"./backup")
+open users.json | from json[User] | where age >= 18 | get name
+git log --oneline | lines | take 5
+ps | where cpu > 50pct | each p => p.kill()
+http get https://api.example.com/items | from json[Item] | to csv | save items.csv
 ```
 
 Trong script:
 
-```python
-fn backup_big(ctx: Ctx, dir: path) -> Result[int, Error]:
+```vis
+fn backup_big(ctx: Ctx, dir: path) -> Result[int, AnyError]:
     let files = cmd { ls (dir) | where size > 5mb } |> collect()
     for f in files:
         f.copy_to(p"./backup")?
@@ -500,15 +695,18 @@
 
 ### 10.1 async có cấu trúc
 
-```python
-async fn fetch_all(net: cap[Net], urls: list[str]) -> list[Result[str, Error]]:
+```vis
+async fn fetch_all(net: cap[Net], urls: list[str]) -> list[Result[str, AnyError]]:
     scope s:
         let tasks = urls.map(u => s.spawn(net.get_text(u)))
         tasks.map(t => await t)
 ```
 
-- `async fn`, `await`. Mọi task con nằm trong `scope`; ra khỏi `scope` thì task con đã xong hoặc bị huỷ. Không task mồ côi.
+- **Kiểu:** `Future[T]` là **kết quả lười**; `Task[T]` là future **đã spawn** (có handle). Gọi một `async fn` **chỉ tạo `Future[T]`, không chạy** (QD §7.3); việc chạy bắt đầu ở `await` hoặc ở `s.spawn`.
+- `s.spawn(fut: Future[T]) -> Task[T]` (QD §7.3); `await t: Task[T] -> T`; `await f: Future[T]` chạy nó.
+- Mọi task con nằm trong `scope`; ra khỏi `scope` thì task con đã xong hoặc bị huỷ. Không task mồ côi.
 - Huỷ (Ctrl-C, timeout) lan theo cây `scope`, điểm huỷ tại mỗi `await`.
+- **Panic trong scope** (QD §7.3): **huỷ các task anh em** trong scope trước, rồi **lan** lên scope cha. Đây là ngữ nghĩa bắt buộc, có test ở 18.2.
 - `timeout(5s, fut)`, `select` trên nhiều future/kênh.
 
 ### 10.2 Kênh
@@ -532,21 +730,22 @@
 - Không hàm toàn cục nào chạm hệ thống. Không có `fs.read_text("x")`; phải là `dir.read_text("x")` với `dir: cap[Dir]`.
 - Capability chỉ có ba nguồn: tham số `main`, `ctx` của phiên shell, hoặc dẫn xuất hẹp hơn từ capability đang có (`dir.subdir("logs")`, `net.restrict("api.example.com")`).
 - Hệ kiểu bảo đảm thư viện chỉ có quyền được truyền vào.
+- `print` là stdio của **chính tiến trình**, không phải capability; `sh "..."` cần `cap[Proc]` (QD §7.3).
 
 ### 11.2 Khai báo trong `main`
 
-```python
-fn main(data: cap[Dir] @ "./data", out: cap[Dir] @ "./out" (write), net: cap[Net] @ "api.example.com") -> Result[(), Error]:
+```vis
+fn main(data: cap[Dir] @ p"./data", out: cap[Dir] @ p"./out" (write), net: cap[Net] @ p"api.example.com") -> Result[(), AnyError]:
     ...
 ```
 
-- `@` khai đường dẫn/host mặc định yêu cầu; ghi đè bằng `vis run app.vis --grant data=./other`.
+- `@` khai đường dẫn/host mặc định yêu cầu; là literal `path` (`p"..."`) như mọi chỗ khác (3.5). Ghi đè bằng `vis run app.vis --grant data=./other`.
 - `vis run --dry` in danh sách quyền chương trình yêu cầu.
 
 ### 11.3 Shell
 
 - Phiên ViShell có quyền của người dùng (`ctx`).
-- Chạy một tệp `.vis` từ shell: chỉ trao quyền nó khai trong `main`; shell hỏi khi quyền lần đầu được dùng (nhớ theo hash tệp).
+- Chạy một tệp `.vis` từ shell: chỉ trao quyền nó khai trong `main`; shell hỏi khi quyền lần đầu được dùng và **nhớ consent theo hash closure** (QD §7.3): hash = **nguồn tệp chính + toàn bộ phụ thuộc (`use`/`deps`) + phiên bản compiler**, dùng **chung hash cache** với 14.5. Nhờ vậy sửa `./util.vis` là consent cũ **mất hiệu lực** — vá lỗ hổng "theo hash tệp chính, bỏ qua phụ thuộc".
 - `cmd` trong cấu hình shell chạy với `ctx` của phiên.
 
 ### 11.4 Mức bảo đảm theo nền tảng
@@ -559,7 +758,7 @@
 | Windows | Runtime kiểm; AppContainer sau |
 | Wasm | Môi trường trình duyệt |
 
-Ở chế độ hạn chế quyền, plugin native bị tắt.
+Ở chế độ hạn chế quyền: **plugin động bị tắt**, **plugin tĩnh chỉ có trong bản `vis` người dùng tự build** (17).
 
 ---
 
@@ -574,9 +773,13 @@
     fn file_write(&self, f: &Handle, buf: &[u8], off: Option<u64>) -> HResult<usize>;
     fn stat(&self, base: &DirHandle, rel: &VPath) -> HResult<Meta>;
     // rename, remove, mkdir, watch ...
-    // tiến trình
-    fn spawn(&self, spec: &ProcSpec) -> HResult<ProcHandle>;     // argv, env, cwd handle, stdio
-    // wait, kill, list_procs ...
+    // tiến trình: stdio là ỐNG (pipe); không có pty ở bản này
+    fn spawn(&self, spec: &ProcSpec) -> HResult<ProcHandle>;     // argv, env, cwd, pipe stdio
+    // list_procs ...; pty/tty server: chưa có ở bản đầu, xem §15
+    fn proc_wait(&self, p: &ProcHandle) -> HResult<ProcStatus>;   // { code: i32, how: Exited|Cancelled|Faulted|Killed }
+    fn proc_suspend(&self, p: &ProcHandle) -> HResult<()>;
+    fn proc_resume(&self, p: &ProcHandle) -> HResult<()>;
+    fn proc_cancel(&self, p: &ProcHandle) -> HResult<()>;
     // mạng: tcp_connect, tcp_listen, udp_bind, dns_resolve ...
     // thời gian
     fn now_mono(&self) -> Instant;
@@ -587,8 +790,12 @@
 }
 ```
 
+- **Tiến trình** (QD §3.1): `spawn` dựng stdio bằng **ống/pipe**, không cấp `cap[Term]` cho con; `proc_wait` trả **`{code, how}`** với `how` dùng chung với `| complete` (9.5) và job control (15); `proc_suspend/resume/cancel` là API bắt buộc của backend.
+- **pty chưa có** ở bản này; pty + controlling terminal để **tty server** sau (15.1). Vì vậy chương trình toàn màn hình chưa hứa chạy.
+- **Trên ViFastOs**, các thao tác tiến trình ở trên là **RPC tới `procsrv`**, không phải syscall: kernel chỉ có `Exit` (syscall 14) và `Tcb.Suspend/Resume`. Đừng đi tìm syscall `wait`.
 - Đường dẫn trong lõi là `VPath` (luôn `/`, UTF-8; tên không UTF-8 giữ dạng bytes thoát). Backend chuyển sang đường dẫn gốc.
-- Backend: `vis-host-posix` (Linux, macOS, BSD), `vis-host-windows`, `vis-host-wasm`, `vis-host-vifast`, `vis-host-mock` (test, tất định).
+- Backend: `vis-host-posix` (Linux, macOS, BSD), `vis-host-windows`, `vis-host-wasm`, `vis-host-vifast` (workspace ngoài `viscript/`), `vis-host-mock` (test, tất định).
+- **Shell sở hữu tty**; tiến trình con nhận pipe. Chương trình toàn màn hình (vim/less) để sau với tty server riêng (QD §3.1).
 - **Bộ hợp chuẩn Host** chạy giống nhau trên mọi backend (mục 18).
 
 ---
@@ -597,19 +804,19 @@
 
 | Module | Nội dung |
 |---|---|
-| `core` | kiểu dựng sẵn, helper Option/Result, trait chuẩn |
+| `core` | kiểu dựng sẵn, helper Option/Result, trait chuẩn (`Show`, `Error`, `Render`), `Future`/`Task` |
 | `text` | chuỗi, grapheme, chuẩn hoá Unicode, regex, định dạng, so sánh theo locale cơ bản |
 | `coll` | list, map, set, deque, heap, sắp xếp ổn định |
 | `stream` | map, filter, take, skip, chunk, zip, group_by, sort_by, uniq, sum, count, collect |
 | `fs` | Dir, File, đọc/ghi, duyệt, watch, glob |
 | `path` | xử lý `path` |
-| `proc` | chạy chương trình, pipe, mã thoát |
+| `proc` | chạy chương trình, pipe, suspend/resume/cancel, mã thoát + `how` |
 | `net` | TCP, UDP, DNS, `http` client, TLS (thuần Rust) |
-| `time` | instant, datetime, múi giờ, duration |
+| `time` | `now()`, instant, datetime, múi giờ, duration |
 | `fmt` | json, toml, csv, base64, hex (yaml là thư viện ngoài) |
 | `crypto` | hash (SHA-2, BLAKE3), random an toàn |
 | `term` | màu, bảng, prompt, tiến độ |
-| `test` | `test fn`, assert, snapshot |
+| `testing` | `test fn`, assert, snapshot (đổi tên từ `test`, QD §7.3) |
 | `sync` | Mutex, kênh, Once |
 | `shell` | `cmd` chuẩn: ls, cd, open, save, where, each, sort_by, get, select, take, lines, from, to, ps, kill, http, walk, ... |
 | `vifast` | chỉ có trên ViFastOs: binding sinh từ IDL |
@@ -623,30 +830,34 @@
 ### 14.1 Đường xử lý
 
 ```
-nguồn (.vis / dòng lệnh)
-  └─ lexer (2 chế độ, trạng thái thụt lề)
-       └─ parser (đệ quy xuống, phục hồi lỗi) ──> CST không mất thông tin (cho fmt, LSP)
-            └─ hạ xuống AST
-                 └─ phân giải tên + module
-                      └─ kiểm kiểu (hai chiều, ràng buộc trait, xét hết match)
-                           └─ HIR có kiểu
-                                ├─ interpreter cây (mô hình tham chiếu)
-                                └─ sinh bytecode ──> xác minh bytecode ──> VM
-                                       └─ cache theo hash
+nguồn (.vis / .vish / dòng lệnh)
+  └─ chuẩn hoá NFC (2.1)
+       └─ lexer (2 chế độ, trạng thái thụt lề, VS0107)
+            └─ parser (đệ quy xuống, phục hồi lỗi) ──> CST không mất thông tin (cho fmt, LSP)
+                 ├─ (script) hạ xuống AST
+                 └─ (lệnh)  CmdCST: `Word` chưa định kiểu
+                      └─ phân giải tên + module
+                           └─ kiểm kiểu + `cmd_lower` (bảng chữ ký)
+                                └─ HIR có kiểu
+                                     ├─ interpreter cây (mô hình tham chiếu)
+                                     └─ sinh bytecode ──> xác minh bytecode ──> VM
+                                            └─ cache theo hash closure
 ```
 
+`cmd_lower` nằm trong `vis-types` (không phải trong parser) để giữ DAG crate: parser không cần bảng chữ ký, còn `vis-types` đã có kiểu.
+
 ### 14.2 Crate
 
 | Crate | Vai trò | Phụ thuộc |
 |---|---|---|
-| `vis-syntax` | lexer, parser, CST, AST, vị trí nguồn | |
+| `vis-syntax` | lexer, parser, CST, AST, `CmdCST`, vị trí nguồn | |
 | `vis-diag` | chẩn đoán, mã lỗi, bản dịch vi/en/ja | |
-| `vis-types` | phân giải tên, kiểm kiểu, HIR | syntax, diag |
+| `vis-types` | phân giải tên, kiểm kiểu, `cmd_lower`, HIR | syntax, diag |
 | `vis-interp` | interpreter cây trên HIR (mô hình tham chiếu, đơn giản) | types |
 | `vis-bytecode` | định dạng bytecode, sinh mã, xác minh | types |
 | `vis-vm` | VM, heap RC/CoW, scheduler task, FFI | bytecode |
 | `vis-host` | trait Host + `vis-host-mock` | |
-| `vis-host-{posix,windows,wasm,vifast}` | backend | host |
+| `vis-host-{posix,windows,wasm}` | backend | host |
 | `vis-std` | thư viện chuẩn (.vis + Rust) | vm, host |
 | `vis-fmt` | định dạng mã từ CST | syntax |
 | `vis-lsp` | language server | types, fmt |
@@ -655,16 +866,20 @@
 | `vishell` | shell | cli, edit |
 | `vis-plugin` | macro + ABI plugin Rust | vm |
 
-Luật: `vis-syntax`, `vis-types`, `vis-bytecode`, `vis-vm` không phụ thuộc hệ điều hành (cổng build target wasm32). Tệp không quá 1.500 dòng. `unsafe` chỉ trong `vis-vm` (heap) và `vis-plugin`, có Miri. Mọi tệp có header SPDX `MIT OR Apache-2.0`.
+- **`vis-host-vifast` KHÔNG nằm trong `viscript/`** (QD §7.5 ST-01): nó ở **workspace ngoài** vì phụ thuộc IDL của ViFastOs; `viscript/` chỉ chứa crate thuần (lõi + 3 backend còn lại + mock). Cổng ST-01 kiểm điều này bằng case âm.
+- **SPDX theo loại tệp** (QD §9.3): `//` cho Rust/asm, `#` cho `.vis .vish .vidl .toml`; tệp không có chú thích dùng sidecar `.license`; tệp sinh do generator phát header.
+- **Giấy phép (D2-21)**: MIT OR Apache-2.0; phụ thuộc qua `cargo-deny` với allowlist MIT, Apache-2.0, BSD-2/3-Clause, ISC, 0BSD, Zlib, Unicode-3.0, Unicode-DFS-2016; cấm MPL/GPL/LGPL; `THIRD-PARTY.md` sinh tự động; LICENSE-MIT/APACHE một bản ở gốc repo; **không** tạo NOTICE.
+- Luật: `vis-syntax`, `vis-types`, `vis-bytecode`, `vis-vm` không phụ thuộc hệ điều hành (cổng build target wasm32). **Tệp không quá 1.500 dòng** [TK]. `unsafe` chỉ trong **danh sách crate đóng** (`vis-vm` cho heap, `vis-plugin` cho ABI), có Miri; **mỗi luật có case âm** chứng minh cổng bắt được vi phạm.
 
 ### 14.3 Kiểm kiểu
 
 - Phân giải tên hai lượt (khai báo trước, thân sau).
 - Suy kiểu hai chiều, biến kiểu trong phạm vi một hàm; hợp nhất cục bộ.
 - Trait: tra cài đặt theo kiểu cụ thể; generic kiểm một lần với ràng buộc.
-- Xét hết `match` bằng thuật toán ma trận pattern.
+- Xét hết `match` bằng thuật toán ma trận pattern; **guard không tính coverage**, **kiểu vô hạn bắt buộc `_`** (4.3).
 - Phục hồi lỗi: kiểu `{error}` lan truyền im lặng, chỉ báo lỗi gốc.
 - Kiểm tăng dần cho LSP và shell: cache theo module.
+- Sau khi có chữ ký, `cmd_lower` kiểm literal/glob/`it` của chế độ lệnh rồi mới sang HIR (9.1).
 
 ### 14.4 VM
 
@@ -679,7 +894,7 @@
 
 ### 14.5 Cache
 
-`<thư mục cache>/vis/<hash>.vbc`, hash = nguồn + phiên bản trình biên dịch + hash phụ thuộc. Mục tiêu khởi động script đã cache dưới 20 ms [ƯL].
+`<thư mục cache>/vis/<hash>.vbc`, hash = **hash closure**: nguồn tệp chính + toàn bộ phụ thuộc + phiên bản trình biên dịch (cùng hash với consent ở 11.3). Mục tiêu khởi động script đã cache dưới 20 ms [ƯL].
 
 ### 14.6 Chẩn đoán
 
@@ -691,7 +906,7 @@
   = gợi ý: dùng `int.parse(user.email ?? "0")?` nếu email chứa số
 ```
 
-Mã lỗi ổn định, `vis explain VS0412`. Ngôn ngữ thông báo theo cấu hình (mặc định theo locale hệ thống).
+Mã lỗi ổn định (`VS` + 4 chữ số, quy tắc cấp ở `ST-02b`), `vis explain VS0412`. Cột theo Unicode scalar (2.4). Ngôn ngữ thông báo theo cấu hình (mặc định theo locale hệ thống).
 
 ---
 
@@ -705,7 +920,7 @@
 │  phiên: biến, ctx (cwd, env, quyền), job, lịch sử            │
 │  cấu hình: ~/.config/vishell/config.vis (prompt là hàm)      │
 ├────────────────────────────────────────────────────────────┤
-│ vis-types (kiểm tăng dần) │ vis-vm │ vis-std.shell          │
+│ vis-types (kiểm tăng dần + cmd_lower) │ vis-vm │ vis-std.shell │
 ├────────────────────────────────────────────────────────────┤
 │ Terminal trait: ANSI (POSIX) │ Windows console │ ViTerm      │
 └────────────────────────────────────────────────────────────┘
@@ -713,18 +928,26 @@
 
 | Thành phần | Thiết kế |
 |---|---|
-| REPL | đọc dòng, phân tích chế độ lệnh, kiểm kiểu, chạy, hiển thị; lỗi kiểu thì không chạy gì |
+| REPL | đọc dòng, phân tích chế độ lệnh, kiểm kiểu, chạy, hiển thị; lỗi kiểu thì không chạy gì; `DEDENT` ngầm ở cuối mỗi input (2.2) |
 | Biến phiên | `let` trong shell sống hết phiên |
 | Gợi ý | theo kiểu tham số: `path` gợi ý tệp, `enum` gợi ý biến thể, trường bản ghi trong `where`, cờ của `cmd`, lệnh ngoài theo `extern cmd` |
 | Tô màu | theo CST + kết quả phân giải (lệnh không tồn tại tô đỏ) |
 | Lịch sử | append-only + chỉ mục; lưu dòng, cwd, thời điểm, thời lượng, mã thoát; tìm mờ |
-| Job | `&`, `jobs`, `fg`, `bg`, `kill`; POSIX dùng process group; ViFastOs qua process server |
+| Job | `&`, `jobs`, `fg`, `bg`, `kill` — POSIX dùng process group; ViFastOs qua **RPC procsrv** (15.1) |
 | Huỷ | Ctrl-C huỷ pipeline qua token huỷ lan theo stream và scope |
-| Tương thích | `^cmd` ép gọi chương trình ngoài; `sh "..."` chạy chuỗi qua `/bin/sh` khi thật cần |
+| Tương thích | `^cmd` ép gọi chương trình ngoài; `sh "..."` chạy chuỗi qua `/bin/sh` khi thật cần, cần `cap[Proc]` |
 | Prompt | hàm ViScript, có giới hạn thời gian |
 | Khởi động | cấu hình cache bytecode; mục tiêu dưới 50 ms [ƯL] |
 | Terminal | ViShell không tự là terminal; ViTerm nhận giao thức hiển thị có cấu trúc tuỳ chọn |
 
+### 15.1 Job control và terminal
+
+- **Shell sở hữu tty** (QD §3.1, §7.3, §15): ViShell giữ `cap[Term]`; tiến trình con nhận **pipe**, **không** nhận `cap[Term]`.
+- Job control `&`, `jobs`, `fg`, `bg`, `kill` là lệnh của shell, thực hiện qua **RPC `procsrv`** (trên ViFastOs) hoặc process group (POSIX). RPC procsrv: `Spawn`, `Wait(pid|gid) -> {code, how}`, `Stop(gid)`, `Cont(gid)`, `Cancel(gid)`, `List`.
+- **Ctrl-C huỷ pipeline**: huỷ từ **stage cuối về đầu**, chờ tối đa `T` [ƯL 2 s], rồi finalise; mã thoát **130**.
+- **Chương trình toàn màn hình** (vim, less) **để sau**: cần **tty server** riêng cấp pty + controlling terminal. Bản này không hứa; `fg` chỉ hứa với chương trình dùng pipe.
+- Ranh giới panic/arena của lệnh: xem 6.6.
+
 ---
 
 ## 16. Công cụ
@@ -733,7 +956,7 @@
 |---|---|
 | `vis run f.vis [args]` | kiểm kiểu + chạy |
 | `vis check` | chỉ kiểm |
-| `vis fmt` | định dạng (một kiểu duy nhất) |
+| `vis fmt` | định dạng (một kiểu duy nhất: 4 dấu cách, sửa tab) |
 | `vis test` | chạy `test fn`, snapshot |
 | `vis doc` | sinh tài liệu từ `##` |
 | `vis explain VSxxxx` | giải thích mã lỗi |
@@ -749,12 +972,13 @@
 #[vis::module(name = "image")]
 mod image {
     #[vis::func]
-    pub fn resize(data: Bytes, w: u32, h: u32) -> Result<Bytes, VisError> { ... }
+    pub fn resize(data: Bytes, w: u32, h: u32) -> Result<Bytes, vis::Error> { ... }
 }
 ```
 
-- Macro sinh chữ ký ViScript từ chữ ký Rust, kiểm lúc build.
-- Nạp: liên kết tĩnh vào bản `vis` tuỳ biến (ưu tiên, dùng trên ViFastOs) hoặc thư viện động với ABI ổn định có số phiên bản.
+- Macro sinh chữ ký ViScript từ chữ ký Rust, kiểm lúc build. Lỗi plugin dùng **`vis::Error`** — tên phía Rust của `trait Error` (6.1).
+- Nạp: **liên kết tĩnh** vào bản `vis` tuỳ biến, hoặc **thư viện động** với ABI ổn định có số phiên bản.
+- **Chế độ hạn chế quyền** (QD §7.3): **plugin động bị tắt**; **plugin tĩnh chỉ có** trong bản `vis` do **người dùng tự build** — bản phát hành mặc định không kèm plugin tĩnh nào.
 - Plugin không nhận quyền ngầm; muốn chạm tệp phải nhận `cap[...]`.
 
 ---
@@ -775,21 +999,24 @@
 
 | Phần | Cách kiểm |
 |---|---|
-| Lexer | test bảng token; fuzz không panic; thụt lề, tab bị cấm, NFC |
-| Parser | **vòng tròn**: parse → fmt → parse cho AST bằng nhau; in lại CST đúng văn bản gốc; fuzz không panic; phục hồi lỗi có số lỗi hợp lý |
-| Chế độ lệnh | bảng dòng lệnh → AST mong đợi; cùng chương trình viết hai chế độ ra cùng HIR |
+| Lexer | bảng token vàng (60 dòng ở `SPEC-lex.md`); fuzz không panic; **thụt lề suy đơn vị**, **tab = `VS0107` nhưng vẫn lex**, **NFC** (`'ố'` NFD thành một literal hợp lệ) |
+| Parser | **vòng tròn**: parse → fmt → parse cho HIR bằng nhau; in lại CST **đúng văn bản gốc** (lossless); fuzz không panic; mỗi tệp lỗi trong `tests/parse-errors` cho **đúng một** lỗi đầu tại dòng ghi trong fixture |
+| CST | **không mất thông tin**: giữ độ rộng thụt lề gốc, chú thích, dấu phẩy cuối, thứ tự trường; `fmt` idempotent |
+| Chế độ lệnh | `bảng dòng lệnh → CmdCST` vàng cho **từng hàng bảng 9.4** + 10 ca mơ hồ (ST-06); "cùng HIR" chỉ kiểm sau khi có `cmd_lower` (vòng 2) |
 | Kiểm kiểu | tệp `.vis` có chú thích kỳ vọng `# error: VS0412` hoặc `# ok`; mỗi mã lỗi có ca dương và ca âm; snapshot thông báo lỗi |
-| Xét hết match | so với bộ liệt kê vét cạn trên enum nhỏ sinh ngẫu nhiên |
+| Xét hết match | so với bộ liệt kê vét cạn trên enum nhỏ sinh ngẫu nhiên; **guard không tính**, **kiểu vô hạn cần `_`** |
+| Optional | `T → T?` tự nâng; `T??` lỗi; `?` trên `T?`; `if let`; `.get/.last` khi rỗng; `xs[i]?` |
+| Số | `%` dấu theo số bị chia; `%0` panic; `**` phải-kết-hợp; mũ âm panic; bit count ≥ 64 panic; NaN `==` false |
 | Interpreter | mô hình tham chiếu, giữ đơn giản |
 | Bytecode + VM | **vi sai**: bộ sinh chương trình đúng kiểu theo seed, chạy trên interpreter và VM, so đầu ra, mã thoát, lỗi; tự thu nhỏ ca lệch |
 | Trình xác minh | đột biến bytecode hợp lệ phải bị từ chối hoặc vẫn an toàn |
 | Heap RC/CoW | Miri; test rò rỉ (đối tượng sống về 0); test ngữ nghĩa giá trị |
-| async/scope | lập lịch tất định theo seed trong `vis-host-mock`; không task mồ côi; huỷ lan đúng |
-| Host | **bộ hợp chuẩn**: cùng tệp test chạy trên mọi backend; khác biệt hợp lệ khai báo rõ |
-| Quyền | test âm: `..`, symlink ra ngoài, dẫn xuất rộng hơn đều phải bị chặn |
+| async/scope | lập lịch tất định theo seed trong `vis-host-mock`; không task mồ côi; huỷ lan đúng; **panic trong scope huỷ anh em rồi lan** |
+| Host | **bộ hợp chuẩn**: cùng tệp test chạy trên mọi backend; khác biệt hợp lệ khai báo rõ; `wait -> {code, how}`; suspend/resume/cancel |
+| Quyền | test âm: `..`, symlink ra ngoài, dẫn xuất rộng hơn đều phải bị chặn; consent theo **hash closure** mất hiệu lực khi phụ thuộc đổi |
 | Thư viện chuẩn | test `.vis` + so với triển khai tham chiếu trên host (ví dụ json với serde_json) |
-| ViShell | phiên kịch bản qua pty giả: gõ phím, chụp màn hình văn bản, so snapshot; gợi ý; job; Ctrl-C |
-| LSP | kịch bản JSON-RPC, snapshot |
+| ViShell | phiên kịch bản qua pty giả: gõ phím, chụp màn hình văn bản, so snapshot; gợi ý; job; Ctrl-C; panic giữ biến/cwd |
+| LSP | kịch bản JSON-RPC, snapshot; cột scalar ↔ UTF-16 ở biên |
 | Hiệu năng | benchmark cố định (vòng lặp số, chuỗi, map, pipeline 1 triệu dòng, khởi động lạnh/ấm); lịch sử, cảnh báo hồi quy > 10% |
 
 ### 18.3 Hồi quy
@@ -801,28 +1028,33 @@
 | Lệnh | Nội dung |
 |---|---|
 | `vis-dev check` | L0 crate bị chạm |
-| `vis-dev gate --fast` | fmt, clippy, build wasm32 cho crate lõi, header SPDX, L0, L1 |
+| `vis-dev gate --fast` | fmt, clippy, build wasm32 cho crate lõi, header SPDX **theo loại tệp**, **doc-examples parse**, luật 1.500 dòng + `unsafe` (có case âm), L0, L1 |
 | `vis-dev gate` | + L2 |
 | `vis-dev nightly` | L4 + benchmark + báo cáo |
 
 Không retry. Không `#[ignore]` để qua cổng.
 
+- **Cổng "doc-examples parse"** (QD §7.4): trích **mọi** khối mã nhãn `vis`/`vish-session` trong `docs/`, chạy `vis check`; **danh sách ngoại lệ phải khai báo tường minh** trong `docs/viscript/doc-examples-allow.txt` — dự kiến: khối sơ đồ ASCII, Rust, EBNF và các khối cố ý sai. Không có ngoại lệ ngầm.
+- **Tiêu chí lossless của ST-07**: (a) `fmt` in lại **đúng văn bản gốc** trên 100 % fixture đã đúng chuẩn (lossless CST→text); (b) `fmt` **idempotent** — `fmt(fmt(x)) == fmt(x)`; (c) tệp có tab: `fmt` sửa thành 4 dấu cách; (d) tệp có lỗi cú pháp: `fmt` **không sửa**, báo lỗi, mã thoát 2.
+
 ---
 
 ## 19. Lộ trình
 
 | Mốc | Nội dung | Tiêu chí xong | Phụ thuộc |
 |---|---|---|---|
-| VS-SPEC | `SPEC.md` chuẩn: ngữ pháp đầy đủ (EBNF, cả chế độ lệnh), bảng ưu tiên toán tử, quy tắc kiểu, bảng mã lỗi | Agent 0 duyệt | |
-| VS0 | syntax 2 chế độ + diag + types (kiểu cơ bản, bản ghi, enum, `T?`, Result, list/map, match xét hết, hàm, closure) + interpreter + bộ test ngữ nghĩa | ≥ 300 test `.vis`; fuzz parser 1 giờ không panic; vòng tròn fmt | VS-SPEC |
-| VS1 | generic, trait, module, bytecode + VM + xác minh + cache, vis-std lõi, vis-host-posix, `vis run/check/fmt/test` | vi sai interp/VM 100.000 chương trình 0 lệch; khởi động ấm < 20 ms [ƯL] | VS0 |
-| VSH1 | ViShell trên Linux/macOS: chế độ lệnh đầy đủ, stream, chương trình ngoài, gợi ý theo kiểu, lịch sử, job | dùng hằng ngày được; test pty xanh | VS1 |
+| VS-SPEC | **Duyệt** `docs/viscript/SPEC-lex.md` + **Phụ lục A (EBNF tự chứa)** + bảng ưu tiên 12 mức (Agent 1 đã soạn; `ST-02a` chỉ duyệt/bổ sung) + sổ mã lỗi (`ST-02b`) | `APPROVED <ngày> Agent 0` trong `viscript/agent/QUEUE.md`; 60 golden token + 30 golden biểu thức; mọi khối mã trong tài liệu parse được (ngoại lệ khai báo) | |
+| VS0 | syntax 2 chế độ + diag + types (kiểu cơ bản, bản ghi, enum, `T?`, Result, list/map, match xét hết, hàm, closure) + interpreter + bộ test ngữ nghĩa | ≥ 300 test `.vis`; fuzz parser 1 giờ không panic; vòng tròn fmt; **100 % fixture tài liệu parse** | VS-SPEC |
+| VS1 | generic, trait, module, bytecode + VM + xác minh + cache, vis-std lõi, vis-host-posix, `vis run/check/fmt/test`, `cmd_lower` | vi sai interp/VM 100.000 chương trình 0 lệch; khởi động ấm < 20 ms [ƯL] | VS0 |
+| VSH1 | ViShell trên Linux/macOS: chế độ lệnh đầy đủ, stream, chương trình ngoài, gợi ý theo kiểu, lịch sử, job (chưa chương trình toàn màn hình) | dùng hằng ngày được; test pty xanh | VS1 |
 | VS2 | async có cấu trúc, kênh, net/http, quyền + Landlock, vis-host-windows, vis-host-wasm, LSP, gói | bộ hợp chuẩn Host xanh 4 nền tảng | VS1 |
-| VSH2 | ViShell trên ViFastOs + vis-host-vifast + binding IDL | thay shell tạm của ViFastOs | VS2, ViFastOs U1 |
+| VSH2 | ViShell trên ViFastOs + vis-host-vifast (workspace ngoài) + binding IDL | thay shell tạm của ViFastOs | VS2, ViFastOs U1 |
 | VS3 | plugin Rust, thread song song, binding toolkit, ViTerm hiển thị có cấu trúc | ứng dụng desktop nhỏ viết bằng ViScript | VS2, ViFastOs G3 |
 
 VS-SPEC tới VS2 không phụ thuộc ViFastOs, chạy song song với kernel.
 
+**Cổng chặn:** `ST-02a` ở trạng thái `REVIEW` **chặn `ST-04..ST-09`** (QD §7.5); lexer/parser/fmt không bắt đầu trước khi `ST-02a` được duyệt. `ST-02b`, `ST-03`, `ST-10` không bị chặn.
+
 ---
 
 ## 20. Quy trình Agent 2 và giao việc vòng 1
@@ -832,42 +1064,66 @@
 Dùng nguyên quy trình mục 11 tài liệu ViFastOs (QUEUE, STATE, JOURNAL, BLOCKED, DECISIONS, báo cáo MD + JSON, vòng 8 bước, luật cấm, ngân sách, chạy qua đêm), với các điểm riêng:
 
 - Thư mục điều khiển `viscript/agent/`; `vifast-agentctl --project viscript`. Hai dòng có QUEUE riêng, không giẫm nhau.
-- Lệnh cổng là `vis-dev ...` thay cho `v2 ...`.
-- Thay đổi ngữ nghĩa ngôn ngữ (cú pháp, quy tắc kiểu) **luôn** qua Agent 0; Agent 2 cài đặt theo tài liệu này và SPEC. Chỗ chưa rõ: ghi BLOCKED kèm đề xuất + ví dụ, làm task khác.
+- Lệnh cổng là **`vis-dev ...`** thay cho `v2 ...` (`vis-dev check`, `vis-dev gate --fast`, `vis-dev nightly`).
+- Thay đổi ngữ nghĩa ngôn ngữ (cú pháp, quy tắc kiểu) **luôn** qua Agent 0; Agent 2 cài đặt theo tài liệu này, `SPEC-lex.md` và Phụ lục A. Chỗ chưa rõ: ghi BLOCKED kèm đề xuất + ví dụ, làm task khác.
 - Lỗi phát hiện từ fuzz/vi sai: thu nhỏ, ghi `tests/regress/`, test đỏ trước rồi mới sửa.
+- Duyệt **chỉ** bằng dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `viscript/agent/QUEUE.md` + một mục VD mới; không duyệt bằng chat (QD §4, D2-22).
 
 ### 20.2 Giao việc vòng 1
 
-| ID | Task | Phụ thuộc | Tiêu chí xong |
-|---|---|---|---|
-| VT01 | Workspace `viscript/` theo 14.2 (crate rỗng biên dịch được), `viscript/agent/`, LICENSE-MIT + LICENSE-APACHE, cổng `vis-dev gate --fast` khung, build wasm32 cho crate lõi | | Cổng chạy |
-| VT02 | `SPEC.md` từ tài liệu này: EBNF đầy đủ (cả chế độ lệnh), bảng ưu tiên toán tử, danh sách từ khoá, bảng mã lỗi ban đầu | | Gửi Agent 0 duyệt; **không chặn** VT03 tới VT09 (các task đó bám theo tài liệu này) |
-| VT03 | `vis-diag`: cấu trúc chẩn đoán, render như 14.6, bảng dịch vi/en/ja | VT01 | Snapshot test |
-| VT04 | Lexer 2 chế độ: thụt lề, literal (size, duration, percent, path, chuỗi nội suy), cấm tab, định danh Unicode + NFC | VT01 | Test bảng + fuzz 10 phút |
-| VT05 | CST + parser chế độ script (khai báo, biểu thức, match, khối), phục hồi lỗi | VT04 | In lại CST đúng nguồn; fuzz |
-| VT06 | Parser chế độ lệnh (pipeline, từ trần, cờ, `$x`, `(expr)`, closure-arg `it`, viết tắt trường) hạ về cùng AST | VT05 | Hai chế độ ra cùng AST |
-| VT07 | `vis-fmt` tối thiểu + test vòng tròn | VT05 | parse → fmt → parse bằng nhau |
-| VT08 | Runner bộ test ngữ nghĩa: đọc `.vis` + chú thích kỳ vọng, xuất TAP | VT01 | Chạy được với backend giả |
-| VT09 | Bộ test cú pháp ban đầu ≥ 100 tệp lấy từ ví dụ trong tài liệu (cả hai chế độ) | VT05, VT06, VT08 | Xanh |
-| VT10 | Báo cáo R1 (MD + JSON) | tất cả | Theo mục 11.7 tài liệu ViFastOs |
+Nhãn task đổi **VT → ST** (QD §2.1); cột Trạng thái dùng `READY | BLOCKED | REVIEW | DONE`.
 
-Kiểm kiểu và interpreter (phần còn lại của VS0) giao ở vòng 2 sau khi SPEC được duyệt. Nếu SPEC duyệt có thay đổi, Agent 2 cập nhật lexer/parser ở vòng 2.
+| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
+|---|---|---|---|---|---|
+| ST-01 | Workspace `viscript/` + `viscript/agent/` (không ghi đè) + SPDX **theo loại tệp** + cổng 1.500 dòng/`unsafe` có case âm + **`vis-host-vifast` không nằm trong `viscript/`** | — | 3 h | READY | cổng chạy; 3 case âm; `cargo build` workspace rỗng xanh |
+| ST-02a | **Duyệt + hoàn thiện** `SPEC-lex.md` + **Phụ lục A** + bảng ưu tiên (Agent 1 đã soạn) + 60 golden token + 30 golden biểu thức | ST-01 | 3 h (2 phiên) | **REVIEW** | mọi khối mã trong tài liệu parse được (ngoại lệ khai báo); Agent 0 duyệt |
+| ST-02b | Sổ mã lỗi `VS` + 4 chữ số + quy tắc cấp | ST-01 | 2 h | READY | bảng mã; `vis explain` được |
+| ST-03 | `vis-diag`: cột theo **Unicode scalar**, render, bảng vi/en/ja | ST-01 | 3 h | BLOCKED | snapshot 20 ca, có tiếng Việt/emoji; cột LSP đổi UTF-16 ở biên |
+| ST-04 | Lexer theo `SPEC-lex` (sau duyệt) | ST-02a | 3 h | BLOCKED | bảng token vàng + fuzz 10 phút + NFC/NFD; tab = `VS0107` vẫn lex |
+| ST-05 | Parser + **CST lossless** | ST-04 | 3 h | BLOCKED | 100 % fixture tài liệu parse; mỗi tệp lỗi đúng 1 lỗi đầu |
+| ST-06 | `CmdCST` + golden từng hàng bảng 9.4 + 10 ca mơ hồ | ST-05 | 3 h | BLOCKED | golden `CmdCST`; `cmd_lower` để vòng 2 |
+| ST-07 | `vis-fmt`: lossless, idempotent, sửa tab | ST-05 | 3 h | BLOCKED | vòng tròn parse→fmt→parse; 4 tiêu chí ở 18.4 |
+| ST-08 | Runner chỉ `--check` cú pháp (chưa interpreter) | ST-05 | 2 h | BLOCKED | TAP; chạy fixture |
+| ST-09 | Fixture theo QD §7.4: 1 fixture/khối mã + 1/quyết định 7.1..7.3; cấm `...` | ST-06, ST-07, ST-08 | 2 h | BLOCKED | xanh |
+| ST-10 | Báo cáo `R1-viscript` (MD + JSON, có `project`) | các task | 1 h | READY | theo §11.7 tài liệu ViFastOs |
 
+`ST-02a` **chặn `ST-04..ST-09`**; `ST-02b/ST-03/ST-10` chạy song song được. Kiểm kiểu và interpreter (phần còn lại của VS0) giao ở vòng 2 sau khi SPEC được duyệt; `cmd_lower` cũng ở vòng 2 (sau `vis-types`).
+
 ---
 
 ## 21. Các lựa chọn đã chốt
 
 | Mã | Câu hỏi | Chốt | Lý do |
 |---|---|---|---|
-| Q1 | Từ khoá tiếng Anh hay tiếng Việt? | **Tiếng Anh**; định danh Unicode cho phép tiếng Việt; thông báo lỗi có tiếng Việt | Giống JS/TS, Python, Rust: từ khoá tiếng Anh, tên biến Unicode. Từ khoá có dấu gõ chậm, phụ thuộc bộ gõ, mất quen thuộc với công cụ và tài liệu |
-| Q2 | `int / int` cho `int` hay `float`? | `int`, có lint khi chia hai literal | Nhất quán với kiểu tĩnh, không chuyển đổi ngầm |
-| Q3 | Thụt lề | 4 dấu cách bắt buộc, cấm tab | Một kiểu duy nhất, `vis fmt` tự sửa |
+| Q1 | Từ khoá tiếng Anh hay tiếng Việt? | **Tiếng Anh**; định danh Unicode cho phép tiếng Việt; thông báo lỗi có tiếng Việt; **thêm `const`, bỏ `as`, `test` là từ khoá theo ngữ cảnh** | Giống JS/TS, Python, Rust: từ khoá tiếng Anh, tên biến Unicode. Từ khoá có dấu gõ chậm, phụ thuộc bộ gõ |
+| Q2 | `int / int` cho `int` hay `float`? | `int`, có lint khi chia hai literal (hạ mức) và lint khi kết quả dùng với `float`/`percent`; `%`/`**` theo bảng 3.6 | Nhất quán với kiểu tĩnh, không chuyển đổi ngầm |
+| Q3 | Thụt lề | **Đầu vào: thụt lề nhất quán bất kỳ, đơn vị suy từ cấp đầu; đầu ra `vis fmt` luôn 4 dấu cách** [TK]. **Tab**: `VS0107` là cảnh báo-lỗi của lexer, coi tab = 4 cột, **vẫn lex tiếp**; `vis check` coi là lỗi; `vis fmt` sửa | Dán mã không chết ở dòng đầu; vẫn một kiểu ở đầu ra; vá mâu thuẫn "tab là lỗi từ vựng nhưng fmt tự sửa" |
 | Q4 | Tên | ViScript (`.vis`, lệnh `vis`), ViShell (`.vish`, lệnh `vish`) | Ngắn, nhất quán với hệ ViFast |
-| Q5 | Chỉ số ngoài phạm vi | `xs[i]` panic, `xs.get(i)` trả `T?` | Lỗi lập trình hiện rõ; vẫn có đường an toàn |
-| Q6 | Giấy phép | **MIT OR Apache-2.0** | Giống Rust; dễ dùng lại, có điều khoản bằng sáng chế của Apache |
+| Q5 | Chỉ số ngoài phạm vi | **`xs[i]` panic** với thông điệp nêu chỉ số + độ dài; **`xs[i]?` trả `T?`**; `xs.get(i)` trả `T?`; `xs[-1]` panic có thông điệp; **cấm `s[i]`** trên `str` (dùng `.chars()/.graphemes()/.bytes()/.byte_at(i)?`) | Lỗi lập trình hiện rõ; vẫn có đường an toàn; `str` không có chỉ số theo byte/scalar |
+| Q6 | Giấy phép | **MIT OR Apache-2.0** | Giống Rust; dễ dùng lại, có điều khoản bằng chứng nhận của Apache |
 | Q7 | `decimal` trong lõi | Không, là thư viện | Giữ lõi nhỏ |
-| Q8 | Toán tử bit | Hàm (`bit_or`...), không ký hiệu | Giải phóng `\|` cho pipe ở mọi chế độ |
+| Q8 | Toán tử bit | Hàm `bit_and`/`bit_or`/`bit_xor`/`bit_not`/`shl`/`shr`/`ushr`, không ký hiệu. **`\|` không bao giờ là toán tử nhị phân trong biểu thức; pattern giữ `\|`** (or-pattern) và chế độ lệnh giữ `\|` cho pipe | Giải phóng `\|` khỏi bit-ops mà vẫn giữ or-pattern quen thuộc; sửa câu cũ "giải phóng `\|` cho pipe ở mọi chế độ" |
 
+### 21.1 Chốt bổ sung sau review (trỏ QD §7)
+
+| # | Chốt | QD |
+|---|---|---|
+| 1 | **NFC toàn tệp trước khi lex**; cột chẩn đoán theo scalar, LSP đổi UTF-16 ở biên | §7.1 |
+| 2 | Float phải có chữ số sau `.`; `1..2` là range; `_` chỉ giữa hai chữ số; literal tràn i64 = lỗi biên dịch | §7.1 |
+| 3 | Đơn vị `size` = `b kb mb gb tb kib mib gib tib`; `duration` = `ns us ms s m h d w`, ghép giảm dần là một token | §7.1 |
+| 4 | **Bỏ `50%` → `50pct`**; `%` chỉ modulo; job spec `job 1` | §7.1 |
+| 5 | Năm dạng chuỗi + bảng escape + nội suy `{expr}`/`{expr:spec}`/`{{`/`}}` | §7.1 |
+| 6 | `#`: script luôn là chú thích; lệnh chỉ khi đứng đầu từ; `https://x/y#frag` giữ nguyên | §7.1 |
+| 7 | `set{` dính liền là set literal; có khoảng trắng là định danh; `?` maximal munch | §7.1 |
+| 8 | `%` dấu theo số bị chia, `%0` panic, có `rem_euclid`/`mod_floor`; `**` phải-kết-hợp, cao hơn `*`, mũ âm panic | §7.3 |
+| 9 | Bit: thêm `bit_not`, `ushr`; `shr` số học; đếm ≥ 64 panic; có `checked_shl/shr` | §7.3 |
+| 10 | Bảng ưu tiên **12 mức** (postfix → `**` → một ngôi → `* / %` → `+ -` → `..` → `??` → so sánh → `not` → `and` → `or` → `\|>`) | §7.3 |
+| 11 | Optional: tự nâng `T → T?` ở gán/trả/đối số; `T??` lỗi; `?` trên `T?`; pattern `none/some/ok/err` | §7.3 |
+| 12 | Xét hết: guard **không** tính coverage; kiểu vô hạn **bắt buộc** `_` | §7.3 |
+| 13 | `trait Error: Show { message, source, span }`; `AnyError` xoá kiểu; `?` tự chuyển; `main -> Result[(), AnyError]`; plugin dùng `vis::Error` | §7.3 |
+| 14 | Mã thoát 0/1/2/101/126/127/130; panic payload `AnyError`; ViShell bọc **mỗi lệnh** trong task arena riêng, panic giải phóng arena, giữ biến/cwd/job/lịch sử; chưa có `defer` | §7.3 |
+| 15 | Consent nhớ theo **hash closure** (nguồn + phụ thuộc + phiên bản compiler), dùng chung hash cache | §7.3 |
+
 ---
 
 ## 22. Quyết định
@@ -875,7 +1131,7 @@
 | Mã | Nội dung |
 |---|---|
 | VD-01 | ViScript có kiểu tĩnh bắt buộc, không `any`, không null |
-| VD-02 | Một ngôn ngữ, hai chế độ cú pháp, cùng AST/kiểm kiểu/VM; ViShell là chương trình riêng |
+| VD-02 | Một ngôn ngữ, hai chế độ cú pháp, cùng **HIR**/kiểm kiểu/VM; ViShell là chương trình riêng |
 | VD-03 | Ngữ nghĩa giá trị + RC + copy-on-write; không GC truy vết; chu trình chỉ qua `ref`, phá bằng `weak` |
 | VD-04 | Lõi (syntax, types, bytecode, vm) không phụ thuộc hệ điều hành, build được wasm32 |
 | VD-05 | Mọi truy cập hệ thống qua capability; không hàm toàn cục chạm hệ thống |
@@ -884,6 +1140,18 @@
 | VD-08 | Thay đổi SPEC chỉ qua Agent 0; mỗi tính năng mới phải có ví dụ + test trước |
 | VD-09 | Từ khoá tiếng Anh, định danh Unicode (NFC), thông báo lỗi vi/en/ja |
 | VD-10 | Giấy phép MIT OR Apache-2.0; phụ thuộc phải tương thích |
+| VD-11 | **Parse chế độ lệnh hai pha**: `vis-syntax` sinh `CmdCST` (Word chưa định kiểu) → `cmd_lower` trong `vis-types` dùng bảng chữ ký → HIR. "Cùng AST" đổi thành "cùng HIR"; `ST-06` chỉ làm `CmdCST` + golden, `cmd_lower` ở vòng 2 |
+| VD-12 | `docs/viscript/SPEC-lex.md` là **nguồn từ vựng duy nhất** (token, escape, đơn vị, maximal munch, golden token); Phụ lục A là nguồn cú pháp; `ST-02a` chặn `ST-04..ST-09` |
+| VD-13 | **Toàn bộ tệp nguồn chuẩn hoá NFC trước khi lex**; lexer, chẩn đoán và `vis fmt` đều làm việc trên dạng NFC |
+| VD-14 | **Cấm `s[i]`** trên `str`; dùng `.chars()/.graphemes()/.bytes()/.byte_at(i)?`; `xs[i]` panic, `xs[i]?` trả `T?` |
+| VD-15 | **Bỏ literal `%`**: percent viết `50pct`; `%` chỉ là modulo; job spec `job 1` |
+| VD-16 | **Bảng ưu tiên 12 mức** là một phần SPEC; `**` kết hợp phải và cao hơn `*`; `not` thấp hơn so sánh; `\|` không bao giờ là toán tử nhị phân |
+| VD-17 | **Shell sở hữu tty**; tiến trình con nhận pipe, không nhận `cap[Term]`; chương trình toàn màn hình để sau với tty server |
+| VD-18 | `from json` không tham số trả `Json`; `where`/`get` trên `Json` là lỗi kiểu gợi ý `from json[T]`; SIGPIPE và `take` không phải lỗi; `\| complete` trả `{stdout,stderr,code,how}` |
+| VD-19 | **Bảng mã thoát** 0/1/2/101/126/127/130 dùng chung cho CLI, `main`, `\| complete`, job control |
+| VD-20 | **`trait Error: Show { message, source, span }`**, `AnyError` là kiểu xoá, mọi `E: Error` tự chuyển qua `?`, `main -> Result[(), AnyError]`, plugin dùng `vis::Error` |
+| VD-21 | Consent và cache dùng chung **hash closure** (nguồn + phụ thuộc + phiên bản compiler) |
+| VD-22 | `ST-02a` ở `REVIEW` chặn `ST-04..ST-09`; duyệt chỉ bằng dòng `APPROVED` (D2-22) |
 
 ---
 
@@ -893,52 +1161,247 @@
 |---|---|
 | Phạm vi ngôn ngữ phình | VD-08; lộ trình bậc; tính năng chỉ vào khi có ca dùng thật |
 | Kiểm kiểu generic + trait phức tạp, thông báo lỗi khó hiểu | Suy kiểu cục bộ; generic kiểm một lần với ràng buộc; snapshot thông báo lỗi là test bắt buộc |
-| Hai chế độ cú pháp mập mờ | Bảng xung đột 9.4 là một phần SPEC; mọi dòng lệnh mẫu có test AST |
+| Hai chế độ cú pháp mập mờ | Bảng xung đột 9.4 là một phần SPEC; mọi dòng lệnh mẫu có test `CmdCST`; hai pha tách bạch (VD-11) |
 | VM chậm | Lệnh có kiểu, không thẻ kiểu; benchmark từ VS1; chuyên biệt hoá generic khi có số đo |
 | Ngữ nghĩa giá trị gây sao chép tốn | CoW + chuyển quyền khi RC = 1; đo trong benchmark |
 | Quyền trên OS khác không chặt bằng ViFastOs | Ghi rõ mức bảo đảm (11.4); tăng cường Landlock |
 | Hai dòng ViScript và ViFastOs tranh tài nguyên Agent 2 | QUEUE riêng; ViScript làm nhánh độc lập khi dòng OS BLOCKED |
+| Tài liệu lệch mã (như v1) | Cổng **doc-examples parse** (18.4) + fixture ST-09 |
 
 ---
 
-## Phụ lục A. Ngữ pháp rút gọn (chế độ script)
+## Phụ lục A. EBNF tự chứa (chế độ script + chế độ lệnh)
 
+Đây là **văn phạm chuẩn tắc**, không phải "giả ngữ pháp": mọi luật dưới đây là điều kiện cần và đủ để parser của `ST-05`/`ST-06` chấp nhận đúng các ví dụ trong tài liệu. Token viết HOA (`IDENT`, `INT`, `STRING`, `WORD`, `NAME`, `NEWLINE`, `INDENT`, `DEDENT`...) do `vis-syntax` sinh theo `SPEC-lex.md`; mọi ký tự còn lại là terminal của ngôn ngữ. `(* ... *)` là chú thích của văn phạm.
+
 ```
-file        := item*
-item        := use | fn_decl | cmd_decl | type_decl | enum_decl | trait_decl | impl_decl | const_decl | test_decl | extern_cmd
-fn_decl     := ['pub'] ['async'] 'fn' IDENT generics? '(' params ')' ['->' type] ':' block
-cmd_decl    := ['pub'] 'cmd' IDENT '(' params ')' ['->' type] ':' block
-params      := (param (',' param)*)?
-param       := ['--'] IDENT ':' type ['=' expr] | ['var'] 'self'
-type_decl   := ['pub'] 'type' IDENT generics? ['derives' path_list] ':' NEWLINE INDENT field+ DEDENT
-enum_decl   := ['pub'] 'enum' IDENT generics? ['derives' path_list] ':' NEWLINE INDENT variant+ DEDENT
-block       := stmt | NEWLINE INDENT stmt+ DEDENT
-stmt        := 'let' pattern [':' type] '=' expr ['else' IDENT? ':' block]
-             | 'var' IDENT [':' type] '=' expr
-             | assign | 'return' expr? | 'break' | 'continue'
-             | if | for | while | loop | match | scope | expr
-expr        := lambda | pipe_expr
-pipe_expr   := or_expr ('|>' or_expr)*
-or_expr     := and_expr ('or' and_expr)*
-and_expr    := not_expr ('and' not_expr)*
-not_expr    := 'not' not_expr | cmp_expr
-cmp_expr    := coalesce (cmp_op coalesce)?
-coalesce    := range ('??' range)*
-range       := add (('..' | '..=') add)?
-add         := mul (('+' | '-') mul)*
-mul         := unary (('*' | '/' | '%') unary)*
-unary       := '-' unary | power
-power       := postfix ('**' unary)?
-postfix     := primary ( '.' IDENT | '?.' IDENT | '(' args ')' | '[' expr ']' | '?' | '[' types ']' | 'with' field_updates )*
-primary     := literal | IDENT | '(' expr ')' | list | map | set | record | 'cmd' '{' cmd_pipeline '}' | if_expr | match_expr | 'await' postfix
-type        := IDENT ['[' type (',' type)* ']'] ['?'] | '(' types ')' | 'fn' '(' types ')' '->' type | '{' field_types '}'
+(* ===== A.1 Khung tệp và item ===== *)
+
+file            := NEWLINE* item* EOF
+item            := use_decl | fn_decl | cmd_decl | type_item | enum_decl
+                 | trait_decl | impl_decl | const_decl | test_decl | extern_decl
+
+use_decl        := 'use' dotted_path NEWLINE
+dotted_path     := IDENT ('.' IDENT)*
+
+fn_decl         := ['pub'] ['async'] 'fn' IDENT generics? '(' params? ')' ['->' type] ':' suite
+cmd_decl        := ['pub'] 'cmd' IDENT '(' params? ')' ['->' type] ':' suite
+extern_decl     := 'extern' 'cmd' IDENT '(' params? ')'
+
+params          := param (',' param)*
+param           := '...' IDENT ':' type
+                 | ['--'] IDENT ':' type ['@' path_lit] ['(' grant_list ')'] ['=' expr]
+                 | ['var'] 'self'
+grant_list      := IDENT (',' IDENT)*
+
+generics        := '[' generic (',' generic)* ']'
+generic         := IDENT [':' bound]
+bound           := dotted_path | 'has' record_type
+
+type_item       := type_decl | alias_decl | newtype_decl
+type_decl       := ['pub'] 'type' IDENT generics? derives? ':' NEWLINE INDENT field+ DEDENT
+field           := IDENT ':' type ['=' expr] NEWLINE
+alias_decl      := 'alias' IDENT '=' type NEWLINE
+newtype_decl    := 'newtype' IDENT '=' type NEWLINE
+
+enum_decl       := ['pub'] 'enum' IDENT generics? derives? ':' NEWLINE INDENT variant+ DEDENT
+variant         := IDENT ['(' variant_fields ')'] NEWLINE
+variant_fields  := IDENT ':' type (',' IDENT ':' type)*
+derives         := 'derives' dotted_path (',' dotted_path)*
+
+trait_decl      := ['pub'] 'trait' IDENT generics? [':' supertraits] ':' NEWLINE INDENT trait_item+ DEDENT
+supertraits     := dotted_path (',' dotted_path)*
+trait_item      := ['pub'] 'fn' IDENT '(' params? ')' ['->' type] (':' suite | NEWLINE)
+
+impl_decl       := 'impl' [dotted_path 'for'] type generics? ':' NEWLINE INDENT impl_item+ DEDENT
+impl_item       := fn_decl
+
+const_decl      := ['pub'] 'const' IDENT ':' type '=' expr NEWLINE
+test_decl       := 'test' 'fn' IDENT '(' ')' ':' suite
+
+(* ===== A.2 Khối và câu lệnh ===== *)
+
+suite           := simple_stmt NEWLINE                  (* khối một dòng: `if x > 0: return x` *)
+                 | NEWLINE INDENT stmt+ DEDENT
+simple_stmt     := let_head | var_head | assign_head | return_stmt | break_stmt
+                 | continue_stmt | expr
+
+stmt            := let_stmt | var_stmt | assign_stmt | return_stmt | break_stmt
+                 | continue_stmt | for_stmt | while_stmt | loop_stmt | scope_stmt
+                 | if_stmt | match_stmt | expr_stmt
+
+let_stmt        := let_head NEWLINE
+let_head        := 'let' pattern [':' type] '=' expr [else_clause]
+else_clause     := 'else' [IDENT] ':' suite
+
+var_stmt        := var_head NEWLINE
+var_head        := 'var' IDENT [':' type] '=' expr
+assign_stmt     := assign_head NEWLINE
+assign_head     := lvalue assign_op expr
+assign_op       := '=' | '+=' | '-=' | '*=' | '/=' | '%='
+lvalue          := IDENT postfix_op*
+
+return_stmt     := 'return' [expr] NEWLINE
+break_stmt      := ('break' | 'continue') NEWLINE
+
+for_stmt        := 'for' pattern 'in' expr ':' suite
+while_stmt      := 'while' expr ':' suite
+loop_stmt       := 'loop' ':' suite
+scope_stmt      := 'scope' [IDENT] ':' suite
+
+if_stmt         := 'if' if_cond ':' suite elif_arm* else_arm?
+if_cond         := 'let' pattern '=' expr | expr        (* `if let` = 6.3 *)
+elif_arm        := 'elif' if_cond ':' suite
+else_arm        := 'else' ':' suite
+
+match_stmt      := 'match' expr ':' NEWLINE INDENT match_arm+ DEDENT
+match_arm       := pattern ['if' expr] ':' suite
+expr_stmt       := expr NEWLINE
+
+(* ===== A.3 Biểu thức — đúng bảng ưu tiên A.5 ===== *)
+
+expr            := lambda | pipe_expr
+lambda          := 'fn' '(' params? ')' ['->' type] ':' suite
+                 | lambda_params '=>' expr
+lambda_params   := IDENT | '(' IDENT (',' IDENT)* ')'
+
+pipe_expr       := or_expr ('|>' or_expr)*
+or_expr         := and_expr ('or' and_expr)*
+and_expr        := not_expr ('and' not_expr)*
+not_expr        := 'not' not_expr | cmp_expr
+cmp_expr        := coalesce [cmp_op coalesce]
+cmp_op          := '==' | '!=' | '<' | '<=' | '>' | '>='
+coalesce        := range_expr ('??' range_expr)*
+range_expr      := add_expr [('..' | '..=') add_expr]
+add_expr        := mul_expr (('+' | '-') mul_expr)*
+mul_expr        := unary_expr (('*' | '/' | '%') unary_expr)*
+unary_expr      := '-' unary_expr | power_expr
+power_expr      := postfix ['**' unary_expr]
+postfix         := primary postfix_op*
+postfix_op      := '.' IDENT | '?.' IDENT | '(' args? ')' | '[' expr ']'
+                 | '[' types ']' | '?' | 'with' field_updates
+
+primary         := literal | 'self' | IDENT | 'ref' '(' expr ')' | '(' ')'
+                 | '(' expr ')' | '(' expr ',' expr (',' expr)* ','? ')'
+                 | list_lit | map_lit | set_lit | record_lit
+                 | 'cmd' '{' pipeline '}' | if_expr | match_expr
+                 | 'await' postfix
+
+args            := positional_args? named_args?
+positional_args := expr (',' expr)*
+named_args      := (',' '--' IDENT [':' expr])+
+
+field_updates   := IDENT ':' expr (',' IDENT ':' expr)*
+
+list_lit        := '[' [expr (',' expr)* ','?] ']'
+map_lit         := '{' [map_entry (',' map_entry)* ','?] '}'
+map_entry       := expr ':' expr
+set_lit         := 'set' '{' [expr (',' expr)* ','?] '}'
+record_lit      := '{' record_field (',' record_field)* ','? '}'
+record_field    := IDENT ':' expr | IDENT
+
+if_expr         := 'if' if_cond ':' suite elif_arm* else_arm      (* else bắt buộc *)
+match_expr      := 'match' expr ':' NEWLINE INDENT match_arm+ DEDENT
+
+(* ===== A.4 Pattern ===== *)
+
+pattern         := or_pattern
+or_pattern      := range_pattern ('|' range_pattern)*
+range_pattern   := atom_pattern [('..' | '..=') atom_pattern]
+atom_pattern    := '_' | literal | '-' INT | 'none' | dotted_path
+                 | '(' pattern (',' pattern)* ')'
+                 | '[' [list_pat] ']'
+                 | '{' [record_pat (',' record_pat)*] '}'
+                 | dotted_path '(' [pattern (',' pattern)*] ')'
+list_pat        := pattern (',' pattern)* [',' '..' [IDENT]]
+record_pat      := IDENT ':' pattern | IDENT | '..'
+
+(* ===== A.5 Kiểu ===== *)
+
+type            := type_primary ['?']                 (* T?? là lỗi kiểu, 4.4 *)
+type_primary    := dotted_path ['[' types ']']
+                 | 'ref' '[' type ']'
+                 | 'weak' '[' type ']'
+                 | 'cap' '[' cap_kind (',' grant_list)? ']'
+                 | 'fn' '(' [types] ')' '->' type
+                 | '(' ')'                            (* unit *)
+                 | '(' type ',' type (',' type)* ')'  (* tuple *)
+                 | '{' record_type '}'
+types           := type (',' type)*
+cap_kind        := dotted_path
+record_type     := IDENT ':' type (',' IDENT ':' type)*
+
+(* ===== A.6 Token ===== *)
+
+literal         := INT | FLOAT | CHAR | BOOL | 'none'
+                 | STRING | RAW_STRING | TRIPLE_STRING | RAW_TRIPLE_STRING
+                 | BYTES | PATH_STRING | size_lit | duration_lit | percent_lit
+size_lit        := INT SIZE_UNIT
+duration_lit    := INT DUR_UNIT (INT DUR_UNIT)*         (* ghép giảm dần: 2h30m *)
+percent_lit     := (INT | FLOAT) 'pct'
+path_lit        := PATH_STRING | STRING
+
+SIZE_UNIT       := 'b' | 'kb' | 'mb' | 'gb' | 'tb' | 'kib' | 'mib' | 'gib' | 'tib'
+DUR_UNIT        := 'ns' | 'us' | 'ms' | 's' | 'm' | 'h' | 'd' | 'w'
+
+NEWLINE         := xuống dòng ngoài ngoặc và ngoài chuỗi; bị bỏ khi dòng trước
+                   kết thúc bằng `|>` hoặc dòng này bắt đầu bằng `|>` ở độ sâu 0
+INDENT          := một mức thụt lề mới; đơn vị suy từ cấp thụt lề đầu tiên của tệp
+DEDENT          := hết mức thụt lề; ngầm ở EOF và ở cuối mỗi input REPL
+
+(* ===== A.7 Chế độ lệnh ===== *)
+
+pipeline        := stage ('|' stage)*
+stage           := word_cmd arg* | '(' expr ')'
+word_cmd        := NAME | '^' NAME | '$' IDENT ('.' IDENT)*
+arg             := word | flag | '(' expr ')' | STRING | closure_arg
+flag            := '--' IDENT ['=' expr] | '-' IDENT ['=' expr] | '--'
+word            := WORD
+closure_arg     := IDENT '=>' closure_body | field_shorthand
+closure_body    := biểu thức kéo tới `|` / `;` / `&` / NEWLINE ở độ sâu 0
+field_shorthand := biểu thức chế độ biểu thức với `it` ngầm
+
+shell_file      := shell_item*
+shell_item      := use_decl | fn_decl | cmd_decl | let_stmt | pipeline NEWLINE
 ```
 
-Chế độ lệnh: xem 9.3. Bản đầy đủ nằm trong `SPEC.md` (VT02).
+### A.5 Bảng ưu tiên (bản chuẩn tắc, khớp 4.5)
 
+| Mức | Nhóm | Toán tử | Kết hợp |
+|---|---|---|---|
+| 1 | postfix | `.` `?.` `()` `[]` `?` `with` | trái |
+| 2 | luỹ thừa | `**` | phải |
+| 3 | một ngôi | `-` | phải |
+| 4 | nhân | `* / %` | trái |
+| 5 | cộng | `+ -` | trái |
+| 6 | khoảng | `.. ..=` | không kết hợp |
+| 7 | coalesce | `??` | phải |
+| 8 | so sánh | `== != < <= > >=` | không kết hợp |
+| 9 | phủ định | `not` | phải |
+| 10 | hội | `and` | trái, ngắn mạch |
+| 11 | tuyển | `or` | trái, ngắn mạch |
+| 12 | nối | `\|>` | trái |
+
+### Ghi chú ràng buộc văn phạm
+
+1. **Khối một dòng** (`suite := simple_stmt NEWLINE`) không sinh `INDENT`/`DEDENT`; dùng cho `if x > 0: return x`, `if loud: s.upper() else: s`.
+2. **`if`/`match` vừa là câu lệnh vừa là biểu thức**: bản biểu thức (`if_expr`) **bắt buộc** có `else`; bản câu lệnh thì không.
+3. **`elif`** có mặt ở cả hai dạng (`elif_arm`), và có thể lặp.
+4. **Phân biệt bằng lookahead 2 token (LL(2)):**
+   - `{ IDENT ':'` → `record_lit`; ngược lại `{` → `map_lit`. Map có khoá là định danh trần phải viết khoá không-trần (`{"a": 1}`, `{(k): v}`).
+   - `IDENT '[' ... ']' '('` **và** `IDENT` là hàm generic đã biết → danh sách kiểu (gọi generic); ngược lại → index (5.3).
+   - `'(' IDENT (',' IDENT)* ')' '=>'` → `lambda`; ngược lại → nhóm/tuple.
+   - `set` **dính liền** `{` → `set_lit`; có khoảng trắng → định danh (2.4).
+   - `cmd` `{` → khối lệnh; `cmd` khác → định danh.
+5. **`|` không có luật sinh trong biểu thức** (chỉ có trong `or_pattern` và `pipeline`); đó là cách văn phạm cưỡng chế Q8/VD-16.
+6. **Chế độ lệnh** dùng `CmdCST` chưa định kiểu (A.7); `closure_body` và `field_shorthand` được cắt theo **độ sâu ngoặc 0** và được phân giải ở `cmd_lower`, không ở parser. `word` không chứa metachar (9.3).
+7. **`NEWLINE`, `INDENT`, `DEDENT`** do lexer sinh theo 2.2; "giả ngữ pháp" của bản trước (12 luật, thiếu `pattern`, `if_expr`, khối một dòng) **bị bỏ** — thay bằng văn phạm này.
+
+---
+
 ## Phụ lục B. Chương trình mẫu
 
-```python
+```vis
 #!/usr/bin/env vis
 use std.fmt.json
 use std.time
@@ -949,7 +1412,7 @@
     modified: datetime
 
 ## Tìm 20 tệp lớn nhất đã sửa trong 7 ngày, ghi báo cáo JSON
-fn main(src: cap[Dir] @ ".", out: cap[Dir] @ "./reports" (write)) -> Result[(), Error]:
+fn main(src: cap[Dir] @ p".", out: cap[Dir] @ p"./reports" (write)) -> Result[(), AnyError]:
     let since = time.now() - 7d
     let files = src.walk()
         |> filter(e => e.is_file() and e.modified >= since)
@@ -967,15 +1430,25 @@
     assert xs == [3, 2, 1]
 ```
 
-Cùng việc trong ViShell:
+Ghi chú cho ví dụ trên (đều là sửa sau review):
 
-```python
-> walk . | where is_file and modified >= (now) - 7d | sort_by size --desc | take 20 | to json --pretty | save reports/big-files.json
+- Dòng bắt đầu bằng `|>` **nối với dòng trước** (2.2, A.6) — không cần ngoặc.
+- `str(e.path)` hợp lệ vì `str(x) = x.show()` (3.5).
+- `time.now()` thay `now` trần; `is_file()` nhất quán ở cả hai chế độ (gọi phương thức, có `()`).
+- `@ p"."` là literal `path` tường minh (3.5, 11.2).
+- `Result[(), AnyError]` theo VD-20.
+
+Cùng việc trong ViShell (mỗi dòng là **input hợp lệ**, không có tiền tố `>`):
+
+```vish-session
+walk . | where it => it.is_file() and it.modified >= time.now() - 7d | sort_by size --desc | take 20 | to json --pretty | save reports/big-files.json
+ps | where cpu > 50pct | each p => p.kill()
+ls ./photos | where size > 5mb | sort_by modified --desc | take 10
 ```
 
 Định danh tiếng Việt:
 
-```python
+```vis
 fn tính_tổng(danh_sách: list[int]) -> int:
     var tổng = 0
     for số in danh_sách:

````

---

# PHẦN C — SPEC-lex.md (toàn văn, tệp mới)

# SPEC-lex — Đặc tả từ vựng ViScript

Trạng thái: **REVIEW** (ST-02a). Nguồn sự thật: `docs/QUYET-DINH-sau-review.md` §7.1–7.2 (QD).
Tài liệu liên quan: `ViScript-thiet-ke.md` §2, §4.5, §9, Phụ lục A.
Quy ước: "scalar" = Unicode scalar value; mọi offset tính bằng byte, cột tính bằng scalar (1-based).

## 0. Nguyên tắc

1. Tệp UTF-8; **chuẩn hoá NFC toàn tệp trước khi lex** (QD §7.1). Mọi literal/định danh đều ở dạng NFC.
2. Lexer là hàm thuần: `bytes -> [Token] | [Diagnostic]`; không phụ thuộc kiểu.
3. Maximal munch, trừ các luật ghi rõ.
4. Token luôn có `{kind, text, span:{start,end}, line, col}`; `col` theo scalar.
5. Tab: sinh `VS0107` (cảnh báo-lỗi), **coi tab = 4 cột**, vẫn lex tiếp; `vis check` coi là lỗi, `vis fmt` sửa.

## 1. Bảng token

| Kind | Regex / luật | Ví dụ | Ghi chú |
|---|---|---|---|
| `IDENT` | UAX #31 + `_`; chữ số không đứng đầu; NFC | `số_lượng`, `_tmp` | từ khoá tra sau |
| `KW` | danh sách §2.1 tài liệu | `let`, `fn`, `const` | `test` theo ngữ cảnh |
| `INT` | `[0-9](_?[0-9])*` hoặc `0x[0-9a-fA-F](_?..)*` hoặc `0b[01](_?..)*` | `42`, `1_000`, `0xff` | `_` chỉ **giữa** hai chữ số; tràn i64 = lỗi `VS0104` |
| `FLOAT` | `INT '.' [0-9]+ ([eE][+-]?[0-9]+)?` hoặc `INT [eE][+-]?[0-9]+` | `3.14`, `1e-9` | `1.` sai (`VS0104`); `1..2` là `INT RANGE INT` |
| `CHAR` | `'` scalar `'` | `'a'`, `'ố'` | đúng 1 scalar sau NFC; nhiều scalar = `VS0105` |
| `STRING` | `"..."` có escape + nội suy | `"xin {name}"` | xem §3 |
| `RAW_STRING` | `r"..."` không escape, không nội suy | `r"C:\raw"` | |
| `MULTI_STRING` | `"""..."""` có escape + nội suy | `"""a\nb"""` | |
| `RAW_MULTI` | `r"""..."""` | | thô |
| `BYTES` | `b"..."` ASCII + `\xNN` | `b"\x00"` | ký tự > 0x7F = `VS0106` |
| `PATH` | `p"..."` | `p"./a.txt"` | thân như STRING, không nội suy? (QD: p là path; nội suy **có**) |
| `SIZE` | `INT` + `b\|kb\|mb\|gb\|tb\|kib\|mib\|gib\|tib` **không cách** | `5kb` | `kb`=1000, `kib`=1024 |
| `DURATION` | `INT` + `ns\|us\|ms\|s\|m\|h\|d\|w`, ghép **giảm dần** không cách | `250ms`, `2h30m` | `2h30m` = **một** token |
| `PERCENT` | `INT` + `pct` | `50pct` | **không còn** `50%` |
| `PUNCT` | `( ) [ ] { } , : . .. ..= ; -> => = == != < <= > >= + - * / % ** & \| \|> ? ?. ?? ! @ # ## ^ ~ \\` | | xem §5 |
| `NEWLINE` | `\n` (chuẩn hoá CRLF→LF) | | trong ngoặc bị nuốt theo §4 |
| `INDENT`/`DEDENT` | theo §4 | | |
| `EOF` | hết tệp | | sinh DEDENT còn lại |

## 2. Số

- `INT` là `i64`; literal tràn là **lỗi biên dịch** (`VS0104`), không phải panic runtime.
- `FLOAT` là `f64`; `NaN`/`inf` không có literal; dùng `float.nan`/`float.inf`.
- `_` chỉ giữa hai chữ số: `1_000` đúng; `_1`, `1_`, `1__0` sai.
- `..`/`..=` thắng `.` khi gặp `1..2`; `1.2..3.4` là `FLOAT RANGE FLOAT`.
- `-` là toán tử một ngôi, không thuộc literal; `-9223372036854775808` = `-(922...)` và **tràn** ⇒ lỗi; viết `i64.min`.

## 3. Chuỗi, escape, nội suy

| Dạng | Escape | Nội suy | Ghi chú |
|---|---|---|---|
| `"..."` | có | có | |
| `r"..."` | không | không | `\` là ký tự thường |
| `"""..."""` | có | có | nhiều dòng, giữ newline |
| `r"""..."""` | không | không | |
| `b"..."` | chỉ `\xNN` | không | ASCII |
| `p"..."` | có | có | path literal |

Escape hợp lệ: `\n \t \r \0 \\ \" \' \{ \} \u{X..X}` (X là 1–6 hex, ≤ 0x10FFFF, không surrogate).
Nội suy: `{expr}` hoặc `{expr:spec}`; `{{` → `{`, `}}` → `}`. `{` không đóng = `VS0108`.
Trong `r"..."`/`r"""..."""` không có escape/nội suy, nên `{` là ký tự thường.
Chuỗi chưa đóng = `VS0101`; escape sai = `VS0102`; `\x` ngoài `b""` = `VS0102`.

## 4. Thụt lề và NEWLINE/INDENT/DEDENT

Thuật toán (chuẩn, QD §7.1):
1. Đầu vào: thụt lề nhất quán bất kỳ; **đơn vị = mức thụt đầu tiên > 0** gặp trong tệp. Mọi mức sau phải là bội của đơn vị; sai = `VS0109`.
2. Dòng trống hoặc chỉ có chú thích: **không** sinh NEWLINE/INDENT/DEDENT.
3. Trong `() [] {}`: bỏ qua NEWLINE/INDENT/DEDENT (xuống dòng tự do); đóng ngoặc trả lại trạng thái ngoài.
4. Chuỗi nhiều dòng (`"""`, `r"""`) nuốt newline bên trong trước khi tính cột.
5. Dòng bắt đầu bằng `|>` được nối với dòng trước (không sinh NEWLINE); áp dụng cả trong ngoặc và ngoài.
6. `DEDENT` ngầm ở EOF và ở cuối mỗi input REPL; `INDENT`/`DEDENT` không khớp = `VS0109`.
7. Tab: `VS0107`, tab = 4 cột (chỉ để tính cột), **vẫn lex tiếp**.

## 5. Xử lý nhập nhằng (maximal munch + luật riêng)

| Cặp | Luật |
|---|---|
| `.` vs `..` vs `..=` | dài nhất thắng; `1..2` không phải float |
| `?` vs `?.` vs `??` | dài nhất; `x??y` = `x ?? y` |
| `\|` vs `\|>` | dài nhất; `\|>` là pipe biểu thức, `\|` or-pattern/pipe lệnh |
| `*` vs `**` | dài nhất; `**` luỹ thừa, `*` nhân/glob |
| `%` | **chỉ** modulo; `50pct` là percent; `%1` không phải jobspec (dùng `job 1`) |
| `set{` | `set` **dính liền** `{` = set literal; `set {` = định danh + block |
| `#` | script: luôn chú thích; lệnh: chỉ khi đứng đầu từ; `https://x/y#frag` giữ nguyên |
| `-5` | số âm nếu ngữ cảnh kiểu số; ngược lại là cờ (do `cmd_lower`, không phải lexer) |
| `@` | chỉ trong tham số `main` (`cap[Dir] @ "..."`); ngoài ra `VS0110` |
| `^` | chỉ ở đầu stage chế độ lệnh (ép gọi ngoài) |

## 6. Chế độ lệnh

Metachar: khoảng trắng, `( ) [ ] { } ; & $ ' " \|`; `#` khi đứng đầu từ; `^` ở đầu stage.
- `word` = chuỗi liên tục không chứa metachar; có thể chứa `\` escape **chỉ trong chuỗi**.
- `"..."` có nội suy; `'...'` thô; `$x` biến phiên; `$env.NAME` môi trường.
- `;` tách pipeline; `&` nền chỉ khi token cuối + có khoảng trắng trước; `--` kết thúc cờ.
- `[`/`]` là list literal/glob class: list literal phải nằm trong `( )` khi làm đối số; glob class giữ nguyên.
- `closure_arg` kéo tới `|` kế tiếp ở độ sâu 0.
- `>` `<` là so sánh, **không** redirect; ghi tệp bằng `| save`.
- Luật `.vish`/REPL: `shell_item := use | let | fn | cmd | pipeline`.

## 7. Mã lỗi từ vựng

| Mã | Nghĩa |
|---|---|
| VS0101 | chuỗi chưa đóng |
| VS0102 | escape không hợp lệ |
| VS0103 | ký tự lạ ngoài bảng token |
| VS0104 | literal số sai/tràn i64 |
| VS0105 | char literal không đúng 1 scalar |
| VS0106 | byte string có ký tự > 0x7F |
| VS0107 | tab (cảnh báo-lỗi; tab = 4) |
| VS0108 | nội suy `{` không đóng |
| VS0109 | thụt lề không khớp đơn vị/DEDENT sai |
| VS0110 | `@`/`^` sai ngữ cảnh |

Mọi mã có `vis explain VSxxxx`; bảng đầy đủ ở ST-02b.

## 8. Golden test (định dạng `tests/lex/*.tok`)

Mỗi dòng: `input => kind(text) ...` hoặc `input => !VSxxxx`. 60 ca tối thiểu:

```
42 => INT(42)
1_000_000 => INT(1000000)
0xff => INT(255)
1. => !VS0104
3.14 => FLOAT(3.14)
1e-9 => FLOAT(1e-9)
0..10 => INT(0) RANGE INT(10)
0..=10 => INT(0) RANGE_EQ INT(10)
1.2..3.4 => FLOAT(1.2) RANGE FLOAT(3.4)
50pct => PERCENT(50)
50 % 3 => INT(50) PUNCT(%) INT(3)
50% => !VS0103
5kb => SIZE(5,kb)
5kib => SIZE(5,kib)
2h30m => DURATION(2h30m)
250ms => DURATION(250ms)
1d => DURATION(1d)
"a\nb" => STRING(a,b)
r"C:\raw" => RAW_STRING
"a{x:>8}b" => STRING(parts)
"{{" => STRING({)
"{x" => !VS0108
"""a
b""" => MULTI_STRING
b"\x00\x01" => BYTES
b"ố" => !VS0106
p"./a.txt" => PATH
'ố' => CHAR(ố)
'ố' NFD => CHAR(ố)  (sau NFC)
'ab' => !VS0105
set{1,2} => IDENT(set) PUNCT({) ...
set {1} => IDENT(set) PUNCT({) ...
x??y => IDENT ?? IDENT
x?.y => IDENT ?. IDENT
f()? => IDENT ( ) ?
a|b => IDENT | IDENT
a|>b => IDENT |> IDENT
2**3 => INT ** INT
# c => COMMENT
## doc => DOC_COMMENT
ls -5 => WORD(ls) WORD(-5)
ls -la => WORD(ls) WORD(-la)
ls -- -x => WORD(ls) WORD(--) WORD(-x)
echo "a b" => WORD(echo) STRING(a b)
echo 'a b' => WORD(echo) RAW_STRING(a b)
$HOME => DOLLAR IDENT(HOME)
$env.PATH => DOLLAR env . IDENT(PATH)
a;b => WORD(a) PUNCT(;) WORD(b)
a & => WORD(a) PUNCT(&)
a > b => WORD(a) PUNCT(>) WORD(b)
https://x/y#frag => WORD(https://x/y#frag)
^git => CARET WORD(git)
\tif => !VS0107 INDENT
if x:  y => !VS0109
(1 +
 2) => INT + INT
x |> f => IDENT |> IDENT
```

(60 dòng bao gồm cả ca lỗi; ST-04 phải chạy đúng toàn bộ.)


---

# PHẦN D — QUEUE v2 (KT)

# QUEUE — ViFastOs v2, vòng 1

Người ghi: **Agent 0** (qua người điều phối). Người đọc/thi hành: **Agent 2** dòng OS.
Nguồn: `docs/QUYET-DINH-sau-review.md` (QD) §4, §5.1, §5.2, §10.4 và `review/PATCH-BRIEF.md` §1.
Trạng thái hợp lệ: `READY | BLOCKED | REVIEW | DONE`.
**Tệp này tự chứa**: đọc hết 5 khối dưới đây là đủ để chạy một task mà không cần mở tài liệu khác.

---

## Khối A — Luật chơi (bắt buộc)

**Vòng lặp 8 bước cho mỗi task**

1. **Đọc** `STATE.md`, `QUEUE.md`, `DECISIONS.md`, phần spec liên quan. Không đọc lại toàn repo.
2. **Lập kế hoạch nhỏ** trong `STATE.md`: các bước, tệp sẽ chạm, test sẽ thêm.
3. **Test đỏ trước**: viết test/kịch bản thể hiện yêu cầu; chạy, xác nhận đỏ **đúng lý do**.
4. **Cài đặt** theo bước nhỏ; sau mỗi bước chạy `./v2 check`.
5. **Cổng**: `./v2 gate --fast` (luôn); `./v2 gate` nếu chạm platform/user.
6. **Commit**: `[KT-##] <tóm tắt>` + kết quả cổng + `BUILD_ID`.
7. **Cập nhật** `STATE.md`, `JOURNAL/YYYY-MM-DD.md`; `vifast-agentctl done <id>`.
8. **Nén ngữ cảnh** nếu lớn, rồi quay lại bước 1 với task kế.

**Cấm (9 điều)**

1. Sửa `spec/` — trừ `KT-04a` (được ghi `spec/abi.toml`, `spec/kernel.toml`).
2. Sửa `accept()` / kỳ vọng / snapshot của harness để test xanh.
3. `#[ignore]`, `#[should_panic]` để né, tắt test, bỏ case khỏi danh sách chạy.
4. Nới ngưỡng (thời gian, seed, số ca đỏ, `MAX_IO`, `MAX_TCBS`, `MAX_OBJECTS`, `KERNEL_STATIC_BUDGET`).
5. Retry để qua cổng; flake là `T-INFRA`, ghi log, chạy lại tối đa **một** lần chỉ để phân loại.
6. Thêm `unsafe` ngoài danh sách crate đóng; thêm `cfg(target_arch)` vào kernel/runtime.
7. Đưa số đo viết tay vào tài liệu (số đo do `./v2 nightly` sinh).
8. Sửa `DECISIONS.md` của mình hoặc của dòng ViScript.
9. `pkill -f qemu-system` toàn cục — chỉ `kill <pid>` do chính task tạo.

**Bắt buộc**: hồi quy ⇒ replicate rồi **revert trước**, điều tra sau; lỗi logic phải có `.scn` đỏ trước; `cargo fmt` trước commit; header SPDX `// SPDX-License-Identifier: MIT OR Apache-2.0` cho mọi tệp mã nguồn mới; mọi số liệu mang nhãn `[V1:…]`/`[TK]`/`[GT]`/`[ƯL]`.

## Khối B — Ngân sách và điều kiện dừng (3 giờ/task)

| Tình huống | Hành động |
|---|---|
| Task vượt **3 giờ** | Dừng, ghi tiến độ + lý do vào `STATE.md`/`JOURNAL`, chuyển task kế không phụ thuộc |
| Kẹt **hai lần** cùng một ý | `BLOCKED` + câu hỏi cho Agent 0, chuyển task kế (quy tắc 2.5: đổi nhánh, không thử lần ba) |
| Spec mâu thuẫn/thiếu | `BLOCKED` kèm đề xuất cụ thể, **không** tự đổi spec |
| Cổng đỏ do lỗi không thuộc task | Mở mục lỗi trong `JOURNAL` (+ `.scn` nếu có), **không** sửa lan |
| Hết task khả dụng | Chạy `./v2 nightly`, viết báo cáo, dừng |

**Mỗi task ≤ 3 giờ. Task lớn hơn phải tách** (ví dụ KT-04 tách a/b/c/d). Không gộp hai task vào một commit.

## Khối C — Định nghĩa "xong" (DoD)

1. Tiêu chí trong bảng dưới đạt, **có bằng chứng** (đầu ra lệnh, số seed, TAP).
2. Test mới **đã từng đỏ** (dán log đỏ vào `JOURNAL`).
3. Cổng tương ứng **xanh, không retry**.
4. `STATE.md` + `JOURNAL/` cập nhật; không còn TODO không có task.

## Khối D — Lệnh cổng

```bash
./v2 check          # fmt + clippy + luật cấu trúc (nhanh, không sim)
./v2 gate --fast    # BẮT BUỘC cuối mỗi task: fmt, clippy, luật cấu trúc, L0
                    # + gen-check (từ khi có KT-05) + L1 1.000 seed (từ khi có sim)
./v2 nightly        # L1 100.000 seed, mutation, Miri, soak — không chạy trong vòng task
```

- Giai đoạn đầu (trước KT-03): cổng **chưa có** ⇒ ghi "cổng chưa có" vào `STATE.md`, không tự tạo cổng ngoài task.
- **L1 chưa bật tới KT-08** (chưa có sim). Đo thời gian L1 ngay khi sim chạy; vượt 60 s thì giảm seed của `--fast`, **không** giảm invariant.
- `./v2 nightly` và soak **không** chạy song song với build/cổng khác; tối đa 3 QEMU song song; sim dùng tối đa `số core − 1`.

## Khối E — Định dạng báo cáo

`reports/R<n>-v2.md` — 6 mục: (1) tóm tắt 5 dòng; (2) bảng task `ID | trạng thái | commit | cổng`; (3) lỗi tìm thấy (`.scn`); (4) số đo thay đổi; (5) đột biến sống; (6) `BLOCKED` + câu hỏi + đề xuất task kế.

`reports/R<n>-v2.json` — 8 khoá, **có `project`**:

```json
{
  "project": "v2",
  "round": 1,
  "head": "ca2eaf5",
  "build_id": "—",
  "tasks": [{"id": "KT-01", "status": "DONE", "commits": ["…"], "gates": ["fast"]}],
  "gates": {"fast": "PASS", "full": "—", "nightly": "—"},
  "sim": {"seeds": 0, "failures": 0},
  "coverage": {"transitions": "—", "error_paths": "—"},
  "mutants": {"killed": 0, "alive": 0},
  "perf": {},
  "blocked": []
}
```

Agent 0 đọc **JSON trước**, MD sau. JSON thiếu `project` bị trả lại.

---

## Bảng task vòng 1

| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
|---|---|---|---|---|---|
| KT-01 | tag `v1-archive` trên `ca2eaf5`; `docs/v1/KNOWN-ISSUES.md`; chuyển tài liệu v1 vào `docs/v1/` (Q10) | — | 1 h | READY | tag có message "T33a mở"; KNOWN-ISSUES đủ 7 mục QD §6, mỗi mục có ca kiểm v2 |
| KT-02 | khung `v2/` + `v2/agent/` (không ghi đè tệp đã có), LICENSE tham chiếu root, thêm `vifast-fsutil` | — | 3 h | READY | `cargo build` workspace rỗng xanh; `ls v2/agent` đủ tệp |
| KT-03 | `v2 check` + `v2 gate --fast` khung: fmt, clippy, luật cấu trúc, L0, SPDX theo loại tệp; **chưa** gen-check/L1 | KT-02 | 3 h | BLOCKED | cổng chạy; 5 luật có 5 case âm |
| KT-04a | bảng ánh xạ v1→v2 (25 crate, 58 syscall→15+method, 22 KError→19) + `spec/abi.toml` lõi (Untyped, CNode, Tcb, SchedContext, Endpoint, Notification, Reply, MessageInfo, lỗi, quy ước gọi riscv64, layout slot 48 B) | KT-02 | 3 h | READY | abi.toml parse; bảng ánh xạ đủ; `abi_version`/`min_compat` |
| KT-04b | object reserved: chỉ số + `NotSupported` (VCpu.Configure, IOSpace.Attach, VSpace khách, PowerControl…) | KT-04a | 2 h | BLOCKED | method reserved trả NotSupported; số không trùng |
| KT-04c | 20 kịch bản `.scn` viết **chỉ bằng ABI** + bảng kỳ vọng (cap copy/revoke, call/reply, reply bị xoá, suspend khi chờ, kill khi chờ, exit, 4 cap, no_reply, donation) | KT-04a | 3 h | BLOCKED | 20 tệp + 5 ca chạy trên mock CNode/Untyped tối giản trong test |
| KT-04d | gửi Agent 0 duyệt: checklist 5 mục/method + 20 kịch bản | KT-04a,b,c | 1 h | REVIEW | dòng `APPROVED <ngày> Agent 0` trong QUEUE |
| KT-05 | `vifast-gen` + gen-check + `docs/abi.md`; sinh từ `abi.toml` | KT-04a | 3 h | BLOCKED | code sinh biên dịch; sửa tay bị bắt; gen-check bật sau APPROVED |
| KT-06 | `vifast-agentctl` `--project v2\|viscript`: next/start/done/block/report, bỏ qua REVIEW | KT-02 | 2 h | BLOCKED | dùng được cho chính vòng này; JSON có `project` |
| KT-07 | K0.5: boot riscv64 QEMU qua OpenSBI nạp trực tiếp; DebugPutStr/DebugHalt; TAP 1 test; 50 boot 0 lỗi | KT-02, KT-03 | 3 h | BLOCKED | 50 boot 0 lỗi, có log |
| KT-08 | sim harness + store giả + scheduler tối thiểu 1 hart | KT-07 | 3 h | BLOCKED | 1 kịch bản end-to-end (Retype→Configure→Call/Reply) chạy |
| KT-09 | Untyped trên sim (watermark, Retype, Revoke qua CNode) | KT-08 | 3 h | BLOCKED | invariant tổng byte; preempt |
| KT-10 | CNode/MDB test-first, mô hình tối giản **trong test** | KT-09 | 3 h | BLOCKED | property test cây; Revoke |
| KT-11 | `vifast-model` Untyped/CNode | KT-10 | 2 h | BLOCKED | khớp sim 10.000 seed; chạy lại 20 ca KT-04c |
| KT-12 | `vifast-traceformat` + ring (trước khi sim dump) | KT-08 | 2 h | BLOCKED | giải mã dump từ sim; cùng định dạng QEMU |
| KT-T26 | host test `vifast-fs` + block giả + cache; MAX_IO=256; đọc RAW kiểm read-your-own-write; theo D-03 "ai chen giữa" | — | 3 h | READY | test đỏ trước; chạy seed |
| KT-R1 | báo cáo `R1-v2` phủ task đã mở + `blocked[]` | các task READY | 1 h | READY | MD + JSON theo Khối E, có `project` |

## Ghi chú vận hành

- `vifast-agentctl next` **bỏ qua** task `REVIEW` (KT-04d) — không tự làm, không tự duyệt. `next` chỉ trả task `READY` có phụ thuộc `DONE`.
- **Mỗi task ≤ 3 h; task lớn tách** (KT-04 đã tách a/b/c/d; KT-01 tách khỏi soak v1 — v1 **không** soak lại).
- `KT-04d` REVIEW chờ đúng dòng `APPROVED <YYYY-MM-DD> Agent 0`; **APPROVED mở KT-05**. Duyệt không bằng chat.
- Câu hỏi O1 (**Exit** vào ABI) đã chốt trong QD §1 Q-A: syscall **14 `Exit(code)`**.
- Nhánh chạy song song được ngay: (KT-01, KT-02, KT-04a, KT-T26) và dòng ViScript `viscript/agent/QUEUE.md` (`ST-01`, `ST-02b`, `ST-10`).
- L1 **chưa bật** tới KT-08; ghi rõ trong `STATE.md`.
- Mọi số trong bảng này lấy từ QD/brief; số nào chưa có nguồn thì ghi `[TK]`/`[ƯL]`, không tự đặt.


---

# PHẦN E — QUEUE ViScript (ST)

# QUEUE — ViScript, vòng 1

Người ghi: **Agent 0** (qua người điều phối). Người đọc/thi hành: **Agent 2** dòng ViScript.
Nguồn: `docs/QUYET-DINH-sau-review.md` (QD) §7, `review/PATCH-BRIEF.md` §2, `docs/viscript/ViScript-thiet-ke.md` §20.2.
Trạng thái hợp lệ: `READY | BLOCKED | REVIEW | DONE`.
**Tệp này tự chứa**: đọc hết 5 khối dưới đây là đủ để chạy một task mà không cần mở tài liệu khác.

---

## Khối A — Luật chơi (bắt buộc)

**Vòng lặp 8 bước cho mỗi task**

1. **Đọc** `STATE.md`, `QUEUE.md`, `DECISIONS.md`, phần SPEC liên quan. Không đọc lại toàn repo.
2. **Lập kế hoạch nhỏ** trong `STATE.md`: các bước, tệp sẽ chạm, test sẽ thêm.
3. **Test đỏ trước**: viết test/kịch bản thể hiện yêu cầu; chạy, xác nhận đỏ **đúng lý do**.
4. **Cài đặt** theo bước nhỏ; sau mỗi bước chạy `./vis-dev check`.
5. **Cổng**: `./vis-dev gate --fast` (luôn).
6. **Commit**: `[ST-##] <tóm tắt>` + kết quả cổng + `BUILD_ID`.
7. **Cập nhật** `STATE.md`, `JOURNAL/YYYY-MM-DD.md`; `vifast-agentctl --project viscript done <id>`.
8. **Nén ngữ cảnh** nếu lớn, rồi quay lại bước 1 với task kế.

**Cấm (9 điều)**

1. Sửa `SPEC-lex.md` / `SPEC.md` ngoài phạm vi `ST-02a`/`ST-02b`.
2. Sửa `accept()` / kỳ vọng / snapshot golden để test xanh.
3. `#[ignore]`, tắt test, bỏ case khỏi danh sách chạy, dùng `...` trong fixture (QD §7.4 cấm).
4. Nới ngưỡng (thời gian fuzz, số golden, số fixture).
5. Retry để qua cổng; flake là `T-INFRA`, ghi log, chạy lại tối đa **một** lần chỉ để phân loại.
6. Đổi ngữ nghĩa ngôn ngữ (cú pháp, quy tắc kiểu) mà không qua Agent 0 — bắt buộc `BLOCKED` + đề xuất + ví dụ (VD-08).
7. Thêm `unsafe` mà không có case âm; vượt **1.500 dòng/tệp**.
8. Đưa số đo viết tay vào tài liệu (số đo do `./vis-dev nightly` sinh).
9. Sửa `DECISIONS.md` hoặc QUEUE của dòng OS.

**Bắt buộc**: hồi quy ⇒ **revert trước**, điều tra sau; lỗi từ fuzz/vi sai ⇒ thu nhỏ, ghi `tests/regress/`, test đỏ trước rồi mới sửa; `cargo fmt` trước commit; SPDX theo loại tệp (`#` cho `.vis .vish .vidl .toml`, `//` cho Rust); `vis-host-vifast` **không** nằm trong `viscript/`.

## Khối B — Ngân sách và điều kiện dừng (3 giờ/task)

| Tình huống | Hành động |
|---|---|
| Task vượt **3 giờ** (ST-02a: 3 giờ / **2 phiên**) | Dừng, ghi tiến độ + lý do, chuyển task kế không phụ thuộc |
| Kẹt **hai lần** cùng một ý | `BLOCKED` + câu hỏi cho Agent 0, đổi nhánh (quy tắc 2.5) |
| SPEC mâu thuẫn/thiếu | `BLOCKED` kèm đề xuất + ví dụ cụ thể, **không** tự đổi SPEC |
| Cổng đỏ do lỗi không thuộc task | Mục lỗi trong `JOURNAL`, **không** sửa lan |
| Hết task khả dụng | Chạy `./vis-dev nightly`, viết báo cáo, dừng |

**Mỗi task ≤ 3 giờ. Task lớn hơn phải tách.**

## Khối C — Định nghĩa "xong" (DoD)

1. Tiêu chí trong bảng dưới đạt, **có bằng chứng** (đầu ra lệnh, số golden, TAP, snapshot).
2. Test mới **đã từng đỏ** (dán log đỏ vào `JOURNAL`).
3. Cổng tương ứng **xanh, không retry**.
4. `STATE.md` + `JOURNAL/` cập nhật; không còn TODO không có task.

## Khối D — Lệnh cổng

```bash
./vis-dev check          # fmt + clippy + luật cấu trúc (≤1.500 dòng/tệp, unsafe có case âm, SPDX)
./vis-dev gate --fast    # BẮT BUỘC cuối mỗi task: check + L0 + golden hiện có
./vis-dev nightly        # fuzz dài, snapshot toàn bộ, benchmark — không chạy trong vòng task
```

- Dòng ViScript **không có** công cụ cho tới khi `VT01`/`ST-01` dựng khung: giai đoạn đầu ghi "cổng chưa có" vào `STATE.md`.
- Fuzz 10 phút là **một phần tiêu chí** của ST-04, không phải cổng thường ngày.

## Khối E — Định dạng báo cáo

`reports/R<n>-viscript.md` — 6 mục: (1) tóm tắt 5 dòng; (2) bảng task `ID | trạng thái | commit | cổng`; (3) lỗi tìm thấy (fixture/`.regress`); (4) số đo thay đổi; (5) golden/fixture đã thêm; (6) `BLOCKED` + câu hỏi + đề xuất task kế.

`reports/R<n>-viscript.json` — cùng schema với dòng OS, **có `project: "viscript"`**:

```json
{
  "project": "viscript",
  "round": 1,
  "head": "ca2eaf5",
  "build_id": "—",
  "tasks": [{"id": "ST-01", "status": "DONE", "commits": ["…"], "gates": ["fast"]}],
  "gates": {"fast": "PASS", "full": "—", "nightly": "—"},
  "sim": {"seeds": 0, "failures": 0},
  "coverage": {"tokens": "—", "fixtures": "—"},
  "mutants": {"killed": 0, "alive": 0},
  "perf": {},
  "blocked": []
}
```

Agent 0 đọc **JSON trước**, MD sau. JSON thiếu `project` bị trả lại.

---

## Bảng task vòng 1

| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
|---|---|---|---|---|---|
| ST-01 | workspace `viscript/` + `viscript/agent/` (không ghi đè) + SPDX theo loại tệp + cổng 1.500 dòng/unsafe có case âm + `vis-host-vifast` **không** nằm trong `viscript/` | — | 3 h | READY | cổng chạy; 3 case âm |
| ST-02a | **Duyệt + hoàn thiện** `SPEC-lex.md` + Phụ lục A + bảng ưu tiên (Agent 1 đã soạn) + 60 golden token + 30 golden biểu thức | ST-01 | 3 h (2 phiên) | REVIEW | mọi khối mã parse được; Agent 0 duyệt |
| ST-02b | sổ mã lỗi VS + 4 chữ số + quy tắc cấp | ST-01 | 2 h | READY | bảng mã; `vis explain` được |
| ST-03 | `vis-diag`: cột theo Unicode scalar, render, bảng vi/en/ja | ST-01 | 3 h | BLOCKED | snapshot 20 ca, có tiếng Việt/emoji |
| ST-04 | lexer theo SPEC-lex (sau duyệt) | ST-02a | 3 h | BLOCKED | bảng token vàng + fuzz 10 phút + NFC/NFD |
| ST-05 | parser + CST lossless | ST-04 | 3 h | BLOCKED | 100 % fixture tài liệu parse; mỗi tệp lỗi đúng 1 lỗi đầu |
| ST-06 | CmdCST + golden từng hàng bảng 9.4 + 10 ca mơ hồ | ST-05 | 3 h | BLOCKED | golden AST/CST |
| ST-07 | `vis-fmt` lossless, idempotent, sửa tab | ST-05 | 3 h | BLOCKED | vòng tròn parse→fmt→parse |
| ST-08 | runner `--check` cú pháp (chưa interpreter) | ST-05 | 2 h | BLOCKED | TAP; chạy fixture |
| ST-09 | fixture theo QD §7.4 (1/khối mã + 1/quyết định; cấm `...`) | ST-06,07,08 | 2 h | BLOCKED | xanh |
| ST-10 | báo cáo `R1-viscript` | các task | 1 h | READY | MD + JSON có `project` |

## Ghi chú vận hành

- **ST-02a = REVIEW**: chờ đúng dòng `APPROVED <YYYY-MM-DD> Agent 0` trong tệp này. **Chặn ST-04..ST-09.** **Không** chặn `ST-02b`, `ST-03`, `ST-10`.
- `vifast-agentctl --project viscript next` **bỏ qua** task `REVIEW`; `next` chỉ trả task `READY` có phụ thuộc `DONE`.
- **Mỗi task ≤ 3 h; task lớn tách.** ST-02a tách khỏi ST-02b để phần sổ mã lỗi không phải chờ duyệt.
- Parse chế độ lệnh **hai pha** (QD §7.2 + D §6.D): `vis-syntax` sinh `CmdCST` → `cmd_lower` (vis-types, dùng bảng chữ ký) → **HIR**. "Hai chế độ ra cùng AST" đọc là **cùng HIR**; `cmd_lower` là task vòng 2. Vòng 1 dừng ở lexer → parser/CST → `CmdCST` → `vis-fmt` → runner `--check` cú pháp (chưa interpreter, chưa kiểm kiểu).
- `ST-09` cấm `...` trong fixture: mọi khối mã trong tài liệu phải parse được, ngoại lệ phải khai báo trong SPEC.
- Ưu tiên hai dòng: **OS trước**; dòng này chỉ lấy task khi `v2/agent/QUEUE.md` không còn task `READY` (D2-20). Ngoại lệ: `ST-02a` được chạy song song vì chỉ là tài liệu.
- Số nào chưa có nguồn trong QD/tài liệu ViScript thì ghi `[TK]`/`[ƯL]`, không tự đặt.


---

# PHẦN F — DECISIONS v2 (D-01..08 + D2-01..24)

# DECISIONS — ViFastOs v2

Người ghi: **Agent 0**. Agent 2 **không** sửa tệp này; cần đổi thì ghi `BLOCKED.md` + đề xuất.
Nguồn: `docs/QUYET-DINH-sau-review.md` (QD) §2.3, §4, §8 và `review/src/ViFastOs-dossier.md` §19.1.
Không gian tên đã chốt (QD §2.1): quyết định v1 giữ `D-01..D-08` (**chỉ áp v1**); quyết định v2 là **`D2-01..D2-24`**. Số cũ `D-09..D-27` **hết hiệu lực**, không dùng lại.

---

## Phần I — D-01..D-08: quyết định v1 (**chỉ áp v1**)

Chép từ dossier §19.1 ("Tám quyết định chốt cho đợt chống flake"). Trạng thái trong v2 theo QD §2.3.

| Mã | Nội dung | Lý do (v1) | Trạng thái trong v2 |
|---|---|---|---|
| **D-01** | Giữ gate **mềm**; `--strict` chỉ dùng khi **đo** | Máy đo chưa ổn định; bật sẽ đỏ vì flake hạ tầng | Thay bằng **D2-07** (không retry) cho v2; D-01 chỉ còn hiệu lực cho v1 |
| **D-02** | **Đóng băng tính năng**, chỉ sửa lỗi + đo | Tránh thêm biến khi đang săn lỗi hiếm | **Hết hiệu lực** từ khi v1 lưu trữ (v2 thay thế v1, không mở tính năng trên v1) |
| **D-03** | T26: đánh **"ai/cái gì chen giữa"**, **không** đánh nhánh `fsck` (đã đóng) | fsck không phải nguyên nhân (đã chứng minh) | **Chuyển sang v2** dưới dạng **KT-T26** (QD §6) |
| **D-04** | Đo ở `MAX_IO=256` (**chỉ nhánh đo**); không bật ở gate/phát hành | 256 làm lộ race; 128 giữ cổng xanh | Giữ cho **KT-T26** (cấu hình tái hiện) |
| **D-05** | Teardown tối đa **2 chỗ**, mỗi chỗ **một vòng riêng**, hồi quy ⇒ **revert** | Gộp nhiều chỗ đã làm hỏng 0/3 cả ba arch | Giữ **tinh thần** trong v2: finalise cài theo nhóm, mỗi task tối đa 2 loại object, mỗi loại có test riêng |
| **D-06** | Soak nền ~600–1000 boot x86_64 trên máy `ps` | Cần mẫu lớn; máy rảnh | Giữ: mọi soak v2 chạy trên máy **ps**, N theo QD §5.3 |
| **D-07** | **Không** đụng nợ kiến trúc (không tách `lib.rs`) | Ưu tiên đóng lỗi trước | **Hết đối tượng** (v2 viết mới) — ghi rõ để không ai trích lại |
| **D-08** | **Không** đụng đồ hoạ / ViFastKey | Ngoài phạm vi đợt | **Còn hiệu lực tới K6**; G-line và ViFastKey chỉ mở sau K6 (= D2-23) |

Ghi chú: D11 (không sửa mò) → **D2-02**; D18 (hồi quy thì revert) → **D2-03**. Quy tắc 2.5 giữ nguyên văn: hai phiên kẹt cùng một ý thì **đổi nhánh**, không thử lần ba.

---

## Phần II — D2-01..D2-24: quyết định v2 (chép nguyên văn QD §8)

- **D2-01** v1 lưu trữ ở `v1-archive`, sổ lỗi mang sang (mục 6).
- **D2-02** không sửa mò (D11 cũ).
- **D2-03** hồi quy thì revert (D18 cũ).
- **D2-04** core không chứa FS/fd/block/console/args/demo; thay đổi ABI qua Agent 0.
- **D2-05** commit chạm `kernel/`, `lib/` phải xanh `--fast`; lỗi logic tái hiện trên sim, kèm `.scn` đỏ trước.
- **D2-06** luật cấu trúc là cổng cứng.
- **D2-07** không retry ở mọi cổng; flake là T-INFRA.
- **D2-08** test kernel không nằm trong boot sản phẩm.
- **D2-09** trace luôn bật, không có cờ tắt.
- **D2-10** chỉ riscv64 tới K6.
- **D2-11** một nguồn sự thật trong `spec/`.
- **D2-12** đồng thời theo S1..S4 theo số đo; đọc trường công bố của hart khác được phép.
- **D2-13** kịch bản `.scn` chạy cả sim và QEMU.
- **D2-14** object trong Untyped qua objmem (API closure, Miri từ đầu).
- **D2-15** ABI đóng băng 5 thứ + version + golden (mục 4 QD).
- **D2-16** MCS: donation từ K1b, thuật toán refill sau K6.
- **D2-17** unsafe chỉ trong danh sách crate đóng.
- **D2-18** ứng dụng hệ thống viết bằng Rust std, crate thuần Rust; libc, wasm để sau.
- **D2-19** ViScript là ngôn ngữ script và shell chính; Linux qua máy ảo.
- **D2-20** OS trước, ViScript khi không còn task OS READY.
- **D2-21** giấy phép MIT OR Apache-2.0, kiểm bằng `cargo-deny` (allowlist mục 9.3 QD), `THIRD-PARTY.md` sinh tự động, license nhúng vào ảnh, không tạo NOTICE.
- **D2-22** duyệt spec bằng dòng APPROVED; Agent 2 soạn nháp spec ở trạng thái REVIEW, sau duyệt chỉ Agent 0 sửa.
- **D2-23** D-08 còn hiệu lực tới K6.
- **D2-24** mọi soak trên máy ps, N theo $n \ge 3/p$ (p mục tiêu 0,3 % ⇒ **N = 1.000**, song song 3 luồng).

---

## Phần III — Duyệt

- **T04 cũ = KT-04d; APPROVED mở KT-05.**
- Mẫu: `APPROVED <YYYY-MM-DD> Agent 0`, ghi ngay dưới mục KT-04d trong `v2/agent/QUEUE.md`; nếu quyết định làm đổi nội dung ABI thì thêm một mục D2 mới ở tệp này. Không duyệt bằng chat. Không tự suy ra "coi như đã duyệt".
- Agent 2 soạn nháp ở `REVIEW`, sau duyệt **chỉ Agent 0** sửa `spec/` (D2-22).
- `ST-02a` (dòng ViScript) duyệt theo cùng mẫu trong `viscript/agent/QUEUE.md`; quyết định tương ứng ghi vào `viscript/agent/DECISIONS.md` (VD-).

## Phần IV — Bảng nguồn cho các con số đang có hiệu lực

| Số | Giá trị | Nguồn |
|---|---|---|
| slot cap | **48 B** `[TK]` (sửa 32 B) | QD §3.9 |
| `MAX_HARTS` | mặc định **8**, trần cấu hình **64** `[TK]` (sửa 256) | QD §3.15 |
| `vifast-core` | ≤ **25.000** dòng `[ƯL]` (sửa 15.000); tệp ≤ 1.500 dòng | QD §3.15 |
| `.data + .bss` | ≤ **256 KiB** `[TK]` `[GT]`, đo ở K0.5 | QD §3.5, §9 Q8 |
| mã lỗi kernel | **19** (bỏ `Restart`); syscall **0..13 + 14 Exit**; dải 15..31 reserved | QD §4 |
| soak v2 | **N = 1.000**, máy **ps**, 3 luồng, $n \ge 3/p$, p = 0,3 % | QD §5.3 |
| L1 trong `--fast` | **1.000** seed (nightly **100.000**) | QD §5.2 |
| số kịch bản ABI | **20** `.scn` | QD §4 |


---

# PHẦN G — DECISIONS ViScript (VD-01..22)

# DECISIONS — ViScript

Người ghi: **Agent 0**. Agent 2 **không** sửa tệp này; cần đổi thì ghi `BLOCKED.md` + đề xuất.
Nguồn: `docs/viscript/ViScript-thiet-ke.md` §21–§22 và `docs/QUYET-DINH-sau-review.md` (QD) §7.

---

## Phần I — VD-01..VD-10 (chép từ tài liệu ViScript §22)

| Mã | Nội dung |
|---|---|
| **VD-01** | ViScript có kiểu tĩnh bắt buộc, không `any`, không null |
| **VD-02** | Một ngôn ngữ, hai chế độ cú pháp, cùng AST/kiểm kiểu/VM; ViShell là chương trình riêng |
| **VD-03** | Ngữ nghĩa giá trị + RC + copy-on-write; không GC truy vết; chu trình chỉ qua `ref`, phá bằng `weak` |
| **VD-04** | Lõi (syntax, types, bytecode, vm) không phụ thuộc hệ điều hành, build được wasm32 |
| **VD-05** | Mọi truy cập hệ thống qua capability; không hàm toàn cục chạm hệ thống |
| **VD-06** | Interpreter cây là mô hình tham chiếu, giữ mãi để kiểm vi sai VM |
| **VD-07** | Không JIT; không `eval` chuỗi trong chế độ quyền hạn chế |
| **VD-08** | Thay đổi SPEC chỉ qua Agent 0; mỗi tính năng mới phải có ví dụ + test trước |
| **VD-09** | Từ khoá tiếng Anh, định danh Unicode (NFC), thông báo lỗi vi/en/ja |
| **VD-10** | Giấy phép MIT OR Apache-2.0; phụ thuộc phải tương thích |

## Phần II — Q1..Q8 (tóm tắt 1 dòng/mục, kèm chỗ QD §7 đã sửa)

| Mã | Chốt | Ghi chú sau QD §7 |
|---|---|---|
| Q1 | Từ khoá **tiếng Anh**; định danh Unicode cho phép tiếng Việt; thông báo lỗi có tiếng Việt | giữ nguyên |
| Q2 | `int / int` cho `int`; có lint khi chia hai literal | giữ nguyên |
| Q3 | Thụt lề **4 dấu cách bắt buộc, cấm tab** | **sửa bởi VD-12**: đầu vào nhận thụt lề nhất quán bất kỳ (đơn vị suy từ cấp đầu), tab = 4 + VS0107, `vis fmt` luôn ra 4 |
| Q4 | Tên **ViScript** (`.vis`, lệnh `vis`), **ViShell** (`.vish`, lệnh `vish`) | giữ nguyên |
| Q5 | `xs[i]` ngoài phạm vi **panic**; `xs.get(i)` trả `T?` | **thêm bởi VD-18**: có `xs[i]?` trả `T?`; `xs[-1]` panic có thông điệp |
| Q6 | Giấy phép **MIT OR Apache-2.0** | giữ nguyên (= VD-10, D2-21) |
| Q7 | `decimal` **không** trong lõi, là thư viện | giữ nguyên |
| Q8 | Toán tử bit là **hàm** (`bit_or`…), không ký hiệu, để giải phóng `\|` cho pipe | **sửa bởi VD-15**: `\|` **không bao giờ** là toán tử nhị phân trong biểu thức |

## Phần III — VD-11..VD-22: chốt bổ sung từ QD §7 (mỗi dòng một mục)

- **VD-11** Chuẩn hoá: **toàn bộ tệp nguồn được chuẩn hoá NFC trước khi lex** — 'ố' gõ kiểu nào cũng là một scalar.
- **VD-12** Thụt lề: đầu vào nhận thụt lề nhất quán bất kỳ, đơn vị suy từ cấp đầu; đầu ra `vis fmt` luôn **4**. Tab: lexer báo cảnh báo-lỗi **VS0107**, coi tab = 4, **vẫn lex tiếp**; `vis check` coi là lỗi, `vis fmt` sửa.
- **VD-13** Phần trăm: **bỏ literal `50%`**, dùng `50pct`; `%` chỉ là modulo; job spec dùng `job 1`.
- **VD-14** Chú thích: script — `#` ngoài chuỗi luôn là chú thích; lệnh — `#` chỉ là chú thích khi **đứng đầu từ** (sau khoảng trắng hoặc đầu dòng), nên `https://x/y#frag` giữ nguyên.
- **VD-15** `|` **không bao giờ là toán tử nhị phân trong biểu thức**. Pattern: or-pattern dùng `|`; lệnh: pipe ở **độ sâu ngoặc 0**; `|>` là pipe script. (Sửa Q8.)
- **VD-16** Bảng ưu tiên chốt (cao → thấp): postfix (`. ?. () [] ? with`) → `**` → `-` một ngôi → `* / %` → `+ -` → `.. ..=` → `??` → so sánh (không kết hợp) → `not` → `and` → `or` → `|>`. `**` kết hợp **phải**, cao hơn `*`; mũ âm với int panic; có `checked_pow`.
- **VD-17** Float: NaN — `==` trả false; float **không** có `Eq/Ord/Hash`; kiểu chứa float không derive được `Eq/Hash`; dùng `total_cmp`. `percent` so sánh được với float theo tỉ lệ (`50pct == 0.5`); `ps.cpu` có kiểu `percent`.
- **VD-18** Chuỗi: **cấm `s[i]`**; dùng `.chars() .graphemes() .bytes() .byte_at(i)?`; có `byte_len()`, `char_count()`, `grapheme_count()`; so sánh theo dãy scalar. Index: thêm `xs[i]?` trả `T?`; `xs[-1]` panic có thông điệp.
- **VD-19** Optional: tự nâng `T → T?` ở gán, trả, đối số; `T??` là lỗi kiểu; `?` dùng được trên `T?` trong hàm trả `U?`; pattern `none`, `some(x)`, `ok(v)`, `err(e)`.
- **VD-20** Lỗi: `trait Error: Show { message, source, span }`; `AnyError` là kiểu xoá; mọi `E: Error` tự chuyển sang `AnyError` qua `?`; `main -> Result[(), AnyError]`; tên phía plugin Rust là `vis::Error`. Mã thoát: 0 ok, 1 lỗi từ main, 2 lỗi dùng/kiểu, 101 panic, 126 không chạy được, 127 không tìm thấy, 130 bị huỷ.
- **VD-21** Chế độ lệnh (QD §7.2): metachar là khoảng trắng, `( ) [ ] { } ; & $ ' " |`, `#` ở đầu từ; `"..."` có nội suy, `'...'` thô; `$x` biến phiên, `$env.NAME` môi trường; `;` tách pipeline; `&` nền chỉ khi là token cuối có khoảng trắng phía trước; `--` kết thúc cờ; `-5` là số nếu tham số kiểu số, ngược lại là cờ; `.vish`/REPL: `shell_item := use | let | fn | cmd | pipeline`.
- **VD-22** Parse chế độ lệnh **hai pha**: `vis-syntax` sinh `CmdCST` (từ trần chưa định kiểu) → `cmd_lower` trong `vis-types` dùng bảng chữ ký → **HIR**. Tiêu chí "cùng AST" đọc thành **"cùng HIR"**; vòng 1 chỉ làm `CmdCST` + golden, `cmd_lower` là task vòng 2.

## Phần IV — Duyệt

- Mẫu: `APPROVED <YYYY-MM-DD> Agent 0`, ghi ngay dưới mục **ST-02a** trong `viscript/agent/QUEUE.md`; nếu duyệt làm đổi nội dung SPEC thì thêm một mục VD mới ở tệp này. Không duyệt bằng chat.
- `ST-02a` = REVIEW: Agent 2 soạn nháp (`SPEC-lex.md` + EBNF tự chứa + bảng ưu tiên + 60 golden token + 30 golden biểu thức); **chỉ Agent 0** sửa sau duyệt (D2-22, VD-08).
- Cổng "mọi khối mã trong tài liệu parse được" là điều kiện duyệt; ngoại lệ phải khai báo trong SPEC (QD §7.4).

## Phần V — Bảng nguồn cho các con số đang có hiệu lực

| Số | Giá trị | Nguồn |
|---|---|---|
| golden token (ST-02a) | **60** | QD §7.5 |
| golden biểu thức (ST-02a) | **30** | QD §7.5 |
| mã lỗi | `VS` + **4 chữ số**, quy tắc cấp (ST-02b) | QD §7.5 |
| thụt lề ra của `vis fmt` | **4** | VD-12 |
| giới hạn tệp | **1.500** dòng/tệp | QD §3.15 áp cho cả hai dòng |
| ca mơ hồ chế độ lệnh (ST-06) | **10** + mỗi hàng bảng 9.4 | QD §7.5, B §10.2 |
| fuzz lexer | **10 phút** | QD §7.5 (ST-04) |
| snapshot `vis-diag` (ST-03) | **20** ca | QD §7.5 |


---

# PHẦN H — KNOWN-ISSUES v1

# KNOWN-ISSUES — sổ lỗi v1 mang sang v2

Trạng thái: **lưu trữ** (đi cùng tag `v1-archive` trên `ca2eaf5`). Cập nhật: 2026-11-12.
Nguồn: `docs/QUYET-DINH-sau-review.md` (QD) §5.4, §6, §9 Q3–Q6 và `review/src/ViFastOs-dossier.md` §8.6, §15.1–15.8.
Quy ước: v1 **không** sửa nữa và **không** soak lại; mỗi lỗi dưới đây chỉ đóng được bằng **cơ chế + test tất định** ở v2 (dossier §15.10: cơ chế viết ra · test tất định · N ≥ 600 cùng `BUILD_ID`/máy/cấu hình · không đổi ngưỡng/`accept()` · một dòng trong nhật ký quyết định).
Nhãn: `[V1:ĐO]` giữ nguyên độ tin cậy gốc từ dossier; ca kiểm v2 là `[TK]` trừ khi ghi khác.

| # | Lỗi v1 | Mô tả (1–2 dòng) | Tiêu chí đóng gốc (dossier) | Ca kiểm tương ứng trong v2 | Trạng thái |
|---|---|---|---|---|---|
| 1 | **T33a** — treo x86_64 sau `shell>` `[V1:ĐO]` | ~0,3–0,5 % lần boot x86_64 (TCG, `-smp 2`, soak song song 3 luồng) đứng sau `shell>`, không in `PREEMPT OK`. Gốc đã lộ một nửa: worker C thấy `TIME MONOTONIC BAD` → `panic!` → root chờ mãi | §15.1: **600 boot** x86_64 song song 3 luồng, `VIFAST_NO_RETRY=1`, **0 treo**, **0** `TIME MONOTONIC BAD`, **0** `ROOT PANIC`, **và** cơ chế được viết thành test host (§15.10) | Kịch bản sim **"đơn điệu sau migrate"** + conformance chéo hart; v2 không có trạng thái reset (QD §6) | **Mang sang v2** (Q3: không sửa trên v1; Q4: thành kịch bản đơn điệu đồng hồ) |
| 2 | **T26** — FS vi phạm read-your-own-write `[V1:ĐO]` | Ở `MAX_IO=256`, `smp2`: **10/50 = 20 %** boot đỏ; entry thư mục ghi xong, liệt kê ngay sau không thấy, lần sau thấy. `-accel tcg,thread=single` vẫn 1/3 ⇒ race **logic**, không phải race bộ nhớ host | §15.2: có phép đo **tất định** tái hiện vi phạm; bản vá làm nó xanh **và** tỉ lệ boot đỏ ở `MAX_IO=256` về **0/50**; sau đó mới bàn nâng `MAX_IO` về 256 | **KT-T26**: host test `vifast-fs` + block giả + cache, đan xen theo seed, `MAX_IO=256`, kiểm read-your-own-write bằng đọc **RAW**; đánh **"ai chen giữa"** theo D-03 | **Mang sang v2** (Q5; D-04 giữ cấu hình tái hiện) |
| 3 | **T31** — PAGER2 nhận message lặp `[V1:ĐO]` | `PAGER EVICT OK` in **hai lần** ⇒ nhánh xử lý chạy ≥ 3 lần ⇒ có message bị giao/xử lý lặp. r309 đã thêm bảng `pager2_step(seen)` + marker `PAGER2 DUP`, **chưa** tìm đường giao lặp | §15.3 + §15.10: tìm được đường giao lặp và có test tất định canh cơ chế (đếm `USER FAULT … addr=0x1001001000`: 2 lần ⇒ replay phía server; 3 lần ⇒ phía client) | Kịch bản sim **pager 4 client** + checker "giao đúng một lần" + **đột biến "phát lại Call khi Resume"** (QD §6) | **Mang sang v2** |
| 4 | **T-IPC** — lỗi cổng đo `[V1:ĐO]` | "Boot đỏ" r307 thực chất là cổng `test-exit` thoát đúng lúc client đang chạy user. r308 vá: hoãn exit một tick khi task còn `Running` ⇒ 0/80 boot thiếu marker. Đã đóng, giữ làm bài học | §15.4: 0/80 boot thiếu marker (đã đạt ở r308) + bài học "kiểm đường thoát trước khi nghi kernel" | **Luật**: mọi harness mới phải có **case âm** chứng minh nó bắt được lỗi trước khi dùng để tuyên bố (QD §6) — hiện thực ở `harness-v2` (`FORBIDDEN`, `EXACT_ONE`, bộ case âm cho chính harness) | **Đã đóng ở v1**; bài học **mang sang v2** dưới dạng luật harness |
| 5 | **x86 AP→BSP + PCID** — TLB cũ `[V1:CODE]` | Lỗi **đúng đắn**: sau unmap, TLB của BSP có thể còn cũ ⇒ tiến trình đọc được trang đã thu hồi. Không lộ trên TCG mặc định nên mọi cổng vẫn xanh | §15.6: có test (host hoặc QEMU `-cpu max`/KVM) chứng minh TLB cũ gây **đọc sai**; bản vá làm nó xanh; **hoặc** tắt PCID có kiểm soát + ghi lý do | Đột biến **"bỏ xử lý hộp thư khi chờ ack"** + conformance TLB khi tới x86 (QD §6); x86 chỉ làm sau K6 (D2-10) | **Mang sang v2**, mở ở mốc K5' (x86_64) |
| 6 | **Audit P1-5** — `Ready` mà không trong runqueue `[V1:DOC]` | `enqueue` trả sớm ⇒ TCB mang trạng thái `Ready` nhưng **không** nằm trong runqueue (liên quan K37–K40). Audit ghi **OPEN** | §15.8: đọc lại `enqueue`, đóng theo invariant "ở trong hàng X khi và chỉ khi trạng thái mang X"; §15.10 cho điều kiện "đã sửa" | Invariant **hàng đợi khớp trạng thái** (đã có trong thiết kế) + kịch bản riêng (QD §6) | **Mang sang v2** dưới dạng invariant + kịch bản |
| 7 | **Audit Q1-A** — kênh wake lệch 3 port `[V1:DOC]` | Trong `Mailbox`, kênh đánh thức chưa thống nhất 3 port: riscv64 có doorbell riêng cho reschedule, arm/x86 dùng mailbox chung. Nợ từ r293 | §15.8: thống nhất sau r293–r298; đóng khi một cơ chế chung cho cả 3 port | **`vifast-sync` dùng chung cho mọi arch** (QD §6); v2 chỉ riscv64 tới K6 nên ca kiểm chạy ở sim + riscv64 trước | **Mang sang v2** (thiết kế lại hộp thư theo hart) |

## Ghi chú ngoài 7 mục

- **P1-6** (aarch64 SGI 2 chưa enable trong GICD) và **P1-7** (GDT/TSS per-hart: bảng round92 ghi `CLOSED (r261)`, mục "việc tiếp" ghi `OPEN (P1)` — **mâu thuẫn nội bộ**) vẫn **OPEN** theo dossier §15.8; QD §5.4 yêu cầu ghi kèm nhưng §6 chỉ chốt 7 ca kiểm, nên hai mục này ở lại v1 dưới dạng ghi chú, **không** có ca kiểm v2 riêng. P1-6/P1-7 chỉ có nghĩa khi mở aarch64/x86_64 (sau K6).
- **`slot_arg(..).unwrap_or(0)` ở 53 chỗ** (§15.8, rủi ro thấp): không mang sang v2 — v2 viết mới, không có mẫu này.
- **T33b** (1 ca riscv64 "fault + no runnable" trong soak 100×3, r290): chưa tái hiện, không đủ dữ kiện để thành mục — nếu T33a đóng mà còn ca này thì mở mục mới (dossier §15.9).
- **D-02 hết hiệu lực** từ khi lưu trữ; **D-01/D-04/D-06** chỉ còn nghĩa lịch sử, đã thay bằng D2-07/D2-24 (xem `v2/agent/DECISIONS.md`).
- Không tuyên bố "v1 đã sạch": 5 mục đầu **chưa đóng ở v1**, chỉ chuyển thành ca kiểm ở v2.


---

# PHẦN I — CHECK

# Kiểm tra hậu vá (2026-11-12)

## docs/v2
- D2 rows: 24/24
- KT rows: 17/17
- syscall rows: 15/15
- Exit row: 1
- 19 mã lỗi: 1
- slot 48 B: 1
- K0.5: 8, K3.5: 3
- old T tasks: 0
- Q1–Q12 phụ lục: 3
- sổ lỗi phụ lục: 2
- dòng:     1131

## docs/viscript
- ST rows: 11/11
- VD rows: 22/22
- Q rows: 8/8
- luật EBNF: 122
- 50pct: 8
- CmdCST: 13
- SPEC-lex refs: 9
- dòng:     1457

## SPEC-lex
- tồn tại,      180 dòng, 29 mã lỗi

## điều khiển
- QUEUE v2 KT: 17/17
- QUEUE ST: 11/11
- DECISIONS D2: 26/24
- DECISIONS D-0x: 8/8
- KNOWN-ISSUES rows: 7/7
- docs/v1 files:      105
- AGENTS files:        3/3
- docs/README 2a: 1


---

# PHẦN J — AGENTS (luật chơi)

# AGENTS.md — luật làm việc trong repo ViFastOs

Cập nhật: 2026-11-12 (vòng vá sau review). Nguồn sự thật của vòng vá:
[`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md) (gọi tắt **QD**).
Tệp này áp cho **mọi** agent làm việc trong repo. Hai dòng sản phẩm có bản rút gọn:
[`v2/AGENTS.md`](v2/AGENTS.md) và [`viscript/AGENTS.md`](viscript/AGENTS.md).

## 1. Thứ tự nguồn sự thật

Khi hai nguồn nói khác nhau, **nguồn đứng trước thắng**:

1. **Code + harness** — thứ chạy được và đo được: `crates/`, `v2/`, `viscript/`, sim, tệp `.scn`, TAP, log cổng.
2. **`spec/`** — `spec/abi.toml`, `spec/kernel.toml`, `docs/viscript/SPEC-lex.md`: một nguồn sự thật duy nhất cho ABI và ngữ nghĩa ngôn ngữ.
3. **`v2/agent/DECISIONS.md`** (D-01..D-08, D2-01..D2-24) và **`viscript/agent/DECISIONS.md`** (VD-01..VD-22).
4. **Tài liệu thiết kế** — [`docs/v2/ViFastOs-v2-thiet-ke.md`](docs/v2/ViFastOs-v2-thiet-ke.md), [`docs/viscript/ViScript-thiet-ke.md`](docs/viscript/ViScript-thiet-ke.md).

QD mạnh hơn hai tài liệu thiết kế. Không có nguồn nào cho phép **nới ngưỡng để qua cổng**.

Nhãn số liệu (bắt buộc, QD §2.2): `[V1:CODE]` `[V1:ĐO]` `[V1:DOC]` (từ dossier, giữ độ tin cậy gốc), `[TK]` thiết kế mới (**mặc định** cho dòng không nhãn), `[GT]` giả thuyết, `[ƯL]` số ước lượng cần đo. Hằng số thiết kế là `[TK]`, **không** phải `[ƯL]`. Số đo chỉ do `v2 nightly` / `vis-dev nightly` sinh; tài liệu không chứa số đo viết tay (QD §9 Q12).

## 2. Bản đồ tài liệu

| Nhóm | Đường dẫn | Trạng thái |
|---|---|---|
| Quyết định vòng vá | `docs/QUYET-DINH-sau-review.md` | nguồn sự thật, **cấm sửa** |
| Thiết kế v2 | `docs/v2/ViFastOs-v2-thiet-ke.md` | cấm sửa (chỉ Agent 0 / SA-A theo QD) |
| Thiết kế ViScript | `docs/viscript/ViScript-thiet-ke.md`, `docs/viscript/SPEC-lex.md` | cấm sửa (chỉ Agent 0 / SA-B, SA-C) |
| Điều khiển OS | `v2/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md`, `JOURNAL/`, `reports/` | xem bảng chủ sở hữu §3 |
| Điều khiển ViScript | `viscript/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md`, `JOURNAL/`, `reports/` | xem bảng chủ sở hữu §3 |
| Lưu trữ v1 | `docs/v1/` (101 `agent2-report-*.md`, `overnight-log.md`, `decisions/`, `KNOWN-ISSUES.md`) | **đọc-only** |

## 3. Ai ghi tệp nào

| Tệp | Người ghi | Ghi chú |
|---|---|---|
| `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `viscript/agent/QUEUE.md`, `viscript/agent/DECISIONS.md` | **Agent 0** (qua người điều phối) | Agent 2 **không** sửa; cần đổi thì BLOCKED + đề xuất |
| `v2/agent/STATE.md`, `v2/agent/BLOCKED.md`, `v2/agent/JOURNAL/`, `v2/agent/reports/` | **Agent 2** dòng OS | STATE cập nhật **sau mỗi bước** |
| `viscript/agent/STATE.md`, `viscript/agent/BLOCKED.md`, `viscript/agent/JOURNAL/`, `viscript/agent/reports/` | **Agent 2** dòng ViScript | như trên |
| `spec/` | chỉ task được ghi rõ (KT-04a, ST-02a) | xem §4 |
| `v2/AGENTS.md`, `viscript/AGENTS.md`, `AGENTS.md` | người điều phối / Agent 0 | Agent 2 không sửa |

Hai QUEUE **độc lập**: một task OS bị BLOCKED **không** chặn dòng ViScript và ngược lại (trừ cặp phụ thuộc ghi trong chính QUEUE).

## 4. Cấm

Cấm tuyệt đối, không có ngoại lệ "tạm thời":

1. **Sửa `spec/`** trừ task được ghi rõ trong QUEUE (`KT-04a`: `spec/abi.toml`, `spec/kernel.toml`; `ST-02a`: `docs/viscript/SPEC-lex.md`). Mọi thay đổi ABI/ngữ nghĩa khác phải qua Agent 0 (D2-04, D2-22).
2. Sửa `accept()` / kỳ vọng / snapshot của harness để test xanh.
3. Thêm `#[ignore]`, `#[should_panic]` để né, tắt test, hoặc bỏ case khỏi danh sách chạy.
4. **Nới ngưỡng** (thời gian, seed, số ca đỏ, `MAX_IO`, `MAX_TCBS`, `MAX_OBJECTS`, `KERNEL_STATIC_BUDGET`).
5. **Retry** để qua cổng (D2-07). Flake ghi `T-INFRA` kèm log; chạy lại **tối đa một lần** chỉ để phân loại, không để lấy xanh.
6. Sửa `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/viscript/ViScript-thiet-ke.md`, `docs/viscript/SPEC-lex.md`.
7. Sửa bất kỳ tệp nào trong `docs/v1/` (kho lưu trữ v1, tag `v1-archive`).
8. Thêm `unsafe` ngoài danh sách crate đóng (D2-17); thêm `cfg(target_arch)` vào kernel/runtime.
9. Thêm phụ thuộc crate ngoài allowlist giấy phép (D2-21, QD §9.3).
10. Đưa số đo viết tay vào tài liệu.

## 5. v1 đã đóng băng

- Tag **`v1-archive`** trên commit `ca2eaf5`. **Không** có tag `v1-final`.
- `docs/v1/` là **đọc-only**: 101 tệp `docs/v1/agent2-report-*.md`, `docs/v1/overnight-log.md`, `docs/v1/decisions/`, `docs/v1/KNOWN-ISSUES.md`, `docs/v1/README.md`.
- **Không soak lại v1**, không phát triển v1, không mở tính năng trên v1 (D-02 hết hiệu lực từ khi lưu trữ, QD §2.3).
- Sổ lỗi v1 còn mở đã mang sang v2 dưới dạng ca kiểm: [`docs/v1/KNOWN-ISSUES.md`](docs/v1/KNOWN-ISSUES.md).

## 6. Lệnh

```bash
./v2 check                  # nhanh: fmt + clippy + luật cấu trúc, không chạy sim
./v2 gate --fast            # cổng bắt buộc cuối mỗi task: fmt, clippy, luật cấu trúc, L0, gen-check (từ KT-05), L1 1.000 seed (từ khi có sim)
./v2 nightly                # L1 100.000 seed, mutation, Miri, soak (không chạy trong vòng task)

./vis-dev check             # dòng ViScript
./vis-dev gate --fast       # cổng bắt buộc cuối mỗi task ViScript
./vis-dev nightly
```

- `v2 nightly` và soak **không** chạy song song với build/cổng khác.
- **L1 chưa bật** cho tới khi có sim (KT-08); giai đoạn trước đó `v2/agent/STATE.md` ghi rõ "L1 chưa bật".
- Chưa có `gate` đầy đủ trong vòng 1; chỉ `gate --fast`.
- Đo thời gian L1 ngay khi sim chạy; vượt 60 s thì giảm seed của `--fast`, **không** giảm invariant (QD §5.2).

## 7. Commit

- Dòng OS: `[KT-##] <tóm tắt>` — ví dụ `[KT-07] boot riscv64 qua OpenSBI, 50 boot 0 lỗi`.
- Dòng ViScript: `[ST-##] <tóm tắt>` — ví dụ `[ST-04] lexer theo SPEC-lex`.
- Hậu tố task con giữ nguyên: `[KT-04a]`.
- Mỗi commit: `cargo fmt` đã chạy, cổng `--fast` xanh, header SPDX đúng loại tệp (QD §9.3: `//` cho Rust/asm, `#` cho `.vis .vish .vidl .toml`).
- Commit chạm `kernel/` hoặc `lib/` phải xanh `--fast`; lỗi logic phải có `.scn` đỏ trước (D2-05).
- Hồi quy ⇒ **revert trước**, điều tra sau (D2-03).

## 8. Duyệt

- Duyệt **chỉ** bằng một dòng trong tệp, không bằng chat:
  `APPROVED <YYYY-MM-DD> Agent 0` — ghi ngay dưới mục task trong `v2/agent/QUEUE.md` hoặc `viscript/agent/QUEUE.md`, kèm một mục D2/VD mới nếu quyết định thay đổi nội dung.
- `KT-04d` = gửi duyệt (checklist 5 mục/method + 20 kịch bản ABI); **APPROVED mở KT-05**. `ST-02a` = REVIEW, chặn `ST-04..ST-09`, không chặn `ST-02b/ST-03/ST-10`.
- Agent 2 soạn nháp spec ở trạng thái REVIEW; sau duyệt chỉ Agent 0 sửa (D2-22).
- Không tự duyệt, không tự suy ra "coi như đã duyệt".

## 9. QEMU và tiến trình nền

- **Cấm** `pkill -f qemu-system` toàn cục: chỉ giết **PID do chính task tạo** (`kill <pid>`, hoặc `pkill -P <pid>`).
- Tối đa 3 QEMU song song; sim dùng tối đa `số core − 1`.
- Máy bận/QEMU treo: giết đúng PID đã ghi, ghi `T-INFRA`, tiếp.

## 10. Trạng thái task

`QUEUE.md` dùng đúng 4 trạng thái: `READY | BLOCKED | REVIEW | DONE`.
`vifast-agentctl --project v2|viscript` : `next` (task kế có phụ thuộc đã DONE) **bỏ qua REVIEW**, `start <id>`, `done <id>`, `block <id> <lý do>`, `report`.
Mỗi task ≤ 3 giờ; task lớn hơn phải tách (QD §5.1, §10.4).

## 11. Định nghĩa "bàn giao xong"

Người mới mở máy, đọc **một** thư mục `agent/` + **hai** tài liệu là chạy được vòng lặp mà không hỏi ai: mọi task có (i) tiêu chí đo được, (ii) ngân sách, (iii) lệnh cổng, (iv) trạng thái.


---
