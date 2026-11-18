# K6 — `Tcb.SetTls` + `exec` chain (KT-74)

Trạng thái: **DONE phần `SetTls` + `exec` + chuỗi ba bước + không rò** (K6, sau KT-73). Ảnh test
riêng **`ktest-exec`**. **Ảnh sản phẩm: BLOCKED** phần U-mode/shell — lý do + số ở §7 (nợ
**D2-113** vẫn mở cho KT-76).

Quy ước tài liệu (**D2-112**): **mọi số đo nằm trong khối** ```text scope=<tệp-log>``` **và mỗi
dòng là trích nguyên văn** từ đúng tệp đó; ngoài khối chỉ có **định danh** (mã task, hex, số hiệu
spec dạng `x.y`, `file:line`, ngày). Không có "haystack chung", không bảng ngoại lệ chứa số đo,
và log mutation/ca âm **không** bao giờ là nguồn. Script kiểm:
[`JOURNAL/kt74-verify-numbers.sh`](JOURNAL/kt74-verify-numbers.sh) (chạy lại được, `KT74_DOCS_ROOT`
trỏ được vào **bản copy** — luật **F** của D2-113).

## 1. Phạm vi và nợ

| Nợ | Trạng thái sau KT-74 |
|---|---|
| K6 §10.1 "exec chain" | **ĐÓNG** — `exec(path)` nạp ELF mới **thay** ảnh, giữ `pid`/cspace, giải phóng VSpace+Frame cũ, **nguyên tử** khi lỗi; chuỗi **ba** bước A→B→C chạy thật |
| K6 §4 "`SetTls` ở K6" | **ĐÓNG (phần core + U-mode)** — invocation đúng method id trong `spec/abi.toml`, lưu trong TCB, nạp vào `x4`/`tp` của U-mode mỗi lần resume; test host + TAP QEMU |
| D2-113 "ảnh sản phẩm **không** có U-mode/shell" | **CHƯA ĐÓNG** — chứng minh **bất khả trong ngân sách task** + số đo; xem §7 (**BLOCKED**, không phải "trần") |
| D2-104 (arity chính xác) | **ÁP** cho `Tcb.SetTls`: `length` khác **một** ⇒ `InvalidArgument`; xem §3.2 |

## 2. `Tcb.SetTls` — method id **lấy từ spec**, không tự đặt

`spec/abi.toml` `[object].Tcb` khai `SetTls` (method id do spec cấp); `vifast-gen` sinh
`vifast_abi::TCB_SETTLS` từ chính bảng đó, và core **không** viết số:

- `kernel/vifast-core/src/invoke.rs`: `pub const TCB_SET_TLS: u32 = vifast_abi::TCB_SETTLS;`
  kèm `tcb_set_tls_matches_spec()` (đối chiếu lại bảng `vifast_abi::TCB_METHODS`).
- Nhánh dispatch `(ObjType::Tcb, TCB_SET_TLS)` ⇒ `set_tls(...)`; **không** thêm method/syscall nào.
- **Không sửa `spec/`** (D2-99 hết lượt): bảng `[object].Tcb` **đã có** `SetTls` nên chỉ cần
  **cài đặt**, đúng tiền lệ **D2-97**.

### 2.1 Ngữ nghĩa

| Khía cạnh | Quyết định | Vì sao |
|---|---|---|
| Kênh tham số | `MR0 = tls`, `length` = **một**, trả `a1` = **không** | D2-99 (`MR0..MR3`, `label` = method); `MessageInfo` **không** đổi layout |
| Arity | **chính xác một** tham số (thiếu **và** thừa đều `InvalidArgument`) | D2-104 |
| Quyền | cap TCB phải có `RIGHT_WRITE`, thiếu ⇒ `InsufficientRights` | cùng luật D2-99(e) |
| Trạng thái TCB | ghi được ở **mọi** trạng thái sống; giá trị có hiệu lực ở lần vào U-mode **kế tiếp** | chuỗi `exec → SetTls → resume` không cần TCB `Inactive` (khác seL4 `TCB_SetTLSBase`) |
| Nơi lưu | `TcbData.tls: u64` (model-only `[TK]`) + `TcbInfo.tls` | một nguồn cho arch; kernel **không** đọc vùng TLS |
| Nạp vào user | `user::apply_tls` ghi `tf.x[REG_TP]` (=`x4`) ở `enter_with`, `run_task`, `exec_frame` | RISC-V: `tp` = `x4`; mọi lần resume đều lấy từ **core** |

