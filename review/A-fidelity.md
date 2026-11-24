# A-fidelity — Kiểm tra trung thực & nhất quán: ViFastOs v2 so với dossier v1

**Người kiểm:** reviewer kỹ thuật (subagent) · **Ngày:** 2026-11-12
**Nguồn sự thật:** `src/ViFastOs-dossier.md` (2220 dòng, hồ sơ v1 hợp nhất)
**Bản cần kiểm:** `src/ViFastOs-v2-thiet-ke.md` (986 dòng, do agent0 dựng lại)
**Quy ước dẫn chiếu:** `v2:NNN` = dòng trong bản thiết kế; `dos:NNN` = dòng trong dossier.
**Phạm vi đọc:** toàn bộ v2; dossier đọc kỹ §0, §2.3, §4.2, §5.1, §6.6, §7.3, §7.6, §8.3, §9.1, §10.2–10.8, §12.3, §13, §14, §15.1–15.8, §16.2–16.6, §17, §18.2, §19, §20, §21.2. Không đọc lại toàn bộ 2220 dòng; các kết luận dưới đây chỉ dựa trên phần đã đọc và dẫn chiếu được.

---

## 0. Phán quyết

**Bản v2 KHÔNG trung thực đầy đủ và CHƯA đủ để làm tài liệu điều khiển thi công.** Đây là một bản thiết kế kỹ thuật có nhiều ý đúng và kế thừa tốt, nhưng ở vai trò "tài liệu điều khiển thực thi" nó vi phạm chính điều kiện mà dossier đặt ra: mọi con số/ mã phải truy vết được, và các ràng buộc cứng/ lỗi mở phải được mang theo hoặc tuyên bố rõ là đã bỏ. V2 đã (a) làm mất 8 quyết định đang hiệu lực D-01..D-08, (b) làm mất/ làm nhẹ 4 lỗi mở và 3 mục audit, (c) đổi ABI lõi (syscall, KError, quyền, MR) mà không gắn nhãn, (d) có mâu thuẫn nội bộ về ID, thư viện, queue và đo lường, và (e) hứa hẹn vượt bằng chứng ở các mốc 750 boot, 90 ngày, K6 "tương đương v1". Cần sửa P0 trước khi giao Agent 2; nếu không, Agent 2 sẽ thi công một ABI khác dossier và không biết cái gì là bắt buộc.

---

## 1. Quy ước nhãn: có, nhưng không đủ và dùng sai

### 1.1 Hai hệ nhãn khác nhau

Dossier §0.1 (dos:57–67) định nghĩa **bốn** nhãn độ tin cậy và nói rõ **mặc định là `[CODE]`**:
`[CODE]` (đã đọc mã/chạy lệnh), `[ĐO]` (số đo trong nhật ký), `[DOC]` (chỉ tài liệu, chưa đối chiếu), `[GT]` (giả thuyết).

V2:8 định nghĩa **ba** nhãn: `[DOSSIER]` "lấy từ hồ sơ v1", `[GT]` "giả thuyết cần kiểm", `[ƯL]` "con số ước lượng, phải đo lại". V2 **không có** nhãn cho thiết kế mới, không có `[DOC]`/`[ĐO]`, và không có quy tắc "không nhãn thì mặc định là gì". Hệ quả: một dòng không nhãn trong v2 có thể là số liệu dossier, số liệu tự nghĩ, hay quyết định thiết kế — người đọc không phân biệt được.

### 1.2 Mật độ nhãn

Trên 986 dòng, v2 dùng nhãn khoảng **14 lần** (chưa kể dòng định nghĩa): `[DOSSIER]` 2 lần dùng thật (v2:51, v2:899), `[GT]`/`[GT: …]` 3 lần (v2:317, 441, 517), `[ƯL]` 9 lần (v2:212, 213, 257, 451, 614, 731, 743, 832 và tiêu đề §10.6). Tức khoảng **1,4% số dòng** có nhãn, trong khi phần lớn khẳng định định lượng của bản thiết kế nằm ở dạng không nhãn.

### 1.3 `[DOSSIER]` bị dùng sai

- `v2:51` gắn `[DOSSIER]` cho cả §1, nhưng bảng G1..G6 là **cách phân loại mới** của agent0, không có trong dossier. Dossier nói **bốn nguyên nhân gốc** (dos:1787), không có G1..G6; câu "v1 có 6 lỗi thiết kế gốc" (v2:34) không truy vết được.
- Cột "Hướng v2" trong bảng §1 là thiết kế mới, không phải dossier.
- `v2:899` ghi "tối đa 3 QEMU song song [DOSSIER]". Dossier chỉ nói **3 luồng soak song song** (dos:1502, dos:1828), không nói 3 QEMU. Đây là quy đổi của người viết, không phải dữ kiện dossier.

### 1.4 `[ƯL]` bị dùng cho hằng số thiết kế, không phải ước lượng

`v2:212` ("tối đa 2^20 slot"), `v2:213` ("slot cap 32 byte"), `v2:257` ("có thể 6 MR") là **quyết định/giả định thiết kế**, không phải "con số ước lượng phải đo lại". Gắn `[ƯL]` làm chúng trông như số đo tạm, trong khi chúng sẽ đi thẳng vào `abi.toml` ở K0.

### 1.5 Số lượng lớn không có nhãn

Các con số sau không có nhãn nào: **14 syscall** (v2:55, 391–406), **4 cap/message** và **64 MR** (v2:256–258), **20 mã lỗi** (v2:383), **256 mức ưu tiên** (v2:250), **K=8 refill** (v2:245), **256 hart** (v2:359), **750 boot** (v2:780, 786, 952), **100.000 seed** (v2:782, 784), **10%** ngưỡng hiệu năng (v2:743), **800/1.500/15.000/2.500 dòng** (v2:165, 614, 619), **90 ngày** (v2:834). Với một tài liệu điều khiển, đây là lỗi nhãn nghiêm trọng, không phải lỗi trình bày.

