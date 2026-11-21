# KT-320 — riscv64 **`Tcb.Configure` tham số `vspace?`** (nợ KT-311)

Trạng thái: **xong**, nhánh `task/kt320-vspace`, base `main` @ `90ea39ef`. 2026-11-22.
Sub-agent `t320` (kế thừa công việc dang dở trên đĩa — xem §1). Quyết định: **D2-224** (mở
KT-320: "cài tham số cuối; nếu cần đổi ABI thì DỪNG"), **D2-83** (core không đọc/ghi bộ nhớ
user, **không** ghi CSR — `satp`/PTE thuộc arch), **D2-84** (pin `size_of::<Machine>()`
**196.608** B), **D2-04/D2-22** (`spec/` đóng băng), **D2-11** (hằng/method sinh từ spec),
**D2-05** (test đỏ trước), **D2-198** (uỷ quyền commit trên nhánh task; chỉ lead merge/push).

**Không** mở lại K3 (D2-205) và **không** tuyên bố tiêu chí K3'/K5' nào đạt: KT-320 chỉ **trả một
nợ đã công bố** (KT-311 §7.2: *"`vspace?` chưa cài ⇒ `NotSupported` 18"*).

## 1. Khảo sát (kế thừa hiện trường)

Agent trước đã dựng sẵn phần **core + hook arch + test core** rồi bị dừng giữa lượt. `git diff
--stat` lúc nhận: 8 tệp, **+434/−33** (`configure_tests.rs`, `core_tcb.rs`, `invoke.rs`, `ipc.rs`,
`syscall.rs`, `tcbdata.rs`, `plat-riscv64/src/{user,user_run}.rs`). Đo lại độc lập:
`cargo test -p vifast-core --lib` = **81 passed / 0 failed** (base 79 ⇒ **+2** test mới).

**Phương án đã dùng = (A) trường core + hook arch** (không phải (B) chỉ lưu):

* `TcbData.vspace: ObjId` (thêm **một** trường 4 B) + `Machine::tcb_vspace`;
* `SyscallEnv::tcb_vspace(tcb, vspace) -> bool` (hook mới, mặc định `false`);
* `invoke::configure` đọc CPTR `vspace` (word 1 vùng 608..640) thay cho nhánh `NotSupported`,
  và **giao** giá trị qua hook sau khi `tcb_configure_invocation` xanh;
* arch: `user_run::{arm_vspace, apply_vspace, vspace_applied}` + `user::enter_loaded_with_root`
  + `impl SyscallEnv for UserMemAccess::tcb_vspace`.

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| `vspace?` có trong ABI chưa? | **CÓ** từ trước — `params = ["tcb", "cspace?", "vspace?", "entry?", "ipc_buffer?", "priority?"]` ⇒ **không** đổi ABI | `spec/abi.toml:248` |
| Mask bit của `vspace?` | bit **1** = `CONFIGURE_MASK_VSPACE` (kernel `ipc.rs`, U-mode `vifast-plat-user/lib.rs`) | `vifast-core/src/ipc.rs:114`, `vifast-plat-user/lib.rs:297` |
| Vùng tham số nằm đâu? | word 1 của vùng **608..640** (`[TK]` KT-311, `IPC_BUFFER_CONFIGURE_OFFSET = 608`) — **không** đổi `MessageInfo` 35 bit | `vifast-core/src/ipc.rs:98..112` |
| KT-311 để lại gì? | `vspace?` bật ⇒ `NotSupported` **18**; `TcbData` **chưa** có trường `vspace` | `invoke.rs` (nhánh bị xoá), `tcbdata.rs` trước KT-320 |
| Core có được ghi `satp` không? | **KHÔNG** — D2-83: `satp`/PTE là của arch; core chỉ giữ **danh tính** object | `syscall.rs:181..` (trait), KT-282/KT-314 cùng khuôn |
| Có tiền lệ "core giao — arch tiêu thụ"? | **CÓ**: `tcb_resume` (KT-314, `sepc`), `ipc_header.extra` (KT-309), `ipc_buffer::request_write` (KT-282) | `syscall.rs`, `user_run.rs` |
| Danh tính object đi qua đâu? | `Machine::add_foreign_object(VSpace, key)` + `foreign_key`; `key = (object_id << 16) \| cap_index` của object layer ⇒ giải ra `(root_pa, asid)` | `core_foreign.rs:22..40`, `mem_bridge.rs:88..96`, `ktest_inv.rs:123` |
| `TcbData` còn đệm không? | **CÓ** — 824 B, căn 8, trường cuối `entry: u64`; `ObjId` (4 B) **vừa** đệm sẵn ⇒ `size_of::<Machine>()` **không** đổi | `tls_tests.rs:339..347` (phép tính KT-311) + §2 dưới |

