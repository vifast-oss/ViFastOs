# KT-417c — x86_64 `syscall`/`sysret` (`swapgs` + GDT `planned`, giữ `int 0x80` & `7C-r13`) + phục hồi phủ preempt CPL3

> Đợt **W8b** (Slot 3), nhánh `task/kt417c-syscall-sysret`, commit **`a5541601`** (nền `main` `afb2556b`).
> Ngân sách 3 h — dùng **~35 phút** cho mã + T0 (lượt 1: 02:17 → 02:51 JST) + **~14 phút** T1.

## 0. Kết luận ngắn (đọc trước)

1. GDT đã đảo sang thứ tự **`planned`**: `INDEX_USER_DATA = 3` (`0x18`), `INDEX_USER_CODE = 4`
   (`0x20`) ⇒ ring 3 chạy `cs = 0x23`, `ss = 0x1b` (trước: `0x1b`/`0x23`).
2. Đường **`syscall`/`sysret` chạy thật**: `swapgs` + **GS scratch per-hart** + `STAR`/`LSTAR`/
   `FMASK` + `EFER.SCE` (đọc-sửa-ghi, giữ `LME|NXE`), nạp ở **BSP và từng AP**.
3. **`int 0x80` không suy suyển**: stub cũ **0 dòng sửa**; hai cửa cùng gọi `syscall_dispatch`
   ⇒ `mut-no-int80` vẫn **KILLED** (đo thật, 357 s).
4. Nợ **`KT-413c` §6.1 đã đóng**: `# sched: cpl3 preempted≥1 resumed=1 cs=0x23` được **assert
   trong `run-x86_64.sh`** (trước chỉ `hardening-max.sh` — không nằm trong cổng).
5. `spec/kernel.toml`: `sysret_ready = true` + khối `planned` có `applied_at`, `trap = "int 0x80"`
   và `min_compat = 1` **giữ nguyên**; `spec/abi.toml` **0 chạm**.
6. **0 TAP mới** (`TAP_TESTS = 50`): bằng chứng `syscall`/CPL3 kiểm bằng `grep` trong `check_log`
   ⇒ `expected.txt` + `lib/tap-parity.sh` không đổi.
7. T1 `gate --dev`: **PASS** — `# gate-steps registered=82 ran=68 skipped=14 failed=0`
   (`skip` đều in `BỎ QUA (T1 — chỉ chạy ở T2)`). T2 để Lead.

## 1. Đỏ-trước (D2-05) — log trong `v2/agent/JOURNAL/kt417c-syscall/`

| # | Ca đỏ | Log | Exit | Nội dung |
|---|---|---|---|---|
| 1 | Máy kiểm đòi `sysret_ready = true` trong khi spec còn `false` | `red-kernel-rule.log` | 1 | `not ok - E_GDT_SYSRET: sysret_ready phải true từ KT-417c …` |
| 2 | Spec đã đảo mà `gdt.rs` **chưa** ⇒ lệch hằng | `red-const-lech.log` | 1 | **4** `E_CONST_LECH`: `index_user_code` 4↔3, `index_user_data` 3↔4, `selector_user_code` 32↔24, `selector_user_data` 24↔32 |
| 3 | `--selftest` khi mới thêm 2 ca (spec chưa sửa) | `red-kernel-selftest.log` | 1 | **3/13 sai** — dương + 2 ca mới "không đột biến được" |
| 4 | **Đối chứng âm**: gỡ 2 luật mới khỏi bản sao máy kiểm ⇒ 2 ca mới phải đỏ | `red-neg-control.log` | 1 | **2/13 sai** (E_GDT_SYSRET_APPLIED / E_GDT_SYSRET không còn được bắt) |
| 5 | 11 ca âm cho `check_syscall`/`check_cpl3` trên **log boot thật** | `harness-checks.log` | 11/11 ĐỎ | xem §4 |

Điểm quan trọng: ca 4 chứng minh **hai ca âm mới có răng** (nếu gỡ luật thì chúng không bắt được),
chứ không phải "viết test cho có".

## 2. Nguyên nhân gốc & cách vá

### 2.1 Vì sao **buộc** đảo GDT

