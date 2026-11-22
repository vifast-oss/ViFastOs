# KT-345a — Áp `spec/` ABI v1.1 (D2-242) + `VspaceOutcome` rollback — sub-agent `abi11`

**Nhánh:** `task/kt345a-abi` · **Base:** `main` @ `d0c5817c` · **Worktree:** `/private/tmp/kt345a-s1`
**Ngày:** 2026-11-22 · **Quyền đặc biệt:** chỉ được sửa `spec/` đúng **9 mục** D2-242 (D2-234 khoá lại sau `DONE`).

## 0. Chốt một câu

Đã áp **đủ 9 nhóm** của D2-242 lên `spec/` + phần sinh code/checker/tests đi kèm, **đổi `SyscallEnv::tcb_vspace` sang ba trạng thái + rollback** (mục 3.1A) với **đột biến bỏ rollback ⇒ ĐỎ**, giữ `size_of::<Machine>()` **184.792 B**, và nối `check_kernel.py` vào **T2 `do_gate_fast` id 77** theo chỉ đạo (b) của lead. Không đổi `abi_version` (1), không đổi `min_compat` (1), không đóng băng thêm byte wire nào.

## 1. Bảng 9 mục D2-242 — đạt/không, tệp, bằng chứng

| # | Mục D2-242 | Đã làm gì | Tệp | Bằng chứng |
|---|---|---|---|---|
| 1 | **Nhóm 1 (`1C`)** — `spec/kernel.toml` tối thiểu + máy kiểm theo **tên hằng** | Tạo `spec/kernel.toml` (`status = "APPROVED"`, đủ `[limits]`/`[limits.core]`/`[limits.plat]`/`[mailbox]`/`[features]`/`[arch.x86_64.gdt]`/`[reserved]`, mỗi giá trị `{value, source}` trỏ **tên hằng** thật). Tạo `check_kernel.py` (37 giá trị; giải biểu thức hằng như `(INDEX_TSS as u16) * 8`; **8 ca** selftest: 1 dương + 7 âm). Nối vào `v2/bin/v2`: `step_kernel` + **`lane_step D 77`** (T2, id 77, làn D chỉ đọc) + `GATE_FAST_EXTRA_EXPECTED` 29→30. | `spec/kernel.toml` (**mới**), `v2/test/kernel/check_kernel.py` (**mới**), `v2/bin/v2` | `python3 v2/test/kernel/check_kernel.py` → `PASS (37 giá trị…)`; `--selftest` → `# selftest: 8/8 ca đúng như kỳ vọng (1 dương + 7 âm)`; `bash -n v2/bin/v2` OK; không trùng id `lane_step`; `v2 check` PASS |
| 2 | **Nhóm 2 (`2C`)** — `[ipc_buffer]` đóng băng vùng extra caps | Thêm `[ipc_buffer]` đúng 7 số D2-242 (`bytes=4096, align=16, mr_offset=80, mr_words=60, extra_caps_offset=560, extra_caps_max=4, extra_caps_dst_offset=600`), `configure_offset=608` + `configure_offset_status="[TK]"` (**KHÔNG** đóng băng). `gen.py` sinh `IPC_BUFFER_*`; **không** sửa `[message_info]` 35 bit. | `spec/abi.toml`, `v2/tools/vifast-gen/gen.py`, `v2/lib/vifast-abi/src/generated.rs` | `git diff spec/abi.toml` không chạm `[message_info]`; `./v2/bin/vifast-gen --check` PASS; `check_abi.py` kiểm 8 số + `[TK]` |
| 3 | **Nhóm 3 (`3.1A + 3.2A`)** — kênh lỗi `VSpace` + luật ô 0 `CSpace` + **`VspaceOutcome` rollback** | `[method_signature."Tcb.Configure"].errors = [InvalidCapability, WrongObjectType, InsufficientRights, InvalidArgument, Busy]`; thêm `[cspace]` (`slot0=CS_SELF`, `cptr_zero=absent`, `vspace_zero=clear`). Đổi `SyscallEnv::tcb_vspace` → `VspaceOutcome { Applied, Unsupported, Failed(KError) }` (mặc định `Unsupported`); `configure` rollback `TcbData.vspace` khi `Failed` + trả mã lỗi; riscv64 map bảng khe tràn → `Failed(Busy)`; host test mới + **đột biến**. **Không** thêm trường `TcbData`. | `spec/abi.toml`, `v2/kernel/vifast-core/src/{syscall,invoke,core_tcb,configure_tests}.rs`, `v2/kernel/vifast-plat-riscv64/src/user.rs` | `cargo test -p vifast-core --lib` → **98 passed**; đột biến bỏ rollback ⇒ `configure_vspace_hook_that_bai_rollback` **FAILED** (`left: 0, right: 8`), khôi phục ⇒ PASS |
| 4 | **Nhóm 4 (`4C`)** — `meminfo`/`fsstat` IDL | Tạo `spec/protocols/meminfo.vidl` (`total`, `free` → u64) và `fsstat.vidl` (`blocks`, `blocks_free`, `block_size` → u64); **không** thêm syscall/object. | `spec/protocols/{meminfo,fsstat}.vidl` (**mới**), `v2/lib/vifast-idl/src/generated_{meminfo,fsstat}.rs` (sinh) | `./v2/bin/vifast-idl --check` PASS; `cargo test -p vifast-idl --lib` 9/9 PASS |
| 5 | **Nhóm 5 (`5A`)** — hình học GDT + `planned` | `[arch.x86_64.gdt]` ghi hình học **hiện tại** (`entries=7`, index/selector 0x08/0x10/0x18/0x20/0x28, TSS 2 entry, `sysret_ready=false`, `descriptors`) + `[arch.x86_64.gdt.planned]` (thứ tự đích `null,kcode,kdata,**udata,ucode**,TSS`; `star_63_48=0x10`; `trap="int 0x80"`; `min_compat=1`). **Không** sửa `gdt.rs`, **không** đổi `trap` trong ABI. | `spec/kernel.toml`, `v2/bin/v2` (port payment) | `check_kernel.py` kiểm `entries = len(descriptors) + (TSS−1)`, `selector = index × 8`, `planned.order` đúng D2-242; `git diff v2/kernel/vifast-plat-x86_64/src/gdt.rs` rỗng |
| 6 | **Nhóm 6 (`6A`)** — socket IDL + generator quét `spec/protocols/` | Tạo `socket.vidl` (`open/connect/send/recv/poll/close`, `handle u32`); `gen_idl.py` sinh thêm 3 stub từ `spec/protocols/*.vidl` (ring/pager **giữ nguyên** đường cũ); nối module + test trong `vifast-idl`; fixture IDL chép thêm `spec/protocols/`. | `spec/protocols/socket.vidl` (**mới**), `v2/tools/vifast-gen/gen_idl.py`, `v2/lib/vifast-idl/src/{lib.rs,generated_socket.rs}`, `v2/test/gen/run_idl_fixtures.sh` | `./v2/bin/vifast-idl --check` PASS; `bash v2/test/gen/run_idl_fixtures.sh` → `# idl fixtures: PASS`; `cargo test -p vifast-idl --lib` 9/9 |
| 7 | **Nhóm 7 = `7C-r13`** — hợp đồng trả về `x86_64` | `[calling.x86_64]` thêm `return_message_info="r13"`; `clobbered=["rax","rdx","r13","r10","r8","r9","r12"]` (7); `preserved=["rsp","rbx","rbp","r14","r15"]` (r12 ra). `calling.rs`: `INT80_RETURN_MSGINFO_REG=Reg::R13`, `INT80_CLOBBERED` đủ 7, `INT80_MR_REGS` = `SYSCALL_MR_REGS` (một hợp đồng); host test khoá 7C-r13 (r13 ≠ rdx, rcx/r11 trống cho CPU). | `spec/abi.toml`, `v2/kernel/vifast-plat-x86_64/src/calling.rs`, `v2/kernel/vifast-plat-x86_64/tests/host.rs`, generated.rs | `cargo test -p vifast-plat-x86_64 --test host syscall_proposal_registers` → PASS; `check_abi.py --selftest` ca "return_message_info r13→r11 ⇒ đỏ" PASS |
| 8 | **Nhóm 8 (`8C`)** — `[object_size]` một model + 3 alias; `[debug_info].return_channel` theo vai trò | `[object_size]`: `model` + `riscv64/aarch64/x86_64 = { same_as = "model" }`; `gen.py` giải alias, **`die`** nếu thiếu/lệch. `[debug_info].return_channel = { value="abi_version", mr=["min_compat","features","num_harts"] }`; `gen.py` suy kênh per-arch từ `[calling.*]` (thiếu/trùng thanh ghi ⇒ `die`), sinh `*_DEBUG_INFO_RETURN_*`; `check_abi.py` kiểm 3 arch bằng nhau + kênh khớp. | `spec/abi.toml`, `v2/tools/vifast-gen/gen.py`, `v2/lib/vifast-abi/src/generated.rs`, `v2/test/abi/check_abi.py` | `gen-check` PASS; `check_abi.py` PASS; selftest ca "aarch64 trỏ alias khác ⇒ đỏ" + "đổi vai trò value ⇒ đỏ" |
| 9 | **Nhóm 9 (`9B`)** — trạng thái object VM | `Frame` → `stability = "frozen"` (Map/Unmap đã cài); `VSpace`/`PageTable` → `"core"`; method chưa cài giữ `unsupported = true` + `install_at = "Chu-ky-3"` (**không** mở method ABI mới). `check_reserved.py` còn **5** reserved `12..16`; `vifast-abi/src/lib.rs` cập nhật (CORE 11 / RESERVED 5). Đồng bộ consumer để **không đổi hành vi**: `is_core()` của model/sim/core = 11 object; guard `NotSupported` tường minh cho Retype 3 object VM (model/core); `mem_bridge` riscv64 tra **hợp** `CORE_OBJECTS ∪ RESERVED_OBJECTS`. | `spec/abi.toml`, `v2/test/abi/check_reserved.py`, `v2/lib/vifast-abi/src/lib.rs`, `v2/lib/vifast-model/src/{object,machine}.rs`, `v2/sim/vifast-sim/src/kobj.rs`, `v2/kernel/vifast-core/src/{error,machine}.rs`, `v2/kernel/vifast-plat-riscv64/src/mem_bridge.rs` | `check_reserved.py` → `PASS (5 object reserved)`; `check_reserved.py --selftest` 5/5; `cargo test -p vifast-core --lib` 98 passed; `cargo test -p vifast-sim kobj::tests::` 27 passed; `cargo test -p vifast-model` green |