⇒ **Kết luận khảo sát**: (a) **không** cần đổi ABI; (b) hook arch là **bắt buộc** (D2-83), không
phải lựa chọn; (c) trường `ObjId` **không** làm `Machine` lớn thêm ⇒ **không** phải sửa bốn dòng
evidence `bytes=`/`sum=`/`static=`; (d) ảnh `ktest-ipc` còn đúng **một** chỗ nói `vspace?` chưa
cài (`# ipc: configure2 neg … vspace=18`) — theo `k6-test-invariants.md` §4.1 tiền đề đó **phải**
đổi.

## 2. `size_of::<Machine>()` trước/sau — **không** đổi, **không** nới pin

Đo bằng **cùng** test `tls_tests::pin_machine_tang_dung_8_byte_moi_tcb` (in dòng `# tls pin:`),
trước = `git stash push` đúng 8 tệp của KT-320 rồi đo, sau = `git stash pop`:

```
# tls pin: size_of_machine=184792 ... size_of_tcbdata=824 ... kt311_tcb=512 kt312_giam=10240 tinh=-4080 khop=1 duoi_pin=1   (TRƯỚC)
# tls pin: size_of_machine=184792 ... size_of_tcbdata=824 ... kt311_tcb=512 kt312_giam=10240 tinh=-4080 khop=1 duoi_pin=1   (SAU)
```

* `size_of::<Machine>()` = **184.792 B** trước **và** sau (Δ = **0**);
* `TcbData` = **824 B** trước **và** sau — `vspace: ObjId` (4 B) nằm trong **đệm sẵn có**;
* pin D2-84 **196.608** **không** nới (headroom 11.816); trần KT-312 `≤ 184.832` **không** chạm;
* phép tính `pin_machine_tang_dung_8_byte_moi_tcb` **không** phải sửa (không thêm byte nào) và
  vẫn `khop=1`;
* bốn dòng evidence của ảnh `ktest` (`# core: link … bytes=184792 … sum=184792 …`,
  `# core: phase … static=184792 pin=196608 …`) **giữ nguyên** — và **đã được chứng minh** bằng
  matrix §6 (ảnh `ktest` xanh với đúng các regex đó trong `expected.txt`).

> Vì sao **không** `Option<ObjId>`: `Option<u32>` không có niche ⇒ **8 B** mỗi TCB ⇒
> `+8 × 64 = 512 B` vào `Machine` (đè trần KT-312). `0` = "chưa cấu hình" là **miền chứng minh
> được**: `ObjId` của core là **1-based** (`Machine::object_index` từ chối `0`) — cùng quy ước
> `TcbData::entry`/`tls`.

## 3. Ngữ nghĩa đã cài

| Đường | Hành vi |
|---|---|
| `Tcb.Configure` mask **không** có bit 1 | như cũ (không đọc/không ghi `TcbData.vspace`) |
| mask có bit 1, CPTR **ngoài bảng** / ô **rỗng** / cap **chết** | `InvalidCapability` **1** (giải CPTR trong CSpace **caller**) |
| mask có bit 1, cap **không phải `VSpace`** (Frame/CNode/PageTable…) | `WrongObjectType` **2** |
| mask có **bit lạ** (≥ bit 4) | `InvalidArgument` **5** — kiểm mask đứng **trước** mọi kiểm khác |
| mask hợp lệ, cap `VSpace` sống, các tham số khác hợp lệ | lưu **danh tính** (ObjId) vào `TcbData.vspace`; **gọi** `SyscallEnv::tcb_vspace(tcb, obj)` **một lần** |
| nguyên tử | sai **bất kỳ** tham số nào ⇒ **không** ghi `vspace` (kiểm tất cả trước khi ghi) |
| arch (riscv64), lần vào U-mode **kế tiếp** của **đúng** TCB | giải `ObjId` → `(root_pa, asid)` qua `foreign_key`; root **khớp** `loaded.root` ⇒ `satp` lấy **từ VSpace của TCB**; **lệch**/không giải được ⇒ **không** vào U-mode (fail-closed, `Stop::Blocked`) |

