# KT-418c — `-cpu max` `VSpace` isolation (`TF=1`) + `hardening-max.sh` + `PAN` aarch64 + di động host `a64 fpu.rs` + phủ preempt ring-3

- **Task**: `KT-418c` (P1, Chặng 2 / Đợt `W7b` Slot 2) — `v2/agent/QUEUE.md` dòng `KT-418c`;
  cơ sở `D2-261`/`D2-262` (APPROVED 2026-11-26), `KE-HOACH-KIEN-TRUC.md` §`W7b` Slot 2.
- **Nhánh / worktree / base**: `task/kt418c-hardening-max` @ `/private/tmp/vifast-wt/2`, base
  `main` @ **`297b89dc`**; `CARGO_TARGET_DIR=/Users/mps/dvchd/ViFastOs/target-task/slot-2`.
- **Ngân sách**: 2 h; khung Thấp điểm chính (`10:00 UTC`+), log mốc đầu `19:01 JST` = `10:01 UTC`.
- **Phạm vi (0 % chồng tệp 3 slot còn lại)**: `v2/test/boot/hardening-max.sh` (**mới**),
  `v2/kernel/vifast-plat-aarch64/{src/asm.rs,src/asm/fpu.rs,tests/pan_encoding.rs}`,
  `v2/kernel/vifast-kbin-x86_64/src/{trap,umode,bootasm,product,main}.rs`.
  **0 chạm** `v2/bin/**`, `spec/**`, `run-{x86_64,aarch64}.sh`, `soak-*.sh`, `ktest_persist.rs`,
  `b1.1-ratchet.toml`, `v2/kernel/vifast-kbin-riscv64/**`.

## 0. Kết luận ngắn (đọc trước)

1. **Lỗ `-cpu max` `VSpace` — tìm ra nguyên nhân gốc và vá**: `#PF` cô lập `VSpace` **có** xảy ra,
   nhưng `sec_probe::run` (2 ca âm `SMEP`/`SMAP`, TAP 50) gọi `exc_arm(VEC_PF, …)` mà `exc_arm`
   **xoá `EXPECT_CR2`** (đúng thiết kế `KT-418`) ⇒ arm của ca cô lập bị mất ⇒ handler rơi vào
   nhánh `# exc:` chung rồi `halt(2)`. Trên `qemu64` hai phép đo SMEP/SMAP **bị bỏ qua**
   (`cpuid_*=0`) nên lỗi không lộ. **`TF=1` trong mô tả cũ là đọc sai bit**: `rflags=0x10202`
   là **`RF=1`** (bit 16 — CPU đặt khi fault) + `IF=1`, **không** phải `TF` (bit 8).
2. **Lỗ thật thứ hai (mới, do `-cpu max` mở ra) — `PAN` aarch64 chưa từng chạy**: `pan_set` phát
   `.inst 0xD503417F` = `op1 = 0b011` — **không** phải trường PSTATE `PAN` (đúng: `op1 = 0b000`,
   `op2 = 0b100` ⇒ `0xD500419F`). Trên CPU **có** `PAN` (`-cpu max`) lệnh này trap `ec=0x00` và
   boot chết ngay trước TAP 50. Không lộ trước đây vì runner dùng `-cpu cortex-a72` (ARMv8.0,
   `pan_supported() = 0`) nên TAP 50 "xanh" bằng nhánh `pan_supported=0 … ok=1` — **xanh giả**.
3. **Phủ preempt ring-3 (nợ `KT-413c` §6.1) — phục hồi**: tick PIT cắt ngang **CPL3** trong một
   vòng quay có trần ở ring 3 ⇒ handler đếm, ghi `cs` **đo được**, và `iretq` về nhãn ring-3 do
   **kernel chọn**; `int 0x80` kế tiếp **từ CPL3** in
   `# sched: cpl3 preempted=1 resumed=1 cs=0x1b`. Tất định theo **sự kiện** (không ngưỡng ms).
   `cs` in **giá trị đo được** — sau `KT-417c` (GDT `planned`) cùng dòng thành `cs=0x23` mà
   **không** phải sửa mã.
