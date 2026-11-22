# BLOCKED — lịch sử Chu kỳ 1 (KT-01..KT-329)

**Lưu trữ**: 2026-11-22 (KT-332). Chuyển **nguyên văn** khỏi `v2/agent/BLOCKED.md` tại `0809b93a`
khi dọn các mục **đã đóng**. Tệp này **đọc-only**, **không xoá thông tin**.
Mục còn mở / nợ có chủ: [`../BLOCKED.md`](../BLOCKED.md).

Các mục đã đóng trong tệp này (đối chiếu khi cần tra cứu): bảng REVIEW vòng 1 (`KT-04d`, `KT-70..KT-73`,
hai mục S1/console ảnh sản phẩm), bảng `BLOCKED` theo `PATCH-BRIEF` §1 (`KT-03..KT-12`), câu hỏi
`YC-01..YC-18`, dòng ViScript (§4 — nay tách repo riêng), `KT-67` (memsrv — đóng bởi `KT-67b`/`KT-68`),
`KT-74` (phần ảnh sản phẩm — đóng bởi `KT-77b`/`KT-77c`), `KT-75` (ba giới hạn có văn bản),
`KT-76`/`S1` (đóng cùng `KT-77b`/`KT-77c`), `KT-78`, `KT-77a` và ghi chú phối hợp `KT-80`.

---

# BLOCKED — ViFastOs v2

Người ghi: **Agent 2**. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

## 1. REVIEW — chờ Agent 0 duyệt (không phải lỗi kỹ thuật)

| Task | Trạng thái | Nội dung chờ | Cần gì để mở |
|---|---|---|---|
| ~~KT-04d~~ | **APPROVED 2026-11-12 Agent 0** | Gói bản 2 + C1/C2; D2-35..39; `KT-05` mở | — |
| ~~KT-70~~ | **DONE + đã commit** `e50540b2` (D2-108) | Pager U-mode + page cache; TAP 30..34; matrix 12/12; gate 689/95 | — |
| ~~KT-71~~ | **DONE + đã commit** `36f59041` (D2-109) | Journal/fsck/rename/truncate; TAP 65..69; matrix 12/12; gate 703/96 | — |
| ~~KT-72~~ | **DONE + đã commit** `4e1fe995` (D2-111) | virtio-blk thật **chỉ trong ảnh test `ktest-vblk`**: PCI ECAM + kiểm chồng lấn rồi mới gán BAR + split virtqueue + đàm phán **FLUSH** + `BlockDev` + FS trên đĩa; TAP 6; matrix 15/15; gate 718/99; soak 0/1000 | — |
| KT-73 | **DONE phần cài + đo**, chờ reviewer verdict | UART RX IRQ → ring (KT-69) → **U-mode đọc byte thật** (shell `ktest-proc` hết message cắm sẵn); virtio-console `0x1af4:0x1003` hai queue qua PCI của KT-72; ảnh mới `ktest-con` 5 TAP (ca âm không-dữ-liệu + ring đầy); matrix **18/18**; conformance 61/61 + 5/5; soak 0/1000; verify D2-110A fail=0 + break-test. Log `JOURNAL/kt73-journal.md`, `docs/k6-console.md` | Reviewer verdict → lead commit `[KT-73]` |
| **K6 "tương đương v1" sẽ KHÔNG có shell trên ảnh sản phẩm** (hệ quả kiến trúc, không chỉ nợ KT-73) | Ảnh sản phẩm không có U-mode ⇒ shell chỉ tồn tại trong **ảnh test** `ktest-proc`. Vì vậy KT-76 **không thể** nhận "tương đương v1" chỉ vì `ktest-proc` có shell; điều kiện hợp lệ: (a) KT-74 đưa U-mode/shell vào ảnh sản phẩm, hoặc (b) KT-76 ghi rõ "tương đương v1 ở mức tính năng kernel, chưa có shell trên ảnh sản phẩm" + `BLOCKED`. Chi tiết `docs/k6-console.md` §6 | Mở ở KT-74 (SetTls + exec chain) / KT-76 (K6-DOD) |
| **Console trong ảnh sản phẩm** (nợ D2-111 mục "driver chưa vào ảnh sản phẩm") | **BLOCKED — thiếu người nhận, không phải thiếu trần**: ảnh sản phẩm **không có U-mode/shell** (`v2/kernel/vifast-kbin-riscv64/src/main.rs:360-361` halt sau boot; U-mode sau `#[cfg(feature = "user")]` `v2/kernel/vifast-plat-riscv64/src/lib.rs:95-96`; driver sau `#[cfg(feature = "vcon")]` `lib.rs:110-111`) ⇒ bật `vcon` cho ảnh sản phẩm là **code chết** (D2-08) và đổi `.text`/BUILD_ID (`02ed5a1810ba59ce`, soak neo). Trần **không** phải lý do: `ktest-con` dùng **214152**/262144 | Mở ở **KT-74** (SetTls + exec chain: ảnh sản phẩm có U-mode/shell thật) → bật `vcon`, đo lại trần + BUILD_ID + soak |

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
| ~~KT-11~~ | **DONE** `035584b`; 184 test, parity 10k, .scn 9/9, reviewer APPROVE | — |
| ~~KT-12~~ | **DONE** `e750847`..`d3dfe31` (host/format); no_std sang **KT-12b** (D2-42) | — |

Không có phụ thuộc vòng. Không có task nào bị chặn vì thiếu spec/thiếu quyết định: QD đã trả lời Q1..Q12 (QD §9).

