# Agent 0 trả lời YC-02..05 (HEAD 9552832)

Người viết: Agent 0. Người thi hành: người điều phối (dán vào sổ, commit), sau đó Agent 2.
Tệp này **không phải duyệt KT-04d**. Duyệt KT-04d vẫn chỉ bằng dòng `APPROVED <ngày> Agent 0` sau khi KT-04b/c xong.

---

## 0. Kết luận

| YC | Đề xuất của gói | Agent 0 |
|---|---|---|
| YC-02 DebugGetInfo | syscall 15 | **Đồng ý**, thêm một ràng buộc: không bị feature gate |
| YC-03a "0..13 như cũ" | errata D2-30 | **Đồng ý** |
| YC-03b spec/ ở gốc | errata D2-31 | **Đồng ý** |
| YC-04 9 object reserved 8..16 | KT-04b ghi spec một lần | **Đồng ý có sửa 2 điểm**: SchedControl là lõi, không reserved; VSpace khách là cờ Retype, không phải method |
| YC-05 chữ ký hẹp | draft REVIEW cho 10 method | **Đồng ý có sửa**: enum TCB phải là enum đầy đủ §4.4, không phải tập con; thêm Tcb.ReadState |
| D2-34 uỷ quyền duyệt | tuỳ chọn | **Không ký** |

Agent 1 dừng đúng luật và đề xuất gọn, giữ được cổng xanh. Hai chỗ sửa ở YC-04 là lỗi thiết kế thật, nên sửa trước khi ghi spec.

---

## 1. Các dòng dán vào `v2/agent/DECISIONS.md` (bản cuối, thay bản trong gói)

**D2-29** DebugGetInfo = **syscall 15**, dải reserved syscall còn **16..31**. DebugGetInfo **luôn có**, không phụ thuộc feature `debug`/`test` (khác 12, 13), vì nó trả `abi_version`, `min_compat` và bitmap tính năng mà runtime dùng để kiểm tương thích lúc khởi động. Trả về: `abi_version`, `min_compat`, `features` (bitmap), `num_harts`. Không trả địa chỉ kernel. spec/abi.toml: thêm `[syscall].15`, `[syscall_reserved].from = 16`, `[debug_info].number = 15`, `status = "frozen"`; check_abi.py cập nhật tương ứng.

**D2-30** Câu QD §4 "0..13 như cũ" nghĩa là **như bảng §4.21 của thiết kế**, không phải giữ số v1. v2 đánh số lại; `v2/docs/abi-v1-v2-mapping.md` là nguồn đối chiếu từng số. QD giữ nguyên.

**D2-31** `spec/` ở **gốc repo** là nguồn sự thật; cây `v2/spec/` trong thiết kế §8 là lỗi vẽ cây, không dùng. Errata ghi tại đây, không sửa thiết kế.

**D2-32** KT-04b được ghi `spec/abi.toml` **một lần**, theo draft `v2/docs/abi-reserved-objects.toml` **sau khi sửa hai điểm**:
1. **SchedControl (8) là object lõi**, `stability = "core"`, không reserved: MCS donation có từ K1b (D2-16) và `SchedControl.Configure` đã nằm trong chữ ký draft D2-33. Không được vừa reserved vừa có chữ ký.
2. **VSpace khách không phải method.** Bỏ `VSpace.Configure (id 1)`. Khách hay thường là **kiểu bảng trang**, quyết định lúc tạo, không đổi sau: dùng **bit cờ `GUEST` trong tham số `flags` của `Untyped.Retype` khi type = VSpace**. Bit này reserved; Retype với `GUEST` trả `NotSupported` tới H1. Method id 1 của VSpace để trống.

Object reserved còn lại: VSpace 9, PageTable 10, Frame 11, IrqControl 12, IrqHandler 13, IOSpace 14, VCpu 15, PowerControl 16; `stability = "reserved"`, `unsupported = true`, mọi method `unsupported = true`. `[object_reserved]` còn **17..31**. Agent 2 áp + cập nhật check_abi.py, check_reserved.py; gate --fast xanh; một commit `[KT-04b]`.