4. **Nợ mới có tên (ngoài phạm vi, đề xuất `KT-418d`)**: sau khi vá (1), `-cpu max` x86_64 đi tiếp
   và **deadlock** khoá `Machine` (`IrqMcsLock::lock_irqsave`, `IrqMcsLock<()>`) khi `CR4.SMAP = 1`
   — sau `# umode: int80 nr=12`, dừng ở TAP 13/50. Đối chứng: `-cpu max,-smap` ⇒ **50/50 TAP +
   marker + halt**; `-cpu max,-smep` vẫn deadlock ⇒ thủ phạm là **SMAP**, không phải SMEP.
   `-smp 1` cũng deadlock ⇒ tự khoá (không phải hart phụ). Đây là lỗi **khác**, không phải lỗ
   `VSpace` của task này; `hardening-max.sh` in `# known-blocker:` và **vẫn** kiểm đủ hợp đồng
   hardening (đặt `HARDENING_REQUIRE_FULL=1` để nợ đó thành ĐỎ khi được đóng).

## 1. Đỏ-trước (D2-05) — mỗi thay đổi hành vi đều có ca đỏ **trước** khi sửa

### 1.1 `-cpu max` x86_64 — `#PF` cô lập `VSpace` không khớp hook (log `/tmp/w7b-418c/max-01.log`)

Boot ảnh ktest **trước** khi sửa, `-cpu max`:

```text
# umode: iretq entry=0x100000000f sp=0x1000002000 rflags=0x202 cs=0x1b ss=0x23 …
# exc: vec=14 err=0x1 rip=0x100410 cs=0x8 rflags=0x10002          ← ca âm SMAP (đã arm)
# exc: cr2=0x1000000000
# exc: vec=14 err=0x11 rip=0x1000000000 cs=0x8 rflags=0x10002     ← ca âm SMEP (đã arm)
# exc: cr2=0x1000000000
# smep/smap: … cpuid_smep=1 cpuid_smap=1 smep=1 smap=1 … smap_err=0x1 … smep_err=0x11 … hardened=1
ok 50 - smep/smap: bat va ca am
# exc: vec=14 err=0x6 rip=0x1000000027 cs=0x1b rflags=0x10202     ← **#PF cô lập thật** của ring 3
# exc: cr2=0x1000003000
# exc: frame rsp=0x14bf50 ss=0x23 task_rsp=0x1000002000
qemu-system-x86_64: terminating on signal 15                     ← boot DỪNG: 13 `ok`, 0 marker
```

Đỏ **đúng lý do**: `EXPECT_CR2` đã bị `exc_arm` (của 2 ca âm SMAP/SMEP) xoá ⇒ `#PF` thật rơi vào
nhánh `# exc:` chung + `halt(2)` thay vì `# vspace: cross fault … user=1`. Cùng ảnh trên `qemu64`
(2 phép đo SMEP/SMAP bị bỏ qua) ⇒ **50/50 TAP + marker** (`/tmp/w7b-418c/qemu64-01.log`) — tức lỗi
chỉ hiện khi CPU **có** SMEP/SMAP.

### 1.2 `-cpu max` aarch64 — `msr pan` sai mã hoá ⇒ trap `ec=0x00` (log `/tmp/w7b-418c/a64full-max.log`)

Boot đầy đủ (TCP server + GPU + bàn phím) **trước** khi sửa: 49 `ok`, rồi dừng ở `TAP 50`:

```text
# trap: slot=4 off=0x200 ec=0x00 il=1 far=0x400b4d74 elr=0xffffffc08000f894 spsr=0x3c5
```

`elr` = đúng lệnh `msr S0_3_C4_C1_3, xzr` (giải mã `llvm-objdump` của ảnh: `0x4008f894`) — tức
`.inst 0xD503417F` trong `pan_set(true)`. Không có `# pan:`, không marker.

Đối chiếu mã hoá **bằng hợp dịch thật** (máy đo, `clang` của macOS):

```text
$ clang --target=aarch64-none-elf -march=armv8.2-a -c pan.s
$ od -An -tx1 pan82.o -j64 -N16
 9f 41 00 d5  9f 40 00 d5  df 42 03 d5  7f 41 00 d5
   ↑ msr pan,#1 = 0xD500419F   ↑ msr pan,#0 = 0xD500409F   ↑ msr daifset,#2   ↑ msr uao,#1
```

⇒ `op1` của `PAN` là **`0b000`** (bản `KT-418` dùng `0b011` = vùng `DAIF`), `op2 = 0b100`.

### 1.3 `vifast-plat-aarch64` không biên dịch được cho host `x86_64` (build log)

