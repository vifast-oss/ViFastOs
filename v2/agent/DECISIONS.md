# DECISIONS — ViFastOs v2

Người ghi: **Agent 0**. Agent 2 **không** sửa tệp này; cần đổi thì ghi `BLOCKED.md` + đề xuất.
Nguồn: `docs/QUYET-DINH-sau-review.md` (QD) §2.3, §4, §8 và `review/src/ViFastOs-dossier.md` §19.1.
Không gian tên đã chốt (QD §2.1): quyết định v1 giữ `D-01..D-08` (**chỉ áp v1**); quyết định v2 là **`D2-01..D2-54`**. Số cũ `D-09..D-27` **hết hiệu lực**, không dùng lại.

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

## Phần II — D2-01..D2-54: quyết định v2 (D2-01..28 chép QD §8; D2-29..46 bổ sung 2026-11-12)

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
- **D2-24** mọi soak trên máy ps, N theo `n ≥ 3/p` (p mục tiêu 0,3 % ⇒ **N = 1.000**, song song 3 luồng).
- **D2-25** Tài liệu `docs/v2/ViFastOs-v2-thiet-ke.md` bản vá 2026-11-12 được Agent 0 duyệt, kèm errata `DUYET-AGENT0-vong-va.md`.
- **D2-28** **KT-02** dựng khung (`v2/Cargo.toml`, `v2/bin/`, `v2/agent/`, LICENSE) và được phép port **tối đa** các thư viện nền cần cho `vifast-fsutil`: `vifast-fs`, `vifast-fsutil`, `vifast-elf` (D2-26), với điều kiện `cargo build` xanh, mỗi tệp ≤1.500 dòng, giữ SPDX + `PORTED-FROM`. Không port thêm thư viện khác trong KT-02; quá 3 h thì dừng, phần còn lại sang KT-T26.
- **D2-27** KT-04c có **21 kịch bản**: **20 ca gốc** theo QD §4 + **1 ca "Exit khi procsrv chưa Recv"** theo DUYET §1.3; QD §4 giữ nguyên 20 ca gốc, không sửa QD. Đây là câu trả lời Q-01.
- **D2-26** Thư viện "giữ từ v1" (`vifast-pte`, `vifast-elf`, `vifast-acpi`, `vifast-fs`, `vifast-fsutil`, `vifast-virtio`) được **copy/port vào `v2/lib/`**, **không** path-dependency sang `crates/`; giữ SPDX và ghi `PORTED-FROM: crates/<tên>@ca2eaf5`. v1 vẫn đóng băng.
- **D2-29** `DebugGetInfo` = **syscall 15**, dải reserved syscall còn **16..31**. `DebugGetInfo` **luôn có**, không phụ thuộc feature `debug`/`test` (khác 12, 13), vì nó trả `abi_version`, `min_compat` và bitmap tính năng mà runtime dùng để kiểm tương thích lúc khởi động. Trả về: `abi_version`, `min_compat`, `features` (bitmap), `num_harts`. Không trả địa chỉ kernel. `spec/abi.toml`: thêm `[syscall].15`, `[syscall_reserved].from = 16`, `[debug_info].number = 15`, `status = "frozen"`; `check_abi.py` cập nhật tương ứng.
- **D2-30** Câu QD §4 "0..13 như cũ" nghĩa là **như bảng §4.21 của thiết kế**, không phải giữ số v1. v2 đánh số lại; `v2/docs/abi-v1-v2-mapping.md` là nguồn đối chiếu từng số. QD giữ nguyên.
- **D2-31** `spec/` ở **gốc repo** là nguồn sự thật; cây `v2/spec/` trong thiết kế §8 là lỗi vẽ cây, không dùng. Errata ghi tại đây, không sửa thiết kế.
- **D2-32** KT-04b được ghi `spec/abi.toml` **một lần**, theo draft `v2/docs/abi-reserved-objects.toml` **sau khi sửa hai điểm**:
  1. **SchedControl (8) là object lõi**, `stability = "core"`, không reserved: MCS donation có từ K1b (D2-16) và `SchedControl.Configure` đã nằm trong chữ ký draft D2-33. Không được vừa reserved vừa có chữ ký.
  2. **VSpace khách không phải method.** Bỏ `VSpace.Configure (id 1)`. Khách hay thường là **kiểu bảng trang**, quyết định lúc tạo, không đổi sau: dùng **bit cờ `GUEST` trong tham số `flags` của `Untyped.Retype` khi type = VSpace**. Bit này reserved; Retype với `GUEST` trả `NotSupported` tới H1. Method id 1 của VSpace để trống.

  Object reserved còn lại: VSpace 9, PageTable 10, Frame 11, IrqControl 12, IrqHandler 13, IOSpace 14, VCpu 15, PowerControl 16; `stability = "reserved"`, `unsupported = true`, mọi method `unsupported = true`. `[object_reserved]` còn **17..31**. Agent 2 áp + cập nhật `check_abi.py`, `check_reserved.py`; `gate --fast` xanh; một commit `[KT-04b]`.