**Kết luận mục 1:** quy ước nhãn của v2 tự nó không đủ để phân biệt "dossier", "thiết kế mới", "ước lượng"; và được dùng không nhất quán. Đây là **BLOCKER** vì toàn bộ khả năng truy vết của tài liệu dựa vào nó.

---

## 2. Đối chiếu định lượng với dossier

| # | Khẳng định của v2 | Dossier | Kết luận |
|---:|---|---|---|
| 1 | **25 crate**: v2 §8 (v2:592–611) không nêu con số, cây crate có ~33 tên; 14 crate cũ biến mất (`vifast-abi`, `vifast-kernel`, `vifast-runtime`, `vifast-user`, `vifast-maint`, `vifast-arch*`, `vifast-platform-*`, `vifast-root/server/client/bin`, `vifast-fsutil`, `vifast-kernel-*`) | dos:239–255: bản đồ **25 crate** | Không có bảng ánh xạ tên cũ→mới. Nguy hiểm hơn: `v2:64` nói "giữ ... `vifast-fsutil`" nhưng cây §8 **không có** `vifast-fsutil`. **MAJOR** |
| 2 | **58 syscall**: chỉ nhắc ở chẩn đoán (v2:55); ABI mới có **14** syscall 0..13 (v2:391–406) | dos:1140–1204, 1558, 2155: **58** syscall 0..57 | Đổi 58→14 là thiết kế mới, không nhãn; **đánh số lại** (v1 `0=Send, 1=Call`; v2 `0=Call, 1=Send`) nên mọi tham chiếu số syscall của v1 sai. Không có bảng ánh xạ 58→14. **BLOCKER** cho ABI |
| 3 | **KError**: 20 tên (v2:383) | dos:1078–1104, 2155: **22** giá trị `0..21` | V2 không nêu "22", không nêu "20"; đổi gần như toàn bộ tên: `InvalidCap→InvalidCapability`, `WrongType→WrongObjectType`, `NoRights→InsufficientRights`, `OutOfMemory→NotEnoughMemory`, `HasDependents→HasChildren`, `NotImplemented→NotSupported`; thêm `WxViolation`, `SlotEmpty`, `RangeError`, `Restart`; bỏ `NoCaller`, `ObjectDestroyed`, `IrqInUse`, `PrioTooHigh`, `WouldBlock`, `NotFound`. Chỉ 8 tên trùng dossier. Dossier còn cảnh báo `NoRights` có hai nghĩa (dos:1105–1107) — v2 tách `WxViolation` nhưng không nói đây là sửa bài học hay đổi ABI. **BLOCKER** |
| 4 | **Nguyên tắc**: "Mười hai nguyên tắc" (v2:70) | dos:155–163: **Năm nguyên tắc** đang hiệu lực | 5→12 là thiết kế mới, không nhãn. V2 bỏ nguyên tắc "Test là hợp đồng; `harness_spec.py` là nguồn sự thật duy nhất" — đây là nguyên tắc bị mất, không phải mở rộng. **MAJOR** |
| 5 | **Trần tĩnh**: "Trần 566 KB, 24 TCB" chỉ ở chẩn đoán G5 (v2:59) | dos:342–352, 1559, 1927: `MAX_OBJECTS=168`, `MAX_TCBS=24`, `KERNEL_STATIC_BUDGET=566.272 B`, 4 hart | V2 không đặt ngân sách tĩnh cho chính nó; không nhắc 168 object; không nhắc `const assert`/guard runtime. **MAJOR** |
| 6 | **Hart**: "`MAX_HARTS` cấu hình tối đa **256**" (v2:359); sim tới **64** nightly (v2:709); gate smp 1/2/4/8 (v2:784, 786) | dos:184, 350, 1340, 1367: `MAX_HARTS=4`, scheduler **mới chứng minh trên 2**, 3–4 hart chưa làm | 256 là bịa thêm không nhãn; không có bằng chứng, không có kế hoạch đo 256. **MAJOR** |
| 7 | **Mốc K0..K6**: K0, K1a/b/c, K2..K6 được định nghĩa (v2:778–789) | Không có hệ mốc này trong dossier (dossier dùng M1–M5, §16) | Hệ mốc mới là hợp lý, nhưng **K1** được dùng 11 lần (v2:345, 346, 412, 731...) mà **không được định nghĩa**; chỉ có K1a/b/c. "Cổng K1" (v2:731) không rõ là cổng nào. **MAJOR** |
| 8 | **U1, VSH2, VS3, G3**: có bảng (v2:795–798, 815–819) | Không có trong dossier; đây là mốc mới | Chấp nhận được, nhưng **G3 trùng tên**: G3 là "Trạng thái TCB tách khỏi hàng đợi" ở v2:57 và là "Toolkit Rust" ở v2:817; G1/G2 cũng trùng tương tự. **MAJOR** |
| 9 | **Quyết định D-01..D-27**: v2 chỉ có **D-09..D-27** (v2:922–940) | dos:2012–2023: **D-01..D-08 đang hiệu lực**; dos:2027–2031: D18, D11, quy tắc 2.5 | 8 quyết định cứng bị bỏ; D-09..D-27 là 19 quyết định mới không có trong dossier. Tệ hơn: **D-11** của v2 là "gate --fast cho kernel/lib" (v2:924), còn **D11** của dossier là "không sửa mò" (dos:2029); **D-18** của v2 là "thang S1–S4" (v2:931), còn **D18** của dossier là "hồi quy thì revert" (dos:2027). Trùng mã, khác nghĩa. **BLOCKER** |
| 10 | **Task T01..T16** (v2:950–967) | Không có; dossier dùng mã lỗi T33a/T26/T31/T-IPC | Không có bảng ánh xạ T-xx mới với lỗi cũ. T33a chỉ xuất hiện như triệu chứng (v2:34, 56); **T26, T31, T-IPC biến mất hoàn toàn**. **BLOCKER** |
| 11 | **750 boot** cho V1-END/K3/T01 (v2:780, 786, 952) | Không có số 750. Tiêu chí T33a là **600 boot x86_64 song song 3 luồng**, 0 treo, 0 `TIME MONOTONIC BAD`, 0 `ROOT PANIC`, có test host (dos:1667–1669); D-06 là **600–1000 boot trên máy `ps`** (dos:2021) | 750 là số bịa, không nhãn, không nêu máy, không nêu song song. **MAJOR** |
| 12 | **"hợp đồng marker có case âm" được giữ** (v2:64) | dos:1398–1431: 55 COMMON / 29 EXACT_ONE / 3 ARCH / 38 FORBIDDEN / 7 FAULT_ADDRESSES / 1 ORDER / 3 NUMERIC / 3 EXIT_CODES, **677 case âm** | §9 của v2 **không có** marker contract, không có `harness_spec.py`, không có 677 case âm, không có FORBIDDEN/EXACT_ONE; chỉ có TAP (v2:700). Tuyên bố "giữ" là sai. **BLOCKER** |
| 13 | **Test/gate**: L0 <15 s, L1 10.000 seed <60 s, L2 <5 phút, L3 <2 phút/arch (v2:643–647) | dos:1568: **402** host test; dos:1575: gate đầy đủ **396 s** đo trên **một** máy; dos:1578: "3/3 PASS" là **có retry** | V2 thay số dossier bằng ngưỡng mới không nhãn, không nhắc ba chỗ "chưa chốt" (402 vs 388/391; 396 s một máy; PASS có retry). **MAJOR** |
| 14 | **`MAX_IO`**: không xuất hiện | dos:1563: `MAX_IO=128`, 256 chỉ nhánh đo; D-04 (dos:2019); T26 đo ở 256 (dos:1678) | V2 xoá `MAX_IO`, nên **cấu hình tái hiện T26 biến mất**. **MAJOR** |
| 15 | **3 QEMU song song [DOSSIER]** (v2:899) | dos:1502, 1615, 1828: **3 luồng soak** song song, không phải 3 QEMU | Nhãn `[DOSSIER]` sai. **MINOR** |
| 16 | **T-INFRA**: "chạy lại tối đa **một** lần" (v2:874); "không retry ở mọi cổng" (v2:768) | dos:1723: T-INFRA "chạy lại tối đa **2** lần"; dos:1462: retry mặc định 1 | V2 vừa siết (no retry) vừa nới/đổi (1 lần) và tự mâu thuẫn với chính nó. **MINOR** |
| 17 | **6 lỗi thiết kế gốc** (v2:34) | dos:1787: **4 nguyên nhân gốc**; dos:1922–1940: 15 nợ kỹ thuật | Đổi trục phân loại (4→6) không nhãn; G1..G6 không có trong dossier. **MINOR/MAJOR** |
| 18 | **5 luồng điển hình** | dos:305–334: **năm luồng** (boot, IPC, fault, exec, IRQ...) | V2 không có bảng luồng; không sai, nhưng mất một mô hình đọc nhanh. **MINOR** |
| 19 | **Rights**: READ, WRITE, GRANT, GRANT_REPLY; "EXEC là thuộc tính lúc map" (v2:215) | dos:403, 717–725, 1111: `READ|WRITE|EXEC|GRANT` (4 bit), `FrameMap` kiểm `EXEC`; W^X theo Frame | Đổi mô hình quyền không nhãn; `mapped_x` "giữ từ v1" (v2:296) nhưng quyền không còn `EXEC`. **MAJOR** |
| 20 | **Message**: `len u7 (≤64 MR)`, 4 cap (v2:256–258), IPC buffer | dos:1121–1126: `data:[u64;8]` = **8 word**, `cap:u64` = **1 cap**; dos:1229–1231: chưa có shared-memory message chuẩn, Reply không mang cap | Mở rộng 8→64 MR và 1→4 cap là thay đổi ABI lớn, không nhãn, không nêu lý do/chi phí. **MAJOR** |