### 2.2 Phát hiện kiến trúc: `tp` đang là **hart id** của kernel

Kernel dùng `tp` làm hart id (`read_hart_id` = `mv ?, tp`; `_start` nạp từ `a0`), và `sret` xuống
U-mode nạp **toàn bộ** GPR từ trapframe — kể cả `x4`. Trước KT-74 điều này **vô hại** vì `tp` của
user luôn bằng **không** và handler U-mode **không** gọi `hart_id()`. Khi `SetTls` đặt `tp` khác **không**,
`current_hart()` trong handler trả **rác** ⇒ `dispatch_on_private_stack` panic
(`syscall stack chua cap`). **Ca âm** (D2-108B) — bỏ **đúng một dòng** vá, boot lại:

```text scope-red=kt74-red-tp.log
# stack: chua cap cho hart=7 (init_syscall_stacks)
PANIC: syscall stack chua cap
```

`hart` in ra chính là `tls` của TCB (`0x7431_0001`) bị cắt bởi `min(MAX_HARTS-1)` ⇒ chứng minh
`hart_id()` đã đọc `tp` **của user**, không phải giá trị kernel tự khai. Khôi phục ⇒ md5 tệp trùng
và boot lại đạt đủ TAP.

**Vá (một đường, không mở rộng `unsafe`):** `user::restore_kernel_tp()` nạp lại `tp` từ
`USER_KERNEL_CTX[3]` (do asm `user_enter` lưu) ở **đầu** `on_u_exception`; đường `sret` về U-mode
tự khôi phục `x4` từ frame (`ld x4, …` của `trap_entry`) nên **không** rò hart id sang user, và
`user_exit_resume` cũng nạp lại `tp` sau `Exit`. Bằng chứng: `# tls:` có `observed` =
giá trị `SetTls` **và** TAP kernel/SMP của các ảnh cũ vẫn xanh (matrix §5).

### 2.3 Bằng chứng `x4`/`tp` là **phần cứng**

Trap entry lưu **toàn bộ** GPR vào trapframe, nên `tf.x[REG_TP]` tại `ecall` **là** `tp` của U-mode.
Không cần `mv ?, tp` trong chương trình U-mode ⇒ **không** `unsafe` trong mọi chương trình U-mode
(giữ D2-93/D2-101). Hai lượt chạy của chương trình A chứng minh đúng hợp đồng "hiệu lực ở lần
resume kế tiếp": lượt đầu `observed_first` bằng không, lượt sau `observed` bằng giá trị đã `SetTls`.

```text scope=kt74-qemu-ktest-exec.log
# tls: set=0x74310001 observed_first=0 observed=0x74310001 method=15 const:method_source=spec traps=6 stats=2 sys0_calls=2 sys0_err=0 exit=1 exit2=1 tcb=0x3 tcb_tls=0x74310001 ok=1
ok 1 - tls dat qua invocation
```

## 3. Ca âm của `SetTls`

| Ca | Kỳ vọng | Đo được |
|---|---|---|
| method **chưa cài** của `Tcb` (`SetPriority`) | `NotSupported` | `method_bad` |
| `length` **thiếu** | `InvalidArgument` | `arity0` |
| `length` **thừa** | `InvalidArgument` | `arity2` |
| cap **Untyped** + label `SetTls` | `NotSupported` (danh sách trắng hẹp) | `cap_untyped` |
| cap TCB **chỉ đọc** (`RIGHT_READ`) | `InsufficientRights` | `cap_ro` |
| sau **mọi** ca âm | TLS **giữ nguyên** giá trị cũ | `tls_giu`, `set_tls_unchanged` |

```text scope=kt74-qemu-ktest-exec.log
# tls neg: method_bad=18 arity0=5 arity2=5 cap_untyped=18 cap_ro=3 tls_giu=0x74310001 set_tls_unchanged=1 ok=1
ok 2 - tls ca am
```

Test **host** (`kernel/vifast-core/src/tls_tests.rs`, `cargo test -p vifast-core`) phủ cùng ma
trận **cộng** ba khẳng định chỉ kiểm được ở host: số method khớp bảng sinh từ spec, `SetTls`
**không** đổi trạng thái TCB, và ghi **đè** (TLS là giá trị, không cộng dồn).