## 4. Ảnh `ktest-ipc`: TAP **7 → 8**

`ok 8 - ipc: tcb configure vspace`. Bốn điều kiện, mỗi điều đo **độc lập** (không tin lời chương
trình U-mode):

1. client U-mode in `# ipc: configure3 ok=1` — `Call(cap TCB đích, {label = Tcb.Configure,
   length = 3}, MR0 = CPTR Frame, MR1 = VA, MR2 = MASK_VSPACE)` trả `(0, 0)`;
2. client in `# ipc: configure3 neg mask=5 frame=2 ok=1` — **hai ca âm**: mask bit lạ ⇒ **5**,
   CPTR là cap **`Frame`** ⇒ **2**;
3. kernel đọc **độc lập** `TcbData.vspace` của TCB đích = **đúng** object đại diện `VSpace`
   (`# ipc: vspace ok obj=18 applied=18 match=1 root=0x804f8000 asid=1`);
4. `user_run::vspace_applied()` báo arch **đã tiêu thụ** đúng object với `match=1`, **và** tác vụ
   đích vẫn `Exit(0)` — nó chạy được **sau khi** `satp` được chọn từ VSpace của TCB.

**Vì sao bất biến không phụ thuộc thứ tự/thời điểm** (`k6-test-invariants.md` §1): rig gọi
`run_turn` **tuần tự** trên **một** hart (mọi hart phụ `park` trong `rust_secondary_main`), nên
"arm ở lượt 9 → tiêu thụ ở lượt 10" là **quan hệ nhân quả do mã rig bảo đảm**, không phải "thường
đúng". Không có điều kiện nào đảo khi đổi thứ tự tick giữa các hart.

**Ca âm cũ đổi tiền đề** (§4.1 của cùng tài liệu): `# ipc: configure2 neg … vspace=18` → `vspace=2`
(ô 0 của CSpace gốc là cap `CNode` ⇒ `WrongObjectType`). Test khẳng định "tính năng X **chưa**
cài" **phải** đổi khi X được cài.

## 5. Harness (chỗ hay quên — KT-311/312 đều vấp)

| Tệp | Đổi |
|---|---|
| `expected.txt` | `ktest-ipc 7 7` → **`8 8`** + đoạn tài liệu KT-320 |
| `ktest.sh` | `manifest_ipc` 7 → **8** (`ipc: tcb configure vspace`); mẫu `PANIC` mới (`KT-320 … (8 TAP, 0 fail)`); **3** dòng evidence mới (`configure3 ok`, `configure3 neg mask=5 frame=2`, `vspace ok … match=1 …`); `vspace=18` → `vspace=2`; header 7 TAP → 8 TAP |
| `selftest.sh` | ca **đỏ** mới: xoá dòng `# ipc: vspace ok obj=` ⇒ runner phải đỏ `ev36` (chứng minh dòng evidence là **bắt buộc**) |
| `vifast-plat-user/lib.rs` | chú thích `CONFIGURE_MASK_VSPACE` hết nói "chưa cài" |
| `ktest_ipc.rs` | header "Tám TAP `1..8`" + mục 7/8; rig: object đại diện `VSpace` của client + cap ô 17 trong cspace client; đo `tcb_vspace` **trước** lượt 10, `vspace_applied` **sau** lượt 10; TAP 8; PANIC 8 TAP |
| `ipc_client_user.rs` | mục **7c**: mask lạ ⇒ 5, `vspace` = cap `Frame` ⇒ 2, đường xanh ⇒ `(0, 0)`; mã thoát 111/112/113 |

Bốn dòng evidence **kích thước** (`bytes=`/`sum=`/`static=`) **không** phải sửa vì
`size_of::<Machine>()` không đổi (§2) — đã kiểm bằng log thật, không phải suy đoán.

## 6. Log QEMU nguyên văn (runner tự sinh, smp=1, ảnh `ktest-ipc`)