**Tổng hợp mục 2:** các con số lớn của dossier (25 crate, 58 syscall, 22 KError, 168 object, 24 TCB, 566.272 B, 4 hart, 128 `MAX_IO`, 402 test, 677 case âm) hoặc bị đổi không nhãn, hoặc bị bỏ mà không tuyên bố. Chỉ một số ít (58, 566 KB, 24 TCB) còn xuất hiện, và chỉ trong phần chẩn đoán v1, không phải trong ràng buộc v2.

---

## 3. Mâu thuẫn nội bộ của v2

### 3.1 ID trùng / thiếu / tham chiếu sai

1. **G1–G3 trùng nghĩa**: G1..G6 là lỗi gốc (v2:55–60); G1..G5 là mốc desktop (v2:815–819). Cùng một ký hiệu G3 vừa là "TCB state tách khỏi queue" vừa là "Toolkit Rust". Không thể grep an toàn.
2. **K1 không định nghĩa**: dùng ở v2:345, 346, 412, 731; §10.1 chỉ có K1a/b/c (v2:782–784). "Cổng K1" (v2:731) không có tiêu chí.
3. **§4.16 dẫn sai mục**: "Khi chờ khoá > ngưỡng (**mục 9.9**)" (v2:347) nhưng ngưỡng nằm ở **§9.10** (v2:743); §9.9 là Mutation.
4. **"perf" là phụ thuộc không tồn tại**: S3 và F1 phụ thuộc "perf" (v2:828, 829), nhưng không có mốc/ID "perf"; chỉ có §9.10.
5. **T04 "xong = BLOCKED"**: tiêu chí xong của T04 là "Gửi Agent 0 duyệt (BLOCKED tới khi duyệt)" (v2:955), trong khi §11.8 định nghĩa xong là tiêu chí đạt + cổng xanh (v2:909–914). Một task không thể vừa "xong" vừa "BLOCKED".
6. **T16 phụ thuộc "tất cả"** (v2:967) trong khi T04 chờ duyệt và T05..T15 có thể BLOCKED (v2:969): R1 không bao giờ sinh được nếu T04 chưa duyệt, trái với §11.6 "hết task thì viết báo cáo" (v2:891).