```text
$ cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --target x86_64-apple-darwin
error: invalid operand for instruction  (×40, tại asm/fpu.rs: "stp q0, q1, [{b}, #0]" …)
   --> vifast-plat-aarch64/src/asm/fpu.rs
```

Đỏ **đúng lý do**: inline asm aarch64 nằm ngoài `cfg` ⇒ host `x86_64` không dịch được (host
`aarch64-apple-darwin` thì lọt, nên lỗi chỉ lộ khi **đổi máy đo** — đúng ý `D2-262`).

### 1.4 Preempt ring-3 — bằng chứng **không tồn tại** trước khi sửa (nợ `KT-413c` §6.1)

`grep -rn 'sched: cpl3' v2/` = **0** dòng trước thay đổi; `dispatch.rs:17-20` tự ghi "**không** còn
khẳng định preempt ring 3 trong bằng chứng". Sau khi sửa, cùng grep có dòng thật trong log boot
(§3.1). Đỏ-trước ở đây là "hợp đồng bằng chứng chưa tồn tại" (khuôn `KT-413c` §2) + ca kiểm
`hardening-max.sh` (mục §4) **ĐỎ** khi thiếu dòng (`--selftest` ca 11).

## 2. Nguyên nhân gốc & cách vá

### 2.1 x86_64 — `EXPECT_CR2` bị xoá **sau** khi arm (thứ tự sai)

`v2/kernel/vifast-kbin-x86_64/src/umode.rs` (trước): arm ca cô lập **trước** `sec_probe::run`, mà
`sec_probe::run` gọi `exc_arm(VEC_PF, …)` hai lần (`sec_probe.rs:54,67`) và
`trap.rs::exc_arm` **xoá `EXPECT_CR2`** (`KT-418`, để `#PF` SMEP không bị nhánh `CR2` nuốt —
đúng, giữ nguyên). Vá **hai lớp**:

1. **arm lại** `exc_arm_user_pf(VSPACE_PEER_VA, resume)` **sau** `sec_probe::run`, ngay trước
   `enter_user` (một điểm arm duy nhất — bỏ lần arm sớm);
2. **cổng fail-closed mới** `trap.rs::user_pf_armed(cr2)` (`EXPECT_CR2 == cr2 && EXPECT_VEC == #PF`)
   kiểm ngay trước `iretq`: sai ⇒ in `# vspace: BAD arm …` + `halt(3)` thay vì để `# exc:` chung
   khó đọc. Đây là bất biến **mới**: một lần nữa đổi thứ tự sẽ chết **có tên** tại chỗ.

### 2.2 aarch64 — sửa mã hoá `PAN` + khoá bằng test host

`v2/kernel/vifast-plat-aarch64/src/asm.rs`: `pan_set` phát `.inst 0xD500419F` / `0xD500409F`
(`op1 = 0b000`, `op2 = 0b100`); hằng **`pub const MSR_PAN_ON` / `MSR_PAN_OFF`** là nguồn duy nhất
được test host giải mã lại từng trường (`tests/pan_encoding.rs`, 3 ca) — cùng khuôn `TLBI` của
crate. Ca thứ 3 của test khẳng định mã hoá **sai** cũ `0xD503417F` (vùng `DAIF`) không còn.

### 2.3 Di động host `aarch64 fpu.rs`

`use core::arch::asm` + hai thân `save_raw`/`restore_raw` bọc
`#[cfg(all(target_arch = "aarch64", target_os = "none"))]`; nhánh host là
`host_stub("save_raw"|"restore_raw")` — **panic có tên** (không no-op), đúng khuôn `asm.rs`
(`KT-167`). Trên ảnh, mã phát ra **không đổi một lệnh nào**.

### 2.4 Phủ preempt ring-3 (nợ `KT-413c` §6.1)

- `bootasm.rs`: trong `__user_entry`, **sau** ca `#PF` cô lập, chèn vòng quay **có trần**
  `__user_cpl3_spin … __user_cpl3_resumed` (`CPL3_SPIN = 0x0800_0000` ≈ 0,3–0,5 s ở TCG, PIT 20 ms).
