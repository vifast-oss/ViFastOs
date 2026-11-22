# KT-341 — `x86_64` (ĐƯỜNG GĂNG): shell sản phẩm dùng chung `vifast-plat-user`, `Machine` ra arena NOLOAD

- **Task**: `KT-341` (D2-244, D2-247, D2-242 nhóm 7 `7C-r13`, D2-83, D2-243, D2-245), 3 h, nhánh
  `task/kt341-prod`, base `main` @ `b5ad0a01`, worktree `/tmp/kt341-s1`.
- **Phạm vi**: `v2/kernel/vifast-plat-x86_64` (`core_bridge.rs`, `userload.rs` mới,
  `syscall_env.rs`, `Cargo.toml`, `lib.rs`), `v2/kernel/vifast-kbin-x86_64` (`build.rs`,
  `shellpath.rs` mới, `main.rs`, `product.rs`, `umode.rs`, `bootpt.rs`, `linker.ld`, `net.rs`,
  `Cargo.toml`), `v2/kernel/vifast-plat-user/lib.rs` (backend x86_64),
  `v2/test/boot/{run,budget,soak,gpu}-x86_64.sh`. **Không** chạm `spec/`, 19 tệp `[[legacy]]`,
  `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/**`, `.github/**`, `main`.
- **Kết quả một dòng**: `Machine` 184.792 B ra section **NOLOAD `.corespace`** ⇒ `.data+.bss`
  ảnh **sản phẩm** `43.072 → 50.496 B` (< 65.536) mà **vẫn có core** (`# core: addr=0x1ea000
  size=184792 in_image=1`); shell **cùng nguồn** `vifast-plat-user/programs/shell.rs` nạp **ELF64
  thật**, chạy **CPL3** trên **ảnh sản phẩm** (`--features product-shell`) qua `int 0x80` của
  core (`7C-r13` giữ nguyên), in prompt + `help` + `echo` + `exit code=0`; TAP x86_64 **43 → 45**,
  runner **36/36**, gpu **46**. Hai đột biến bắt buộc **ĐỎ**: (i) `mut-machine-bss` ⇒ budget
  `235.288 ≥ 65.536`; (ii) `mut-no-int80` ⇒ TAP 44 `not ok` (`puts=0`, không marker).

---

## 1. Khảo sát trước khi sửa

### 1.1 Ảnh **sản phẩm** x86_64 trước KT-341 có gì

Đo `budget-x86_64.sh` + boot thật (`qemu-system-x86_64 -M q35 -m 128M -smp 4`):

```text
# image=x86_64-product … data+bss=43072 budget=262144 label_budget=65536 …
# sections-seen: .text=69632 .rodata=1202 .note.Xen=20 .data=1168 .bss=41904
# viFastOs x86_64: long mode … lma=1 → product gdt/idt ok=1 → boot ok → halt   (0 TAP)
```

| Có | Chưa có |
|---|---|
| long mode, identity 1 GiB + MMIO 2..4 GiB, COM1, GDT/TSS/IDT thật, W^X kernel, marker, halt | `vifast-core::Machine`, ELF loader, ring 3 thật, shell/procsrv, net/blk/gpu (chỉ trong ảnh ktest) |

### 1.2 Ảnh **ktest** trước KT-341 (nguồn `kt340c-core.md` §9)

- `Machine` 184.792 B **trong `.bss`** ⇒ `.data+.bss` **251.024 / 262.144** — headroom **11.120 B**
  (báo động: chỉ cần thêm vài static là đỏ).
- Ring 3 chỉ là stub asm 1 trang (`.user` 472 B), **không** có ELF loader, **không** có chương
  trình `vifast-plat-user` nào; `int 0x80` pha core đi qua core nhưng chỉ phục vụ rig `coretest`.