## 4. `exec(path)` — nạp ELF mới **thay** ảnh hiện tại

### 4.1 Vì sao `exec` là **kernel-mediated** (không thêm syscall, không thêm method)

ABI hiện tại **không** có đường cho U-mode yêu cầu `exec`: `[syscall]` đóng băng (`[frozen]`), và
`[object].Tcb` **không** có method `Exec`. Ràng buộc task là **không thêm syscall**, nên cơ chế
nạp ảnh do **kernel** cài (`kernel/vifast-plat-riscv64/src/exec.rs`), còn *quyết định exec tiến
trình nào sang ảnh nào* là **policy** — cùng tiền lệ **D2-96** ("memsrv POLICY-ONLY,
kernel-mediated"). Nợ ABI ghi ở §8: `Tcb.Exec(path)` chỉ có nghĩa khi `path` đọc được từ **VFS
thật** (KT-75+), không phải bảng ảnh nhúng.

### 4.2 Hợp đồng

| Yêu cầu | Cài đặt |
|---|---|
| giữ `pid` | nạp vào **cùng ô** bảng dải (`user::load_init_slot(..., pid)`), không tạo tác vụ mới |
| giữ `cspace` | **không** tạo/thu hồi TCB core ⇒ CSpace, cap, `fault_ep`, priority nguyên vẹn |
| reset `entry`/`sepc` | `user::exec_frame` = trapframe **mới** từ `e_entry` ảnh mới, `sp` = đỉnh stack user |
| reset thanh ghi | **mọi** GPR = **không** (kể cả `a0` — ảnh mới không thấy tham số ảnh cũ) |
| reset TLS | xoá TLS của TCB (`tcb_set_tls` với **không**) **trước** khi hoán ảnh ⇒ ảnh mới vào với `tp` = giá trị đó |
| giải phóng ảnh cũ | `init_pages::release_with` (unmap → flush → revoke từng Frame + VSpace) |
| **nguyên tử** | tra `path` và **parse**/kiểm W^X/overlap ELF **trước** khi cấp gì; **mọi** đường lỗi của loader đi qua `bail!` ⇒ trả lại Frame/VSpace vừa cấp; ảnh cũ **chỉ** bị giải phóng **sau** khi ảnh mới nạp xong |
| đo không rò | `# exec:` in `objects/frames/caps` **trước → sau** + `check_objmem` |

### 4.3 `entry` của bốn ảnh **giống nhau** — và vì sao đó không phải lỗi

`init.ld` link mọi chương trình U-mode tại `USER_BASE`, nên `e_entry` bằng nhau. Vì vậy "reset
`sepc`" **không** được chứng minh bằng `entry` khác nhau (evidence ghi rõ
hằng số `elf_linked_at_base` bằng một), mà bằng: (a) **VSpace đổi** (map lại toàn bộ ảnh mới), (b) ảnh mới
**chạy thật** với mã thoát riêng, (c) ảnh mới **khác byte** ảnh cũ (`img_differ`, `img_a`/`img_b`
đọc từ `include_bytes`).

```text scope=kt74-qemu-ktest-exec.log
# exec: step=1 pid=0 tcb=3 cspace_same=1 old_vspace=2 new_vspace=14 vspace_changed=1 old_entry=0x40000000 new_entry=0x40000000 const:elf_linked_at_base=1 img_differ=1 img_a=8520 img_b=8496 freed=11 new_pages=11 exit=2 objects=17 frames=11 caps=14 vspaces=1 page_tables=3 mappings=11 -> objects=17 frames=11 caps=14 vspaces=1 page_tables=3 mappings=11 same=1 check=1 ok=1
ok 3 - exec giu pid cspace
```

## 5. Chuỗi **ba** bước + không rò

Mỗi bước: `exec` → **chạy thật** (mã thoát riêng) → `exec` tiếp. `recycled` là **giá trị suy ra**
(D2-106): `recycled = freed − (after.frames − before.frames)` — khi không rò thì bằng `freed`.
Ba bước cộng lại bằng **ba lần** `freed` của một bước ⇒ **Frame ảnh cũ đã được ảnh mới dùng lại**,
không cộng dồn bộ đếm.