`SYSRET` **cứng** công thức (SDM Vol 2B "SYSRET"): `CS = STAR[63:48] + 16`,
`SS = STAR[63:48] + 8`. Với `STAR[63:48] = 0x10` (kernel data) ⇒ cần `CS = 0x20` (**user code**)
và `SS = 0x18` (**user data**), tức **user data phải đứng ngay trước user code**. Đó đúng là
`[arch.x86_64.gdt.planned]` đã soạn sẵn trong spec. Việc (1) của QUEUE vì vậy chỉ là **đổi chỗ
hai hằng** `INDEX_USER_CODE`/`INDEX_USER_DATA` (selector tự suy ra `× 8`) + đổi chỗ **hai phần tử**
trong initializer `static GDT` (bẫy: đổi hằng mà quên initializer là GDT lệch thầm).

### 2.2 `syscall` **không** chuyển stack — chỗ QUEUE không nhắc

| | `int 0x80` | `syscall` |
|---|---|---|
| đẩy `SS/RSP/RFLAGS/CS/RIP` | CPU (từ CPL3) | **không** |
| đổi `RSP` sang stack kernel | CPU qua `TSS.RSP0` | **không** |
| `GS.base` | giữ nguyên | phải `swapgs` |
| đường ra | `iretq` | `sysretq` (`RCX`/`R11` bắt buộc hợp lệ) |

Nếu stub chỉ `call` handler rồi `sysretq` thì **kernel chạy trên stack user** — user ghi đè được
khung ngắt. Vì vậy thêm **GS scratch per-hart** trong `plat-x86_64`:

```
static GS_SCRATCH: [[AtomicU64; 4]; MAX_HARTS]   // [0]=rsp0, [8]=RSP user, [16..24]=dự phòng
```

`init(hart, rsp0)` ghi `IA32_GS_BASE = 0` (user) + `IA32_KERNEL_GS_BASE = &scratch[hart]`; stub
`swapgs` ⇒ `GS.base` = scratch ⇒ `mov rsp, gs:[0]`. Lựa chọn `plat` (không phải `kbin/.bss`) theo
đúng tầng L2 "IRQ/trap/syscall" của kiến trúc đích và dùng được cho cả ảnh product lẫn ktest.

### 2.3 `EFER.SCE` chưa từng được ghi + `FMASK` phải xoá `AC`

`grep write_msr` trước task chỉ có `APIC_BASE` ⇒ `syscall` đầu tiên sẽ `#UD`. Hàm thuần
`efer_with_sce(current) = current | LME | NXE | SCE` (đọc-sửa-ghi; **cấm** `mov efer, imm` vì ghi
đè mất `NXE` = mất W^X). `FMASK = IF|TF|DF|NT|AC`: `AC` (bit 18) là bit **bắt buộc** — SMAP đã bật
từ `KT-418b`, ring 3 đặt `AC = 1` là vô hiệu hoá mọi `stac`/`clac`.

### 2.4 Hai cửa, **một** hợp đồng byte-level — không thêm tham số thứ 12

Stub `syscall` dựng frame **cùng bố cục** `SYSCALL_FRAME_*` (err + vector + 15 GPR) rồi gọi
**cùng** `syscall_dispatch`. Cửa vào được phân biệt bằng **ô `vector` của frame**:
`int 0x80` đẩy `0x80`, stub `syscall` đẩy `SYSCALL_FAST_VEC = 0x100` (không phải vector hợp lệ của
x86). Nhờ vậy **không** phải thêm tham số thứ 12 ⇒ hợp đồng `7C-r13` (`return_message_info = r13`)
và 11 tham số SysV của `KT-340c` **giữ nguyên** cho cả hai cửa.

### 2.5 Nợ `KT-413c` §6.1 — "phục hồi" = **đưa assert vào cổng**, không dựng lại cơ chế

Cơ chế đã có từ `KT-418c` (`kbin-x86_64/src/trap.rs` `cpl3_arm`/`cpl3_on_tick`/`cpl3_report`).
Việc còn lại đúng như Lead chốt: thêm `check_cpl3()` vào `run-x86_64.sh` (nơi **có** trong
`steps_gate.sh`) và so `cs` với selector ring 3 mà `# umode: iretq` công bố — **không hardcode**
`0x1b`/`0x23` (khuôn `hardening-max.sh:156-162`), nên lần đảo GDT sau này phép so vẫn đúng nghĩa.

## 3. Bằng chứng **xanh** (máy sinh)

### 3.1 Log boot thật (`boot-1.log`, `qemu64`) — `boot1-evidence.txt`