- **Ngân sách**: 184.792 B > trần sản phẩm 65.536 B ⇒ `Machine` **không thể** nằm trong
  `.data+.bss` ảnh sản phẩm; 43.072 + 184.792 = 227.864 ⇒ nếu để trong `.bss` thì vượt trần
  (đúng đột biến (i)). Đường đúng: **NOLOAD arena** (KT-340b đã dời 8 bảng trang + stack ra
  `.bootpt`/`.bootstacks`; KT-338a aarch64 đã dời `BootMemory`/DMA ra RAM thật).

### 1.3 Bảng ngân sách sau KT-341 (đo `llvm-size -A` ảnh **sản phẩm** `--features product-shell`)

| Thành phần | Byte | Ở đâu | Ghi chú |
|---|---:|---|---|
| `.text` (boot + core + loader + rig) | 135.168 | `.text` | không tính trần |
| `.rodata` (bảng, chuỗi, **ELF shell nhúng**) | 20.836 | `.rodata` | shell ELF 8.021 B (R) |
| `.data` | 7.808 | `.data` | **cộng trần** |
| `.bss` | 42.688 | `.bss` | **cộng trần** |
| ⇒ **`.data+.bss`** | **50.496** | — | **< 65.536** (dư 15.040) |
| `vifast_core::Machine` | 184.792 | **`.corespace` NOLOAD** | `in_image=1` (trong `[__text_start,__kernel_end)`) |
| bảng trang boot + arena 64 frame | 294.912 | `.bootpt` NOLOAD | KT-340b; KT-341 nâng 32→64 frame |
| stack IST1 + stack user | 86.016 | `.bootstacks` NOLOAD | KT-340b |
| stack per-hart | 557.056 | `.smp_stacks` NOLOAD | KT-276 |
| `.user` (stub KT-270/304) | 88 | `.user` | không phải data |

`.data+.bss` **trước** (nguyên văn, ảnh sản phẩm không feature, đo đầu phiên):
`# image=x86_64-product … data+bss=43072 …`
`.data+.bss` **sau** (nguyên văn, ảnh sản phẩm `--features product-shell`, lúc kết thúc):
xem §4.1. Ktest **trước**: 251.024/262.144 (headroom 11.120) → **sau**: 66.536/262.144
(**headroom 195.608 B**, +184.792 do `Machine` ra NOLOAD, +~1.200 static shell/capture).

---

## 2. Cách làm (không nới ngưỡng nào)

1. **`Machine` ra `.corespace` NOLOAD** (`core_bridge.rs` + `linker.ld`): giữ nguyên
   `MaybeUninit` + `init_zeroed` tại chỗ (bài học tràn stack KT-61/KT-338b), chỉ đổi
   `link_section`. `init` zero tường minh nên NOLOAD (không được `.bss` zero hộ) vẫn đúng.
   Đột biến (i) bỏ `link_section` bằng feature `mut-machine-bss`.
2. **Loader ELF x86_64** (`vifast-plat-x86_64/src/userload.rs`, khuôn aarch64 KT-339): parse bằng
   **cùng** `vifast-elf` (D2-26, không parser thứ hai), map từng segment bằng `VSpace` **động**
   (W^X: `PF_X`→RX, `PF_W`→RW, còn lại RO), zero frame trước khi copy, đăng ký dải user cho
   `DebugPutStr` fail-closed.
3. **Shim `vifast-plat-user` thêm backend x86_64** (`int 0x80`, `MessageInfo` gửi ở `rdx`,
   MR0..3 `r10/r8/r9/r12`; trả `MessageInfo` ở **`r13`**, `7C-r13` D2-242) — **cùng nguồn**
   `shell.rs` với riscv64/aarch64, không `cfg` trong chương trình.
4. **`shellpath.rs`** dựng rig trên `Machine` (CSpace 16 ô, Endpoint 2/4/5/8 khớp hằng
   `EP_*` của shell, TCB, Frame IPC buffer), nạp `shell.elf` (nhúng `include_bytes!` từ
   `build.rs`), map stack + **ring KT-69** (bơm script `help`/`echo same-source`/`exit` bằng
   `conchan` — một codec, hai phía), `iretq` xuống CPL3. `Exit` của shell là syscall **của core**
   ⇒ kernel khôi phục `CR3`, đọc bằng chứng rồi tiếp quản.
