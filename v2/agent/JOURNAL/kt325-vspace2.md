# KT-325 — riscv64 **xoá `vspace?`** + **`vspace_applied` theo TCB** (nợ §9 của KT-320)

Trạng thái: **xong**, nhánh `task/kt325-vspace2`, base `main` @ `42cca7cd`, commit mã **`5939b923`**.
2026-11-22, sub-agent `vspace2`. Quyết định áp: **D2-224** (mở KT-320: *"cài tham số cuối; nếu cần
đổi ABI thì DỪNG"* — KT-325 **không** đổi ABI), **D2-83** (core không ghi `satp`/PTE), **D2-84**
(pin `Machine` **196.608**), **D2-04/D2-22** (`spec/` đóng băng), **D2-05** (test đỏ trước),
**D2-198** (uỷ quyền commit trên nhánh task; chỉ lead merge/push), **D2-225** (vòng 23),
`docs/k6-test-invariants.md` §4.1 (tiền đề phép kiểm đổi thì phải sửa **cả** code + harness).

**Không** tuyên bố K3/K6 đạt lại. KT-325 chỉ **trả hai nợ đã công bố** ở `JOURNAL/kt320-vspace.md`
§9: (1) *chưa có đường xoá* `vspace?`; (2) *`vspace_applied` là trạng thái TOÀN CỤC*.

## 0. Ghi chú danh sách đọc

- **`D2-226` không tồn tại** trong `v2/agent/DECISIONS.md`: mục cuối là **D2-225** (dòng 427).
  Đã đọc D2-083 (dòng 137), D2-190 (dòng 357), D2-224 (dòng 425), D2-225 (dòng 427),
  `docs/k6-test-invariants.md` (181 dòng) và `JOURNAL/kt320-vspace.md` (218 dòng). Không sửa
  `DECISIONS.md` (cấm) — ghi ở đây để lead biết danh sách đọc có một mục không tồn tại.
- Không sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`, `spec/` (§9 xác nhận).

## 1. Khảo sát

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| `vspace_applied()` lưu gì, ở đâu? | **4 static toàn cục** trong `vifast-plat-riscv64/src/user_run.rs`: `VSPACE_APPLIED` (`u32` obj), `VSPACE_ROOT` (`u64`), `VSPACE_ASID` (`u32`), `VSPACE_MATCH` (`u32`); cộng **khe chờ** `VSPACE_TCB` (`u32`, `u32::MAX` = rỗng) + `VSPACE_OBJ` (`u32`) | `git show 42cca7cd:v2/kernel/vifast-plat-riscv64/src/user_run.rs` (dòng ~148..257) |
| Ai đọc `vspace_applied`? | **Chỉ rig/TAP** của ảnh `ktest-ipc` (`ktest_ipc.rs:691` + 3 chỗ in TAP 8). `run_task` **không** đọc nó — nó chỉ **ghi** (`apply_vspace`), và `run_task` **không** giữ `&Machine` (đọc `TcbData` qua `user::core_tcb()` + `core_bridge`). Vì vậy API này là **bằng chứng**, không phải API sản phẩm | `grep -rn vspace_applied v2/kernel` |
| `TcbData.vspace` kiểu gì? | `ObjId` = `u32`, **1-based**; `0` = **chưa cấu hình** (`object_index` từ chối `0`) — cùng quy ước `entry`/`tls`. `TcbData` = **824 B**, `ObjId` vừa đệm sẵn nên KT-320 không làm `Machine` lớn thêm | `vifast-core/src/tcbdata.rs:79`; `tls_tests.rs::pin_machine…` |
| `Configure` đọc word 1 vùng 608..640 thế nào? | `invoke::configure` (`length = 3`) lấy `env.ipc_header(tcb)` (arch đọc **một trang** IPC buffer) → `IpcHeader.configure = IpcConfigureParams::from_words(&words)` với `words[1]` = CPTR `vspace?` (u64). Mask bit **1** = `CONFIGURE_MASK_VSPACE` (MR2). Arch đọc thật ở `plat-riscv64/src/ipc_buffer.rs::read_configure` (4 word tại `IPC_BUFFER_CONFIGURE_OFFSET = 608`) | `vifast-core/src/ipc.rs:90..162`, `invoke.rs:487..540`, `plat-riscv64/src/ipc_buffer.rs` |
| Có phải đổi ABI không? | **Không.** `vspace?` đã có trong `spec/abi.toml` từ trước; vùng 608..640 là `[TK]` (spec không chốt layout vùng dự phòng ⇒ không phải ABI). Biểu diễn "xoá" = **CPTR rỗng (`0`)** trong word 1 — **không** thêm bit mask (mask là `MR2`, mặt ABI), **không** thêm MR, **không** đổi `MessageInfo` 35 bit | `spec/abi.toml:248`, `ipc.rs:91..105` |
| Đánh đổi của "CPTR 0 = xoá"? | Ô **0** của CSpace không còn dùng được làm CPTR `vspace?` — theo quy ước hiện có ô 0 là cap **`CNode` của chính nó** (`CS_SELF`), nên không mất khả năng thật. Ngữ nghĩa này **phải** ghi: CPTR ≠ 0 mà không phải `VSpace` **vẫn** ⇒ `WrongObjectType` **2** | TAP 8 ca âm (cap `Frame` ⇒ 2) giữ nguyên; core test mới |

**Kết luận khảo sát**: (a) **không** đổi ABI; (b) "xoá" biểu diễn được bằng CPTR 0 — một **giá trị**,
không phải **ô**, nên phải chặn ở `invoke::configure` **trước** phép tra CSpace; (c) trạng thái
arch cần **khoá theo TCB** ở **cả hai** chỗ: khe **chờ** (`arm_vspace`/`take_vspace`) và **bản ghi**
(`vspace_applied`) — nếu chỉ sửa bản ghi thì hai TCB cùng chờ vẫn đè nhau (TAP 8 sẽ đỏ khi TCB thứ
hai arm trước lượt chạy của TCB thứ nhất); (d) bảng theo TCB **không** được nằm trong `Machine`
(pin D2-84/trần KT-312, headroom **40 B**) ⇒ đặt static trong `plat-riscv64` (`.bss` ảnh, không
phải `size_of::<Machine>()`).

## 2. Thiết kế

### 2.1 Core — đường **xoá** (`vifast-core`)

- `TcbConfigure.vspace` đổi từ `Option<SlotRef>` sang **enum ba trạng thái**:

  ```rust
  pub(crate) enum VspaceUpdate { Keep, Set(SlotRef), Clear }
  ```

  `Keep` = mask không bật bit 1 (giữ nguyên); `Set` = CPTR ≠ 0 (cap `VSpace` sống); `Clear` = CPTR
  **rỗng**. Enum thay vì `Option + bool` để trạng thái *"vừa set vừa xoá"* **không biểu diễn được**.
- `invoke::configure`: `extra.vspace == 0` ⇒ `Clear` (bỏ hẳn phép tra `cnode_slot_count`/ô 0);
  ngược lại giữ nguyên đường KT-320 (CPTR ngoài bảng ⇒ `InvalidCapability` 1, cap sai loại ⇒ 2).
- `Machine::tcb_configure_invocation`: `Clear` ⇒ `t.vspace = 0` **trong cùng khối "áp"** — mọi kiểm
  (`cspace`/`ipc_buffer`/`entry`/`priority`) vẫn chạy **trước**, nên **nguyên tử** giữ nguyên; `Clear`
  **không** đụng `entry`/`ipc_buffer`/`cspace`.
- Hook `SyscallEnv::tcb_vspace` được gọi khi `vspace != Keep` và nhận **giá trị kết quả** đọc từ
  `TcbData` (`0` khi xoá) ⇒ arch **quên** danh tính cũ, không còn hai nguồn sự thật lệch nhau.

### 2.2 Arch — hai bảng **theo TCB** (`vifast-plat-riscv64/src/user_run.rs`)

- `const VSPACE_SLOTS: usize = user::MAX_PROCS + 2;` = **8** (6 ô tác vụ U-mode + 2 dự phòng).
- `struct VspaceArm { tcb: AtomicU32, obj: AtomicU32 }` — khe **chờ** (8 B × 8 = **64 B**).
- `struct VspaceApplied { tcb, obj, root, asid, khop }` — bản ghi **đã áp** (24 B × 8 = **192 B**).
- Tổng **256 B `.bss`** — **không** vào `Machine` (pin/trần không đổi).
- `arm_vspace(tcb, vspace)`:
  * `vspace == 0` ⇒ **huỷ** khe chờ của đúng `tcb` + `forget_applied(tcb)` ⇒ trả `true`;
  * cùng `tcb` đã có khe ⇒ ghi đè **giá trị của chính nó** (Configure hai lần liên tiếp);
  * khe rỗng ⇒ CAS `tcb: 0 → tcb` rồi ghi `obj`;
  * **tràn** ⇒ `false` (hook trả `false`; **không** ghi đè TCB khác) — fail-closed.
- `take_vspace(tcb)`: chỉ lấy khe **có `tcb` trùng**, `swap` obj rồi xoá khoá.
- `record_applied(tcb, …)`: ghi đè bản ghi **của chính `tcb`**; tràn ⇒ **không ghi** (TAP đỏ thay vì
  xanh oan). `forget_applied(tcb)` xoá cả bản ghi đó.
- `pub fn vspace_applied(tcb: ObjId)` — API **theo TCB** (trước: `vspace_applied()` toàn cục).
- Giữ nguyên hợp đồng KT-320: `Configure` là invocation của **caller**, `satp` chỉ ghi được lúc
  scheduler `sret` vào **đích** ⇒ vẫn là "khe chờ tiêu thụ **một lần** ở lượt kế tiếp của đúng TCB";
  KT-325 chỉ đổi **khoá** từ toàn cục sang `(tcb)`.

### 2.3 Vì sao **không** đọc thẳng `Machine::tcb_vspace` trong `run_task`

Đó là phương án khác (bỏ hẳn khe chờ). Không chọn vì: (a) `run_task` hiện **không** giữ `&Machine`
(R1/không đệ quy `with_machine` trong syscall); (b) đổi sang **bền** (mọi lượt vào U-mode đều áp lại)
là **đổi ngữ nghĩa** ngoài phạm vi nợ (KT-320 đã công bố "một lần"); (c) khe theo TCB đã đủ đóng nợ
và giữ nguyên mọi bằng chứng cũ.

## 3. `size_of::<Machine>()` trước/sau — **không đổi**

| Thời điểm | `size_of::<Machine>()` | `size_of::<TcbData>()` | pin D2-84 | trần KT-312 |
|---|---:|---:|---:|---:|
| **Trước** (base `42cca7cd`) | **184.792** | 824 | 196.608 | 184.832 |
| **Sau** (commit `5939b923`) | **184.792** | 824 | 196.608 | 184.832 |

Đo bằng **cùng** test `tls_tests::pin_machine_tang_dung_8_byte_moi_tcb` (`--nocapture`):

```
# tls pin: size_of_machine=184792 pin_cu=188872 delta=-4080 size_of_tcbdata=824 size_of_message=816 max_tcbs=64 8xmax=512 kt282=1536 kt309_tcb=512 kt309_msg=3072 kt309_scalar=16 kt311_tcb=512 kt312_giam=10240 tinh=-4080 khop=1 duoi_pin=1
```

Đo lại **trên ảnh boot** (`ktest`, log `ktest-smp1-1.log`):

```
# core: link addr=0x80255800 bytes=184792 pin=196608 image_cap=262144 parts=29 sum=184792 scalars=311 padding=1 sum_ok=1 in_image=1
# core: phase fails=0 static=184792 pin=196608 image_cap=262144
```

`.data+.bss` ảnh `ktest-ipc` (runner đo bằng `llvm-size`, trần 262.144):

```
# image=ktest-ipc path=…/vifast-kbin-ktest-ipc file=440096 data+bss=205608 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
```

## 4. Test đỏ trước (D2-05) — core

Thêm **3** test vào `vifast-core/src/configure_tests.rs` **trước** khi sửa code: chạy trên code cũ ⇒
**3 đỏ** đúng lý do (`Err(2)` = nếu CPTR rỗng bị tra như ô 0 thì ra `WrongObjectType`):

```

thread 'configure_tests::configure_vspace_xoa_goi_hook_arch' (110251194) panicked at vifast-core/src/configure_tests.rs:1088:5:
assertion `left == right` failed
  left: Err(2)
 right: Ok((0, 0))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- configure_tests::configure_vspace_xoa_nguyen_tu stdout ----

thread 'configure_tests::configure_vspace_xoa_nguyen_tu' (110251195) panicked at vifast-core/src/configure_tests.rs:1041:5:
assertion `left == right` failed: entry lẻ ⇒ 6 (kiểm TẤT CẢ trước khi ghi)
  left: Err(2)
 right: Err(6)

---- configure_tests::configure_vspace_xoa_ve_zero stdout ----

thread 'configure_tests::configure_vspace_xoa_ve_zero' (110251196) panicked at vifast-core/src/configure_tests.rs:976:5:
assertion `left == right` failed: CPTR rỗng ⇒ xoá vspace, KHÔNG tra ô 0 (nếu tra: ô 0 = CNode ⇒ 2)
  left: Err(2)
 right: Ok((0, 0))


failures:
    configure_tests::configure_vspace_xoa_goi_hook_arch
    configure_tests::configure_vspace_xoa_nguyen_tu
    configure_tests::configure_vspace_xoa_ve_zero

test result: FAILED. 3 passed; 3 failed; 0 ignored; 0 measured; 88 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p vifast-core --lib`
```

Sau khi sửa: **94 passed / 0 failed** (base 91 ⇒ **+3**).

## 5. Đo trên QEMU (ảnh `ktest-ipc`, TAP **10**)

- **TAP 8** đổi sang API theo TCB: `vspace_applied(rig.tcb_t)` — giá trị đo **sau** lượt chạy của
  TCB thứ nhất; **vẫn** `applied=20 match=1 root=0x804f8000 asid=1`.
- **TAP 10** (`ipc: tcb configure vspace clear + hai tcb`) — ba TCB, ba nhóm bằng chứng **đo độc lập
  ở kernel**:
  * T1 (tcb 10): `TcbData.vspace` = VSpace **A** (obj 20); `vspace_applied(T1)` = `(20, match=1)` —
    **đọc lại SAU khi TCB thứ hai đã áp** (`ap1=20`) ⇒ không bị đè.
  * T2 (tcb 14): `TcbData.vspace` = VSpace **B** (obj **21**, VSpace **thật** thứ hai của object
    layer, root `0x804fa000` ≠ `loaded.root` `0x804f8000`); `vspace_applied(T2)` = `(21, match=0)`
    ⇒ arch **từ chối** vào U-mode (`Stop::Blocked`, fail-closed) nhưng bản ghi **riêng** vẫn là B.
  * T0 (tcb 11): cấu hình A + `entry?` rồi **xoá** `vspace?` (CPTR rỗng) ⇒ `TcbData.vspace == 0`,
    `entry` **giữ nguyên** (`0x40001926`), tác vụ **chạy thật** ở `vspace_clear_entry` (in
    `# ipc: vspace clear ran=1`) và `vspace_applied(T0) == None` ⇒ arch **không** áp VSpace cũ nữa.
  * Bằng chứng gộp: `ap1=20 ap2=21 ap0=0 match2=0 diff=1 st2=Running` — **hai TCB cho hai giá trị
    khác nhau**.
- **Ca âm** (giữ nguyên ngữ nghĩa, có đo): mask bit lạ ⇒ 5; CPTR **khác 0** không phải `VSpace`
  (cap `Frame`) ⇒ **2**; `entry` lẻ ⇒ 6; `priority` ngoài dải ⇒ 5; `cspace` không phải `CNode` ⇒ 2;
  ô rỗng ⇒ 8; CPTR ngoài bảng ⇒ 1. **Đổi tiền đề duy nhất** (§4.1): ca âm `configure2` của KT-311
  trước dùng CPTR **0** (ô 0 = cap `CNode` ⇒ 2) — nay CPTR 0 là **đường xoá**, nên ca âm dùng ô
  **4** (cap `Frame` ⇒ 2, in ra vẫn `vspace=2`); ca âm rig-side `Resume(entry = 0)` dời từ TCB thứ
  hai sang TCB thứ ba (TCB thứ hai nay được client ghi `entry?` cho TAP 10a) — cả hai **giữ nguyên
  đúng ngữ nghĩa** cần kiểm.

**Log QEMU nguyên văn** (`ktest-ipc`, `-smp 1`, lần chạy của cổng cuối `5939b923`):

```

OpenSBI v1.8.1
   ____                    _____ ____ _____
  / __ \                  / ____|  _ \_   _|
 | |  | |_ __   ___ _ __ | (___ | |_) || |
 | |  | | '_ \ / _ \ '_ \ \___ \|  _ < | |
 | |__| | |_) |  __/ | | |____) | |_) || |_
  \____/| .__/ \___|_| |_|_____/|____/_____|
        | |
        |_|

Platform Name               : riscv-virtio,qemu
Platform Features           : medeleg
Platform HART Count         : 1
Platform HART Protection    : pmp
Platform IPI Device         : aclint-mswi
Platform Timer Device       : aclint-mtimer @ 10000000Hz
Platform Console Device     : uart8250
Platform HSM Device         : ---
Platform PMU Device         : ---
Platform Reboot Device      : syscon-reboot
Platform Shutdown Device    : syscon-poweroff
Platform Suspend Device     : ---
Platform CPPC Device        : ---
Firmware Base               : 0x80000000
Firmware Size               : 321 KB
Firmware RW Offset          : 0x40000
Firmware RW Size            : 65 KB
Firmware Heap Offset        : 0x47000
Firmware Heap Size          : 37 KB (total), 0 KB (reserved), 12 KB (used), 23 KB (free)
Firmware Scratch Size       : 4096 B (total), 1464 B (used), 2632 B (free)
Runtime SBI Version         : 3.0
Standard SBI Extensions     : time,rfnc,ipi,base,hsm,srst,pmu,dbcn,fwft,legacy,dbtr,sse
Experimental SBI Extensions : none

Domain0 Name                : root
Domain0 Boot HART           : 0
Domain0 HARTs               : 0*
Domain0 Region00            : 0x0000000080040000-0x000000008005ffff M: (F,R,W) S/U: ()
Domain0 Region01            : 0x0000000080000000-0x000000008003ffff M: (F,R,X) S/U: ()
Domain0 Region02            : 0x0000000000100000-0x0000000000100fff M: (I,R,W) S/U: (R,W)
Domain0 Region03            : 0x0000000010000000-0x0000000010000fff M: (I,R,W) S/U: (R,W)
Domain0 Region04            : 0x0000000002000000-0x000000000200ffff M: (I,R,W) S/U: ()
Domain0 Region05            : 0x000000000c400000-0x000000000c5fffff M: (I,R,W) S/U: (R,W)
Domain0 Region06            : 0x000000000c000000-0x000000000c3fffff M: (I,R,W) S/U: (R,W)
Domain0 Region07            : 0x0000000000000000-0xffffffffffffffff M: () S/U: (R,W,X)
Domain0 Next Address        : 0x0000000080200000
Domain0 Next Arg1           : 0x0000000087e00000
Domain0 Next Mode           : S-mode
Domain0 SysReset            : yes
Domain0 SysSuspend          : yes

Boot HART ID                : 0
Boot HART Domain            : root
Boot HART Priv Version      : v1.12
Boot HART Base ISA          : rv64imafdch
Boot HART ISA Extensions    : sstc,zicntr,zihpm,zicboz,zicbom,sdtrig,svadu
Boot HART PMP Count         : 16
Boot HART PMP Granularity   : 2 bits
Boot HART PMP Address Bits  : 54
Boot HART MHPM Info         : 16 (0x0007fff8)
Boot HART Debug Triggers    : 2 triggers
Boot HART MIDELEG           : 0x0000000000001666
Boot HART MEDELEG           : 0x0000000000f4b509
TAP version 13
1..10
# ipc rig: stvec=0x802000a0 untyped=0 core_untyped=1 cspace_c=17 cspace_s=18 cspace_alt=19 tcb_c=3 tcb_s=4 ep=5 reply=6 ipc_buf=16
# ipc: configure ok va=0x40020000
# ipc: configure err=0
# ipc: configure neg cap=18 method=18 outside=5 ok=1
# ipc configure: method=0 delta=3 denied=2 bound=1 va=0x40020000
# ipc call: blocked=1 state=BlockedOnSend call=1 ep_len=1 ep_mr=0x1234 label=0x280 stop=Blocked
# ipc server: recv=1 label=0x280 mr=0x1234 client=BlockedOnReply caller=3 server=Running stop=Yield
# ipc: server ran nr=1
# ipc reply: client=Ready reply_caller=0 server=BlockedOnRecv stop=Blocked
# ipc: umode call ok reply=23130
# ipc: mr count=8 mr7=0x28210007
# ipc mr: round=1 srv_len=8 srv_mr4=0x28200004 srv_mr7=0x28200007 cli_len=8 cli_mr4=0x28210004 cli_mr7=0x28210007
# ipc client: reply_label=0x281 reply_mr0=0x5a5a reply_mr1=1 const:label_reply=0x281 const:reply_value=0x5a5a
# ipc: server ran nr=2
# ipc mr: round=2 srv_len=64 srv_mr4=0x28200004 srv_mr63=0x2820003f
# ipc: mr count=64 mr63=0x2821003f
# ipc mr: round=2 cli_len=64 cli_mr4=0x28210004 cli_mr63=0x2821003f
# ipc: cap busy=7 ok=1
# ipc: cap used slot=5 copy=6
# sys: nr=14 err=0 value=0 tcb=4 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=4 state=Inactive
# ipc caps: sent=1 received=1 slot=5 rights=0xb obj=16 ty=Frame alive=1 copy_slot=6 copy_obj=16 copy_ty=Frame same=1
# ipc: cap sent=1 slot=5
# ipc: cap neg grant=3 over=5 ok=1
# ipc: configure2 ok=1 entry=0x40000112 prio=4
# ipc: configure2 neg align=6 prio=5 cspace=2 vspace=2 ok=1
# ipc: configure2 entry=0x40000112 prio=4 cspace=17 vspace=0 calls=2 denied=5 ok=1
# ipc: resume ok=1 entry=0x400018e4
# ipc: resume neg noentry=5 running=17 exited=17 ok=1
# ipc: configure3 ok=1
# ipc: configure3 neg mask=5 frame=2 ok=1
# ipc: vspace2 set=0 clear=0 entry=0x40001926 ok=1
# ipc: neg empty=1 reply=18 ok=1
# ipc: neg mr over=5 ok=1
# ipc: suspend ok=1 entry=0x400018e4
# ipc: suspend neg defer=5 arity=5 arity2=5 inactive=17 blocked=17 ro=3 ok=1
# sys: nr=14 err=0 value=0 tcb=3 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=3 state=Inactive
# ipc: resume ok entry=0x400018e4 state=Running
# ipc: resume neg noentry=5 state=Inactive
# ipc: suspend state=Inactive suspended=0 calls=2 resumed=Running blocked=BlockedOnSend
# ipc: resume ran entry=0x400018e4
# sys: nr=14 err=0 value=0 tcb=10 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=10 state=Inactive
# ipc: vspace ok obj=20 applied=20 match=1 root=0x804f8000 asid=1
# ipc: vspace clear ran=1
# sys: nr=14 err=0 value=0 tcb=11 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=11 state=Inactive
# ipc: vspace2 t1=10 t2=14 t0=11 v1=20 v2=21 v0=0 ap1=20 ap2=21 ap0=0 match2=0 diff=1 st1=Inactive st2=Running st0=Inactive
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
ok 9 - ipc: tcb suspend
ok 10 - ipc: tcb configure vspace clear + hai tcb
PANIC: KT-325 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + configure vspace + tcb suspend + vspace clear + hai tcb xong (10 TAP, 0 fail)
```

Cùng dòng bằng chứng ở `-smp 2` và `-smp 4` (giống **từng ký tự**, chỉ khác `Platform HART Count`):

```
# ipc: vspace2 t1=10 t2=14 t0=11 v1=20 v2=21 v0=0 ap1=20 ap2=21 ap0=0 match2=0 diff=1 st1=Inactive st2=Running st0=Inactive
```

## 6. Đột biến

### (i) **Bỏ nhánh xoá** (`invoke::configure`: CPTR 0 tra như ô 0)

Sửa tạm `invoke.rs` (bỏ `if extra.vspace == 0`), chạy lại **3 test core** ⇒ **3 đỏ**:

```

thread 'configure_tests::configure_vspace_xoa_goi_hook_arch' (110548848) panicked at vifast-core/src/configure_tests.rs:1093:5:
assertion `left == right` failed
  left: Err(2)
 right: Ok((0, 0))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- configure_tests::configure_vspace_xoa_nguyen_tu stdout ----

thread 'configure_tests::configure_vspace_xoa_nguyen_tu' (110548849) panicked at vifast-core/src/configure_tests.rs:1041:5:
assertion `left == right` failed: entry lẻ ⇒ 6 (kiểm TẤT CẢ trước khi ghi)
  left: Err(2)
 right: Err(6)

---- configure_tests::configure_vspace_xoa_ve_zero stdout ----

thread 'configure_tests::configure_vspace_xoa_ve_zero' (110548850) panicked at vifast-core/src/configure_tests.rs:976:5:
assertion `left == right` failed: CPTR rỗng ⇒ xoá vspace, KHÔNG tra ô 0 (nếu tra: ô 0 = CNode ⇒ 2)
  left: Err(2)
 right: Ok((0, 0))


failures:
    configure_tests::configure_vspace_xoa_goi_hook_arch
    configure_tests::configure_vspace_xoa_nguyen_tu
    configure_tests::configure_vspace_xoa_ve_zero

test result: FAILED. 3 passed; 3 failed; 0 ignored; 0 measured; 88 filtered out; finished in 0.01s

error: test failed, to rerun pass `-p vifast-core --lib`
```

Boot lại ảnh `ktest-ipc` với đột biến ⇒ **FAIL 2/10**, đúng dấu hiệu "xoá không xảy ra":

```
# ipc: vspace clear err=2
# ipc: vspace2 t1=10 t2=14 t0=11 v1=20 v2=21 v0=20 ap1=20 ap2=21 ap0=20 match2=0 diff=1 st1=Inactive st2=Running st0=Inactive
not ok 10 - ipc: tcb configure vspace clear + hai tcb: v1=20 v2=21 v0=20 entry0=0x40001926 ap1=Some((20, 2152693760, 1, true)) ap2=Some((21, 2152718336, 3, false)) ap0=Some((20, 2152693760, 1, true)) clear_refused=0 clear_exit=0 vs2_refused=1 said_vs2=0 said_ran=1 check_ipc=1 check_caps=1
PANIC: KT-325 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + configure vspace + tcb suspend + vspace clear + hai tcb CO LOI
```

(Client nhận `err=2` ở lời gọi xoá ⇒ `exit(128)`; `TcbData.vspace` của T0 **vẫn** 20 = VSpace A ⇒
`ap0=Some(A)`; đỏ đúng **lý do** "xoá không có đường".)

### (ii) **`vspace_applied` trả bản ghi toàn cục** ("lần áp gần nhất")

Sửa tạm `user_run.rs`: thêm `LAST_APPLIED` (chỉ số bản ghi vừa ghi), `vspace_applied(_tcb)` đọc
bản ghi đó (đúng ngữ nghĩa KT-320) — **bỏ** khoá TCB. Boot lại:

```
# ipc: vspace2 t1=10 t2=14 t0=11 v1=20 v2=21 v0=0 ap1=21 ap2=21 ap0=20 match2=0 diff=0 st1=Inactive st2=Running st0=Inactive
ok 8 - ipc: tcb configure vspace
not ok 10 - ipc: tcb configure vspace clear + hai tcb: v1=20 v2=21 v0=0 entry0=0x40001926 ap1=Some((21, 2152718336, 3, false)) ap1_luc_ap=Some((20, 2152693760, 1, true)) ap2=Some((21, 2152718336, 3, false)) ap0=Some((20, 2152693760, 1, true)) clear_refused=0 clear_exit=0 vs2_refused=1 said_vs2=1 said_ran=1 check_ipc=1 check_caps=1
PANIC: KT-325 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + configure vspace + tcb suspend + vspace clear + hai tcb CO LOI
```

Hai TCB đọc ra **cùng** giá trị (`ap1 = ap2 = 21`, `diff=0`) và bản ghi của T0 bị "lần áp gần nhất"
đè (`ap0 = Some(20)`); `ap1_luc_ap=20` (đọc ngay sau lượt áp của T1) vẫn đúng ⇒ **TAP 8 xanh, TAP 10
đỏ** — đột biến bị bắt **đúng thuộc tính** "không đè theo TCB", không phải đỏ lây. Đây cũng là lý do
TAP 10 **đọc lại** bản ghi của T1 **sau** lượt của T2 (nếu chỉ dùng ảnh chụp sớm, đột biến (ii) chỉ
bị bắt qua `ap0`).

Cả hai đột biến được **hoàn nguyên** (commit tạm chỉ nằm ở reflog; cây sạch); commit cuối
`5939b923` không chứa mã đột biến.

## 7. Cổng (trên commit `5939b923`, cây sạch)

| Bước | Lệnh | Kết quả |
|---|---|---|
| T0 core | `cd v2/kernel && cargo test -p vifast-core --lib` | **94 passed / 0 failed** (0 ignored) |
| check | `./v2/bin/v2 check` | **exit 0** — fmt + `clippy -D warnings` + 5 luật cấu trúc + self-test + fixtures; `TỔNG check` **112.369 ms**; `R1` không tệp nào vượt 1.500 dòng (`user.rs` **1.497**, `user_run.rs` 428, `ktest_ipc.rs` 1.384) |
| boot | `bash v2/test/boot/ktest.sh 1 matrix` | **27/27 boot sạch** (9 ảnh × smp 1/2/4), `ktest.sh: PASS`; `ktest-ipc` **10/10 TAP** ở cả ba smp |
| danh tính bản dựng | `REV-STATE` / `REV-GATE` | `head_rev=5939b9237892 inputs=20fb3420f8a0a43e count=332 **dirty=0**`; `REV-GATE: ACCEPT` |
| harness | `bash v2/test/boot/selftest.sh <log ktest smp1> 1` | **28/28 ca**, gồm ca (s) mới: xoá dòng `# ipc: vspace2 t1=` ⇒ runner đỏ `ev37`; `selftest.sh: PASS` |
| ngân sách ảnh | runner (`budget.sh` tích hợp) | `ktest-ipc data+bss=205608 / 262144` (headroom **56.536 B**) |
| đo hợp lệ | D2-116 | lần chạy **cuối**: `MEASUREMENT_INVALID` = **0** (`qemu_foreign` = 0). Lần chạy trước có **một** dòng `MEASUREMENT_INVALID tag=after ly_do=qemu_foreign=2` (QEMU của **agent khác**) — TAP/bằng chứng KT-325 **không** dùng số thời gian nên không ảnh hưởng |

## 8. Rủi ro / việc chưa làm

1. **Ô 0 CSpace không còn dùng được làm CPTR `vspace?`** (quy ước "0 = xoá"). Hiện ô 0 là cap
   `CNode` của chính cspace nên không mất khả năng thật; nếu tương lai cần đặt `VSpace` ở ô 0 thì
   phải mở D2 mới (đây là **quyết định ngữ nghĩa**, không phải nới ngưỡng).
2. **Bảng khe 8** (`.bss` 256 B) — tràn ⇒ fail-closed: `arm_vspace` trả `false` (hook `false`) và
   `vspace_applied` trả `None`. Ảnh hiện dùng **3** khe (T1/T2/T0). Hook **không** có kênh lỗi về
   U-mode (nợ KT-320 giữ nguyên, ngoài phạm vi KT-325) ⇒ Configure vẫn trả `(0,0)` dù arch không
   giữ được; TAP/`vspace_applied` sẽ đỏ nếu điều đó xảy ra trong ảnh này.
3. **TCB bị arch từ chối** (VSpace B, root khác ảnh nạp) vẫn ở trạng thái core `Running` — arch
   **không** sửa trạng thái core (D2-83), TAP 10 ghi `st2=Running` như **chẩn đoán**. Không thêm
   đường "đánh dấu lỗi" trong phạm vi này.
4. **`D2-226` không tồn tại** trong `DECISIONS.md` (mục cuối D2-225) — xem §0; nếu lead muốn một
   quyết định riêng cho KT-325 thì phải do Agent 0 thêm (tôi **không** sửa `DECISIONS.md`).
5. **Chưa chạy** `gate --fast` (T2) / nightly / soak: KT-325 chạm `kernel/` (core + plat + ảnh test)
   nhưng **không** chạm đồng bộ SMP/memory ordering/page table/boot layout/toolchain; phạm vi task
   yêu cầu `v2 check` + `ktest.sh 1 matrix`. **Đề nghị lead** chạy T2 trên ứng viên tích hợp (luật
   AGENTS §6.1 áp cho người tích hợp, không phải sub-agent trong 3 h).
6. Log QEMU của **cả matrix** (27 log) nằm ở `$TMPDIR/k3-ktest/ktest-*-smp(1, 2, 4)-1.log` của máy
   chạy; nhật ký này chép **nguyên văn** log `ktest-ipc` smp1 (ảnh đo) + dòng tóm tắt.

## 9. Xác nhận

- **Không** push, **không** merge, **không** chạm `main`; chỉ **một** commit local **`5939b923`** trên
  nhánh `task/kt325-vspace2` (cộng commit nhật ký này). Hai commit đột biến tạm đã bị `git reset`,
  không nằm trên nhánh.
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/`: `git diff --name-only 42cca7cd..HEAD` = **14 tệp** (11 tệp `v2/kernel/**`, 3 tệp
  `v2/test/boot/**`) — không tệp nào trong bốn nhóm cấm.
- **Không** nới ngưỡng nào: pin `Machine` **196.608**, trần footprint **184.832**, trần ảnh
  **262.144**, seed/`MAX_*`/`KERNEL_STATIC_BUDGET` — không đổi một số nào. `size_of::<Machine>()`
  **không đổi** nên không có gì để nới.
- **Không** `#[ignore]`/`#[should_panic]`/retry; **không** `cfg(target_arch)` trong `vifast-core`.
- `git add` **đường dẫn cụ thể** (14 tệp); cây sạch sau commit (`git status --porcelain` rỗng).