```
# umode: iretq entry=0x100000000f sp=0x1000002000 rflags=0x202 cs=0x23 ss=0x1b code_va=0x1000000000 code_pa=0x143000 code_bytes=516 stack_va=0x1000001000 stack_pa=0x22f000 syscall_stack=0x14d000
# syscall: msr efer=0xd01 sce=1 nxe=1 star=0x0010000800000000 fmask=0x44700 ac_mask=1 retcs=0x23 retss=0x1b gs_scratch=0x1501a0 kstack=0x14d000
# sched: cpl3 preempted=1 resumed=1 cs=0x23
# umode: int80 nr=12 cs=0x23 rip=0x1000000051 user=1
# umode: ring3 ran nr=12 ret=14
# umode: int80 ret error=0 value=14 ptr=0x1000000086 len=14
# syscall: ran nr=12 ret=14 err=0 cs=0x23 ac=0 user=1
# umode: exit code=0 nr=14
ok=50 notok=0
```

Đọc dòng `# syscall: msr` (tất cả là **MSR đọc lại**, không phải hằng):
`efer=0xd01` = `SCE(0x1) | LME(0x100) | LMA(0x400) | NXE(0x800)` — tức `0xd00` cũ **+** `SCE`,
không mất bit nào; `star=0x0010000800000000` = `[63:48]=0x10`, `[47:32]=0x08`;
`fmask=0x44700` = `IF|TF|DF|NT|AC`; `retcs/retss=0x23/0x1b` = đúng cặp selector ring 3;
`gs_scratch` = địa chỉ vùng scratch của hart 0; `kstack=0x14d000` = `__syscall_stack_top`.

Dòng `# syscall: ran … ac=0` là **đo** tại cửa vào handler (trước mọi `stac`) ⇒ `FMASK` thật sự
xoá `AC`. Ngay sau `sysretq`, ring 3 **ghi vào stack user** (`mov [rsp-16], rdx`): nếu `sysret`
trả về CPL0 thì lần ghi đó là `#PF` dưới SMAP (`-cpu max`) ⇒ boot chết **có tên**.

### 3.2 `run-x86_64.sh` (đứng riêng và trong T1): `39/39 kiểm sạch (boot=1, tests=50, exc=6)`

`check_syscall` + `check_cpl3` chạy **trong `check_log`** nên mọi ca boot đều phải qua.

### 3.3 `mut-no-int80` **vẫn ĐỎ** — `mut-no-int80.log`

```
ok - mut-no-int80: KILLED (357.3 s) sites=1 rc=1 marker=ok
# mutants: chạy=1 killed=1 survived=0 wrong-reason=0 stale=0 chưa-nối=0
```

Lý do **cấu trúc**: hook `#[cfg(feature = "mut-no-int80")]` nằm trong `shellpath::dispatch`, mà
**cả hai** cửa đều đi qua `syscall_dispatch`; đường `syscall` mới chạy ở pha umode (shellpath
**chưa** active) nên không "vá" được đột biến.

### 3.4 Test host (`cargo test -p vifast-plat-x86_64`: 20 binary `ok`)

`tests/syscall_fast.rs` (8 ca mới) khoá bằng **giá trị tuyệt đối viết tay** từ SDM:
thứ tự `planned`; `star_value() == 0x0010_0008_0000_0000`; công thức `SYSRET` ⇒ `0x23`/`0x1b`
**khớp** `USER_CS`/`USER_SS`; `FMASK` có đủ 5 bit (kèm ca âm `& !AC`); `efer_with_sce` giữ
`LME`/`NXE`/`LMA`; GS scratch per-hart rời nhau + 8-byte aligned; `rsp0` per-hart; `SYSCALL_FAST_VEC`
không phải vector thật.

## 4. Ca âm của **cổng** (11 ca, `harness-checks.log`)

