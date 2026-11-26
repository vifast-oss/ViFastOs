# KT-417 — Cửa sổ `spec/` ABI Sạch + `[boot_caps]` + `Tcb.{Bind,Unbind}Notification` + `CNode.{Mutate,Rotate}`

- **Task**: `KT-417` (P0, đường găng) — `QUEUE.md:475`, `D2-261` (APPROVED 2026-11-26 Agent 0).
- **Người thi hành**: Lead (slot 1 W7a), Giờ Cao điểm 1 `2026-11-26` (một luồng, `D2-256`).
- **Ranh giới**: **0** chạm `vifast-plat-x86_64` / `vifast-kbin-x86_64` (và 0 chạm aarch64);
  `vifast-kbin-riscv64` chỉ **2 tệp TAP/harness** (`ktest_capa.rs`, `ktest_inv.rs` — điểm tích hợp
  buộc phải sửa khi thêm trường stats/method); `spec/` chỉ sửa trong
  đúng cửa sổ này; giữ `abi_version = 1`, `min_compat = 1`; giữ `[limits.plat]` &
  `sysret_ready = false` (tới `KT-417b`/`KT-417c`).

## 1. Hợp đồng đã áp (theo `D2-261`)

`spec/abi.toml`:
1. `[object].Tcb` thêm **method 16 `Exec`** (`unsupported = true, install_at = "KT-420"`); tham số
   **cap `Frame`**, KHÔNG `path`, KHÔNG `Untyped+offset`; `VSpace.Map` vẫn **CẤM**.
2. `[method_signature]` thêm 13 chữ ký: `Tcb.{BindNotification,UnbindNotification,Exec}`,
   `CNode.{Mutate,Rotate}`, `IrqControl.{GetLine,GetMsi}`, `IrqHandler.{SetNotification,Ack,Clear,SetAffinity}`,
   `PageTable.{Map,Unmap}`. `Irq*` giữ `reserved` + `unsupported` tới `KT-419`; `PageTable.*`
   `unsupported` tới `KT-417b`; `Tcb.Exec` tới `KT-420`.
3. `[boot_caps]`: `boot_abi = 1` (khoá **riêng**, độc lập `abi_version`), sentinel
   `none_cspace = 0xFFFF` / `none_proc = 0xFF`; **2 bảng không gian chỉ số**:
   `[boot_caps.cspace]` = 17 `EP_*` + `self_cspace` + `tcb[5]` + `sc[5]` + 3 `loop_*` (nguồn
   `proc_abi.rs`, đối chiếu bằng máy), `[boot_caps.proc]` = `procsrv`/`shell` + `test.child`/
   `product.netsrv` (cùng ô **2**, khác ảnh).

`spec/kernel.toml`:
4. `[profiles.mission]` (`StaticStore` + `NoopObserver` + `limits.core` + pin `196608`),
   `[profiles.general]` (`UntypedStore` + 12 hằng `GENERAL_*` + luật `BootArena` pin bằng
   `#text:` trỏ `untyped_store.rs` — **không** chép số đo `617208`), `[fpu]`
   (`FPU_REGS/REG_BYTES/CTRL_WORDS/AREA_BYTES=528/SLOTS=65`).

Sinh tự động: `v2/lib/vifast-abi/src/generated.rs` (+`BootCapLayout` `#[repr(C)]`,
`BOOT_CAPS_TEST`/`BOOT_CAPS_PRODUCT`, `BOOT_ABI`, 2 sentinel) và `docs/abi.md`.

## 2. Cài đặt trong `vifast-core`

| Thành phần | Vị trí |
|---|---|
| Whitelist + dispatch 13/14 + arity/handler | `src/invoke.rs` (`bind_notification`, `unbind_notification`, `CNODE_MUTATE`, `CNODE_ROTATE`, `TCB_EXEC`) |
| `CNode.Mutate` (**tại chỗ**, `D2-44`) | `src/machine/lifecycle.rs::mutate` |
| `CNode.Rotate` (**hoán vị hai slot**, `[TK]`) | `src/machine/lifecycle.rs::rotate` |
| Đọc binding cho test | `src/core_sync.rs::tcb_bound_notification` |
| Test host (7 ca) | `src/invoke417_tests.rs` |

`Mutate`: `rights?` phải ⊆ quyền hiện có **và** ⊆ `RIGHTS_ALL`; `badge?` theo quy ước `Mint`
(`None` giữ, `0` xoá, chỉ Endpoint/Notification); **không** đổi slot/MDB.
`Rotate`: swap nội dung **hai** slot của **cùng** CNode; giữ nguyên cha/con MDB; hai slot rỗng ⇒
`SlotEmpty` 8; khác CNode ⇒ `InvalidArgument` 5. (Chữ ký 2 slot là `[TK]` — phép quay N slot nếu
`D2` sau cần thì mở rộng.)