Danh sách trên đã dọn theo trạng thái thật ngày 2026-11-12: KT-03/KT-07 **DONE**, KT-12 thôi ghi sai dep KT-08.

## 3. Câu hỏi cho Agent 0

> **Cập nhật 00:15:** KT-11 DONE; OS vòng 1 hoàn tất, không còn task READY. YC-06..17 đã chốt; không hỏi lại.



> **Cập nhật 18:20:** YC-02..05 đã được Agent 0 trả lời trong **D2-29..D2-33**; các mục dưới giữ làm lịch sử. **Không hỏi lại.**

> **Cập nhật 04:20:** KT-14 xong mã (reviewer DRIFT nhẹ, đã sửa); mở **YC-18** (thiết kế `with_many_mut`).
> Không chặn KT-14 — chỉ cần Agent 0 chốt **trước KT-15**.

### YC-18 — **ĐÃ CHỐT 2026-11-13 (D2-48, chọn a)** — `with_many_mut` cần vtable cho `ObjRef<dyn KObj>` (mở 2026-11-13, Agent 2)

- **Hiện trạng**: thiết kế `:207-208` yêu cầu
  `with_many_mut<const N, R>(&mut self, rs: [ObjRef<dyn KObj>; N], f: impl FnOnce([&mut dyn KObj; N]) -> R)`.
  Nhưng `ObjRef<T: ?Sized> { pa, id, PhantomData }` (thiết kế `:199`) chỉ **16 B** và **không mang vtable**;
  từ `(pa, id)` + `header.kind` không dựng được `&mut dyn KObj` (thiếu vtable). KT-14 đã làm **khung
  fail-closed**: kiểm trùng `pa` → `Alias`, còn lại cũng `Alias`, closure **không** được gọi, không panic
  (K1A-PLAN §4 cho phép "stub/khung"; ma trận alias + fuzz thuộc KT-15).
- **Đã chốt D2-48: chọn (a)** — bảng tĩnh `kind → vtable/constructor`, giữ `ObjRef` 16 B; KT-15 cài.
- **Đề xuất 2 lựa chọn** (Agent 0 chốt **trước KT-15**):
  - **(a) khuyến nghị** — bảng `kind → vtable/constructor` trong `vifast-objmem`: `ObjMem` tra
    `header.kind` để dựng `*mut dyn KObj` (bảng tĩnh, `no_std`); không đổi layout `ObjRef` 16 B.
  - **(b)** — đổi biểu diễn ref: mang thêm metadata/vtable (fat pointer thật, ~24 B) — lệch thiết kế
    `:199` (16 B), phải sửa thiết kế/QD trước.
- **Không phải lỗi KT-14**: phần core (`Arena`, `place`, `with`, `with_mut`, `destroy`, `Dead`, id) đúng DoD;
  KT-14 đã Miri 15/15 + build bare-metal + `gate --fast` xanh (199 test).

### Q-01 — KT-04c là **20** hay **21** kịch bản? — **ĐÃ TRẢ LỜI 2026-11-12 (D2-27)**

- **Chốt**: **21** kịch bản = 20 theo QD §4 + ca **"Exit khi procsrv chưa Recv"** (D2-27, Agent 0). KT-04c/KT-04d dùng 21, không hỏi lại, không tự đổi.
- Bối cảnh mâu thuẫn (giữ lại để tra cứu): **20** ở `docs/QUYET-DINH-sau-review.md:176` (QD §4) và `v2/agent/DECISIONS.md` Phần IV; **21** ở `v2/agent/QUEUE.md:104` + `docs/v2/ViFastOs-v2-thiet-ke.md:1068`; `review/PATCH-BRIEF.md` §1 ghi 20.
- Đóng cùng ngày với **D2-28** (KT-02 chỉ port tối đa `vifast-fs`, `vifast-fsutil`, `vifast-elf`; `cargo build` xanh + mỗi tệp ≤ 1500 dòng).

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
  ⇒ sau mỗi `create`/`write` chỉ còn **một** khối bẩn (commit record LBA 29); 1000 seed với kẻ thứ ba ghi
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

## 5. KT-67 — memsrv **U-mode** BLOCKED (ABI hiện tại không đủ), 2026-11-16

Trạng thái: phần **`vifast-alloc` + cầu nối D2-83 + TAP QEMU (60..64) đã xong**; phần **chương
trình `memsrv` chạy U-mode với vòng `NBRecv` riêng** bị chặn — **không** thêm syscall/sửa `spec/`
theo ràng buộc task.

| Chặn bởi | Dẫn chứng | Cần gì để mở |
|---|---|---|
| U-mode **không** gửi được cap trong message: shim ABI chỉ có `ecall` 7 thanh ghi, `NBSend` = label + 4 MR | `v2/kernel/vifast-plat-user/lib.rs:28` (`syscall7`), `:16` (`SYS_NB_SEND`); `Message.caps` chỉ kernel dựng (`v2/kernel/vifast-core/src/core_capxfer.rs`) | **D2 mới** mở rộng `NBSend`/`NBRecv` có khe cap (sửa `spec/abi.toml`), hoặc chọn phương án (A) dưới |
| U-mode **không** có invocation tạo `Frame`: dispatch chỉ syscall `0..15`; `Retype` VM trong core trả `NotSupported` (biên D2-83) | `v2/kernel/vifast-core/src/syscall.rs:279` (`dispatch`), `v2/kernel/vifast-core/src/machine.rs:828` (`if !ty.is_core()`), `v2/kernel/vifast-plat-common/src/obj/mod.rs:714` (`Machine::retype` — chỉ kernel) | giữ nguyên: Frame do kernel/object layer cấp hộ; memsrv = policy (phương án A) |

