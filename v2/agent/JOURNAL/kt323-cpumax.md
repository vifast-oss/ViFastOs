# KT-323 — x86_64 `-cpu max`: đo trên tập tính năng rộng nhất (mục cuối "như K3" của K5')

Trạng thái: **DONE** (KT-323, 2026-11-22, nhánh `task/kt323-cpumax`, base `main` @ `f4debca0`).
Quyết định: **D2-201** (K5' bước 1), **D2-149** (soak 600 của K5'), **D2-216** (số đo soak bắt lỗi
thật), **D2-224** (K5' còn lại), **D2-198** (uỷ quyền commit nhánh task), **D2-05** (test đỏ trước),
**D2-07** (không retry), **D2-116** (đo hợp lệ). Luật viết phép kiểm:
[`k6-test-invariants.md`](../docs/k6-test-invariants.md) (bốn lớp lỗi của phép kiểm).

> **D2-225 không tồn tại** trong `v2/agent/DECISIONS.md` tại base `f4debca0` (mục cuối là **D2-224**).
> Nhiệm vụ KT-323 yêu cầu đọc D2-225 ⇒ đã đọc D2-216/D2-224 và **ghi lại** việc thiếu mục này thay vì
> tự suy ra nội dung (khuôn §16/§17 của `k5-x86_64-boot.md`).

**Không** tuyên bố K5' đạt.

## 1. Khảo sát

### 1.1 QEMU 11.1.3 có model `max` — model **mặc định** đo được là `qemu64`

```text
$ /opt/homebrew/bin/qemu-system-x86_64 --version
QEMU emulator version 11.1.3
Copyright (c) 2003-2026 Fabrice Bellard and the QEMU Project Developers

$ /opt/homebrew/bin/qemu-system-x86_64 -cpu help | grep -nE '^  (max|qemu64|qemu64-v1|qemu32|base) '
184:  qemu32                (alias configured by machine type)
186:  qemu64                (alias configured by machine type)
187:  qemu64-v1             QEMU Virtual CPU version 2.5+
188:  base                  base CPU model type with no features enabled
189:  max                   Enables all features supported by the accelerator in the current host
```

`-cpu help` **không** in model mặc định. Vì vậy model mặc định được **đo** bằng QOM tree
(`-M q35 -S`, monitor qua unix socket, `info qom-tree`) — mỗi lần một QEMU ngắn, không boot ảnh:

```text
không truyền -cpu :  /device[0] (qemu64-x86_64-cpu)
-cpu qemu64       :  /device[0] (qemu64-x86_64-cpu)
-cpu max          :  /device[0] (max-x86_64-cpu)
```

⇒ (a) mọi số đo cũ của runner/soak — vốn **không** truyền `-cpu` — chạy trên **`qemu64`**; (b) `-cpu max`
là model **khác thật** ở tầng QEMU; (c) giá trị `V2_X64_CPU` **tới** QEMU, không chỉ được in ra
(xem thêm đột biến (iii) ở §5).

`max` = "all features supported by the accelerator in the current host" — trên máy này accelerator là
**TCG** (host arm64), nên `max` = tập **TCG** hỗ trợ cho x86_64, **không** phải "mọi tính năng của CPU
thật". Trong `-cpu help` của bản 11.1.3 có các cờ liên quan: `x2apic`, `la57`, `umip`, `smep`, `smap`,
`fsgsbase`, `pcid`, `invpcid`, `xsave`, `avx`/`avx2`/`avx512f`, `svm`/`vmx`, `tsc-deadline`, `pku`,
`pdpe1gb`, `rdtscp`.

### 1.2 Kernel dùng gì có thể **khác** dưới `-cpu max`

| # | Khác biệt tiềm năng của `max` | Ảnh có dùng? | Bằng chứng |
|---|---|---|---|
| 1 | **CPUID** (leaf mới, feature bit) | **Không** — `grep -rn cpuid v2/kernel --include=*.rs` = **0** dòng | kernel không có lệnh `cpuid`; tính năng chỉ-CPUID không thể đổi hành vi |
| 2 | **x2APIC** (MSR `IA32_APIC_BASE` bit 10 `EXTD`) | **Không** — chỉ bật bit `ENABLE` (11) nếu thiếu, **không** đặt `EXTD` | `main.rs:921`, `smp.rs:924`; QEMU reset `EXTD=0` ⇒ vẫn xAPIC MMIO; `# apic: id=0 version=0x14 svr=0x1ff` **giống hệt** hai model |
| 3 | **MSR khác** (`IA32_PAT`/`STAR`/`LSTAR`…, `tsc-deadline`) | **Không** — chỉ **ghi** `IA32_APIC_BASE`; `EFER` chỉ **đọc** | `main.rs:1228` (đọc `EFER`); `control.rs` là hằng; không `wrmsr` nào khác |
| 4 | **SMEP/SMAP/OSXSAVE/PCIDE/LA57** (`max` phơi ra) | **Không** — CR4 = **0x20** (chỉ `PAE`) | log: `… cr4=0x0000000000000020 …`; `# smp: boot block … cr4=0x20` |
| 5 | **UMIP** (`max` có) — chặn `sgdt/sidt/sldt/str/smsw` ở CPL3 | **Không** — stub ring 3 chỉ `mov`/`inc qword [rcx]`/`jmp` + `int 0x80` | `vifast-kbin-x86_64/src/umode.rs:71-81` |
| 6 | **AVX/XSAVE** | **Không** | không có `xsave`/`xgetbv` trong `v2/kernel` |
| 7 | **Timer/PIT/IOAPIC/virtio** | Mô hình **thiết bị** của QEMU — không phụ thuộc model CPU | `# timer: …`, `# blk: device … features=…`, `# net: mac … features=…` **giống hệt** hai model |
| 8 | **Bảng trang 1 GiB / 5 mức** | **Không** — 4 mức, lá 2 MiB | `boot.rs` (`LARGE_PAGE_BYTES = 1 << 21`, `PD_ENTRIES = 512`) |

**Kỳ vọng (đỏ-trước nếu sai)**: `-cpu max` **không** đổi được hành vi kiến trúc mà ảnh dùng ⇒ kỳ vọng
TAP 30/30 **xanh**; chỉ các **số thời gian/thứ tự tick** (`waited_us`, `ticks`, `hz`, `runs`, thứ tự
hart chọn) được phép lệch. Nếu **đỏ** ở bất kỳ bất biến nào thì đó là **phát hiện** (kernel phụ thuộc
model) — phải ghi nguyên nhân, **không** che.

### 1.3 Chỗ nào **giả định** `qemu64`?

1. **Hai script không truyền `-cpu`** ⇒ phụ thuộc model mặc định của QEMU. Kernel **không** so sánh tên
   model/CPUID, nên đây là giả định **của harness**, không phải của ảnh.
2. **Bằng chứng không tự mô tả**: log runner/soak trước KT-323 **không** ghi model ⇒ không biết một lần
   đo chạy model nào. Đây là lỗ hổng chính mà KT-323 vá (dòng `# infra: cpu=…`) — điều kiện của
   AGENTS §6.3 ("mỗi phép đo lưu khóa đầu vào … config").
3. Kernel **không** kiểm/phát hiện tính năng (không CPUID): mọi thứ nó dùng là tập **nền kiến trúc**
   (long mode, xAPIC MMIO, IOAPIC, PIT, page 2 MiB, gate `int 0x80`, `iretq`, SIPI real mode). Vì vậy
   "xanh trên `max`" nghĩa là "**không vỡ** với tập tính năng rộng", **không** phải "**dùng** AVX/x2APIC".

## 2. Thiết kế

- **Một biến, hai script, cùng ngữ nghĩa**: `V2_X64_CPU` (mặc định **`qemu64`**).
  - `run-x86_64.sh`: `X64_CPU="${V2_X64_CPU:-qemu64}"`; `QEMU_MACHINE=(… -cpu "$X64_CPU" …)` — **một
    chỗ quyết định**, dùng cho boot thường **và** ba ca âm (b)(k)(o) (ca âm phải chạy **cùng** model;
    nếu không nó không còn là "cùng ảnh, cùng máy").
  - `soak-x86_64.sh`: cùng dòng gán; `QEMU_ARGS=(… -cpu "$X64_CPU" …)` dùng cho **probe** và mọi boot.
- **Mặc định = `qemu64`** (model QEMU chọn cho `-M q35` — đo ở §1.1), nên mọi số đo cũ **tái lập
  được** (AGENTS §6.3). `-cpu max` là **tuỳ chọn**, không phải mặc định.
- **Bằng chứng tự mô tả**: `# infra: cpu=$X64_CPU …` ở **cả hai** script; soak thêm `-cpu $X64_CPU` vào
  dòng `# cấu hình:`; runner in `# qemu <ver>  cpu <model>` ở đầu.
- **Fail-closed**: model sai ⇒ QEMU thoát ngay (`unable to find CPU model '<tên>'`) ⇒ runner **ĐỎ**
  (log boot không có TAP), soak **TỪ CHỐI ĐO** (exit 3, không đếm boot nào) + in dòng chẩn đoán
  `QEMU TỪ CHỐI model CPU` (chỉ **đọc** log, không mở thêm QEMU).
- **Không** đổi `-M`, `-smp`, thiết bị, timeout, ngưỡng, bất kỳ bất biến TAP/soak nào; **không** sửa
  kernel ⇒ BUILD_ID của ảnh **không đổi** (`build=05c16bca7b4e006f text=69632`, §3.3).

## 3. Đo — runner

Hai lần chạy **cùng một ảnh** (`build=05c16bca7b4e006f text=69632`), khác **duy nhất** `V2_X64_CPU`.
Mỗi lần: build + 4 boot QEMU (boot thường + ba ca âm (b)(k)(o)) + 26 ca âm log + 1 ca dương KT-313.

### 3.1 `V2_X64_CPU=qemu64` (mặc định — model cũ, số đo cũ phải tái lập)

```text
$ KT257_LOGDIR=/tmp/kt323-qemu64 bash v2/test/boot/run-x86_64.sh
```

Runner stdout (nguyên văn, đầu và cuối):

```text
# qemu 11.1.3  cpu qemu64  objdump /Users/mps/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/llvm-objdump
# target x86_64-unknown-none  image /tmp/kt323-s1/v2/kernel/target/x86_64-unknown-none/release/vifast-kbin-x86_64
# build ảnh x86_64 (x86_64-unknown-none, release)
ok 1 - elf: entry=_start, .text tại 0x100000, multiboot2 + PVH note đúng
ok 2 - boot 1: TAP 30/30 + 5 dòng '# exc:' + marker 1 lần + halt sạch
…
ok 29 - ca dương (KT-313): picks=[2, 4, 1, 1] first=[a, a, u, c] diff=1 ⇒ XANH
# ca âm: 26/26 ĐỎ đúng như phải (+ 1 ca dương KT-313)
# 20/20 kiểm sạch (boot=1, tests=30, exc=5)
# infra: cpu=qemu64 guard_log=/tmp/kt323-qemu64/qemu-guard.log qemu_own=0
```

Log QEMU nguyên văn (`/tmp/kt323-qemu64/boot-1.log`):

```text
c[?7l[2J[0mSeaBIOS (version rel-1.17.0-0-gb52ca86e094d-prebuilt.qemu.org)


iPXE (http://ipxe.org) 00:02.0 CA00 PCI2.10 PnP PMM+06FD1AA0+06F31AA0 CA00
Press Ctrl-B to configure iPXE (PCI 00:02.0)...                                                                               


Booting from ROM..
TAP version 13
1..30
# conf image: build=05c16bca7b4e006f text=69632
# viFastOs x86_64: long mode cr0=0x0000000080000011 cr4=0x0000000000000020 cr3=0x0000000000117000 efer=0x0000000000000d00 lma=1
# viFastOs x86_64: boot eax=0x00000000 ebx=0x00001580 pvh_magic=0x336ec578 pvh_ok=1
# viFastOs x86_64: serial com1=0x3f8 divisor=1 lcr=0x03
# viFastOs x86_64: identity map 0x0..0x40000000 pages=512 pml4=0x117000
# viFastOs x86_64: mmio map 0x80000000..0x100000000 pages=1024 pcd=1
# viFastOs x86_64: gdt base=0x114370 limit=55 tr=0x28 tss=0x1400bc ist1=0x11d000
ok 1 - exc: gdt loaded
# viFastOs x86_64: idt base=0x13f0b0 limit=4095 entries=256 df_ist=1
ok 2 - exc: idt loaded
# exc: vec=3 err=0x0 rip=0x100421 cs=0x8 rflags=0x2
ok 3 - exc: int3
# exc: vec=6 err=0x0 rip=0x100430 cs=0x8 rflags=0x10002
ok 4 - exc: ud2
# exc: vec=14 err=0x0 rip=0x100440 cs=0x8 rflags=0x10002
# exc: cr2=0x700000000000
ok 5 - exc: page fault
# exc: vec=13 err=0x38 rip=0x100454 cs=0x8 rflags=0x10002
ok 6 - exc: gp
# exc: vec=254 err=0x0 rip=0x100462 cs=0x8 rflags=0x2
ok 7 - exc: unknown
# apic: id=0 version=0x14 svr=0x1ff
ok 8 - apic: lapic id
# apic: ioapic id=0 ver=0x20 entries=24
ok 9 - apic: ioapic
# timer: lapic count0=978000 count1=941000 runs=1 ioapic_route=1 div_ok=1 nominal_hz=100 pit=0x2e78->0x29da
# timer: hz=41 ticks=21 window_pit=604126
ok 10 - timer: ticks
# timer: eoi=21 ticks=21 spurious=0
ok 11 - timer: eoi
# viFastOs x86_64: exc count=5 last_vec=254
# umode: iretq entry=0x4000000000 sp=0x4000002000 rflags=0x202 cs=0x1b ss=0x23 code_va=0x4000000000 code_pa=0x115000 code_bytes=95 stack_va=0x4000001000 stack_pa=0x122000 syscall_stack=0x12b000
ok 12 - umode: ring3 entered
# umode: int80 nr=12 cs=0x1b rip=0x4000000025 user=1
ring3 says hi
ok 13 - umode: syscall handled
# umode: ring3 ran nr=12 ret=14
# umode: int80 ret error=0 value=14 printed=14 ptr=0x4000000039 len=14
ok 14 - umode: returned to ring3
# umode: exit code=0 nr=14
# umode: done
# pci: ecam base=0xb0000000 buses=0..255 functions=7
# pci: dev 00:00.0 vendor=0x8086 device=0x29c0 class=0x0600 bridge hdr=0x00
# pci: dev 00:01.0 vendor=0x1234 device=0x1111 class=0x0300 display hdr=0x00
# pci: dev 00:02.0 vendor=0x1af4 device=0x1000 class=0x0200 network hdr=0x00
# pci: dev 00:03.0 vendor=0x1af4 device=0x1001 class=0x0100 storage hdr=0x00
# pci: dev 00:1f.0 vendor=0x8086 device=0x2918 class=0x0601 bridge hdr=0x80
# pci: dev 00:1f.2 vendor=0x8086 device=0x2922 class=0x0106 storage hdr=0x80
# pci: dev 00:1f.3 vendor=0x8086 device=0x2930 class=0x0c05 other hdr=0x80
ok 15 - pci: ecam
# pci: net 00:02.0 bar=4 kind=mem64 base=0xfe000000 size=0x4000 command=0x0107 caps=4
# net: mac=52:54:00:12:34:56 mtu=65535 status=0x0001 queues=3 rx=16 tx=16 features=0x0000010130bf8024 driver=0x0000000100000020
ok 16 - net: mac
# net: arp gw=10.0.2.2 mac=52:55:0a:00:02:02 sent=1 recv=1 elapsed_us=2346 errors=0 timeouts=0
ok 17 - net: arp
# smp: arena pa=0x141000 bytes=557056 expect=557056 layout=Ok(()) ok=1
# smp: trampoline base=0x8000 bytes=4096 src=0x110000 magic=0x00000000534d5032 ok=1
# smp: boot block pml4=0x117000 stack=0x163000 hart=1 entry=0x108b0a idt=0x13f0b0/0xfff cr0=0x80000011 cr4=0x20 efer=0x900 lapic=0xfee00000 bsp_id=0 dest=1
# smp: icr init dest=1 low=0xc500 high=0x01000000 sipi vector=0x08 low=0x0608
# smp: hart=1 online
# smp: hart=1 lapic id=1 base=0xfee00000 svr=0x1ff tpr=0 sp=0x162ea0 slot=1 stack=0x153000-0x163000
# smp: hart=1 tss base=0x140124 tr=0x28 type=0x8b ist1=0x11e000 rsp0=0x163000
# smp: cpu_on hart=1 online=1 waited_us=6212 lapic_id=1 sp=0x162ea0 slot=Some(1) init_ok=1 sipi_ok=1/1
# smp: icr init dest=2 low=0xc500 high=0x02000000 sipi vector=0x08 low=0x0608
# smp: hart=2 online
# smp: hart=2 lapic id=2 base=0xfee00000 svr=0x1ff tpr=0 sp=0x173ea0 slot=2 stack=0x164000-0x174000
# smp: hart=2 tss base=0x14018c tr=0x28 type=0x8b ist1=0x11f000 rsp0=0x174000
# smp: cpu_on hart=2 online=1 waited_us=3416 lapic_id=2 sp=0x173ea0 slot=Some(2) init_ok=1 sipi_ok=1/1
# smp: icr init dest=3 low=0xc500 high=0x03000000 sipi vector=0x08 low=0x0608
# smp: hart=3 online
# smp: hart=3 lapic id=3 base=0xfee00000 svr=0x1ff tpr=0 sp=0x184ea0 slot=3 stack=0x175000-0x185000
# smp: hart=3 tss base=0x1401f4 tr=0x28 type=0x8b ist1=0x120000 rsp0=0x185000
# smp: cpu_on hart=3 online=1 waited_us=2649 lapic_id=3 sp=0x184ea0 slot=Some(3) init_ok=1 sipi_ok=1/1
# smp: online mask=0xf expect=0xf harts=4 booted=3 slots=[Some(0), Some(1), Some(2), Some(3)]
ok 18 - smp: 4 harts online
# smp: tss bases=[0x1400bc, 0x140124, 0x14018c, 0x1401f4] ist1=[0x11d000, 0x11e000, 0x11f000, 0x120000] distinct=Ok(()) ist1_distinct=Ok(()) tr=0x28 type=0x8b ok=1
ok 19 - smp: tss per-hart
# smp: ipi send dest=1 vector=0x30 low=0x4030 high=0x01000000
# smp: ipi recv=1 intid=0x30
# smp: ipi sent=1 count=1 waited_us=1202 ok=1
ok 20 - smp: ipi delivered
# smp: hart=1 alive
# smp: report hart=1 done=1 waited_us=105139 alive=1
# smp: hart=2 alive
# smp: report hart=2 done=1 waited_us=4966 alive=1
# smp: hart=3 alive
# smp: report hart=3 done=1 waited_us=10444 alive=1
# smp: hart=0 alive
# smp: online mask=0xf alive=0xf expect=0xf harts=4 ap_ticks=11 waited_online_us=12277 waited_alive_us=120549
ok 21 - smp: all harts report
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: sleep task=b until=4 woke=1
# sched: sleep task=c until=4 woke=2
# sched: sleep task=a until=5 woke=3
# sched: sleep task=u until=4 woke=4
# sched: hart=2 picked=b n=2
# sched: hart=1 picked=u n=3
# sched: hart=3 picked=c n=4
# sched: ticks=10 tasks=4
# sched: runs a=429138 b=520939 c=186814 switches=9 ab=1 ba=2 a_rsp=0x12f208 b_rsp=0x133208 u_rsp=0x13b210 c_rsp=0x137208
# sched: umode task ran n=13675348 ring=3 preempted=2 resumed=1
# sched: umode counts first=2188818 last=3051866 cs=0x1b kernel_stack=0x13b2c0
# sched: umode progress first=2188818 last=3051866 delta=863048
# sched: hart=0 picked=a n=1
# sched: multi-hart harts=4 rounds=11 picks=[3, 3, 3, 2] first=[a, u, b, c] diff=8 double=0
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
ok 24 - sched: umode task
ok 25 - sched: multi-hart
# pci: blk 00:03.0 vendor=0x1af4 device=0x1001 bar=4 kind=mem64 base=0xfe004000 size=0x4000 command=0x0107 caps=4
# blk: device vendor=0x1af4 device=0x1001 bar=4 base=0xfe004000 size=0x4000 net_base=0xfe000000 bar_distinct=1 capacity=8 blk_size=512 size_max=0 seg_max=254 queue=8 max=256 status=0x0f features=0x0000010130007e54 driver=0x0000000100000200 flush=1
ok 26 - blk: device
# blk: sector0 hex=5649464153544f532d4b543238382d53 ascii="VIFASTOS-KT288-S" magic=1 capacity=8 status=0x0f dma=0x13c000 desc=0x13c000 avail=0x13c080 used=0x13c098 requests=1 completions=1 errors=0 timeouts=0
ok 27 - blk: sector0
# com: lock ok=1 contended=0
ok 28 - com: lock
# sched: prio first=b viol=0 ticks=8 hard=0 picks=[2, 2, 2, 2]
# sched: sleep wakes=4 viol=0 ticks=8 picks=[2, 2, 2, 2]
ok 29 - sched: priority
ok 30 - sched: sleep wake
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

### 3.2 `V2_X64_CPU=max` (tập tính năng rộng nhất — **mục đích của KT-323**)

```text
$ KT257_LOGDIR=/tmp/kt323-max V2_X64_CPU=max bash v2/test/boot/run-x86_64.sh
```

**Kết quả: XANH — không phát hiện lỗi kernel phụ thuộc model.** Runner stdout (nguyên văn, đầu và cuối):

```text
# qemu 11.1.3  cpu max  objdump /Users/mps/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/llvm-objdump
# target x86_64-unknown-none  image /tmp/kt323-s1/v2/kernel/target/x86_64-unknown-none/release/vifast-kbin-x86_64
# build ảnh x86_64 (x86_64-unknown-none, release)
ok 1 - elf: entry=_start, .text tại 0x100000, multiboot2 + PVH note đúng
ok 2 - boot 1: TAP 30/30 + 5 dòng '# exc:' + marker 1 lần + halt sạch
…
ok 29 - ca dương (KT-313): picks=[2, 4, 1, 1] first=[a, a, u, c] diff=1 ⇒ XANH
# ca âm: 26/26 ĐỎ đúng như phải (+ 1 ca dương KT-313)
# 20/20 kiểm sạch (boot=1, tests=30, exc=5)
# infra: cpu=max guard_log=/tmp/kt323-max/qemu-guard.log qemu_own=0
```

Log QEMU nguyên văn (`/tmp/kt323-max/boot-1.log`):

```text
c[?7l[2J[0mSeaBIOS (version rel-1.17.0-0-gb52ca86e094d-prebuilt.qemu.org)


iPXE (http://ipxe.org) 00:02.0 CA00 PCI2.10 PnP PMM+06FD1AA0+06F31AA0 CA00
Press Ctrl-B to configure iPXE (PCI 00:02.0)...                                                                               


Booting from ROM..
TAP version 13
1..30
# conf image: build=05c16bca7b4e006f text=69632
# viFastOs x86_64: long mode cr0=0x0000000080000011 cr4=0x0000000000000020 cr3=0x0000000000117000 efer=0x0000000000000d00 lma=1
# viFastOs x86_64: boot eax=0x00000000 ebx=0x00001580 pvh_magic=0x336ec578 pvh_ok=1
# viFastOs x86_64: serial com1=0x3f8 divisor=1 lcr=0x03
# viFastOs x86_64: identity map 0x0..0x40000000 pages=512 pml4=0x117000
# viFastOs x86_64: mmio map 0x80000000..0x100000000 pages=1024 pcd=1
# viFastOs x86_64: gdt base=0x114370 limit=55 tr=0x28 tss=0x1400bc ist1=0x11d000
ok 1 - exc: gdt loaded
# viFastOs x86_64: idt base=0x13f0b0 limit=4095 entries=256 df_ist=1
ok 2 - exc: idt loaded
# exc: vec=3 err=0x0 rip=0x100421 cs=0x8 rflags=0x2
ok 3 - exc: int3
# exc: vec=6 err=0x0 rip=0x100430 cs=0x8 rflags=0x10002
ok 4 - exc: ud2
# exc: vec=14 err=0x0 rip=0x100440 cs=0x8 rflags=0x10002
# exc: cr2=0x700000000000
ok 5 - exc: page fault
# exc: vec=13 err=0x38 rip=0x100454 cs=0x8 rflags=0x10002
ok 6 - exc: gp
# exc: vec=254 err=0x0 rip=0x100462 cs=0x8 rflags=0x2
ok 7 - exc: unknown
# apic: id=0 version=0x14 svr=0x1ff
ok 8 - apic: lapic id
# apic: ioapic id=0 ver=0x20 entries=24
ok 9 - apic: ioapic
# timer: lapic count0=978000 count1=943000 runs=1 ioapic_route=1 div_ok=1 nominal_hz=100 pit=0x2e78->0x29bc
# timer: hz=36 ticks=19 window_pit=622171
ok 10 - timer: ticks
# timer: eoi=19 ticks=19 spurious=0
ok 11 - timer: eoi
# viFastOs x86_64: exc count=5 last_vec=254
# umode: iretq entry=0x4000000000 sp=0x4000002000 rflags=0x202 cs=0x1b ss=0x23 code_va=0x4000000000 code_pa=0x115000 code_bytes=95 stack_va=0x4000001000 stack_pa=0x122000 syscall_stack=0x12b000
ok 12 - umode: ring3 entered
# umode: int80 nr=12 cs=0x1b rip=0x4000000025 user=1
ring3 says hi
ok 13 - umode: syscall handled
# umode: ring3 ran nr=12 ret=14
# umode: int80 ret error=0 value=14 printed=14 ptr=0x4000000039 len=14
ok 14 - umode: returned to ring3
# umode: exit code=0 nr=14
# umode: done
# pci: ecam base=0xb0000000 buses=0..255 functions=7
# pci: dev 00:00.0 vendor=0x8086 device=0x29c0 class=0x0600 bridge hdr=0x00
# pci: dev 00:01.0 vendor=0x1234 device=0x1111 class=0x0300 display hdr=0x00
# pci: dev 00:02.0 vendor=0x1af4 device=0x1000 class=0x0200 network hdr=0x00
# pci: dev 00:03.0 vendor=0x1af4 device=0x1001 class=0x0100 storage hdr=0x00
# pci: dev 00:1f.0 vendor=0x8086 device=0x2918 class=0x0601 bridge hdr=0x80
# pci: dev 00:1f.2 vendor=0x8086 device=0x2922 class=0x0106 storage hdr=0x80
# pci: dev 00:1f.3 vendor=0x8086 device=0x2930 class=0x0c05 other hdr=0x80
ok 15 - pci: ecam
# pci: net 00:02.0 bar=4 kind=mem64 base=0xfe000000 size=0x4000 command=0x0107 caps=4
# net: mac=52:54:00:12:34:56 mtu=65535 status=0x0001 queues=3 rx=16 tx=16 features=0x0000010130bf8024 driver=0x0000000100000020
ok 16 - net: mac
# net: arp gw=10.0.2.2 mac=52:55:0a:00:02:02 sent=1 recv=1 elapsed_us=2273 errors=0 timeouts=0
ok 17 - net: arp
# smp: arena pa=0x141000 bytes=557056 expect=557056 layout=Ok(()) ok=1
# smp: trampoline base=0x8000 bytes=4096 src=0x110000 magic=0x00000000534d5032 ok=1
# smp: boot block pml4=0x117000 stack=0x163000 hart=1 entry=0x108b0a idt=0x13f0b0/0xfff cr0=0x80000011 cr4=0x20 efer=0x900 lapic=0xfee00000 bsp_id=0 dest=1
# smp: icr init dest=1 low=0xc500 high=0x01000000 sipi vector=0x08 low=0x0608
# smp: hart=1 online
# smp: hart=1 lapic id=1 base=0xfee00000 svr=0x1ff tpr=0 sp=0x162ea0 slot=1 stack=0x153000-0x163000
# smp: hart=1 tss base=0x140124 tr=0x28 type=0x8b ist1=0x11e000 rsp0=0x163000
# smp: cpu_on hart=1 online=1 waited_us=4586 lapic_id=1 sp=0x162ea0 slot=Some(1) init_ok=1 sipi_ok=1/1
# smp: icr init dest=2 low=0xc500 high=0x02000000 sipi vector=0x08 low=0x0608
# smp: hart=2 online
# smp: hart=2 lapic id=2 base=0xfee00000 svr=0x1ff tpr=0 sp=0x173ea0 slot=2 stack=0x164000-0x174000
# smp: hart=2 tss base=0x14018c tr=0x28 type=0x8b ist1=0x11f000 rsp0=0x174000
# smp: cpu_on hart=2 online=1 waited_us=3 lapic_id=2 sp=0x173ea0 slot=Some(2) init_ok=1 sipi_ok=1/1
# smp: icr init dest=3 low=0xc500 high=0x03000000 sipi vector=0x08 low=0x0608
# smp: hart=3 online
# smp: hart=3 lapic id=3 base=0xfee00000 svr=0x1ff tpr=0 sp=0x184ea0 slot=3 stack=0x175000-0x185000
# smp: hart=3 tss base=0x1401f4 tr=0x28 type=0x8b ist1=0x120000 rsp0=0x185000
# smp: cpu_on hart=3 online=1 waited_us=3 lapic_id=3 sp=0x184ea0 slot=Some(3) init_ok=1 sipi_ok=1/1
# smp: online mask=0xf expect=0xf harts=4 booted=3 slots=[Some(0), Some(1), Some(2), Some(3)]
ok 18 - smp: 4 harts online
# smp: tss bases=[0x1400bc, 0x140124, 0x14018c, 0x1401f4] ist1=[0x11d000, 0x11e000, 0x11f000, 0x120000] distinct=Ok(()) ist1_distinct=Ok(()) tr=0x28 type=0x8b ok=1
ok 19 - smp: tss per-hart
# smp: ipi send dest=1 vector=0x30 low=0x4030 high=0x01000000
# smp: ipi recv=1 intid=0x30
# smp: ipi sent=1 count=1 waited_us=789 ok=1
ok 20 - smp: ipi delivered
# smp: hart=1 alive
# smp: report hart=1 done=1 waited_us=127358 alive=1
# smp: hart=2 alive
# smp: report hart=2 done=1 waited_us=488 alive=1
# smp: hart=3 alive
# smp: report hart=3 done=1 waited_us=55238 alive=1
# smp: hart=0 alive
# smp: online mask=0xf alive=0xf expect=0xf harts=4 ap_ticks=6 waited_online_us=4592 waited_alive_us=183084
ok 21 - smp: all harts report
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: sleep task=b until=4 woke=1
# sched: sleep task=c until=4 woke=2
# sched: sleep task=a until=5 woke=3
# sched: sleep task=u until=4 woke=4
# sched: hart=1 picked=b n=2
# sched: hart=2 picked=u n=3
# sched: hart=3 picked=c n=4
# sched: ticks=10 tasks=4
# sched: runs a=612931 b=451440 c=120614 switches=9 ab=1 ba=2 a_rsp=0x12f208 b_rsp=0x133208 u_rsp=0x13b210 c_rsp=0x137208
# sched: umode task ran n=12655625 ring=3 preempted=2 resumed=1
# sched: umode counts first=996429 last=3740347 cs=0x1b kernel_stack=0x13b2c0
# sched: umode progress first=996429 last=3740347 delta=2743918
# sched: hart=0 picked=a n=1
# sched: multi-hart harts=4 rounds=9 picks=[3, 2, 2, 2] first=[a, b, u, c] diff=6 double=0
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
ok 24 - sched: umode task
ok 25 - sched: multi-hart
# pci: blk 00:03.0 vendor=0x1af4 device=0x1001 bar=4 kind=mem64 base=0xfe004000 size=0x4000 command=0x0107 caps=4
# blk: device vendor=0x1af4 device=0x1001 bar=4 base=0xfe004000 size=0x4000 net_base=0xfe000000 bar_distinct=1 capacity=8 blk_size=512 size_max=0 seg_max=254 queue=8 max=256 status=0x0f features=0x0000010130007e54 driver=0x0000000100000200 flush=1
ok 26 - blk: device
# blk: sector0 hex=5649464153544f532d4b543238382d53 ascii="VIFASTOS-KT288-S" magic=1 capacity=8 status=0x0f dma=0x13c000 desc=0x13c000 avail=0x13c080 used=0x13c098 requests=1 completions=1 errors=0 timeouts=0
ok 27 - blk: sector0
# com: lock ok=1 contended=0
ok 28 - com: lock
# sched: prio first=b viol=0 ticks=8 hard=0 picks=[2, 2, 2, 2]
# sched: sleep wakes=4 viol=0 ticks=8 picks=[2, 2, 2, 2]
ok 29 - sched: priority
ok 30 - sched: sleep wake
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

### 3.3 A/B với bản **trước** khi sửa (không truyền `-cpu`): mặc định cũ = `qemu64`

Chạy lại runner **nguyên bản** (base `f4debca0`, chưa có `V2_X64_CPU`) với `KT257_LOGDIR=/tmp/kt323-base`:
**20/20 kiểm sạch, exit 0** (TAP 30/30, 5 `# exc:`, marker 1 lần, halt sạch). So sánh log boot 1 với
lần chạy `V2_X64_CPU=qemu64` ở §3.1:

- `# conf image: build=05c16bca7b4e006f text=69632` — **giống** (kernel không đổi ⇒ danh tính ảnh không đổi);
- **toàn bộ** dòng `ok …`/`not ok …` — **giống tuyệt đối** (30 dòng `ok`);
- 92 dòng `#` mỗi bên, **19 dòng khác** — **đều** là số thời gian/thứ tự tick, cùng bốn lớp như §3.4
  (`timer` 3, `smp` 8, `sched` 7, `net` 1). Ví dụ nguyên văn hai bên:

```text
# timer: hz=42 ticks=22 window_pit=611949                 (mặc định cũ)
# timer: hz=41 ticks=21 window_pit=604126                 (V2_X64_CPU=qemu64)
# smp: cpu_on hart=1 online=1 waited_us=5184 … init_ok=1 sipi_ok=1/1   (mặc định cũ)
# smp: cpu_on hart=1 online=1 waited_us=6212 … init_ok=1 sipi_ok=1/1   (V2_X64_CPU=qemu64)
# sched: hart=3 picked=b n=2                             (mặc định cũ)
# sched: hart=2 picked=b n=2                             (V2_X64_CPU=qemu64)
```

⇒ **`-cpu qemu64` tường minh ≡ model mặc định cũ** ở mọi dòng bằng chứng; khác biệt chỉ là **nhiễu
thời gian/thứ tự** — đúng loại khác biệt xuất hiện giữa hai lần chạy **cùng** model (số dòng khác cũng
là 19). Đây là cơ sở để nói mặc định mới **không phá** số đo cũ (AGENTS §6.3).

### 3.4 `qemu64` vs `max` — khác biệt **chỉ** là thời gian/thứ tự

Cùng ảnh, cùng runner, cùng ca; khác **duy nhất** `-cpu`:

| Lớp | Kết quả |
|---|---|
| TAP (30 dòng `ok 1..30`) | **giống tuyệt đối** — `diff` trên các dòng `ok` rỗng |
| `# exc:` (5 dòng: 3, 6, 14 + `cr2`, 13/err 0x38, 254) | **giống tuyệt đối** (cả `rip`, `cs=0x8`, `rflags`) |
| `# apic:` LAPIC | **giống tuyệt đối** — `id=0 version=0x14 svr=0x1ff` |
| `# apic: ioapic` | **giống tuyệt đối** — `id=0 ver=0x20 entries=24` |
| `# long mode … cr0/cr4/efer` + `lma=1` | **giống tuyệt đối** — `cr4=0x20` (không bật thêm gì) |
| `# smp: tss bases=[…] ist1=[…] tr=0x28 type=0x8b` | **giống tuyệt đối** (bốn TSS/IST rời nhau) |
| `# blk: device` + `# blk: sector0` | **giống tuyệt đối** (`capacity=8`, `status=0x0f`, `errors=0 timeouts=0`) |
| `# net: mac … features=…` | **giống tuyệt đối** |
| `# sched: prio` + `# sched: sleep` + `# sched: ticks=10 tasks=4` + `# com: lock` | **giống tuyệt đối** |
| kết luận runner | 20/20 kiểm sạch, exit 0, **cả hai** model |

92 dòng `#` mỗi bên, **19 dòng khác** (73 dòng giống hệt), phân bố:

| Nhóm | Số dòng | Trường khác | Bản chất |
|---|---:|---|---|
| `timer` | 3 | `count1`, `hz`, `ticks`, `window_pit`, `pit` | thời gian |
| `smp` | 8 | `waited_us` (3× `cpu_on`, IPI, 3× `report`), `ap_ticks`/`waited_online_us`/`waited_alive_us` | thời gian |
| `sched` | 7 | 2 dòng `hart=N picked=… n=` (thứ tự), `runs`, `umode … n=`, `umode counts/progress`, `multi-hart rounds/picks/first/diff` | thời gian/thứ tự |
| `net` | 1 | `arp … elapsed_us` | thời gian |

Ví dụ nguyên văn (hai model):

```text
# timer: hz=41 ticks=21 window_pit=604126                                    (qemu64)
# timer: hz=36 ticks=19 window_pit=622171                                    (max)
# net: arp gw=10.0.2.2 mac=52:55:0a:00:02:02 sent=1 recv=1 elapsed_us=2346 errors=0 timeouts=0   (qemu64)
# net: arp gw=10.0.2.2 mac=52:55:0a:00:02:02 sent=1 recv=1 elapsed_us=2273 errors=0 timeouts=0   (max)
# sched: multi-hart harts=4 rounds=11 picks=[3, 3, 3, 2] first=[a, u, b, c] diff=8 double=0        (qemu64)
# sched: multi-hart harts=4 rounds=9 picks=[3, 2, 2, 2] first=[a, b, u, c] diff=6 double=0         (max)
```

Hai dòng `multi-hart` **khác giá trị nhưng cùng đạt bất biến**: `harts=4`, mọi `picks ≥ 1`, `diff ≥ 1`,
`double=0` — đúng bài học `k6-test-invariants.md` §1 (bất biến không được phụ thuộc thứ tự tick; `first`
chỉ là chẩn đoán). Vì vậy **không** có dòng nào đỏ vì `max`.

## 4. Soak smoke 60 boot trên `max`

```text
$ V2_X64_CPU=max SOAK_X86_64_LOGDIR=/tmp/kt323-max-soak bash v2/test/boot/soak-x86_64.sh --smoke 60 3
```

```text
# cấu hình: -M q35 -cpu max -m 128M -nographic -no-reboot -smp 4; KHÔNG icount (D2-74 c)
# probe: '# conf image:' khớp ELF (build=05c16bca7b4e006f text=69632)
# probe: log đạt mọi bất biến của một boot sạch (TAP 30/30 + marker + halt)
…
# infra after: qemu_total=2 qemu_ppid1=0 qemu_own=0 qemu_foreign=2 loadavg_1m=10.94 loadavg_5m=8.42 loadavg_15m=6.62 ncpu=12 boot_done=1 verdict=KHONG_HOP_LE ly_do=qemu_foreign=2
WARNING: phép đo [after] KHÔNG HỢP LỆ (qemu_foreign=2) — số THỜI GIAN không dùng để kết luận (D2-116/OD-K6-6)
# MEASUREMENT_INVALID tag=after ly_do=qemu_foreign=2
# infra: cpu=max invalid=2 isolation_bad=0 guard_log=/tmp/kt323-max-soak/qemu-guard.log
# guard: boot_end=61 rc_124=0 qemu_own=0
# soak: xong wall=110s (0 boot/s); ok=60 fail=0 tổng=60/60
# soak: BUILD_ID=05c16bca7b4e006f sha256=d262055d7d285efe7d43e5c57ad094d45c3b5da6df6f876c616580366232bfde git=f4debca03b054d1835f671084dcc6f25e37b62f2
# soak: 60/60 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: SMOKE N=60 < 600 — KHÔNG phải bằng chứng cổng K5'
```

- **`60/60`, `fail=0`, `rc_124=0`, `qemu_own=0`** — mọi boot dưới `-cpu max` đạt **đủ** bất biến soak
  (TAP 30/30 + 5 `# exc:` + `# apic:`/`# timer:` + PCI/net + U-mode + blk + 4 hart + priority/sleep).
- Exit **1** là **đúng thiết kế** của `--smoke` (N=60 < 600 ⇒ "KHÔNG phải bằng chứng cổng").
- **Hạ tầng KHÔNG HỢP LỆ cho số thời gian**: lúc đo có **2 QEMU aarch64** của hai sub-agent khác chạy
  song song ⇒ `qemu_foreign=2`, `verdict=KHONG_HOP_LE`, `invalid=2` (trước + sau). Theo **D2-116**,
  `wall=110s` và mọi số **thời gian** ở đây **không dùng để kết luận**; **60/60** là **số đúng/sai của
  bất biến**, không phải tuyên bố hiệu năng. Không retry (D2-07).

## 5. Đột biến (biến **có tác dụng** hay chỉ là chữ in?)

| # | Đột biến | Cách làm | Kết quả |
|---|---|---|---|
| (i) | **bỏ `-cpu max` ⇒ model mặc định** | không đặt `V2_X64_CPU` | `# qemu 11.1.3  cpu qemu64 …`, `# infra: cpu=qemu64 …`, **20/20** xanh ⇒ mặc định **đúng là model cũ** (số đo cũ tái lập, §3.3) |
| (ii) | **`-cpu max` mà không sửa gì** | `V2_X64_CPU=max`, kernel y nguyên | **XANH 20/20** (TAP 30/30, `# exc:` = 5, marker 1 lần, halt sạch) ⇒ ghi rõ: **"kernel không phụ thuộc model"** trong phạm vi §1.2 |
| (iii) | **model không tồn tại** (chứng minh biến tới QEMU) | `V2_X64_CPU=bogus` | **ĐỎ, exit 1**: `boot-1.log` = `qemu-system-x86_64: unable to find CPU model 'bogus'`; runner `not ok 2 - boot 1: # thiếu dòng 'TAP version 13' …`, `# 18/20 kiểm sạch`, `# infra: cpu=bogus …` ⇒ nếu biến bị bỏ qua thì lần này **đã xanh** — đây là ca âm **có răng** (k6-test-invariants §2: ca âm phải chứng minh đã đột biến) |
| (iv) | **soak model sai** | `V2_X64_CPU=bogus` + `--smoke 60 3` | **exit 3** (TỪ CHỐI ĐO, **0** boot được đếm) + `# soak-x86_64.sh: QEMU TỪ CHỐI model CPU (V2_X64_CPU='bogus') — xem …/kt293-probe.log` |

Nguyên văn (iii):

```text
not ok 2 - boot 1: # thiếu dòng 'TAP version 13' # thiếu plan '1..30' # có 0 dòng 'ok', cần 30 , log /tmp/kt323-bogus/boot-1.log
…
# 18/20 kiểm sạch (boot=1, tests=30, exc=5)
# infra: cpu=bogus guard_log=/tmp/kt323-bogus/qemu-guard.log qemu_own=0
```

`boot-1.log` (nguyên văn, toàn bộ):

```text
qemu-system-x86_64: unable to find CPU model 'bogus'
```

## 6. Cổng

`./v2/bin/v2 check` trên cây có hai tệp shell sửa + journal mới: **exit 0** (PASS). Các bước liên quan:

```text
== v2 check (HEAD f4debca0) ==
…
# check-scope-table: PASS
# check-scope-table-selftest: PASS (39/39 ca)
abi-check: PASS (/tmp/kt323-s1/spec/abi.toml)
scn-check: PASS (27 tệp, 0 blocked, 16 nhóm)
report-check: PASS (1 báo cáo, 297 hàng QUEUE phủ đủ)
gen-check: PASS (code + docs khớp spec/abi.toml)
…
# gate-wall 150814 ms (150.8 s)
```

Task này **không** chạm mã Rust/kernel/spec ⇒ BUILD_ID ảnh **không đổi** (`build=05c16bca7b4e006f
text=69632`), fmt/clippy giữ nguyên; cổng chạy **đầy đủ** để chắc không luật nào bắt hai tệp runner/soak.

Selftest của soak (không mở QEMU, kiểm bộ đọc log 44 ca):

```text
$ bash v2/test/boot/soak-x86_64.sh --selftest
…
# soak-x86_64 --selftest: 44/44 ca đúng như phải (2 dương + 42 âm)
# soak-x86_64 --selftest: PASS
```

Runner **không** có cờ `--selftest` riêng: 26 ca âm + 1 ca dương KT-313 của nó chạy **trong mỗi lần
chạy thật** (§3.1/§3.2 — 29/29 `ok` ở cả hai model).

## 7. Rủi ro / còn thiếu

1. **Chưa soak 600 trên `max`.** KT-323 đo runner **1 boot** + soak **smoke 60/60** ⇒ **không** phải
   bằng chứng cổng K5'. Muốn "như K3" **trên `max`** phải chạy soak **600** (task riêng), và phải chạy
   khi `qemu_foreign=0` để số thời gian hợp lệ (D2-116).
2. **`max` không bền giữa các phiên bản QEMU** — "all features supported by the accelerator" đổi khi
   QEMU/TCG đổi. Bằng chứng `max` **phải** ghi kèm QEMU version (nay có ở `# qemu` và `# cấu hình:`);
   không được coi `max` là model cố định như `qemu64`.
3. **Kernel không đọc CPUID** ⇒ "xanh trên `max`" **không** chứng minh dùng được AVX/x2APIC/FSGSBASE…
   Nếu sau này dùng, phải thêm đường kiểm CPUID + bật CR4/MSR tương ứng — và khi đó `max` là môi trường
   đo **có ý nghĩa** hơn `qemu64`.
4. **Timing lệch giữa hai model** (19/92 dòng evidence): **không** đổi ngưỡng nào; các bất biến hiện
   hành (KT-308/KT-313) vốn không phụ thuộc thứ tự nên xanh ở cả hai model. Nếu thêm bất biến **phụ
   thuộc thời gian**, phải theo `k6-test-invariants.md` §1 (thứ tự kiểm, cơ chế bảo đảm, đảo thứ tự
   thì có đỏ oan không) — `max` là môi trường dễ lộ lớp lỗi này (tốt) nhưng cũng dễ đỏ oan.
5. **Tài liệu `k5-x86_64-boot.md` chưa cập nhật**: §9 bảng "còn thiếu", §12.12, §19.6 còn ghi "`-cpu max`
   chưa". Task KT-323 chỉ sửa `run-x86_64.sh`/`soak-x86_64.sh` + journal (không sửa docs) — cần một mục
   tài liệu riêng khi hợp nhất.
6. **Không** tuyên bố K5' đạt: còn "như K3" (TAP smp 1|2|4 + soak **600** boot) và các mục ngoài soak
   của `k5-x86_64-boot.md` §12.12.

## 8. Liên quan

- `v2/docs/k5-x86_64-boot.md` §9, §12.12, §15–§19 — hiện trạng x86_64 và các mục còn thiếu.
- `v2/docs/k6-test-invariants.md` — bất biến không phụ thuộc thứ tự tick (§1), ca âm phải chứng minh
  đã đột biến (§2).
- `v2/test/boot/run-x86_64.sh`, `v2/test/boot/soak-x86_64.sh` — hai tệp được sửa trong KT-323.
- `v2/test/boot/qemu_guard.sh` + `v2/docs/k6-harness.md` — mọi QEMU đi qua guard (KT-78 / D2-116).
