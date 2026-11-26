# KT-417d — PageTable.{Map,Unmap} thật + xoá `[limits.plat]`/shim `obj` + đóng `Q4` F6 (phần 1/2)

**Trạng thái task: PARTIAL** — 3/5 mục của hàng task `QUEUE.md:479` xong và có bằng chứng máy
sinh; mục (1) bảng VM vào `KernelStore`, (2) xoá **bảng thứ hai** + `core_foreign.rs` +
`ArchVmOps`, (5) `CNode.Revoke` 1 `Untyped` thu hồi 4 loại object **chưa làm** (≈ 8–11 h, vượt
ngân sách 3 h của task) — Lead đã ghi nhận và sẽ trình Agent 0 tách `KT-417e`.

Đợt **W8a · Slot 1** · nhánh `task/kt417d-vm-store` · worktree `/private/tmp/vifast-wt/1` ·
nền `b76e3bc8` · commit `2fc15e0e` · 2026-11-27 (giờ máy đo: `Asia/Tokyo`).

## 1. Bối cảnh

Hàng task `KT-417d` (`D2-262`): **hợp nhất 1 cây Cap/MDB bước 2** — đưa bảng VM vào `KernelStore`,
xoá bảng thứ hai + `core_foreign.rs`, cài `PageTable.{Map,Unmap}`, xoá `[limits.plat]`, đóng `Q4`
F6 (13 chữ ký thiếu `errors`), chứng minh `CNode.Revoke` 4 loại object. Slot 1 là **đường găng**
mở khoá Chặng 3; chủ sở hữu duy nhất cửa sổ `spec/{abi,kernel}.toml` của Đợt W8a (`KT-420` chạy
song song, 0 chạm 2 tệp này).

Đo tại `b76e3bc8` (kế thừa `KT-417b`): `vifast_core::vm::Machine<PhysMem>` vẫn giữ **bảng thứ hai**
(`untypeds[4]`/`objects[96]`/`caps[160]`/`mappings[64]`/`flushes[16]`), ảnh kernel giữ
`static mut` `MACHINE` ở `vifast-plat-riscv64/src/obj.rs`, core giữ object **đại diện**
(`core_foreign.rs`) với `key` cầu nối một chiều; `plat-common::obj` chỉ còn shim re-export.

**Phạm vi cắt được trong 3 h**: (3a) `PageTable.{Map,Unmap}` thật, (3b) gỡ `unsupported` cho hai
method + máy kiểm, (3c) xoá `[limits.plat]` + sửa `check_kernel.py`, (4) đóng F6; cộng phần
**shim** của mục (2) (xoá shim, đổi 30 tệp caller sang `vifast_core::vm`) — phần **không** đòi
dịch 2.400 LOC sang view mượn `KernelStore` và **không** đụng hợp đồng `SyscallEnv::invoke`
(điểm sẽ chết khoá nếu làm ẩu — xem §6).

## 2. Thiết kế / quyết định

1. **`PageTable.Map/Unmap` = cài bảng do caller sở hữu rồi map qua chính bảng đó** (đọc từ
   chữ ký `params = ["page_table", "vspace", "va", "frame", "flags"]` của `D2-261 nhóm 4`):
   `Map` cài bảng 4 KiB `pt` làm **bảng mức lá** cho `va` trong `vspace` (cấp **bảng mức 1** nếu
   thiếu, từ Untyped của VSpace), rồi map `frame` bằng chính bảng đó (§ `vm/vspace.rs`).
   `Unmap` gỡ lá tại `va` **qua bảng `pt`**; bảng phải đứng đúng mức lá (chưa cài ⇒
   `InvalidCapability` 1; khác bảng/mức ⇒ `MissingPageTable` 12). Đây là đường để pager userland
   **sở hữu bảng** ở Chặng 3, khác `Frame.Map` (VSpace tự cấp bảng).
2. **Atomic**: `install_leaf_table` trả `LeafInstall{Already,Linked,Created{..}}` để rollback
   **đúng phần vừa đổi** khi `map` hỏng (hết slot mapping/flush). Bảng đã cài sẵn **không** bị
   tháo oan. `teardown_vspace` thu hồi luôn **cap** của bảng đã cài (tránh cap trỏ object chết —
   bất biến `check_objmem`).
3. **Hook arch chở 3 khoá**: thêm `InvokeOp.aux2_obj`/`aux2_key` cho cap `Frame` ở `MR2`;
   `invoke.rs` phân giải cap theo `MR` (`aux_cap` thay `aux_vspace`), `mem_bridge.rs` (riscv64)
   thi hành trên object layer. Core **không** ghi PTE (giữ D2-83).
