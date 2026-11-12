# ViFastOs — Hồ sơ kỹ thuật hợp nhất (bản một tệp)

> **Mục đích.** Tệp này là *toàn bộ* ngữ cảnh cần thiết để một kiến trúc sư/agent
> **không đọc được mã nguồn và không đọc được thư mục `docs/`** vẫn phân tích,
> phản biện và thảo luận lại dự án từ đầu, có căn cứ.
>
> **Đối tượng.** `agent0` (phiên tư vấn/kiến trúc), người điều phối, và người
> đọc lại sau này.
>
> **Ảnh chụp (snapshot).**
> - Thời điểm ghi: 2026-11-12, ~10:05 JST.
> - `HEAD` = `8077d71` (`r322: vòng lặp phát triển 18 giây — gate.sh --smoke + BUILD_ID chỉ hash mã nguồn`).
> - Cây làm việc **có 3 tệp sửa chưa commit**: `crates/vifast-kernel/src/lib.rs`,
>   `crates/vifast-root/src/main.rs`, `docs/11-hien-tai/09-testing.md` — đây là
>   mẻ `r323` đang dở (bỏ `panic!` của worker C, thêm phép kiểm đỏ-trước
>   "reset không được lùi đồng hồ"). Mọi số liệu trong tệp này nói **trạng thái
>   `r322` + mẻ r323 như đang có trên đĩa**, không phải một bản phát hành.
> - 973 commit, nhánh `main`.
>
> **Quy mô.** 25 crate Rust, ~52.500 dòng `*.rs`, kernel portable 19.989 dòng
> (riêng `crates/vifast-kernel/src/lib.rs` = 17.715 dòng). `docs/` = 274 tệp /
> 14 MB; `artifacts/` = 94 thư mục / 1.722 tệp / 16 MB.

---

---

## Mục lục