- **Luật nâng bậc** (áp cho mọi object reserved): một object chuyển từ `reserved` sang `core` chỉ khi mốc cài nó (K2, K3, D3, H1, P1) gửi chữ ký method + kịch bản và có dòng `APPROVED` mới. ID object và ID method đã cấp không bao giờ đổi hay tái dùng (D2-15 mục 5).
- **D2-33** KT-04c **không** kiểm arity từ `spec/`. Thay bằng draft `v2/docs/abi-call-signatures.toml` (`status = "REVIEW"`):
  - Chữ ký cho **các method dùng trong 21 `.scn`**, tối thiểu: `Untyped.Retype` (có `flags`, gồm bit `GUEST` reserved), `CNode.Copy`, `CNode.Mint`, `CNode.Revoke`, `CNode.Delete`, `Tcb.Configure`, `Tcb.SetFaultEp`, `Tcb.Suspend(mode)`, `Tcb.Resume`, **`Tcb.ReadState`**, `SchedContext.Bind`, `SchedControl.Configure`. Method nào `.scn` dùng mà không có trong danh sách này thì thêm vào, **không** viết chữ ký cho method ngoài 21 ca.
  - **Enum trạng thái TCB là enum đầy đủ §4.4**, không phải tập con: `Inactive | Ready | Running{hart} | BlockedOnSend{ep, call, fault: Option<FaultKind>} | BlockedOnRecv{ep, reply} | BlockedOnReply{reply} | BlockedOnNotification{ntfn} | BlockedOnAddress{frame, offset} | Sleeping{reason: Timeout|Budget} | Stopping | Dead`, cộng cờ trực giao `suspended` và `exit_code: Option<u64>`. Trường `fault` trong `BlockedOnSend` là bổ sung được duyệt tại đây (dùng cho Exit, mục 1 điểm 3 của `DUYET-AGENT0-vong-va`).
  - Chữ ký method **không** thuộc 5 thứ đóng băng của D2-15, nên sau APPROVED draft vào `spec/` với `stability = "stable-draft"`; đổi chữ ký ở K1 vẫn qua Agent 0 nhưng không cần tăng `abi_version` major.