### 3.2 Enum và trạng thái

7. **Thiếu trạng thái chờ refill**: §4.5 nói "hết budget: TCB vào hàng chờ refill" (v2:246), nhưng `ThreadState` (v2:223–235) không có `BlockedOnRefill`/`BlockedOnBudget`; `Sleeping` không thay thế được vì ngữ nghĩa khác.
8. **Slot 32 byte không đủ chỗ**: `ObjRef` là `{pa: PAddr, id: u64}` (v2:169) = 16 byte; slot 32 byte còn phải chứa `kind`, `rights`, `badge` 8, `mdb_prev` 8, `mdb_next` 8, `mdb_depth` (v2:213) ⇒ tối thiểu 43 byte. Con số `[ƯL]` này không khả thi với cấu trúc đã mô tả.
9. **Arena "rất nhỏ" vs "vài trăm KB mỗi hart"**: v2:158 nói arena rất nhỏ; v2:451 nói vài trăm KB mỗi hart; với `MAX_HARTS=256` (v2:359) là **50–75 MB** static — trái với tinh thần 566 KB của dossier và trái với chính câu "rất nhỏ".

### 3.3 Lập luận và luật

10. **Không work stealing vì phải đọc trạng thái hart khác** (v2:280), nhưng "Wake chéo hart ... nếu ưu tiên cao hơn current của đích" (v2:281) cũng phải đọc trạng thái hart đích. Cùng một lý do bị áp dụng không nhất quán.
11. **Không đưa số viết tay** (v2:879) nhưng tài liệu đầy số viết tay: 566 KB/58/750/100.000/95%/10%/1.500/15.000/2.500. Tự vi phạm luật của chính mình.
12. **Trace "luôn bật"** (D-14, v2:927; §9.11 v2:747) nhưng `KernelConfig` có feature `trace` (v2:433) — có thể tắt, tức không "luôn bật".
13. **GPU/IME bị cấm bởi D-08** (dos:2023) nhưng v2 mở G1–G5 và G4 (v2:811–819). Không có dòng nào nói D-08 hết hiệu lực.

### 3.4 Queue §13 và lộ trình §10

14. **Thiếu task cho K1a/K1b/K1c**: K1a cần "finalise khung" (v2:782) nhưng không có task finalise; K1b cần "scheduler, hộp thư, MCS" (v2:783) nhưng không có task `vifast-sync`, scheduler, MCS; K1c cần "IPC + Reply + Notification + WaitAddr + fault" (v2:784) nhưng không có task IPC. Queue chỉ tới T16 và không chạm các mảng này.
15. **Phụ thuộc vòng T12↔T14**: T12 "SimPlatform" có tiêu chí "dump trace" (v2:963), nhưng `vifast-traceformat` là T14 và **phụ thuộc T12** (v2:965). Không thể dump trước khi có định dạng trace.
16. **T15 thiếu phụ thuộc T14**: `vifast-shrink` cần trace để thu nhỏ (v2:966), nhưng T15 chỉ phụ thuộc T12; có thể chạy trước khi có traceformat.
17. **T11 thiếu phụ thuộc T05**: ThreadState chứa `ObjRef<Endpoint>`, `ObjRef<Reply>`... (v2:223–235), cần kiểu từ `vifast-abi2` do T05 sinh (v2:956); T11 chỉ phụ thuộc T08 (v2:962).
18. **Kích thước task vượt ngân sách**: §11.5 mặc định **3 giờ/task** (v2:887), §11.6 yêu cầu task **1–3 giờ** (v2:896); nhưng T02 (toàn bộ workspace ~33 crate), T04 (toàn bộ `abi.toml`), T12 (scenario + sim + scheduler + 4 checker) rõ ràng nhiều ngày. Không có task nào được chia nhỏ.
19. **K6 "tương đương v1"** (v2:789) không có định nghĩa đo được, trong khi v1 có 58 syscall, 402 test, 677 case âm, VFS VF04 với journal/fsck/rename/truncate/double-indirect (dos:1540, 1568). V2 không nói "tương đương" đo bằng gì.
20. **V1-END trước khi commit r323**: header nói "HEAD 8077d71 + mẻ r323 **chưa commit**" (v2:3), nhưng V1-END ghi "Commit r323" (v2:780); T01 mới là task commit (v2:952). Thứ tự không nhất quán.

### 3.5 ABI và D-22

21. **ABI chốt ở K0 nhưng thiếu method**: D-22 nói ABI lõi chốt ở K0, sau chỉ thêm (v2:935). Nhưng bảng §4.21:
   - `VCpu` chỉ có `Run, ReadRegs, WriteRegs, InjectIrq, SetTsc` (v2:424) — **không có Configure** (entry, VSpace khách, tham số stage-2), không thể chạy VCpu.
   - `IOSpace` chỉ có "(Map qua Frame), Flush" (v2:423) — **không có method attach theo BDF/stream ID**, dù §4.12 nói IOSpace gắn với thiết bị (v2:316).
   - `VSpace` chỉ có `GetAsid (debug)` (v2:418) — không có tạo/cấu hình VSpace khách cho H1.
   Đây là ABI không đủ để cài H1/D3, tức D-22 sẽ bị vi phạm ngay khi làm các mốc sau.
22. **Lỗi mã `Restart` "không bao giờ tới user"** (v2:383) nhưng §4.13 nói syscall trả `Restart` để platform quay lại lệnh (v2:323); nếu user thấy `Restart` thì trái câu trên; nếu platform thấy thì cần bit/ABI riêng, chưa định nghĩa.