## 2. `spec/` diff — số dòng

| Tệp | Trạng thái | Số dòng |
|---|---|---|
| `spec/abi.toml` | sửa | **+69 −10** (`git diff --numstat`: `69 10`) |
| `spec/kernel.toml` | **mới** | **115** |
| `spec/protocols/socket.vidl` | **mới** | **49** |
| `spec/protocols/meminfo.vidl` | **mới** | **16** |
| `spec/protocols/fsstat.vidl` | **mới** | **20** |
| **Tổng `spec/`** | | **269 dòng thêm / 10 dòng bớt** |

Không chạm: `[message_info]` (35 bit), `[syscall]` 0..15, `[error]` 19 mã, `[frozen]` 5 thứ, `[method_signature]` khác, `spec/v1-parity.toml`; `abi_version = 1`, `min_compat = 1`.

## 3. `size_of::<Machine>()` trước/sau — **nguyên văn**

```
# tls pin: size_of_machine=184792 pin_cu=188872 delta=-4080 size_of_tcbdata=824 size_of_message=816 max_tcbs=64 8xmax=512 kt282=1536 kt309_tcb=512 kt309_msg=3072 kt309_scalar=16 kt311_tcb=512 kt312_giam=10240 tinh=-4080 khop=1 duoi_pin=1
```