4. **Mã lỗi nội bộ mới**: `ObjError::{MissingPageTable, Busy}` (`abi_code` 12/14) — trước đó
   object layer không có hai mã này dù `[error]` đã có.
5. **`spec/` (đúng cửa sổ được cấp)**: `[object].PageTable` bỏ `unsupported`/`install_at` +
   ghi chú mới; 13 `[method_signature]` thêm `errors=[...]` (tập con của 19 mã);
   `spec/kernel.toml` **xoá hẳn `[limits.plat]`**; `check_kernel.py` bỏ `limits.plat` khỏi
   `required` và thêm **ratchet `E_LIMITS_PLAT`** (cấm section quay lại); `check_abi.py` thêm
   máy kiểm F6 (pin 13 tên, `errors` khác rỗng/tập con/không trùng) + đổi chiều canh
   `PageTable.{Map,Unmap}` (cấm `unsupported` trở lại, cấm còn `install_at`). Không nới ngưỡng.
6. **Tách `mod tests` khỏi `invoke.rs`** sang `invoke_tests.rs` (`#[path]`, nội dung không đổi)
   để giữ **R1 ≤ 1.500 dòng/tệp** sau khi thêm `pt_map/pt_unmap`.

## 3. Đỏ-trước (log máy sinh, đã lưu)

| Ca | Log | Kết quả log |
|---|---|---|
| F6: máy kiểm mới bắt **13** chữ ký thiếu `errors` | `kt417d-vm-store/red-f6-selftest.log` | exit **1**, 13× `not ok - F6: …errors phải là danh sách khác rỗng` |
| `PageTable.{Map,Unmap}` gỡ `unsupported` trước khi sửa danh sách canh | `kt417d-vm-store/red-pagetable-unsupported.log` | exit **1**, 4× `not ok` (`chưa cài ⇒ phải unsupported = true`, `install_at`) |
| `[limits.plat]` xoá khỏi spec khi checker còn đòi | `kt417d-vm-store/red-limits-plat.log` | exit **1**, `not ok - E_SECTION: thiếu mục [limits.plat]` |
| `PageTable.{Map,Unmap}` API chưa có | `kt417d-vm-store/red-pagetable-api.log` | exit **101**, 7× `error[E0599]: no method named map_page_table/unmap_page_table` |

Sau mỗi bước: máy kiểm/spec xanh lại (`checkers.log`, `t0.log`).

## 4. Thay đổi theo tệp (nhóm)

- **`spec/`**: `abi.toml` (PageTable object + 13 `errors` + ghi chú), `kernel.toml` (xoá
  `[limits.plat]`).
- **Máy kiểm**: `v2/test/abi/check_abi.py` (F6 + PageTable đã cài + 4 ca âm selftest),
  `v2/test/kernel/check_kernel.py` (`required`, `E_LIMITS_PLAT`, ca âm selftest).
- **Object layer (`vifast-core`)**: `vm/vspace.rs` (`map_page_table`, `unmap_page_table`,
  `leaf_table_pa`, `install_leaf_table`, `rollback_leaf_table`, `LeafInstall`),
  `vm/mod.rs` (`ObjError` 2 biến thể + `abi_code`/`code_name` + teardown thu hồi cap),
  `invoke.rs` (`aux_cap`, `pt_map`, `pt_unmap`, `aux2_*`, danh sách trắng, tách `mod tests`),
  `lib.rs` (`mod ptmap_tests`, ghi chú), `ptmap_tests.rs` (mới), `invoke_tests.rs` (tách ra),
  `tlb.rs`/`machine.rs`/`vm/arena.rs` (ghi chú đường dẫn mới).
- **Shim**: xoá `v2/kernel/vifast-plat-common/src/obj/mod.rs` + bỏ `pub mod obj;`; **30 tệp**
  caller đổi `vifast_plat_common::obj` → `vifast_core::vm` (8 `plat-riscv64`, 17 `kbin-riscv64`,
  2 `vifast-ktest`, `core_foreign`, `tlb`, test host, `vifast-alloc`, `vifast-pte`, 1 test sim).
- **Env riscv64**: `mem_bridge.rs` nhánh `PageTable.{Map,Unmap}` (kiểm 3 khoá + `drain_flushes`).
- **Test**: `vifast-plat-common/tests/obj.rs` (5 ca mới: cài→map→unmap qua bảng, ca âm sai
  loại/căn/W^X, **atomic rollback** khi hết slot, bảng từ Untyped khác, map lần hai + `protect`);
  `vifast-core/src/ptmap_tests.rs` (8 ca dispatch).

## 5. Bằng chứng máy sinh (trích nguyên văn log)