---

## 4. Ràng buộc cứng và bài học đắt: cái gì bị mất, bị nhẹ, bị mâu thuẫn

### 4.1 Tám quyết định D-01..D-08

| Mã | Dossier | V2 xử lý | Đánh giá |
|---|---|---|---|
| **D-01** | Giữ gate mềm; `--strict` chỉ dùng khi đo (dos:2016) | Không nhắc; thay bằng "không retry ở mọi cổng" (v2:768) | Siết hơn nhưng **không nói D-01 hết hiệu lực**, không nhắc `--strict`/`RETRY n/N`; máy đo chưa ổn định vẫn là rủi ro. **MAJOR** |
| **D-02** | Đóng băng tính năng, chỉ sửa lỗi + đo (dos:2017) | Mở K0–K6 + 5 dòng sản phẩm (v2:776–830) | **Đảo ngược hoàn toàn**, không ghi lý do hay ngày hết hiệu lực. **MAJOR** |
| **D-03** | T26: đánh "ai chen giữa", không đánh fsck (dos:2018) | T26 **không tồn tại** | Mất cả lỗi lẫn kỷ luật điều tra. **BLOCKER** |
| **D-04** | Đo ở `MAX_IO=256`, không bật ở gate (dos:2019) | `MAX_IO` biến mất | Mất cấu hình tái hiện T26; không còn cách đo 20% đỏ. **BLOCKER** |
| **D-05** | Teardown tối đa 2 chỗ, mỗi chỗ một vòng, hồi quy revert (dos:2020) | §4.2 finalise **12 loại object** trong một bảng (v2:191–206); K1a gộp finalise khung (v2:782) | Không còn giới hạn 2 chỗ; nguy cơ lặp lại "gộp teardown hỏng 0/3 arch". **MAJOR** |
| **D-06** | Soak ~600–1000 boot x86_64 trên máy `ps` (dos:2021) | 750 boot, không nêu máy (v2:780, 952) | Mất ràng buộc máy/N; 750 không có cơ sở. **MAJOR** |
| **D-07** | Không đụng nợ kiến trúc, không tách `lib.rs` (dos:2022) | Rewrite tách crate (v2:592–611) | Về lý thuyết v2 thay thế v1 nên D-07 hết đối tượng, nhưng phải nói rõ; hiện không nói. **MINOR** |
| **D-08** | Không đụng đồ hoạ/ViFastKey (dos:2023) | Mở G1–G5, G4 (v2:811–819) | **Đảo ngược**, không ghi lý do. **MAJOR** |

Ngoài ra **D18** (hồi quy ⇒ revert ngay, dos:2027) và **D11** (không sửa mò, dos:2029) bị **trùng mã** bởi D-18/D-11 mới của v2, khác nghĩa. **Quy tắc 2.5** (hai phiên kẹt cùng ý ⇒ đổi nhánh, không thử lần ba, dos:2030) bị làm nhẹ thành "kẹt hai lần ⇒ BLOCKED, chuyển task kế" (v2:888) — không còn yêu cầu đổi nhánh.

### 4.2 Luật đo lường §13

- **Mất**: hợp đồng marker (55/29/3/38/7/1/3/3), 677 case âm, `FORBIDDEN`, `EXACT_ONE`, `BUILD_ID` "mọi dòng BUILD cùng id, lệch = T-BUILD không tính" (dos:1476–1487), phân loại 5 lớp T-BOOT/T-FS/T-SCHED/T-KHAC/T-BUILD (dos:1714–1723), "0/N chỉ là trần" (dos:1513), "không đếm artifact" (dos:1514).
- **Còn một phần**: §9.12 giữ "chốt n trước, cùng BUILD_ID, cùng máy" (v2:755) nhưng **bỏ "cấu hình (smp, MAX_IO)"** (dos:1512); §9.8 có in BUILD_ID (v2:735) nhưng không có luật T-BUILD.
- **Mâu thuẫn nội bộ**: §9.12 nói `n ≥ 3/p` (v2:755), nhưng V1-END/K3 dùng n=750 (v2:780, 786). Với p=0,3–0,5% (tỉ lệ T33a), n=750 chỉ cho cận trên ~0,4%, **không đủ** để nói "đã giảm" so với 0,3–0,5%; muốn tuyên bố dưới 0,3% cần n≥1000. Chính v2 tự bác con số 750 của mình.
- **Mất luật "không so sánh chéo máy nếu khác BUILD_ID"** (dos:1504–1505) — v2 chỉ nói "cùng máy" ở §9.12, không nói điều kiện BUILD_ID cho so sánh.
- **Mất "test đỏ-trước" cho bản vá?** V2 giữ ở §9.8 (v2:735) và §11.3 (v2:863), tốt.

### 4.3 Quy ước kỹ thuật §19.3

- **`unsafe` gom một chỗ**: dossier yêu cầu kernel/runtime `forbid(unsafe_code)`, `unsafe` chỉ ở `vifast-arch-*`, `vifast-platform-*`, entry asm (dos:2042–2043). V2 mở `vifast-objmem` và `vifast-sync` là crate lib có unsafe (v2:598–599, 609), dùng trong kernel **và sim**. V2 có D-20/D-23 biện minh (v2:933, 936) nhưng **không nói đây là thay đổi quy ước §19.3**, không nêu phạm vi/kiểm soát mới. **MAJOR** — bề mặt unsafe tăng từ 2 nhóm arch/platform lên 2 crate lõi.
- **`COPY_WINDOWS`/cửa sổ copy user**: dossier coi là "bài học đắt" (exec lỗi vòng 112→145 chỉ vì thiếu một cửa sổ VA, dos:766–793; quy tắc C1/C2; luật "mọi thay đổi layout user phải cập nhật COPY_WINDOWS", dos:2047). V2 bỏ `COPY_WINDOWS` vì "không con trỏ user trong kernel" (v2:58, 75), nhưng:
  - vẫn có **IPC buffer** do kernel copy (`v2:258`) và **WaitAddr đọc giá trị tại Frame+offset** (`v2:269`);
  - không có luật kiểm cận/tràn/“một cửa sổ” tương đương C1;
  - không có const assert/test canh layout.
  Đây là **mất bài học đắt** dưới hình thức mới. **MAJOR**.