**Hai lựa chọn** (chi tiết + phân tích: `v2/docs/k6-memsrv.md` §6):

- **(A) đề xuất — SUPERSEDED bởi D2-97/D2-98/D2-99 (giữ làm lịch sử)** — memsrv `NBRecv` request rồi gửi yêu cầu cấp; kernel
  (`vifast-plat-riscv64::mem_bridge::grant` + `prepare_caps`) cấp Frame và cài cap vào receive
  window của client. Không sửa `spec/`; nhược điểm: quyền của cap client do kernel kiểm hộ.
- **(B)** — mở ABI một lần (D2 mới + sửa `spec/` + sim/parity): memsrv tự cấp/trả cap.

**ĐÃ CHỐT (D2-96, lead 2026-11-16) — SUPERSEDED bởi D2-97/D2-98/D2-99**: chọn **(A)** — KT-67 =
allocator thật + memory bridge **kernel-mediated**; `memsrv` U-mode = **policy façade** (chưa cài).
Invocation hẹp
`Untyped.Retype` (để memsrv giữ cap `Untyped` và tự retype) tách sang **KT-67a** (D2 mới vì mở
`spec/[method_signature]`), **không** nhét vào KT-67.

Không còn câu hỏi mở cho KT-67; **không** tự mở `spec/`.

**CẬP NHẬT KT-67a (2026-11-16, Agent 2)**: **D2-97** phát hiện invocation **đã** nằm trong ABI
(`spec/abi.toml` `[syscall].0`: “object khác: invocation (method = label)” + `[object]`), nên
**không** cần mở `spec/`; KT-67a đã **cài** đường invocation hẹp và **gỡ** phần “memsrv không tự
retype”. Đường cấp Frame của pha này **không** đi qua `mem_bridge::grant`: U-mode `Call` ⇒ core
`invoke` (kiểm cap/quyền của chính caller) ⇒ `SyscallEnv::invoke` ⇒ `mem_bridge::invoke` — đây là
**hook arch của object layer** (nơi **duy nhất** sở hữu Frame theo D2-83), **không** phải bước
kernel quyết định/cấp hộ như `grant`.

- `Untyped.Retype` (method 0): memsrv U-mode giữ cap `Untyped` (cấp lúc spawn) và **tự** retype
  `Frame` — không còn đường kernel-mediated `mem_bridge::grant` trong pha này;
- `Frame.Map`/`Frame.Unmap` (method 0/1): client U-mode **tự map** Frame vào VSpace của mình, ghi/
  đọc thật 8 B, unmap rồi đọc lại ⇒ `FaultKind::Vm{NotMapped,Read}` (TAP `ktest-user` 15..18).

**Còn một điểm chưa làm — nhưng KHÔNG phải thiếu ABI (D2-98 sửa lại)**: **chuyển cap Frame
giữa hai TCB U-mode** (memsrv → client khác tiến trình). ABI **đã có** receive window trong IPC
buffer: `spec/abi.toml` `[calling.riscv64].ipc_buffer = "Frame bind qua Tcb.Configure; header chứa
receive window (CPTR, ≤4 cap)"` (dòng 58), và core đã có mô hình
`tcb_set_recv_window`/`plan_cap_transfer`/`deliver_caps` (`v2/kernel/vifast-core/src/core_ipc.rs`,
`core_capxfer.rs`). Phần **thiếu là shim U-mode**: `syscall7`
(`v2/kernel/vifast-plat-user/lib.rs:28`) **không** ghi header IPC buffer (nơi khai `caps` + CPTR
nguồn); chỗ **từ chối** `MessageInfo.caps > 0` là nhánh **invocation**
(`v2/kernel/vifast-core/src/invoke.rs`, D2-99(g)), còn `build_message`
(`v2/kernel/vifast-core/src/syscall.rs`) **không** kiểm `caps` — nó chưa có đường đọc CPTR
nguồn/window từ header. *(Đính chính nit reviewer-kt67a: câu cũ ghi sai chỗ từ chối.)*
Vì vậy KT-67a chứng minh **memsrv+client trong một cspace**;
**KT-67b (D2-98)** cài header IPC buffer + receive window trong shim để tách hai tiến trình —
**không** cần sửa `spec/`.

**ĐÓNG BLOCKED NÀY — KT-67b (2026-11-16, Agent 2)**: header IPC buffer (spec `abi.toml` dòng 58)
đã được cài trong shim (`kernel/vifast-plat-user/lib.rs`) + kernel đọc qua
`SyscallEnv::ipc_header`/`apply_ipc_header`/`build_message`; **hai tiến trình riêng**
(`memsrv.elf` → `client.elf`, 2 TCB/2 cspace/2 VSpace) chuyển cap Frame, cap nằm trong **cspace
bên nhận** và map/ghi/đọc thật (TAP 19, `# capx:` evidence). Chi tiết + bằng chứng:
`docs/k6-memsrv.md` §8, `JOURNAL/kt67b-journal.md`. Nợ còn lại của đường cap: policy **copy**
move chưa có (`CNode.Delete`/`Revoke` ngoài danh sách trắng) — ghi ở journal §11. Ngoài ra `CNode.Insert` mà D2-96/97 nhắc **không có method id** trong
bảng `CNode` của spec (`Copy=0..Rotate=6`, spec đóng băng) nên giữ `NotSupported`; core đã có API
`copy/mint/delete/revoke` để gắn id có sẵn ở task sau.