```text scope=kt74-qemu-ktest-exec.log
# exec chain step: i=1 path=/c pid=0 tcb=3 freed=11 recycled=11 new_vspace=2 new_entry=0x40000000 exit=3 frames=11->11 same=1 check=1
# exec chain step: i=2 path=/d pid=0 tcb=3 freed=11 recycled=11 new_vspace=14 new_entry=0x40000000 exit=4 frames=11->11 same=1 check=1
# exec chain step: i=3 path=/a pid=0 tcb=3 freed=11 recycled=11 new_vspace=2 new_entry=0x40000000 exit=1 frames=11->11 same=1 check=1
# exec chain: execs=3 pid=0 tcb=3 exits=3,4,1 recycled=33 freed=33 objects=17 frames=11 caps=14 vspaces=1 page_tables=3 mappings=11 -> objects=17 frames=11 caps=14 vspaces=1 page_tables=3 mappings=11 frames_same=1 objects_same=1 caps_same=1 total=4 ok=1
ok 4 - exec chain 3 buoc
```

**Ca âm của phép đo "không rò"** (D2-108B — bộ đếm bằng nhau mà không có ca âm thì không chứng
minh gì): bản đột biến **bỏ `release_with`** ⇒ `frames` và `vspaces` **tăng gấp đôi** (khối
`scope-red` dưới), TAP `exec giu pid cspace` **đỏ** (`same` bằng không).

```text scope-red=kt74-red-qemu.log
# exec: step=1 pid=0 tcb=3 cspace_same=1 old_vspace=2 new_vspace=14 vspace_changed=1 old_entry=0x40000000 new_entry=0x40000000 const:elf_linked_at_base=1 img_differ=1 img_a=8520 img_b=8496 freed=11 new_pages=11 exit=2 objects=17 frames=11 caps=14 vspaces=1 page_tables=3 mappings=11 -> objects=32 frames=22 caps=26 vspaces=2 page_tables=6 mappings=22 same=0 check=1 ok=0
```

Log mutation: [`JOURNAL/kt74-red-qemu.log`](JOURNAL/kt74-red-qemu.log) — **không** dùng làm nguồn
số (D2-112c); khôi phục ⇒ ảnh build lại trùng sha đã niêm yết.

## 6. Thất bại ⇒ **nguyên tử**

**Phạm vi "nguyên tử" (D3 reviewer — thu hẹp đúng sự thật):** tài nguyên (VSpace/PageTable/Frame
và cap) **hoàn nguyên** ở **mọi** đường lỗi của loader; **bảng dải VA** của ô tác vụ được **chụp
trước** và **khôi phục** khi loader lỗi muộn (`user::ranges_save`/`ranges_restore`, vì
`loading_ranges().reset()` chạy **trước** khi ảnh mới chắc nạp được). **Chưa có ca kiểm** cho đường
lỗi **muộn**: muốn tạo lỗi sau `reset()` phải làm cạn tài nguyên object layer đúng một bậc
(VSpace xong, PageTable/Frame hỏng), mà `ObjKind::Untyped` **không** được object layer hỗ trợ
(`v2/kernel/vifast-plat-common/src/obj/mod.rs:720` trả `NotSupported`) nên không dựng được untyped
nhỏ để ép lỗi. Ghi thành **nợ có ca** (KT-76), không tuyên bố mạnh hơn bằng chứng.

| Ca | Kỳ vọng | Đo được |
|---|---|---|
| `path` không có trong bảng | `NotFound`, **không** chạm gì | `path_err` |
| ELF hỏng (magic sai) | `Load(Elf)`, loader **chưa** cấp Frame nào | `elf_err` |
| sau hai ca trên | VSpace/`entry`/`objects`/`frames`/`caps` **y nguyên**, `execs` **không** tăng | `vspace_same`, `entry_same`, `frames_same`, `execs_total` |

```text scope=kt74-qemu-ktest-exec.log
# exec atomic: path_err=NotFound elf_err=Elf vspace_same=1 entry_same=1 objects=17 frames=11 caps=14 vspaces=1 page_tables=3 mappings=11 -> objects=17 frames=11 caps=14 vspaces=1 page_tables=3 mappings=11 frames_same=1 objects_same=1 caps_same=1 execs_total=4 ok=1
ok 5 - exec that bai nguyen tu
```