- **Trước** (base `d0c5817c`, cùng pin test / `v2/agent/JOURNAL/kt325-vspace2.md` §3): **184.792 B**.
- **Sau** (worktree này, dòng trên): **184.792 B** — Δ = **0**; `khop=1 duoi_pin=1`, trần KT-312 `184.832` còn nguyên 40 B; **không** thêm trường `TcbData` (rollback chỉ ghi lại trường `vspace` có sẵn).

## 4. Ba máy kiểm `--selftest` — kết quả nguyên văn

```
$ python3 v2/test/kernel/check_kernel.py --selftest
# selftest: 8/8 ca đúng như kỳ vọng (1 dương + 7 âm)

$ python3 v2/test/abi/check_abi.py --selftest
# selftest: 8/8 ca đúng như kỳ vọng (1 dương + 7 âm)

$ python3 v2/test/abi/check_reserved.py --selftest
# selftest: 5/5 ca đúng như kỳ vọng (1 dương + 4 âm)
```

Ca âm của `check_kernel.py` (ĐỎ **đúng mã**): `E_STATUS`, `E_CONST_LECH` (đúng tên `MAX_TCBS`), `E_CONST_MISSING`, `E_SOURCE_FILE`, `E_SECTION` (mất `[mailbox]`), `E_SOURCE_THIEU`, `E_GDT_PLANNED_ORDER`.