**ĐÓNG NỢ "CNode.Delete/Revoke ngoài danh sách trắng" — KT-68 (2026-11-16, Agent 2)**: KT-67b
kết thúc với nợ *"policy copy, move chưa có (`CNode.Delete`/`Revoke` ngoài danh sách trắng)"*.
KT-68 đã cài **đủ 5 method** `CNode.Copy=0/Mint=1/Move=2/Delete=4/Revoke=5` (số **sinh từ
`spec/abi.toml`**, không sửa spec) + `Machine::move_cap` (D2-44), và chứng minh trên QEMU:
capalloc U-mode cấp/thu hồi **theo cây MDB** (kể cả **xuyên CNode**), `Delete` reparent, cấp lại
**cùng slot**, không rò slot/cap — TAP 20..24, `# capa:` evidence, `docs/k6-capalloc.md`.
`CNode.Mutate=3`/`Rotate=6` vẫn **chưa cài** (giữ `NotSupported` 18, có TAP) — nợ có chủ, cần D2
mới nếu muốn mở; `CNode.Insert` vẫn **không có số** trong spec.

**NỢ KT-71 (không chặn) — pager U-mode GHI trang**: KT-71 cài **write-back** trong
`vifast-vfs` (`PageCache.dirty`, `mark_dirty`, `flush_page` 8 sector + `dev.flush()`,
`flush_all`, `load()` từ chối slot bẩn `Busy=14`, giữ bản ghi mục vừa trục xuất để flush được)
và **đo trên QEMU** bằng `RamDisk` trong kernel (cùng mã crate, TAP 66 `dirty_marks=0->1
dirty_now=0 writes=8 flushes=1 backend_ok=true reopen_ok=true`). Nhưng **pager U-mode**
(`init/pager_server_user.rs`) vẫn **chỉ đọc** — đường ghi từ chương trình U-mode qua
`Frame.Map` + shim (`read_volatile/write_volatile` trên Frame của chính nó, D2-101) cần
**KT-72** vì phải có virtio-blk thật để flush ra thiết bị; KT-72 chỉ **thêm `impl BlockDev`**
(không sửa `PagerCore`). Ghi ở `docs/k6-vfs-journal.md` §7 + `k3b-core.md` §19.

**NỢ KT-72 (không chặn) — ghi rõ "chưa xong" theo phạm vi task**: KT-72 đã đạt cả 8 mục tiêu
(PCI+virtio, virtqueue, `BlockDev`, FS **thật** trên đĩa, TAP `# vblk:`, red-before host + QEMU,
không hồi quy, docs). Các phần **còn nợ có chủ**, không phải blocker:
1. **Poll, chưa ngắt** virtio (MSI-X/INTx); queue depth thực tế 1 — đủ cho FS tuần tự, chưa tối ưu.
2. **`flush`** mới chứng minh ở **mức thiết bị**: request `VIRTIO_BLK_T_FLUSH` được hoàn tất
   (`flushes=2 completions=2`) + dữ liệu host đọc lại được; **bền qua reboot** thuộc **KT-75**
   (`test-persist` 7 pha). Không tuyên bố hơn mức đo.
3. **`id_mismatch`/`timeouts` chưa có ca âm** (D2-108B) ⇒ "chưa chứng minh" — đã **bỏ khỏi
   điều kiện `ok`** của TAP 6 (`const:id_timeout_proven=0`) nên TAP 6 **không phải** bằng chứng;
   `errors` có ca dương (ca ép `read_sector_force` + mutation bỏ kiểm dải ⇒ QEMU `not ok 6`).
4. **PCI**: một bus/một function, BAR gán **cố định** đầu cửa sổ, map leaf 2 MiB — **kiểm chồng
   lấn** đã có (BAR function khác + vùng nền tảng + trang đã map) nhưng **chưa đọc DT/`ranges`**;
   chưa phải enumerator tổng quát (không cấp phát động/multiple bridges). Nợ thật cho KT-73.
5. Driver **chưa vào ảnh sản phẩm** (feature `vblk = ["ktest"]`) → **KT-73** mở; vì vậy soak
   KT-72 không đi qua virtio (`-drive` không làm hỏng soak — đúng yêu cầu task).
6. Ảnh `ktest-vblk` chưa in `# conf image:` (digest D2-80) — thêm cùng lúc khi KT-73/75 cần neo ảnh.
7. Ảnh đĩa 4 MiB do runner tạo; driver từ chối `blk_size != 512` (`decode_config` — **đã có ca
   âm host test**) — chưa hỗ trợ sector 4 KiB (không cần cho `vifast-fs`).
8. **Coverage tích hợp không có**: không boot nào chạy **driver + 70 TAP kernel**, **driver +
   U-mode**, hay **driver + conformance** cùng lúc; tách ảnh giữ coverage **cũ**, không thêm tổ hợp.
9. **Xung đột VA là thật** (`ktest_obj.rs:31` dùng `0x4000_0000`) — lý do tách ảnh, đã ghi docs.

## KT-74 — `Tcb.SetTls` + `exec` chain: **DONE phần cơ chế** / **BLOCKED** phần ảnh sản phẩm