- `trap.rs`: `cpl3_arm(spin_va, resume_va)` + `cpl3_on_tick(cs, rip)` — nhánh **timer** khi
  `cs == USER_CS` **và** `rip ∈ [spin, resume)` mới đếm + trả `FrameReturn::resume(<resume_va>)`
  ⇒ `iretq` **quay lại CPL3** đúng nhãn kernel chọn. **Giới hạn khoảng `rip`** là bắt buộc: đổi
  `RIP` vô điều kiện làm tick rơi trước lệnh ghi `VSPACE_PEER_VA` **nhảy qua** ca cô lập ⇒ đo được
  `not ok 14` trên `qemu64` (đã sửa, xem §3.2).
- `umode.rs`: `int 0x80` **từ CPL3** sau tick ⇒ in **một** dòng
  `# sched: cpl3 preempted=<n> resumed=1 cs=0x<cs>`; arm mà **không** tick nào ⇒ nhánh `Exit` in
  `# sched: cpl3 preempted=0 resumed=0 cs=0x0 (BAD: …)` (**ĐỎ có tên**, không treo boot).
- Vì sao **không** hardcode `cs=0x23`: `cs` là giá trị **CPU đẩy vào frame**. Hiện GDT `KT-261`
  cho `user_code = 0x1b` ⇒ in `0x1b`; sau `KT-417c` (`planned`, `user_code = 0x20`) **cùng mã** in
  `0x23` — kịch bản kiểm đối chiếu `cs` với dòng `# umode: iretq … cs=` nên **không** phải sửa khi
  đổi selector (`KT-417c` mục 4).

## 3. Bằng chứng **xanh** (log máy sinh)

### 3.1 `-cpu max` x86_64 — `/tmp/w7b-418c/hardening3/x64-max.log`

```text
# smep/smap: va=0x1000000000 cpuid_smep=1 cpuid_smap=1 smep=1 smap=1 smap_deferred=0 cr4=0x0000000000300020 smap_err=0x1 smap_cr2=0x1000000000 smap_ok=1 smep_err=0x11 smep_cr2=0x1000000000 smep_ok=1 hardened=1
ok 50 - smep/smap: bat va ca am
# vspace: cross fault vec=14 err=0x6 cr2=0x1000003000 cs=0x1b rip=0x1000000027 user=1
# sched: cpl3 preempted=1 resumed=1 cs=0x1b
ok 12 - umode: ring3 entered
# umode: int80 nr=12 cs=0x1b rip=0x1000000051 user=1      ← dừng ở đây: nợ SMAP §0.4
```

### 3.2 `qemu64` (baseline cổng cũ **không hồi quy**) — `/tmp/w7b-418c/q64-final.log`

```text
# vspace: cross fault vec=14 err=0x6 cr2=0x1000003000 cs=0x1b rip=0x1000000027 user=1
# sched: cpl3 preempted=1 resumed=1 cs=0x1b
ok 14 - umode: returned to ring3
…
ok = 50, not ok = 0, marker 1 lần, halt 1 lần
```

### 3.3 `-cpu max,-smap` (đối chứng: cả ảnh chạy hết) — `/tmp/w7b-418c/hardening3/x64-nosmap.log`

```text
# smep/smap: … cpuid_smep=1 cpuid_smap=0 smep=1 smap=0 … smep_err=0x11 … smep_ok=1 hardened=1
ok 50 - smep/smap: bat va ca am
# vspace: cross fault vec=14 err=0x6 cr2=0x1000003000 cs=0x1b …  ok 12 - umode: ring3 entered
# sched: cpl3 preempted=1 resumed=1 cs=0x1b
ok = 50, not ok = 0, marker 1, halt 1
```

### 3.4 `-cpu max` aarch64 (sau khi sửa `PAN`) — `/tmp/w7b-418c/hardening3/a64-max.log`

```text
# pan: va=0x70000000 pan_supported=1 ec=0x25 far=0x70000000 elr=0xffffffc08000f898 fault_ok=1 pan_off_read=0xd280018110000140 no_fault=1 restored=1 ok=1
ok 50 - pan: el1 khong doc trang el0
ok = 50, not ok = 0, '# viFastOs aarch64: boot ok' 1 lần, '# viFastOs aarch64: halt' 1 lần
```

`fault_ok=1` = data abort **thật** (`EC=0x25`, `FAR` đúng VA, `Access::Read`); `no_fault=1` =
`PAN=0` cùng lệnh **không** fault ⇒ fault là do `PAN`; `restored=1` = `PAN` bền sau phép đo.

### 3.5 Kịch bản kiểm `v2/test/boot/hardening-max.sh`

