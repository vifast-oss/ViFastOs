# ViFastOs v2: Tài liệu thiết kế

Người viết: Agent 0 (Copilot). Đầu vào: ViFastOs-dossier.md (snapshot 2026-11-12, HEAD 8077d71 + mẻ r323 chưa commit) và toàn bộ thảo luận kiến trúc.
Đối tượng: người điều phối + Agent 2.
Tài liệu liên quan: `ViScript-thiet-ke.md` (ngôn ngữ script và shell, sản phẩm độc lập chạy đa nền tảng).
Giấy phép mã nguồn: **MIT OR Apache-2.0** (giống Rust).

Nhãn (QD §2.2): `[V1:CODE]`, `[V1:ĐO]`, `[V1:DOC]` = lấy từ dossier, giữ độ tin cậy gốc; `[TK]` = thiết kế mới (**mặc định** cho dòng không nhãn); `[GT]` = giả thuyết cần kiểm; `[ƯL]` = số ước lượng phải đo lại. Hằng số thiết kế là `[TK]`, không phải `[ƯL]`.

## Thay đổi sau review (2026-11-12)

Bản này đã vá theo `docs/QUYET-DINH-sau-review.md` (QD). Các nhóm thay đổi:
1. Không gian tên: D-09..D-27 → **D2-01..D2-27**; G1..G6 → **R1..R6**; task T01.. → **KT-##**; báo cáo `R<n>-v2`.
2. ABI: syscall 0..13 + **14 `Exit(code)`**; dải 15..31 reserved; 19 mã lỗi; thêm `Tcb.ReadState/Suspend(mode)/SetFaultEp`; object chưa cài giữ số reserved; `abi_version`, `min_compat`, golden file.
3. Kernel: cờ `suspended` + `Suspend(Abort/Defer)`; `Resume` không phát lại IPC; `FaultKind::Exit`; objmem API closure; slot cap 48 B; WaitAddr bảng băm; id theo cụm; MCS giai đoạn đầu.
4. Tích hợp: U1 không hứa net (net ở N1); VSH2 phụ thuộc N1; shell sở hữu tty; `term.vidl` thành mốc `TUI`.
5. Lộ trình không ngày: K0 → K0.5 → K1a → K1a' → K1b → K1c → K2 → K3 → K3.5 → K6 → K4'/K5'; chỉ riscv64 tới K6.
6. Đo lường: N=1.000 trên máy `ps`, 3 luồng; bỏ 750; harness-v2; `--fast` L1 1.000 seed.
7. v1: tag `v1-archive`, không soak lại; sổ lỗi mang sang v2 (Phụ lục A).
8. Quyết định/Q&A: D2-01..D2-27 (mục 12); Q1–Q12 của dossier (Phụ lục B).

---

## Mục lục

0. Kết luận chính
1. Chẩn đoán v1
2. Nguyên tắc và tiêu chí "làm ngay hay chừa chỗ"
3. Kiến trúc tổng thể
4. Kernel: thiết kế chi tiết
5. Platform và boot
6. User space hệ thống
7. Runtime ứng dụng, ViScript, máy ảo Linux
8. Cấu trúc crate và tái sử dụng
9. Quy trình kiểm thử theo từng phần
10. Lộ trình đầy đủ
11. Quy trình phát triển lặp liên tục cho Agent 2
12. Quyết định
13. Giao việc vòng 1
14. Rủi ro

Phụ lục A. Sổ lỗi v1 mang sang v2
Phụ lục B. Q1–Q12 của dossier

---

## 0. Kết luận chính

1. v1 có 6 lỗi thiết kế gốc; các lỗi hiếm đang săn (T33a, mất message, shootdown AP->BSP) là triệu chứng của chúng. v1 đóng băng sau r323.
2. v2 là một microkernel capability kiểu seL4, **chốt ABI lõi một lần ở K0** với mọi thứ một OS desktop thực tế cần:
   - object kernel nằm trong Untyped của người tạo (không pool cố định);
   - CSpace hai cấp; IPC tối đa 4 cap/message; Reply object;
   - **MCS từ đầu** (SchedContext đi theo IPC);
   - Frame 4 KiB / 2 MiB / 1 GiB với thuộc tính cache;
   - WaitOnAddress (futex) cho Rust std;
   - IRQ line + MSI; IOSpace (IOMMU); VCpu (ảo hoá) có trong ABI từ đầu, cài đặt dần;
   - mọi thao tác dài đều ngắt được và làm tiếp;
   - FPU eager, đồng hồ đọc thẳng ở user, KPTI tuỳ chọn.
3. Kernel là **máy trạng thái thuần** `handle(Event) -> Decision`; khoá và phần cứng nằm ngoài core. Đồng thời: BKL (queue lock) bây giờ, cụm khi số đo cho thấy cần, khoá mịn cục bộ cuối cùng.
4. Ứng dụng mặc định viết bằng **Rust std** (không libc). Viết nhanh không cần build lại bằng **ViScript**: kiểu tĩnh, cú pháp gọn như Python, một ngôn ngữ cho cả script và shell (ViShell), chạy đa nền tảng (tài liệu riêng). Tương thích Linux bằng **máy ảo** chạy trên VCpu.
5. Kiểm thử: **sim tất định + mô hình tham chiếu** là nền tảng; kịch bản viết một lần chạy trên sim và QEMU cả 3 arch; Miri cho phần unsafe; mutation; tự thu nhỏ ca lỗi; đo hiệu năng tự động.
6. Agent 2 chạy theo **vòng lặp chuẩn** có tệp trạng thái, hàng đợi task, ngân sách thời gian, điều kiện dừng và báo cáo máy đọc được (mục 11), để chạy qua đêm liên tục mà không trôi.

---

## 1. Chẩn đoán v1 [V1:DOC]

| # | Lỗi gốc | Triệu chứng | Hướng v2 |
|---|---|---|---|
| R1 | 58 syscall [V1:CODE], ramfs, fd, block, console, args trong kernel; kernel dựng VSpace cho server | lib.rs 17.715 dòng [V1:CODE] | 15 syscall (0..14) + invocation; dịch vụ ra user |
| R2 | Trạng thái static reset hai lần; đồng hồ nằm trong trạng thái reset được | T33a: thời gian lùi 49 về 8 | Đồng hồ phần cứng; `init` một lần |
| R3 | Trạng thái TCB tách khỏi hàng đợi; message chờ trong ô `resume` | Mất message r319; hàng 8 phần tử đầy; O(n) | Trạng thái mang object chờ; danh sách xâm nhập; ghi thẳng context |
| R4 | COPY_WINDOWS, read_user/write_user | Lỗi exec vòng 112 tới 145 [V1:ĐO] | Không con trỏ user trong kernel |
| R5 | Identity map nhỏ, pool trong .data | Trần 566 KB, 24 TCB, tràn .data AArch64 [V1:ĐO] | Nửa cao + physmap; object trong Untyped |
| R6 | Test = boot demo + grep marker | 11 vòng cho một lỗi [V1:DOC] | Sim tất định, mô hình, QEMU TAP |

Phụ: Untyped chỉ là quota (object thật nằm trong pool static, hai trần chồng nhau); SMP không có giao thức thống nhất (kênh wake lệch 3 port, poll_maintenance khác nhau, lỗ shootdown AP->BSP).

Mã lỗi gốc đổi **G1..G6 → R1..R6** (QD §2.1): chữ **G** để dành cho mốc desktop (G1..G5, mục 10.4), tránh trùng mã khi grep.

Giữ từ v1: mô hình seL4, `forbid(unsafe_code)` cho core, check_invariants + mutation, **cơ chế marker (kế thừa dưới dạng `harness-v2`, mục 9.12)**, BUILD_ID, W^X theo Frame, văn hoá test đỏ trước; thư viện vifast-pte, vifast-elf, vifast-acpi, vifast-fs, vifast-fsutil, vifast-virtio, ý tưởng hộp thư của vifast-maint. Các thư viện này **copy/port vào `v2/lib/`** (D2-26), không path-dependency sang `crates/`.

---

## 2. Nguyên tắc

### 2.1 Mười hai nguyên tắc

1. Kernel là máy trạng thái thuần; không biết khoá, không đọc thanh ghi, không đọc bộ nhớ user.
2. Một lần khởi tạo; không reset; không selftest trong boot sản phẩm.
3. Trạng thái mang theo lý do; thành viên hàng đợi suy ra từ trạng thái.
4. Không con trỏ user trong kernel.
5. Mọi thao tác có cận hằng số hoặc ngắt được và làm tiếp.
6. Cơ chế ở kernel, chính sách ở user.
7. Ai tạo object người đó trả bộ nhớ (object trong Untyped).
8. Tất định trước, phần cứng sau.
9. Một nguồn sự thật cho ABI, cấu hình, giao thức, định dạng; phần còn lại sinh ra.
10. Viết một lần, chạy nhiều nơi (thư viện host/OS; kịch bản sim/QEMU).
11. `unsafe` chỉ ở crate nhỏ có tên, mỗi crate có kiểm riêng (Miri hoặc loom/stress).
12. Tối ưu hiệu năng chỉ khi có số đo; nhưng **hình dạng ABI** phải đúng từ đầu.

### 2.2 Tiêu chí phân loại

Với mỗi tính năng: *thêm sau có phải đổi ABI hoặc viết lại lõi không?*
- Có: **làm ngay** trong K0 đến K2.
- Không, nhưng cần kiểu object/method: **khai báo trong ABI từ K0**, cài đặt sau, trả `NotSupported` tới lúc đó.
- Không: **để sau**, chỉ cần không chặn đường.

| Làm ngay | Khai báo ABI, cài sau | Để sau |
|---|---|---|
| Object trong Untyped, CSpace 2 cấp, IPC 4 cap [TK], Reply, MCS phần **donation** (mục 4.5), Frame nhiều cỡ + attrs, WaitOnAddress, thao tác làm tiếp, FPU eager, đồng hồ user, hộp thư, đẩy cân bằng tải | IOSpace, MSI, VCpu + VSpace khách, KPTI, power hook, SetTls (cài ở K6), phần hoãn của MCS (refill nhiều đoạn, timeout fault) | IPC fastpath, cụm (S3), khoá mịn (S4), libc, wasm |

---

### 2.3 Ràng buộc v1 còn hiệu lực (QD §2.3)

| v1 | Trạng thái trong v2 |
|---|---|
| D-01 gate mềm | thay bằng D2-07 (không retry) cho v2; còn hiệu lực cho v1 |
| D-02 đóng băng tính năng | **hết hiệu lực** từ khi v1 lưu trữ (`v1-archive`) |
| D-03 T26 "ai chen giữa" | chuyển sang v2: **KT-T26** |
| D-04 `MAX_IO=256` khi đo | giữ cho KT-T26 (cấu hình tái hiện) |
| D-05 teardown ≤2 chỗ/vòng | giữ tinh thần: finalise theo nhóm, mỗi task ≤2 loại object, mỗi loại có test |
| D-06 soak 600..1000 trên máy `ps` | giữ: mọi soak v2 trên `ps`, N theo mục 9.12 |
| D-07 không tách `lib.rs` | hết đối tượng (v2 viết mới) |
| D-08 không đụng đồ hoạ/ViFastKey | **còn hiệu lực tới K6** |
| D11 không sửa mò | D2-02 |
| D18 hồi quy thì revert | D2-03 |
| Quy tắc 2.5 | giữ nguyên văn: hai phiên kẹt cùng ý thì đổi nhánh, không thử lần ba |

## 3. Kiến trúc tổng thể

```
┌────────────────────────────────────────────────────────────────────────┐
│ ỨNG DỤNG: App Rust (std) │ ViScript + ViShell │ Linux khách (qua VMM)   │
├────────────────────────────────────────────────────────────────────────┤
│ RUNTIME (trong tiến trình): Rust std ViFastOs │ vifast-urt │ vifast-alloc│
│   cap allocator │ mutex/condvar trên WaitOnAddress │ đồng hồ kiểu vDSO  │
│   stub sinh từ IDL │ VM ViScript                                       │
├────────────────────────────────────────────────────────────────────────┤
│ SERVER HỆ THỐNG                                                        │
│  init │ memory server │ process server │ VFS (pager, page cache)       │
│  driver: virtio, NVMe, AHCI, xHCI (ring + MSI + IOSpace)               │
│  net │ power │ compositor │ input │ ViFastKey (IME) │ audio │ VMM       │
├────────────────────────────────────────────────────────────────────────┤
│ KERNEL (vifast-core, forbid unsafe)                                     │
│  objmem: object trong Untyped │ CSpace 2 cấp + MDB │ IPC + Reply ≤4 cap │
│  MCS │ scheduler theo hart + đẩy cân bằng │ Notification │ WaitOnAddress│
│  VSpace/PageTable/Frame 4K/2M/1G │ ASID + shootdown │ IOSpace │ VCpu   │
│  IrqHandler + MSI │ thao tác làm tiếp │ trace ring                      │
├────────────────────────────────────────────────────────────────────────┤
│ PLATFORM: vifast-plat-common (khoá, vòng trap, hộp thư) + 3 arch         │
│  riscv64 (Sv39/48, AIA/IMSIC, PLIC, H) │ aarch64 (GICv3/ITS, EL2)       │
│  x86_64 (LAPIC/x2APIC, IOAPIC, MSI, VT-x) │ vifast-boot (UEFI x3)       │
└────────────────────────────────────────────────────────────────────────┘
```