## 5. Đột biến **bắt buộc** — bỏ rollback ⇒ test ĐỎ (chứng minh rollback có răng)

Đột biến (i): trong `vifast-core::invoke::configure`, thay nhánh
`VspaceOutcome::Failed(err) => { if let Some(prev) = prev_vspace { let _ = m.tcb_restore_vspace(c.obj, prev); } … }`
bằng `let _ = prev_vspace;` (bỏ rollback), chạy:

```
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-core configure_vspace_hook_that_bai_rollback
thread 'configure_tests::configure_vspace_hook_that_bai_rollback' panicked:
assertion `left == right` failed: rollback: TcbData.vspace phải về giá trị TRƯỚC (A), không giữ giá trị hook từ chối
  left: 0
 right: 8
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 97 filtered out
```

Khôi phục mã ⇒ `test result: ok. 1 passed` (đã chạy lại, xem §6). Vì vậy rollback **có răng**, không phải nhánh chết.

## 6. Cổng — log chính

### 6.1 `./v2/bin/v2 check` → **PASS** (rc = 0)

```
$ ./v2/bin/v2 check ; echo rc=$?
rc=0
# gate-steps registered=44 ran=44 skipped=0 failed=0
# step-time-total    78232 ms  TỔNG check
```

### 6.2 `cargo test` (map lệnh yêu cầu → lệnh thật; `vifast-gen` **không** phải crate Rust)

- `cargo test -p vifast-abi -p vifast-gen -p vifast-core` — **`vifast-gen` là công cụ Python**
  (`v2/tools/vifast-gen/gen.py`, không có `Cargo.toml`), nên `-p vifast-gen` không tồn tại
  (`package ID specification did not match`). Đã chạy tương đương:
  - `cargo test --manifest-path v2/Cargo.toml -p vifast-abi -p vifast-idl -p vifast-model` → **xanh**
    (abi 6 test, idl 9, model 6 unit + 10 integration…);
  - `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-core --lib` → **`98 passed; 0 failed`**;
  - `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common -p vifast-plat-aarch64 -p vifast-plat-x86_64` → **xanh** (đúng bộ host-buildable như `step_l0_kernel`; `vifast-plat-riscv64` chỉ build cho target);
  - `cargo test --manifest-path v2/Cargo.toml -p vifast-sim kobj::tests::` → **`27 passed`**;
  - `./v2/bin/vifast-gen --check` → `gen-check: PASS`; `./v2/bin/vifast-idl --check` → `PASS` (thay cho
    `cargo test -p vifast-gen`, vì gen là Python).
- `bash v2/test/boot/ktest.sh 1 1` → xem §6.3.

### 6.3 `bash v2/test/boot/ktest.sh 1 1` (riscv64, 1 boot) → **PASS**

Cây **sạch** tại commit code **`ae8cf583`**, `rev-gate` chấp nhận:

```
REV-GATE: ACCEPT inputs=50dbfbec1cf58615 current_inputs=50dbfbec1cf58615 head_rev=ae8cf58356d0 count=349
```

Dòng tổng kết **nguyên văn** (cuối log):

```
# KTEST: 9/9 boot sạch (smp=1, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
```

Bảng TAP từng ảnh: `ktest 70/70 PASS`, `ktest-guard 3/3 PASS`, `ktest-user 37/37 PASS`,
`ktest-proc 10/10 PASS`, `ktest-vblk 6/6 PASS`, `ktest-con 5/5 PASS`, `ktest-exec 6/6 PASS`,
`ktest-ipc 11/11 PASS`, `ktest-netsrv 3/3 PASS`; `infra after: … qemu_foreign=0 … verdict=OK`.

Ảnh `ktest` vẫn in đúng pin sizeof — bằng chứng độc lập với host test:

```
# core: link addr=0x80255800 bytes=184792 pin=196608 image_cap=262144 parts=29 sum=184792 scalars=311 padding=1 sum_ok=1 in_image=1
```