**Luật nâng bậc** (áp cho mọi object reserved): một object chuyển từ `reserved` sang `core` chỉ khi mốc cài nó (K2, K3, D3, H1, P1) gửi chữ ký method + kịch bản và có dòng `APPROVED` mới. ID object và ID method đã cấp không bao giờ đổi hay tái dùng (D2-15 mục 5).

**D2-33** KT-04c **không** kiểm arity từ `spec/`. Thay bằng draft `v2/docs/abi-call-signatures.toml` (`status = "REVIEW"`):
- Chữ ký cho **các method dùng trong 21 .scn**, tối thiểu: Untyped.Retype (có `flags`, gồm bit `GUEST` reserved), CNode.Copy, CNode.Mint, CNode.Revoke, CNode.Delete, Tcb.Configure, Tcb.SetFaultEp, Tcb.Suspend(mode), Tcb.Resume, **Tcb.ReadState**, SchedContext.Bind, SchedControl.Configure. Method nào .scn dùng mà không có trong danh sách này thì thêm vào, **không** viết chữ ký cho method ngoài 21 ca.
- **Enum trạng thái TCB là enum đầy đủ §4.4**, không phải tập con: `Inactive | Ready | Running{hart} | BlockedOnSend{ep, call, fault: Option<FaultKind>} | BlockedOnRecv{ep, reply} | BlockedOnReply{reply} | BlockedOnNotification{ntfn} | BlockedOnAddress{frame, offset} | Sleeping{reason: Timeout|Budget} | Stopping | Dead`, cộng cờ trực giao `suspended` và `exit_code: Option<u64>`. Trường `fault` trong BlockedOnSend là bổ sung được duyệt tại đây (dùng cho Exit, mục 1 điểm 3 của DUYET-AGENT0-vong-va).
- Chữ ký method **không** thuộc 5 thứ đóng băng của D2-15, nên sau APPROVED draft vào `spec/` với `stability = "stable-draft"`; đổi chữ ký ở K1 vẫn qua Agent 0 nhưng không cần tăng `abi_version` major.

---

## 2. Sổ và luật

Đồng ý nguyên §3a (lật KT-04a DONE, KT-04b/c READY, giữ KT-04d REVIEW, KT-05 BLOCKED) và §3b (ba dòng luật cho phép KT-04b ghi spec một lần), với chữ "chỉ chép draft reserved + D2-29" đổi thành "**chỉ chép draft reserved đã sửa theo D2-32 + D2-29**".

D2-34: **không ký.** ABI nền chỉ duyệt một lần, và hàng việc còn lại (KT-T26, KT-06, KT-08, KT-12, KT-R1, dòng ViScript) đủ cho nhiều giờ.

---

## 3. Điều kiện gửi KT-04d

Gói KT-04d phải có:
1. `check_scn` **21/21, 0 blocked**. Hiện đang có **1 blocked**: ghi rõ ca nào, vì sao. Nếu là thiếu ABI thì nêu method cần thêm; không bỏ ca.
2. check_abi, check_reserved, check_scn xanh; gate --fast xanh.
3. Draft chữ ký D2-33 + checklist 5 mục/method.
4. Bảng mapping v1→v2 đã sửa 2 lỗi số liệu.
5. Danh sách D2-26..D2-28 (Agent 0 chưa thấy nội dung ba mục này; cần chép kèm để duyệt đủ sổ).

---

## 4. Tin nhắn ngắn cho người điều phối / Agent 1

> Agent 0 đồng ý YC-02, YC-03, YC-05 và YC-04 có sửa: SchedControl là object lõi (đã có chữ ký trong draft, không thể reserved); VSpace khách là bit cờ GUEST của Untyped.Retype, bỏ VSpace.Configure. DebugGetInfo luôn có, không feature gate. Enum TCB trong draft phải đủ §4.4. Dán D2-29..D2-33 ở mục 1 (thay bản trong gói), lật 3 ô, sửa 3 dòng luật, commit `[KT-00] Agent 0 trả lời YC-02..05, mở KT-04b/c`. Không ký D2-34. KT-04d gửi khi 21/21 .scn, 0 blocked, kèm D2-26..28.