5. **Đếm byte kernel-side** (`syscall_env::capture_*`): `DebugPutStr` cộng dồn puts/bytes/hash +
   khớp 4 mẫu (`shell>`, `# shell: help cmds=6`, `# shell: echo same-source`,
   `# shell: exit code=0`) — TAP đọc **byte thật** đã in, không tin lời U-mode (D2-102).
6. **Hai ảnh, một mã**: `product-shell = ["shell"]`, `ktest` cũng bật `shell`; ảnh sản phẩm
   không feature giữ đường tối giản (D2-08). Ảnh sản phẩm in `# core:`/`# shell:` (không TAP);
   ảnh ktest phát TAP 44/45 rồi trả chuỗi boot về `net::run` (TAP 15.. không đổi).

---

## 3. Đỏ trước (D2-05)

- Đầu phiên: ảnh sản phẩm **không** có `# core:` và **không** có `shell>` (boot log chỉ tới
  `product gdt/idt ok=1` + marker) — đúng trạng thái "core chưa vào ảnh sản phẩm".
- `run-x86_64.sh` đặt `EXPECT_TESTS=45` trước khi có TAP 44/45 ⇒ boot ktest đỏ (`có 43 dòng ok,
  cần 45`) — sau đó mới thêm hai ca.
- Hai đột biến (mục 5) đều chạy **một lần**, log giữ nguyên; không `#[ignore]`/không retry.

---

## 4. Bằng chứng (nguyên văn từ log)

### 4.1 Ảnh **sản phẩm** — budget + core + shell CPL3

```text
# image=x86_64-product path=… file=206440 arch=x86_64(e_machine=62) data+bss=50496 budget=262144 label_budget=65536 … bootpt=294912 bootstacks=20480 corespace=188416 user=88 noload=1060864
# data+bss-sections: .data=7808 .bss=42688
# core: addr=0x1ea000 size=184792 pin=196608 in_image=1 harts=1 objects=1 caps=0 healthy=1
# shell: load entry=0x700000d0 base=0x70000000+3p segs=2 stack_top=0x70010000 buf_va=0x70009000 ring_va=0x43000000 script=27B root=0x1c2000
# shell: enter cpl=3 entry=0x700000d0 rsp=0x70010000 cr3=0x1c2000 tcb=8 untyped=1 fresh=1
# shell: up cmds=help/echo/ps/wait/kill/exit
shell>
# shell: help cmds=6
# shell: echo same-source
# shell: exit code=0
# shell: exit code=0 state=0 puts=15 bytes=227 hash=0xeb001aff46401395 prompt=1 help=1 echo=1 exit_line=1 syscalls=18 yields=0 user_cs=1 …
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt                     (QEMU exit 0)
```

(`shellpath` in `# shell: load … +3p`: 3 trang = `.text` 2 trang + segment RO/RW 1 trang; shell
ELF 8.021 B.)

### 4.2 Ảnh **ktest** — TAP 43 → 45

```text
TAP version 13
1..45
…
ok 43 - lock: RFLAGS.IF masked in critical section, restored after
# shell: exit code=0 state=0 puts=15 bytes=227 hash=0xeb001aff46401395 prompt=1 help=1 echo=1 exit_line=1 …
ok 44 - shell: prompt + clean Exit from CPL3 through core int 0x80
ok 45 - shell: help + echo builtin (16 core syscalls, no ktest code in ELF)
# viFastOs x86_64: boot ok
```

### 4.3 Runner (nguyên văn)

```text
# 36/36 kiểm sạch (boot=1, tests=45, exc=6)
# ca âm: 32/32 ĐỎ đúng như phải (+ 1 ca dương KT-313)
```