## 3. Bằng chứng đỏ-trước (test có răng)

Gỡ **4 nhánh dispatch** (`(Tcb,13)`, `(Tcb,14)`, `CNODE_MUTATE`, `CNODE_ROTATE`) khỏi `invoke.rs`
rồi chạy `cargo test -p vifast-core invoke417`:

```text
test result: FAILED. 2 passed; 5 failed; 0 ignored; 0 measured; 126 filtered out
failures:
    invoke417_tests::bind_busy_khi_notification_da_gan_tcb_khac
    invoke417_tests::bind_unbind_notification
    invoke417_tests::mutate_doi_quyen_va_badge
    invoke417_tests::mutate_kiem_quyen_va_arity
    invoke417_tests::rotate_hoan_vi_hai_slot
... assertion `left == right` failed: left: Err(18)  right: Ok((0, 0))
```

Khôi phục nguyên văn (`md5` trùng `c1ec604036a3f3294347f6028eba90f8`) ⇒ xanh. Hai ca còn lại
(`so_method_va_boot_caps_sinh_tu_spec`, `exec_chua_cai_tra_not_supported`) không phụ thuộc nhánh
dispatch nên vẫn xanh ở lượt đỏ — đúng thiết kế.

## 4. T0 / kiểm máy

- `./v2/bin/v2 t0 vifast-core`: **PASS** (fmt + clippy `-D warnings` + **133 test**, 0 fail).
- `check_abi.py` + `--selftest` **10/10**; `check_reserved.py` PASS; `check_kernel.py` + `--selftest`
  **10/10** (`54` giá trị đối chiếu theo tên hằng); `gen-check` PASS; `verify-numbers --quiet`
  `ok=5 fail=0`.
- Hai lỗi công cụ phát hiện & vá trong máy kiểm (không nới ngưỡng):
  (a) `check_kernel.py::_Expr` không đọc literal có dấu `_` (`196_608`) → vá nhánh số;
  (b) `[profiles.general].max_harts` trỏ `store.rs#GENERAL_MAX_HARTS` nhưng hằng đó là **import**
  (`MAX_HARTS` từ `machine.rs`) → trỏ đúng nguồn `machine.rs#MAX_HARTS`.
- `v2 check`: mọi bước xanh **trừ** `api boundary (KT-241)` — harness compile-fail đó **fail-closed
  khi cây bẩn** (từ chối ghi đè `vifast-core/src/lib.rs`), nên phải chạy trên cây đã commit; đây là
  giới hạn quy trình, không phải lỗi của thay đổi.

## 5. T2 `gate --fast` trên cây sạch — lượt 1 (ĐỎ 2 bước, đã phân loại)

`# gate-steps registered=81 ran=81 skipped=0 failed=2` (TỔNG 673.532 ms):

1. **`selftest runner ktest (KT-54)` — lỗi THẬT của thay đổi**: `ktest_inv.rs:213` dựng
   `InvocationStats { … }` **liệt kê đủ trường** ⇒ thêm 4 trường ở KT-417 làm vỡ build
   `--features suite-user` (`E0063: missing fields bound, mutated, rotated and 1 other`).
   **Đã vá**: thêm 4 delta (`mutated/rotated/bound/unbound` — pha KT-67a không gọi method nào của
   KT-417 nên kỳ vọng **0**, không nới); build lại `--features suite-user` **xanh** (12 s).
2. **`selftest qemu guard (KT-78)` — `T-INFRA`, không phải hồi quy**: máy có **QEMU mồ côi
   PID 45520** (`PPID=1`, `etime 12:44:13`, ảnh `target-task/slot-1/…` của **phiên trước**) ⇒
   baseline `qemu_total=1` và ca 6 `old-timeout` đếm 2 QEMU "mới" (`37462 40032`) rồi `not ok`.
   Đã `kill 45520` (đúng **một** PID theo §9, không `pkill`) + xác nhận `pgrep` = 0 trước lượt 2.

### Lượt 2 (sau vá) — còn **1** bước đỏ, cũng là lỗi thật

`# gate-steps registered=81 ran=81 skipped=0 failed=1` (TỔNG 692,7 s):