**Đã đóng**: `Tcb.SetTls` (method id **do `spec/abi.toml` `[object].Tcb` cấp**, core lấy qua
`vifast_abi::TCB_SETTLS`, **không** tự đặt số), `tls` lưu trong `TcbData`/`TcbInfo`, `x4`/`tp` của
U-mode nạp từ core ở **mọi** lần resume; `exec(path)` nạp ELF mới **thay** ảnh, giữ `pid`/cspace,
reset entry/stack/TLS/thanh ghi, **giải phóng** VSpace+Frame cũ, **nguyên tử** khi lỗi; chuỗi
**ba** bước A→B→C mỗi bước chạy thật; TAP `ktest-exec` + matrix + gate + soak + docs.

**Phát hiện kiến trúc (đáng chú ý cho mọi task sau)**: kernel dùng `tp` làm **hart id**
(`read_hart_id`), còn `sret` xuống U-mode nạp **toàn bộ** GPR từ trapframe ⇒ trước KT-74 `tp` của
user luôn `0` nên `hart_id()` trong handler U-mode đúng **nhờ may**; khi `SetTls` đặt `tp` khác `0`
thì `current_hart()` trả rác (đo được `hart=7`) và `dispatch_on_private_stack` panic. Vá:
`user::restore_kernel_tp()` nạp lại `tp` từ `USER_KERNEL_CTX[3]` ở đầu `on_u_exception`; đường
`sret` về U-mode tự khôi phục `x4` từ frame nên **không** rò hart id sang user. Ghi ở
`docs/k6-exec.md` §2.2.

**BLOCKED (có văn bản, theo phương án (b) của D2-113): ảnh SẢN PHẨM vẫn KHÔNG có U-mode/shell.**
- **Trần KHÔNG phải lý do**: `.data+.bss` ảnh sản phẩm `200696 / 262144` (headroom `61448`), và ảnh
  **đã có** U-mode+procsrv+shell (`ktest-proc`) chỉ `207256 ≤ 262144`; ảnh U-mode mới
  (`ktest-exec`) `202992 ≤ 262144`. Không nới trần, không cần tách ảnh vì trần.
- **Lý do thật**: ảnh sản phẩm **không có đường dẫn nào tới U-mode** — `src/main.rs` (376 dòng)
  không có `user::`/`include_bytes!`/`procsrv`/`shell`; `Cargo.toml:13` `default = []` và mọi bin
  U-mode `required-features = ktest,user*`; **mọi** call-site `user::load_init_slot` nằm trong
  `src/ktest_*.rs`. Đưa U-mode vào sản phẩm = **viết mới** policy procsrv + vòng scheduler U-mode
  + đường bàn phím (ring UART) cho ảnh sản phẩm ⇒ **vượt ngân sách ba giờ** của KT-74.
- **Hệ quả cho KT-76**: K6 "tương đương v1" **không** được nhận chỉ vì `ktest-proc` có shell; nợ
  D2-113 chuyển nguyên trạng. Ảnh sản phẩm **đã đổi** (sha mới, `.text` mới) ⇒ soak **đã chạy
  lại** trên đúng sha đó.

**Ngưỡng đã công bố (KHÔNG nới)**: `size_of::<Machine>()` `188872 → 189384` (+`512` B = **một**
`u64` TLS cho mỗi TCB × trần `MAX_TCBS`). Pin **196608** và trần **262144** giữ nguyên; ba mẫu
evidence trong `v2/test/boot/ktest.sh` (`# core: link`, `# core: phase`, `# user core:`) được cập
nhật **cùng lúc** để khớp số đo mới — đây là **đo lại**, không phải nới ngưỡng (D2-109C).

**Sửa test host lỗi thời (công bố rõ)**: `capalloc_tests::capalloc_cap_thu_hoi_cap_lai_cung_slot`
khẳng định `invoke::stats()` (bộ đếm **toàn cục**) `>= 4` cố định ⇒ **phụ thuộc thứ tự chạy**
(đo được: ca này **FAIL một mình** trên HEAD sạch, trước mọi thay đổi của KT-74). KT-74 đổi sang
khẳng định **delta** của chính ca (`Copy 2 · Revoke 2 · Delete 2`) — chặt hơn, không phụ thuộc
lịch chạy; không đổi ngữ nghĩa sản phẩm.

**Nợ còn lại**: ABI `Tcb.Exec(path)` (cần VFS thật, KT-75+); `exec` chưa có ca SMP thật (một hart
chạy chuỗi); ca chạm biên `MAX_INIT_PAGES` chưa có; ảnh sản phẩm U-mode/shell = BLOCKED ở trên.

**KT-74 — bổ sung bằng chứng cho reviewer (sau khi lead hỏi)**: (a) **ca âm `restore_kernel_tp`** —
bỏ dòng đó trong `on_u_exception` ⇒ `hart_id()` đọc `tp` của user (`hart=7` = `0x7431_0001` cắt bởi
`min(MAX_HARTS-1)`) ⇒ `PANIC: syscall stack chua cap` ngay syscall đầu, **không** tới được `ok` nào
(`JOURNAL/kt74-red-tp.log`); khôi phục ⇒ md5 `user.rs` trùng, boot đạt đủ TAP. (b) **phép tính pin**
được **test** `pin_machine_tang_dung_8_byte_moi_tcb` chứng minh `8 × MAX_TCBS` chứ không suy luận
(`JOURNAL/kt74-pin-calc.log`). (c) **`capalloc_cap_thu_hoi_cap_lai_cung_slot` hỏng có sẵn**: chạy
**một mình** trên worktree ở commit nền ⇒ đỏ (`JOURNAL/kt74-red-capalloc-head.log`) — bản sửa đổi
sang khẳng định **delta**, không xoá assertion nào.