`gpu-x86_64.sh`: `EXPECT_TESTS=46` (TAP gpu dời 44→46) — `0 lỗi`, `gpu_ppm.py PASS (23 kiểm)`.
`soak-x86_64.sh --selftest`: **54/54** (2 dương + 52 âm; thêm ca âm "thiếu bằng chứng shell").
`budget-x86_64.sh` báo thêm `.corespace` (NOLOAD) như `.bootpt`/`.bootstacks`.

### 4.4 `size_of::<Machine>()`

`184.792 B ≤ 196.608 B` (pin D2-84) — in trong `# core: size=184792 pin=196608`; không đổi
so với KT-340c, chỉ đổi **chỗ nằm** (`.bss` → `.corespace` NOLOAD).

---

## 5. Hai đột biến (bắt buộc ĐỎ — D2-05)

### 5.1 (i) `--features product-shell,mut-machine-bss` (`Machine` trở lại `.bss`)

```text
# image=x86_64-product … data+bss=235288 … corespace=0 …
# data+bss-sections: .data=7808 .bss=227480
budget-x86_64: VƯỢT TRẦN ẢNH SẢN PHẨM 235288 ≥ 65536 byte … — exit 1
```

Đúng kỳ vọng: `50.496 + 184.792 = 235.288`. Chứng minh phép đo ngân sách **có răng**: chỉ
`link_section` đổi, hành vi boot không đổi, mà cổng ĐỎ.

### 5.2 (ii) `--features ktest,mut-no-int80` (tắt `DebugPutStr` của core)

```text
# shell: exit code=0 state=0 puts=0 bytes=0 hash=0xcbf29ce484222325 prompt=0 help=0 echo=0 exit_line=0 … user_cs=1
not ok 44 - shell: prompt + clean Exit from CPL3 through core int 0x80
# TAP 44 FAIL — dừng (không in marker)
```

`puts=0` (kernel không in byte nào) ⇒ TAP 44/45 ĐỎ, không marker — chứng minh TAP mới bám vào
đường `DebugPutStr` **của core**, không phải hằng số.

### 5.3 Lỗi **thật** bắt được ở lượt cổng đầu (đã sửa): IRQ0 của 8259 PIC ⇒ vector 8 trùng `#DF`

Lượt `gate --dev` đầu (trước commit) **ĐỎ** ở bước QEMU x86_64: boot sản phẩm **quá 20 s**, log có
`# exc: vec=8 err=0x700000d0 rip=0x1b cs=0x202 rflags=0x70010000`. Truy vết `-d int` của QEMU:

```text
Servicing hardware INT=0x08
     0: v=08 e=0000 i=0 cpl=3 IP=001b:0x700000d0 … CR3=0x1c2000
```

Đây **không** phải `#DF`: SeaBIOS để lại **8259 PIC** với IRQ0 (PIT, 18,2 Hz) ánh xạ **vector 8**
— trùng ô IDT `#DF` (stub `isr_df` không đẩy mã lỗi vì CPU *phải* đẩy, nhưng IRQ thì không) ⇒
khung lệch một ô, in rác rồi halt. `iretq` đặt `RFLAGS.IF = 1` nên lỗi chỉ hiện khi tick rơi đúng
lúc CPL3 — **1/6 boot** trên máy đo (5 lần đầu đều xanh, đúng kiểu flake nguy hiểm). Ảnh ktest
không lộ vì LAPIC/IOAPIC đã cấu hình trước ring 3.

Bản sửa: chặn **cả hai** PIC (`0x21`/`0xA1` = `0xFF`) ngay trước `iretq` trong `shellpath` (ảnh
này chưa có driver IRQ legacy; ktest cũng chặn để pha shell không nhiễu). Sau sửa: **10/10** boot
sản phẩm sạch (`# exc` = 0, marker + halt), runner vẫn 36/36. Học được: "chạy được" một lần không
phải bằng chứng — cổng chạy dưới tải mới lộ.

---

## 6. Cổng (tất cả chạy trên cây **sạch** sau commit)