- **`cfg(target_arch)` cấm trong core/lib**: v2 giữ (v2:615), tốt.
- **Không build tay đè kernel cổng**: v2 không nhắc; có thể mất bài học "thiếu `test-exit` ⇒ thiếu `PREEMPT OK`" (dos:2045). **MINOR**.

### 4.4 Nợ kỹ thuật §17

V2 **giải quyết tốt** nhiều mục: #1 tách module (v2:592–611), #3 BKL trước (v2:345), #4 IRQ thay polling (v2:306–312), #7 CSpace 2 cấp (v2:212), #8 ASID (v2:298), #9 id u64 (v2:180), #10 O(1) pick (v2:278), #11 sim tất định (v2:707–714), #13 test matrix (v2:653–680), #14 FPU eager (v2:329), #15 init mảnh (v2:498–500). Đây là điểm mạnh thật.

Nhưng:
- **#2 trần tĩnh**: v2 tuyên bố "không còn `ObjectPoolExhausted`; giới hạn chỉ là RAM" (v2:158). Dossier Q8 nói phương án tách pool **đã thử 3 lần, hỏng AArch64 vì layout `.data`** (dos:2096–2097; dos:1927). V2 **không nhắc lịch sử 3 lần thất bại**, không có [GT], không có kế hoạch đo headroom. **MAJOR/BLOCKER**.
- **#5 IOMMU**: dossier nói chỉ có software IOMMU, hardware chưa có, DMA thật không được bảo vệ (dos:1930; dos:1543). V2 **bỏ hẳn software IOMMU** (v2:429) và thay bằng "driver DMA tin cậy trong manifest" (v2:318). Trên mọi QEMU hiện tại (chưa D3) và mọi máy thật (HW1), không có IOMMU phần cứng; chính sách "tin cậy" không phải cơ chế bảo vệ. **MAJOR** — tuyên bố bảo mật §4.15 không còn đúng cho DMA.
- **#6 allocator vật lý**: v2 đẩy sang memsrv (v2:504), hợp lý; nhưng chưa nói phân mảnh/revoke Untyped con. **MINOR**.

### 4.5 Lỗi mở §15

| Lỗi | Dossier | V2 | Đánh giá |
|---|---|---|---|
| **T33a** | Gốc lộ một nửa; tiêu chí xong: 600 boot song song 3 luồng, 0 treo, 0 `TIME MONOTONIC BAD`, 0 `ROOT PANIC`, **có test host giải thích cơ chế** (dos:1667–1669) | Chỉ nhắc như triệu chứng (v2:34, 56); T01 chỉ yêu cầu **báo cáo** n, số treo, số marker (v2:952); V1-END "soak n=750, có báo cáo" (v2:780) | **Làm nhẹ tiêu chí**: có thể tag `v1-final` dù còn treo, miễn có báo cáo. Trái "chặn mọi tuyên bố ổn định" (dos:107). **BLOCKER** |
| **T26** | 20% boot đỏ ở `MAX_IO=256`; cơ chế chưa vá; cấm khoá thô không test đỏ-trước (dos:1675–1692) | **Không tồn tại**; v2 vẫn giữ `vifast-fs` (v2:64) và VFS page cache (v2:515) | Lỗi FS sống trong thành phần được tái sử dụng; không có repro, không có lệnh cấm. **BLOCKER** |
| **T31** | PAGER2 nhận message lặp, chưa có gốc (dos:1694–1705) | Không tồn tại; chỉ có checker "message giao đúng một lần" chung (v2:721) | Mất ca lỗi cụ thể; checker chung không thay thế được repro. **MAJOR** |
| **T-IPC** | Bài học: "lỗi sản phẩm" có thể là lỗi cổng đo; kiểm đường thoát trước (dos:1707–1712) | Không nhắc | Có thể lặp lại khi viết harness mới. **MAJOR** |
| **x86 shootdown AP→BSP + PCID** | Lỗi đúng đắn, chưa sửa, chỉ lộ KVM/`-cpu max` (dos:1725–1732) | Có thiết kế shootdown chờ ack + ASID generation (v2:298–303); K2 có đột biến AP→BSP (v2:785) | **Tốt**, nhưng chưa có tiêu chí "chứng minh TLB cũ gây đọc sai" như dossier. **MINOR** |
| **`--strict`** | Chưa bật, chặn mọi kết luận (dos:1734–1741) | Không nhắc; thay bằng no-retry (v2:768) | No-retry mạnh hơn, nhưng không có `RETRY n/N`/T-BUILD; cần nói rõ thay thế. **MINOR** |
| **Audit Q1-A, P1-5, P1-6, P1-7** | 3 OPEN + 1 mâu thuẫn, trong đó Q1-A "kênh wake chưa thống nhất 3 port", P1-5 "enqueue trả sớm ⇒ Ready nhưng không trong runqueue" (dos:1750–1751) | Không nhắc; v2 thiết kế lại đúng mailbox (v2:361–366) và ready queue (v2:277) | **Đúng các subsystem bị viết lại**, nhưng không mang theo ca lỗi. **MAJOR** |
| **§15.10 "điều kiện coi là đã sửa"** | Áp cho mọi lỗi hiếm (dos:1762...) | Không có tương đương; §11.8 chỉ định nghĩa "xong" theo task | Mất tiêu chí đóng lỗi. **MAJOR** |

### 4.6 Câu hỏi mở §20