```text
$ bash v2/test/boot/hardening-max.sh --selftest
# hardening-max --selftest: 25/25 ca đúng như phải

$ bash v2/test/boot/hardening-max.sh          # 3 leg thật, exit 0
ok 1 - x86_64 -cpu max: SMEP/SMAP bật thật + 2 ca âm thật + cross fault + preempt CPL3
ok 2 - x86_64 -cpu max,-smap: cả ảnh chạy hết 50/50 TAP + marker + halt (đối chứng nợ SMAP)
ok 3 - aarch64 -cpu max: PAN bật thật + data abort thật + 50/50 TAP + marker + halt
# known-blocker: x86_64 -cpu max dừng sau TAP 13/50 — deadlock khoá 'Machine' khi CR4.SMAP=1; nợ KT-418d
```

`--selftest` phủ **từng** phép kiểm bằng ca âm (sửa đúng một trường ⇒ ĐỎ **có tên**), gồm các
bẫy thật: `smap=0` (hardened nhờ tắt), `smap_deferred=1`, `smap_err=0x0`, `smep_err=0x1`,
`cr4` thiếu bit 21, mất `# vspace: cross fault`, mất `ok 12`, `preempted=0`, `cs` lệch selector,
marker 2 lần, thiếu halt; **aarch64**: `pan_supported=0` (**xanh giả** — đúng kiểu runner cũ),
`fault_ok/no_fault/restored/ok = 0`, có trap `ec=0x00`, mất `ok 50`, một TAP đỏ, mất marker.
Đây là "khoá bằng ca kiểm" cho cả 3 thay đổi hành vi của task (không chỉ chạy tay một lần).

## 4. Ratchet `B1.1` (KHÔNG nới)

```text
$ python3.12 v2/tools/check-api-boundary.py --ratchet
# B1.1 ratchet: dep_edges=3 (đóng băng 3, mới 0) driver_files=23 (đóng băng 23, mới 0) stale=0
# B1.1: XANH (0 vi phạm, ratchet một chiều)
```

0 tệp mới trong `v2/kernel/` khớp mẫu `virtio_*`/`*tcp*`/`*_svc`; 0 cạnh dep mới; `b1.1-ratchet.toml`
**không** bị sửa. Tệp mới duy nhất ngoài `v2/kernel/`: `v2/test/boot/hardening-max.sh`,
`tests/pan_encoding.rs` (trong crate sẵn có, không phải tệp driver).

## 5. T0 / T1

- **T0**: `cargo test ... -p vifast-plat-aarch64` (2 ca cũ + 3 ca `pan_encoding`), `cargo test
  ... -p vifast-plat-common`; `cargo build -p vifast-plat-aarch64 --target x86_64-apple-darwin`
  (trước đỏ 40 lỗi → sau xanh); `cargo build` cả 3 ảnh ktest (x64/a64 + `qemu64`, `-cpu max`,
  `max,-smap`); `bash v2/test/boot/hardening-max.sh --selftest` (25/25). Log: `/tmp/w7b-418c/`.
**T0 (đo được)**:

```text
$ cargo test … -p vifast-plat-aarch64            → 8 passed (2 cũ + 3 pan_encoding + 3 tệp khác)
$ cargo test … -p vifast-plat-aarch64 --test pan_encoding → 3 passed, 0 failed
$ cargo build -p vifast-plat-aarch64 --target x86_64-apple-darwin → Finished (trước: 40 lỗi asm)
$ bash v2/test/boot/hardening-max.sh --selftest  → 25/25 ca đúng như phải
$ python3.12 v2/tools/check-api-boundary.py --ratchet → B1.1 XANH (dep_edges 3/3, driver_files 23/23, mới 0)
```

### 5.1 T1 lượt 1 — **ĐỎ 2 bước** (giữ lại vì là bài học thật, không phải flake)

`/tmp/w7b-418c/t1.log` (`# gate-steps registered=50 ran=50 skipped=0 failed=2`,
`TỔNG gate --dev 128 711 ms`):