Nguồn: `bash v2/test/boot/ktest.sh --log-path ktest-ipc 1 1` →
`$TMPDIR/k3-ktest/ktest-ipc-smp1-1.log` (118 dòng; trích nguyên văn từ lượt configure2 tới PANIC):

```
# ipc: configure2 ok=1 entry=0x40000112 prio=4
# ipc: configure2 neg align=6 prio=5 cspace=2 vspace=2 ok=1
# ipc: configure2 entry=0x40000112 prio=4 cspace=15 vspace=0 calls=2 denied=5 ok=1
# ipc: resume ok=1 entry=0x40001324
# ipc: resume neg noentry=5 running=17 exited=17 ok=1
# ipc: configure3 ok=1
# ipc: configure3 neg mask=5 frame=2 ok=1
# ipc: neg empty=1 reply=18 ok=1
# ipc: neg mr over=5 ok=1
# sys: nr=14 err=0 value=0 tcb=3 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=3 state=Inactive
# ipc: resume ok entry=0x40001324 state=Running
# ipc: resume neg noentry=5 state=Inactive
# ipc: resume ran entry=0x40001324
# sys: nr=14 err=0 value=0 tcb=10 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=10 state=Inactive
# ipc: vspace ok obj=18 applied=18 match=1 root=0x804f8000 asid=1
# ipc buf: bound=1 va=0x40020000 mr_off=80 mr_words=60
ok 1 - ipc: umode call
ok 2 - ipc: server replied
ok 3 - ipc: mr4..mr7 via buffer
ok 4 - ipc: tcb configure via invocation
ok 5 - ipc: cap transfer
# ipc final: client_exit=0 server_exit=0 target_exit=0 reply_clean=1 check_ipc=1 check_caps=1 capture_client=1 capture_server=1 client=Inactive server=Inactive target=Inactive
ok 6 - ipc: tcb configure full
ok 7 - ipc: tcb resume
ok 8 - ipc: tcb configure vspace
PANIC: KT-320 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + configure vspace xong (8 TAP, 0 fail)
```

`# ipc: vspace ok obj=18 applied=18 match=1 root=0x804f8000 asid=1` = bằng chứng của **toàn bộ**
TAP 8: `obj`/`applied` cùng một object (danh tính core ⇒ arch), `match=1` (root của VSpace đã cấu
hình **khớp** root ảnh đã nạp), và tác vụ đích `Exit(0)` ở lượt **sau** khi `satp` đổi.

## 7. Đột biến — cả hai **ĐỎ đúng lý do**

| # | Đột biến | Kết quả |
|---|---|---|
| (i) | `core_tcb.rs`: **bỏ kiểm loại** — lấy `.obj` của cap bất kỳ làm `vspace` | `configure_vspace_duong_xanh_va_ca_am` **FAILED**: `left: Ok((0, 0))` / `right: Err(2)` ("vspace = cap Frame ⇒ 2"); `configure_vspace_goi_hook_arch` **FAILED**: `left: Ok((0, 0))` / `right: Err(2)` ⇒ **nhận `Frame` làm VSpace** bị bắt |
| (ii) | `invoke.rs`: **bỏ kiểm mask lạ** (`mask & !CONFIGURE_MASK_ALL != 0`) | `configure_vspace_va_mask_la` **FAILED**: `left: Ok((0, 0))` / `right: Err(5)` ("mask bit lạ ⇒ 5") ⇒ **nhận mask lạ** bị bắt |

Phục hồi: `cp` từ bản sao chụp **trước** đột biến, `diff -q` **identical**, rồi `cargo test -p
vifast-core --lib` = **81 passed / 0 failed**. Không `#[ignore]`, không retry, không nới ngưỡng.

## 8. Cổng đã chạy

| Lệnh | Kết quả |
|---|---|
| `cargo test -p vifast-core --lib` (trong `v2/kernel/`) | **81 passed, 0 failed** (base 79 ⇒ +2) |
| `./v2/bin/v2 check` | **PASS** — exit 0, tổng 122.545 ms |
| `bash v2/test/boot/ktest.sh 1 matrix` | **PASS** — **27/27** boot sạch (`smp=1/2/4` × 9 ảnh, `FAIL=0 TIMEOUT=0`) |
| `bash v2/test/boot/selftest.sh` | **PASS** — **26/26** ca (base 25, +1 ca `ev36` của KT-320) |