Kernel chỉ làm 5 việc: cấp quyền (capability), chia RAM (Untyped), chạy luồng (TCB + MCS), chuyển tin nhắn (IPC), chuyển ngắt thành tín hiệu. Mọi thứ khác là chương trình user.

---

## 4. Kernel: thiết kế chi tiết

### 4.1 Máy trạng thái và trait platform

```rust
pub enum Event {
    Syscall   { hart: HartId },
    Timer     { hart: HartId },
    Irq       { hart: HartId, src: IrqSource },        // Line(n) hoặc Msi(vector)
    UserFault { hart: HartId, kind: FaultKind, addr: u64 },
    VmExit    { hart: HartId, reason: VmExitReason },  // từ H1
    Ipi       { hart: HartId },
}
pub enum Decision { RunUser(ObjRef<Tcb>), RunVCpu(ObjRef<VCpu>), Idle(IdleHint) }

impl<P: Platform> Kernel<P> {
    pub fn init(cfg: &KernelConfig, boot: &BootDesc, p: &mut P) -> Self;   // một lần
    pub fn handle(&mut self, p: &mut P, ev: Event) -> Decision;
}
```

Trait nhỏ, mock riêng: `Clock`, `Timer`, `Ipi`, `PhysMem`, `Mmu` (với `type Fmt: PageTableFormat`), `IoMmu`, `IrqCtl`, `Fpu`, `Virt`, `Power`, `Arch` (`type Context: ArchContext`), `Debug`. `Platform` là tổng của chúng.

`ArchContext`: `syscall_no()`, `arg(i)`, `set_ret(i, v)`, `ip()/set_ip()`, `sp()`, `tls()/set_tls()`. Core chỉ dùng trait này, không biết tên thanh ghi.

Vì có BKL, mỗi lần `handle` nguyên tử với trạng thái kernel; đan xen ở mức event trong sim là **chính xác** cho logic core.

### 4.2 Bộ nhớ object: object nằm trong Untyped

**Ý tưởng**: Retype cắt vùng nhớ trong Untyped của người gọi và đặt object kernel (TCB, Endpoint, ...) ngay tại đó. Kernel chỉ còn một **arena theo hart** rất nhỏ. Không có `ObjectPoolExhausted`; giới hạn số luồng/endpoint chỉ là RAM của người tạo.

```
RAM:  [firmware][kernel image][init + module][arena theo hart][ UNTYPED (gần 100%) ]
                                                                 └ mọi object kernel nằm ở đây
```

**vifast-objmem** (crate duy nhất có unsafe liên quan object, mục tiêu dưới 800 dòng [ƯL]):

```rust
#[repr(C)] pub struct ObjHeader { kind: ObjKind, state: LifeState, _pad: u16, id: u64 } // không còn `refs`
pub struct ObjRef<T> { pa: PAddr, id: u64, _t: PhantomData<T> }

pub trait ObjStore {
    fn with<T: KObj, R>(&self, r: ObjRef<T>, f: impl FnOnce(&T) -> R) -> Result<R, Dead>;
    fn with_mut<T: KObj, R>(&mut self, r: ObjRef<T>, f: impl FnOnce(&mut T) -> R) -> Result<R, Dead>;
    fn with_many_mut<const N: usize, R>(&mut self, rs: [ObjRef<dyn KObj>; N],
        f: impl FnOnce([&mut dyn KObj; N]) -> R) -> Result<R, Alias>; // N ≤ 8, kiểm từng cặp
    unsafe fn place<T: KObj>(&mut self, pa: PAddr, v: T) -> ObjRef<T>;   // chỉ Retype gọi
    unsafe fn destroy<T: KObj>(&mut self, r: ObjRef<T>);                  // chỉ finalise gọi
}
```

- `id = (cluster_id << 56) | counter` của cụm; duy nhất, không tái dùng. Truy cập bằng `ObjRef` có `id` khác header là `Dead`. Retype không phải đường nóng nên đếm dưới khoá cụm là đủ; sang S3 mỗi cụm đếm riêng.
- `with_many_mut` thay `get2_mut`: IPC cần chạm ≥4 object (2 TCB + Endpoint + Reply + IPC buffer); trùng địa chỉ trả `Alias`. Cài đặt bằng con trỏ thô + `UnsafeCell`, **không** tạo tham chiếu tới cả vùng nhớ; sim cấp vùng bằng bộ cấp phát thô căn lề, không dùng `Vec<u8>`; Miri từ commit đầu.
- Trên máy thật đi qua physmap; trên sim đi qua **bộ cấp phát thô có căn lề** (không `Vec<u8>` — QD §3.5). **Cùng một code unsafe**, kiểm bằng Miri.
- Kích thước object là hằng số theo arch lúc build, riêng TCB = phần cố định + vùng FPU/vector tính lúc boot theo tính năng CPU (XSAVE size, SVE VL, RISC-V VLEN).

**Untyped**:
- Mỗi Untyped có `watermark` (bump). Retype lấy từ watermark, căn lề theo kích thước object.
- Thu hồi bằng **`CNode.Revoke` trên cap Untyped** (không có method `Untyped.Revoke` riêng): finalise mọi con, đặt watermark về 0, xoá trắng (làm tiếp được, mục 4.13).
- Untyped con: Retype Untyped thành Untyped nhỏ hơn để chia quyền sở hữu (memory server dùng).
- Device Untyped: chỉ Retype thành Frame với attrs Device/UC; không chứa object kernel.

**Finalise** (bắt buộc trước khi giải phóng):

| Object | Việc phải làm |
|---|---|
| TCB | gỡ khỏi ready queue, hàng endpoint/notification/address-wait, timer heap; huỷ Reply đang giữ; trả SchedContext; nếu Running ở hart khác: gửi `Stop` qua hộp thư và chờ |
| SchedContext | gỡ khỏi refill queue; unbind TCB/Notification |
| Endpoint | mọi người chờ nhận `Cancelled` |
| Notification | waiter nhận `Cancelled`; unbind TCB và IrqHandler |
| Reply | caller nhận `Cancelled` |
| CNode | Delete mọi slot (đệ quy, làm tiếp được) |
| VSpace | thu hồi ASID, shootdown, gỡ mọi PageTable con |
| PageTable | unmap mọi mục con, gỡ khỏi cha |
| Frame | unmap khỏi VSpace/IOSpace, shootdown; đánh thức waiter WaitAddr với `Cancelled` |
| IOSpace | thu hồi domain IOMMU, invalidate IOTLB |
| VCpu | dừng nếu đang chạy (hộp thư), gỡ khỏi VSpace khách |
| IrqHandler | mask nguồn, unbind |

Invariant sim: sau mỗi event, **không trường nào trong bất kỳ object sống nào trỏ tới object chết**.

**Trần tĩnh [GT]**: lỗi v1 khi tách pool ra khỏi `.data` (3 lần, hỏng AArch64) đến từ pool tĩnh trong `.data`; v2 đặt object trong RAM Untyped nên cơ chế đó không áp dụng, **nhưng vẫn phải đo** ở K0.5 (kích thước image, `.data`/`.bss`, headroom) và giữ **cổng cứng `.data + .bss ≤ 256 KiB` [TK]** (mục 8).

### 4.3 CSpace và capability

- **Hai cấp cố định**: địa chỉ cap 64 bit = `[L1 index | L2 index]`, kích thước mỗi cấp do CNode quyết định (2^n slot, n do người tạo chọn, tối đa **2^20 slot [TK]**). CNode gốc chứa cap tới CNode lá. Đủ cho hàng triệu cap, không có guard.
- **Slot cap 48 B** [TK]: `pa u64 | id u64 | badge u64 | mdb_prev u64 | mdb_next u64 | kind u8, rights u8, depth u16, flags u32`. Thêm test `size_of::<Slot>() == 48`.
- **MDB** kiểu seL4: danh sách liên kết theo thứ tự cây dẫn xuất; Revoke xoá con theo thứ tự, làm tiếp được.
- **Rights**: READ, WRITE, GRANT, GRANT_REPLY. **Cap không có bit EXEC — thay đổi có chủ ý so với v1** (QD §3.12): EXEC là **thuộc tính lúc map** (mục 4.10), kiểm bởi W^X theo Frame; attrs cũng là thuộc tính lúc map.
- **Badge**: Mint endpoint/notification với badge 64 bit.
- **Thao tác**: Copy, Mint, Move, Mutate, Delete, Revoke, Rotate (Rotate để server đổi Reply hiệu quả).
- **Cap allocator ở user** (`vifast-capalloc`): quản lý slot trống trong CSpace của tiến trình, tự xin CNode lá mới từ memory server khi cần.

### 4.4 TCB và bảng chuyển trạng thái

```rust
pub enum SleepReason { Timeout, Budget }

pub enum ThreadState {
    Inactive,
    Ready,                                         // trong ready queue của hart, chỉ khi !suspended
    Running { hart: HartId },
    BlockedOnSend  { ep: ObjRef<Endpoint>, call: bool },
    BlockedOnRecv  { ep: ObjRef<Endpoint>, reply: Option<ObjRef<Reply>> },
    BlockedOnReply { reply: ObjRef<Reply> },
    BlockedOnNotification { ntfn: ObjRef<Notification> },
    BlockedOnAddress { frame: ObjRef<Frame>, offset: u32 },
    Sleeping { reason: SleepReason },   // Timeout | Budget
    Stopping,                                      // chờ hart khác dừng mình (finalise, migrate)
    Dead,
}
```

- `transition(tcb, to)` duy nhất, tra `const ALLOWED`. Bảng xuất ra tài liệu và dùng sinh case âm.
- Liên kết xâm nhập bằng `ObjRef` (crate `vifast-ilist`, không unsafe vì đi qua `ObjStore`).
- **Invariant (sửa)**: TCB ở trong hàng của X **khi và chỉ khi** trạng thái mang X **và `!suspended`**. `Ready + suspended` **không** nằm trong ready queue; `Abort` gỡ khỏi hàng **trước** khi bật cờ, nên bất biến vẫn kiểm được sau mỗi event.
- Không ô `resume`: message ghi thẳng vào context + IPC buffer rồi mới chuyển Ready. Đường mất message kiểu r319 không biểu diễn được.
- **Cờ `suspended: bool` trực giao** với `ThreadState` (không thêm trạng thái mới). `Tcb.Suspend(mode)`:
  - `Abort` (mặc định): nếu đang `BlockedOn*`, gỡ khỏi hàng chờ, syscall đang chờ trả `Cancelled`, ip đã tiến qua syscall, TCB thành `Ready + suspended`. Nếu đang `BlockedOnReply`, Reply object được tách khỏi caller: server `Send(reply)` sau đó **thành công nhưng reply bị bỏ**, để client bị dừng không làm hỏng đường lỗi của server.
  - `Defer`: chỉ hợp lệ khi TCB đang trong thao tác dài có `Restart`; với IPC trả `InvalidArgument`.
- `Tcb.Resume` xoá cờ. **Không bao giờ phát lại Call/Send/Recv/ReplyRecv**; `Restart` chỉ dùng cho thao tác dài (mục 4.13).
- `Tcb.ReadState -> {state, suspended, blocked_on_kind, exit_code: Option<u64>}`; `Tcb.SetFaultEp` tách khỏi `Configure`.
- `Exit(code)` (QD §1 Q-A, sửa E2): lưu `exit_code: Option<u64>` trong TCB; nếu có `fault_ep` thì TCB thành `BlockedOnSend { ep: fault_ep, call: false }` mang `FaultKind::Exit{code}` — **giao qua hàng gửi của `fault_ep`, không có Reply**; khi procsrv `Recv` thì TCB thành `Inactive`. Không có `fault_ep` thì `Inactive` ngay. `Resume`/`Configure` xoá `exit_code`. `Suspend(Abort)` khi đang chờ giao Exit huỷ message nhưng `exit_code` vẫn đọc được qua `ReadState`.
- Trường TCB chính: context, FPU area, cspace_root, vspace, ipc_buffer (Frame), fault_ep, timeout_ep, sched_context, bound_notification, priority, max_controlled_priority, affinity (HartMask), home hart, tls, **suspended**.

### 4.5 MCS: SchedContext