1. `--- cổng selftest có cổng gọi (KT-134): FAIL` —
   `check-selftest-wiring: FAIL (1 script tự kiểm không cổng nào gọi: v2/test/boot/hardening-max.sh)`.
   Đúng luật: script có `--selftest` phải được **một cổng** gọi. Nhưng bước cổng nằm ở
   `v2/bin/lib/steps_gate.sh` — **quyền Lead** (`BRIEF` §3 cấm slot sửa `v2/bin/**`) ⇒ dùng đúng
   cơ chế **ngoại lệ có lý do** của chính công cụ (`NGOAI_LE`), kèm ghi chú **xoá khi Lead nối
   `gate --fast`** (khuôn `gpu.sh` của KT-263). Đây là **đề nghị của slot gửi Lead**, không phải
   nới luật: `hardening-max.sh` vẫn kiểm đủ 25 ca khi chạy tay/T2.
2. `--- workspace v2/kernel: clippy host-testable (KT-409): FAIL` — lỗi **của tôi**:
   `clippy::unusual_byte_groupings` ở `tests/pan_encoding.rs:23` (`0b110_1010_1000_00`).
   Sửa: `0b1_1010_1010_0000` / `0b1_1111`. **Không** `#[allow]`, không tắt lint.

### 5.2 T1 lượt 2 — **ĐỎ 1 bước**, và bước đó đỏ vì **cây bẩn** (không phải hồi quy)

`/tmp/w7b-418c/t1b.log`: `# gate-steps registered=81 ran=67 skipped=14 failed=1`,
`# step-time-total 564 548 ms`, bước đỏ duy nhất:

```text
--- selftest pre-measure (KT-95)
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)     ← 7 = `tree-dirty`
```

`v2/tools/pre-measure.sh:207-209`: `dirty != 0 ⇒ refuse 7 tree-dirty`. Worktree của agent **luôn**
bẩn trước commit ⇒ ca (a) của selftest (đòi cây **sạch**) không thể xanh trước commit — **đúng lớp
nợ** `KT-418b` §7.3 đã ghi (`api boundary` + `selftest pre-measure` từ chối trên cây bẩn,
fail-closed theo thiết kế, không phải lỗi của thay đổi).

**Chứng minh nguyên nhân (đo được)**: `git stash push -u` (cây sạch) rồi
`bash v2/tools/pre-measure.sh --selftest` ⇒ `# pre-measure-selftest: PASS (a/b/c/d/e/f/g)`
(exit 0); `git stash pop` khôi phục. Vì vậy thứ tự đúng là **commit rồi T1 lại trên cây sạch** —
T1 lượt 3 ghi ở §5.3 (không sửa ngưỡng, không tắt bước, không retry lấy xanh: bước này đỏ vì
trạng thái cây, và trạng thái đó **đổi được** bằng commit).

### 5.3 T1 lượt 3 — trên cây **sạch** (sau commit `[KT-418c]`)

## 5.4 Chặn commit — hook `vifast-guard` đòi `VIFAST_ROLE=lead` (KHÔNG tự đặt)

```text
$ git commit -F <message [KT-418c] …>
vifast-guard(pre-commit): TỪ CHỐI COMMIT — KT-85 / D2-123 mục (4)(b),(d)
  Lý do      : chưa đặt VIFAST_ROLE (không có trong môi trường / rỗng) ⇒ fail-closed
  vifast-guard: RESULT=DENY hook=pre-commit role=(chưa đặt)
```