Nhánh **có** cấp phát (hết Frame/physmap) đi qua `bail!` trong loader
(`user.rs` khối `macro_rules! bail`) nên cũng không để lại Frame mồ côi — đây là thay đổi **có
chủ ý** của KT-74 (trước đó đường lỗi của loader bỏ rơi tài nguyên).

## 7. TLS sau `exec` (reset + đặt lại)

Sau `exec` sang ảnh **D** (D **không** gọi `SetTls`): TLS của TCB = **không** **và** `tp` quan sát = **không**.
Rồi `SetTls` (đường core trực tiếp — invocation đã kiểm ở TAP một) + resume **không** `exec`:
`tp` = giá trị mới. Đây là bằng chứng "reset đúng" và "resume lấy từ core" cùng lúc.

```text scope=kt74-qemu-ktest-exec.log
# tls reset: after_exec=0x0 observed=0 exit1=4 set=0xb2 observed2=0xb2 exit2=4 tcb_tls=0xb2 execs_total=5 ok=1
ok 6 - tls reset sau exec
# exec final: c0 objects=17 frames=11 caps=14 vspaces=1 page_tables=3 mappings=11 -> objects=2 frames=0 caps=2 vspaces=0 page_tables=0 mappings=0 releases_clean=1 fail=0 execs_total=5
PANIC: KT-74 tls va exec xong (6 TAP, 0 fail)
```

## 8. Ảnh **sản phẩm** có U-mode chưa? — **KHÔNG** (BLOCKED có số)

**Kết luận: chưa.** Ảnh sản phẩm vẫn **không** có U-mode/shell, nên K6 **không** được nhận
"tương đương v1" chỉ vì ảnh test có shell (giữ nguyên nợ D2-113 cho KT-76). Đây là **quyết định có
văn bản**, không phải bỏ sót:

**Trần KHÔNG phải lý do** — số đo dưới đây (`.data+.sdata+.bss+.sbss`, trần chung):

```text scope=kt74-product-budget.log
# image=product path=v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64 file=188552 data+bss=200696 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
```

```text scope=kt74-ktest-matrix.log
# image=ktest-exec path=/Users/mps/dvchd/ViFastOs/v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-exec file=357480 data+bss=202992 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
```

```text scope=kt74-ktest-matrix.log
# image=ktest-proc path=/Users/mps/dvchd/ViFastOs/v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-proc file=437672 data+bss=207256 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
```

Suy ra (**D2-106**, công thức ngay tại đây): `headroom(product) = tran − data_bss(product)`, và
ảnh **đã có** U-mode + procsrv + shell (`ktest-proc`) có `data+bss` **nhỏ hơn** trần. Vậy phần U-mode
quan sát được **nhỏ hơn** headroom ⇒ **không** cần tách ảnh vì trần, và **không** được nới trần.

**Lý do thật: ảnh sản phẩm không có *đường dẫn* nào tới U-mode.** Bằng chứng cấu trúc
(`file:line`):

| Bằng chứng | Ý nghĩa |
|---|---|
| `kernel/vifast-kbin-riscv64/src/main.rs` (toàn bộ) **không** có `user::`, `include_bytes!`, `procsrv`, `shell` | ảnh sản phẩm boot tuyến tính: MMU → obj → core smoke → PLIC/UART IRQ → timer → SMP → `ok` → `halt`; **không** có `sret` xuống U-mode |
| `kernel/vifast-kbin-riscv64/Cargo.toml:13` `default = []` và mọi `required-features` của bin U-mode là `ktest,user*` | module U-mode (`plat/user.rs`) **không** được biên dịch vào ảnh sản phẩm |
| call-site `user::load_init_slot` chỉ nằm trong ảnh test (`src/ktest_*.rs`) và trong `src/proc_demo.rs` (chỉ biên dịch khi bật feature của ảnh test) | chưa có driver/policy U-mode nào cho ảnh sản phẩm |

Nói cách khác: đưa U-mode vào ảnh sản phẩm **không** phải "bật một feature" mà là **viết mới**
policy procsrv + vòng scheduler U-mode + đường bàn phím (ring UART) cho ảnh sản phẩm, rồi **soak
lại** (ảnh đổi ⇒ luật soak bắt buộc). Việc đó **vượt ngân sách ba giờ** của KT-74 (đã dùng cho
`SetTls` + `exec` + chuỗi + cổng), nên **BLOCKED có văn bản** — đúng phương án (b) của D2-113.