- **SchedContext** object: `budget`, `period`, danh sách refill là chi tiết cài đặt (tối đa **K = 8 refill [TK]**), `badge`, hart.
- TCB chỉ chạy khi có SchedContext còn budget. Hết budget: TCB vào **`Sleeping` với `reason = Budget`** — **không thêm enum mới** (QD §3.4); `timeout_ep` và timeout fault thuộc phần **hoãn sau K6**.
- **SchedControl** cap (mỗi hart một cái, chỉ init có): `Configure(sc, budget, period, refills)`.
- **Passive server**: server không có SchedContext riêng. Khi client Call, SchedContext của client **đi theo** sang server; Reply trả về. Thời gian phục vụ tính cho client. Đảo ưu tiên được xử lý vì server chạy bằng SchedContext của client (MCP kiểm soát mức ưu tiên).
- Notification có thể bind SchedContext để đánh thức passive driver.
- Ưu tiên: **256 mức [TK]**; `max_controlled_priority` ngăn tự nâng ưu tiên.
- Kiểm sim: tổng thời gian chạy của mỗi SchedContext trong mọi cửa sổ `period` không vượt `budget` (cộng sai số tick); không TCB nào chạy khi SchedContext hết budget.

**Giai đoạn đầu (QD §3.4):**

| Có ở K1b | Hoãn sau K6 (không đổi ABI) |
|---|---|
| SchedContext object, `SchedControl.Configure`, budget/period đơn giản (nạp lại đầu mỗi chu kỳ), **donation qua Call/Reply**, passive server, bind Notification | Refill sporadic nhiều đoạn, timeout fault + `timeout_ep`, phân tích độ trễ audio |

Trạng thái chờ ngân sách dùng `Sleeping` với `reason = Budget` (không thêm enum, QD §3.4). Bảng chuyển trạng thái liên quan ngân sách:

| Từ | Sự kiện | Đến |
|---|---|---|
| `Running` | hết budget, còn refill kế | `Sleeping{reason = Budget}` |
| `Sleeping{reason = Budget}` | timer refill kế tới hạn | `Ready` |
| `Sleeping{reason = Budget}` | `Tcb.Suspend(Abort)` | `Sleeping{reason = Budget}` + `suspended` (không ở hàng chờ) |
| `Sleeping{reason = Budget} + suspended` | `Tcb.Resume` | `Sleeping{reason = Budget}` (chờ refill; **không phát lại** gì) |

### 4.6 IPC

- **Syscall**: Call, Send, NBSend, Recv, NBRecv, ReplyRecv, NBSendRecv, NBSendWait (hai cái cuối cho passive server và driver).
- **MessageInfo** (một thanh ghi): `label: u20`, `caps: u3 (≤4)`, `no_reply: u1`, `extra_caps_unwrapped: u4`, `len: u7 (≤64 MR)`.
- MR0 tới MR3 trong thanh ghi (riscv64/aarch64 có thể 6 [ƯL]), còn lại trong IPC buffer. 64 MR + 4 cap là thay đổi có chủ ý so với v1: compositor cần buffer + fence + vùng chia sẻ trong **một** message; MR ngoài thanh ghi nằm trong IPC buffer nên chi phí chỉ phát sinh khi dùng.
- **IPC buffer** là Frame bind qua `Tcb.Configure`; kernel copy bằng `PhysMem`. Header chứa **receive window là CPTR** (địa chỉ cap giải qua CSpace của chính thread, không phải `ObjRef<CNode>`) cho tối đa 4 cap; kernel kiểm slot trống và quyền ghi trước khi cài.
- **Chuyển cap**: cần GRANT; cap nhận bỏ GRANT; cap cùng endpoint với badge được "unwrap" thành badge (kiểu seL4). Chuyển thất bại không mất message, đặt cờ trong MessageInfo.
- **Reply object do bên nhận cấp** (QD §3.2): `Recv(ep, reply)` và `ReplyRecv`; **bên gọi Call không cần và không cấp Reply**. Call làm caller sang `BlockedOnReply{reply}`; server trả lời bằng `Send(reply)` hoặc `ReplyRecv`. Server giữ nhiều Reply, phục vụ xen kẽ nhiều client. Generation trong `id` thay cho reply_epoch của v1. MCS: SchedContext gắn với Reply khi server passive. **Không có `Tcb.SetReplyObject`.**
- **Fault**: Call tới fault endpoint với Reply; trả lời Reply để tiếp tục, có thể kèm thanh ghi mới. Loại fault: VM, unknown syscall, user exception, timeout (MCS), **Exit (Send, không Reply)**, VM exit (H1).
- **Ca biên `no_reply`** (QD §3.2): Call gặp bên nhận đã `Recv` **không** có Reply ⇒ message vẫn giao, cờ `no_reply` bật trong MessageInfo phía nhận, caller **trả ngay `Cancelled`**. Kèm kịch bản sim.
- **Invocation trả ngay**: Call tới object **không phải Endpoint** là invocation (method = label) — **không chặn, không sinh Reply**, kết quả trả trong MR (mục 4.21). Chỉ Endpoint mới đi đường IPC chặn.
- **Một hàm lõi** `ipc::transfer`. Fastpath sau này phải gọi cùng hàm copy cap/MR hoặc được sim so vi sai.

### 4.7 Notification và WaitOnAddress

**Notification** (giữ **nguyên ngữ nghĩa seL4**, QD §3.11): word 64 bit; Signal OR badge, đánh thức **một** waiter, giao word, **xoá word**; Wait/Poll; bound notification với TCB và SchedContext; IrqHandler signal vào notification. **Đây là thay đổi có chủ ý so với v1** (v1 đổi sang "đánh thức tất cả"): lý do v1 phải đổi là root đậu 4 worker trên **một** notification — **lỗi cách dùng**, không phải yêu cầu của giao diện. Broadcast = mỗi bên chờ một notification riêng, bên phát Signal từng cái (việc của user space).

**WaitOnAddress** (futex, QD §3.8):
- Hàng chờ là **bảng băm trong arena kernel**, khoá `(frame_id, offset)`, số bucket theo `kernel.toml`; nút nằm trong TCB. Không có gì nằm trong trang của user.
- `WaitAddr(frame_cap, offset, expected, timeout)`: đọc giá trị và xếp hàng **trong cùng một lần giữ khoá**; `WakeAddr` cũng giữ khoá, nên không mất đánh thức.
- Khoá theo **địa chỉ vật lý** (Frame + offset), hoạt động với bộ nhớ chia sẻ giữa tiến trình.
- Kiểm: không mất đánh thức (Wake sau khi giá trị đổi luôn đánh thức người đã Wait với giá trị cũ).

### 4.8 Scheduler theo hart

- Mỗi hart: 256 mức `ilist` + bitmap; refill queue (MCS); timer heap theo deadline.
- `pick` O(1). Lượng thời gian do SchedContext quyết định.
- **Affinity** là HartMask; **home** là hart hiện tại.
- **Đẩy cân bằng (push migration)**: mỗi hart **công bố** `idle: AtomicBool` và `cur_prio: AtomicU8` trong mảng căn cache line (QD §3.10). Khi hart X có > 1 TCB Ready cùng mức cao nhất và có hart Y trong affinity đang `idle`, X đẩy một TCB sang Y qua hộp thư `Migrate`. Quyết định tất định (hart rảnh có chỉ số nhỏ nhất). Không work stealing (kéo) vì phải **ghi** trạng thái hart khác; **đọc** trường công bố thì được phép (mục 4.16 luật 3).
- Wake chéo hart: hộp thư `Wake` + IPI nếu ưu tiên cao hơn **`cur_prio` công bố** của đích.
- Idle: `Decision::Idle(hint)`, platform chọn mức idle sâu theo deadline kế tiếp.
- Đo: thời gian chờ khoá, độ dài ready queue, số migrate.

### 4.9 Thời gian

- Nguồn: bộ đếm phần cứng (RISC-V `time`, AArch64 CNTVCT_EL0, x86 TSC invariant; không invariant thì HPET và tắt đọc ở user). Kernel không lưu ticks, không có gì để reset.
- **Trang thời gian (kiểu vDSO)**: Frame chỉ đọc map vào mọi tiến trình qua process server: `{ counter_kind, freq, mult, shift, epoch_offset, seq }`. User đọc thẳng bộ đếm (CNTVCT ở EL0, `time` CSR ở U-mode, RDTSC). Syscall `ClockRead` chỉ là dự phòng.
- Kernel tickless: one-shot theo hart, deadline = min(hết budget, refill kế, timeout gần nhất).
- Kiểm: đơn điệu chéo hart (conformance), đơn điệu sau migrate (sim).

### 4.10 Bộ nhớ ảo

- Object: VSpace, PageTable, Frame (`size_bits` 12/21/30), Device Frame.
- `Frame.Map(vspace, va, rights, attrs)`: `rights` chỉ còn READ/WRITE; **EXEC là thuộc tính lúc map** (trong `attrs`), **không phải bit trong cap** (QD §3.12). attrs: `WB`, `WC`, `UC`, `Device`, `NX`, `EXEC`. Kernel chọn cấp lá theo kích thước Frame. Kernel không tự cấp page table.
- W^X: một Frame vật lý không được map vừa W vừa X ở bất kỳ VSpace nào (bộ đếm mapped_w/mapped_x, giữ từ v1). JIT (nếu có sau này) phải unmap W rồi map X.
- **Không gian kernel**: nửa cao + physmap (trang lớn, NX). RISC-V copy entry nửa cao vào root mới; AArch64 TTBR1; x86 copy PML4[256..511], hoặc KPTI. Logic copy nửa cao nằm trong `PageTableFormat` của vifast-pte, dùng chung.
- **ASID** có generation (hết ASID thì tăng generation và flush toàn bộ), module chung; độ rộng theo `Mmu`.
- **Shootdown**:
  1. VSpace giữ `active_on: HartMask`.
  2. Unmap/hạ quyền: sửa PTE, flush cục bộ, gửi `TlbFlush{asid, va, gen}` vào hộp thư các hart trong mask, IPI, chờ ack (bộ đếm thế hệ).
  3. Luật chống deadlock: xử lý hộp thư không cần khoá; hart quay chờ khoá hoặc chờ ack phải xử lý hộp thư của mình. (Lỗ AP->BSP của v1 vi phạm đúng luật này.)
  4. AArch64 có thể dùng TLBI broadcast; RISC-V có thể dùng SBI RFENCE; khác biệt nằm trong `Mmu`, giao thức không đổi.
- Xây ở user trên nền này: copy-on-write, cấp phát lười, file ánh xạ (VFS làm pager qua fault).

### 4.11 Ngắt

- `IrqControl.GetLine(line, trigger, polarity, dest)`, `IrqControl.GetMsi(pci_bdf, vector_hint, dest)`.
- `IrqHandler`: SetNotification, Ack, Clear, SetAffinity.
- Line: kernel mask khi nhận, driver Ack để unmask. MSI: edge, không cần mask.
- Platform: x86 LAPIC/x2APIC + IOAPIC + MSI/MSI-X; AArch64 GICv2/v3 + ITS; RISC-V PLIC và AIA (APLIC + IMSIC).
- Timer và IPI thuộc kernel. Console thật là driver user; kernel chỉ có `DebugPutStr` dưới feature debug.

### 4.12 IOSpace (IOMMU)

- Object `IOSpace` gắn với một thiết bị (PCI BDF hoặc stream ID). `Frame.MapIo(iospace, iova, rights)`.
- Khai báo trong ABI từ K0; cài ở mốc D3: Intel VT-d trên QEMU `intel-iommu`, virtio-iommu, ARM SMMUv3; RISC-V IOMMU khi QEMU hỗ trợ ổn định [GT].
- Máy không có IOMMU: memory server chỉ cấp Frame cho DMA từ một vùng riêng, driver DMA được đánh dấu tin cậy trong manifest. Đây là chính sách user.
- **Threat model (QD §3.14)**: trước mốc **D3** và trên máy **không** có IOMMU, **mọi driver có DMA thuộc vùng tin cậy**; tuyên bố bảo mật ở mục 4.15 chỉ áp cho **CPU, không áp cho DMA**. Không làm lại software IOMMU (v1 đã bỏ).

### 4.13 Thao tác dài làm tiếp được

- Áp dụng cho: Retype lớn (xoá trắng), Revoke, Delete CNode lớn, finalise VSpace, unmap vùng lớn.
- Cơ chế chung `Preempt`: mỗi thao tác chia thành bước có cận; sau mỗi bước kiểm `p.pending_interrupt()`; nếu có, lưu tiến độ **trong object** (ví dụ `untyped.zero_cursor`, `cnode.delete_cursor`), trả `Restart` để platform quay lại chính lệnh syscall (đặt lại ip). **`Restart` là tín hiệu nội bộ core → platform, KHÔNG phải mã lỗi ABI** (QD §3.7): nó không bao giờ tới user và **đã bị bỏ khỏi bảng mã lỗi** (mục 4.20).
- Trạng thái trung gian phải thoả mọi invariant (sim kiểm ở mọi điểm preempt).
- Ở sim: bộ lập lịch có thể ép preempt tại mọi bước.