Đúng `AGENTS.md` §7 ("Hook `vifast-guard` đòi `VIFAST_ROLE=lead` khi commit/push … Agent **không**
tự đặt biến này và **không** dùng `--no-verify`: stage xong thì báo lead commit"). Vì vậy slot
dừng ở trạng thái **đã stage toàn bộ** trong worktree `/private/tmp/vifast-wt/2`, nhánh
`task/kt418c-hardening-max` (HEAD vẫn `297b89dc`), và **báo Lead commit**:

```bash
VIFAST_ROLE=lead git -C /private/tmp/vifast-wt/2 commit -F <message [KT-418c] …>   # đã stage sẵn
```

Hệ quả lên T1: bước `selftest pre-measure (KT-95)` **chỉ xanh trên cây sạch** (§5.2) ⇒ T1 đầy đủ
chỉ có thể xanh **sau** commit của Lead; lượt 2 đo được **66/67 bước xanh**, 1 bước đỏ đúng lý do
"tree-dirty" (đã chứng minh bằng `git stash`: `pre-measure-selftest: PASS (a/b/c/d/e/f/g)`).

## 6. Nợ / rủi ro (trung thực)

1. **`-cpu max` x86_64 chưa chạy hết boot (nợ `KT-418d`, đề xuất)**: deadlock khoá `Machine`
   (`IrqMcsLock<()>` — xác định bằng `RIP` trong `llvm-objdump` + `info registers` của QEMU:
   `RIP=0x128c19` nằm giữa `IrqMcsLock::lock_irqsave`), chỉ khi `CR4.SMAP=1`; `-cpu max,-smap`
   ⇒ 50/50 + marker; `-cpu max,-smep` **vẫn** deadlock; `-smp 1` cũng deadlock. Đây là **lỗi
   khác** (không phải `VSpace`), cần task riêng: nghi vấn `stac`/`clac` + khoá (đường `SMAP` chưa
   từng chạy trên cổng vì `qemu64` không có SMAP).
2. **`PAN` chỉ được kiểm trên `-cpu max`** (kịch bản mới). `run-aarch64.sh`/`soak-aarch64.sh` vẫn
   `-cpu cortex-a72` ⇒ TAP 50 của chúng đi nhánh `pan_supported=0` (xanh **vô nghĩa** cho PAN).
   Đề nghị: hoặc thêm `-cpu` cấu hình được cho runner aarch64 (task riêng, 0 chồng tệp với
   `KT-408e`), hoặc chấp nhận `hardening-max.sh` là nơi duy nhất kiểm PAN thật.
3. **`# sched: cpl3` in `cs=0x1b`** hôm nay (GDT `KT-261`); `KT-417c` đổi GDT `planned` ⇒ cùng mã
   in `cs=0x23`, **không** phải sửa gì; kịch bản kiểm đối chiếu `cs` với `# umode: iretq` nên không
   hardcode selector.
4. **Vòng quay CPL3 tốn ~0,3–0,5 s/boot** (chỉ khi tới pha ring 3 của ảnh ktest). Với
   `run-x86_64.sh` (N boot) và **đặc biệt `soak-x86_64.sh` (1.000 boot)** đây là chi phí thật
   (~+0,4 s/boot ⇒ ~+7 phút cho soak). Nếu Lead thấy đắt: giảm `CPL3_SPIN` (vẫn phải > 20 ms để
   chắc ≥ 1 tick) — **không** đổi ngưỡng bằng chứng nào, chỉ đổi số vòng quay.
5. **Chưa chạy `mutants`, `nightly`, `soak`, L1-medium** (ngoài phạm vi W7b). Không sửa `spec/`.

## 7. Tệp đã đổi

| Tệp | Loại | Nội dung |
|---|---|---|
| `v2/test/boot/hardening-max.sh` | **mới** | 3 leg QEMU thật + `--selftest` 25 ca; khoá bằng chứng hardening `SMEP`/`SMAP`/`PAN`/`VSpace`/preempt-CPL3 |
| `v2/kernel/vifast-plat-x86_64/src/**` | **0 thay đổi** | (không cần: nguyên nhân gốc nằm ở ảnh `kbin-x86_64`) |
| `v2/kernel/vifast-plat-aarch64/src/asm.rs` | sửa | sửa mã hoá `PAN` (`0xD500419F`/`0xD500409F`) + `pub const MSR_PAN_ON/OFF` |
| `v2/kernel/vifast-plat-aarch64/src/asm/fpu.rs` | sửa | `cfg(all(target_arch="aarch64", target_os="none"))` cho `asm!` + host stub panic |
| `v2/kernel/vifast-plat-aarch64/tests/pan_encoding.rs` | **mới** | 3 ca giải mã bit-field `MSR (immediate)` cho `PAN` |
| `v2/kernel/vifast-kbin-x86_64/src/trap.rs` | sửa | `user_pf_armed`, trạng thái + `cpl3_on_tick/report`, nhánh timer CPL3 (giới hạn `rip`) |
| `v2/kernel/vifast-kbin-x86_64/src/umode.rs` | sửa | arm lại sau `sec_probe` + cổng fail-closed; arm CPL3; in dòng `# sched: cpl3` + nhánh BAD |
| `v2/kernel/vifast-kbin-x86_64/src/bootasm.rs` | sửa | vòng quay `__user_cpl3_spin…__user_cpl3_resumed` |
| `v2/kernel/vifast-kbin-x86_64/src/product.rs` | sửa | `CPL3_SPIN` |
| `v2/kernel/vifast-kbin-x86_64/src/main.rs` | sửa | re-export `cpl3_*`, `user_pf_armed` |