**Ghi chú ngưỡng (D2-84/85, KHÔNG nới):** `size_of::<Machine>()` tăng do `TcbData` thêm **một**
`u64` TLS cho mỗi TCB (trần `MAX_TCBS`), và `TcbData` là phần tử của `Machine`:

```text scope=kt74-qemu-ktest.log
# core: link addr=0x80253800 bytes=189384 pin=196608 image_cap=262144 parts=29 sum=189384 scalars=295 padding=1 sum_ok=1 in_image=1
# core: phase fails=0 static=189384 pin=196608 image_cap=262144
```

Pin và trần **giữ nguyên** (giá trị trong khối evidence); giá trị mới vẫn **dưới pin**. Con số này được
cập nhật **đồng thời** ở `v2/test/boot/ktest.sh` (ba mẫu evidence `# core: link`, `# core: phase`,
`# user core:`) và ở đây — **không** phải nới ngưỡng, mà là **đo lại** sau khi thêm trường (luật
D2-109C: số phải lấy từ log).

## 9. Nợ còn lại

- **ABI `exec`**: chưa có `Tcb.Exec(path)` (không được thêm syscall trong KT-74). `path` hiện là
  khoá trong **bảng ảnh nhúng** (`ImageTable`, tối đa `MAX_IMAGES`) — VFS thật là KT-75+.
- **Ảnh sản phẩm U-mode/shell**: **BLOCKED** (§8) — nợ D2-113 chuyển nguyên trạng cho KT-76.
- **`ktest-exec` chưa có ca SMP thật cho `exec`**: matrix chạy ba cấu hình `-smp` nhưng tiến trình
  `exec` chỉ chạy trên hart boot (hart phụ park) — chưa kiểm `exec` đồng thời hai hart.
- **`MAX_INIT_PAGES`** (danh sách trang của loader) giới hạn ảnh U-mode ở một cỡ nhất định; ảnh
  lớn hơn ⇒ `LoadError::Image` (nguyên tử, không rò) nhưng **chưa** có ca kiểm chạm biên này.

## Bảng đối chiếu số ↔ khối `scope=` (KT-133, Q6 của Agent 0)

Mỗi trường dưới đây **có số đo nằm trong một khối `scope=`/`scope-red=`** của chính tệp này —
cột "Tệp log" chỉ tệp, cột "Khối" chỉ **tiết** chứa khối. Cột "Khối" là **bắt buộc** khi một trường
có **nhiều** giá trị trong tài liệu (KT-140): thiếu nó thì người đọc tra khối **đầu tiên** gặp giá
trị khác và tưởng bảng sai. Nhờ vậy mọi con số đều trích được nguyên văn từ log, đúng luật số một,
mà không phải viết số vào prose.

| Trường đo | Giá trị (chữ) | Loại khối | Tệp log | Khối |
|---|---|---|---|---|
| `observed` | không | `scope=` | `kt74-qemu-ktest-exec.log` | §7 |
| `observed_first` | không | `scope=` | `kt74-qemu-ktest-exec.log` | §2.3 |
| `elf_linked_at_base` | một | `scope=` | `kt74-qemu-ktest-exec.log` | §4.3 |
| `same` | một | `scope=` | `kt74-qemu-ktest-exec.log` | §4.3 |

**Ghi chú `observed` (KT-140 — hai khối, hai tình huống, không mâu thuẫn):** dòng bảng trên trỏ
**khối §7**, nơi `observed` bằng **không** — sau `exec` sang ảnh **D** (ảnh D **không** gọi `SetTls`, nên TLS của
TCB và `tp` quan sát đều bằng **không**). Khối **§2.3** cũng in trường `observed` nhưng là **trường
hợp khác**: `observed=0x74310001` — lượt chạy **thứ hai** của chương trình A **sau** `SetTls`, tức
giá trị TLS vừa đặt, không phải giá trị sau `exec`. Bảng chọn §7 vì cột "Giá trị (chữ)" ghi
**không**; §2.3 là **ca khác** và **không** được dùng để đọc dòng bảng này. Đây đúng là lý do cột
"Khối" tồn tại: nếu chỉ ghi tệp log, người đọc tra khối `observed` **đầu tiên** (§2.3) sẽ thấy
`0x74310001` và kết luận sai rằng bảng lệch log.