`ktest-ipc` xanh ở **cả ba** `-smp 1/2/4`; ảnh `ktest` (in `bytes=184792`/`static=184792`) xanh ⇒
bốn dòng evidence kích thước **khớp số mới** (thực chất là **không đổi số**).

**Hiệu lực phép đo (D2-116/OD-K6-6)**: matrix in **2** dòng `# MEASUREMENT_INVALID`
(`tag=before ly_do=loadavg_1m=14.26>ncpu=12`; `tag=after ly_do=qemu_foreign=1` — máy đang chạy
**ba** agent song song, một QEMU `x86_64` của agent khác, pid 74954). Theo D2-116 điều đó làm
**số THỜI GIAN** không dùng được để kết luận — và journal này **không** dùng số thời gian nào.
Kết quả matrix là **chức năng** (đếm `ok N`, khớp tên TAP, khớp evidence regex trong
`expected.txt`), **không** phụ thuộc thời gian ⇒ vẫn hiệu lực. Không có dòng `^not ok` nào.

## 9. Rủi ro / nợ đã biết (ghi trung thực)

1. **Một khe chờ `VSpace` toàn cục** (`user_run::VSPACE_TCB`/`VSPACE_OBJ`): `arm_vspace` **ghi
   đè** — nếu bộ giám sát `Configure(vspace?)` cho **hai** TCB rồi mới chạy chúng, chỉ giá trị
   **cuối** được áp; giá trị của TCB kia **mất im lặng** (danh tính trong `TcbData.vspace` vẫn
   đúng). Khuôn này giống `RESUME_TCB` của KT-314. Ảnh TAP chỉ arm **một** TCB trước lượt chạy
   của nó. Hướng sửa khi cần: mảng `(tcb, obj)` theo `user::MAX_PROCS` (hoặc đọc thẳng
   `Machine::tcb_vspace(core_tcb())` trong `run_task` — nhưng như vậy `run_task` phải giữ
   `&Machine`, phá R1 của `vifast-plat-riscv64`).
2. **Chưa có đường "VSpace khác ảnh đã nạp"**: `apply_vspace` **từ chối** (fail-closed) khi root
   lệch `loaded.root`, vì `Frame.Map` vào VSpace **khác** ngoài phạm vi KT-320. Đường đó **có**
   mã và **có** nhánh từ chối, nhưng **không** có ca dương end-to-end (cần `Frame.Map` trước).
   TAP 8 phủ đường **match=1** (cùng VSpace) — đúng phạm vi "cài tham số cuối".
3. **Không có cách "xoá" `vspace`** sau khi đặt (giống `entry`/`tls`): `0` chỉ là "chưa từng đặt".
4. `vspace_applied()` là **trạng thái toàn cục** của lần áp gần nhất — dùng cho bằng chứng TAP,
   **không** phải API sản phẩm.

## 10. Xác nhận luật

* **KHÔNG** push, **KHÔNG** merge, **KHÔNG** chạm `main` (chỉ commit trên `task/kt320-vspace`).
* **KHÔNG** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **KHÔNG** sửa `v2/bin/v2`, **KHÔNG** sửa
  `spec/` — **không** đổi ABI (chữ ký `Tcb.Configure` đã có `vspace?` từ trước).
* **KHÔNG** nới ngưỡng nào (`196.608`, `184.832`, `262.144`, seed) — `size_of::<Machine>()`
  **không** đổi nên **không** có gì để nới.
* **KHÔNG** `#[ignore]`/`#[should_panic]`/retry; **KHÔNG** `cfg(target_arch)` trong `vifast-core`.
* `git add` **đường dẫn cụ thể** (13 tệp của commit mã + 2 tệp của commit nhật ký) — **không**
  `git add -A`.
* QEMU: **chỉ** qua `v2/test/boot/qemu_guard.sh` (qua runner); **không** `pkill -f qemu-system`.
  Trong lúc làm có **một** QEMU `x86_64` của **agent khác** (pid 74954, logdir `/tmp/kt318-logs3`)
  — **không** đụng tới.
* Cây làm việc sạch sau commit (`git status --porcelain` rỗng).