| § | Nội dung | Đọc khi |
|---:|---|---|
| [0](#0-cách-đọc-tài-liệu-này) | Cách đọc, nhãn độ tin cậy | luôn |
| [1](#1-tldr--đọc-1-trang-là-đủ-để-bắt-đầu-thảo-luận) | TL;DR một trang | luôn |
| [2](#2-bối-cảnh-mục-tiêu-phạm-vi) | Mục tiêu, phạm vi, ngoài phạm vi | cần bối cảnh |
| [3](#3-từ-điển-tối-thiểu-chỉ-những-từ-mà-tệp-này-dùng) | Thuật ngữ | lần đầu |
| [4](#4-kiến-trúc) | Tầng, 25 crate, `Platform`/`Arch`, luồng điển hình | kiến trúc |
| [5](#5-object-capability-và-untyped) | Object, cap, Untyped, invariant | kernel |
| [6](#6-ipc-notification-scheduler-preemption) | IPC, notification, scheduler, 3 đường mất message | kernel |
| [7](#7-bộ-nhớ-ảo-frame-vspace-pagetable-wx) | Frame/VSpace/PTE, W^X, TLB, copy-window | kernel |
| [8](#8-lưu-trữ-vifast-fs-vf04-và-vfs-server-ở-user-space) | VF04, VFS server, RPC, T26 | FS |
| [9](#9-runtime-boot-chương-trình-user) | Boot, BootInfo, layout VA, root/server/client | bring-up |
| [10](#10-abi-syscall-lỗi-quyền-thanh-ghi) | 58 syscall, lỗi, quyền, thanh ghi | ABI |
| [11](#11-driver-và-thiết-bị) | VirtIO, ATA, input, FP off | driver |
| [12](#12-smp-và-đồng-bộ) | BKL, IPI, 2→4 hart | SMP |
| [13](#13-hệ-thống-đo-lường-hợp-đồng-marker-cổng-soak) | Marker, cổng, retry, artifact, BUILD_ID | đo |
| [14](#14-trạng-thái-hiện-tại-số-chuẩn) | Số chuẩn, ngân sách cứng, chưa có | trạng thái |
| [15](#15-lỗi-mở--hồ-sơ-đầy-đủ) | T33a, T26, T31, TLB/PCID, audit | lỗi mở |
| [16](#16-vì-sao-vòng-lặp-phát-triển-hiện-tại-không-hiệu-quả) | Phân tích vòng lặp + 6 phương án | chiến lược |
| [17](#17-nợ-kỹ-thuật-và-rủi-ro-kiến-trúc) | Nợ kỹ thuật, điểm mạnh cần giữ | chiến lược |
| [18](#18-bản-đồ-tài-liệu--và-những-chỗ-tài-liệu-sai-so-với-mã) | Bản đồ docs + chỗ SAI | trước khi trích dẫn |
| [19](#19-ràng-buộc-đang-có-hiệu-lực-đừng-vi-phạm-khi-đề-xuất) | D-01..D-08, luật vận hành, quy ước | trước khi đề xuất |
| [20](#20-câu-hỏi-mở-để-thảo-luận-lại-từ-đầu-với-agent0) | 12 câu hỏi quyết định | bắt đầu thảo luận |
| [21](#21-phụ-lục) | Chỉ mục tệp, hằng số, lệnh, tự kiểm | tra cứu |

## 0. Cách đọc tài liệu này

### 0.1 Bốn nhãn độ tin cậy

Mỗi khẳng định kỹ thuật trong tệp này thuộc một trong bốn loại. Nhãn được ghi
ngay trong câu khi cần phân biệt; mặc định là **[CODE]**.

| Nhãn | Nghĩa | Cách kiểm lại |
|---|---|---|
| **[CODE]** | Tôi đã đọc trực tiếp mã nguồn / chạy lệnh trên cây này | `file:line` ghi kèm khi quan trọng |
| **[ĐO]** | Số đo đã được ghi lại trong nhật ký vòng (không tự chạy lại) | `docs/reviews/progress-g1.md` |
| **[DOC]** | Chỉ có trong tài liệu, **chưa** đối chiếu mã ở phiên này | Cần xác minh trước khi dùng để quyết định |
| **[GT]** | Giả thuyết / suy luận của tôi, chưa chứng minh | Phải có phép đo trước khi tin |

### 0.2 Ba điều tuyệt đối không làm khi đọc tiếp

1. **Đừng cộng số liệu từ `docs/`.** Tài liệu trong repo đã lệch nhau (xem §18).
   Số chuẩn nằm ở §14 của tệp này, và ngay cả §14 cũng ghi rõ chỗ nào chưa chốt.
2. **Đừng tin "PASS" của cổng QEMU là "không có lỗi".** Cổng có retry và có
   `--strict` chưa bật; xem §13.4 và §15.
3. **Đừng đề xuất thêm tính năng trước khi đọc §16.** Vòng lặp phát triển hiện
   tại đang bị chặn bởi *khả năng đo*, không phải bởi *ý tưởng tính năng*.

### 0.3 Nếu cần chi tiết vượt quá tệp này

Yêu cầu theo mẫu: `path:line-range` (ví dụ `crates/vifast-kernel/src/ipc.rs:100-210`),
hoặc "trích toàn văn tài liệu X". Tệp này cố ý **không** chép lại 274 tệp tài liệu;
nó giữ *mô hình*, *bất biến*, *số đo* và *các quyết định đã chốt*.

---

## 1. TL;DR — đọc 1 trang là đủ để bắt đầu thảo luận

**ViFastOs là gì.** Một hệ điều hành viết từ đầu bằng Rust, lõi là **microkernel
kiểu seL4** (capability-based, object sinh từ `Untyped`, IPC đồng bộ, không heap
chung trong kernel). Kernel chỉ làm 5 việc: object/capability, IPC, lập lịch,
bộ nhớ ảo, biến ngắt thành thông báo. Driver, filesystem, pager, supervisor, shell
chạy ở **user space** như tiến trình thường. Cùng một lõi, **ba cổng**:
RISC-V64 (OpenSBI), AArch64 (QEMU `virt`), x86_64 (UEFI/OVMF). Chỉ chạy trên QEMU;
**chưa lên máy thật**.

**Nó đang ở đâu.** Boot + user mode + SMP 2 hart + IPC + capability + VFS có
journal + pager user-space + exec ELF + driver VirtIO (block/net/GPU/input/RNG)
đều **chạy được trên cả 3 kiến trúc** [ĐO]. Cổng đầy đủ 10 bước (build, host test,
sàn test, fuzz, QEMU 3×3, boundary mutation, persistence 7 pha, docs, harness âm,
fmt) xanh ở `r322`. Có ~391–402 host test (con số chưa chốt, xem §14.2),
**677 case âm** của hợp đồng marker, persistence 7 pha × 3 kiến trúc.

**Bốn vấn đề đang mở, theo thứ tự quan trọng:**

| Mã | Vấn đề | Trạng thái | Ảnh hưởng |
|---|---|---|---|
| **T33a** | Treo x86_64 sau `shell>` (~0,3–0,5 % khi soak song song 3 luồng) | **Gốc đã lộ một nửa**: worker C in `TIME MONOTONIC BAD` → `panic!` → root chờ mãi | Chặn mọi tuyên bố "ổn định" |
| **T26** | Vi phạm *read-your-own-write* ở mức entry thư mục của FS (20 % boot đỏ ở `MAX_IO=256`) | Đã có bằng chứng cơ chế, **chưa vá** | Chặn mọi tuyên bố "FS đúng" |
| **x86 shootdown AP→BSP + PCID** | Yêu cầu maintenance gửi tới BSP không được phục vụ → TLB cũ; PCID không xoá đúng | Đã biết, chưa sửa | Chỉ lộ trên KVM/`-cpu max`, nhưng là lỗ hổng đúng đắn |
| **Đo lường** | Cổng có retry, `--strict` chưa bật, đèn chẩn đoán làm nhiễu phép đo | Có cơ chế đếm `RETRY` từ r293 | Mọi kết luận "đã sửa" đều yếu |

**Vì sao vòng lặp hiện tại kém hiệu quả (luận điểm chính của tệp này).**
Không phải vì thiếu ý tưởng, mà vì **chi phí một thí nghiệm quá cao so với độ
phân giải của nó**: một lần cổng đầy đủ ~396 s [ĐO]; một lần soak đủ mạnh để
phân biệt "đã sửa" với "chưa sửa" cần ~600–1200 boot (hàng giờ); trong khi đó
mỗi đèn chẩn đoán thêm vào lại **làm đổi thời gian** của đúng cái race đang săn
(đã ghi nhận: đèn nặng ở server làm tỉ lệ exec hỏng tăng). Kết quả là các vòng
liên tiếp *r312 → r323* (11 vòng) vẫn chưa đóng được một lỗi. Chi tiết và các
phương án ở §16.

**Câu hỏi lớn nhất cần agent0 trả lời** (chi tiết §20): *có nên tiếp tục săn
T33a/T26 bằng quan sát thụ động trên QEMU, hay đổi chiến lược sang (a) tái hiện
tất định trên host, (b) cắt bớt bề mặt sản phẩm để giảm nhiễu, (c) chấp nhận
đóng băng tính năng và chỉ làm hạ tầng đo?*

---

## 2. Bối cảnh, mục tiêu, phạm vi

### 2.1 Hai hướng sản phẩm, một kernel [DOC + CODE]

| | **Desktop** | **Space** (an toàn sống còn) |
|---|---|---|
| Mục đích | Máy tính cá nhân, máy trạm | Hệ thống không được phép sai |
| Tạo tiến trình lúc chạy | Có | Không (cấu hình tĩnh) |
| Cắm thiết bị lúc chạy | Có | Không |
| Ngân sách CPU (MCS) | Tuỳ chọn | Bắt buộc cho task không quan trọng |
| Cách "khoá" cấu hình | Không khoá | Boot xong xoá `Untyped` gốc + quyền IRQ/sched |

**Hiện trạng:** kernel có đủ *cơ chế* cho cả hai (Untyped, MCS budget/period,
quyền tối thiểu) nhưng **chưa có cơ chế "profile tĩnh" hoàn chỉnh**; bản đang chạy
là cấu hình desktop-demo. Nguyên tắc đã chốt: *không sửa kernel để "làm tĩnh"* —
tính tĩnh đạt được bằng việc không còn ai giữ cap tạo object.

### 2.2 Ngoài phạm vi (đã tuyên bố, đừng đề xuất lại như việc mới)

- Tương thích nhị phân Linux/POSIX đầy đủ (là dự án riêng; hướng đang cân nhắc là
  chạy Linux trong VM hoặc lớp tương thích dần).
- Kiểm chứng hình thức (Kani/Verus) — chưa có, không nằm trong lộ trình gần.
- CI hosted (GitHub) — **đã bỏ có chủ ý**; cổng là script cục bộ + máy thứ hai qua SSH.
- Compositor/framebuffer console, bộ gõ ViFastKey, mạng đầy đủ (socket API/TCP
  stack thật), libc, dynamic linking: **chưa có**, là mốc M3–M5 của lộ trình.
- Máy thật: chưa. Mọi số đo là QEMU/TCG.

### 2.3 Năm nguyên tắc làm việc đang có hiệu lực [DOC, nhưng được thực thi bằng script]

1. **Đo được mới gọi là xong.** Mỗi thay đổi phải có test host hoặc marker cổng.
2. **Test là hợp đồng.** `scripts/harness_spec.py` là nguồn sự thật duy nhất về
   ngữ nghĩa "boot đúng"; đổi hành vi guest ⇒ đổi hợp đồng + tài liệu **cùng commit**.
3. **Cơ chế ở kernel, chính sách ở user.**
4. **Mọi syscall có thời gian giới hạn**; thao tác dài phải có preemption point.
5. **`unsafe` gom một chỗ**: kernel portable `#![forbid(unsafe_code)]`; `unsafe`
   chỉ ở `vifast-arch-*`, `vifast-platform-*`, entry assembly.

---

## 3. Từ điển tối thiểu (chỉ những từ mà tệp này dùng)

| Thuật ngữ | Nghĩa trong dự án này |
|---|---|
| **Untyped** | Vùng nhớ vật lý thô, chưa có kiểu; nguồn duy nhất để sinh object. User giữ cap Untyped; kernel không có heap chung. |
| **Retype** | Biến một phần Untyped thành object có kiểu (Frame, TCB, Endpoint…). |
| **Capability (cap)** | "Chìa khoá" không giả mạo được: `{object, quyền, badge}`. Mọi truy cập đi qua cap. |
| **ObjRef** | `{index, gen}` — con trỏ logic vào bảng object; `gen` tăng khi slot tái sử dụng ⇒ phát hiện use-after-free. |
| **CSpace / CNode** | Bảng slot chứa cap của một TCB. Bản này: **phẳng**, 128 slot cho root, 16 slot cho task. |
| **Endpoint** | Điểm hẹn IPC đồng bộ: hai hàng đợi FIFO 8 phần tử (`send`, `recv`). |
| **Notification** | Word bit cho báo hiệu bất đồng bộ; có waiter; dùng cho ngắt và cờ. |
| **Badge** | Số gắn vào cap Endpoint/Notification khi mint; người nhận thấy, dùng để phân biệt người gửi. |
| **reply_epoch** | Số thế hệ mỗi lần `Call`; chống `Reply` tới nhầm client sau restart (K49). |
| **VSpace** | Không gian địa chỉ ảo: root page table + range VA + reverse map. |
| **Frame** | Trang vật lý 4 KiB, có thể map. |
| **W^X** | Một trang không vừa ghi vừa thực thi — kiểm **theo Frame**, không theo mapping. |
| **BKL** | Big Kernel Lock: một khoá cho toàn kernel; bản này dùng ticket lock. |
| **Hart** | Hardware thread (lõi). `MAX_HARTS = 4`, scheduler mới chứng minh trên 2. |
| **Shootdown** | Xoá mục TLB trên hart khác sau khi đổi page table. |
| **Maintenance IPI** | IPI "hộp thư" (`vifast-maint`) để yêu cầu hart khác flush TLB/I-cache. |
| **Fault endpoint** | Endpoint mà kernel gửi message khi task user bị page fault. |
| **Pager** | Tiến trình user xử lý page fault của tiến trình khác (demand paging). |
| **MCS** | Lập lịch có ngân sách (`SchedContext` budget/period). |
| **Marker** | Dòng log có ngữ nghĩa hợp đồng, ví dụ `PREEMPT OK`, `SHELL READY`; harness đếm. |
| **`EXACT_ONE`** | Marker phải xuất hiện **đúng một lần** (không lặp, không thiếu). |
| **`--strict`** | Chế độ cổng coi `RETRY > 0` là ĐỎ. Đã có, **chưa bật**. |
| **`BUILD_ID`** | `<git sha>-<hash8 của diff mã nguồn>`; in trong serial để chặn "đo nhầm binary". |
| **Soak** | Chạy N boot liên tiếp (thường song song nhiều luồng) để đo tỉ lệ lỗi hiếm. |
| **T33a / T26 / T31…** | Mã lỗi nội bộ (xem §15). |

---

## 4. Kiến trúc

### 4.1 Sơ đồ tầng và ranh giới tin cậy [CODE]

```
┌────────────────────────────────────────────────────────────────────────┐
│ USER SPACE (không tin cậy ở mức kernel; root là supervisor tin cậy)    │
│  root (Rust ELF, supervisor: spawn, pager, exec)                       │
│  server (VFS: sở hữu BlockDevice cap, phục vụ RPC)                     │
│  client (shell + demo: ls/cat/run/runfs/exec/fsck…)                    │
│  /bin/vf (multicall: uname, uptime, free, ps, df, args…)               │
│  pager (xử lý page fault; 4 client)                                    │
├───────────────────────── syscall (ecall / svc #0 / int 0x80) ──────────┤
│ vifast-user: wrapper syscall 3 arch, layout user, slot/untyped helper  │
├────────────────────────────────────────────────────────────────────────┤
│ vifast-kernel: object · capability · IPC · scheduler · VSpace ·        │
│                ramfs · invariants · MCS     [no_std, forbid(unsafe)]   │
├────────────────────────────────────────────────────────────────────────┤
│ vifast-runtime: boot sequence · syscall dispatch · timer/preempt ·     │
│                 context switch dùng chung · trait Platform             │
├────────────────────────────────────────────────────────────────────────┤
│ vifast-platform-*: trap frame, MMU, timer, IRQ, SMP, user entry        │
│ vifast-arch-*:    UART/PL011/COM1, clock, IRQ mask, shutdown, port I/O │
├────────────────────────────────────────────────────────────────────────┤
│ vifast-kernel-{riscv64,aarch64,x86_64-uefi}: entry asm + linker +      │
│                                                ảnh user nhúng          │
└────────────────────────────────────────────────────────────────────────┘
   RISC-V64 (OpenSBI)      AArch64 (QEMU virt)      x86_64 (UEFI/OVMF)
```

**Ranh giới tin cậy thực tế:** kernel + root task là TCB (trusted computing base).
`server`/`client`/`/bin/vf` là tiến trình user bình thường, chỉ có cap được cấp.
Nhưng lưu ý: `root` hiện giữ **rất nhiều** cap (128 slot) và làm cả supervisor,
pager dispatcher, loader, và chủ sở hữu phần lớn Untyped — đây là điểm nợ kiến
trúc (xem §17).

**Thiết bị:** `vifast-virtio` (VirtIO-MMIO v2: block/net/GPU/input/RNG) cho
RISC-V và AArch64; x86 dùng **ATA PIO + PS/2**, không VirtIO. Không có PCI/MSI
ở cả ba cổng (x86 có quét PCI config space để chẩn đoán, không dùng để bind driver).

### 4.2 Bản đồ 25 crate [CODE]

| Nhóm | Crate | Vai trò | LOC |
|---|---|---|---|
| ABI | `vifast-abi` | Syscall (58), mã lỗi, quyền, object type, `Message`, layout user, `COPY_WINDOWS`, protocol pager/VFS | 1.477 |
| Kernel | `vifast-kernel` | Object/cap/IPC/sched/VSpace/ramfs/invariants | 19.989 |
| Runtime | `vifast-runtime` | Boot, dispatch, timer/preempt, trait `Platform` | 2.558 |
| Paging | `vifast-pte` | Encode/decode PTE Sv39 / AArch64 4-level / x86 PML4 | 515 |
| ELF | `vifast-elf` | Parse/validate ELF64 `PT_LOAD`, W^X, overlap | 725 |
| FS | `vifast-fs`, `vifast-fsutil` | VFS VF04 (journal, link, fsck, cache) + tool host | 4.543 + 328 |
| Driver | `vifast-virtio` | VirtIO-MMIO: block, net, GPU, input, RNG | 2.755 |
| Platform helper | `vifast-acpi`, `vifast-maint` | ACPI RSDP/MADT; hộp thư shootdown dùng chung | 341 + 417 |
| Arch primitives | `vifast-arch`, `vifast-arch-{riscv64,aarch64,x86_64}` | Trait `Arch`; UART/PL011/COM1, clock, IRQ, shutdown | 633 + 117/115/115 |
| BSP | `vifast-platform-{riscv64,aarch64,x86_64}` | Trap, MMU, timer, IRQ, SMP, user entry, static pools | 2.203 / 2.139 / 3.592 |
| Binary kernel | `vifast-kernel-{riscv64,aarch64,x86_64-uefi}` | Entry asm, linker script, ảnh user nhúng | 128 / 124 / 270 |
| User | `vifast-user` | Wrapper syscall 3 arch | 1.228 |
| Task | `vifast-root`, `vifast-server`, `vifast-client`, `vifast-bin` | Supervisor, VFS server, shell/demo, `/bin/vf` multicall | 4.487 / 1.235 / 2.169 / 267 |

**Quy tắc phụ thuộc (được thực thi bằng cấu trúc crate, không phải bằng review):**
`platform-* → runtime → kernel → abi`; `vifast-kernel` **không bao giờ** phụ thuộc
platform/arch cụ thể. Thêm kiến trúc mới = thêm 3 crate (`arch-*`, `platform-*`,
`kernel-*`) và **không** sửa runtime/kernel — nếu phải thêm `cfg(target_arch)`
vào kernel/runtime thì đó là dấu hiệu thiết kế sai.

### 4.3 Hợp đồng `Platform` / `Arch` — chỗ kernel "mù" với phần cứng [CODE]

`vifast_arch::Arch` cho kernel biết: trap frame, hart id, MMIO copy, page-table
primitive, flush TLB, `read_user`/`write_user`. Toàn bộ phần còn lại do từng port
cài trong `Platform`:

```rust
pub trait Platform: Arch + Console {
    type Context: Copy + 'static;
    const USER_CODE_VA: usize;
    const USER_DATA_VA: usize;
    const ELF_MACHINE: u16;

    fn init_platform(info: PlatformInfo);
    fn init_kernel() -> &'static mut Kernel;   // reset() static Kernel
    fn set_kernel(kernel: *mut Kernel);
    fn kernel() -> &'static mut Kernel;

    fn data_page() -> &'static mut [u8];       // data Untyped
    fn metadata_page() -> &'static mut [u8];   // metadata Untyped
    fn boot_page() -> &'static mut [u8];       // object khởi động
    fn vspace_root() -> u64;
    fn switch_root(root_pa: u64, asid: u16) -> bool;
    fn page_table_ops() -> &'static dyn PageTableOps;
    fn check_kernel_mappings(root_pa: u64) -> bool;
    fn tick_hz() -> u32 { 100 }

    fn context_for_user(entry: usize, stack: usize) -> Self::Context;
    fn contexts() -> &'static mut [Self::Context];
    fn enter_user(context: &Self::Context) -> !;
    fn prepare_user();
    fn start_timer(); fn reload_timer(); fn ack_timer();
    fn start_smp();
    fn post_idle_ready() -> bool;              // dùng cho marker SMP POST IDLE OK
}
```

**Hệ quả thiết kế:** `vifast-kernel` không biết địa chỉ UART, page size thật, số
cấp bảng trang hay vector ngắt. Đổi port không sửa kernel. Ngược lại, **mọi thứ
"thật" nằm ở platform** — nên bug tầng platform chỉ hiện trên QEMU, không hiện
trong host test (đây là gốc của nhiều lần "host xanh, QEMU đỏ").

### 4.4 Năm luồng điển hình [CODE]

**Boot** (chi tiết §9.1):
`entry asm → Platform::init_platform → Kernel::init_kernel (reset) → selftest →
start_smp → tạo Untyped → build root VSpace + parse root ELF → dựng server VSpace →
BootInfo → prepare_user → enter_user(root)`.

**Call/Reply:**
`client Call(ep, msg)` → kernel tra cap (quyền W) → nếu server đang `Recv` thì giao
message, client `BlockedReply`, server ghi `caller = (client, reply_epoch)` →
server `ReplyRecv` → kernel kiểm `epoch` còn khớp mới đánh thức client.

**Ngắt:**
thiết bị → platform `deliver_irq` → `Kernel::signal` notification → driver (user)
đang `Recv`/`Wait` nhận `LABEL_NOTIFICATION` → xử lý → `IrqAck` → kernel mở che.
Hiện phần lớn driver **polling**; chỉ console input và timer là IRQ thật.

**Page fault:**
MMU fault → `fault::suspend` lưu trap frame → nếu TCB có `fault_ep` thì kernel
gửi `Message(LABEL_FAULT, [addr, cause, slot, tcb])` như một `send` **không phải
Call** → TCB `BlockedFault` → pager `FrameMap` vào VSpace của nạn nhân →
`TcbResume` → task chạy lại đúng lệnh.

**Exec:**
shell gửi `LABEL_RUN`/`runfs` → root đọc ELF từ VFS → tạo VSpace mới, copy theo
`PT_LOAD`, cấp stack, mint cap qua `TcbGrantCap`, ghi `argc/argv` bằng `TcbSetArgs`
→ `TcbResume`. Chuỗi teardown của bản exec trước là chỗ từng rò rỉ (đã vá r205).

---

## 5. Object, capability và Untyped

Nguồn: `crates/vifast-kernel/src/object.rs`, `crates/vifast-kernel/src/lib.rs`.

### 5.1 Trần cố định — toàn bộ hệ thống nằm trong các mảng này [CODE]

```rust
MAX_OBJECTS      = 168     // tổng số object mọi loại
MAX_TCBS         = 24      // số thread tối đa
MAX_CAPS_ROOT    = 128     // cspace của root
MAX_CAPS_TASK    = 16      // cspace của task thường (CNode phẳng 16 slot)
MAX_QUEUE        = 8       // hàng đợi send/recv mỗi endpoint
MAX_FDS          = 8       // fd mỗi TCB
MAX_MAPPINGS     = 64      // mapping ngược mỗi VSpace
MAX_IOMMU_MAPPINGS = 32    // DMA mapping mỗi domain
MAX_HARTS        = 4
NOTIFICATION_WAITERS = 6   // waiter tối đa một notification
KERNEL_STATIC_BUDGET = 566_272 byte  // const assert: size_of::<Kernel>() <= ngân sách
```

**Đây là ràng buộc kiến trúc quan trọng nhất khi thảo luận tính năng.** Không có
heap chung: mọi object lấy từ `Untyped` do user giữ. `Kernel` là một static
(~566 KB) nằm trong `.data`/`.bss` của platform; nếu vượt ngân sách, build **phải
đỏ** (đã có `const assert` + guard runtime, thêm sau vụ AArch64 tràn `.data` đè
`KERNEL_PTR` làm DABT mù ở vòng 30).

**`MAX_TCBS = 24` là trần cứng của "số tiến trình đồng thời"**, trừ khi tái cấu
trúc pool. Hiện tại root + server + client + 4 fault + ~8 worker + pager + pager
client ×3 + idle + exec ≈ 20–22 slot — **rất sát trần**. Đây là lý do nhiều tính
năng "thêm một tiến trình nữa" bị chặn ngay từ thiết kế.

### 5.2 Object và ObjRef [CODE]

```rust
pub struct Object {
    pub gen: u32,                 // tăng mỗi lần slot tái sử dụng
    pub kind: Kind,
    pub owner: Option<ObjRef>,    // Untyped đã tạo object này
    pub creator: Option<CapRef>,  // cap chính xác đã dùng để retype
    pub alloc_offset: u32,        // offset trong Untyped (để reclaim)
    pub alloc_len: u32,           // byte đã charge
}
```

`Kind` gồm: `Free`, `Endpoint`, `Notification`, `Tcb`, `Untyped`, `Frame`,
`VSpace`, `PageTable`, `CNode`, `SchedContext`, `Irq`, `Iommu`, `BlockDevice`.

`ObjRef { index, gen }` là **con trỏ logic**. Slot tái sử dụng ⇒ `gen` tăng ⇒ mọi
`ObjRef` cũ bị từ chối (`ObjectDestroyed`). Đây là cách phát hiện use-after-free
mà không cần con trỏ thô trong lõi portable. Giới hạn đã biết: `gen: u32` có thể
tràn sau 2^32 lần tái sử dụng cùng slot (rất xa, nhưng với hệ chạy nhiều năm cần
`u64` hoặc cấm tái sử dụng — xem §17).

### 5.3 Capability [CODE]

```rust
pub struct Cap {
    pub obj: ObjRef,
    pub ty: ObjType,
    pub rights: Rights,       // bitmask 8-bit
    pub badge: u64,
    pub id: u64,              // tăng đơn điệu toàn cục
    pub root: bool,           // true chỉ cho Untyped gốc do runtime cài
    pub parent: Option<CapRef>,
}
pub struct CapRef { pub addr: SlotAddr, pub id: u64 }
```

`Rights`: `READ=0x1 | WRITE=0x2 | EXEC=0x4 | GRANT=0x8`.

Bất biến:
- `derive` **chỉ giảm** quyền (giao quyền), không bao giờ tăng.
- Chỉ `Endpoint`/`Notification` mang badge qua `derive`.
- Cap nhận qua IPC **bị bỏ `GRANT`** ⇒ không chuyển tiếp tuỳ tiện được.
- `parent` dựng CDT để `CapRevoke`; revoke khớp **cả slot và id** nên slot tái
  sử dụng không "thừa kế" creator cũ (một lỗi đã gặp, K62).
- `root = true` chỉ do runtime cài cho Untyped lúc boot; `CapCopy`/IPC/reparent
  luôn giữ `root = false` ⇒ chỉ root cap mới thu hồi được cả nhóm.

Ba thao tác nền:
- `install_cap(tcb, slot, cap)`: slot hợp lệ (1..MAX-1, slot 0 không dùng),
  object còn sống, slot trống.
- `lookup_cap(tcb, slot, want_type, need_rights)`: tồn tại + đúng type + đủ quyền
  + object còn sống.
- `copy_cap(src, dst, rights, badge)`: derive + gắn `parent` + cài vào đích.

### 5.4 Revoke — đường thu hồi duy nhất [CODE]

`revoke_cap` xoá mọi capability con–cháu, **giữ** cap gốc. Nếu cap gốc là
`Untyped`, kernel còn phải:

1. unmap mọi `Frame` con khỏi **tất cả** VSpace (và trả danh sách VA cần flush);
2. detach `Tcb` con khỏi runqueue/endpoint queue, xoá `caller` trỏ tới nó;
3. đánh thức waiter/sender/receiver của Endpoint/Notification bằng `ObjectDestroyed`;
4. đặt object con `Kind::Free`, tăng `gen`;
5. **sau đó** mới reset `bump = 0`, `objects = 0`.

Thứ tự revoke với Untyped là **Frame → PageTable sâu nhất → PageTable cha →
VSpace**, để Frame còn unmap được leaf trước khi bảng cha bị gỡ. Sai thứ tự này
là một lớp lỗi đã từng xuất hiện (K52, K61).

Giới hạn: độ sâu revoke bị chặn bởi `MAX_OBJECTS * MAX_CAPS_ROOT`; chưa có
preemption point khi huỷ Endpoint có hàng đợi dài.

### 5.5 Untyped: cấp phát và thu hồi [CODE]

```rust
pub struct Untyped {
    pub base: usize,  pub size: usize,  pub bump: usize,
    pub page_size: usize,
    pub max_objects: usize,  pub objects: usize,  pub reserved: usize,
    pub free: [Option<FreeRegion>; 16],   // (offset, len) đã trả
    pub leaked: u32,                      // byte không tái sử dụng được
}
```

**Cấp phát `untyped_take`:** (1) tìm vùng free **đúng kích thước & căn**; (2) nếu
không, lấy vùng free lớn hơn đầu tiên rồi tách phần dư (first fit); (3) nếu hết,
cấp từ `bump` (căn lên theo `align`). Mỗi object tiêu 1 slot `objects`; thiếu
byte **hoặc** thiếu `objects + reserved + 1` ⇒ `OutOfMemory`.

**`reserved`** giữ tổng `max_objects` của các Untyped con trực tiếp, để sibling
không tiêu hết quota của con. Untyped con có `bump/objects/reserved/free` riêng;
revoke Untyped cha thu hồi object bên trong con trước, rồi huỷ con và trả
`reserved`; revoke cap con chỉ reset nội dung con, không đụng cha.

**Thu hồi `untyped_reclaim`:** nếu vùng sát `bump` thì hạ `bump` rồi gộp các vùng
free liền kề bên dưới; ngược lại gộp với vùng liền kề và đưa vào free list 16
entry. Free list đầy ⇒ object **vẫn** bị destroy nhưng byte cộng vào `leaked`
(không tái sử dụng, in ra để theo dõi). Double free bị coi là leak, không làm
hỏng free list.

**Charge của mỗi loại (quan trọng khi tính ngân sách):**

| Loại | Charge | Ghi chú |
|---|---:|---|
| `Frame` | 4.096 | căn trang; zero khi cấp |
| `PageTable` | 4.096 | zero khi cấp |
| `VSpace` | 4.096 × `vspace_pages` | root + aux; zero + install kernel slots |
| `Endpoint` | 256 | |
| `Notification` | 128 | |
| `Tcb` | 4.096 | **quota**, không phải `size_of::<Tcb>()` (TCB thật ~1.648 B) |
| `CNode` | 4.096 | |
| `SchedContext` | 64 | |
| `Iommu` | 1.024 | |
| `BlockDevice` | 64 | |

### 5.6 Object lifecycle — ba đường chết [CODE]

1. `CapRevoke` trên Untyped: thu hồi cả cây object do cap con tạo.
2. `TcbDestroy` (syscall 49): reparent cspace rồi destroy một TCB **không phải
   current**; `WouldBlock` khi target có waiter (`TcbWait`).
3. `ObjectDestroy` (syscall 50): destroy **một** object qua cap gốc. Nhận
   `Frame`, `Endpoint`, `Notification`, `PageTable`, `VSpace`, `CNode`,
   `SchedContext`; từ chối `Tcb`, `Untyped`, `Irq`, `Iommu`, `BlockDevice`.

Điều kiện `ObjectDestroy`, **theo đúng thứ tự**:

1. cap hợp lệ, object còn sống, caller có `WRITE`;
2. cap phải là cap **gốc** do `Retype` tạo (`cap.parent == object.creator`),
   không phải bản copy/mint, không phải cap runtime (`root = true`);
3. không còn cap nào khác trỏ tới ⇒ nếu còn, trả `HasDependents` (phải `CapRevoke` trước);
4. object không đang dùng ⇒ nếu đang dùng, trả `Busy`.

| Loại | `Busy` khi |
|---|---|
| `Frame` | còn mapping trong bất kỳ VSpace/IOMMU domain, hoặc là IPC buffer |
| `Endpoint` | send/recv queue không rỗng, hoặc là `fault_ep` của TCB |
| `Notification` | có waiter, bind vào TCB, hoặc gắn với IRQ |
| `PageTable` | đang gắn vào VSpace, hoặc còn PTE khác 0 |
| `VSpace` | có TCB dùng, còn mapping, hoặc có PageTable gắn vào |
| `CNode` | đang là cspace của TCB |
| `SchedContext` | **không** từ chối: destroy tự unbind + đánh thức TCB |

Điểm chưa làm: destroy `Irq/Iommu/BlockDevice/Untyped`; huỷ Frame đang map (phải
`FrameUnmap` trước); pager eviction vẫn dùng đường riêng chứ chưa dùng
`ObjectDestroy`.

### 5.7 Invariant checker — "hợp đồng tự kiểm" của kernel [CODE]

`Kernel::check_invariants()` chạy trong selftest (và có mutation test chứng minh
nó **bắt được** lỗi). Nội dung:

- runqueue ≤ `MAX_TCBS`, không trùng TCB; TCB queued phải thật sự có trong runqueue;
- `current` phải `Running` và **không** queued;
- cspace trỏ object sống, đúng type; fd trỏ file còn tồn tại;
- mapping đúng alignment, không overlap, có `READ`, không W+X trên một leaf;
- `Frame.mapped_w/mapped_x` khớp **chính xác** số leaf W/X trong toàn bộ reverse map;
- `Untyped.objects`/`reserved` khớp cây owner; `objects + reserved <= max_objects`;
- Frame/PageTable/VSpace root+aux **không chồng dải PA**; Untyped chỉ lồng theo
  owner chain và con nằm trọn trong cha;
- alignment của Frame/PageTable; `bump` không vượt `size`; free region dương,
  không chồng nhau, nằm dưới `bump`; `leaked <= size`;
- `alloc_offset/alloc_len` của object nằm trong Untyped chủ;
- notification waiter còn sống;
- bit `harts_loaded` chỉ được đặt cho hart đã **từng thật sự chạy** (`harts_seen`).

Đây là tài sản lớn nhất của dự án: nó biến nhiều bất biến ngầm thành phép kiểm
chạy được. Khi đề xuất thay đổi cấu trúc, câu hỏi đầu tiên nên là *"bất biến nào
trong danh sách này sẽ phải đổi, và đổi thì test nào canh?"*.

---

## 6. IPC, notification, scheduler, preemption

Nguồn: `crates/vifast-kernel/src/ipc.rs`, `crates/vifast-kernel/src/lib.rs`,
`crates/vifast-runtime/src/lib.rs`.

### 6.1 Endpoint [CODE]

```rust
pub struct Endpoint { pub send: Queue, pub recv: Queue }   // mỗi queue 8 phần tử, FIFO
```

- `send` chứa TCB đang `BlockedSend`; `recv` chứa TCB đang `BlockedRecv`.
- Hàng đầy ⇒ `OutOfMemory`.
- Không có "endpoint nhiều người nhận"; server phải `Recv` tuần tự, giữ **một**
  caller tại một thời điểm.

### 6.2 Ngữ nghĩa bốn syscall IPC [CODE]

**`Send`** (`is_call = false`): kiểm quyền `GRANT` nếu có cap; nếu có receiver
đang chờ thì chuyển cap (nếu cần), gán `receiver.resume = Some(msg)`, đưa receiver
`Ready`; nếu không có receiver thì lưu `PendingSend` trong sender, sender
`BlockedSend`, đẩy vào `endpoint.send`.

**`Call`**: như trên nhưng sender **luôn** `BlockedReply` (kể cả khi được giao
ngay), và ghi `caller = (sender, reply_epoch)`.

**`Recv`**: nếu receiver đang có caller chưa `Reply` ⇒ `WouldBlock` (không cho ghi
đè slot caller). Nếu có sender chờ ⇒ lấy `PendingSend`, chuyển cap vào **đúng**
`recv_slot` receiver chỉ định (slot phải trống; **không fallback**), nếu là Call
thì ghi caller, trả message. Nếu không có ai ⇒ `BlockedRecv` + lưu `recv_slot`.

**`Reply`**: lấy `caller` của server; kiểm client vẫn `BlockedReply` **và** đúng
`reply_epoch`; gán resume, đưa client `Ready`. Reply hai lần ⇒ `NoCaller`.
Server phải `Reply` trước khi nhận Call tiếp theo.

**`ReplyRecv`**: reply rồi recv; nếu không có caller thì chỉ recv.

**`reply_epoch`** là chốt chống "Reply tới nhầm client" sau restart (K49). Lịch sử:
epoch từng **không bao giờ tăng** nên guard thành rỗng (phát hiện ở r290/r293);
nay tăng mỗi `Call` và nới lên `u16` (u8 quay vòng sau 256 lần). Có test
`every_call_issues_a_fresh_reply_epoch` và `epoch_survives_256_calls`.

### 6.3 Notification [CODE]

```rust
pub struct Notification { pub word: u64, pub waiters: [Option<ObjRef>; 6] }
```

- `signal(ntfn, bits)`: OR bit vào `word`; đánh thức **mọi** waiter đang block
  (thiết kế đổi ở vòng 88: root park 4 worker trên một notification; mọi biến thể
  "một waiter"/"hai slot" đều làm hỏng boot).
- `wait_as`: trả `word` nếu có bit (hoặc ngay khi có), nếu không thì block.
- `poll`: trả `word` rồi **clear**, không block. `poll` còn hạ `pending` của mọi
  IRQ trỏ tới notification đó để cạnh ngắt kế không bị coalesce (K46).
- Không có waiter ⇒ bit tích luỹ (không mất tín hiệu).
- `recv` trên endpoint có **bound notification** sẽ poll notification trước khi
  chặn (K42) — cần cho driver kiểu "vừa chờ lệnh vừa chờ ngắt".

### 6.4 Chuyển capability qua IPC [CODE]

1. `lookup_cap(sender, slot, None, GRANT)`;
2. chọn slot đích = đúng `recv_slot` receiver chỉ định; nếu slot 0/đã dùng ⇒
   `InvalidArg`/`SlotOccupied`, **không** fallback;
3. `copy_cap(..., rights.without(GRANT), badge = 0)`;
4. ghi `message.cap = dest`.

Nếu bước 3 thất bại, sender được đưa **lại** queue để không mất trạng thái (K48).
Quy tắc "không rửa quyền": cap nhận qua IPC không có `GRANT` ⇒ không rebadge,
không chuyển tiếp tuỳ ý — có marker `IPC RIGHTS OK`/`BADGE SPOOF DENIED OK` canh.

### 6.5 Scheduler [CODE]

- Ready queue **cố định theo từng hart**: `runq[[Option<ObjRef>; MAX_TCBS]; 4]`.
- `enqueue` chọn hart thấp nhất trong affinity mask còn chỗ.
- `pick_on(hart)`: local queue trước, sau đó **work-steal** TCB có affinity chứa
  hart đích (chỉ lấy TCB `Ready` — K40).
- Ưu tiên **số lớn hơn chạy trước**; cùng ưu tiên dùng `queue_seq` FIFO.
- `pick_on()` đặt `Running`, tăng `context_switches`; `yield_current_on()` đặt
  current về `Ready` rồi enqueue lại.
- Blocked **không** nằm trong runqueue.
- `BlockedSleep` dùng `wake_at`; mỗi `tick()` gọi `wake_sleepers()`.
- `SchedContext` budget/period: `BlockedBudget`, `wake_budgeters()`.
- Hart 0 có **idle TCB** priority 0 để scheduler không hết task khi mọi task block;
  hart phụ dùng idle loop của platform.

**Chuyển context** (`runtime::syscall::switch_blocked`): lưu context cũ vào
`Platform::contexts()[old]` → `kernel.pick()` → nạp context mới → nếu TCB mới có
`resume` thì ghi message vào thanh ghi.

**Timer/preemption** (`runtime::timer::on_timer`): lưu context → `kernel.tick()` →
(đèn sau 20 tick) → reload/ack timer → yield + pick → nạp context + resume.

### 6.6 Ba đường mất message đã biết — và bài học chung

Đây là chỗ đáng đọc kỹ nhất, vì nó là *mẫu* của loại lỗi đang săn.

**Đường 1 (đã vá, r319 — có test đỏ-trước 0,0 giây).** Trong `ipc::send` nhánh
"giao ngay": code lấy receiver ra khỏi `ep.recv` **mà không kiểm trạng thái**, rồi
`make_ready(receiver)` — mà `make_ready` **cố ý không đánh thức** task đang
`Running` (để không có hai chủ). Nếu receiver đang `Running` (vừa được đánh thức
bởi message khác nhưng chưa về user), message nằm trong `resume` mà không ai
chạy ⇒ **mất vĩnh viễn**. Vá: `send` chỉ pop receiver nếu nó **wakeable**; nếu
không thì để nguyên trong hàng và xếp sender vào `ep.send`.

**Đường 2 (giả thuyết, đã bị bác bằng đo).** Worker gửi báo cáo bằng `send` và
`let _ =` nuốt lỗi ⇒ mất báo cáo. Đo 300 boot với bản vá "gửi lại tới khi được
nhận": **0 lần** `WORKER SEND RETRY` xuất hiện ⇒ `send` không hề lỗi ⇒ **revert**
theo luật D18.

**Đường 3 (đang là nghi phạm chính của T33a).** Không phải mất message ở kernel:
**người gửi chết trước khi gửi**. Worker C thấy `GetTime` lùi ⇒ `panic!` ⇒ TCB
chết ⇒ root chờ mãi trong `recv`. Đây là lý do "GATE STATES" cho thấy
`ready=0, running=1, brecv=2` — hệ đứng yên vì một task chết, không vì message mất.

**Bài học chung:** mọi `let _ = send(...)` là một chỗ giấu lỗi. Đèn `GATE SENDFAIL`
(r318) được thêm đúng để phân biệt *"chưa ai gửi"* với *"gửi rồi mà bị từ chối"*.

### 6.7 Nợ kỹ thuật của tầng này

- Chưa có priority donation; MCS mới ở mức budget/period.
- `RunQueue.remove`, `cancel_ipc` (`retain`), `find_tcb` là O(n) theo dữ liệu ⇒
  chưa đạt yêu cầu "mọi vòng lặp trong kernel có cận trên phụ thuộc hằng số".
- Chưa có shootdown đồng bộ cho mọi đường unmap (một phần đã có qua maintenance IPI).
- `Yield` có thật sự chuyển context (đã vá), nhưng comment cũ trong `01-abi.md`
  còn ghi "chưa thực sự nhường CPU" — tài liệu lệch.

---

## 7. Bộ nhớ ảo: Frame, VSpace, PageTable, W^X

Nguồn: `crates/vifast-kernel/src/object.rs`, `crates/vifast-kernel/src/lib.rs`,
`crates/vifast-runtime/src/lib.rs`, `crates/vifast-platform-*/src/platform.rs`.

### 7.1 Mô hình: page table thật là nguồn sự thật duy nhất [CODE]

Kernel portable đi **generic page-table walk** qua trait `PageTableOps`
(`read_pte`, `write_pte`, `flush_va`, `sync_icache`, `table_span`, …). Page table
thật là nguồn sự thật của phần cứng; `VSpace.mappings` chỉ còn là **reverse map**
phục vụ revoke/unmap và tính bộ đếm W/X.

```
FrameMap syscall
  → Kernel::map_frame()   kiểm rights/W^X/PA/VA/overlap
  → map_page_table()      walk generic, user không truyền level
  → PageTableOps::write_pte + flush_va
  → (nếu EXEC) sync_icache
```

Thiếu bảng trung gian ⇒ `MissingPageTable`; **kernel không tự cấp phát page table**.
Đây là quyết định có chủ ý (đúng kiểu seL4): user phải `PageTableMap` trước.

### 7.2 Ba đường dựng không gian địa chỉ [CODE]

| Hàm | Dùng cho | Hành vi |
|---|---|---|
| `build_task_space(untyped, image, layout)` | root/bootstrap user.S | retype VSpace, gắn PageTable còn thiếu, copy ảnh vào Frame, map code RX + `stack_pages` RW; lỗi ⇒ rollback object + bump/objects |
| `build_task_space_shared_code(untyped, code_frame, layout)` | server | map **cùng** Frame code của root vào VSpace server (1 trang code ở bring-up hiện tại) |
| `build_task_space_elf(untyped, vspace, image, bytes, stack)` | root ELF, `/bin/vf` | map từng `PT_LOAD` theo trang: R→R+NX, RX→RX, RW→RW+NX; copy đúng `filesz` tại `vaddr % 4096`, BSS zero; stack 4 trang + guard không map; rollback PTE khi lỗi |

Layout cụ thể (xem §9.4): root VSpace dùng `stack_pages = 0` rồi runtime map rời
4 stack page `+0x5000/+0x6000/+0x7000`; trang `+0x2000` dành riêng cho server và
**cố ý không map trong root VSpace** để test isolation fault được.

### 7.3 VSpace, Mapping, W^X theo Frame [CODE]

```rust
pub struct VSpace {
    pub root: u64,               // handle mờ: satp root / TTBR0 / CR3
    pub aux_pa: Option<u64>,     // PDPT/L1 riêng (x86/AArch64)
    pub page_size: u64,
    pub base: u64, pub next_va: u64, pub end: u64,
    pub mappings: [Option<Mapping>; 64],
}
pub struct Mapping { pub va: u64, pub pa: u64, pub len: u64,
                     pub rights: Rights, pub frame: ObjRef }

pub struct Frame { pub pa: usize, pub size: usize, pub mapped_w: u16, pub mapped_x: u16 }
```

**W^X là bất biến theo Frame, không theo mapping.** `map_frame` từ chối `WRITE`
nếu Frame đã có leaf `EXEC` ở **bất kỳ** VA/VSpace, và ngược lại. Vì vậy hai alias
của cùng một Frame không thể tạo ra trang vừa ghi vừa chạy. `FrameUnmap`, revoke
VSpace/PageTable và destroy Frame đều giảm bộ đếm; `check_invariants()` đối chiếu
bộ đếm với reverse map.

Kiểm tra của `map_frame`: rights chỉ gồm `READ|WRITE|EXEC`; không W+X trên một
leaf; PA căn trang; Frame đủ một page; VA căn trang; VA trong `[base, end)`; không
overlap mapping cũ; **mọi mapping phải có `READ`** trên cả 3 kiến trúc.

`PageTableMap(page_table, vspace, va)`: kernel walk từ root tới entry invalid đầu
tiên trên đường tới `va`; **tự chọn level**; kiểm toàn bộ `table_span(level)` nằm
trong `user_va_range` (không chỉ `va`); ghi đúng **một** PTE cha; flush; lưu
`{vspace, va_base, level}`. Gắn lần hai ⇒ `AlreadyMapped`.

### 7.4 Ba cổng MMU — bảng so sánh [CODE]

| Thành phần | RISC-V64 | AArch64 | x86_64 (UEFI) |
|---|---|---|---|
| Chế độ | Sv39 (3 cấp, 4 KiB) | 4-level, 4 KiB | PML4/PDPT/PD, 4 KiB |
| Bảng tĩnh | `ROOT` tĩnh giữ kernel slots | bảng kernel-only tĩnh | tự dựng PML4/PDPT/PD identity |
| Identity map | `0x8000_0000..0x803f_ffff` (kernel image) | RAM `0x4000_0000..` ở L1[1]; GICv2 `0x0800_0000` + PL011 `0x0900_0000` ở L1[0] | `0..1 GiB` cho kernel/UEFI/stack/GDT/TSS/IDT |
| Kernel accessible | ~6 MiB: `0x8000_0000..0x8060_0000` (nới ở vòng 158; tài liệu cũ ghi 4 MiB) | theo identity map | 0..1 GiB |
| Code user | `R\|X\|U` | EL0 RO + X (`AP_USER_RO=0b11`, UXN=0) | `P\|US`, RX, NX clear |
| Stack/data | `R\|W\|U`, NX | EL0 RW + NX (`AP_USER_RW=0b01`, UXN=1) | `P\|W\|US\|NX` |
| Flush | `sfence.vma` | `tlbi vae1is` + `dsb ish` + `isb` (đổi root: `vmalle1is`) | `invlpg` (đổi root: ghi CR3) |
| Cô lập user | `SUM=0`, bật tạm khi copy | PAN (nếu có), clear tạm khi copy | SMEP/SMAP + STAC/CLAC |
| Trap stack | `sscratch` (kernel trap stack) | `SP_EL1` | `TSS.RSP0` |
| Ghi chú | `sscratch=0` khi ở kernel để phát hiện trap từ S-mode | `KERNEL_SHARED_SLOTS = [0,1]` | PDPT[0] là shared slot; PML4[0]/PDPT private mỗi VSpace |

Ba port dùng **chung** layout user: `CODE_VA = 0x10_0000_0000` (64 GiB).

### 7.5 TLB, shootdown và hai lỗ hổng đã biết [CODE + DOC]

Đã có: maintenance IPI (`vifast-maint`) + per-VA flush; RISC-V/IPI + `sfence.vma`;
AArch64 broadcast (`tlbi ...is`); x86 `invlpg` + flush toàn bộ khi đổi CR3.

**Lỗ hổng 1 — x86 shootdown AP→BSP.** Hook `poll_maintenance` chỉ chạy cho hart 1,
nên yêu cầu maintenance gửi **tới BSP** chờ hết 5M vòng rồi bỏ ⇒ TLB của BSP có
thể cũ. `flush_all_user` (r299) cố sửa nhưng chưa trọn. Đây là lỗi đúng đắn
(correctness), không phải hiệu năng.

**Lỗ hổng 2 — PCID.** `CR3` đã đúng bits 11:0, nhưng `flush_tagged_va` cho ASID
không active dùng `reload_cr3()`; với `CR4.PCIDE = 1` điều này **không** xoá PCID
khác (chỉ lộ trên KVM hoặc `-cpu max`, không lộ trên TCG mặc định).

Chưa có: IPI shootdown đồng bộ có chờ ACK cho **mọi** đường unmap; ASID/PCID
allocator; flush lan sang hart khác sau `sync_icache` khi có SMP.

### 7.6 Hợp đồng "cửa sổ copy user" — bài học đắt [CODE]

Mọi syscall copy giữa kernel và user (`Read`, `Write`, `DebugWrite`, `FrameWrite`,
IPC buffer, `exec` đọc ELF/argv) phải kiểm con trỏ **trước khi** dereference.
Dự án dùng **so cận với danh sách cửa sổ cố định** (nhanh), và **walk page table**
(chính xác) cho đường map/frame.

```rust
pub const USER_COPY_MAX: usize = 256;   // trần một lần copy
pub const COPY_WINDOWS: &[(usize, usize)] = &[
    (CODE_VA, 0x2_0000),                              // text+rodata+stack bootstrap
    (STACK_BASE, STACK_LEN),                          // stack thường 0x8000
    (EXEC_STACK_BASE, EXEC_STACK_TOP - EXEC_STACK_BASE), // stack của task exec
    (DATA_VA, 4096),                                  // data page
    (ROOT_MODULE_SERVER_VA, ROOT_MODULE_END - ROOT_MODULE_SERVER_VA), // staging module
    (IPC_BUF_SERVER_VA, 4096),
    (IPC_BUF_CLIENT_VA, 4096),
    (ROOT_ELF_BASE, ROOT_ELF_STACK_TOP - ROOT_ELF_BASE), // root ELF + stack
];
```

Bất biến: (C1) `[ptr, ptr+len)` phải nằm **trọn trong một** cửa sổ — `len == 0`,
tràn số, hoặc vắt qua hai cửa sổ đều sai; (C2) **cả ba port kiểm cùng danh sách**,
không port nào có danh sách riêng.

**Vì sao phải nhấn mạnh:** lỗi "exec không đọc được trang stack" tồn tại từ vòng
112 đến 145 chỉ vì `read_user`/`write_user` **thiếu một cửa sổ VA** — bảng trang
hoàn toàn đúng. Đây là ví dụ mẫu của loại lỗi "kiến trúc đúng, hợp đồng sai".

### 7.7 Giới hạn đã biết của tầng bộ nhớ

- Không có allocator page-table page tổng quát (user phải `PageTableMap`).
- Không có allocator dải VA nhiều vùng: mỗi VSpace chỉ có `[base, end)` và `next_va`.
- Không có "unmap toàn bộ VSpace" (destroy VSpace vẫn phải đi qua mapping).
- Tối đa **64 mapping/VSpace** — trần này va chạm khi thêm task demo mới.
- Không có frame reclaim policy: frame trả về Untyped khi destroy, không có LRU/evict
  (pager demo làm evict thủ công).
- Kernel address space của RISC-V chỉ ~6 MiB ⇒ kernel image + metadata/data Untyped
  phải nằm trọn; đây là lý do `KERNEL_STATIC_BUDGET` là ràng buộc cứng.

---

## 8. Lưu trữ: `vifast-fs` (VF04) và VFS server ở user space

Nguồn: `crates/vifast-fs/src/lib.rs`, `crates/vifast-server/src/main.rs`,
`crates/vifast-client/src/main.rs`, `crates/vifast-abi/src/lib.rs`.

### 8.1 Hai thứ khác nhau cùng bị gọi là "FS" — đừng lẫn

| | **Kernel `ramfs`** (`crates/vifast-kernel/src/fs.rs`) | **`vifast-fs` + VFS server** |
|---|---|---|
| Ở đâu | Trong kernel | User space (server giữ cap `BlockDevice`) |
| Dùng cho | Demo boot: marker `ViFastOs ramfs`, `ViFastOs write` | Hệ FS thật của hệ thống |
| Giới hạn | 8 file, tên 24 B, file 512 B, path 64 B, fd 8 | 64 inode, ảnh 4 MiB, tên 28 B, path 128 B |
| Đặc điểm | Không thư mục, không phân quyền | Có thư mục, link, journal, fsck, cache |

**Đây là nguồn gây hiểu nhầm lớn nhất trong tài liệu:** `docs/11-hien-tai/05-vfs.md`
mô tả **ramfs** như thể nó là VFS của hệ thống, và liệt kê "chưa có: thư mục,
quyền, mount, symlink…" — tất cả những thứ đó **đã có** trong `vifast-fs`.

### 8.2 Hình học đĩa VF04 [CODE]

```text
BLOCK_SIZE            = 512
ảnh mặc định          = 4 MiB  (8192 block)
MAX_INODES            = 64
MAX_NAME              = 28 byte
MAX_PATH              = 128 byte
VF04: 10 direct + 1 single-indirect + 1 double-indirect
MAX_SYMLINK_DEPTH     = 8
MAX_TX_WRITES         = 16        // số ghi trong một transaction journal
```

Superblock mang magic `VF04` + version 4. `VF03` (11 direct + single indirect) và
`VF02` vẫn **mount được** (đường migration), có test persistence riêng.

### 8.3 Tính năng đã có và đã có test [CODE]

Journal write-ahead + replay (kể cả **torn commit**), thư mục, hard link,
symlink (giới hạn độ sâu), double-indirect, `O_APPEND`, `O_EXCL`, `O_DIRECTORY`,
`STAT_PATH`, `rename` (chống chu trình), `truncate` (zero-fill khi mở rộng, trả
block khi cắt), `unlink`, `usage()` (nguồn số của `df`), quyền theo **uid**
(uid khác chỉ đọc), block cache **write-back** + `fsync`/`BlockFlush`,
`fsck` (có test chứng minh bắt được hỏng, không chỉ "trả true").

### 8.4 Giao thức RPC của VFS server [CODE]

- Client → server: `Call(endpoint)`; **label = op (16 bit thấp) | (seq << 32)**;
  3 data word mang tham số (fd, offset, len hoặc con trỏ buffer); payload đi qua
  **IPC buffer** dùng chung (tối đa `MAX_IO = 128` byte).
- Server → client: `Reply`; 1 label + 3 data word. Vị trí word mang `echo =
  (seq << 32) | hash` **phụ thuộc op**: `OPEN/STAT/STAT_PATH` dùng nửa cao của
  `d1` (nửa thấp giữ nghĩa cũ: size/ino); các op còn lại dùng `d2`.
- Op chẩn đoán `RINGDUMP = 0x2014`: xin server in 16 bản ghi cuối của ring request.
  Chỉ client gọi khi đã thấy một đèn `V* BAD` đầu tiên.
- `MAX_IO = 128` **là biện pháp thu hẹp cửa sổ race, không phải sửa gốc** —
  comment trong `vifast-abi` ghi rõ: race nằm ở đường FS/cache khi có ≥ 2 hart,
  `-smp 1` thì 8/8 xanh; muốn nâng lại 256 phải có test tái hiện + sửa gốc.

### 8.5 Lệnh shell gọi tới VFS [CODE, bảng sinh tự động từ mã]

`cat`, `ls`, `stat`, `mkdir`, `rm`, `mv`, `ln`, `truncate`, `sync`, `fsck`,
`run`, `runfs`, `exec`, `kill`, `loop`, `time`, `help`, `history`, `!!`, `pwd`,
`cd`, `echo` (một số lệnh thuần client, không IPC). `/bin/vf` là **một ảnh
multicall**: `uname`, `uptime`, `free`, `ps`, `df`, `jobs`, `args` — mode chọn
theo `argv[0]`, chứng minh ABI argv hoạt động (marker `BIN OK uname`…).

### 8.6 T26 — vi phạm read-your-own-write ở mức entry thư mục [ĐO]

Đây là lỗi mở quan trọng thứ hai, và là ví dụ tốt về *cách đo*.

**Triệu chứng.** Ở `MAX_IO = 256`, `smp2`, tỉ lệ boot đỏ **10/50 = 20 %**; boot
xanh in đủ marker. `-smp 1` = 0/3 (mẫu nhỏ) và **`-smp 1` không thoát được boot**
(phải hạ timeout khi đo). `-accel tcg,thread=single` vẫn **1/3 đỏ** ⇒ đây là
**race logic**, không phải race bộ nhớ phía host.

**Bằng chứng cơ chế (do agent2 đo):**
- Ring của **chính server** ghi `srv=111 req=50 op=8199 idx=6 res=1`, khớp đúng
  `VEND seq=50 idx=6` của client ⇒ **ENOENT do FS tạo**, không phải reply ghép lệch.
- `V2MK open=1` (mkdir xong, `open("/d")` thấy) **+** `V2CHK open=1 rd=2` (hai lần
  liệt kê sau đều thấy `d`) ⇒ entry **có thật**; chỉ **một** lần liệt kê ở giữa
  không thấy.
- Đèn `VSEQ=VARG=VHASH=0` trên toàn bộ các lượt ⇒ đã bác "reply cũ", "tham số cũ",
  "payload bị đè".
- Lặp **61.000 vòng trong một boot, 0 miss** ⇒ lỗi **không** tái hiện bằng lặp
  đơn thuần; cần **hoạt động đồng thời của root lúc boot**.

**Kết luận kỹ thuật:** vi phạm *read-your-own-write* ở mức **directory entry** —
ghi entry xong, đọc lại (qua đường cache khác) không thấy, nhưng lần đọc sau thì thấy.

**Việc chưa làm (đã có thiết kế đo):** đặt đèn `WRITE_THEN_READ` **bên trong**
`write_dir_entry`/`remove_dir_entry` của `vifast-fs`: ngay sau khi ghi entry, đọc
lại **RAW** (`read_uncached`) và so, **kèm `seq`** trong bản ghi. Lý do dùng RAW:
cache có thể che đúng/sai; RAW là nhân chứng độc lập. Lý do đặt **bên trong**:
đèn "lần gần nhất" ở client không có `seq` nên không đủ để khẳng định hai lần đọc
cùng thời điểm (bài học đã ghi).

---

## 9. Runtime, boot, chương trình user

Nguồn: `crates/vifast-runtime/src/lib.rs`, `crates/vifast-abi/src/lib.rs`
(`user_layout`, `BootInfo`), `crates/vifast-root/src/main.rs`,
`crates/vifast-server/src/main.rs`, `crates/vifast-client/src/main.rs`,
`crates/vifast-bin/src/main.rs`.

### 9.1 Trình tự boot [CODE]

```text
1.  A::init()                          console + clock
2.  A::irq_disable()
3.  A::init_platform(info)             MMU/SMP/IRQ tối thiểu, static pools
4.  banner "ViFastOs booting on <arch> (hart=, dtb=)"
5.  kernel = A::init_kernel()          → Kernel::reset()   (LẦN 1 — cho selftest)
6.  selftest::run(kernel)              → in SELFTEST PASS (ipc_tx=… ticks=…)
7.  kernel = A::init_kernel()          → Kernel::reset()   (LẦN 2 — kernel thật)
8.  set_page_table_ops / set_timer_hz
9.  guard: size_of::<Kernel>() <= KERNEL_STATIC_BUDGET  (nếu vượt: exit rõ ràng)
10. A::set_kernel(ptr)                 công bố kernel TRƯỚC khi AP online
11. A::start_smp()                     HSM (RISC-V) / PSCI (ARM) / MADT+LAPIC (x86)
12. root TCB priority 15 (trên task thường để fault được phục vụ sớm)
13. tạo Untyped: boot (giới hạn 40 object), metadata (34 object), data (86 object)
    — vùng nhớ lấy từ `boot_page()`/`metadata_page()`/`data_page()` của platform
14. cài metadata/data Untyped vào slot 2/3
15. build_task_space: root VSpace (code user.S RX, 3 stack page fault1/2/3)
16. parse root ELF (cửa sổ 1 MiB) + build_task_space_elf(Some(root)) → root ELF + stack 4 trang + guard
17. dựng server VSpace từ boot Untyped (code dùng chung Frame root, stack +0x2000) → slot 12
18. check_kernel_mappings(root, server); log BOOT/DATA Untyped
19. retype BootInfo page, ghi nội dung, map READ-ONLY tại ROOT_BOOTINFO_VA
20. A::prepare_user()                  x86: GDT/TSS/IDT/SMEP/SMAP/PCID/SSE-off
21. tạo context root từ entry ELF + ROOT_ELF_STACK_TOP; a0 = ROOT_BOOTINFO_VA
22. enqueue root → pick() → ensure_current_root() → "ROOT SWITCH 1"
23. A::start_timer()
24. A::enter_user(root)                sret / eret / iretq
```

**Chi tiết quan trọng cho người săn lỗi:**
- `A::init_kernel()` được gọi **hai lần** (bước 5 và 7), mỗi lần gọi `kernel.reset()`.
  `reset()` xoá `system_ticks` về 0 (trước mẻ r323) — đây chính là cơ chế bị nghi
  trong T33a (xem §15.1).
- `set_kernel` phải xong **trước** `start_smp`: đường boot của AP gọi
  `Platform::kernel()` ngay khi được bring-up.
- `KERNEL_STATIC_BUDGET = 566_272` và guard runtime tồn tại vì một vụ thật: kernel
  pool 743 KB tràn qua `BKL`/`KERNEL_PTR` trong `.data`, `reset()` zero con trỏ
  kernel ⇒ AArch64 DABT `FAR=0xb4410` sau `BOOTINFO OK` (vòng 30). Thông báo lỗi
  giờ là `KERNEL STATIC OVERFLOW size=… budget=…`, không còn exception mù.

### 9.2 `BootInfo` v3 — hợp đồng giữa runtime và root [CODE]

```rust
pub const BOOTINFO_MAGIC: u64 = u64::from_le_bytes(*b"VFBOOT01");
pub const BOOTINFO_VERSION: u32 = 3;

#[repr(C)] pub struct BootInfo {
    magic: u64, version: u32, size: u32,
    root_vspace: u16, meta_untyped: u16, data_untyped: u16, server_vspace: u16,
    user_code_first: u16, user_code_count: u16,
    free_slot_start: u16, free_slot_end: u16,   // [start, end)
    block_device: u16,                          // slot chứa cap BlockDevice
    server_elf_va: u64, server_elf_len: u64,    // staging module ELF (READ-ONLY)
    client_elf_va: u64, client_elf_len: u64,
    sched_harts: u32,                           // bitmask hart được chạy task user
}
```

Root kiểm `magic/version/size` rồi in `BOOTINFO OK`; **mọi slot root dùng đều đọc
từ đây**, không hard-code. Cap của code user.S là `READ|EXEC` (không W, không
GRANT). Root cấp object mới tuần tự từ `free_slot_start`.

**Lưu ý tài liệu:** `docs/11-hien-tai/01-abi.md` chỉ liệt kê 12 trường (bản v2)
⇒ thiếu `block_device`, `server_elf_va/len`, `client_elf_va/len`, `sched_harts`.

### 9.3 Layout VA người dùng (hằng số thật) [CODE]

```text
CODE_VA                  = 0x0000_0010_0000_0000      (64 GiB)
DATA_VA                  = CODE_VA + 0xa000
STACK_BASE = +0x1000, STACK_LEN = 0x8000  (cửa sổ stack bootstrap; các stack riêng bên dưới
                                            nằm trong cùng dải VA nhưng ở VSpace khác)
SERVER_STACK             = CODE_VA + 0x2000 .. 0x5000  (cố ý KHÔNG map trong root VSpace)
CLIENT_STACK             = CODE_VA + 0x8000 .. 0x9000
FAULT1/2/3_STACK         = +0x5000/+0x6000/+0x7000     (mỗi cái 4 KiB)
FAULT4_STACK             = +0x9000 .. 0xa000
IPC_BUF_SERVER_VA        = +0xb000     IPC_BUF_CLIENT_VA = +0xc000
WORKER_A..E, IDLE, CNODE, PAGER, PAGER_SERVER, PAGER2, PAGER3, WORKER_G/H, EXEC_STACK
                         = +0xd000 .. +0x24000  (mỗi vùng 4 KiB, có const assert chống chồng lấn)
ROOT_ELF_BASE            = CODE_VA + 0x20_0000
ROOT_ELF_STACK_GUARD     = ROOT_ELF_BASE + 0x10_0000   (1 trang KHÔNG map)
ROOT_ELF_STACK_BASE/TOP  = guard + 0x1000  .. + 0x4000  (4 trang)
ROOT_BOOTINFO_VA         = ROOT_ELF_STACK_TOP + 0x1000 (READ-ONLY)
ROOT_MODULE_SERVER_VA    = BOOTINFO + 0x1000           (staging 128 KiB)
ROOT_MODULE_CLIENT_VA    = +0x2_0000                   (staging 128 KiB)
ROOT_MODULE_END          = +0x4_0000
```

Module ELF (server/client/`/bin/vf`) được **link** tại `0x1000010000`
(`--defsym=ROOT_ELF_BASE=0x1000010000` trong `build.sh`, script `module.ld` gộp
`.rodata` vào `.text` để chỉ còn 2 segment = 2 vùng trang), root task link tại
`0x1000200000`. Có `const assert` cho **mọi** cặp vùng liền kề — layout sai là
lỗi biên dịch, không phải lỗi runtime.

Điểm cần thuộc: **root VSpace không map `+0x2000`** (trang server) — đó là cách
test `Isolation` tạo fault thật; và client có VSpace riêng từ r308 nên không ghi
được vào stack của root/server.

### 9.4 Nhiệm vụ của root task [CODE]

Root là Rust ELF (`vifast-root`, ~4.487 dòng) — phần "supervisor/demo" của hệ:

1. kiểm BootInfo, in `BOOTINFO OK`;
2. retype Endpoint, TCB server/client/notification, 4 fault TCB từ metadata Untyped;
3. `CapCopy` endpoint cho server (READ) và client (WRITE); mint notification;
4. dựng **VSpace client** từ data Untyped: code RX (alias Frame code), client stack
   RW, fault4 stack RW → `CLIENT VSPACE OK`;
5. cấp **Untyped con 64 KiB / 8 object** cho client;
6. `TcbConfigure` + `TcbResume` server, client, fault1/2/3 (root VSpace), fault4
   (client VSpace, priority 6, fault_ep slot 6);
7. `Recv(fault_endpoint)` → in `FAULT EP OK addr=…` khi fault4 báo;
8. spawn **worker SMP** a/b/c/g/h (demo chứng minh preempt + migration);
9. chạy **pager server** + 3 pager client (demand paging), demo CNode, destroy-smp,
   exec/`runfs`, kill, loop;
10. in `SMP WORKERS OK (a=50, b=50, c=1)` khi đủ báo cáo worker ⇒ đây là marker mà
    T33a làm mất.

**Worker C** (`smp_worker_c`) là nhân vật chính của T33a: nó busy-spin, đọc
`GetTime` liên tục, `panic!` nếu thời gian lùi, chỉ thoát khi root set
`SUSPEND_FLAG` (root set sau khi nhận `WORKER_LABEL_C_RUNNING`). Mẻ r323 đổi
`panic!` thành in `C TIME BACK last=… now=… start=…` **và không break** (break sớm
làm `elapsed` mất nghĩa ⇒ sinh `TCB SUSPEND BAD`).

### 9.5 Server, client, `/bin/vf` [CODE]

**Server** (`vifast-server`, ~1.235 dòng): in `VFS SERVER OK`, vòng lặp
`Recv(endpoint, recv_slot=4)` → `handle(request)` → `Reply`. Sở hữu cap
BlockDevice (từ BootInfo) và gọi `BlockRead/BlockWrite/BlockFlush`. Có các đèn
nặng chỉ bật tay: `DIRSNAP/DIRNAME/RDD/EVL` — **cố ý tắt mặc định** vì chúng nằm
trong `handle()` làm dài cửa sổ IPC, tức tự bóp méo phép đo (phát hiện của r297).

**Client** (`vifast-client`, ~2.169 dòng): là **shell** + bộ demo. Thứ tự demo:
mở `/motd` đọc in (`ViFastOs ramfs`) → tạo `/scratch`, ghi `ViFastOs write`, seek,
đọc lại → `FrameMap`/`FrameUnmap` → `Call` chuyển cap notification (`Cap OK`) →
`Poll` → `Reply OK` → chạy chuỗi test FS (`FS V2 OK`, `FS FSCK OK`, `FS PERM OK`,
`BADGE SPOOF DENIED OK`, `PAGER*`, `KILL OK`, `RUN FS OK`, `HIARG`…) → `SHELL READY`
→ vòng shell đọc lệnh.

**`/bin/vf`** (`vifast-bin`, 267 dòng): multicall, mode theo `argv[0]`:
`uname`, `uptime`, `free`, `ps`, `df`, `jobs`, `args`. Được exec từ FS qua
`runfs` để chứng minh chuỗi *FS → ELF → VSpace → argv → cap*.

### 9.6 `vifast-user` — lớp tiện dụng cho user space [CODE]

Wrapper syscall cho 3 kiến trúc (`send`, `call`, `recv`, `reply`, `yield`,
`wait`, `signal_bits`, `get_time`, `debug_write`, `open/read/write/close`,
`retag`…), cộng helper slot/untyped và layout chung. Đây là "libc mini" của dự án;
chưa có allocator heap, chưa có libc, chưa có dynamic linking.

---

## 10. ABI: syscall, lỗi, quyền, thanh ghi

Nguồn: `crates/vifast-abi/src/lib.rs`.

### 10.1 Thuộc tính [CODE]

`#![no_std]`, `#![forbid(unsafe_code)]`. Mọi kiểu trên biên kernel/user là
`#[repr(C)]`/`#[repr(u32)]`/`#[repr(u64)]`/số nguyên — **không** `String`, `Vec`,
`Option` trong cấu trúc wire.

Hằng số: `DEFAULT_PAGE_SIZE = 4096`, `MSG_MAX_WORDS = 8`, `CAP_NONE = u64::MAX`,
`USER_IO_MAX = 256`, `IPC_BUFFER_WORDS = 32`, `OBJ_ENDPOINT_BYTES = 256`,
`OBJ_NOTIFICATION_BYTES = 128`, `OBJ_TCB_BYTES = 4096` (quota).

### 10.2 Mã lỗi `KError` (22 giá trị) [CODE]

| # | Tên | Khi nào |
|---:|---|---|
| 0 | `Ok` | thành công |
| 1 | `InvalidCap` | slot không tồn tại |
| 2 | `WrongType` | cap trỏ sai loại object |
| 3 | `NoRights` | thiếu quyền (kể cả W^X, chuyển cap) |
| 4 | `SlotOccupied` | slot đã có cap |
| 5 | `OutOfMemory` | hết object/arena/queue |
| 6 | `InvalidArg` | tham số sai |
| 7 | `Alignment` | địa chỉ/PA không căn trang |
| 8 | `AlreadyMapped` | VA đã map |
| 9 | `NotMapped` | VA chưa map |
| 10 | `NoCaller` | Reply khi không có client |
| 11 | `ObjectDestroyed` | `ObjRef` cũ / object đã mất |
| 12 | `IrqInUse` | dự phòng |
| 13 | `PrioTooHigh` | dự phòng |
| 14 | `Cancelled` | dự phòng |
| 15 | `MissingPageTable` | thiếu page table trung gian |
| 16 | `IllegalOperation` | bất biến/trạng thái sai |
| 17 | `NotImplemented` | chưa hỗ trợ |
| 18 | `WouldBlock` | server đang giữ caller khác |
| 19 | `NotFound` | không tìm thấy file |
| 20 | `Busy` | object còn mapping/queue/binding/TCB |
| 21 | `HasDependents` | còn cap khác trỏ tới object |

**Cẩn thận khi chẩn đoán:** `NoRights` có **hai** nghĩa (quyền cap **và** W^X/tham
số) — đừng kết luận chỉ từ mã lỗi (bài học vòng 5x-2). Tương tự `OutOfMemory` có
thể là hết object slot, hết byte Untyped, hoặc hàng đợi đầy.

### 10.3 Quyền, object type, thread state [CODE]

`Rights`: `READ 0x1 | WRITE 0x2 | EXEC 0x4 | GRANT 0x8`.
`ObjType`: `Untyped, Tcb, Endpoint, Notification, CNode, Frame, VSpace, PageTable,
SchedContext, Iommu, Irq, BlockDevice`.
`ThreadState`: `Inactive, Ready, Running, BlockedSend, BlockedRecv, BlockedReply,
BlockedNotification, BlockedFault, BlockedSleep, BlockedBudget, Faulted` (enum thật
nhiều hơn 8 trạng thái mà tài liệu cũ liệt kê).

### 10.4 Message IPC [CODE]

```rust
#[repr(C)] pub struct Message {
    pub label: u64,
    pub len: u8, pub _pad: [u8; 7],
    pub cap: u64,          // slot gửi, hoặc CAP_NONE
    pub data: [u64; 8],
}
```

- `Message::new(label, &[words])` kiểm `len <= 8`; `words()` clamp theo `len`.
- **Giới hạn syscall cứng:** `Send`/`Call` mang tối đa **3 data word** (a2..a4) và
  `a5` là slot cap; `Reply` mang **4 data word** (a2..a5) và **không** chuyển cap.
- Receiver phải chỉ định `recv_slot`; kernel không tự chọn slot fallback.
- IPC buffer 32 word/page được copy giữa hai TCB (`TcbSetIpcBuffer`).

**Vì sao giới hạn 3 word lại quan trọng:** chuỗi VFS cần `fd, offset, len` = đúng
3 word ⇒ **không còn thanh ghi trống cho `seq`**. Đó là lý do `seq` phải nhét vào
32 bit cao của **label** (r295). Đây là ví dụ điển hình của việc ABI hẹp tạo ra
thiết kế chẩn đoán phức tạp.

### 10.5 Bảng 58 syscall [CODE]

Đánh số `0..57` **ổn định**; tất cả đều có handler thật (`SYSCALL_STATUS` toàn
`true`, có test kiểm không thiếu số nào).

| # | Tên | Tham số chính | Kết quả / ghi chú |
|---:|---|---|---|
| 0 | `Send` | a0 ep, a1 label, a2..a4 data, a5 cap slot | có thể block |
| 1 | `Call` | như Send | block tới khi server `Reply` |
| 2 | `Recv` | a0 ep, a1 recv slot | message hoặc block |
| 3 | `Reply` | a1 label, a2..a5 data | trả lời caller hiện tại |
| 4 | `ReplyRecv` | a0 ep, a1 slot, a2 label, a3..a5 data | reply rồi recv |
| 5 | `Signal` | a0 notification | set bit, đánh thức waiter |
| 6 | `Wait` | a0 notification | word hoặc block |
| 7 | `Poll` | a0 notification | word + clear |
| 8 | `Yield` | — | nhường CPU nếu có TCB Ready |
| 9 | `Retype` | a0 Untyped, a1 ObjType, a2 dest; con: a3 size, a4 max_objects | tạo object/con Untyped |
| 10 | `CapCopy` | a0 src, a1 TCB, a2 dest, a3 rights | copy cap sang TCB khác |
| 11 | `CapMint` | a0 src, a1 TCB, a2 dest, a3 rights, a4 badge | mint badge (Endpoint/Notification) |
| 12 | `CNodeMint` | a0 src, a1 CNode, a2 index, a3 rights, a4 badge | mint thẳng vào CNode |
| 13 | `CapDelete` | a0 slot | xoá cap lá; từ chối nếu còn phụ thuộc |
| 14 | `CapRevoke` | a0 slot | revoke cây cap; Untyped ⇒ destroy object con |
| 15 | `TcbConfigure` | a0 TCB, a1 entry, a2 stack, a3 VSpace, a4 prio, a5 fault_ep slot | set context |
| 16 | `TcbResume` | a0 TCB | vào runqueue (chỉ Inactive/Suspended/Ready/BlockedFault) |
| 17 | `TcbSuspend` | a0 TCB | rút khỏi queue/endpoint |
| 18 | `TcbRestart` | a0 TCB | dựng lại context tại entry/stack |
| 19 | `TcbSetPriority` | a0 TCB, a1 prio | chỉ hạ/giữ trong prio caller |
| 20 | `TcbSetAffinity` | a0 TCB, a1 hart mask | requeue nếu đang queued |
| 21 | `TcbBindNotification` | a0 TCB, a1 notification | bind (mỗi notification 1 TCB) |
| 22 | `SchedContextConfigure` | a0 ctx, a1 budget, a2 period | 1 ≤ budget ≤ period |
| 23 | `SchedContextBind` | a0 TCB, a1 ctx | 1 context ↔ 1 TCB |
| 24 | `PageTableMap` | a0 PageTable, a1 VSpace, a2 VA | kernel chọn level; trả level |
| 25 | `FrameMap` | a0 VSpace, a1 Frame, a2 VA, a3 rights | rights phải có READ; W^X theo Frame |
| 26 | `FrameUnmap` | a0 VSpace, a1 VA, a2 Frame | unmap + flush |
| 27 | `IoMap` | a0 Iommu, a1 Frame, a2 IOVA, a3 rights | READ/WRITE, không EXEC |
| 28 | `IoUnmap` | a0 Iommu, a1 IOVA, a2 frame slot | đòi vé Frame khớp (K74) |
| 29 | `IrqGet` | a0 logical IRQ, a1 dest slot | root-only |
| 30 | `IrqSetNotification` | a0 IRQ, a1 notification | signal nếu đang pending |
| 31 | `IrqAck` | a0 IRQ | xoá pending |
| 32 | `DebugWrite` | a0 ptr, a1 len | in console (qua `validate_user_range`) |
| 33 | `GetTime` | — | **thời gian toàn hệ** (`system_ticks`, max-update mọi hart) |
| 34 | `Exit` | — | thoát QEMU |
| 35 | `Open` | a0 path ptr, a1 len, a2 flags | trả fd |
| 36 | `Read` | a0 fd, a1 buf, a2 len | trả số byte |
| 37 | `Write` | a0 fd, a1 buf, a2 len | trả số byte |
| 38 | `Close` | a0 fd | |
| 39 | `Stat` | a0 fd | trả kích thước |
| 40 | `Seek` | a0 fd, a1 offset | trả offset mới |
| 41 | `TcbSetIpcBuffer` | a0 TCB, a1 Frame, a2 VA | gắn IPC buffer |
| 42 | `FrameWrite` | a0 Frame, a1 offset, a2 ptr, a3 len | ghi qua PA (kiểm `mapped_x`, K57/K72) |
| 43 | `BlockRead` | a0 LBA, a1 buffer | 512 byte |
| 44 | `BlockWrite` | a0 LBA, a1 buffer | 512 byte |
| 45 | `BlockFlush` | — | flush cache thiết bị |
| 46 | `Sleep` | a0 deadline tick | `BlockedSleep` |
| 47 | `ConsoleRead` | a0 buffer, a1 len | đọc input IRQ-driven |
| 48 | `CNodeInstall` | a0 TCB, a1 CNode (0 = bỏ) | chỉ TCB Inactive/Suspended/Faulted |
| 49 | `TcbDestroy` | a0 TCB | reparent cspace rồi destroy; `WouldBlock` nếu có waiter |
| 50 | `ObjectDestroy` | a0 cap slot gốc | 7 loại; trả bộ nhớ về Untyped |
| 51 | `TcbWait` | a0 TCB | block tới khi target Exited |
| 52 | `TcbInfo` | a0 TCB | state/prio/hart/PC cho `ps`/`jobs` |
| 53 | `MemInfo` | — | untyped/đã dùng/còn lại cho `free` |
| 54 | `TimerInfo` | — | tick + tần số cho `uptime` |
| 55 | `ModuleRelease` | a0 module cap | trả trang staging module về Untyped |
| 56 | `TcbSetArgs` | a0 TCB, a1 argv | ghi `argc/argv` vào khối 256 B trên stack con |
| 57 | `TcbGrantCap` | a0 TCB, a1 cap nguồn, a2 slot đích | uỷ quyền cap cho tiến trình con (đường exec) |

**Lưu ý:** `docs/11-hien-tai/01-abi.md` có câu "Tất cả 52 số 0..51 đều có handler"
— **sai**, phải là 58 số 0..57 (bảng ngay trên câu đó liệt kê 58). Đây là ví dụ
doc drift trong cùng một tệp.

### 10.6 Quy ước thanh ghi [CODE]

| | Số hiệu syscall | Tham số | Kết quả | Lệnh |
|---|---|---|---|---|
| RISC-V64 | `a7` | `a0..a5` | `a0=error, a1=value, a2=label, a3..a6=data` | `ecall` |
| AArch64 | `x8` | `x0..x5` | `x0=error, x1=value, x2=label, x3..x6=data` | `svc #0` |
| x86_64 | `rax` | `rdi, rsi, rdx, rcx, r8, r9` | `rax=error, rdi=value, rsi=label, rdx, rcx, r8, r9=data` | `int 0x80` |

### 10.7 Quy tắc con trỏ user [CODE]

- Kernel **không** đọc trực tiếp con trỏ user.
- Runtime gọi `Platform::read_user`/`write_user` (kiểm bằng `COPY_WINDOWS`, §7.6).
- `DebugWrite`, `Open`, `Write` đọc từ code/data page và các cửa sổ đã khai báo;
  `Read` chỉ ghi vào data page.
- Đường cần chính xác (map/frame/khởi tạo user) đi qua `validate_user_range()`
  walk page table thật: leaf phải non-kernel, có `READ`, và `WRITE` nếu cần ghi.

### 10.8 Giới hạn ABI hiện tại (đừng giả định là có)

- Chưa có encode/decode message đầy đủ cho shared memory / IPC buffer dạng chuẩn
  (seL4 style); message đi qua struct + thanh ghi.
- `Reply` **không** mang capability.
- Chưa có quyền file kiểu POSIX đầy đủ (có uid ở FS, không có uid/gid trong IPC).
- Chưa có `Irq` object hoàn chỉnh cho user (chỉ root `IrqGet`).
- FP/SIMD của EL0 **bị tắt có chủ ý** trên cả 3 port (xem §11.6) — chương trình
  dùng FP/SSE sẽ **trap**, không chạy sai. Muốn bật phải làm **đồng thời** vùng lưu
  trong TrapFrame **và** bật FS/FPEN/SSE; làm lẻ một trong hai là lỗi bảo mật.

---

## 11. Driver và thiết bị

Nguồn: `crates/vifast-virtio/src/lib.rs`, `crates/vifast-platform-*/src/platform.rs`.

### 11.1 VirtIO-MMIO v2 (chỉ RISC-V & AArch64) [CODE]

Không PCI, không MSI: mọi thiết bị được **thăm dò bằng magic** `0x74726976`
("virt") trong cửa sổ MMIO của từng máy.

| Arch | Cửa sổ | Bước |
|---|---|---|
| riscv64 (`virt`) | `0x1000_1000 .. 0x1000_9000` | `0x1000` |
| aarch64 (`virt`) | `0x0a00_0000 .. 0x0a00_4000` | `0x200` |

Thiết bị: block `id=2`, net `id=1`, GPU `id=16`, input `id=18`, RNG `id=4`.

**Trình tự khởi tạo (đã trả giá bằng lỗi thật):**
1. `Status = 0` (reset) → `1` (ACK) → `3` (ACK|DRIVER).
2. Thoả thuận feature: chọn word bằng `DeviceFeaturesSel (0x14)` **rồi mới** đọc
   `DeviceFeatures (0x10)`; ghi `DriverFeatures (0x20)` sau khi chọn
   `DriverFeaturesSel (0x24)` (bit 0 = `VIRTIO_F_VERSION_1`); đọc lại `Status`
   phải có bit 3 (`FEATURES_OK`).
3. Queue: `QueueSel (0x30)`, `QueueNum (0x38)` luỹ thừa 2 ≤ `QueueNumMax (0x34)`,
   `QueueDesc/Driver/Device` ở `0x80/0x90/0xa0` (mỗi cái 2 thanh ghi 32-bit),
   rồi **`QueueReady (0x44) = 1`**.
4. `Status = 15` (DRIVER_OK).
5. Đăng buffer vào avail ring rồi **kick** `QueueNotify (0x50) = index`.
6. Sau mỗi gói dùng xong: đọc `used.idx`, xử lý, **re-post** descriptor và kick lại.

**Ba lỗi từng làm RX "im lặng" suốt nhiều vòng (đã vá):**
- `QueueReady` phải ghi **1**, không phải index queue (ghi 0 cho queue 0 làm RX
  không bao giờ ready).
- Header virtio-net phải **12 byte** khi `version == 2` và đã đàm phán
  `VIRTIO_F_VERSION_1` (QEMU dùng `virtio_net_hdr_mrg_rxbuf`); dùng 10 byte
  (legacy) thì QEMU ăn mất 2 byte đầu khung Ethernet ở TX và ghi lệch buffer ở RX
  ⇒ mọi gói ARP/PING/DNS/TCP đều mất. Vá ở r307.
- Thiếu kick sau khi đăng buffer, và thiếu re-post sau khi tiêu thụ.

Với thiết bị **hai queue** (net, input): cấu hình **cả hai queue trước DRIVER_OK**;
attach queue sau DRIVER_OK từng làm treo QEMU.

### 11.2 Từng thiết bị [CODE]

| Thiết bị | Cấu hình | API / marker |
|---|---|---|
| **Block** | queue 0, 3 descriptor: header (device đọc) + data 512 B + status | `read_sector`/`write_sector`/`flush`; `BLOCK OK`, `FS DISK OK` |
| **Net** | RX queue 0 (header 12 B + buffer 2048 B), TX queue 1 | `net_init_both`, `net_send_arp_to`, `net_ping` (ICMP + checksum RFC 1071), `net_send_udp`, `net_recv_udp`, `net_recv_icmp`, `net_recv_dns_a`; `VIRTIO NET PING REPLY`, `VIRTIO NET DNS sent` (cứng), `VIRTIO NET DNS A` (best-effort) |
| **GPU** | control queue 0; header 24 B + payload; response `0x1100` OK / `0x1101` display info | `GET_DISPLAY_INFO → RESOURCE_CREATE_2D → ATTACH_BACKING → SET_SCANOUT → TRANSFER_TO_HOST_2D → FLUSH`; framebuffer **160×120**; font 5×7 vẽ "ViFastOs"; `VIRTIO GPU DRAW OK` |
| **Input** | event queue 0 (16 descriptor 8 B), status queue 1 | `input_poll()`, `input_key_ascii()` (map mã Linux: Shift/Enter/Backspace/Space); platform poll trong timer IRQ hart 0 → đẩy byte vào `RxQueue` UART + `irq_raise(IRQ_UART)`; `VIRTIO INPUT READY`, `VIRTIO KEY <code>` |
| **RNG** | queue 0, một buffer 64 B | `rng_read(&mut buf)`, tối đa 8 byte/lần; platform đòi 2 mẫu khác nhau; `VIRTIO RNG READY` |

**Kích thước framebuffer 160×120 là quyết định ngân sách**, không phải thẩm mỹ:
framebuffer lớn vượt headroom của kernel RISC-V.

### 11.3 x86_64: không VirtIO [CODE]

- **Đĩa:** ATA PIO (`isa-ide` + `ide-hd` ở `ide.6` trong QEMU), **polling**.
- **Bàn phím:** PS/2 (`i8042`), 6 lỗi từng gặp ở vòng 98.
- **Timer:** PIC/PIT; **ngắt:** LAPIC + IDT; **SMP:** ACPI MADT + LAPIC + trampoline AP.
- Đường boot: OVMF → EFI application → `efi_main(image, system_table)` → runtime boot.
- GDT/TSS/IDT được cài trong `prepare_user()` (sau khi không còn gọi UEFI MP
  Services) để không phá môi trường firmware.

### 11.4 Ngắt: phần lớn vẫn là **polling** [CODE]

Chỉ **timer** và **console input** (UART/PL011/virtio-input qua đường giả lập) là
IRQ thật. Block/net/GPU/RNG **poll** trong luồng gọi. Hệ quả đã biết:
- không có driver bất đồng bộ thật; mọi I/O nằm trên đường syscall hoặc trong
  vòng chờ có bound;
- `sleep`/preemption vẫn hoạt động vì timer là ngắt thật;
- mốc M2 của lộ trình là "driver chạy bằng IRQ" (VirtIO block/net/GPU dùng ngắt +
  notification), có tiêu chí `VIRTIO IRQ OK` trên 3 arch.

### 11.5 Giới hạn thiết bị

- Không PCI/MSI (x86 có quét config space để chẩn đoán, không bind).
- Không hardware IOMMU (VT-d/SMMU): chỉ có **mô hình software** (§7, K55/K56/K73/K74).
- Không hotplug; không driver ngoài QEMU.
- ATA PIO chỉ chạy được trên QEMU `q35` với `ide.6`; máy thật sẽ cần driver khác.

### 11.6 FP/SIMD của user: **tắt có chủ ý** (vòng 185) [CODE]

| Port | Cơ chế tắt | Marker |
|---|---|---|
| riscv64 | `sstatus.FS = 0` trong `USER_SSTATUS` (+ `const assert!` chặn bật) | `EL0 FP TRAP (FS=0, …)` |
| aarch64 | `CPACR_EL1.FPEN = 0b01` | `EL0 FP TRAP (FPEN=0b01, …)` |
| x86_64 | `disable_sse()` trên **từng hart** (CR0.TS) | `SSE USER OFF` |

Lý do: `TrapFrame` của RISC-V chỉ có 32 thanh ghi số + `sepc/sstatus/scause`,
**không có vùng `f0..f31`**; AArch64 tương tự với `q0..q31`. Bật FP khi chưa có
vùng lưu ⇒ hai task dùng FP **ghi đè dữ liệu của nhau**, không có triệu chứng rõ.
Muốn hỗ trợ phải làm **cả hai** việc cùng lúc; làm lẻ là tạo lỗi bảo mật.

---

## 12. SMP và đồng bộ

### 12.1 Cấu hình hiện tại [CODE]

```
MAX_HARTS = 4        // nhưng scheduler mới chỉ chứng minh trên 2 hart
- QEMU: -smp 2 cho cả 3 arch
- per-hart ready queue + work stealing
- BKL: ticket lock (một khoá toàn kernel)
- maintenance IPI: vifast-maint (hộp thư + mask hart)
- hart 0 có idle TCB; hart phụ dùng idle loop của platform
```

### 12.2 Đặc thù từng port [CODE]

| | RISC-V64 | AArch64 | x86_64 |
|---|---|---|---|
| Bring-up hart phụ | SBI **HSM** (`sbi_hart_start`) | **PSCI** `CPU_ON` | **ACPI MADT** + `LAPIC` + **trampoline** AP |
| Đường AP | `smp.S`: stack riêng → `rust_main` | `smp.S` tương tự | real mode → long mode; danh tính hart đi qua ô `0x204` của trang trampoline |
| Ngắt liên hart | SBI IPI / CLINT | GIC SGI | LAPIC ICR |
| Flush TLB liên hart | `sfence.vma` + IPI | `tlbi ...is` **broadcast** | IPI + `invlpg` |
| Marker | `SMP OK (ipi=1)`, `TLB STATS` (yêu cầu `ipi>=1`, `skipped>=1`) | `SMP OK`, yêu cầu TLB `ipi == 0` (broadcast) | `ACPI MADT OK (cpus=2)`, `LAPIC OK`, `AP TRAMPOLINE OK (page < 0xA0000)`, `AP REAL MODE OK` → `AP LONG MODE OK` → `SMP OK`, `SMP IPI COUNT (1 <= seen <= sent)` |
| Exit code khi test | 0 | 0 | **3** (kernel ghi 1 vào `isa-debug-exit`) |

### 12.3 Những gì đã được chứng minh / chưa

**Đã chứng minh [ĐO]:**
- 2 hart chạy được trên cả 3 arch; timer preempt trên hart 1; work stealing;
  maintenance IPI; TLB flush có đếm `ipi/skipped`; `SMP WORKERS OK (a=50, b=50, c=1)`;
  `SMP POST IDLE OK`; `SMP SCHED OK`; `SMP PREEMPT OK`.

**Chưa chứng minh / còn hở:**
- **3–4 hart**: code chuẩn bị cho 4 (`GDTS[MAX_HARTS]`, `CPU_STATES[]`, affinity
  mask), nhưng khởi động hart 2–3 cần **fan-out IPI cho `AP_MAINT_*`** — chưa làm.
  Stress `-smp 4` mới chạy thủ công, không nằm trong harness.
- **x86 shootdown AP→BSP**: `poll_maintenance` chỉ chạy cho hart 1 (§7.5).
- **PCID**: xem §7.5.
- **BKL**: chưa có khoá mịn; mọi syscall bị tuần tự hoá giữa các hart. Đây là
  quyết định có chủ ý ("BKL trước, khoá mịn sau"), seL4 cũng từng dùng.
- **IRQ routing**: UART IRQ hiện chỉ route BSP (hart 0).

### 12.4 Bài học SMP đã trả giá [ĐO]

- Hart index hard-code (`TSSS[1]`, `CPU_STATES[1]`, `pick_on(1)`) từng là P1; đã
  sửa ở r261 thành `GDTS[MAX_HARTS]` + `setup_hart_gdt_tss_idt(hart)` cho cả BSP & AP.
- Ba `assert` trong `ap_entry` từng **vacuous** (đọc lại giá trị vừa ghi ⇒ không
  thể fail); nay kiểm APIC ID đọc từ LAPIC phải khớp `AP_APIC_ID`, `GS_BASE` phải
  nằm trong `CPU_STATES`.
- `AP_GDT` từng không nằm trong `kernel_data_ranges` ⇒ chạy được dưới NX; đã gộp
  vào `GDTS`/`GDT_PTRS` và thêm vào danh sách data.
- Idle-entry race trên x86 (`AP_IDLE_WINDOW`) từng làm `AP PIC IRQ` — đã sửa
  bằng `post_idle_ready`.

---

## 13. Hệ thống đo lường: hợp đồng marker, cổng, soak

Đây là phần **quan trọng nhất để hiểu vì sao vòng lặp hiện tại chậm** (§16).
Nguồn: `scripts/harness_spec.py`, `scripts/test-qemu.py`,
`scripts/test-harness-spec.py`, `scripts/build.sh`, `scripts/gate.sh`,
`scripts/check-all.sh`, `scripts/par-gate.sh`, `scripts/test-persist.py`,
`scripts/test-floor.py`, `scripts/diag.sh`.

### 13.1 Hợp đồng marker — nguồn sự thật duy nhất về "boot đúng" [CODE]

`harness_spec.py` khai báo:

| Nhóm | Số điều kiện | Nghĩa |
|---|---:|---|
| `COMMON` | **55** | Phải xuất hiện trên **mọi** arch |
| `EXACT_ONE` | **29** | Phải xuất hiện **đúng một lần** (không lặp, không thiếu) |
| `ARCH` | **3** | Riêng từng arch (RISC-V/AArch64/x86_64) |
| `FORBIDDEN` | **38** | Xuất hiện là ĐỎ (ví dụ `… BAD`, `ROOT PANIC`, `STALL`, `FS CORRUPT`) |
| `FAULT_ADDRESSES` | **7** | Địa chỉ fault phải gặp (6 dòng `USER FAULT` với địa chỉ cụ thể) |
| `ORDER` | **1** | Thứ tự bắt buộc (x86: `AP REAL MODE OK` → `AP LONG MODE OK` → `SMP OK`) |
| `NUMERIC` | **3** | Ràng buộc số (`ipi`, `seen/sent`, …) |
| `EXIT_CODES` | **3** | 0 / 0 / 3 |

Marker tiêu biểu: `SELFTEST PASS`, `booting on`, `BOOT UNTYPED`, `PREEMPT OK`,
`Isolation`, `Server`, `Client`, `SLEEP OK`, `Mapped`, `Cap OK`, `ViFastOs ramfs`,
`ViFastOs write`, `FS V2 OK`, `FS FSCK OK`, `FS PERM OK`, `BADGE SPOOF DENIED OK`,
`IPC BUF OK`, `BLOCK DENIED OK`, `Reply OK`, `ROOT RUST`, `BOOTINFO OK`,
`SCHED CTX OK`, `RUST MODULES OK`, `CLIENT VSPACE OK`, `FAULT EP OK`, `ROOT READY`,
`Zero OK`, `BLOCK OK`, `FS DISK OK`, `PAGER RECLAIM OK`, `RUN FS OK`, `HIARG`,
`WX OK`, `MEM OOM OK`, `FS USAGE OK`, `KILL OK`, `SLOT REUSE OK`, `IPC DEAD OK`,
`IPC JUNK OK`, `SEC PROBE OK`, `SYS REJECT OK`, `IPC RIGHTS OK`, `EXEC MEM STABLE`,
`EXEC MEM OK`, `MEM FREE OK`, `FS MOUNT CLEAN`, `FS OPS FSCK OK`, `FS RENAME OK`,
`FS LINK OK`, `FS TRUNC OK`, `FS RM OK`, `PROG MID OK`, `PROG TAIL OK`,
`PROG SIZE OK`, `PAGER SRV 0/0/0`, `SHELL READY`, `SMP WORKERS OK`, `DESTROY OK`,
`KSTORE CANARY OK`, `BIN OK args/uname/uptime`.

**Hai đặc điểm thiết kế đáng chú ý:**
1. **Marker phủ định là bắt buộc** (`FORBIDDEN`, 38 mục) — một chương trình ngoài
   hỏng **phải** làm cổng đỏ, không được "lặng lẽ thiếu marker".
2. **677 case âm** (`test-harness-spec.py`): với mỗi điều kiện, script xoá nó khỏi
   output tổng hợp và khẳng định `accept()` trả `False`. Đây là mutation test cho
   **chính bộ kiểm** — bảo đảm "PASS" không phải vì điều kiện rỗng.

### 13.2 Cấu hình QEMU [CODE]

| Arch | Lệnh (rút gọn) | Timeout |
|---|---|---|
| riscv64 | `-machine virt -smp 2 -m 128M -bios default`, VirtIO-MMIO block | **40 s** |
| aarch64 | `-machine virt,gic-version=2 -cpu max -smp 2 -m 128M`, ELF trực tiếp | **40 s** |
| x86_64 | `-machine q35 -cpu max -smp 2 -m 256M` + OVMF + FAT ESP + `isa-ide`/`ide-hd` + `isa-debug-exit 0x501` | **80 s** (`VIFAST_TIMEOUT`) |

Serial ghi ra `/tmp/vifast-<arch>.serial` (x86: `serial.txt` trong thư mục tạm).
`VIFAST_KEEP_SERIAL=1` giữ serial từng lần chạy.

### 13.3 Persistence: 7 pha × 3 arch [CODE]

`scripts/test-persist.py` dùng `vifast-fsutil` + QEMU với **đĩa tạm mới mỗi pha**:

1. format ảnh trắng;
2. persistence hai chiều;
3. migration `VF03`/`VF02` → `VF04`;
4. replay journal hợp lệ;
5. **torn journal** (ghi dở);
6. superblock hỏng (case âm: phải từ chối mount);
7. file lớn double-indirect.

Kỳ vọng: mỗi arch in `PASS`, exit 0; tổng 21 dòng phase PASS.

### 13.4 Retry, phân loại, artifact — và chỗ "PASS" nói dối [CODE]

`test-qemu.py` có ba cơ chế:

1. **Retry** (mặc định 1 lần) khi: timeout, guest không boot, exit code lạ, hoặc
   thiếu `SHELL READY`. Biến môi trường: `VIFAST_RETRY_TIMEOUT` (số lần),
   `VIFAST_NO_RETRY=1` (đúng **một** boot — dùng khi đo tỉ lệ).
2. **Phân loại + đếm**: in `RETRY n/N (T-FS=.., T-SCHED=.., T-KHAC=..)`; từ r293
   mọi lần hỏng đều được đếm **kể cả khi retry che nó**.
3. **Artifact**: mọi lần hỏng ghi `artifacts/flake-<arch>-<i>-a<lần>/` gồm
   `serial` + `status` + `tail` (và `registers.txt` khi có monitor). Hiện có
   **94 thư mục / 1.722 tệp / 16 MB** — đây là "bộ nhớ" của các cuộc điều tra.

**Vấn đề chưa đóng:** `--strict` (RETRY > 0 ⇒ ĐỎ) **đã có từ r293.3 nhưng chưa
bật trong gate** (D-01: "giữ gate mềm, `--strict` chỉ dùng khi đo"). Hệ quả: một
bản có thể "xanh" nhờ retry. Muốn bật phải có môi trường đo ổn định — chính là
điều đang thiếu.

### 13.5 `BUILD_ID` — chống "đo nhầm binary" [CODE]

- Mỗi thành phần in một dòng `BUILD <tên>=<git sha>-<hash8> feats=<feature>`.
- Một lần boot chỉ **hợp lệ** khi **mọi** dòng `BUILD` cùng một id, và id đó bằng
  trạng thái nguồn harness vừa build. Lệch ⇒ `T-BUILD`, **không** được tính vào
  tỉ lệ flake.
- `feats` **cố ý** không nằm trong id (kernel bản `test-destroy-smp` đi với
  server/client bản thường là hợp lệ; gộp vào id sẽ sinh `T-BUILD` giả).
- r322: id chỉ hash `crates/`, `scripts/`, `Cargo.toml`, `Cargo.lock`. **Lý do**:
  trước đó hash toàn bộ `git diff HEAD`, nên sửa **một dòng tài liệu** cũng đổi id
  (qua `option_env!`) ⇒ 254 lượt `Compiling`, ~6,6 phút. Đây là bài học rất đáng
  giữ: *đầu vào của hash quyết định chi phí vòng lặp.*

### 13.6 Ba cổng chạy [CODE]

| Lệnh | Nội dung | Thời gian [ĐO] |
|---|---|---|
| `./scripts/build.sh` | host test 10 crate, sàn test, fuzz, build module 3 arch, kiểm ngân sách trang, build 3 kernel | ~6,6 phút khi rebuild toàn bộ; **0** khi id không đổi (r322) |
| `./scripts/gate.sh --smoke` | build (có cache) + host test kernel + harness + **1 boot riscv64** + fmt | **~18 s** khi mã không đổi |
| `./scripts/gate.sh` | 10 bước: build, kernel-test, tests-present, fuzz, QEMU 3×3, boundary (+mutation), persist, docs, harness, fmt | **~396 s** |
| `./scripts/check-all.sh` | fmt + build + QEMU `test-destroy-smp` 5 lần/arch + QEMU 3 lần/arch + persist + harness + docs | lâu hơn gate (thêm 15 boot feature) |
| `./scripts/par-gate.sh` | chia việc: máy này riscv+arm, máy `ps` qua SSH chạy x86; rsync nguồn trừ `target/` | giảm ~1/3 |

### 13.7 Soak — cách duy nhất bắt lỗi hiếm, và cái giá của nó [ĐO]

- Lỗi hiếm **chỉ hiện dưới tải song song**: 400 boot **tuần tự** trên `ps` = 0
  treo; ~360 boot **song song 3 luồng** = 1 treo; 300 boot song song 3 luồng = 4
  treo (1,3 %); 600 boot = 2–3 treo.
- Nguyên nhân phụ: máy khác nhau (mps vs ps) cho tỉ lệ khác nhau ⇒ **không so sánh
  chéo máy nếu khác `BUILD_ID`**.
- `pkill -f qemu-system` phải chạy trước khi kết luận; QEMU mồ côi làm sai cả
  thời gian lẫn kết quả.
- Load ~2 làm QEMU chậm và timeout giả ("flake hạ tầng").

### 13.8 Kỷ luật đo đã thành luật (đừng vi phạm khi thảo luận)

1. Mọi số phải kèm **N, máy, `BUILD_ID`**, và **cấu hình** (smp, `MAX_IO`).
2. `0/N` chỉ là **trần**, phải ghi khoảng tin cậy.
3. Không đếm artifact để tính tỉ lệ (artifact bị giữ chọn lọc); dùng bảng N.
4. Bản vá phải có **test đỏ-trước** trên host, hoặc chứng minh bằng đo có đối chứng.
5. Hồi quy ⇒ **revert ngay**, không "thử lại ý đó" trong cùng một phiên (D18).
6. Không nới ngưỡng, không sửa `accept()`, không bật feature đo ở gate để "cho xanh".

---

## 14. Trạng thái hiện tại (số chuẩn)

### 14.1 Cái gì chạy được, trên arch nào [CODE + ĐO]

| Hạng mục | RISC-V64 | AArch64 | x86_64 |
|---|---|---|---|
| Boot | OpenSBI + QEMU `virt` | QEMU `virt` | OVMF/UEFI |
| Console sớm | 16550 | PL011 | COM1 |
| Self-test portable trong guest | PASS | PASS | PASS |
| User mode | U-mode `sret` | EL0 `eret` | ring3 `iretq` |
| Syscall vào kernel | `ecall` | `svc #0` | `int 0x80` |
| Timer IRQ + preempt | SBI timer | GICv2 + generic timer | PIC/PIT + LAPIC |
| SMP 2 hart + scheduler | có (SBI HSM) | có (PSCI) | có (ACPI MADT + LAPIC) |
| Per-hart ready queue + work stealing | có | có | có |
| BKL ticket + maintenance IPI + per-VA TLB | có | broadcast | có (còn hở AP→BSP) |
| VSpace riêng cho root/server/client | có | có | có |
| IPC Call/Reply/ReplyRecv, notification, cap transfer | có | có | có |
| Untyped con + BootInfo v3 + TCB idle/restart/bind | có | có | có |
| `ObjectDestroy` + reclaim Untyped (9 loại) | có | có | có |
| VFS VF04: journal, thư mục, quyền uid, hard/sym link, double-indirect, `O_APPEND/O_EXCL/O_DIRECTORY/STAT_PATH`, rename, truncate, usage | có | có | có |
| Block cache write-back + `fsync`/`BlockFlush` | có | có | có |
| Persistence 7 pha | PASS | PASS | PASS |
| Software IOMMU + DMA map, W^X chéo CPU/DMA | có | có | có |
| `Sleep` deadline (`BlockedSleep`) | có | có | có |
| Console input IRQ + shell block trên notification | có | có | có |
| CNode phẳng + `CNodeInstall` | có | có | có |
| Demand paging + user-space pager (4 client) | có | có | có |
| TCB destroy/reap + fault-slot identity | có | có | có |
| User fault suspend (không panic kernel) | có | có | có |
| Exec ELF từ FS + `argc/argv` + uỷ quyền cap | có | có | có |
| VirtIO block/net/GPU/input/RNG | có | có | **không** (ATA PIO, PS/2) |
| MCS budget/period | có | có | có |

### 14.2 Con số — và ba chỗ **chưa chốt** [CODE]

| Hạng mục | Giá trị | Ghi chú |
|---|---|---|
| Syscall | **58** (`0..57`), 58/58 có handler | có test kiểm không thiếu số |
| Object / TCB / hart | 168 / 24 / 4 (chứng minh trên 2) | `MAX_*` §5.1 |
| Cap root / cap task | 128 / 16 (CNode phẳng 16 slot) | |
| Mapping/VSpace, IOMMU mapping | 64 / 32 | |
| fd, queue | 8 / 8 | |
| `MAX_IO` | **128** | 256 là nhánh đo, không bật ở gate |
| `KERNEL_STATIC_BUDGET` | **566.272 B** | `const assert` + guard runtime |
| Ảnh FS | 4 MiB = 8192 block × 512 B | 64 inode, tên 28 B, path 128 B |
| Case âm harness | **677** | chạy được, PASS |
| Persistence | 7 pha × 3 arch | |
| Host test (đếm `#[test]`) | **402** = kernel 263 (+2 `#[ignore]`) + fs 52 + abi 17 + elf 20 + virtio 11 + pte 12 + acpi 8 + arch 8 + maint 7 + runtime 4 | ⚠️ tài liệu ghi **388** (`00-hien-trang`) và **391** (`09-testing`) — cả hai **bỏ sót** `vifast-maint` (7) + `vifast-runtime` (4) |
| Commit | 973 | |

> **Chưa chốt 1:** tổng host test. Đề nghị chốt bằng một script sinh số duy nhất
> (đã có `scripts/gen-test-counts.py --check` nhưng bảng trong `09-testing` vẫn
> liệt kê thiếu hai crate). Đây là loại sai làm mất tin vào mọi con số khác.
>
> **Chưa chốt 2:** thời gian "cổng đầy đủ" 396 s chỉ đo trên **một** máy (mps) và
> một cấu hình. Không có số trên `ps`.
>
> **Chưa chốt 3:** "3/3 arch PASS" là kết quả **có retry**. Số `RETRY n/N` của lần
> chạy cuối cùng chưa được ghi vào tài liệu trạng thái.

### 14.3 Ngân sách cứng — thứ hay bị bỏ qua khi đề xuất tính năng [ĐO]

| Hạng mục | Giá trị | Hệ quả |
|---|---|---|
| `vifast-client.stripped` (riscv) | **25.128 B = 7 trang** (ngân sách **8**) | thêm ~1 trang code là chạm trần |
| `vifast-root.stripped` (riscv) | ~37,4 KB; root banner khác **+6 dòng ⇒ +1 trang** | |
| Headroom kernel riscv | ~2,3 MB (đo vòng 158; giảm dần khi kernel lớn lên) | `diag.sh` cảnh báo nếu < 192 KiB |
| `Kernel` static | ≤ 566.272 B | vượt ⇒ build/exit đỏ |
| Data Untyped (x86) | 76 trang, dùng 69–70 khi boot | exec cần ~5 trang; **x86 là arch chật nhất** |

**Vì sao phải nhớ bảng này:** một thay đổi "nhỏ" ở client/root có thể vượt biên
trang, làm kernel nhúng ELF phình +4 KiB, đủ phá ngưỡng headroom ⇒ QEMU timeout
khó hiểu (bài học vòng 116, 157). Đây là lý do `build.sh` **chặn ngay lúc build**
thay vì để QEMU đỏ.

### 14.4 Chưa có (tuyên bố rõ, đừng giả định)

Máy thật; hardware IOMMU (VT-d/SMMU); CNode nhiều cấp/guard; CDT đầy đủ;
khoá mịn thay BKL; x2APIC; superpage; mạng đầy đủ (chưa socket API/TCP stack
thật); compositor/framebuffer console; bộ gõ ViFastKey; libc/dynamic linking/ứng
dụng thật; lớp tương thích Linux; CI hosted (bỏ có chủ ý); kiểm chứng hình thức
(Kani/Verus); driver chạy bằng IRQ; hotplug; mount/pager tổng quát nạp file thật;
allocator vật lý toàn cục (reclaim nằm trong từng Untyped); job control thật.

---

## 15. Lỗi mở — hồ sơ đầy đủ

Đây là phần agent0 cần đọc chậm nhất. Mỗi mục ghi: **triệu chứng → bằng chứng →
đã bác gì → giả thuyết còn lại → tiêu chí coi là xong**.

### 15.1 T33a — treo x86_64 sau `shell>` (ưu tiên 1)

**Triệu chứng.** Trong ~0,3–0,5 % lần boot x86_64 (TCG, `-smp 2`, soak **song song
3 luồng**), hệ đứng yên sau `shell>`: không in `PREEMPT OK`, QEMU không thoát,
timeout. Chạy **tuần tự** gần như không thấy (400 boot = 0 treo).

**Chữ ký chẩn đoán (giống hệt nhau ở 5 artifact: r317/r319/r320/r322):**

```text
GATE QUEUE  send_max=0 recv_max=1 ready0=0 ready1=0
GATE STATES ready=0 running=1 brecv=2 bsend=0 bother=2 exited=0
GATE STALL  ticks=200 done=0 fs=1 fsl=1 link=1 sym=1 tlb=1 post=1
GATE SENDFAIL n=0
(không có SMP WORKERS OK; không có SUSPEND; không có WORKER SEND RETRY)
```

Đọc: không message nào trong hàng đợi (`send_max=0`); đúng một task chờ nhận
(root trong `recv_message`); **không còn task nào Ready**; một task `Running`
(nghi worker C busy-spin); root + server `BlockedRecv`; hai task chờ "kiểu khác"
(nghi worker a/b đã xong và `wait(2)`).

**Chuỗi suy luận qua 11 vòng (r312 → r323) — đây là lịch sử đáng học:**

| Vòng | Phát hiện | Kết luận |
|---|---|---|
| r312/r313 | Thiếu `SMP WORKERS OK`; `TEST_DONE` không bật ⇒ `PREEMPT OK` không in. `TEST_EXPECT_WORKERS` = true khi `sched_harts` có bit 1 | Treo **không** phải kernel/BKL/scheduler; là "một worker không báo" hoặc "root chờ notification không được đánh thức" |
| r314–r316 | Vá "thử lại send" cho worker a/b/C. Đo 300 boot: 3–4 → 3 → 1 treo. **0 lần** `WORKER SEND RETRY` | `send` **không** lỗi ⇒ bản vá không chạm cơ chế ⇒ **revert** (D18) |
| r317/r318 | Thêm `GATE QUEUE` + `GATE STATES` + `GATE SENDFAIL` | `send_max=0` ⇒ không ai gửi; `ready=0` ⇒ deadlock im lặng |
| r319 | Tìm và vá **một đường mất message thật** trong `ipc::send` (receiver `Running` bị pop khỏi hàng mà không wake được). Có **test đỏ-trước 0,0 giây** | Tỉ lệ 7/1200 → 1/600 (Fisher p≈0,2 ⇒ **chưa đủ kết luận**) và **treo vẫn còn** |
| r320/r321 | **21/21 artifact** treo đều có `TIME MONOTONIC BAD` + `ROOT PANIC` + `crates/vifast-root/src/main.rs:494` (`panic!("GetTime went backwards on hart 1")`), là `smp_worker_c`. Trong 1448 artifact có `SMP WORKERS OK` thì **0** file có `ROOT PANIC` | **Gốc thật là worker C panic vì `GetTime` lùi**, không phải mất message. Mọi đèn trước đó chỉ thấy hệ quả |
| r321 | Vá "clamp thời gian đơn điệu theo hart" — 262 test xanh, gate 10/10, nhưng soak 600 boot vẫn 2 treo, vẫn `TIME MONOTONIC BAD` | Clamp **theo hart** không cứu được nếu task **migrate** giữa hart ⇒ revert |
| r323 (đang dở) | Đo được `start=2, last=49, now=8` ⇒ thời gian lùi từ **49 về 8**. Nguồn **duy nhất** hạ được `system_ticks` là `Kernel::reset()` (mọi hart khác chỉ `fetch_max`) | Giả thuyết: `reset()` chạy **muộn**. Đã thêm test đỏ-trước `reset_does_not_rewind_the_system_clock` và đổi `reset()` để **giữ nguyên** đồng hồ; bỏ `panic!` ở worker C, thay bằng in `C TIME BACK last= now= start=` |

**Đã bác (đừng làm lại):**
- Mất message do `send` lỗi (0 `SENDFAIL`, 0 `WORKER SEND RETRY`).
- Hàng đợi endpoint đầy (`MAX_QUEUE=8`; `send_max=0`).
- Treo do BKL/bế tắc scheduler (kernel vẫn tick 14.784 lần trong artifact).
- `-smp 1` (boot không thoát; mẫu nhỏ).
- Race bộ nhớ phía host (`tcg,thread=single` vẫn 1/3 đỏ).

**Giả thuyết còn lại (xếp theo khả năng):**
1. `Kernel::reset()` được gọi **sau** khi đã có tick > 0. Cần trả lời: đường nào gọi
   `init_kernel()`/`Kernel::new()` sau bước 7 của boot? (Đã xác nhận: chỉ
   `runtime::boot` gọi `init_kernel()` ở bước 5 và 7; `Kernel::new()` gọi `reset()`.)
   Nếu **không có** đường nào thì giả thuyết này **sai** và phải tìm nguồn khác hạ
   được `system_ticks` (hiện chỉ có `reset()`).
2. `ticks[hart]` bị ghi đè/khởi tạo lại ⇒ `fetch_max` không hạ được, nhưng nếu
   `reset()` zero cả mảng `ticks` thì một lần reset muộn sẽ làm đồng hồ "đứng" chứ
   không lùi — vẫn cần giải thích 49→8.
3. Có đường **thứ hai** ghi `system_ticks` mà tôi chưa thấy (ví dụ một `store` trong
   code test/feature, hoặc trong platform). Cần `grep` toàn bộ `.store(`/`swap(` trên
   trường này và kiểm cả `#[cfg(feature)]`.
4. Timeout/restart của QEMU khiến guest **boot lần hai** trong cùng serial
   (kiểm bằng số lần `ViFastOs booting on` trong artifact).

**Tiêu chí xong:** 600 boot x86_64 song song 3 luồng, `VIFAST_NO_RETRY=1`, **0 treo**,
**0** `TIME MONOTONIC BAD`, **0** `ROOT PANIC`; và có giải thích cơ chế được viết
thành test host (không chỉ là "không tái hiện").

**Chi phí ước tính để đạt tiêu chí:** 1 lần soak 600 boot ≈ vài giờ máy; mỗi vòng
thử cần 1 build + 600 boot ⇒ đây là lý do **không thể** thử 10 giả thuyết trong
một đêm.

### 15.2 T26 — FS vi phạm read-your-own-write (ưu tiên 2)

Chi tiết cơ chế ở §8.6. Tóm tắt trạng thái: **đã trả lời "FS hay IPC" ⇒ FS**;
**cơ chế chưa sửa**. Đo: 10/50 = 20 % ở `MAX_IO=256`, `smp2`; `-smp1` 0/3;
`tcg,thread=single` 1/3 ⇒ race **logic**.

**Kế hoạch đo đã thiết kế (chưa làm):** đèn `WRITE_THEN_READ` **bên trong**
`write_dir_entry`/`remove_dir_entry`, đọc lại **RAW** (`read_uncached`) + `seq`;
lượt `S1b` cho vòng lặp `stress-fs` chạy **song song** với exec của root; chỉ khi
có cơ chế mới viết test đỏ-trước trên host rồi vá.

**Cấm (đã chốt):** không làm "hàng rào bền vững kiểu trả Ok sau khi xuống đĩa"
trước khi hiểu cơ chế (dễ che lỗi khác); không sửa `vifast-fs` bằng khoá thô
"chặn hết" mà không có test đỏ-trước.

**Tiêu chí xong:** có một phép đo tất định (host hoặc vòng lặp trong boot) tái hiện
được vi phạm; bản vá làm nó xanh **và** tỉ lệ boot đỏ ở `MAX_IO=256` về 0/50; sau
đó mới bàn nâng `MAX_IO` về 256.

### 15.3 T31 — PAGER2 nhận message lặp (đã có đèn, chưa có gốc)

**Triệu chứng [ĐO]:** `PAGER EVICT OK` in **hai lần** ⇒ nhánh xử lý chạy **≥ 3 lần**
⇒ có message bị giao/xử lý lặp.

**Đã làm (r309):** thay vì một nhánh "≥ 2 là Evict" (chỉ báo `dup` mơ hồ), thêm
bảng quyết định thuần trong ABI: `pager2_step(seen) -> First | Evict | Dup`, và
marker riêng `PAGER2 DUP` để phân biệt "giao lặp" với "in lặp". Bảng này có test
host.

**Chưa làm:** tìm đường giao lặp (nghi phía server hoặc đường fault resume). Cần
đếm `USER FAULT … addr=0x1001001000`: 2 lần ⇒ replay phía server; 3 lần ⇒ phía client.

### 15.4 T-IPC — đã đóng (r308), giữ làm bài học

r307 kết luận "boot đỏ" là do cổng `test-exit` thoát **đúng lúc client đang chạy
user**. r308 vá: **hoãn exit một tick khi task còn `Running`**. Kết quả: 0/80 boot
thiếu marker. Bài học: một "lỗi sản phẩm" có thể là **lỗi của cổng đo**; luôn kiểm
đường thoát trước khi nghi kernel.

### 15.5 Bốn lớp lỗi cổng hay bị lẫn (phân loại chuẩn)

| Mã | Nghĩa | Dấu hiệu | Cách xử |
|---|---|---|---|
| `T-BOOT` | Guest không boot (chỉ vài dòng serial) | serial rất ngắn | flake UEFI/QEMU; chạy lại; kiểm OVMF/ESP |
| `T-FS` | Chết sau `SHELL READY`, thiếu marker FS | có `SHELL READY`, thiếu `FS V2 OK`… | FS/cache race (T26) |
| `T-SCHED` | Treo thiếu `PREEMPT OK` | `shell>` rồi đứng | T33a / scheduler |
| `T-KHAC` | Còn lại | | đọc artifact |
| `T-BUILD` | Các dòng `BUILD` lệch id | | build lại sạch, **không tính vào tỉ lệ** |
| `T-INFRA` | Máy quá tải, QEMU mồ côi | | dọn tiến trình, chạy lại tối đa 2 lần, ghi nhận |

### 15.6 x86 shootdown AP→BSP và PCID (ưu tiên 3)

Xem §7.5. Đây là **lỗi đúng đắn** (không phải hiệu năng): TLB của BSP có thể cũ
sau khi unmap ⇒ một tiến trình có thể đọc trang đã bị thu hồi. Hiện không lộ trên
TCG mặc định nên mọi cổng vẫn xanh.

**Tiêu chí xong:** có test (host hoặc QEMU `-cpu max`/KVM) chứng minh TLB cũ gây
đọc sai; bản vá làm nó xanh; hoặc tắt PCID có kiểm soát + ghi lý do.

### 15.7 `--strict` chưa bật (ưu tiên 4, nhưng chặn mọi kết luận)

Đã có từ r293.3. Lý do chưa bật (D-01): máy đo chưa ổn định, bật sẽ đỏ vì flake hạ
tầng. Nhưng chừng nào chưa bật, "PASS" **không** đồng nghĩa "không có lỗi hiếm".

**Đề xuất để thảo luận:** bật `--strict` trên **một** máy chuyên dụng, chạy ban
đêm, và coi đó là số liệu chính; gate thường ngày vẫn mềm nhưng **in** `RETRY n/N`
ở dòng cuối để không ai quên.

### 15.8 Tồn đọng audit (đã hợp nhất, 0 OPEN nhưng còn mục "cần vòng riêng")

`docs/audits/open-items.md`: K32..K75 = **43 CLOSED · 3 UNREACHABLE · 0 OPEN**;
round82/88/92 còn:

| Mục | Trạng thái | Ghi chú |
|---|---|---|
| `Q1-A` kênh wake trong `Mailbox` chưa thống nhất 3 port (riscv có doorbell riêng cho reschedule, arm/x86 dùng mailbox chung) | **OPEN** | nợ r293; thống nhất sau r293–r298 |
| `P1-5` `enqueue` trả sớm ⇒ `Ready` nhưng không trong runqueue | **OPEN** | liên quan K37–K40; cần đọc lại `enqueue` |
| `P1-6` aarch64 SGI 2 chưa enable trong GICD | **OPEN** | đường panic-hart1 vẫn PASS nên chưa kích hoạt |
| `P1-7` GDT/TSS per-hart | **mâu thuẫn nội bộ**: bảng round92 ghi `CLOSED (r261)`, mục "việc tiếp" ghi `OPEN (P1)` | cần chốt lại; rất có thể đã xong |
| `slot_arg(..).unwrap_or(0)` ở 53 chỗ | **OPEN** | rủi ro thấp (0 rơi vào slot không tồn tại) |

### 15.9 T33b — lỗi hiếm thứ hai (chưa ưu tiên)

Trong soak 100×3 (r290) có **1** ca riscv64: fault rồi không còn thread runnable.
Không liên quan TLB/IPI. Chưa tái hiện. Nếu T33a đóng mà vẫn thấy ca "fault + no
runnable", đây là ứng viên tiếp theo.

### 15.10 Điều kiện coi là "đã sửa" (áp cho mọi lỗi hiếm ở trên)

1. Có **cơ chế** được viết ra (không phải "hết tái hiện").
2. Có **test tất định** (host hoặc một lần boot có marker) canh cơ chế đó.
3. Số đo trước/sau với **N ≥ 600** boot cùng `BUILD_ID`, cùng máy, cùng cấu hình.
4. Không đổi ngưỡng/`accept()`/feature của cổng để đạt xanh.
5. Một dòng trong `docs/decisions/log.md` ghi ngày, lý do, commit.

---

## 16. Vì sao vòng lặp phát triển hiện tại không hiệu quả

Phần này là **ý kiến phân tích có căn cứ số liệu**, không phải mô tả. Mục tiêu là
để agent0 phản biện được, chứ không phải để đồng ý.

### 16.1 Sự thật khô khan: 11 vòng cho một lỗi, và lỗi vẫn còn

`r312 → r323` (11 vòng, 973 commit tích luỹ) dành cho T33a. Kết quả: **cơ chế đã
được thu hẹp đúng** (worker C panic vì `GetTime` lùi) nhưng **chưa chứng minh được
đường gây ra việc lùi**, và mỗi vòng đều tốn: 1 build + 1 gate + một đợt soak
300–600 boot + viết báo cáo.

Cùng lúc, T26 (20 % boot đỏ ở cấu hình đo) đã có cơ chế nhưng chưa vá, và hai lỗ
hổng đúng đắn (x86 shootdown, PCID) đã biết từ lâu mà chưa có vòng nào.

### 16.2 Bốn nguyên nhân gốc (theo mức tác động)

**(1) Độ phân giải đo thấp hơn độ lớn hiệu ứng cần đo.**
Tỉ lệ lỗi ~0,3–0,5 %. Để phân biệt "giảm từ 0,58 % xuống 0,17 %" với "không đổi"
ở mức tin cậy 80 %, cần **hàng nghìn boot mỗi nhánh**. Thực tế mỗi vòng đo 300–600
boot ⇒ mọi kết luận "đã giảm" đều **thiếu công suất thống kê**. Bằng chứng ngay
trong nhật ký: 7/1200 vs 1/600 cho Fisher **p ≈ 0,2** — tức "chưa kết luận được",
nhưng vòng đó vẫn được ghi như một bước tiến.

**(2) Dụng cụ quan sát vừa thô, vừa *làm thay đổi* đối tượng đo (heisenbug).**
Công cụ duy nhất là *in một dòng ra serial rồi grep*. Mỗi đèn thêm vào:
- làm dài đường nóng (ví dụ đèn trong `handle()` của server làm dài cửa sổ IPC ⇒
  **tỉ lệ exec hỏng tăng** — đã ghi nhận ở r297);
- làm thay đổi thời điểm tick ⇒ race dịch chỗ;
- và bản thân nó có thể là bug mới.

Không có: timestamp theo từng sự kiện, ring buffer có cấu trúc, core dump, trace
đa hart có thứ tự nhân quả, replay tất định. `RINGDUMP` chỉ tồn tại cho **request
VFS**, không phải cho IPC/scheduler/fault.

**(3) Cổng đo được thiết kế để "đi tiếp", không phải để "kết luận".**
Retry mặc định 1 lần + `--strict` tắt ⇒ "PASS" là *"đã từng pass trong ≤2 lần thử"*.
Cơ chế đếm `RETRY` (r293) đã có nhưng không nằm trong tiêu chí xanh. Vì vậy một
bản vá không cải thiện gì vẫn có thể "xanh", và một bản vá tốt vẫn có thể "đỏ"
vì flake — cả hai đều làm sai quyết định.

**(4) Chi phí biên của mỗi thay đổi cao vì ràng buộc tĩnh.**
`Kernel ≤ 566 KB`, client ≤ 8 trang, headroom riscv ≥ 192 KiB, `MAX_TCBS = 24`,
`MAX_OBJECTS = 168`. Một tính năng "nhỏ" (thêm một task demo, thêm một đèn nặng)
có thể vượt biên trang ⇒ build đỏ hoặc QEMU timeout **không liên quan** tới logic
đang sửa. Đây là nguồn nhiễu thứ hai, độc lập với (1).

### 16.3 Năm triệu chứng tổ chức (quan sát được)

1. **Tài liệu là nơi ký gửi trạng thái, không phải nơi tra cứu.** 274 tệp / 14 MB;
   90 bundle / 11 MB; 101 báo cáo `agent2-report-*`; một `progress-g1.md` 206 KB;
   một `overnight-log.md` 226 KB. Không ai đọc hết; nên **không ai biết chắc** con
   số nào đúng. Đây là lý do yêu cầu "nén vào 1 tệp" là đúng hướng.
2. **Ba "nguồn sự thật" cạnh tranh:** mã nguồn, `docs/00-hien-trang.md`, và
   `scripts/harness_spec.py`. Khi lệch, không có quy tắc nào nói cái nào thắng
   (trên thực tế mã thắng, nhưng tài liệu không ghi vậy ở mọi chỗ).
3. **Nhiều agent cùng ghi một cây.** Có lúc 3 luồng soak chạy song song, một máy
   remote qua SSH, rsync nguồn trước khi chạy ⇒ **chính môi trường đo** là biến
   gây nhiễu, và `BUILD_ID` khác nhau giữa các máy (đã có sự cố).
4. **Mỗi lần điều tra lại phát minh lại dụng cụ.** Từ r312 đến r322 có 5 bộ đèn
   khác nhau (`GATE STALL`, `GATE QUEUE`, `GATE STATES`, `GATE SENDFAIL`,
   `AP STATE`), mỗi bộ thêm/xoá thủ công, không có khuôn chung.
5. **Chi phí đóng gói lớn hơn chi phí sửa.** Mỗi vòng phải: cập nhật
   `progress-g1.md`, báo cáo, bundle có chữ ký SHA-256, kiểm docs, gate 10/10.

### 16.4 Sáu phương án — kèm cái giá và điều kiện đúng

> Đây là *lựa chọn để thảo luận*, không phải khuyến nghị áp đặt. Mỗi phương án ghi
> rõ "đúng khi nào" và "sai khi nào".

**A. Mô hình hoá boot trên host (tất định).**
Kernel portable đã host-test được (`cargo test -p vifast-kernel`, 263 test). Dựng
một harness chạy **toàn bộ trình tự boot giả lập** với: platform giả (clock giả,
MMU giả), scheduler **tất định theo bước**, và một tập interleaving cố định — rồi
soi các bất biến (đồng hồ đơn điệu, không mất message, không task nào kẹt).
- *Ưu:* tất định, chạy trong giây, không cần QEMU, debug bằng `dbg!`/backtrace.
- *Nhược:* không mô hình hoá được trap/MMU/IRQ thật; có thể "xanh giả" (đã có tiền
  lệ: host test xanh trong khi QEMU gãy pager).
- *Đúng khi:* lỗi nằm ở **logic** tầng portable (như T33a: `GetTime`/`reset`,
  `make_ready`, `recv`). *Sai khi:* lỗi nằm ở platform/timing phần cứng.
- *Chi phí ước tính:* 2–4 ngày người, ~500 dòng harness + 10 kịch bản.

**B. Một trace ring buffer có cấu trúc, thay cho hàng chục marker.**
Ghi `{tick, hart, tcb, event_id, a, b, c}` vào ring cố định (ví dụ 4096 bản ghi,
64 B/bản ghi = 256 KB trong static của platform); khi stall/timeout thì dump ra
serial theo định dạng máy đọc được; script hậu xử lý (grep/awk/python) dựng lại
thứ tự nhân quả.
- *Ưu:* một dụng cụ cho mọi cuộc điều tra; chi phí ghi thấp và **đều** (không làm
  lệch cục bộ như đèn nằm trong `handle()`); có timestamp thật.
- *Nhược:* tốn ngân sách static; vẫn có thể đổi timing (nhưng đổi **đều**).
- *Đúng khi:* cần biết "ai làm gì trước ai" trong 1 ms cuối. *Sai khi:* cần biết
  trạng thái phần cứng (dùng QEMU monitor/debug).
- *Chi phí:* 1–2 ngày + 1 vòng gate.

**C. Cô lập đường gating: test không được giết boot.**
Worker C hiện `panic!` khi phát hiện bất thường ⇒ biến một **phát hiện** thành
**treo**, làm mất toàn bộ thông tin phía sau. Mẻ r323 đã đổi thành in + đi tiếp
(đúng hướng). Nguyên tắc tổng quát: *một demo/test trong guest không được phép
làm chết boot*; nó phải in marker `… BAD` để cổng đỏ, nhưng vẫn để các test khác
chạy tiếp. (Có ngoại lệ: mutation test cố ý.)
- *Ưu:* biến "treo không thông tin" thành "đỏ có thông tin"; giảm hẳn chi phí điều tra.
- *Nhược:* phải rà toàn bộ `panic!`/`unwrap` trong root/client.
- *Chi phí:* nửa ngày.

**D. Tách "build đo" khỏi "build cổng".**
Đèn chẩn đoán chỉ tồn tại trong feature; gate chạy **đúng** bản phát hành; mọi số
đo phải ghi rõ feature. Hiện tại một số đèn nằm ở đường nóng mà không có feature.
- *Ưu:* gate phản ánh sản phẩm; số đo không lẫn.
- *Nhược:* hai cấu hình ⇒ phải kiểm cả hai (đã có tiền lệ `test-destroy-smp`).
- *Chi phí:* 1 ngày.

**E. Thống kê tử tế: chốt N trước, dùng kiểm định tuần tự.**
Mỗi tuyên bố "đã giảm" phải kèm N, khoảng tin cậy, và một ngưỡng quyết định đặt
**trước** khi đo (ví dụ SPRT: dừng khi LLF vượt biên). Đồng thời: chỉ so sánh khi
cùng `BUILD_ID`, cùng máy, cùng cấu hình.
- *Ưu:* hết cảnh "p=0,2 nhưng vẫn ghi là tiến bộ".
- *Nhược:* cần kỷ luật; một số vòng sẽ kết luận "không đủ dữ kiện" (đúng, nhưng
  khó chịu).
- *Chi phí:* nửa ngày viết script + kỷ luật.

**F. Giảm bề mặt: đóng băng tính năng, cắt demo.**
Trong lúc săn lỗi hiếm, tắt các demo không liên quan (GPU/input/net/pager2/3,
exec chain) bằng **một** feature `minimal`, và thay bằng tải tổng hợp **biết
trước** (N TCB cùng làm việc cố định). Khi hết lỗi thì bật lại từng cái.
- *Ưu:* giảm số biến; tái hiện nhanh hơn; ít đèn hơn.
- *Nhược:* có thể che lỗi (T33a cần "hoạt động đồng thời của root lúc boot").
- *Đúng khi:* dùng **song song** với A hoặc B, không thay thế.
- *Chi phí:* 1 ngày.

### 16.5 Nếu chỉ được chọn 3 việc (ý kiến của người viết)

1. **C** (test không được giết boot) — rẻ nhất, mở khoá thông tin ngay.
2. **A** (mô hình boot trên host) — vì đúng chỗ: giả thuyết hiện tại của T33a
   (`reset()` chạy muộn / `system_ticks` bị hạ) là **logic thuần**, kiểm được trên host.
3. **E** (thống kê) — để không tiêu thêm 11 vòng cho một lỗi.

B và D là hạ tầng tốt nhưng nên làm **sau** khi có ít nhất một lỗi hiếm được đóng
bằng A/E, để chứng minh cách làm mới hiệu quả trước khi đầu tư.

### 16.6 Điều KHÔNG nên làm (dựa trên lịch sử đã ghi)

- Thêm đèn vào đường nóng để "xem cho biết" — mỗi đèn là một biến mới.
- Đổi ngưỡng/`accept()`/feature để cổng xanh.
- Chạy soak trên máy đang tải hoặc còn QEMU mồ côi.
- So sánh số giữa hai `BUILD_ID`/hai máy.
- Gộp nhiều chỗ teardown/reap trong một vòng (đã hỏng 0/3 cả ba arch).
- Sửa FS bằng một khoá thô "chặn hết" khi chưa có test đỏ-trước.

---

## 17. Nợ kỹ thuật và rủi ro kiến trúc

| # | Nợ | Bằng chứng | Ảnh hưởng | Chi phí sửa |
|---|---|---|---|---|
| 1 | `vifast-kernel/src/lib.rs` = **17.715 dòng** một tệp | `wc -l` | Mọi thay đổi phải đọc/đụng một tệp khổng lồ; review kém; merge khó | Lớn (tách module theo object/ipc/sched/vspace/cap) — **đang bị D-07 cấm** |
| 2 | Trần tĩnh: 168 object / 24 TCB / 566 KB | §5.1 | "Thêm một task" là thay đổi kiến trúc, không phải thêm dòng | Lớn (pool tách khỏi `Kernel`, hoặc allocator) |
| 3 | BKL toàn kernel | §12 | Mọi syscall tuần tự hoá; không đo được scalability | Rất lớn (khoá mịn) |
| 4 | Driver polling | §11.4 | Không có I/O bất đồng bộ thật; mốc M2 chưa đạt | Trung bình |
| 5 | Không có hardware IOMMU | §11.5 | Chỉ "software IOMMU"; DMA thật trên máy thật sẽ không được bảo vệ | Lớn |
| 6 | Kernel không có allocator vật lý toàn cục | §5.5 | Reclaim nằm trong từng Untyped; phân mảnh khó xử lý | Lớn |
| 7 | Chưa có CNode nhiều cấp/guard, CDT đầy đủ | §5.3 | CSpace phẳng 16 slot ⇒ dễ hết slot (đã gặp: PAGER3 cần nâng root lên 128) | Trung bình |
| 8 | Không có ASID/PCID allocator | §7.5 | Flush thừa hoặc sót; lỗ hổng PCID | Trung bình |
| 9 | `gen: u32` có thể tràn | §5.2 | Lý thuyết; hệ chạy lâu năm | Nhỏ |
| 10 | O(n) trong kernel (`RunQueue.remove`, `cancel_ipc`, `find_tcb`) | §6.7 | WCET không chứng minh được ⇒ chặn hướng Space | Trung bình |
| 11 | Không có test tất định cho race | §16.2 | Nguyên nhân chính của §16 | Trung bình |
| 12 | Tài liệu phân mảnh, lệch số | §18 | Mỗi phiên mất thời gian "tin cái nào" | Nhỏ–trung bình (làm một lần) |
| 13 | `vifast-runtime` và `vifast-user` không có unit test | `test-floor.json` | Logic dispatch/abi ít được canh | Nhỏ |
| 14 | FP/SIMD bị tắt | §11.6 | Không chạy được chương trình dùng FP; cần vùng lưu TrapFrame | Trung bình |
| 15 | Root task giữ quá nhiều vai | §4.1 | Root là TCB lớn: một bug ở root = toàn hệ; khó thu nhỏ | Lớn |

**Điểm mạnh cần giữ (đừng "tối ưu" mất):**
- `check_invariants()` + mutation test cho chính bộ kiểm.
- 677 case âm cho hợp đồng marker.
- Test đỏ-trước là văn hoá đã có (r319 làm được trong 0,0 giây).
- Ranh giới crate sạch: kernel không biết phần cứng.
- Mọi thứ đo được bằng một lệnh (`gate.sh`), không cần CI dịch vụ.

---

## 18. Bản đồ tài liệu — và những chỗ tài liệu **sai** so với mã

### 18.1 Ba loại tài liệu trong repo (theo chính `docs/README.md`)

| Loại | Nơi | Quy tắc |
|---|---|---|
| **SỐNG** (phải khớp mã) | `README.md`, `docs/00-hien-trang.md`, `01-tong-quan`, `02-kien-truc`, `05-port-rust-va-kiem-thu`, `06-lo-trinh`, `10-multi-arch`, `docs/11-hien-tai/*` (18 tệp), `ViFastOs-architecture-summary.txt` (tự sinh) | sửa **cùng commit** với mã |
| **THIẾT KẾ THAM CHIẾU** (có thể lệch) | `docs/03-thanh-phan/*` (9 tệp), `04-diem-chu-y`, `07-thuat-ngu`, `08-giai-thich-cho-sinh-vien-nam-nhat` (81 KB), `design/*` (7 tệp), `ViFastOs-agent-handoff.md` (47 KB) | có banner cảnh báo; nhiều phần **chưa port** (CDT, CNode nhiều cấp, `vifast-sim`, IOMMU phần cứng, compositor) |
| **LỊCH SỬ** (không sửa) | `reviews/` (17 tệp, 1 MB, gồm `progress-g1.md` 206 KB), `audits/` (12), `bugs/` (1), `decisions/log.md`, `bundles/` (**90 tệp, 11 MB**), `overnight-log.md` (226 KB), `agent2-report-*.md` (**101 tệp**), `agent0-inbox.md` | ảnh chụp tại thời điểm viết |

**Đọc gì khi cần gì:**
- muốn biết *đang ở đâu* → tệp này (§14) hoặc `00-hien-trang.md` (nhưng đã cũ ~15 commit).
- muốn biết *vừa xảy ra gì* → `progress-g1.md` (mục cuối) và `git log` (nguồn tốt hơn).
- muốn biết *vì sao quyết định thế* → `docs/decisions/log.md` (bảng 100+ dòng, rất giá trị).
- muốn biết *hợp đồng hành vi* → `scripts/harness_spec.py` (**nguồn sự thật thật sự**).

### 18.2 Bảng lệch đã xác minh (đừng dùng tài liệu này làm chuẩn)

| Tệp | Tài liệu nói | Mã/thực tế | Mức |
|---|---|---|---|
| `docs/11-hien-tai/07-arch-ports.md` §4 | "x86 AP bootstrap **chưa có code hoạt động**… `start_smp` in `SMP SKIP`"; "hướng tiếp theo là ACPI MADT + LAPIC/SIPI" | ACPI MADT + LAPIC + AP trampoline **đã có**; x86 in `ACPI MADT OK (cpus=2)`, `LAPIC OK`, `AP TRAMPOLINE OK`, `SMP OK`; x86 SMP nằm trong `COMMON`/`ARCH` của harness | **SAI hoàn toàn** |
| `docs/11-hien-tai/01-abi.md` §6 | "Tất cả **52** số 0..51 đều có handler" | **58** syscall 0..57, 58/58 có handler (bảng ngay trên đó liệt kê 58) | Sai số |
| `docs/11-hien-tai/01-abi.md` §5b | `BootInfo` 12 trường | `BOOTINFO_VERSION = 3`, **15** trường (thêm `block_device`, `server_elf_va/len`, `client_elf_va/len`, `sched_harts`) | Thiếu |
| `docs/11-hien-tai/05-vfs.md` | Mô tả **ramfs kernel** (8 file, 512 B) như thể là VFS của hệ; "chưa có thư mục/quyền/mount/symlink" | VFS thật là `vifast-fs` VF04 + server user-space: **có** thư mục, uid, hard/sym link, journal, fsck, rename, truncate | **Gây hiểu nhầm** |
| `docs/11-hien-tai/04-memory-vspace.md` §6 | "**Chưa có** IPI shootdown đa hart" | Có `vifast-maint` (hộp thư + mask hart) + per-VA flush; RISC-V/AArch64/x86 đều có maintenance IPI (x86 còn hở AP→BSP) | Lỗi thời |
| `docs/11-hien-tai/04-memory-vspace.md` §2 | "tối đa **32** mapping (reverse map)" | `MAX_MAPPINGS = 64` | Sai số |
| `docs/11-hien-tai/04-memory-vspace.md` §3 | RISC-V kernel accessible "khoảng **4 MiB** (`0x8000_0000..0x8040_0000`)" | đã nới lên **6 MiB** (`..0x8060_0000`, vòng 158); `diag.sh` dùng 0x8060_0000 | Lỗi thời |
| `docs/00-hien-trang.md` | "**388** host test"; `HEAD 3008601` (r307) | đếm được **402** (263+52+17+20+11+12+8+8+7+4); HEAD `8077d71` (r322) | Lỗi thời + sai số |
| `docs/11-hien-tai/09-testing.md` đầu tệp | "Cập nhật 2026-11-06, sau vòng 22.2 (HEAD `c86de49`)" | nội dung đã được sửa tới r319/r323 nhưng header còn cũ; bảng tổng **391** bỏ sót `vifast-maint` (7) + `vifast-runtime` (4) | Header lỗi thời, tổng thiếu |
| `docs/11-hien-tai/09-testing.md` §4 | "Timeout: **20 s** (RISC-V/AArch64), **40 s** (x86)" | thực tế trong `test-qemu.py`: riscv **40**, aarch64 **40**, x86 **80** (`VIFAST_TIMEOUT`) | Lỗi thời |
| `docs/00-hien-trang.md` | "Object table 168 object, 24 TCB, **4 hart**" | `MAX_HARTS=4` nhưng **chưa chứng minh** >2 hart; `-smp 2` là cấu hình đo | Gây hiểu nhầm |
| `docs/agent0-inbox.md` | "Inbox cho agent0" | Nội dung là **vòng 26–30**, cách hiện tại ~290 vòng; các blocker trong đó đã đóng từ lâu | **Lỗi thời nặng** |
| `docs/07-thuat-ngu.md` | Banner "TÀI LIỆU THIẾT KẾ THAM CHIẾU… nhiều phần chưa port" | Đây là **bảng thuật ngữ**; banner vô nghĩa và làm người đọc nghi ngờ cả định nghĩa | Nhiễu |
| `docs/03-thanh-phan/*` (9 tệp) | CDT, CNode nhiều cấp, guard, IOMMU thật, `vifast-sim`, profile manifest | Không có trong mã (CSpace phẳng, IOMMU software, không có sim) | Nhiễu có cảnh báo |
| `docs/audits/open-items.md` | `P1-7` GDT/TSS per-hart "**OPEN (P1)**" ở mục "việc tiếp", nhưng bảng round92 ghi "**CLOSED (r261)**" | Mâu thuẫn **trong cùng một tệp** | Cần chốt |
| `docs/decisions/log.md` | Bảng quyết định tới r294 | Thiếu các quyết định r295–r322 (D-01..D-08 nằm trong bản bàn giao, không trong log) | Thiếu |

### 18.3 Ba nguồn sự thật và thứ tự ưu tiên **nên** áp dụng

1. `scripts/harness_spec.py` — ngữ nghĩa "boot đúng" (có 677 case âm canh chính nó).
2. Mã nguồn + `cargo test` — hành vi kernel/ABI.
3. `docs/decisions/log.md` + `progress-g1.md` — lịch sử quyết định và số đo.
4. `00-hien-trang.md` / `09-testing.md` — chỉ là **bản tóm tắt**, có thể lệch; đừng
   trích dẫn như số liệu gốc.

### 18.4 Đề xuất dọn tài liệu (rẻ, làm một lần)

1. Chuyển `docs/bundles/` (90 tệp, 11 MB) + `docs/agent2-report-*.md` (101 tệp) +
   `overnight-log.md` vào `docs/archive/` và **không** đưa vào đường đọc.
2. Giữ **một** trang trạng thái được **sinh tự động** (từ `harness_spec`, `cargo test
   --list`, `MAX_*`, `git rev-parse`, kết quả gate gần nhất) — bỏ các con số viết tay.
3. Xoá/viết lại `docs/agent0-inbox.md` (hiện là lịch sử vòng 26–30).
4. Thêm luật: mỗi tệp tài liệu có header `<!-- HEAD: <sha> | generated: yes/no -->`;
   `check-docs.py` đỏ nếu sha lệch quá N commit.
5. Đặt tệp này (`ViFastOs-dossier.md`) ở gốc repo và **coi là điểm vào duy nhất**
   cho người/agent mới; mọi tệp khác là "chi tiết tuỳ chọn".

---

## 19. Ràng buộc đang có hiệu lực (đừng vi phạm khi đề xuất)

### 19.1 Tám quyết định D-01..D-08 (chốt cho đợt chống flake, vẫn hiệu lực)

| Mã | Nội dung | Lý do |
|---|---|---|
| **D-01** | Giữ gate **mềm**; `--strict` chỉ dùng khi **đo** | Máy đo chưa ổn định; bật sẽ đỏ vì flake hạ tầng |
| **D-02** | **Đóng băng tính năng**, chỉ sửa lỗi + đo | Tránh thêm biến khi đang săn lỗi hiếm |
| **D-03** | T26: đánh **"ai/cái gì chen giữa"**, **không** đánh nhánh `fsck` (đã đóng) | fsck không phải nguyên nhân (đã chứng minh) |
| **D-04** | Đo ở `MAX_IO=256` (**chỉ nhánh đo**); không bật ở gate/phát hành | 256 làm lộ race; 128 giữ cổng xanh |
| **D-05** | Teardown tối đa **2 chỗ**, mỗi chỗ **một vòng riêng**, hồi quy ⇒ **revert** | Gộp nhiều chỗ đã làm hỏng 0/3 cả ba arch |
| **D-06** | Soak nền ~600–1000 boot x86_64 trên máy `ps` | Cần mẫu lớn; máy rảnh |
| **D-07** | **Không** đụng nợ kiến trúc (không tách `lib.rs`) | Ưu tiên đóng lỗi trước |
| **D-08** | **Không** đụng đồ hoạ / ViFastKey | Ngoài phạm vi đợt |

### 19.2 Luật vận hành khác (từ `progress-g1`, `decisions/log`, runbook)

- **D18** — bản vá hồi quy ⇒ **revert ngay**, ghi nhật ký, **không thử lại ý đó**
  trong cùng phiên.
- **D11** — không sửa mò: cần **đường tái hiện** hoặc chứng minh cơ chế trước.
- **Quy tắc 2.5** — hai phiên liên tiếp bị chặn ở cùng một ý ⇒ **đổi nhánh**, không
  thử lần thứ ba.
- **Cấm** (đã ghi thành luật): nới ngưỡng; sửa `accept()`/test cho xanh; bật
  `--strict` mặc định; bật `MAX_IO=256` ở gate; nâng `MAX_TCBS`/`MAX_OBJECTS`/
  `KERNEL_STATIC_BUDGET`; gộp nhiều chỗ teardown; xoá marker đang dùng để đo; sửa
  `vifast-fs` bằng khoá thô "chặn hết" mà không có test đỏ-trước.
- **Bắt buộc**: đổi hành vi guest ⇒ cập nhật `harness_spec.py` + `09-testing.md`
  **cùng commit**; đổi số test/lệnh shell/số marker ⇒ chạy `gen-*.py` tương ứng;
  chạy `cargo fmt --all --check` trước khi commit.

### 19.3 Quy ước kỹ thuật không được phá

- `vifast-kernel` và `vifast-runtime`: `#![no_std]` + `#![forbid(unsafe_code)]`.
- `unsafe` chỉ ở `vifast-arch-*`, `vifast-platform-*`, entry assembly.
- Không thêm `cfg(target_arch)` vào kernel/runtime.
- Không build tay đè kernel của cổng (thiếu `test-exit` ⇒ thiếu `PREEMPT OK`);
  nếu cần kernel đặc biệt thì dùng `--target-dir` khác rồi build lại.
- Mọi thay đổi layout user phải cập nhật `COPY_WINDOWS` (có const assert + test canh).

---

## 20. Câu hỏi mở để thảo luận lại từ đầu với agent0

Mỗi câu ghi: **cần quyết gì**, **các lựa chọn**, **hệ quả**. Đây là agenda đề xuất.

**Q1. Mục tiêu 90 ngày tới là gì?**
(a) Đóng T33a + T26 + hai lỗ hổng đúng đắn, **không** thêm tính năng;
(b) đạt mốc M1 "gate nghiêm + soak sạch" (bao gồm bật `--strict`);
(c) mở một nhánh sản phẩm (mạng/FS lớn/desktop).
*Hệ quả:* (a) là điều kiện tiên quyết của (b); (c) chỉ nên bắt đầu sau (b).

**Q2. Chiến lược săn lỗi hiếm: A (host model), B (trace ring), C (cô lập), E (thống kê) hay tiếp tục như hiện tại?**
Xem §16.4. *Đề xuất của người viết:* C ngay, A + E song song, B sau.

**Q3. T33a: tiếp tục giả thuyết `reset()` hay dừng lại làm hạ tầng?**
Nếu chọn "tiếp tục": cần trả lời dứt điểm *đường nào gọi `reset()` sau tick > 0*
(grep toàn bộ `.store(`/`swap(` trên `system_ticks`, kiểm cả feature và test).
Nếu chọn "hạ tầng": làm C + A trước, rồi quay lại.
*Hệ quả:* mỗi lần thử giả thuyết = 1 build + 600 boot ≈ vài giờ.

**Q4. Worker C (và các demo trong root) có nên nằm trên đường gating không?**
Nó là **test chứng minh preempt/migration**, không phải tính năng sản phẩm. Lựa chọn:
(a) giữ nguyên, chỉ bỏ `panic!` (r323);
(b) chuyển kiểm "thời gian đơn điệu" thành **test host** và cho worker C ngừng kiểm
trong guest;
(c) tách worker C ra feature `stress-sched`, gate mặc định không chạy.
*Hệ quả:* (c) giảm nhiễu nhưng mất một phép kiểm end-to-end quý giá.

**Q5. T26: ai làm và ngân sách bao nhiêu vòng?**
Cần: đèn `WRITE_THEN_READ` trong `vifast-fs` (RAW + `seq`), lượt `S1b` (vòng lặp
song song với exec), rồi test đỏ-trước + vá. *Đề xuất:* 3 vòng, dừng sau vòng 3 để
đánh giá lại (theo quy tắc 2.5).

**Q6. `--strict` bật ở đâu và khi nào?**
(a) bật trên **một** máy chuyên dụng, chạy đêm, coi là số liệu chính;
(b) bật trong `check-all.sh` (cổng "phát hành") nhưng không trong `gate.sh` hàng ngày;
(c) tiếp tục tắt.
*Hệ quả:* không có (a) hoặc (b) thì mọi tuyên bố "ổn định" đều không kiểm chứng được.

**Q7. Có phá D-07 để tách `vifast-kernel/src/lib.rs` không?**
Lựa chọn: (a) giữ nguyên tới khi đóng xong lỗi; (b) tách **cơ học** (chỉ di chuyển
module, không đổi logic) trong một commit riêng, có test đỏ-trước cho hành vi
không đổi; (c) tách dần theo từng vùng khi đang sửa.
*Hệ quả:* (b) làm review khó một lần nhưng giảm chi phí mọi vòng sau.

**Q8. Trần tĩnh (`MAX_*`, 566 KB, 24 TCB) có mở không?**
Lựa chọn: (a) giữ, mọi tính năng phải vừa; (b) tách pool ra khỏi `Kernel` (đã thử
3 lần, hỏng AArch64 vì layout `.data`); (c) nâng có kỷ luật kèm đo headroom.
*Hệ quả:* (b) là nợ kiến trúc lớn; (c) là nới ngầm — đã bị cấm.

**Q9. Có nên tạm "đóng băng" x86_64 để ổn định riscv/arm không?**
Lựa chọn: (a) không (mất tính 3 cổng); (b) có, trong lúc săn T33a, vẫn build x86
nhưng không đưa vào cổng xanh/đỏ; (c) giữ nhưng tách "cổng x86" khỏi "cổng 2 arch".
*Hệ quả:* (c) giúp tách flake x86 khỏi hồi quy chung.

**Q10. Tài liệu: có archive 90 bundle + 101 report không?**
Xem §18.4. *Hệ quả:* giảm 12 MB nhiễu; nhưng bundle là "bằng chứng có chữ ký" —
cần giữ để truy vết, chỉ đổi vị trí và bỏ khỏi đường đọc.

**Q11. Định nghĩa "xong" cho giai đoạn hiện tại?**
Đề xuất: 3/3 arch, **0 retry**, `--strict`, soak 600 boot × 3 arch trong 7 ngày
không treo, T33a + T26 đóng có cơ chế + test, hai lỗ hổng TLB/PCID đóng hoặc có
quyết định "chấp nhận + ghi lý do".

**Q12. Ai giữ "nguồn sự thật" về số liệu?**
Đề xuất: một script `scripts/gen-status.py` sinh `STATUS.md` từ mã + gate gần nhất;
mọi tài liệu khác **không** được viết số tay.

---

## 21. Phụ lục

### 21.1 Chỉ mục tệp quan trọng (để yêu cầu trích xuất)

| Đường dẫn | Vai trò |
|---|---|
| `crates/vifast-kernel/src/lib.rs` | 17.715 dòng: object/cap/IPC/sched/VSpace/ramfs/invariants |
| `crates/vifast-kernel/src/{object,ipc,fs,selftest,console}.rs` | object table, IPC, ramfs, self-test, log |
| `crates/vifast-abi/src/lib.rs` | 58 syscall, lỗi, quyền, `Message`, `user_layout`, `COPY_WINDOWS`, `BootInfo`, protocol pager/VFS |
| `crates/vifast-runtime/src/lib.rs` | boot, dispatch, timer/preempt, trait `Platform` |
| `crates/vifast-platform-{riscv64,aarch64,x86_64}/src/platform.rs` | MMU/trap/IRQ/SMP/user entry từng port |
| `crates/vifast-fs/src/lib.rs` | VF04 journal, inode, thư mục, link, fsck, cache |
| `crates/vifast-server/src/main.rs` | VFS server user-space + RPC |
| `crates/vifast-client/src/main.rs` | shell + toàn bộ demo/marker |
| `crates/vifast-root/src/main.rs` | supervisor: spawn/pager/exec/worker (chứa `smp_worker_c`) |
| `scripts/harness_spec.py` | **hợp đồng marker** (nguồn sự thật hành vi) |
| `scripts/test-qemu.py` | chạy QEMU, retry, phân loại, artifact |
| `scripts/build.sh` | build + sân test + ngân sách trang + `BUILD_ID` |
| `scripts/gate.sh` | cổng 10 bước + `--smoke` |
| `scripts/test-persist.py` | 7 pha persistence |
| `scripts/test-harness-spec.py` | 677 case âm |
| `docs/decisions/log.md` | nhật ký quyết định (rất nên đọc) |
| `docs/reviews/progress-g1.md` | nhật ký vòng gần nhất (206 KB; đọc **phần cuối**) |
| `docs/audits/open-items.md` | bảng K32..K75 + tồn đọng audit |

### 21.2 Hằng số then chốt (một bảng để tra)

```
MAX_OBJECTS 168 · MAX_TCBS 24 · MAX_CAPS_ROOT 128 · MAX_CAPS_TASK 16
MAX_MAPPINGS 64 · MAX_IOMMU_MAPPINGS 32 · MAX_QUEUE 8 · MAX_FDS 8
MAX_HARTS 4 · NOTIFICATION_WAITERS 6 · KERNEL_STATIC_BUDGET 566272
PAGE 4096 · MSG_MAX_WORDS 8 · IPC_BUFFER_WORDS 32 · USER_IO_MAX 256 · MAX_IO 128
FS: 512 B/block · 8192 block · 64 inode · tên 28 · path 128 · symlink depth 8 · MAX_TX_WRITES 16
VA: CODE_VA 0x10_0000_0000 · DATA_VA +0xa000 · STACK +0x1000..0x9000
    ROOT_ELF_BASE +0x20_0000 · BOOTINFO sau stack root +0x1000
Syscall 58 (0..57) · KError 22 (0..21) · Rights 4 bit
```

### 21.3 Lệnh vận hành (cheat sheet)

```bash
export PATH="$HOME/.cargo/bin:$PATH"

./scripts/diag.sh                    # trạng thái máy: HEAD, kích thước, ngân sách, QEMU mồ côi
./scripts/build.sh                   # build đầy đủ + host test + ngân sách trang
./scripts/gate.sh --smoke            # vòng lặp nhanh (~18 s khi mã không đổi)
./scripts/gate.sh                     # cổng 10 bước (~396 s)
./scripts/check-all.sh               # cổng "phát hành" (thêm test-destroy-smp 5×3)
./scripts/par-gate.sh --repeat 3     # chia việc 2 máy (x86 chạy remote qua SSH)

python3 scripts/test-qemu.py --arch riscv64 --repeat 1
python3 scripts/test-qemu.py --feature test-panic-hart1
python3 scripts/test-qemu.py --tcp-check --input-check --gpu-check
python3 scripts/test-persist.py
python3 scripts/test-harness-spec.py
python3 scripts/gen-docs.py && python3 scripts/check-docs.py
cargo test -p vifast-kernel -- --ignored        # fuzz dài

# đo tỉ lệ (KHÔNG retry, giữ serial, timeout thấp cho ca -smp 1)
VIFAST_NO_RETRY=1 VIFAST_TIMEOUT=150 VIFAST_KEEP_SERIAL=1 \
  python3 scripts/test-qemu.py --arch x86_64 --repeat 100

pkill -f qemu-system    # dọn trước khi kết luận
```

### 21.4 Mười câu tự kiểm để biết mình đã hiểu tệp này

1. Kernel biết gì về phần cứng? (chỉ trait `Arch`; mọi thứ khác ở `Platform`)
2. Vì sao không thể map một Frame vừa W vừa X? (bộ đếm `mapped_w/mapped_x` theo Frame)
3. Vì sao `Reply` phải kiểm `reply_epoch`? (chống reply tới nhầm client sau restart)
4. Vì sao VFS cần `seq` trong **label**? (chỉ có 3 data word, đã dùng hết cho fd/offset/len)
5. Vì sao `MAX_TCBS = 24` là ràng buộc kiến trúc? (không heap chung; `Kernel` static)
6. T33a thực sự là gì? (worker C thấy `GetTime` lùi ⇒ panic ⇒ root chờ ⇒ không `TEST_DONE`)
7. Vì sao `"PASS"` không đủ? (retry + `--strict` tắt)
8. Vì sao sửa một dòng tài liệu từng làm build 6,6 phút? (`BUILD_ID` hash cả `git diff`)
9. Hai chỗ nào trong tài liệu **sai** rõ nhất? (`07-arch-ports` về x86 SMP; `01-abi` 52 vs 58 syscall)
10. Nếu chỉ được sửa 3 thứ tuần này? (C: test không giết boot; A: mô hình boot trên host; E: thống kê)

### 21.5 Những gì người viết **chưa** kiểm (đừng trích dẫn như sự thật)

- Chưa chạy lại cổng QEMU trong phiên này (chỉ đọc số đã ghi + `cargo test` nhỏ).
- Chưa xác minh mọi dòng trong `docs/11-hien-tai/16-van-hanh.md` (55 KB runbook).
- Chưa kiểm `docs/design/*` (7 tệp) có còn khớp mã không.
- Chưa chạy `cargo test` toàn bộ để chốt con số host test (đếm bằng `grep #[test]`).
- Chưa đọc hết 90 bundle / 101 report (chỉ đọc mẫu + mục cuối `progress-g1`).
- Các con số "~396 s", "~18 s", tỉ lệ treo 0,3–0,5 % là **số đo đã ghi lại**, không
  phải số tôi tự đo.
- `ROOT_MODULE_*` VA và quan hệ với VA link của module (`0x1000010000`) tôi suy ra
  từ `build.sh` + `module.ld`; nên xác nhận bằng `nm` nếu cần chính xác tuyệt đối.

### 21.6 Cách cập nhật tệp này

- Tệp này **không** thay thế `harness_spec.py`; nếu mâu thuẫn, `harness_spec` thắng.
- Mỗi khi đóng một lỗi: sửa §15 (trạng thái), §14 (số), và thêm dòng vào
  `docs/decisions/log.md`.
- Mỗi khi đổi `MAX_*`/layout/ABI: sửa §5/§7/§10 **và** `COPY_WINDOWS`/const assert.
- Giữ tệp dưới ~2.500 dòng; nếu dài hơn, tách "phụ lục lịch sử" ra tệp riêng.

---

*Hết hồ sơ. Ảnh chụp: 2026-11-12, `HEAD 8077d71` + mẻ r323 chưa commit.*