**T0** (`kt417d-vm-store/t0.log`, máy sinh):
```
v2 t0: PASS (3 crate; **T0** — KHÔNG phải cổng; cổng = gate --dev (T1) / gate --fast (T2))
```
+ cross-target (host macOS không biên dịch được asm riscv): `cargo check --target
riscv64gc-unknown-none-elf -p vifast-plat-riscv64 -p vifast-kbin-riscv64` và
`--target aarch64-unknown-none`/`x86_64-unknown-none` cho 2 kbin ⇒ **0 lỗi** (cùng log).

**Test**: `vifast-core` **147 passed** (trước 139; +8 dispatch); `plat-common tests/obj`
**20 passed** (trước 15; +5).

**Pin cỡ** (`footprint`, nguyên văn):
```
# kt417b cap: Cap=32 caps[512]=16384 Machine=172424 (truoc: 56 / 28672 / 184712)
```
⇒ `size_of::<Cap>() == 32`, `size_of::<Machine<StaticStore,TestObserver>>() = 172424 B` —
**không đổi so với `KT-417b`** vì bảng VM **chưa** dời vào `KernelStore` (mục 1 chưa làm);
trần `184.832 B` không đụng.

**Máy kiểm** (`checkers.log`):
```
abi-check: PASS (spec/abi.toml)
# selftest: 23/23 ca đúng như kỳ vọng (dương: spec thật + gen đối chứng; còn lại là âm)
kernel-check: PASS (49 giá trị đối chiếu theo tên hằng, …)
# selftest: 11/11 ca đúng như kỳ vọng (1 dương + 10 âm)
reserved-check: PASS (5 object reserved, spec/abi.toml)
gen-check: PASS (code + docs khớp spec/abi.toml)
```
`grep -rn "plat_common::obj" --include=*.rs v2/` ⇒ **0** (chỉ còn 5 nhắc doc lịch sử ghi
"đã xoá"). `v2 check --rules` ⇒ `rules: PASS` (R1 giữ: `invoke.rs` 1.373 dòng).

**T1** — xem §9.

## 6. Rủi ro / nợ còn lại + đề xuất task kế

1. **Chưa làm (1) bảng VM vào `KernelStore`**, **(2) xoá bảng thứ hai + `core_foreign.rs` +
   `ArchVmOps`**, **(5) `CNode.Revoke` 4 loại**. Ước lượng đo được **8–11 h**, không gói trong
   ngân sách 3 h: `vm/` 2.400 LOC phải thành **view** mượn `&mut S::Vm`; `SyscallEnv::invoke`
   **phải** đổi hợp đồng (được gọi *trong* `core_bridge::with_machine` ⇒ lấy khoá lồng =
   deadlock; phải truyền `&mut Machine.vm`); 36 tệp caller; `Retype` VM chuyển vào core.
   Thiết kế B1–B5 + 5 bẫy đã ghi ở `JOURNAL/kt417d-handoff-con-ho-lai.md` (đã commit cùng
   `2fc15e0e`). **Đề xuất**: Agent 0 tách `KT-417e` cho B1–B4.
2. **Nợ bằng chứng QEMU cho `PageTable.{Map,Unmap}`**: chưa có TAP riêng — thêm TAP sẽ đổi
   `TAP_TESTS` của `kbin`/`run-*`/`soak-*` (khoá parity vừa đóng ở `KT-408e`) nên để task sau
   (khi pager U-mode có đường gọi thật). Hiện chứng minh bằng **test host** (PTE thật + atomic +
   `check_objmem`) + 3 ca âm dispatch + smoke T1 9 ảnh.
3. **`[limits.plat]` xoá nhưng module `vm` vẫn là "bảng thứ hai"** — ratchet `E_LIMITS_PLAT`
   chặn spec quay lại; chừng nào `KT-417e` chưa xong thì `vm::Machine` vẫn giữ bảng riêng (nợ
   kiến trúc đã ghi trong handoff).
4. **Bẫy đã gặp** (ghi trong handoff §3): R1 dễ vỡ khi thêm hàm vào `invoke.rs`; `ObjError`
   thiếu mã 12/14; teardown phải kill cap bảng đã cài; `plat-riscv64` không T0 được trên host
   macOS (dùng cross-check); `api boundary` chỉ xanh trên cây sạch.
5. **Không nới ngưỡng/pin/ca**: không `#[ignore]`, không bỏ case, không retry lấy xanh.

## 7. Bảng tệp đã đổi (54 tệp ở `2fc15e0e`)