### 4.14 FPU và vector

- Lưu **eager** khi chuyển context, có theo dõi bẩn: RISC-V bit FS/VS trong sstatus; x86 XSAVEOPT/XSAVES; AArch64 FPEN + SVE.
- Kích thước vùng lưu tính lúc boot, nằm trong TCB.
- Không lazy FPU (lỗ hổng LazyFP).

### 4.15 Bảo mật

- KPTI tuỳ chọn theo CPU (x86 dính Meltdown); cờ trong `KernelConfig` + phát hiện lúc boot.
- Chỉ số lấy từ user dùng `nospec_index` trước khi tra bảng.
- Xả bộ dự đoán nhánh khi đổi VSpace nếu CPU có (IBPB, tuỳ chọn).
- SMEP/SMAP (x86), PAN/PXN (AArch64), SUM tắt (RISC-V).
- Stack kernel có trang bảo vệ.

### 4.16 Đồng thời

| Bậc | Nội dung | Khi nào |
|---|---|---|
| S1 | BKL dạng **queue lock (MCS lock)** | K1a |
| S2 | Đường không khoá: hộp thư, ack TLB, IPI, timer riêng hart, idle | K1a |
| S3 | Cụm: mỗi cụm một khoá + một `Kernel` riêng; chéo cụm qua hộp thư | Khi chờ khoá > ngưỡng (mục 9.10) |
| S4 | Khoá mịn cục bộ | Sau S3, chỉ đúng đường nóng |

Năm luật bắt buộc từ **K1a**:
1. vifast-core không biết khoá; platform lấy khoá rồi gọi `handle`.
2. Đường nóng chỉ ghi `PerHart` của hart đang chạy + object mà thao tác chạm tới; không bộ đếm global trên đường nóng.
3. Không **ghi** trạng thái hart khác; đi qua hộp thư + IPI. **Đọc** trường công bố (`idle`, `cur_prio` — mục 4.8) là **được phép** (QD §3.10).
4. Mỗi object có `home`.
5. Không giữ khoá khi chờ; đang chờ khoá hay chờ ack đều phải xử lý hộp thư của mình.

Build sim có guard ghi lại hart truy cập `PerHart`; vi phạm luật 2, 3 là panic (đọc trường công bố không tính là vi phạm).

`MAX_HARTS` **mặc định 8, trần cấu hình 64 [TK]** (sửa 256); HartMask kích thước theo số hart lúc boot (bitmap trong arena). **Arena chỉ cấp cho hart phát hiện được** lúc boot, không cấp đủ `MAX_HARTS` [TK].

### 4.17 Hộp thư theo hart (`vifast-sync`)

- MPSC kích thước cố định; loại: `Wake`, `TlbFlush`, `Reschedule`, `Migrate`, `Stop`, `VmKick`, `PowerReq`.
- Ack bằng bộ đếm thế hệ.
- Đầy: người gửi xử lý hộp thư của mình rồi thử lại.
- Đây là phần lock-free duy nhất; kiểm bằng `loom` trên host (nếu có trong môi trường, nếu không thì stress đa luồng) + Miri cho phần không atomic.

### 4.18 Ảo hoá (VCpu)

- Object `VCpu` + VSpace loại `Guest` (stage-2/EPT/NPT).
- `VCpu.Run` (chặn tới VM exit), `ReadRegs`, `WriteRegs`, `InjectIrq`, `SetTsc`.
- VM exit không xử lý được trong kernel thành fault message Call tới VMM.
- Khai báo ABI từ K0; cài ở mốc H1 (RISC-V H trước, rồi AArch64 EL2, x86 VT-x).

### 4.19 Điện năng

- Kernel: idle sâu theo `IdleHint`; `PowerControl.HartOffline/HartOnline` (PSCI, SBI HSM, INIT-SIPI); hook suspend/resume lưu trạng thái hart.
- Chính sách: power server.
- Khai báo ABI từ K0, cài ở mốc P1.

### 4.20 Mã lỗi

**19 mã** (QD §4): InvalidCapability, WrongObjectType, InsufficientRights, WxViolation, InvalidArgument, Alignment, SlotOccupied, SlotEmpty, NotEnoughMemory, AlreadyMapped, NotMapped, MissingPageTable, HasChildren, Busy, Cancelled, Timeout, IllegalOperation, NotSupported, RangeError.

Mỗi mã một nghĩa; kèm một word chi tiết (chỉ số tham số sai, cấp bảng trang thiếu, số byte thiếu). Tất cả nằm trong `abi.toml`. `Restart` **không phải mã lỗi ABI** — là tín hiệu nội bộ core → platform (mục 4.13). `BrokenPipe`, `Stopped` thuộc giao thức user (IDL procsrv/pipe), không phải kernel.

### 4.21 Bảng ABI

**Syscall** — 0..13 như cũ, thêm **14 Exit**; dải **15..31 reserved** (QD §4).

| # | Tên | Ghi chú |
|---|---|---|
| 0 | Call | Endpoint: IPC; object khác: invocation (method = label), **trả ngay** |
| 1 | Send | Notification: Signal; Reply: trả lời; fault `Exit`: giao qua hàng gửi của `fault_ep`, không có Reply |
| 2 | NBSend | |
| 3 | Recv | Endpoint hoặc Notification |
| 4 | NBRecv | Poll |
| 5 | ReplyRecv | Đường nóng của server |
| 6 | NBSendRecv | |
| 7 | NBSendWait | |
| 8 | Yield | Trả phần budget còn lại |
| 9 | WaitAddr | futex wait |
| 10 | WakeAddr | futex wake |
| 11 | ClockRead | dự phòng |
| 12 | DebugPutStr | feature debug |
| 13 | DebugHalt | feature test, thoát QEMU với mã |
| 14 | Exit(code) | Lưu `exit_code`; có `fault_ep` → `BlockedOnSend{fault_ep, call:false}` mang `FaultKind::Exit{code}`; procsrv `Recv` xong → `Inactive`; không có `fault_ep` → `Inactive` ngay |

**Object và method** (method id sinh từ `abi.toml`)

| Object | Method | Cài ở |
|---|---|---|
| Untyped | Retype (Revoke qua `CNode.Revoke` trên cap Untyped) | K1a |
| CNode | Copy, Mint, Move, Mutate, Delete, Revoke, Rotate | K1a |
| Tcb | Configure, **SetFaultEp**, **ReadState**, **Suspend(mode)**, SetPriority, SetMcp, SetAffinity, SetSchedParams, SetTimeoutEp, ReadRegs, WriteRegs, CopyRegs, Resume, BindNotification, UnbindNotification, SetTls | K1b (SetTls K6) |
| SchedControl | Configure | K1b |
| SchedContext | Bind, Unbind, Consumed, YieldTo | K1b |
| Endpoint, Notification, Reply | (qua syscall) | K1c |
| VSpace | GetAsid (debug); **tạo/khách: reserved** | K2 |
| PageTable | Map, Unmap | K2 |
| Frame | Map, Unmap, GetAddress, MapIo, UnmapIo | K2 (Io: D3) |
| IrqControl | GetLine, GetMsi | K3 (Msi: D2) |
| IrqHandler | SetNotification, Ack, Clear, SetAffinity | K3 |
| IOSpace | Flush; **Attach: reserved** | D3 |
| VCpu | Run, ReadRegs, WriteRegs, InjectIrq, SetTsc; **Configure: reserved** | H1 |
| PowerControl | HartOffline, HartOnline, Suspend | P1 |

Object chưa cài chỉ giữ **số reserved**, không đóng băng chữ ký; method chưa cài trả `NotSupported`; conformance test kiểm điều đó (QD §1 Q-B).

**Đóng băng ABI (D2-15):** 5 thứ — (1) số syscall + quy ước thanh ghi: **riscv64 đóng băng ngay**; aarch64/x86_64 đóng băng khi port (K4'/K5'), chỉ thêm, không đổi riscv64; (2) layout MessageInfo + MR trong thanh ghi + receive window; (3) bảng mã lỗi + quy ước trả lỗi; (4) kích thước/căn lề object theo arch; (5) luật "ID không tái dùng". Có `abi_version`, `min_compat`, `DebugGetInfo`, golden file.

Bỏ hẳn so với v1: Open, Read, Write, Close, Stat, Seek, Block*, ConsoleRead, TcbSetArgs, ModuleRelease, MemInfo/TcbInfo/TimerInfo, software IOMMU. Bảng ánh xạ 58 syscall v1 → 15 + method, 22 KError → 19 là việc KT-04a.

**Exit, kill, wait** (QD §1 Q-A, §3.3):

| Việc | Ai làm | Ngữ nghĩa |
|---|---|---|
| `exit(code)` | syscall **14** | Lưu `exit_code`; nếu có `fault_ep`, TCB `BlockedOnSend` mang `FaultKind::Exit{code}` (giao qua hàng gửi, không Reply); procsrv nhận xong mới `Inactive`; không có `fault_ep` thì `Inactive` ngay. |
| `wait` | RPC procsrv | Trả `{code, how}` với `how ∈ Exited | Cancelled | Faulted | Killed` (mục 6.4). |
| `kill` | procsrv | `Tcb.Suspend(Abort)` rồi Revoke cap TCB; ghi `how = Killed`. |

`DebugHalt` vẫn chỉ dùng cho test.

### 4.22 KernelConfig

Khai báo trong `spec/kernel.toml`, sinh thành hằng số Rust: `MAX_HARTS` (mặc định **8**, trần **64** [TK]), kích thước trace ring, kích thước hộp thư, số refill tối đa, định nghĩa cụm, feature (debug, test, kpti). **Trace luôn bật, không có cờ tắt** (D2-09). Sim và cả 3 platform đọc cùng tệp; kịch bản sim có thể ghi đè.

---

## 5. Platform và boot

### 5.1 vifast-boot

- Ứng dụng UEFI cho cả 3 arch (QEMU: OVMF cho x86, AAVMF cho aarch64, EDK2/U-Boot cho riscv64 [GT: kiểm EDK2 riscv64 trên QEMU đang dùng]). Đường dự phòng riscv64: OpenSBI + nạp trực tiếp.
- Đọc bản đồ RAM, ACPI (vifast-acpi) hoặc DTB (vifast-dtb), nạp kernel ELF + init + module archive, dựng bảng trang ban đầu (nửa cao + physmap), điền **BootDesc** (vifast-bootdesc) rồi nhảy vào kernel.
- Kết quả: điểm vào kernel giống nhau ở mọi arch.
- **Tới K6 chỉ riscv64, OpenSBI nạp trực tiếp**; UEFI và aarch64/x86_64 chỉ sau K6 (D2-10, QD §1 Q-F(1)).

### 5.2 Chia RAM lúc boot

```
[firmware][boot][kernel][init + module][arena theo hart][UNTYPED ...][device Untyped (MMIO)]
```

Arena theo hart: stack kernel (có trang bảo vệ), PerHart (ready, refill, timer heap), hộp thư, trace ring, HartMask, vùng lưu suspend. Cỡ vài trăm KB mỗi hart [ƯL]. Toàn bộ phần còn lại thành Untyped (chia theo vùng liên tục, căn lề lũy thừa 2) trao cho init.

Phân biệt:
- **Arena**: sổ sách theo hart của kernel, cố định sau boot, user không chạm.
- **Untyped**: RAM thật trao cho user; object kernel cũng nằm ở đây, ai tạo người đó trả.

### 5.3 vifast-plat-common

Vòng lặp chung: trap → dịch thành Event → lấy khoá (xử lý hộp thư khi quay) → `handle` → thực thi Decision → nhả khoá → về user. Đo thời gian chờ khoá. Watchdog. Dump trace khi panic.

### 5.4 Mỗi arch

| | riscv64 | aarch64 | x86_64 |
|---|---|---|---|
| MMU | Sv39 (Sv48 tuỳ chọn) | 4 KiB granule, 48 bit, TTBR0/1 | 4 cấp (5 cấp tuỳ chọn), PCID qua INVPCID |
| Ngắt | PLIC, AIA (APLIC/IMSIC) | GICv3 + ITS (GICv2 dự phòng) | x2APIC, IOAPIC, MSI-X |
| Timer | SBI/Sstc | CNTP/CNTV | LAPIC TSC-deadline |
| IPI | SBI IPI / IMSIC | SGI | LAPIC IPI |
| Hart phụ | SBI HSM | PSCI | INIT-SIPI trampoline (tham khảo v1) |
| Ảo hoá | H extension | EL2 | VT-x |
| Mục tiêu | < 2.500 dòng Rust + asm riêng [ƯL] | như trái | như trái |

---

## 6. User space hệ thống

### 6.1 Khung chung