| Đột biến trên log thật | Kết quả |
|---|---|
| mất dòng `# syscall: ran` | ĐỎ — `xuất hiện 0 lần, cần đúng 1` |
| `ac=0 → ac=1` | ĐỎ — regex `ac=0` không khớp |
| `ret=14 → ret=0` | ĐỎ — `ret` phải `[1-9][0-9]*` |
| `retcs=0x23 → 0x18` | ĐỎ — `retcs không khớp selector ring 3` |
| `sce=1 → sce=0` | ĐỎ — `EFER.SCE/NXE + FMASK.AC + GS scratch` |
| `nxe=1 → nxe=0` | ĐỎ — (mất W^X) |
| `ac_mask=1 → 0` | ĐỎ — (FMASK thiếu `AC`) |
| mất `# sched: cpl3` | ĐỎ — `xuất hiện 0 lần` |
| `preempted=1 → 0` | ĐỎ — phải `[1-9][0-9]*` |
| `cs=0x23 → cs=0x1b` | ĐỎ — `cs của tick CPL3 không khớp selector ring 3` |
| lặp dòng `# syscall: ran` | ĐỎ — `xuất hiện 2 lần` |

`hardening-max.sh --selftest`: **25/25**, trong đó `cpl3-cs-lech` **đảo chiều** (đột biến nay đổi
`cs=0x23 → 0x1b`) — vẫn đỏ vì `cs_cpl3 != cs_umode`. `soak-x86_64.sh --selftest`: **72/72**.

## 5. T0 / T1

### 5.1 T0 (trong lúc sửa, cây staged)

| Lệnh | Kết quả |
|---|---|
| `cargo test -p vifast-plat-x86_64` | 20 binary `test result: ok`, **0 FAILED** |
| `check_kernel.py --selftest` | **13/13** (1 dương + 12 âm; trước 11) |
| `check_kernel.py` | **PASS** (49 giá trị) |
| `./v2/bin/v2 check` | **PASS** (85 s; 51 bước) |
| `run-x86_64.sh` | `39/39 kiểm sạch (boot=1, tests=50, exc=6)` exit 0 |
| `hardening-max.sh --selftest` | 25/25 |
| `soak-x86_64.sh --selftest` | 72/72 |
| `v2 mutants mut-no-int80` | **KILLED** |

### 5.2 T1 — `./v2/bin/v2 gate --dev` trên cây **sạch** @ `a5541601`

Log: `v2/agent/JOURNAL/kt417c-syscall/t1-clean.log` (3.436 dòng, 684 s).

```
--- L1 (sim 1.000 seed): OK                      (940 ms)
kernel-check: PASS (49 giá trị …)
ok 6 - boot 1: TAP 50/50 + 6 dòng '# exc:' + marker 1 lần + halt sạch
# 39/39 kiểm sạch (boot=1, tests=50, exc=6)
# gate-steps registered=82 ran=68 skipped=14 failed=0
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
```

`skipped=14` đều in đúng nhãn `BỎ QUA (T1 — chỉ chạy ở T2)` (git-guard, persist đỏ-trước, cắt
điện, QEMU net/input/gpu, gpu x86_64 …) — **không** bỏ âm thầm bước nào.

**Ghi chú phân loại**: log T1 có **một** dòng `not ok - selftest: netsrv ca âm có dòng PANIC 'xong'
(PASS giả) KHÔNG bị bắt (validator rỗng)` — đây là **dòng mô tả** của `net.sh --red-selftest`
(không có số hiệu TAP; ngay dưới là `# net.sh --red-selftest: 34 kiểm, 0 lỗi` và
`--- selftest validator net (KT-245): OK`). `net.sh` **0 chạm** trong commit này
(`git diff --name-only afb2556b -- v2/test/boot/net.sh` = rỗng) ⇒ **có sẵn từ trước**, không phải
đỏ của KT-417c, và **không** tính vào `failed` của cổng.

## 6. Số đo (bảng)

| Đại lượng | Trước | Sau |
|---|---|---|
| `cs` ring 3 (`# umode: iretq`) | `0x1b` | **`0x23`** |
| `ss` ring 3 | `0x23` | **`0x1b`** |
| `# sched: cpl3 … cs=` | `0x1b` | **`0x23`** |
| `TAP_TESTS` / `expected.txt` | 50 | 50 (**không đổi**) |
| `check_kernel --selftest` số ca | 11 | **13** |
| `code_bytes` section `.user` | 71 | 516 |
| `EFER` (đọc lại, hart 0) | `0xd00` (`LME|LMA|NXE` — `long mode efer=0x…d00` của ảnh) | `0xd01` (`+SCE`) |
| `syscall`/`sysret` | `#UD` (thiếu `SCE`) | chạy thật (`# syscall: ran nr=12 ret=14`) |
| `mut-no-int80` | KILLED | **KILLED** (không đổi) |
| `run-x86_64.sh` | 39/39 | 39/39 |
| T1 `failed` | — | **0** |