**KT-74 — vá theo review (D1..D8)**: (D1) verifier chỉ miễn số **trong đúng dòng tiêu đề** + selfbreak
có bước chèn số trần ở prose ⇒ ĐỎ; (D2) số `hart` nằm trong khối `scope-red` (`kt74-red-tp.log`);
(D3) `exec` chụp/khôi phục bảng dải VA khi loader lỗi muộn — **chưa có ca kiểm** lỗi muộn vì
`ObjKind::Untyped` không được object layer hỗ trợ (`v2/kernel/vifast-plat-common/src/obj/mod.rs:720`
trả `NotSupported`) nên không dựng được untyped nhỏ để ép lỗi ⇒ **nợ có ca** cho KT-76, câu "nguyên
tử" đã thu hẹp; (D5) ảnh A lấy method từ hằng sinh tự động, evidence `const:method_source=spec`;
(D6) `ktest_inv` assert `delta.set_tls == 0`, `releases_clean` dùng `==`; (D7) sửa câu call-site
(`src/proc_demo.rs`); (D8) chép toàn bộ gate log vào `JOURNAL/kt74-gate.log`.

## KT-75 — `test-persist` bảy pha (K6): DONE, ba giới hạn **có văn bản** (không phải chặn)

**Không chặn task nào.** KT-75 đóng nợ D2-111 (“`flush` bền qua reboot”) bằng ảnh test riêng
`ktest-persist` + runner `v2/test/boot/persist.sh` (mỗi pha: ghi ở một tiến trình QEMU, đọc lại ở
tiến trình khác trên cùng ảnh đĩa, so từng byte) + kiểm chứng host `vifast-fsutil` sau mỗi boot.
Ba giới hạn phải nói rõ để KT-76 **không** nhận “tương đương v1” bằng bằng chứng sai:

1. **Ca âm `flush` là mô hình cache**, không phải QEMU tự mất dữ liệu: QEMU ghi thẳng file ảnh
   trên host nên dữ liệu “đã ghi” luôn nằm trong file. Cái được **đo** là **ranh giới bền**: không
   `flush` ⇒ không byte nào ra thiết bị (sha256 ảnh không đổi, `device_flushes=0`), boot sau không
   thấy tệp; có `flush` ⇒ dữ liệu sống qua reboot (bảy pha dương). Muốn chứng minh trên **đĩa thật**
   thì cần phần cứng/thiết bị có cache bay hơi thật — **nợ của K6-DOD nếu reviewer yêu cầu**.
2. **Matrix không chứa `ktest-persist`**: một pha là **hai** boot trên cùng ảnh đĩa, không vừa mô
   hình “mỗi ảnh một boot” của `expected.txt` (D2-110 cấm nới mô hình). Matrix vẫn là bảy ảnh độc
   lập (xanh toàn bộ); bằng chứng persist nằm ở `docs/k6-persist.md` + `JOURNAL/kt75-*`.
3. **Ảnh sản phẩm vẫn không có U-mode/shell** (D2-113) — KT-75 **không** thay đổi ảnh sản phẩm
   byte nào (`sha256` trùng D2-115, soak lại trên đúng sha đó). Nợ này **nguyên trạng** cho KT-76.

**Nợ kỹ thuật còn lại**: ảnh persist chạy **một hart**, poll (như KT-72); chưa có `fsync` từ
U-mode; chưa có ca torn-write giữa transaction trên ảnh này (KT-71 đã có bảy pha crash trên
journal — bằng chứng riêng); chưa có ca dung lượng đầy (`NoSpace`) trong persist.

## KT-76 — cổng K6: DOD trung thực đã xong; **BLOCKED** mục "tương đương v1" ở mức NGƯỜI DÙNG (S1)

Trạng thái (sau review + quyết định Agent 0): **KT-76 = DONE** (task cổng — DOD trung thực, cổng
xanh), **phase K6 = KHÔNG ĐẠT** ở nghĩa "tương đương v1" (dòng **S1** dưới đây); Agent 0 **mở
KT-77**. Tám hạng mục **đo được** của thiết kế §10.1 dòng K6 **ĐẠT** (VFS journal/fsck/rename/truncate/double-indirect; pager nhiều client; exec chain;
“test-persist 7 pha”) và toàn bộ cổng nặng đã xanh (`v2/agent/reports/K6-DOD.md` §5). Cái **chặn**
việc tuyên bố "tương đương v1" là **một** dòng phát hiện, không phải một hạng mục của §10.1:

**S1 — "tương đương v1" ở mức người dùng: ảnh SẢN PHẨM không có U-mode/shell.** Ảnh sản phẩm
`vifast-kbin-riscv64` **halt** ngay sau `# k3 halt` (`v2/kernel/vifast-kbin-riscv64/src/main.rs:361-362`);
module U-mode chỉ biên dịch khi `feature = "user"` (`v2/kernel/vifast-plat-riscv64/src/lib.rs:104-105`),
console/UART cũng sau feature (`lib.rs:99-100`, `lib.rs:123-124`) — mà ảnh sản phẩm build **không
feature** (D2-108A). Shell thật của v2 chỉ có trong **ảnh test** `ktest-proc`; v1 có `shell>` +
`--input-check` trên **chính ảnh boot** (`docs/v1/agent2-report-21.md:83`,
`docs/v1/overnight-log.md:1499`). Bằng chứng đo: `JOURNAL/kt76-product-umode.log` (`procsrv`/`shell`
bằng không ở ảnh sản phẩm, khác không ở `ktest-proc`) + `JOURNAL/kt76-product-boot.log` (dừng ở `# k3 halt`).