- **IDL** (`spec/protocols/*.vidl`): `fn` (Call/Reply), `oneway` (Send), `ring<T, N>` (hàng vòng trong bộ nhớ chia sẻ + notification hai chiều), `shm_ref`, `cap<T>` (tối đa 4).
- **vifast-gen** sinh: client stub, trait server, dispatch, mã hoá `vifast-wire`, adapter host (chạy server trên host để test), binding ViScript.
- **vifast-urt::serve**: vòng ReplyRecv, pool Reply, nhiều luồng worker, xử lý notification (IRQ, ring).
- **Manifest** mỗi server (`spec/manifests/*.toml`): cap cần, ưu tiên, SchedContext, memory quota, chính sách khởi động lại.

Ví dụ IDL:

```
protocol Vfs {
    fn open(path: shm_str, flags: u32) -> (fd: u32);
    fn read(fd: u32, off: u64, len: u32, buf: shm_ref) -> (n: u32);
    fn write(fd: u32, off: u64, len: u32, buf: shm_ref) -> (n: u32);
    fn mkdir(path: shm_str) -> ();
    fn list(path: shm_str, buf: shm_ref) -> (n: u32);
}
```

Dữ liệu lớn luôn đi qua vùng nhớ chia sẻ, message chỉ mang offset và độ dài.

### 6.2 init

Nhận BootInfo + toàn bộ Untyped. Đọc manifest, tạo memory server và process server, chia Untyped, chuyển phần dựng còn lại cho process server. Giám sát: server chết thì khởi động lại theo manifest.

### 6.3 memory server

- Giữ Untyped; cấp Frame cho tiến trình theo quota; cấp CNode lá cho cap allocator.
- Áp lực bộ nhớ: theo dõi quota, gửi `MemoryPressure` cho tiến trình, thu hồi bằng Revoke Untyped con như biện pháp cuối.
- Vùng DMA riêng cho máy không IOMMU.

### 6.4 process server

Tạo tiến trình từ ELF (vifast-elf): CSpace, VSpace, IPC buffer, stack với argv/env, trang thời gian, TLS; SchedContext theo chính sách. **Job control không dùng signal** (QD §3.1, §3.3):

| RPC (IDL) | Ngữ nghĩa |
|---|---|
| `Spawn(spec) -> pid` | tạo tiến trình; trả cap/pid |
| `Wait(pid\|gid) -> {code, how}` | `how ∈ Exited \| Cancelled \| Faulted \| Killed` |
| `Stop(gid)` / `Cont(gid)` | `Tcb.Suspend(Abort)` / `Tcb.Resume`; Resume **không** phát lại IPC |
| `Cancel(gid)` | huỷ từ stage cuối về đầu, chờ tối đa **2 s** [ƯL], rồi finalise TCB |
| `List() -> [...]` | danh sách tiến trình/nhóm |

procsrv giữ bảng `gid -> [tcb cap]`. **Terminal: shell sở hữu tty**; tiến trình con nhận pipe, **không** nhận `cap[Term]`; chương trình toàn màn hình (vim, less) để sau với tty server riêng. Exit: syscall 14 → TCB `BlockedOnSend` trên `fault_ep` mang `FaultKind::Exit{code}` → procsrv `Recv`, ghi mã thoát rồi finalise; `kill` = `Suspend(Abort)` + Revoke cap TCB (message Exit đang chờ bị huỷ, `exit_code` vẫn đọc được).

### 6.5 VFS

- Nhiều luồng worker, mỗi worker giữ Reply object riêng.
- **Page cache** dùng Frame chia sẻ; mmap tệp = VFS làm pager (xử lý fault, map Frame từ cache, copy-on-write cho mapping riêng).
- Driver block qua ring.
- vifast-fs: bổ sung **journal metadata** (hoặc copy-on-write) để nhất quán khi mất điện [GT: kiểm hiện trạng vifast-fs].
- Một và chỉ một đường truy cập block device.

### 6.6 Driver

| Mốc | Driver |
|---|---|
| K6 | virtio-blk, virtio-console, UART, (ATA giữ làm dự phòng x86) |
| D1 | virtio-net, virtio-gpu, virtio-input |
| D2 | PCIe enumeration server, MSI-X, NVMe, AHCI |
| D3 | IOMMU (VT-d, SMMUv3, virtio-iommu) |
| D4 | xHCI + USB HID + mass storage |
| D5 | Âm thanh (virtio-sound, HDA) |

Mỗi driver: thư viện thuần (logic thiết bị, test trên host với thiết bị giả) + vỏ server mỏng.

### 6.7 Mạng

Net server dùng stack thuần Rust (smoltcp [GT: kiểm giấy phép và tính năng]) qua ring với driver; socket API qua IDL; DHCP, DNS ở user.

### 6.8 Đồ hoạ và desktop

- **Compositor**: mỗi cửa sổ là buffer Frame chia sẻ (WC), ứng dụng gửi `commit(damage)` + fence qua IDL; compositor ghép bằng phần mềm (SIMD) ở G2, GPU sau.
- **Input server**: gom bàn phím/chuột/chạm, chuyển cho compositor; focus do compositor quyết.
- **ViFastKey làm IME server** ngay trong đường input (giao thức tương tự input-method-v2).
- **Toolkit**: Rust (ứng viên Slint, egui, hoặc toolkit riêng trên thư viện vẽ thuần Rust) [GT: chọn ở G3].
- **Font, vẽ chữ**: thư viện thuần Rust (rasterizer + shaping), hỗ trợ tiếng Việt và tiếng Nhật.
- **ViTerm**: ứng dụng terminal chạy ViShell, có thể hiển thị dữ liệu có cấu trúc (bảng) như widget.

### 6.9 Audio

Audio server với ring chu kỳ ngắn, SchedContext có budget/period chặt (MCS bảo đảm độ trễ).

---

## 7. Runtime ứng dụng, ViScript, Linux

### 7.1 Rust std cho ViFastOs

- Target `{x86_64,aarch64,riscv64gc}-unknown-vifast`.
- Phần `sys`: alloc (vifast-alloc), thread (process server + TCB + TLS), Mutex/Condvar/RwLock/Once (WaitAddr), time (trang thời gian), fs (VFS), net (net server), process, env, args, stdio.
- Đi thẳng qua vifast-urt, **không cần libc**.
- Luật: chỉ dùng crate thuần Rust trong ảnh hệ thống; crate phụ thuộc C bị chặn bởi kiểm tra phụ thuộc.
- Phát triển chéo từ host; kiểm bằng bộ test con của std chạy trên QEMU.

### 7.2 vifast-alloc

Allocator theo luồng (size class + slab), xin mẻ lớn từ memory server, trả lại khi rảnh lâu, phản hồi `MemoryPressure`. Phần lớn `alloc` không vào kernel. Kiểm bằng fuzz so với allocator hệ thống trên host.

### 7.3 ViScript và ViShell (tích hợp)

Ngôn ngữ, VM, ViShell, lộ trình và kiểm thử nằm trong `ViScript-thiet-ke.md`. ViScript phát triển và chạy trên Mac/Linux/Windows trước, không chờ ViFastOs. Phần ViFastOs chịu trách nhiệm:

| Việc | Nơi | Mốc |
|---|---|---|
| `vis-host-vifast`: cài trait Host bằng IPC tới VFS, process server, net server; quyền là cap thật | workspace ngoài `viscript/`, dùng `vifast-urt` | VSH2 |
| Binding ViScript sinh từ IDL (vifast-gen thêm đầu ra binding) | `tools/vifast-gen` | VSH2 |
| ViShell làm shell mặc định, thay shell tạm của K6 | đóng gói trong `user/` | VSH2 |
| Giao thức hiển thị có cấu trúc ViShell và ViTerm | `term.vidl` | mốc riêng **TUI** (trước G3) |
| Job control không signal trong process server | `procsrv` | U1 |

Yêu cầu từ ViScript lên OS: Rust std phần **thread, time, fs, process** trong U1; **net để ở N1** (U1 không hứa net); WaitAddr cho Mutex; trang thời gian; procsrv có `kill/wait/list`. VSH2 phụ thuộc **U1 + N1 + VS2** (sửa lệch C §3); VS3 phụ thuộc **VS2 + G3**.

### 7.4 Linux qua máy ảo

VMM là tiến trình user: tạo VSpace khách, VCpu, giả lập virtio (blk, net, fs, gpu, console) bằng vifast-virtio ở vai phía thiết bị, chuyển cửa sổ khách vào compositor [GT: giao thức kiểu virtio-gpu cross-domain]. Kernel chỉ thêm VCpu + stage-2 + VM exit thành fault.

### 7.5 Để sau

libc (khi cần chạy code C), wasm (khi cần sandbox đa ngôn ngữ).

---

## 8. Cấu trúc crate và tái sử dụng

```
v2/
  spec/          abi.toml, kernel.toml, deps.toml, protocols/*.vidl, manifests/*.toml
  tools/         vifast-gen, vifast-trace, vifast-shrink, vifast-bench, vifast-agentctl
  lib/           (no_std, chạy được trên host)
    vifast-abi2  vifast-ilist  vifast-pte  vifast-elf  vifast-dtb  vifast-acpi
    vifast-bootdesc  vifast-sync(*)  vifast-objmem(*)  vifast-traceformat
    vifast-wire  vifast-fs  vifast-fsutil  vifast-virtio  vifast-nvme  vifast-pci  vifast-capalloc
  kernel/        vifast-core  vifast-plat-common  vifast-plat-{riscv64,aarch64,x86_64}  vifast-kbin-*
  boot/          vifast-boot
  sim/           vifast-sim  vifast-model
  user/          vifast-urt  vifast-alloc  init  memsrv  procsrv  vfs  drv-*  net  power
                 compositor  input  vifastkey-ime  audio  vmm  shell-tam
  std/           bản vá target + phần sys cho Rust std
  test/          vifast-scenario  vifast-ktest  vifast-conformance  regress/*.scn  perf/  mutants.toml
  docs/          (sinh một phần từ spec)
  agent/         QUEUE.md  STATE.md  JOURNAL/  BLOCKED.md  DECISIONS.md  reports/  (mục 11)
(*) crate có unsafe, danh sách đóng, thêm mới phải qua Agent 0
ViScript nằm ở thư mục riêng viscript/ (xem tài liệu ViScript); v2 chỉ phụ thuộc vis-host-vifast từ VSH2.
```

**Luật cấu trúc (cổng cứng)**:
- Tệp trong `kernel/`, `lib/` không quá **1.500 dòng [TK]**; `vifast-core` dưới **25.000 dòng** không tính test [ƯL].
- `cfg(target_arch)` cấm trong core và lib.
- `unsafe` chỉ trong danh sách crate đóng + `kernel/vifast-plat-*` + `boot/`.
- Đồ thị phụ thuộc theo `deps.toml`.
- Code sinh trùng spec.
- Mỗi platform dưới **2.500 dòng [ƯL]** không tính asm.
- **Bảng ánh xạ 25 crate v1 → v2 là việc của KT-04a** (cùng bảng 58 syscall → 15 + method, 22 KError → 19); tài liệu không chép tay bảng đó.
- Ngân sách tĩnh: `.data + .bss ≤ 256 KiB` [TK], đo ở K0.5; arena chỉ cấp cho hart phát hiện được (QD §3.5, §3.15).

**Ma trận tái sử dụng**:

| Thành phần | Kernel | Sim | User OS | Host tool/test |
|---|---|---|---|---|
| vifast-pte | ✓ | ✓ | (VMM) | ✓ |
| vifast-objmem | ✓ | ✓ | | Miri |
| vifast-sync | ✓ | ✓ | | loom/stress |
| vifast-abi2 | ✓ | ✓ | ✓ | ✓ |
| vifast-traceformat | ✓ | ✓ | | vifast-trace |
| vifast-fs | | | VFS | fsutil, fuzz |
| vifast-virtio | | | driver + VMM | test thiết bị giả |
| IDL sinh | | | client/server | adapter host, ViScript |
| vifast-scenario | | ✓ | ktest | ✓ |

---

## 9. Quy trình kiểm thử

### 9.1 Kim tự tháp và thời gian

| Tầng | Nội dung | Mục tiêu | Khi chạy |
|---|---|---|---|
| L0 | Unit + property test từng crate | < 15 s | mỗi lần lưu, mỗi bước vòng lặp |
| L1 | Sim tất định, invariant sau mỗi event | `--fast`: 1.000 seed; nightly: 100.000; đo lại khi có sim | mỗi commit (sau KT-08) |
| L2 | So mô hình + kịch bản có tên + hồi quy `.scn` | < 5 phút | mỗi commit chạm core |
| L3 | QEMU: ktest + conformance, 3 arch, smp 1/2/4 | < 2 phút/arch | commit chạm platform/user; cổng ngày |
| L4 | Sim triệu seed, mutation đầy đủ, Miri, L3 lặp, soak, đo hiệu năng | hàng giờ | đêm |