Dossier đặt Q1–Q9 như **agenda để quyết**, không phải quyết định đã chốt (dos:2051–2098). V2 không trả lời từng câu, nhưng **âm thầm chọn**:
- Q1: chọn (c) mở nhánh sản phẩm, dù dossier nói (c) chỉ sau (b) (dos:2059);
- Q3: coi T33a là đã hiểu (G2), dù dossier còn 4 giả thuyết (dos:1652–1665);
- Q5: bỏ T26 khỏi lộ trình;
- Q6: bỏ `--strict`;
- Q7: rewrite thay vì tách `lib.rs`;
- Q8: chọn (b) tách pool, dù dossier ghi đã thử 3 lần hỏng (dos:2096–2097);
- Q9: không nhắc đóng băng x86.

Đây là **mất truy vết quyết định**: người điều phối không biết agent0 đã quyết hay chỉ bỏ qua. **MAJOR/BLOCKER**.

---

## 5. Hứa hẹn vượt bằng chứng

1. **"V1 đóng băng sau r323"** (v2:34) trong khi T33a/T26/T31 còn mở và chưa có tiêu chí đóng. Dossier nói T33a "chặn mọi tuyên bố ổn định" (dos:107) và T26 "chặn mọi tuyên bố FS đúng" (dos:108). **BLOCKER**.
2. **"750 boot 0 lỗi"** là tiêu chí K3 (v2:786) và V1-END (v2:780). Với tỉ lệ 0,3–0,5% dưới soak song song, 750 boot **không đủ** để kết luận đã sửa; dossier yêu cầu 600 boot song song 3 luồng **và** test host giải thích cơ chế (dos:1667–1669). V2 biến một phép đo yếu thành cổng mốc. **MAJOR**.
3. **Lộ trình 90 ngày**: K0–K2 trong 90 ngày đầu, K3–K5 trong 90 ngày tiếp (v2:834). Dossier ghi 11 vòng/973 commit cho **một** lỗi, gate 396 s, soak vài giờ (dos:1779–1782, 114–118). Không có cơ sở năng suất, không có buffer, không có [GT] cho từng mốc (chỉ [ƯL] cho cả §10.6). **MAJOR**.
4. **K6 "tương đương v1"** (v2:789): v1 có 58 syscall, VF04 với journal/fsck/rename/truncate, 402 test, 677 case âm, pager 4 client, exec chain (dos:1540–1552, 1568). V2 không định nghĩa "tương đương", không liệt kê tính năng nào phải có. Đây là mục tiêu không đo được. **MAJOR**.
5. **"MCS bảo đảm độ trễ" cho audio** (v2:548): budget/period không tự bảo đảm latency; cần phân tích schedulability, đo worst-case, và driver ring. Không có [GT]/số đo. **MINOR/MAJOR**.
6. **"Không còn `ObjectPoolExhausted`; giới hạn chỉ là RAM"** (v2:158): bỏ qua 3 lần thử tách pool thất bại trên AArch64 (dos:2096–2097), bỏ qua bài học "thêm một task là thay đổi kiến trúc" (dos:1927). Đây là tuyên bố mạnh nhất của v2 và là **ước lượng không có bằng chứng**. **BLOCKER**.
7. **`vifast-core` dưới 15.000 dòng** (v2:614) trong khi v1 kernel portable 19.989 dòng (dos:244) và v2 **thêm** MCS, objmem, CSpace 2 cấp, Reply, WaitOnAddress, IOSpace, VCpu. Giảm 25% dòng trong khi tăng tính năng là không tương thích; [ƯL] không cứu được vì đây là cổng cấu trúc. **MAJOR**.
8. **Rust std đầy đủ + không libc** (v2:556–560, 578): dossier nói libc/dynamic linking/ứng dụng thật **chưa có** (dos:1600–1601); std target mới là dự án lớn, không có task riêng ngoài U1. **MAJOR**.
9. **Linux khách tới shell ở H1** (v2:826): dossier xếp tương thích Linux là dự án riêng/hướng VM (dos:147–148), chưa có VMM; v2 chỉ có [GT] cho virtio-gpu cross-domain (v2:582), không có [GT] cho toàn bộ H1. **MAJOR**.
10. **"Mọi thứ một OS desktop thực tế cần" được chốt ở K0** (v2:35): ABI K0 thiếu `VCpu.Configure`, `IOSpace.Attach`, `VSpace` khách; KError bị đổi; message MR/cap mở rộng. Tuyên bố "đủ" không đúng với chính bảng ABI của v2. **BLOCKER**.
11. **Bảo mật**: bỏ software IOMMU (v2:429), KPTI "tuỳ chọn" (v2:335), DMA tin cậy theo manifest (v2:318). Không có threat model, không nói máy không IOMMU bị gì. **MAJOR**.

---

## 6. Những điểm v2 làm tốt (để công bằng)

- Kế thừa đúng mô hình seL4, object trong Untyped, `forbid(unsafe_code)` cho core, W^X theo Frame, `check_invariants` + mutation, test đỏ-trước (v2:64, 718–725).
- Sim tất định + mô hình tham chiếu + trace ring + shrink + scenario chạy hai nơi (v2:682–714) đúng hướng dossier §16.4 A/B/C.
- MCS từ đầu, Reply object, passive server, WaitOnAddress, preemptible ops: các lựa chọn thiết kế hợp lý, có lý do.
- Đối xử với shootdown/ASID tốt hơn v1 (v2:298–303), có đột biến AP→BSP (v2:785).
- Queue có ID, phụ thuộc, tiêu chí xong, ngân sách, báo cáo JSON (v2:846–907) — khuôn tốt, chỉ cần sửa nội dung.
- Nhận diện đúng 6 lỗi lớn của v1 ở mức mô tả (dù con số 6 và nhãn `[DOSSIER]` sai).

---

## 7. Điều kiện tối thiểu để v2 dùng được làm tài liệu điều khiển

### P0 — phải sửa trước khi giao Agent 2