**Vì sao đây là BLOCKED (không phải nợ thường):** tiêu chí "Tương đương v1" của §10.1 là **câu chữ
của thiết kế**; tôi **không** được tự định nghĩa lại cho dễ đạt (yêu cầu KT-76). Muốn đóng, cần
**một trong hai** quyết định của **Agent 0**:

- **Cách một — KT-77: đưa U-mode/shell vào ảnh sản phẩm**: bật `feature = "user"` cho bin sản phẩm + nhúng
   `procsrv`/`shell` ELF + bật `vcon`/UART IRQ; đo lại BUILD_ID, trần ảnh, soak. Trần **không**
   phải lý do (`JOURNAL/kt76-budget.log`: mọi ảnh dưới trần, có headroom; driver console đã có sẵn
   trong ảnh test).
- **Cách hai — hạ phạm vi có văn bản**: Agent 0 ra D2/sửa thiết kế nói rõ "tương đương v1 **ở mức tính năng
   kernel**" (khi đó S1 thành N/A **có nguồn**). Tôi **không** tự làm việc này.

**Không chặn** phần còn lại: cổng K6 xanh, không nới ngưỡng nào; bảy lỗi v1 đã phân loại ĐẠT/KHÔNG/
N/A trong `K6-DOD.md` §4; nợ kỹ thuật (fsync U-mode, `NoSpace`/torn-write, SMP persist, ca âm bộ
đếm, `MR4..MR63`, IPI core, poll chưa ngắt, `Tcb.Exec(path)`, lỗi-muộn `exec`, matrix không phủ
persist, driver không vào ảnh sản phẩm) liệt kê đủ ở `K6-DOD.md` §6 với `file:line` + đề xuất.

## KT-78 — harness QEMU (`OD-K6-6`): **DONE**, không BLOCKED; ba giới hạn **có văn bản**

- **Không** mục nào bị chặn: ca âm (a) cha chết (`SIGKILL`/`SIGINT`/`SIGTERM`) và (b) boot treo đều
  có log trong repo; `qemu_ppid1=0` và `qemu_foreign=0` **trước và sau** mỗi phép đo (matrix/soak/
  persist/run + `gate`/`L1`); cổng xanh lại (`gate --fast`, matrix, soak, persist) trên **đúng** ảnh
  sản phẩm cũ; đỏ-trước tái hiện `D2-116` bằng chính pattern cũ (QEMU `ppid=1`).
- Giới hạn **có văn bản** (không phải chặn, theo `OD-K6-6`; chi tiết `docs/k6-harness.md` §7):
  1. phụ thuộc `perl` cho `setpgrp` (macOS không có `setsid`): thiếu `perl` ⇒ `qg_init` trả `2` và
     harness **dừng** (fail-closed) chứ không chạy QEMU thiếu process group;
  2. trần `loadavg_1m > ncpu` chỉ là **lưới phụ** — máy mười hai nhân với hai QEMU mồ côi chỉ thêm
     khoảng hai đơn vị tải, nên bộ đếm QEMU mới là detector chính (ghi rõ để không đọc `verdict=OK`
     thành “máy chắc chắn rảnh”);
  3. `qemu_own` của thân script **không** đếm QEMU trong lane soak (chỉ gọi trước/sau phép đo nên
     không ảnh hưởng kết luận).
- **Không** thuộc KT-78: tái lập **byte** của build (nợ `P20`, `KT-79`); KT-78 **không** sửa
  `v2/kernel/**`, `v2/lib/**`, `spec/` (ảnh sản phẩm **trùng byte** KT-76).

## KT-77a — parity v1 (`OD-K6-2` phần đầu): **DONE**, không có mục nào bị chặn

- **Không** mục nào của KT-77a bị chặn: `spec/v1-parity.toml` có **26/26** lệnh kèm `source`
  (`docs/v1/`), runner chạy được cả `--mode transcript` (4/4) và `--mode serial` (4/4) trên ảnh test,
  ba ca âm có log trong repo (exit 3/4/5). Log: `v2/agent/JOURNAL/kt77a-*`,
  tài liệu: `v2/docs/k6-parity.md`.
- **Chuyển tiếp (không phải chặn KT-77a):**
  1. **KT-77b** — shell + U-mode trong **ảnh sản phẩm**, rồi chạy
     `bash v2/test/parity/run.sh --mode serial --image product`; lúc đó mới là **cổng parity thật**
     (OD-K6-2 tiêu chí 1–6). Ảnh sản phẩm hiện **không có** `# shell: up` (S1 **vẫn mở**).
  2. **22 lệnh `deferred-K7`** — mỗi lệnh cần syscall/ABI mới (`MemInfo`, `TcbInfo`, `STATFS`,
     exec-from-FS) hoặc VFS client trong shell. Danh sách + lý do nằm trong `spec/v1-parity.toml`
     (`status = 'deferred-K7'`, `reason`), đúng OD-K6-2 ("lệnh không làm được trong ngân sách ghi
     `deferred-K7` kèm lý do"). Mở các lệnh này = **quyết định ABI/phạm vi của Agent 0** (D2-121
     mục (a)/(d) — Agent 1 không tự quyết).
  3. **Đo serial "chặt" trên ảnh test** cần bỏ script nội bộ `proc_demo::build_script`
     (`v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs:270-280`) — tệp **ngoài vùng KT-77a**
     (`v2/kernel/**/src/**`) và ngoài vùng KT-78. Đo được hiện tại: byte ngoài vào shell nhưng gộp
     cửa sổ RX với script kernel ⇒ `inject_ok=0`, TAP nội bộ ảnh test đỏ
     (`kt77a-serial-1-help-serial-ktest-proc-smp1.log:118`). Không tự sửa.