Vòng lặp hằng ngày khi sửa logic kernel: L0 + L1 trên host, không cần QEMU. **L1 chưa bật tới KT-08**; trước đó `STATE.md` ghi rõ. Đo thời gian L1 ngay khi sim chạy; vượt 60 s thì giảm seed của `--fast`, **không** giảm invariant (QD §5.2).

### 9.2 Kiểm thử theo từng phần

| Phần | L0 | L1/L2 sim | L3 QEMU | Đặc biệt |
|---|---|---|---|---|
| vifast-objmem | place/get/destroy, căn lề, id | dùng trong mọi kịch bản | | **Miri** toàn bộ test; fuzz chuỗi thao tác |
| Untyped/Retype | watermark, căn lề, Untyped con | hết bộ nhớ ngẫu nhiên; Retype lớn bị preempt ở mọi bước | retype toàn bộ RAM rồi revoke | invariant: tổng byte con ≤ Untyped |
| CSpace/MDB | địa chỉ 2 cấp, MDB thứ tự | Revoke giữa IPC; Delete CNode đang dùng; Rotate | | so mô hình cây dẫn xuất |
| TCB/transition | mọi cặp ALLOWED và cấm | bao phủ cặp chuyển trạng thái 100% | | bảng xuất tài liệu |
| MCS | refill, budget | kiểm budget theo cửa sổ; passive server; timeout fault | đo độ lệch budget thật | đột biến: bỏ trừ budget |
| IPC/Reply | transfer MR/cap | bảo toàn message; Reply bị xoá; 4 cap; unwrap badge; tái tạo đường mất message của v1 | 1 server + N client, chéo hart | so mô hình |
| Notification | OR badge, một waiter | bound + Recv; bão IRQ | IRQ thật từ virtio | |
| WaitAddr | | không mất đánh thức; Frame chia sẻ hai tiến trình | Mutex std stress | đột biến: bỏ kiểm giá trị dưới khoá |
| Scheduler | bitmap, ilist | push migration tất định; affinity; không đói | đo ready queue | |
| Thời gian | | đơn điệu sau migrate | đơn điệu chéo hart; đọc user khớp syscall | |
| VM/ASID/shootdown | pte map/unmap mọi cỡ | **mô hình TLB theo hart**, TLB-safety; ASID rollover | map, chạm từ hart khác, unmap, chạm lại phải fault | đột biến: bỏ xử lý hộp thư khi chờ ack (tái hiện AP->BSP) |
| Finalise | | không tham chiếu chết; xoá object ở mọi trạng thái | | đột biến: bỏ một bước finalise |
| Preemptible ops | | ép preempt mọi bước, invariant giữ | Retype 1 GiB dưới tải IRQ | |
| Hộp thư | | đầy hộp thư; đan xen theo seed | | loom/stress host |
| FPU | | mô hình đánh dấu bẩn | giá trị FP/vector giữ qua switch & migrate | |
| IRQ/MSI | | | mỗi bộ điều khiển ngắt | |
| Boot | BootDesc parse | SimBoot nhiều bản đồ RAM | 3 arch, RAM 128 MiB tới 8 GiB, smp 1..8 | case âm: RAM vụn |
| Platform | | | conformance trap/syscall/fault/W^X/SMEP/PAN | |
| vifast-fs | | | test-persist | fuzz host + crash-consistency (cắt ghi tại điểm ngẫu nhiên, kiểm fsck) |
| Driver | logic thiết bị với thiết bị giả | | QEMU thiết bị thật | ghi/so trình tự thanh ghi |
| IDL/wire | round-trip property | | | gen-check |
| vifast-alloc | | | stress đa luồng | fuzz so allocator host |
| Rust std | | | bộ test con của std | |
| ViScript trên ViFastOs | | | bộ hợp chuẩn Host + bộ test ngữ nghĩa | chi tiết: tài liệu ViScript mục 18 |
| Compositor | logic ghép trên host | | ảnh chụp màn hình so pixel | |
| VMM | giả lập virtio trên host | | boot Linux khách tới shell | |

### 9.3 Kịch bản viết một lần, chạy hai nơi

`vifast-scenario`: kịch bản là **dữ liệu**:

```rust
pub struct Scenario {
    pub name: &'static str,
    pub config: ConfigOverride,
    pub setup: &'static [SetupStep],
    pub threads: &'static [ThreadProgram],
    pub expect: &'static [Expectation],
}
pub enum Step { Call{..}, Send{..}, Recv{..}, ReplyRecv{..}, Signal{..}, Wait{..},
                WaitAddr{..}, WakeAddr{..}, Retype{..}, Map{..}, Unmap{..}, Revoke{..},
                Touch{va, write}, ReadClock, Spin{n}, Yield, SetAffinity{..}, Suspend{..}, Exit }
```

- Backend sim: mỗi bước thành `Event`; seed quyết định thứ tự giữa các hart và thời điểm timer/IPI/IRQ.
- Backend QEMU: `vifast-ktest` là tiến trình user chạy cùng kịch bản bằng syscall thật, xuất TAP.
- Kịch bản ngẫu nhiên từ sim xuất được thành `.scn` để chạy trên QEMU. Sim xanh + QEMU đỏ = lỗi platform.

### 9.4 Mô hình tham chiếu

`vifast-model`: kernel ngây thơ bằng std (HashMap, Vec), một hart logic, không objmem. Cùng dãy event tuần tự hoá thì kết quả quan sát (giá trị trả về, message nhận được, trạng thái chặn) phải trùng. Mô hình là đặc tả ngữ nghĩa sống: tranh luận ngữ nghĩa giải quyết bằng sửa mô hình trước. Không dùng chung code với core ngoài vifast-abi2.

### 9.5 SimPlatform

- N hart (1 tới 8 trong test thường, tới 64 trong nightly), đồng hồ ảo ns, deadline theo hart, IPI.
- RAM là **vùng thô có căn lề** (bộ cấp phát thô, **không** `Vec<u8>` — QD §3.5); bảng trang thật ghi vào đó bằng vifast-pte; object thật đặt bằng vifast-objmem.
- **Mô hình TLB theo hart**: cache (asid, va) -> (pa, rights) khi user Touch; chỉ xoá khi kernel flush. Truy cập qua TLB tới PA đã trả về Untyped hoặc quyền đã hạ = vi phạm TLB-safety.
- Hộp thư dùng đúng vifast-sync.
- Đi qua đúng bộ giải mã syscall sinh từ ABI.
- Tiêm lỗi: hết Untyped, Reply bị xoá giữa chừng, Suspend khi đang chờ, migrate giữa IPC, preempt ở mọi bước.

### 9.6 Bộ kiểm sau mỗi event

1. `check_invariants()` (port v1 + mới).
2. Không tham chiếu tới object chết.
3. Luật truy cập `PerHart`.
4. Bảo toàn message: mỗi message giao đúng một lần hoặc người gửi nhận lỗi.
5. Đồng hồ đơn điệu với mọi người quan sát.
6. Không mất đánh thức (Notification, WaitAddr, Reply).
7. TLB-safety.
8. Budget MCS.
9. Tổng bộ nhớ: byte mọi object + byte rảnh = tổng Untyped.
10. Khớp mô hình (L2).

### 9.7 Bao phủ

Sim đếm: cặp chuyển trạng thái, nhánh lỗi của mỗi method, mỗi điểm preempt, mỗi loại hộp thư. Báo phần chưa chạm; bộ sinh ưu tiên kịch bản chạm vào. Cổng K1: 100% cặp ALLOWED, ≥ 95% nhánh lỗi [ƯL].

### 9.8 Tái hiện, thu nhỏ, hồi quy

Hỏng → in (seed, kịch bản, config, BUILD_ID) + trace → `vifast-shrink` thu nhỏ → `test/regress/<mã>.scn` → chạy mãi ở L2 + L3. Commit sửa lỗi phải kèm `.scn` đỏ trước.

### 9.9 Mutation

Danh sách đột biến chuẩn trong `test/mutants.toml`: bỏ một lần transition, bỏ một bước finalise, bỏ trừ budget, bỏ ack TLB, bỏ xử lý hộp thư khi chờ, đảo thứ tự wake, bỏ IPI khi wake chéo hart, bỏ kiểm giá trị WaitAddr, bỏ kiểm W^X. Mỗi đột biến phải bị bắt trong L1 10.000 seed. Đột biến sống = lỗ hổng bộ kiểm, mở task bổ sung kịch bản.

### 9.10 Hiệu năng

`vifast-bench`: IPC round trip cùng/chéo hart, thời gian trong kernel mỗi event (p50/p99), tỉ lệ chờ khoá theo hart, số lần vào kernel mỗi IO, độ trễ đánh thức, độ lệch budget MCS, thời gian Retype/Revoke theo cỡ. Lưu theo BUILD_ID trong `test/perf/history.csv`. Hồi quy > 10% cảnh báo (QEMU nhiễu nên dùng `-icount` cho số đo chu kỳ). Ngưỡng mở S3: chờ khoá > 10% thời gian kernel ở smp 4 dưới tải chuẩn [ƯL].

### 9.11 Trace

Ring theo hart, bản ghi 32 byte `{t_ns, hart, kind, tcb, a, b}`, luôn bật, chi phí đều; dump qua QEMU `pmemsave` khi treo; watchdog hart 0; cùng định dạng sim và máy thật; `vifast-trace` giải mã thành dòng thời gian nhân quả.

### 9.12 Thống kê soak

Không lỗi trong n lần chạy thì cận trên 95% của tỉ lệ lỗi:

$$p_{95} \approx \frac{3}{n}$$

Muốn tuyên bố tỉ lệ dưới p cần $$n \ge \frac{3}{p}$$. Chốt n trước, cùng **máy `ps`**, BUILD_ID, cấu hình (smp, `MAX_IO`); 0/N chỉ là trần; không đếm artifact; T-BUILD không tính. Với p mục tiêu **0,3 %**, N = **1.000** chạy **3 luồng** (QD §5.3). Bỏ con số 750. Lỗi logic phải tái hiện ở L1 trước; soak chỉ xác nhận.

**Hợp đồng marker (harness-v2):** TAP là định dạng kết quả; `harness-v2` có `FORBIDDEN` (panic, invariant fail, `TIME BACK`), `EXACT_ONE` (dòng plan TAP, BUILD_ID), và bộ case âm cho chính harness (T-IPC: harness mới phải chứng minh bắt được lỗi trước khi dùng để tuyên bố).

### 9.13 Lệnh

| Lệnh | Nội dung |
|---|---|
| `./v2/bin/v2 check` | L0 các crate bị chạm |
| `./v2/bin/v2 gate --fast` | fmt, clippy -D warnings, luật cấu trúc, L0 toàn bộ; + gen-check (từ KT-05); + L1 **1.000 seed** (từ KT-08). < 2 phút |
| `./v2/bin/v2 gate` | + L2 + L3 arch đã port |
| `./v2/bin/v2 nightly` | L1 100.000, mutation, Miri, soak + cập nhật `STATUS.md` + perf history |
| `./v2/bin/v2 repro <seed\|scn> [--arch X]` | chạy lại một ca, mở trace |
| `./v2/bin/v2 mutants [tên]` | chạy đột biến |

Không retry ở mọi cổng. Flake hạ tầng phân loại T-INFRA, không bao giờ tính PASS.

---

## 10. Lộ trình đầy đủ

Quy tắc: một mốc bắt đầu khi mọi phụ thuộc đã đạt tiêu chí. Các dòng khác nhau chạy song song nếu không phụ thuộc. **Không có mốc theo ngày** (QD §1 Q-F, §5.1); mọi tiêu chí đo được.

### 10.1 Dòng lõi