| Nhóm | Tệp | Δ |
|---|---|---|
| Spec | `spec/abi.toml`, `spec/kernel.toml` | +26/−9 |
| Máy kiểm | `v2/test/abi/check_abi.py`, `v2/test/kernel/check_kernel.py` | +121/−2 |
| `vifast-core` | `invoke.rs`, `invoke_tests.rs` (mới), `ptmap_tests.rs` (mới), `lib.rs`, `machine.rs`, `tlb.rs`, `vm/{mod,arena,vspace}.rs` | +~820/−~190 |
| Shim | `vifast-plat-common/src/{lib.rs, obj/mod.rs}`, `tests/obj.rs` | −1 tệp, +~200 |
| Caller riscv64 | 8 tệp `vifast-plat-riscv64/src`, 17 tệp `vifast-kbin-riscv64/src`, 2 tệp `vifast-ktest/src` | mỗi tệp 1–4 dòng (đổi đường import) |
| Lib/sim | `lib/vifast-alloc`, `lib/vifast-pte`, `sim/…/kt58_core_parity/main.rs` | ghi chú (1 dòng/tệp) |
| Log + handoff | `JOURNAL/kt417d-vm-store/{red-*.log,t0.log,checkers.log}`, `JOURNAL/kt417d-handoff-con-ho-lai.md` | mới |

## 8. Commit

- Mã + test + log đỏ: **`2fc15e0e`** — `[KT-417d] PageTable.{Map,Unmap} thật + xoá
  [limits.plat]/shim obj + đóng Q4 F6 (phần 1/2)` (Lead commit; sub-agent chỉ stage).
- Journal + log T1: commit kế tiếp (Lead commit hộ), đề xuất
  `[KT-417d] journal + bằng chứng T1 xanh (phần 1/2, 2fc15e0e)`.

## 9. T1 — `./v2/bin/v2 gate --dev` (cây sạch @ `2fc15e0e`)

- Lệnh: `cd /private/tmp/vifast-wt/1 && ./v2/bin/v2 gate --dev`
- Log: `v2/agent/JOURNAL/kt417d-vm-store/t1-clean.log` (3.406 dòng, 228.199 B)
- Kết quả (nguyên văn):
```
v2 check: PASS (L1 chạy trong gate --fast)
rules: PASS (/private/tmp/vifast-wt/1/v2)
abi-check: PASS (/private/tmp/vifast-wt/1/spec/abi.toml)
kernel-check: PASS (49 giá trị đối chiếu theo tên hằng, /private/tmp/vifast-wt/1/spec/kernel.toml)
--- api boundary (KT-241): OK
--- L0 test: OK
--- L0 kernel workspace (KT-148): OK
--- L1: OK (906 ms, đo bởi cổng)          # L1 OK: 1000 seed, 999 trace khác nhau
--- expected[ktest]: plan=74 ok=74 ten-TAP=74
--- QEMU smoke (T1: 7 ảnh × 1 boot): OK    # 9 dòng `# ktest* … PASS` (74/74, 4/4, 38/38, 10/10, 6/6, 5/5, 6/6, 11/11, 3/3)
# image=ktest … data+bss=55192 budget=262144 … corespace=180744
# gate-steps registered=82 ran=68 skipped=14 failed=0
# step-time-total   675356 ms  TỔNG gate --dev (T1)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
```
- **Không** phải full gate (T1 bỏ 14 bước selftest harness — in rõ `BỎ QUA (T1 …)`); **T2**
  (`gate --fast`) và nightly/soak **không** chạy (việc của Lead ở giờ Cao điểm, `D2-256`).
- Không retry lấy xanh: T1 chạy **một lượt** trên cây sạch, PASS ngay.
- 3 ảnh test riscv64 `data+bss ≤ 262.144 B`, `.corespace=180.744 B ≥ Machine 172.424 B` (pin
  `KT-401` không đụng).

## 10. Bàn giao cho Lead

- **PARTIAL có kiểm chứng**: 3/5 mục hàng task xong + shim xoá; T1 PASS trên `2fc15e0e`.
- Việc tiếp: (a) commit journal + log T1 (đề xuất message ở §8); (b) trình Agent 0 tách
  `KT-417e` theo `kt417d-handoff-con-ho-lai.md` §2 (B1–B5, 8–11 h) + bẫy §3; (c) khi `KT-417e`
  xong mới đóng được `D2-262 Q2(c)` phần "bảng VM trong `KernelStore`" + `[limits.plat]` đã xoá
  từ đây; (d) T2/L1-medium/Miri do Lead gom ở giờ Cao điểm.
- Không có tệp nào ngoài phạm vi: `STATE/QUEUE/DECISIONS/AGENTS`, `v2/bin/**`, `.github/**`
  **không** chạm.