- **Không** đổi ABI, không sửa `spec/abi.toml`/`spec/kernel.toml`, không sửa `v2/test/boot/**`,
  không sửa `docs/v1/**`, không nới ngưỡng, không retry để xanh.

## 9. Ghi chú phối hợp từ KT-80 (không phải task bị chặn)

- **Không có task nào bị chặn kỹ thuật** bởi KT-80: năm luật đã cài, `v2 check-rules` xanh, ca phá
  5/5 đỏ đúng mã. Mục này chỉ ghi hai việc phối hợp:
- **Luật 1 áp cho tài liệu mới** (`v2/agent/reports/*.md`, `v2/docs/k6-*.md` **đã có trong git
  index**): tài liệu mới phải đặt số trong khối `scope=<log trong repo>` và trích **nguyên văn cả
  dòng**; tài liệu chưa `git add` là bản nháp nên chưa bị kiểm, **commit xong là bị kiểm**. Tài liệu
  viết trước OD-K6-5 có tên trong `[[legacy]]` (ghim sha256); sửa nội dung tệp legacy ⇒ cổng đỏ cho
  tới khi hoặc đưa số vào khối `scope=`, hoặc cập nhật ngoại lệ có lý do + ngày và lead duyệt.
- **Cần KT-77a xử lý trước khi commit**: `v2/docs/k6-parity.md` (task song song, chưa `git add`)
  hiện chưa đạt luật 1 — khối `scope=` chép kèm tiền tố `số-dòng:` nên dòng không nguyên văn trong
  log (`k6-parity.md:85-89`, `:96-99`; log `kt77a-transcript-ktest-proc-smp1.log`). KT-80 **không**
  sửa tệp của task khác và **không** thêm ngoại lệ cho tài liệu mới; đề nghị KT-77a chép đúng dòng
  log (hoặc thêm khối riêng) trước khi `git add`.

### KT-77a — vá sau review (2026-11-17, reviewer-kt77a)

- **Đã sửa trong vùng được phép** (`spec/v1-parity.toml`, `v2/docs/k6-parity.md`,
  `v2/test/parity/**`, `v2/agent/**`): (1) `reason` của `time`/`uptime`/`jobs` — **không** phải ABI
  thiếu: `ClockRead` (syscall 11) đã có (`spec/abi.toml:117`, `user.rs:1229`, TAP `ktest_user.rs:1105`);
  `ps` đã nhận state từng TCB từ reply `PS_REQ` (`shell_user.rs:189-218`) ⇒ chỉ thiếu **builtin**;
  (2) thêm `cd`, `pwd` với **cả hai chiều** bằng chứng (tag `v1-archive` có; `overnight-log.md:3664-3669`
  ghi đã revert) + `decision = 'cho Agent 0'`; (3) sửa `TcbInfo` khỏi danh sách "ABI thiếu" trong doc;
  (4) `help` không khoá regex vào `cmds=6`; (5) runner ghi `image_sha256`/`image_bytes`/`artifacts_bin`
  vào log; (6) sửa 3 lệch dòng (`free` 2641, `free`/`df` `source_more` 3469, `\r` ở `agent2-report-62.md:24`).
- **BLOCKED-CHO-A0 (một mục, không tự quyết):** `cd`/`pwd` — dữ kiện mâu thuẫn: mã/tài liệu ở tag
  `v1-archive` **có** (`crates/vifast-client/src/main.rs:878,883`; `docs/11-hien-tai/16-van-hanh.md:109,121`)
  nhưng `docs/v1/overnight-log.md:3664-3669` (vòng 222) ghi **đã revert**. Đề nghị Agent 0 chọn:
  (a) coi là lệnh v1 (đưa vào parity, `deferred-K7` vì shell v2 chưa có), hoặc (b) coi là đã bỏ (ghi
  `not-in-v1` + lý do). KT-77a giữ `deferred-K7` + `decision = 'cho Agent 0'`.
- **Cảnh báo bắt buộc (OD-K6-2 tiêu chí 6):** `ps` được đánh `active` nhưng bên phục vụ `PS_REQ`
  **chỉ có trong ảnh test** (`proc_demo.rs:338`), **không** có ở đường sản phẩm ⇒ ảnh sản phẩm muốn
  `ps` chạy được phải có **procsrv ở đường sản phẩm** (KT-77b). Ghi ở `v2/docs/k6-parity.md` §5.2.
- **D2-102a (kernel-side, không sửa được ở KT-77a):** `# shell: help cmds=6` in hằng `CMDS` không
  nhãn `const:` (`shell_user.rs:45,248-249`). Runner đã bỏ khoá regex vào số; thêm nhãn thuộc KT-77b.
- **Ảnh đo:** runner ghi sha+size vào log; `artifacts/<sha>.bin` cho ảnh test **chưa có** (`co=0`).
  Ảnh sản phẩm `fd202276…` = **trước canonical, KHÔNG dùng làm bằng chứng v2.0** (D2-122(2)); đóng băng
  `artifacts/<sha>.bin` thuộc **KT-84**. Vòng đo `-r2b` có **QEMU của tab KT-77b** ⇒ `qemu_foreign=1`,
  `verdict=KHONG_HOP_LE`; không dùng số thời gian của vòng đó.