| Mốc | Nội dung | Tiêu chí xong |
|---|---|---|
| K0 | Spec đầy đủ (`abi.toml` 15 syscall + method + 19 lỗi + reserved; `kernel.toml`; `deps.toml`), bảng trạng thái, vifast-gen, khung workspace, luật cấu trúc, vifast-agentctl | `./v2/bin/v2 gate --fast` chạy; Agent 0 duyệt spec (KT-04d APPROVED) |
| **K0.5** | boot riscv64 QEMU qua OpenSBI nạp trực tiếp; `DebugPutStr`/`DebugHalt`; TAP 1 test; đo image/`.data`/`.bss` | **50 boot 0 lỗi**; `.data + .bss ≤ 256 KiB` [TK] |
| K1a | sim harness store giả + scheduler tối thiểu 1 hart; Untyped; CNode/MDB test-first (mô hình tối giản trong test) | 1 kịch bản end-to-end; invariant tổng byte; property test cây |
| K1a' | thay store giả bằng objmem (API closure) + Miri | Miri xanh từ commit đầu; các ca Untyped/CNode của KT-04c chạy trên mô hình |
| K1b | TCB + transition + `suspended`; scheduler theo hart; hộp thư; **MCS giai đoạn đầu** (donation) | Bao phủ cặp 100%; kiểm budget; push migration |
| K1c | IPC + Reply + Notification + WaitAddr + fault + **Exit** | Bảo toàn message; không mất đánh thức; mọi đột biến chuẩn bị bắt; 100.000 seed × smp 1/2/4/8 |
| K2 | VSpace/PageTable/Frame 3 cỡ + attrs, ASID, shootdown, W^X, mô hình TLB | 0 vi phạm TLB-safety 100.000 seed; đột biến AP→BSP bị bắt |
| K3 | plat-common + plat-riscv64; init tối thiểu; ktest + conformance | TAP 100% smp 1/2/4 [TK]; soak boot **0 lỗi / N = 1.000** trên máy `ps` [TK] (D2-24); perf đầu tiên |
| **K3.5** | procsrv + shell Rust tối thiểu (ABI được ứng dụng thật dùng) | spawn/wait/exit/kill chạy; shell chạy một lệnh |
| K6 | memsrv, vifast-alloc, capalloc, IDL + ring, VFS (pager, page cache, journal), virtio-blk/console, SetTls | "Tương đương v1" theo danh sách đo được (VFS journal/fsck/rename/truncate/double-indirect; pager nhiều client; exec chain; test-persist 7 pha) |
| K4' | plat-aarch64 | như K3 |
| K5' | plat-x86_64 (+ KPTI tuỳ chọn) | như K3 + `-cpu max` |
| UEFI | `vifast-boot` UEFI cho 3 arch (sau K5') | boot 3 arch qua UEFI, hoặc ghi quyết định giữ OpenSBI cho riscv64 |

### 10.2 Dòng runtime và ViScript

| Mốc | Nội dung | Phụ thuộc | Tiêu chí |
|---|---|---|---|
| VS-SPEC..VS2 | Ngôn ngữ, VM, ViShell trên Mac/Linux/Windows/wasm (tài liệu ViScript mục 19) | không phụ thuộc ViFastOs | theo ViScript §19 |
| U1 | Rust std phần thread/time/fs/process + job control trong procsrv | K6 | test con std; spawn/wait/kill |
| **N1** | Net server, socket qua IDL, DHCP/DNS | D1, U1 | ping/DNS qua socket |
| VSH2 | vis-host-vifast, binding IDL, ViShell thành shell mặc định | **U1, N1**, VS2 | chạy được ViShell trên ViFastOs |
| VS3 | Plugin, binding toolkit, ViTerm có cấu trúc | VSH2, G3 | ứng dụng desktop nhỏ |

### 10.3 Dòng thiết bị và mạng

| Mốc | Nội dung | Phụ thuộc | Tiêu chí |
|---|---|---|---|
| D1 | virtio-net, virtio-gpu, virtio-input | K6 | thiết bị giả + QEMU |
| N1 | Net server, socket qua IDL, DHCP/DNS | D1, U1 | xem §10.2 |
| D2 | PCIe enumeration, MSI-X, NVMe, AHCI | K5' | enumerate + IRQ |
| D3 | IOSpace: VT-d, SMMUv3, virtio-iommu | D2 | DMA được cô lập |
| D4 | xHCI + USB HID + mass storage | D2 | HID hoạt động |
| D5 | Âm thanh | D2, số đo MCS | phát 1 tệp |

### 10.4 Dòng desktop

| Mốc | Nội dung | Phụ thuộc | Tiêu chí |
|---|---|---|---|
| **TUI** | `term.vidl` — giao thức hiển thị có cấu trúc ViShell/ViTerm | VS2, K6 | ViShell chạy trong ViTerm |
| G1 | Framebuffer + console đồ hoạ, font Việt/Nhật | D1 | hiện chữ có dấu |
| G2 | Compositor phần mềm, input server, cửa sổ | G1, U1 | 2 cửa sổ |
| G3 | Toolkit Rust, ứng dụng mẫu (ViTerm, quản lý tệp) | G2 | chạy ViShell |
| G4 | ViFastKey IME server | G2 | gõ tiếng Việt |
| G5 | Audio + trình phát | D5, G3 | phát 1 tệp |

**D-08 còn hiệu lực tới K6** (D2-23): dòng G và ViFastKey (G4) chỉ mở **sau K6**, không chạy song song dòng lõi.

### 10.5 Dòng nâng cao

| Mốc | Nội dung | Phụ thuộc | Tiêu chí |
|---|---|---|---|
| P1 | Power: idle sâu, hart offline/online, suspend/resume | K5' | đo tiêu thụ |
| H1 | VCpu + VMM (riscv64 H trước), Linux khách tới shell | K6, D1 | shell khách |
| H2 | Linux khách trên aarch64/x86_64; chia sẻ tệp, cửa sổ khách vào compositor | H1, G2 | chia sẻ tệp |
| S3 | Cụm, khi số đo vượt ngưỡng | perf §9.10 | chờ khoá > 10 % |
| F1 | IPC fastpath | perf §9.10 | p99 giảm |
| HW1 | Máy thật (board riscv64/aarch64 + PC x86) | K5', D2 | boot tới shell |

### 10.6 Thứ tự ưu tiên (không ngày)

K0 → K0.5 → K1a → K1a' → K1b → K1c → K2 → K3 → K3.5 → K6. Dòng ViScript VS-SPEC..VS2 chạy song song nhưng **OS trước**: Agent 2 chỉ lấy task ViScript khi QUEUE v2 không còn task READY (D2-20), ngoại lệ ST-02a (tài liệu). Không cam kết ngày; tiến độ đo bằng mốc đạt.

---

## 11. Quy trình phát triển lặp liên tục cho Agent 2

### 11.1 Vai trò

- **Người điều phối**: chuyển tệp giữa Agent 0 và Agent 2; duyệt quyết định.
- **Agent 0**: phân tích, thiết kế, ra tệp quyết định + hàng đợi task; duyệt báo cáo.
- **Agent 2**: đọc code, đo, sửa, test, commit, báo cáo. Không đổi spec/ABI/quyết định tự ý.

### 11.2 Tệp điều khiển trong `v2/agent/`

| Tệp | Ai ghi | Nội dung |
|---|---|---|
| `QUEUE.md` | Agent 0 (qua người điều phối) | Task có ID, phụ thuộc, ngân sách, **trạng thái `READY\|BLOCKED\|REVIEW\|DONE`**, tiêu chí xong, dòng `APPROVED` khi duyệt spec |
| `STATE.md` | Agent 2 | Task đang làm, bước, HEAD, cổng gần nhất, việc dở. **Cập nhật sau mỗi bước** để nén ngữ cảnh bất cứ lúc nào |
| `JOURNAL/YYYY-MM-DD.md` | Agent 2 | Nhật ký ngắn: làm gì, kết quả, quyết định nhỏ |
| `BLOCKED.md` | Agent 2 | Task bị chặn + lý do + câu hỏi cho Agent 0 |
| `reports/R<n>-v2.md` + `R<n>-v2.json` | Agent 2 | Báo cáo cuối vòng; JSON có `project` |
| `DECISIONS.md` | Agent 0 | Danh sách quyết định có hiệu lực (chép từ mục 12) |

`vifast-agentctl`: `next` (task kế có phụ thuộc đã DONE, **bỏ qua REVIEW**), `start <id>`, `done <id>`, `block <id> <lý do>`, `report` (sinh `R<n>-v2.json` từ git + kết quả cổng). Nhận `--project v2|viscript` để dùng chung cho hai dòng.

### 11.3 Vòng lặp chuẩn cho mỗi task

1. **Đọc** STATE.md, QUEUE.md, DECISIONS.md, phần spec liên quan. Không đọc lại toàn repo.
2. **Lập kế hoạch nhỏ** trong STATE.md: các bước, tệp sẽ chạm, test sẽ thêm.
3. **Test đỏ trước**: viết test/kịch bản thể hiện yêu cầu; chạy, xác nhận đỏ đúng lý do.
4. **Cài đặt** theo bước nhỏ; sau mỗi bước `./v2/bin/v2 check`.
5. **Cổng**: `./v2/bin/v2 gate --fast` (luôn); `./v2/bin/v2 gate` nếu chạm platform/user.
6. **Commit**: `[KT-##] <tóm tắt>` + kết quả cổng + BUILD_ID.
7. **Cập nhật** STATE.md, JOURNAL; `agentctl done`.
8. **Nén ngữ cảnh** nếu lớn, rồi quay lại bước 1 với task kế.

### 11.4 Luật bắt buộc

- Không sửa `spec/` trừ task được ghi rõ (`KT-04a`), không sửa DECISIONS.md, ngưỡng cổng, `accept()` của harness; danh sách đột biến chỉ được thêm. **Duyệt chỉ bằng dòng `APPROVED <ngày> Agent 0` trong `QUEUE.md` + một mục D2**, không duyệt bằng chat (D2-22); Agent 2 soạn nháp spec ở `REVIEW`, sau duyệt chỉ Agent 0 sửa.
- Không tắt test, không `#[ignore]`, không nới invariant để qua cổng.
- Không retry để qua cổng. Flake: ghi T-INFRA kèm log, chạy lại tối đa một lần **chỉ để phân loại**.
- Hồi quy (cổng từng xanh nay đỏ): revert trước, điều tra sau.
- Lỗi logic: tái hiện trên sim trước khi sửa; kèm `.scn`.
- Thêm `unsafe` ngoài danh sách đóng: cấm; cần thì ghi BLOCKED.
- Thêm phụ thuộc crate ngoài: thuần Rust, giấy phép tương thích MIT/Apache-2.0, ghi lý do trong JOURNAL, kiểm `deps.toml`.
- Không đưa số liệu viết tay vào tài liệu; số liệu do `./v2/bin/v2 nightly` sinh.
- `cargo fmt` trước commit; clippy sạch.
- SPDX theo loại tệp (D2-21, QD §9.3): `//` cho Rust/asm, `#` cho `.vis .vish .vidl .toml`; tệp không có chú thích dùng sidecar `.license`; tệp sinh do generator phát header. Thêm `license.workspace = true` cho mọi crate; LICENSE-MIT/APACHE một bản ở gốc; `THIRD-PARTY.md` sinh tự động; **không** tạo NOTICE.

### 11.5 Ngân sách và điều kiện dừng

| Tình huống | Hành động |
|---|---|
| Task vượt ngân sách (mặc định 3 giờ) | Dừng, ghi tiến độ + lý do, chuyển task kế không phụ thuộc |
| Kẹt hai lần cùng một ý | BLOCKED, chuyển task kế |
| Spec mâu thuẫn hoặc thiếu | BLOCKED với đề xuất cụ thể, không tự đổi spec |
| Cổng đỏ do lỗi không thuộc task | Mở mục lỗi trong JOURNAL + `.scn` nếu có; không sửa lan |
| Hết task khả dụng | Chạy `./v2/bin/v2 nightly`, viết báo cáo, dừng |
| Máy bận/QEMU treo | giết **đúng PID do task tạo** (cấm `pkill -f qemu-system` toàn cục), ghi T-INFRA, tiếp |

### 11.6 Chạy qua đêm

- Trước khi rời máy: QUEUE.md có ít nhất 8 task READY chia nhỏ (1 tới 3 giờ mỗi task), phụ thuộc rõ, ít nhất 3 nhánh độc lập để khi một nhánh BLOCKED vẫn còn việc. Dòng ViScript chỉ mở khi OS hết task READY (D2-20).
- Agent 2 chạy vòng 11.3 liên tục; sau mỗi task chạy `./v2/bin/v2 gate --fast` toàn bộ để phát hiện hồi quy chéo.
- Cuối phiên hoặc khi hết task: `./v2/bin/v2 nightly`, `agentctl report`.
- Tài nguyên: tối đa **3 QEMU** song song [TK], sim dùng tối đa `số core − 1`.

### 11.7 Định dạng báo cáo

`R<n>-v2.md`: tóm tắt 5 dòng, bảng task (ID, trạng thái, commit, cổng), lỗi tìm thấy (`.scn`), số đo thay đổi, đột biến sống, BLOCKED + câu hỏi, đề xuất task kế.

`R<n>-v2.json`: `{project:"v2", round, head, build_id, tasks:[{id,status,commits,gates}], gates:{fast,full,nightly}, sim:{seeds, failures}, coverage:{transitions, error_paths}, mutants:{killed, alive}, perf:{...}, blocked:[...]}`.

Agent 0 đọc JSON trước, MD sau.

### 11.8 Định nghĩa "xong"

- Tiêu chí trong QUEUE đạt, có bằng chứng (đầu ra lệnh, số seed, TAP).
- Test mới đã từng đỏ.
- Cổng tương ứng xanh, không retry.
- STATE, JOURNAL cập nhật; không còn TODO không có task.

---

## 12. Quyết định

Quyết định v1 (D-01..D-08, D11, D18) giữ nguyên mã, **chỉ áp cho v1** (xem §2.3). Quyết định v2 dùng **D2-xx** (QD §2.1, §8), không trùng mã v1.

| Mã | Nội dung |
|---|---|
| D2-01 | v1 lưu trữ ở `v1-archive`, sổ lỗi mang sang v2 (Phụ lục A) |
| D2-02 | Không sửa mò (D11 cũ): cần đường tái hiện hoặc chứng minh cơ chế |
| D2-03 | Hồi quy thì revert trước (D18 cũ) |
| D2-04 | Core không chứa FS/fd/block/console/args/demo; thay đổi ABI/ngữ nghĩa qua Agent 0 |
| D2-05 | Commit chạm `kernel/`, `lib/` phải xanh `--fast`; lỗi logic tái hiện trên sim, kèm `.scn` đỏ trước |
| D2-06 | Luật cấu trúc mục 8 là cổng cứng |
| D2-07 | Không retry ở mọi cổng; flake là T-INFRA |
| D2-08 | Test kernel không nằm trong boot sản phẩm |
| D2-09 | Trace luôn bật, không có cờ tắt |
| D2-10 | Chỉ riscv64 tới K6; aarch64/x86 sau K6 |
| D2-11 | Một nguồn sự thật trong `spec/`; code sinh không sửa tay |
| D2-12 | Đồng thời theo S1..S4 theo số đo; đọc trường công bố của hart khác được phép |
| D2-13 | Kịch bản `.scn` chạy cả sim và QEMU |
| D2-14 | Object trong Untyped qua objmem (API closure, Miri từ đầu) |
| D2-15 | ABI đóng băng 5 thứ + `abi_version` + golden (mục 4.21) |
| D2-16 | MCS: donation từ K1b, thuật toán refill sau K6 |
| D2-17 | `unsafe` chỉ trong danh sách crate đóng |
| D2-18 | Ứng dụng hệ thống viết bằng Rust std, crate thuần Rust; libc, wasm để sau |
| D2-19 | ViScript là ngôn ngữ script và shell chính; Linux qua máy ảo |
| D2-20 | OS trước; ViScript khi không còn task OS READY (ngoại lệ ST-02a) |
| D2-21 | Giấy phép MIT OR Apache-2.0; `cargo-deny` allowlist; `THIRD-PARTY.md` tự sinh; license nhúng ảnh; không NOTICE |
| D2-22 | Duyệt spec bằng dòng APPROVED; Agent 2 soạn nháp ở REVIEW, sau duyệt chỉ Agent 0 sửa |
| D2-23 | D-08 (không đụng đồ hoạ/ViFastKey) còn hiệu lực tới K6 |
| D2-24 | Mọi soak trên máy `ps`, N theo `n ≥ 3/p` |
| D2-26 | Thư viện v1 dùng lại được **copy/port** vào `v2/lib/`, không path-dep; `PORTED-FROM: crates/<tên>@ca2eaf5` |
| D2-27 | KT-04c **21** kịch bản = 20 ca gốc (QD §4) + ca "Exit khi procsrv chưa Recv" (DUYET §1.3); trả lời Q-01 |

Ba ghi chú bắt buộc (QD §2.1, §2.3, §5.3):

| Ghi chú | Nội dung |
|---|---|
| D-01..D-08, D11, D18 | **Vẫn hiệu lực cho v1**; v2 dùng D2-xx, không dùng lại mã D-xx (chi tiết mục 2.3). |
| Duyệt | bằng dòng `APPROVED <ngày> Agent 0` trong `QUEUE.md` + một mục D2 — **không duyệt bằng chat**. |
| Soak | mọi soak trên máy `ps`; **N = 1.000** với p mục tiêu 0,3 % (mục 9.12). |

---

## 13. Giao việc vòng 1 cho Agent 2

Đưa vào `v2/agent/QUEUE.md`; bảng dưới là bản rút gọn, QUEUE có đủ 5 khối tự chứa. Ngân sách **3 h/task**; task lớn tách. `agentctl next` bỏ qua `REVIEW`.

| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
|---|---|---|---|---|---|
| KT-01 | tag `v1-archive` trên `ca2eaf5`; `docs/v1/KNOWN-ISSUES.md`; chuyển tài liệu v1 vào `docs/v1/` | — | 1 h | READY | tag có message "T33a mở"; KNOWN-ISSUES đủ 7 mục |
| KT-02 | khung `v2/` + `v2/agent/` + `v2/bin/` (không ghi đè), LICENSE tham chiếu root, `vifast-fsutil` | — | 3 h | READY | `cargo build` workspace rỗng xanh; có `v2/bin/` |
| KT-03 | wrapper `v2/bin/v2` + khung `check`/`gate --fast`: fmt, clippy, luật cấu trúc, L0, SPDX; chưa gen-check/L1 | KT-02 | 3 h | BLOCKED | `./v2/bin/v2 check` chạy từ repo root; 5 luật có 5 case âm |
| KT-04a | bảng ánh xạ v1→v2 + `spec/abi.toml` lõi (15 syscall, 19 lỗi, layout 48 B, riscv64) | KT-02 | 3 h | READY | abi.toml parse; đủ bảng ánh xạ |
| KT-04b | object reserved: số + `NotSupported` | KT-04a | 2 h | BLOCKED | số không trùng; trả NotSupported |
| KT-04c | **21 kịch bản** `.scn` chỉ bằng ABI + **kiểm tĩnh** (syscall/method/mã lỗi có thật, đủ tham số; gồm "Exit khi procsrv chưa Recv") | KT-04a | 3 h | BLOCKED | 21 tệp; kiểm tĩnh pass; ca không viết được → BLOCKED "thiếu ABI" |
| KT-04d | gửi Agent 0 duyệt (checklist + 21 kịch bản) | KT-04a,b,c | 1 h | REVIEW | `APPROVED <ngày> Agent 0` |
| KT-05 | `vifast-gen` + gen-check + `docs/abi.md` | KT-04d | 3 h | BLOCKED | code sinh biên dịch; sửa tay bị bắt |
| KT-06 | `vifast-agentctl --project v2\|viscript` | KT-02 | 2 h | BLOCKED | dùng được; JSON có `project` |
| KT-07 | K0.5: boot riscv64 OpenSBI; DebugPutStr/Halt; TAP; 50 boot | KT-02, KT-03 | 3 h | BLOCKED | 50 boot 0 lỗi |
| KT-08 | sim harness + store giả + scheduler 1 hart; kịch bản rỗng tất định theo seed | KT-02 | 3 h | BLOCKED | cùng seed → cùng trace (chưa cần ABI) |
| KT-09 | Untyped trên sim + end-to-end Retype→Configure→Call/Reply | KT-08, KT-05 | 3 h | BLOCKED | invariant tổng byte; kịch bản end-to-end chạy |
| KT-10 | CNode/MDB test-first | KT-09 | 3 h | BLOCKED | property test cây |
| KT-11 | `vifast-model` Untyped/CNode; chạy ca Untyped/CNode của KT-04c | KT-10 | 2 h | BLOCKED | khớp 10.000 seed |
| KT-12 | `vifast-traceformat` + ring | KT-02 | 2 h | BLOCKED | giải mã dump |
| KT-T26 | host test `vifast-fs` + block giả + cache, `MAX_IO=256`, RAW | — | 3 h | READY | test đỏ trước |
| KT-R1 | báo cáo `R1-v2` + `blocked[]` | task READY | 1 h | READY | MD + JSON có `project` |

Nhánh chạy song song: (KT-01, KT-02, KT-04a, KT-T26), (KT-02 → KT-08 → KT-09 → KT-10 → KT-11), (KT-05/KT-06 sau KT-04a/KT-04d). KT-04d **REVIEW** và **chỉ duyệt khi có sản phẩm KT-04a/b/c**; APPROVED mở KT-05. Duyệt tài liệu thiết kế ghi bằng **D2-25**, không ghi dưới KT-04d.

---

## 14. Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Lõi phức tạp (MCS, objmem, CSpace 2 cấp) gây lỗi khó | Sim tất định + mô hình + mutation + bao phủ; unsafe gom trong crate nhỏ có Miri |
| Phình phạm vi | Dòng lõi tới K6 = tương đương v1 + ABI đầy đủ; dòng khác mở theo phụ thuộc |
| Chốt ABI sai ở K0 | Agent 0 duyệt `abi.toml` từng object; dựa trên seL4 MCS; dùng thử ABI bằng kịch bản sim trước khi lên QEMU |
| Bộ sinh code thành gánh nặng | vifast-gen nhỏ, TOML đơn giản, code sinh dễ đọc, được commit |
| Sim xanh, phần cứng đỏ | Cùng kịch bản hai nơi; conformance riêng; trace cùng định dạng |
| Mô hình sai giống core | Viết độc lập, cực đơn giản |
| Lock-free trong vifast-sync | Phần nhỏ nhất; loom/stress; Miri |
| Agent 2 trôi khi chạy dài | STATE.md sau mỗi bước, ngân sách, điều kiện dừng, báo cáo JSON |
| Dòng ViScript tranh tài nguyên dòng lõi | QUEUE riêng; ViScript dùng làm nhánh độc lập khi lõi BLOCKED hoặc theo tỉ lệ người điều phối đặt |
| Phần cứng thật (GPU, Wi-Fi) quá lớn | Ưu tiên virtio; phần cứng thật theo từng mốc HW |
| **Cắt còn 1 arch tới K6** (D2-10) làm lỗi aarch64/x86_64 dồn về sau | OpenSBI là đường duy nhất phải đúng; `PageTableFormat`/`Mmu` dùng chung từ K2; conformance viết sẵn cho 3 arch, chạy lại ngay khi port (K4'/K5') |
| **MCS cắt phần thuật toán** (refill nhiều đoạn, timeout fault) làm phát hiện muộn lỗi ngữ nghĩa ngân sách | Donation + `Sleeping{reason=Budget}` vào từ K1b; mô hình tham chiếu có budget từ đầu; phần hoãn là chi tiết trong, không đổi ABI (D2-16) |
| **objmem là crate unsafe duy nhất** phải tự viết cấp phát thô + Miri | Miri từ commit đầu (D2-14); test `with_many_mut` kiểm alias từng cặp; fuzz chuỗi thao tác; cấm tạo tham chiếu tới cả vùng |
| **Phạm vi hai dòng (OS + ViScript)** tranh ngân sách | D2-20: OS trước, ViScript chỉ khi QUEUE v2 hết task READY (ngoại lệ ST-02a); hai QUEUE độc lập, mỗi task ≤ 3 h |

---

## Phụ lục A. Sổ lỗi v1 mang sang v2

| Lỗi v1 | Mô tả | Ca kiểm trong v2 |
|---|---|---|
| T33a | Đồng hồ lùi sau migrate/reset | Kịch bản sim "đơn điệu sau migrate" + conformance chéo hart |
| T26 | FS 20 % đỏ ở `MAX_IO=256` | **KT-T26**: host test + block giả + cache, RAW read-your-own-write |
| T31 | PAGER2 nhận message lặp | Sim pager 4 client + checker "giao đúng một lần" + đột biến "phát lại Call khi Resume" |
| T-IPC | Lỗi cổng đo bị coi là lỗi sản phẩm | Luật: mọi harness mới phải có case âm chứng minh bắt được lỗi trước khi tuyên bố |
| x86 AP→BSP + PCID | Shootdown chưa xử lý hộp thư khi chờ ack | Đột biến "bỏ xử lý hộp thư khi chờ ack" + conformance TLB khi tới x86 |
| Audit P1-5 | `Ready` nhưng không trong runqueue | Invariant hàng đợi khớp trạng thái + kịch bản riêng |
| Audit Q1-A | Kênh wake lệch 3 port | `vifast-sync` dùng chung mọi arch |

## Phụ lục B. Câu hỏi mở Q1–Q12 của dossier

| Câu | Trả lời |
|---|---|
| Q1 mục tiêu gần | K0 → K0.5 → K1a; không hứa ngày |
| Q2 chiến lược lỗi hiếm | Sim tất định + mô hình; soak chỉ xác nhận |
| Q3 T33a | Không sửa trên v1; mang sang v2 bằng kịch bản (Phụ lục A) |
| Q4 worker C | Thành kịch bản đơn điệu đồng hồ |
| Q5 T26 | KT-T26 trên host |
| Q6 `--strict` | Thay bằng D2-07 (không retry) |
| Q7 tách `lib.rs` | Không; v2 viết mới |
| Q8 trần tĩnh | Object trong Untyped; `.data+.bss ≤ 256 KiB`; `[GT]` đo ở K0.5 |
| Q9 đóng băng x86 | x86 sau K6 (D2-10) |
| Q10 archive tài liệu | `docs/v1/` (báo cáo, decisions, overnight-log); v2 ở `docs/v2/`, ViScript ở `docs/viscript/` |
| Q11 định nghĩa xong | Mỗi mốc có cột tiêu chí; "tương đương v1" có danh sách đo được |
| Q12 nguồn số liệu | `STATUS.md` sinh bởi nightly; tài liệu chỉ chứa `[TK]`/`[ƯL]` |