- `./v2/bin/v2 check` — **PASS** (183 s, trước commit; cây bẩn không ảnh hưởng checker này).
- `./v2/bin/v2 gate --dev` (**T1**) — **PASS**; `TỔNG gate --dev (T1) 638.499 ms`;
  `QEMU x86_64 (K5' bước 1): OK` (214,6 s: `ok 5 - boot 1: TAP 45/45 + 6 dòng '# exc:' +
  marker 1 lần + halt sạch`, `ok 2 - sản phẩm: …`, `# 36/36 kiểm sạch (tests=45)`);
  `selftest pre-measure (KT-95): OK` (chỉ xanh khi cây sạch — lượt đầu bẩn nên ĐỎ đúng thiết kế).
- `bash v2/test/boot/run-x86_64.sh` — **PASS**, `# 36/36 kiểm sạch (boot=1, tests=45, exc=6)`,
  `# ca âm: 32/32 ĐỎ đúng như phải`.
- `bash v2/test/boot/budget-x86_64.sh <ảnh-sản-phẩm>` — **PASS** (`data+bss=50496`).
- `bash v2/test/boot/gpu-x86_64.sh` — **PASS** (TAP 46, PPM 23/23, 0 lỗi), chạy lại **sau** bản
  sửa PIC.
- `bash v2/test/boot/soak-x86_64.sh --selftest` — **PASS** 54/54.
- **Không** chạy `gate --fast` (D2-245: sub-agent chỉ T0/T1). Không `V2_MEASURE_LOCK=off`.
- QEMU: chỉ chạy các tiến trình do chính task tạo, giết bằng `kill <pid>` đã ghi; không `pkill`.
  Ngoài runner/gate còn 10 boot sản phẩm lặp để kiểm flake PIC (mục 5.3) — tất cả 10/10 sạch.

---

## 7. Nợ còn lại (nói thẳng)

1. **Chưa có procsrv** trên ảnh sản phẩm x86_64: task này làm **shell** (DoD số 3 "TAP mới ≥ 2");
   `procsrv` + `ps`/`wait`/`kill` là việc của **`KT-341a`** (cùng khuôn `shellpath`,
   `SHELL_ELF`→`PROCSRV_ELF`, Endpoint 2/4 đã có sẵn trong CSpace rig). Shell hiện vẫn in
   `# shell: up cmds=…ps…` nhưng `ps` sẽ treo vì không có server — script kiểm chỉ dùng
   `help`/`echo`, không dùng `ps`.
2. **`harts`/`resolve_addr`** (nợ KT-340c §9): ảnh sản phẩm init `Machine` với `harts=1` **trước**
   SMP (ảnh sản phẩm chưa bật SMP); `resolve_addr` vẫn `NotMapped` 11 — shell không cần. Giữ
   nguyên nợ, không mở phạm vi.
3. **W^X cho `.corespace`**: `Machine` nằm sau mốc 2 MiB của identity map (PD[1] leaf), nên không
   được `wx::enable` đặt NX như `.data/.bss` trong 2 MiB đầu; TAP W^X (35/36) không phủ vùng này.
   Đề xuất `KT-341a` hoặc `KT-340e` (bảng trang 4 KiB cho toàn ảnh) — **không** sửa lẻ ở đây.
4. **Ring/`conchan`**: kernel bơm script (không phải UART RX thật) — cùng nợ KT-73 đã ghi cho
   aarch64; đúng phạm vi K3.5.
5. `procsrv`/`echo` ELF chưa được `build.rs` sinh (chỉ `shell.elf`); thêm khi làm `KT-341a`.

---

## 8. Commit

- `2f004d95` — `[KT-341] x86_64: Machine ra .corespace NOLOAD, shell sản phẩm dùng chung chạy
  CPL3 (TAP 43→45)` (21 tệp, +1.714/−38).
- `7550c58c` — `[KT-341] shell: chặn 8259 PIC trước iretq (IRQ0 vector 8 trùng #DF, flake 1/6)`.
- Commit journal này (hash ở báo cáo cuối). Cây sạch sau commit (`git status -s` rỗng).