- **D2-35** Duyệt **nguyên đề xuất** `[TK]`: `[object_size].riscv64` (`tcb=512`, `endpoint=64`, `notification=64`, `reply=64`, `sched_context=256`, `sched_control=64`, `cnode_base=64`) và `[message_info]` 35 bit + 64 MR + 4 cap + IPC buffer 4 KiB. Đổi các số này phải có D2 mới. `DebugGetInfo` trả: `a0=error`, `a1=abi_version`, `a3=min_compat`, `a4=features`, `a5=num_harts` (syscall không dùng MR vào); ghi `[debug_info].return_channel` trong `spec/abi.toml`. Top-level `status = "APPROVED"` đặt trong cùng cửa sổ với dòng APPROVED KT-04d.
- **D2-36** `KT-T26` là **đèn host** (read-your-own-write trên `vifast-fs` + `CachedDev`, `MAX_IO=256`, ca âm mất ghi im lặng), **không** tuyên bố đóng T26. T26 giữ **OPEN** trong v2 tới **K3.5** (VFS/`read_dir` + cache + `smp`); **không** chuyển sang KT-08. Không chạy lại 1.000 seed. DoD KT-T26 = "đèn + ca âm xanh, có log đỏ trước"; ghi T26 OPEN tới K3.5 trong `BLOCKED.md`.
- **D2-37** (a) Agent 2 **không** ghi `v2/agent/QUEUE.md`/`viscript/agent/QUEUE.md`; `vifast-agentctl done` chỉ là **đề xuất đọc-only**, người điều phối/Agent 0 lật sổ bằng commit `[KT-00]`/`[ST-00]`. (b) Comment `spec/abi.toml` còn cũ do **Agent 0** sửa trong cùng cửa sổ APPROVED, không đổi giá trị ABI. (c) D2-21: vòng 1 **không** bắt buộc chạy `cargo-deny`; bắt buộc có `deny.toml` (allowlist QD §9.3) + bước kiểm "không registry dependency" trong `v2 check` **trước dependency ngoài đầu tiên**, `cargo-deny` đầy đủ từ mốc đó/K1a.
- **D2-38** Dump text của KT-08 (`# scn seed=…`, `step=…`, `# trace-hash=`) là **scenario log** phục vụ debug/L1, **không** phải traceformat ABI/kernel. **KT-12** sở hữu bản ghi **32 B** `{t_ns,hart,kind,tcb,a,b}` dùng chung sim/QEMU. Golden text hiện có giữ nguyên làm hồi quy sự kiện; KT-12 thêm golden nhị phân + decoder + cross-check; không có hai định dạng chuẩn.
- **D2-39** `R<n>-v2.json` có đúng **11 khoá**: `project, round, head, build_id, tasks, gates, sim, coverage, mutants, perf, blocked`. "Phủ task đã mở" = **mọi hàng QUEUE** tại thời điểm báo cáo (vòng 1: 17/17), không chỉ DONE/REVIEW. Không thêm khoá `next` vào JSON; `next` chỉ là thông tin CLI/MD.
- **D2-40** `[trace]` trong `spec/abi.toml` là nguồn sự thật cho bản ghi **32 B**: little-endian, `t_ns`@0/u64, `hart`@8/u16, `kind`@10/u16, `tcb`@12/u32, `a`@16/u64, `b`@24/u64; kind 1..7 = syscall/ipc/sched/fault/store/run/idle; header magic `VFT1`, version 1, count u32, hash FNV-1a 64. Đây là **cùng định dạng sim và QEMU** (dump `pmemsave`); text KT-08 chỉ là scenario log (D2-38). `spec.golden = "docs/abi.md"` (sinh từ spec, gen-check so byte); không còn đường dẫn treo `test/abi/abi-v1.golden`.
- **D2-41** `expect_states` của `.scn` là **hậu điều kiện sau mỗi bước** của thread tương ứng, kiểm ngay sau khi bước trả về; với `exit`/`debug_halt` kiểm ngay sau `halt`, **trước** khi procsrv `Recv` xử lý xong. Phải khớp cả tên trạng thái và trường (`call=false`, `fault=Exit{code}` so mã). Không kiểm giữa các bước, không suy diễn trạng thái khi chưa có interpreter (KT-09).
- **D2-42** `vifast-traceformat` phải là `#![no_std]` + feature `alloc` cho `decode_all`; `FixedRing<const N>` là ring chuẩn (không cấp phát, dùng trong kernel), `Ring`/`Vec` chỉ sau feature `std` cho host. KT-12 DONE phần host/format; **KT-12b** (2 h, READY) hoàn tất no_std + cross-check byte; **KT-09 phụ thuộc KT-12b**. Không tạo KT-12 khác.
- **D2-43** Golden nhị phân của trace phải tái lập được bằng script đã commit: `v2/test/trace/gen_golden.py --check` đọc golden TEXT của sim, sinh 200×32 B và so byte với `golden-sim-seed7.bin`; `./v2/bin/v2 check` chạy bước này. Không nhận golden do tay/Python ngoài repo sinh; script không đọc code Rust.
- **D2-44** `CNode.Move` `[TK]`: chuyển cap nguồn sang `dst`; nếu `dst` đã có cap thì cap đích bị xoá trước rồi chuyển (overwrite), vì `01-cnode-copy.scn:13` yêu cầu `ok` sau khi `dst` đã bị `Copy` chiếm. `CNode.Copy`/`CNode.Mint` vẫn yêu cầu `dst` rỗng, trả `SlotOccupied`. Đây là ngữ nghĩa mô hình, chưa phải ABI chốt; `CNode.Mutate` giữ nghĩa biến đổi tại chỗ (chưa cài KT-11).
- **D2-45** F3 `[TK]`: `Untyped.Retype` yêu cầu quyền `WRITE` trên cap nguồn; `CNode.Delete`/`CNode.Revoke` yêu cầu `WRITE` trên cap CNode dùng để triệu hồi. Chưa có nguồn spec/design; là chính sách mô hình `[TK]`, K1a rà lại trước khi đóng ABI. `Copy`/`Mint` giữ quyền ⊆ nguồn; `mint(Some(0))` xoá badge (không giữ badge nguồn).
- **D2-46** Badge chỉ hợp lệ trên `Endpoint`/`Notification`; `Mint` badge trên loại khác trả `InvalidArgument`. Fixture `02-cnode-mint-badge.scn` sửa cho đúng: bước 1 dùng `src=ep0`, bước 2 thêm `c2` và dùng `dst=c2`; mục tiêu `.scn` Untyped/CNode **9/9**. Đây là sửa dữ liệu test, không đổi ABI.
- **D2-47** Phase K1a' theo `review/agent0/K1A-PLAN.md`: (a) cài `miri` + `rust-src` cho nightly đã xong; Miri chạy bằng lệnh rời `v2 miri` và `nightly --only-miri`, **không** đưa vào `gate --fast` (giữ ≤60 s); (b) `cargo-deny`/`deny.toml` **chưa** bắt buộc ở K1a' vì chưa có dependency ngoài — vẫn theo D2-37(c), thêm trước dependency đầu tiên; (c) cho phép tách `KT-16a`/`KT-16b` nếu tích hợp >3 h, theo ranh giới đã định; (d) K1a' chỉ đưa **object body** vào `vifast-objmem`, bảng `Cap` của sim vẫn là `Vec<Cap>` (tiện ích mô hình; CSpace 2 cấp là K1b/K2); (e) không sửa `vifast-model` (`#![forbid(unsafe_code)]`, oracle).
- **D2-48** YC-18: chọn **(a)** — giữ `ObjRef` 16 B; `vifast-objmem` có bảng tĩnh `kind → vtable/constructor` để dựng `*mut dyn KObj` từ `(pa, id, header.kind)`. `with_many_mut` kiểm alias từng cặp rồi dựng `[&mut dyn KObj; N]` qua bảng; không đổi layout/ABI. KT-15 sở hữu bảng + ma trận alias + fuzz; nếu Miri phát hiện UB do fat pointer thì dừng báo, không `#[allow]`.
- **D2-49** Chốt K1b (Q1–Q8, `review/agent0/K1B-PLAN.md`): (a) **1 hart**; **hoãn push migration sang K1c** (nơi có `smp 1/2/4/8`), K1b đạt 3/4 tiêu chí + hộp thư; K1c kế thừa. (b) Gắn `SchedContext` vào TCB bằng **model API** `set_sched_params(tcb, sc)`, không sửa `spec/`/ABI ở K1b. (c) Donation **tự động** qua Call/Reply; server có SC riêng giữ SC riêng; các method chưa có chữ ký chỉ cài model API, không tuyên bố ABI. (d) Bảng `ALLOWED` 11×11 đặt ở `v2/docs/k1b-tcb-transition.md` (model-only `[TK]`), Agent 0 review; không phải spec. (e) Budget kiểm **chu kỳ thẳng hàng**; cửa sổ trượt/sporadic refill hoãn sau K6 (D2-16). (f) K1b chỉ **MCS donation**, không MCS lock/lock-free; hộp thư mô hình tất định; `vifast-sync`/loom để K1c/SMP. (g) Không mở rộng `vifast-model` cho TCB/scheduler; parity 10k chỉ là hồi quy. (h) Preempt/budget dùng **tick ảo**, không timer HW/QEMU.
- **D2-50** K1b/objmem: **TCB không có `SchedContext` hoặc SC chưa `Configure` KHÔNG được chạy** — không phải "không giới hạn". `tcb_has_budget` trả `false`; `dispatch/pick` bỏ qua; SC-less chỉ chạy khi được **donation** (KT-21, passive server). Rig test KT-19/20 được phép gắn SC mặc định **test-only**, không đổi production; KT-21 phải có test SC-less server chỉ chạy khi được donate.
- **D2-51** KT-21 MCP: K1b dùng **kẹp** `p_eff = min(client.priority, client.mcp, server.mcp)` (**model-only `[TK]`**), không strict-reject; K1c chốt strict nếu cần. `reply` chỉ trả donation khi `caller == donor`; Reply sai object giữ donation, không đánh thức nhầm. 13 test + 14.609 bước donation/64.000; strict ownership/ReplyRecv/4 cap để K1c.
- **D2-52** KT-23 được phép sửa **duy nhất một dòng** của `v2/test/scn/12-suspend-abort-while-reply-wait.scn` (`Defer => ok` → `Defer => InvalidArgument`) để khớp thiết kế §4.4 `:282`; các `.scn` khác giữ nguyên, không đổi ABI. Đây là ngoại lệ có ghi cho K1B-PLAN KT-23 DoD 8 (vốn cấm sửa `.scn`).
- **D2-53** Chốt K1c (Q1–Q15, `review/agent0/K1C-PLAN.md`): Q1 mô hình **N hart tất định 1/2/4/8**, không thread/QEMU; Q2 push **cuối hàng**, ≤1 TCB/bước, chỉ khi `published.idle`, chỉ TCB cùng mức Ready cao nhất; Q3 hằng số `WAITADDR_BUCKETS`/`MAX_HARTS` model `[TK]` trong `v2/docs`, **chưa** tạo `spec/kernel.toml` (K3); Q4 `WaitAddr` **không timeout** (spec thắng; timeout K6); Q5 `FaultKind` VM/unknown/user **model-only**, spec là tập con tới K3; Q6 bind Notification bằng model API, không thêm chữ ký; chuyển cap thất bại là **đếm mềm** (`cap_transfer_failed`), không thêm bit MessageInfo; Q7 caller `no_reply=1` ⇒ Call=Send, `Ok` ngay, không Reply, receiver thấy cờ; kernel thiếu Reply ⇒ caller `Cancelled`; Q8 Reply bị xoá khi caller `BlockedOnReply` ⇒ caller `Cancelled`; Q9 `vifast-sync` + stress host có biên + Miri, **không loom**; Q10 procsrv chỉ mô hình, shell K3.5; Q11 Call non-Endpoint `NotSupported`, không block/không Reply; Q12 không mở `vifast-model` IPC, bằng chứng = invariant/coverage/mutation, parity 10k hồi quy; Q13 `v2 nightly --l1` 100k seed × smp 1/2/4/8, gate vẫn 1k, không tách KT-31; Q14 không chạm ViScript; Q15 `NBSendRecv` = `NBSend` rồi `Recv`; nếu NBSend không giao được vẫn `Recv`, message không bị coi là mất bởi kernel.
- **D2-54** K1c: hàng gửi Endpoint dùng **`MAILBOX_CAP=8` + `Busy`** (code KT-22 thắng dòng "không giới hạn" cũ trong K1C-PLAN; `Send` chặn khi đầy là backpressure mô hình, K3 fastpath xem lại). Runner `.scn` dùng quy ước **`ep0`/`ep1` là cùng một Endpoint**, và `recv` không có `reply=` thì cấp **`rep0`** (model-only; K3 conformance phải kiểm lại).

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
| mã lỗi kernel | **19** (bỏ `Restart`); syscall **0..14** + **15 `DebugGetInfo`**; dải reserved **16..31** | QD §4, D2-29 |
| soak v2 | **N = 1.000**, máy **ps**, 3 luồng, `n ≥ 3/p`, p = 0,3 % | QD §5.3 |
| L1 trong `--fast` | **1.000** seed (nightly **100.000**) | QD §5.2 |
| số kịch bản ABI | **21** `.scn` = 20 ca gốc (QD §4) + ca "Exit khi procsrv chưa Recv" (DUYET §1.3) | D2-27 |