1. **Khôi phục D-01..D-08**: thêm phụ lục "ràng buộc đang hiệu lực" chép nguyên trạng dos:2012–2023; đánh dấu cái nào hết hiệu lực, ngày/lý do. Đổi D-09..D-27 thành mã mới không trùng (ví dụ `V2-D01..`) hoặc giữ D-09..D-27 nhưng ghi rõ D-01..D-08 vẫn hiệu lực. Sửa D-11/D-18 collision.
2. **Mở sổ lỗi**: bảng T33a/T26/T31/T-IPC/x86-PCID/audit Q1-A/P1-5/P1-6/P1-7 với trạng thái, tiêu chí đóng (dos:1667–1669, 1690–1692, 1731–1732), và ca kiểm tương ứng trong v2. Không được tuyên bố "v1 đóng băng" nếu chưa đóng hoặc chưa ghi "bỏ có chủ ý".
3. **Hệ nhãn**: định nghĩa `[DOSSIER:CODE]`, `[DOSSIER:ĐO]`, `[DOSSIER:DOC]`, `[THIET-KE]`, `[GT]`, `[ƯL]`; mặc định `[THIET-KE]` cho mọi dòng không nhãn; gắn nhãn lại toàn bộ con số ở §2 bảng trên. `[DOSSIER]` chỉ dùng khi có dòng dossier tương ứng.
4. **ABI v0 đầy đủ**: chốt 14 syscall + method + 20 KError (hoặc 22 nếu giữ) với bảng ánh xạ từ 58/22 của v1; thêm `VCpu.Configure`, `IOSpace.Attach`, `VSpace` khách; quyết định 8→64 MR/1→4 cap có lý do và test; nếu D-22 giữ, phải chứng minh không thiếu method.
5. **Queue §13 chạy được**: thêm task finalise, `vifast-sync`, scheduler, MCS, IPC/Reply/Notification/WaitAddr; sửa T12/T14 (tách traceformat ra trước hoặc bỏ "dump trace" khỏi T12), T15→T14, T11→T05; T16 không phụ thuộc T04; chia T02/T04/T12 thành task ≤3 h.
6. **Đo lường**: khôi phục luật §13.8 (N, máy, BUILD_ID, cấu hình, 0/N là trần, không đếm artifact); bỏ 750 hoặc biện minh bằng `n≥3/p`; K3 dùng 600 boot song song 3 luồng + test host cơ chế; ghi rõ no-retry thay `--strict`/RETRY.

### P1 — nên sửa cùng lúc

7. Bảng ánh xạ 25 crate cũ → crate mới; thêm `vifast-fsutil` hoặc bỏ khỏi "giữ" (v2:64); nêu `vifast-maint`→`vifast-sync`.
8. Khôi phục hợp đồng copy/validate: luật C1/C2 cho IPC buffer và WaitAddr, const assert layout, test.
9. Chính sách IOMMU: giữ software IOMMU hoặc nêu rõ DMA không bảo vệ + threat model + mốc D3/HW1.
10. Khôi phục D-05: chia finalise theo nhóm ≤2 chỗ, mỗi nhóm một vòng, có test.
11. Sửa ID: G1..G6 → R1..R6 (root cause) hoặc đổi mốc desktop; định nghĩa K1 = K1a+b+c; sửa "mục 9.9"→"9.10"; thay "perf" bằng §9.10/ID.
12. Sửa slot 32B (nới lên 48/64 hoặc nén ObjRef), thêm trạng thái chờ refill vào ThreadState, làm rõ "arena nhỏ" vs "vài trăm KB/hart" và ngân sách static mới.

### P2 — vệ sinh

13. Sửa `[DOSSIER]` ở v2:899 (3 luồng soak, không phải 3 QEMU); "6 lỗi gốc" → "6 nhóm lỗi theo phân loại mới" hoặc dẫn dossier.
14. Sửa §4.8 (work stealing vs wake), §5.4 vs §8 (asm), D-14 vs feature trace.
15. Bổ sung console/tty server hoặc nói rõ đường input của shell tạm K6.

---

## 8. Kết luận ngắn

V2 là **bản thiết kế tốt hơn v1 về ý tưởng**, nhưng là **tài liệu điều khiển chưa đạt**: nó không mang theo các ràng buộc cứng (D-01..D-08), không mang theo lỗi mở (T26/T31/T33a/audit), đổi ABI lõi mà không nhãn, và có mâu thuẫn nội bộ ở ID/queue/đo lường. Nếu Agent 2 thi công theo v2 nguyên trạng, sẽ có nguy cơ:
- xây ABI 14 syscall/20 KError khác dossier, không tương thích bất kỳ test/trace/tài liệu v1 nào;
- tuyên bố "xong K6" mà không có định nghĩa;
- bỏ qua T26 trong FS được tái sử dụng;
- không có cổng marker/677 case âm, nên "PASS" lại là "đã từng pass";
- không có đường đóng T33a trước khi tuyên bố ổn định.

**Khuyến nghị:** không giao nguyên trạng. Sửa P0 (mục 7.1–7.6) rồi duyệt lại; P1 có thể làm song song. Sau khi sửa, v2 có tiềm năng trở thành tài liệu điều khiển tốt vì khung kỹ thuật và quy trình đã đúng hướng.

---

### Phụ lục: bảng ký hiệu dùng trong báo cáo

| Ký hiệu | Nghĩa |
|---|---|
| `v2:NNN` | dòng NNN của `ViFastOs-v2-thiet-ke.md` |
| `dos:NNN` | dòng NNN của `ViFastOs-dossier.md` |
| BLOCKER | Phải sửa trước khi Agent 2 thi công; nếu không, sai ABI/ mất ràng buộc/ không đo được |
| MAJOR | Sửa trong vòng 1; ảnh hưởng hiểu đúng hoặc tiêu chí mốc |
| MINOR | Sửa khi có dịp; không chặn thi công nhưng gây nhiễu |
| NIT | Vệ sinh tài liệu |