- **`selftest runner ktest (KT-54)`**: `ktest-user` `0/1` — `not ok 23 - capalloc ca am:
  ca am tra sai ma`. Nguyên nhân: TAP `ktest_capa.rs` vẫn gọi `Mutate`(3)/`Rotate`(6) và đòi
  `NotSupported` 18 (đúng trước KT-417). **Đã vá**: ca âm chuyển sang **luật mới** — `Mutate`
  quyền ngoài `RIGHTS_ALL` ⇒ `InvalidArgument` 5; `Rotate` hai ô rỗng ⇒ `SlotEmpty` 8; vẫn
  **có đếm** `denied` (handler `mutate/rotate` nay `bump(DENIED)` khi lỗi, đồng bộ `Suspend`);
  dòng bằng chứng `# capa: am … mutate=1 rotate=1 other=1 denied=1` **giữ nguyên hình dạng**
  (regex `ktest.sh:697`), TAP 23 giữ nguyên số/nhãn.
- `selftest qemu guard` **PASS** sau khi dọn QEMU mồ côi ⇒ xác nhận đỏ lượt 1 là `T-INFRA`
  đúng phân loại (D2-07), không phải hồi quy.

### Lượt 3 (sau vá) — **PASS 81/81**

```text
# gate-steps registered=81 ran=81 skipped=0 failed=0
# step-time-total   688324 ms  TỔNG gate --fast (T2)
v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
# KTEST: 9/9 boot sạch (smp=1, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
```

`ktest-user` TAP 23 xanh với luật `Mutate`/`Rotate` mới. Bằng chứng T2 gắn commit `c57895df`
(cây sạch — hai bước `pre-measure`/`api-boundary` fail-closed trên cây bẩn nên chỉ xanh sau commit;
D2-07: hai lượt đỏ trước là **lỗi thật**, không phải flake; lượt `qemu guard` đỏ là `T-INFRA` đã dọn).

## 6. Còn lại / bàn giao

- `KT-417` **DONE** — T2 (đầy đủ) đã bao trùm; nhánh push `main`: code `c57895df`, sổ sách `ca437fa8`,
  `--bkl` (việc Lead của `KT-408c`) `885a9173`.
- **Reviewer độc lập** (04:15 UTC) trên `c57895df`: kết luận **CÓ** đủ nền cho `KT-417b` (spec↔gen↔docs
  byte-khớp; 27 giá trị `[boot_caps]` khớp `proc_abi.rs`; 4 method đúng đặc tả; 133 test host xanh).
  Phát hiện **F1 (chặn `KT-419`)** — xem §7, **đã vá**; F2/F3/F5/F6/F7/F9 là nợ **máy kiểm** ⇒ đề xuất
  trong `BLOCKED.md` §1b (Q4/Q5/Q6).
- `KT-417b` (hợp nhất 1 cây Capability/MDB, gỡ `plat-common::obj`, `PageTable.{Map,Unmap}`) — planner
  đo **6,5–7,5 h** ⇒ đề xuất tách `KT-417b-1`/`417b-2` + quyết định pin (Q1/Q2/Q3 trong `BLOCKED.md`);
  `KT-417c` (`x86_64 syscall/sysret`) sau `KT-418c`.

## 7. F1 — reviewer KT-417: `bound_notification` treo khi Notification chết (đã vá)

**Phát hiện**: `finalise_notification_body` chỉ đánh thức waiter, **không** xoá
`TcbData.bound_notification`; sau `CNode.Revoke` trên Untyped chủ, TCB còn trỏ `ObjId` đã chết ⇒
`check_notifications` đỏ; `ObjId` tái dùng ⇒ bind **nhầm** Notification khác / `Busy 14` oan. Trước
KT-417 đường này **không tới được** từ U-mode; sau khi cài `Tcb.BindNotification` thì tới được.

**Đỏ-trước**: `invoke417_tests::notification_chet_thi_go_lien_ket_tcb` (bind n1 → `finalise_notification_body(n1)`):

```text
thread 'invoke417_tests::notification_chet_thi_go_lien_ket_tcb' panicked at …:624:5:
assertion `left == right` failed: F1: TCB còn trỏ Notification đã chết ⇒ bind nhầm sau khi ObjId tái dùng
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 133 filtered out
```

**Vá**: `core_finalise::finalise_notification_body` quét mọi TCB còn sống và gỡ liên kết
`bound_notification == ntfn` (cùng cửa với đánh thức waiter ⇒ `Revoke`/`Delete` đều đi qua).
**Xanh**: `cargo test -p vifast-core` **134 passed**, clippy sạch.

**Bàn giao**: `KT-419` phải có **TAP trên ảnh thật** cho đường Notification-chết (host test gọi thẳng
hook finalise; `Revoke` cần cap Untyped đã bị `retype` tiêu ở cảnh test này).