## 7. Ratchet (KHÔNG nới)

Không nới ngưỡng, không `#[ignore]`, không bỏ case, không retry lấy xanh. `min_compat = 1`,
`trap = "int 0x80"`, `TAP_TESTS = 50`, `EXPECT_EXC = 6`, trần `budget-x86_64.sh` — **tất cả giữ
nguyên**. `hardening-max.sh` chỉ **siết/đổi kỳ vọng theo GDT `planned`** (log giả `cs=0x23`,
đảo đột biến `cpl3-cs-lech`) — không nới ca nào.

## 8. Rủi ro / nợ còn lại

1. 🟠 **`-cpu max` chưa chạm tới đường `syscall`**: boot vẫn dừng ở **TAP 13** vì nợ **`KT-418d`**
   (deadlock khoá `Machine` khi `CR4.SMAP = 1`) — **có trước** task, không do task này. Vì vậy
   `swapgs`/`sysret` chưa được chạy dưới SMAP thật; bù bằng (a) test host `fmask_must_clear_ac`,
   (b) `ac=0` **đo** tại cửa vào, (c) ring 3 ghi stack user ngay sau `sysretq` (SMAP sẽ `#PF` nếu
   còn CPL0). Khi `KT-418d` xong, chạy lại `hardening-max.sh` là có bằng chứng đầy đủ.
2. 🟡 **`sysretq` với RIP không canonical**: `RCX` bẩn ⇒ `#GP` ở CPL0. Hiện **không** có đường
   user nào điều khiển `RCX` (stub lấy `RCX` do CPU đặt = RIP kế tiếp của chính lệnh `syscall`),
   nên chưa nguy hiểm; nếu sau này có `sysret` tới đích do user chọn thì **phải** kiểm canonical.
3. 🟡 **AP chưa có bằng chứng `syscall` riêng**: `init` đã chạy trên từng AP (`smp.rs`,
   `rsp0 = tss::ist(hart,1)`), nhưng pha ring 3 hiện chỉ chạy trên hart 0 nên chưa có log
   `# syscall:` từ hart ≠ 0. MSR là per-hart nên thiếu sót này chỉ lộ khi có tác vụ ring 3 trên AP.
4. 🟡 `# syscall:` **không** nằm trong `PRODUCT_HARNESS_MARKERS` — cố ý: ảnh product không in nó,
   nhưng thêm vào danh sách là **siết** bộ quét D2-08 và nên làm ở task đụng `budget-*`.

## 9. Tệp chạm (26 tệp, +1070 −47 — commit `a5541601`)

`spec/kernel.toml` (chỉ khối `[arch.x86_64.gdt]` + `.planned`; **0 chạm** `spec/abi.toml`) ·
`plat-x86_64/src/{gdt.rs,asm.rs,asm/syscall.rs,lib.rs,syscall_fast.rs*,tests/exc.rs,tests/umode.rs,
tests/syscall_fast.rs*}` · `kbin-x86_64/src/{umode.rs,smp.rs,bootasm.rs}` ·
`v2/test/boot/{run-x86_64.sh,hardening-max.sh,soak-x86_64.sh}` · `v2/test/kernel/check_kernel.py` ·
`v2/agent/JOURNAL/kt417c-plan.md*` + `v2/agent/JOURNAL/kt417c-syscall/*` (* = tệp mới).
**0 chạm** `asm/fpu.rs`, `v2/bin/**`, `spec/abi.toml`, `STATE/QUEUE/DECISIONS/AGENTS`.

## 10. Bàn giao

- Commit `a5541601` trên `task/kt417c-syscall-sysret` — **chưa** push/merge, **chưa** T2.
- **T1 PASS** (`registered=82 ran=68 skipped=14 failed=0`). T2 (`gate --fast`) do Lead trên ứng
  viên tích hợp; gợi ý kèm `V2_X64_CPU=max bash v2/test/boot/hardening-max.sh` để có bằng chứng
  SMAP cho đường `syscall` (kỳ vọng: vẫn dừng ở TAP 13 → `known-blocker` KT-418d, không phải đỏ mới).
- Journal này + 10 log bằng chứng đã stage cùng commit kế của nhánh (tài liệu, không đổi mã).