**Ghi chú tái dùng bằng chứng (AGENTS §6.3)**: bằng chứng `ktest`/`rev-gate` thuộc commit
`ae8cf583`; commit kế tiếp **chỉ** thêm/cập nhật tệp `v2/agent/JOURNAL/kt345a-abi11.md` (không nằm
trong tập build-inputs `v2/kernel v2/lib v2/Cargo.toml v2/Cargo.lock v2/rust-toolchain.toml
v2/tools/repro-build.sh v2/tools/rustc-canon.rs`) ⇒ `inputs=50dbfbec1cf58615`, `dirty=0` và ảnh
không đổi; lý do tái dùng được ghi ở đây, không chạy lại QEMU để "lấy xanh" (D2-07).

### 6.4 Kiểm `v2/bin/v2` theo chỉ đạo lead (nối cổng **T2-only**, id 77)

```
$ bash -n v2/bin/v2
bash -n OK

$ grep 'lane_step' v2/bin/v2 | grep -v '^\s*#' | awk '{print $3}' | sort -n | uniq -d
(rỗng — không trùng id)

$ grep -n 'lane_step' v2/bin/v2 | grep -c ''
83

$ grep '^\s*lane_step' v2/bin/v2 | awk '{print $3}' | sort -n | tr '\n' ' '
# 1 2 … 76 77   (đăng ký: 44 của `check` + 30 của `gate --fast` + 1 của `gate --dev` + 2 dòng kế hoạch nightly)

$ python3 v2/tools/check-selftest-wiring.py --root . --standalone-red
check-selftest-wiring: PASS (31 script tự kiểm: 31 có cổng, 0 ngoại lệ, 0 mồ côi)
```

Ghi chú lệnh nguyên văn của lead `grep -n 'lane_step' v2/bin/v2 | awk '{print $3}'` in ra **tên làn**
(`A..F, main, sau, sau2, sau4, sau5, sau6`) vì `grep -n` chèn `số:` vào cột 1 ⇒ cột 3 là **làn**, không
phải id; lệnh đúng là **bỏ `-n`** (đã chạy, rỗng). Tổng **83** dòng khớp chuỗi `lane_step`, trong đó **77**
dòng đăng ký thật; id **41..77** đủ và duy nhất (id 77 mới).

## 7. Xác nhận phạm vi (D2-242 / D2-234)

- **Chỉ** sửa `spec/` theo đúng **9 mục** D2-242; không mục nào khác (xem §2 — không chạm
  `[message_info]`/`[frozen]`/`[syscall]`/`[error]`/`spec/v1-parity.toml`; không mở method ABI mới).
- **KHÔNG** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`; **KHÔNG** push/merge/chạm `main`;
  **KHÔNG** `git add -A` (stage từng đường dẫn cụ thể); **KHÔNG** nới ngưỡng nào.
- Bổ sung ngoài `spec/` (được phép, thuộc 9 mục + cổng C của lead):
  - `check_abi.py`/`check_reserved.py` thêm `--selftest` + được gọi từ `step_abi`/`step_abi_reserved`
    (yêu cầu cổng C + `check-selftest-wiring.py` KT-134). Không đổi nội dung ABI;
  - consumer `is_core`/`mem_bridge`/retype-guard của item 9 để giữ **nguyên hành vi** khi nhãn
    `stability` đổi (không thêm tính năng, không đổi mã lỗi ABI).

## 8. Ghi chú triển khai `v2/bin/v2` (đã báo lead, làm theo (b))

Lead chốt **(b) — chỉ nối `do_gate_fast` (T2)**, id **77**, làn **D**, `GATE_FAST_EXTRA_EXPECTED=30`,
không đụng `do_gate_dev`. Đã làm đúng; **hai điều chỉnh kỹ thuật tối thiểu** (đã ghi rõ để lead duyệt):

1. `step_kernel` dùng **`step_py`** (chọn Python ≥3.11) thay vì `python3` trần: máy đo có
   `/usr/bin/python3` = **3.9.6** (không có `tomllib`) ⇒ `python3` trần sẽ làm bước 77 đỏ oan.
2. Trong cùng `step_kernel` có **lời gọi `--selftest`** (`check_kernel.py … && check_kernel.py --selftest`)
   vì `check-selftest-wiring.py` (KT-134, step 19 của `v2 check`) đòi mọi script tự kiểm phải có lời gọi
   thật kèm cờ trong `v2/bin/v2`; thiếu ⇒ `E_ORPHAN` làm `v2 check` đỏ. Không thêm `lane_step` nào.
