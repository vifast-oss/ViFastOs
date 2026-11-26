# KT-417d — bàn giao kỹ thuật phần CÒN LẠI (không phải journal cuối)

Ngày: 2026-11-27 (đợt W8a Slot 1) · nhánh `task/kt417d-vm-store` · nền `b76e3bc8`.
Tệp này ghi **thiết kế đã chốt + danh sách việc còn lại** cho lượt kế tiếp của `KT-417d`;
journal 10 mục cuối cùng chỉ được viết **sau** T1 (theo brief §7), nên tệp này **không**
phải journal và **không** chứa số đo nghiệm thu.

## 1. Đã xong trong lượt này (đã stage, T0 xanh)

| Mục hàng task | Trạng thái |
|---|---|
| (3a) cài `PageTable.{Map,Unmap}` thật | **XONG** — `vm/vspace.rs::{map_page_table,unmap_page_table}`, `invoke.rs` dispatch + `InvokeOp.aux2_*` cho cap Frame ở MR2, env riscv `mem_bridge`, 6 test host ở `vifast-plat-common/tests/obj.rs` (cài→map, unmap qua bảng, ca âm, **atomic rollback** khi hết slot, bảng từ Untyped khác, protect), 8 test dispatch ở `vifast-core/src/ptmap_tests.rs`, 1 ca âm cờ/quyền/sai loại |
| (3b) gỡ `unsupported` `PageTable.{Map,Unmap}` + `check_abi.py` | **XONG** — bỏ `unsupported`/`install_at` trong `[object].PageTable`; checker đổi chiều: cấm quay lại `unsupported`, cấm còn `install_at`; + ca âm selftest |
| (3c) xoá `[limits.plat]` khỏi `spec/kernel.toml` + `check_kernel.py:277`/`--selftest` | **XONG** — section xoá hẳn; `required` bỏ `limits.plat`; **ratchet mới** `E_LIMITS_PLAT` cấm thêm lại; + ca âm selftest; 54 → 49 giá trị |
| (4) đóng `Q4` F6 | **XONG** — `errors = [...]` cho đúng 13 `[method_signature]`; `check_abi.py` kiểm `errors` là **tập con khác rỗng** của 19 mã, không trùng, có pin 13 tên; 3 ca âm selftest (thiếu / mã lạ / rỗng) |
| (2) xoá shim `plat-common::obj` | **XONG (phần shim)** — shim xoá, **30 tệp** caller đổi sang `vifast_core::vm`; `grep -r "plat_common::obj" v2/` = **0** |
| (1) bảng VM vào `KernelStore` | **CHƯA** |
| (2) xoá **bảng thứ hai** + `core_foreign.rs` + `ArchVmOps` | **CHƯA** |
| (5) `CNode.Revoke` 1 `Untyped` thu hồi 4 loại object | **CHƯA** (phụ thuộc (1)/(2)) |

## 2. Vì sao (1)/(2)/(5) chưa làm — thiết kế đã chốt cho lượt kế

### 2.1 Hiện trạng đo được

- `vifast_core::vm::Machine<M: PhysMem>` (1.268 + 450 + 445 + 270 dòng) vẫn giữ **bảng thứ hai**:
  `untypeds[4]`, `objects[96]`, `caps[160]`, `mappings[64]`, `flushes[16]`, `next_id/asid`, `mem`.
- Ảnh kernel giữ **static** `MaybeUninit<Machine<PhysmapMem>>` trong `vifast-plat-riscv64/src/obj.rs`
  (`obj::machine()` → `&'static mut`) — đây là **global static** mà `D2-262 Q2` cấm cho trạng
  thái VM chính thức.
- Core giữ **object đại diện** (`core_foreign.rs`) + `key = (object_id << 16) | cap_index` làm cầu
  nối một chiều (D2-83) ⇒ hai cây cap song song.

### 2.2 Hướng đã chốt (không đổi so với `D2-262`)

1. **Bảng VM → `KernelStore`**: thêm vào `store.rs` các kiểu liên kết
   `VmMappings`/`VmFlushes`/`VmUntypeds`(`FrameArena`)/`VmObjMeta` là **associated type** của
   `KernelStore`, đa hình bằng **const generic trên kiểu bảng** (không `generic_const_exprs`):
   `StaticStore::Vm = VmTables<64, 16, 4>` và `UntypedStore::Vm = VmTables<256, 32, 16>` (đúng
   `D2-262 Q2(c)`), đặt **trong** `Machine<S, O>` (field `vm: S::Vm`) ⇒ `Machine` vẫn `Copy`,
   **0 static mới**; `PhysMem` **không** vào core: giữ trait `PhysMem` + impl `PhysmapMem` ở plat
   và truyền `&impl PhysMem` vào các hàm VM (hoặc `ArchVmOps` là tham số hàm, **không** tham số
   kiểu của `Machine` — ràng buộc `Machine<StaticStore, TestObserver>` của pin `footprint.rs`).
2. **`ArchVmOps`**: trait trong core gồm `pte_load/pte_store/flush/physmap` (arch cài; host test
   cài `Ram`); `vm::Machine` biến thành **view** `VmView<'a>` mượn `&'a mut S::Vm` + mem, các hàm
   `map/unmap/protect/walk/revoke/...` giữ nguyên thân (2.400 LOC) — chỉ đổi `self.<bảng>` →
   `self.t.<bảng>`.
3. **Caller**: `aobj::machine()` (36 tệp, ~200 điểm) đổi thành truy cập **cùng** `Machine` của
   kernel. **Bẫy đã xác định**: `SyscallEnv::invoke` được gọi **trong** `core_bridge::with_machine`
   ⇒ không thể lấy lại khoá (non-reentrant). Cách đúng: đổi hợp đồng hook thành
   `fn invoke(&mut self, op: InvokeOp, vm: &mut S::Vm) -> ...` (core truyền `&mut self.vm` —
   tách borrow được), các caller khác dùng `core_bridge::with_machine(|m| m.vm_mut())`.
4. **Xoá bảng thứ hai + `core_foreign.rs`**: `Meta` của core nhận thêm trường VM
   (`pa/off/size_bits/level/owner/asid`, dùng chung `ObjId`), `Retype` VM do **core** cấp (gọi
   `ArchVmOps` để lấy PA/zero bảng), `foreign_key`/`add_foreign_object` xoá; `mem_bridge`,
   `user_run`, `ktest_*`, `x86_64 shellpath/coretest` bỏ phần bridge.
5. **`CNode.Revoke` 4 loại**: khi 4 loại object cùng thuộc **một** `Untyped` trong **một** cây,
   `revoke_untyped` đã có sẵn thứ tự teardown (`VSpace` → `PageTable`/`Frame`);
   thêm `Tcb` vào vòng cuối (hiện `Tcb` do core sở hữu nhưng **không** có arena byte ⇒ chỉ cần
   `kill_cap` + `free_meta`) và test host `revoke_untyped_thu_hoi_4_loai` + TAP trong `ktest_inv`.

### 2.3 Ước lượng & thứ tự an toàn cho lượt kế

| Bước | Nội dung | Ước lượng | Rủi ro |
|---|---|---:|---|
| B1 | `store.rs`: kiểu bảng VM + `Machine.vm` + pin `footprint.rs` (chưa đổi caller) | 1,5 h | thấp |
| B2 | `VmView` + `ArchVmOps` (host `Ram` + plat `PhysmapMem`) — dịch 2.400 LOC cơ học | 2,5–3 h | trung bình |
| B3 | Đổi hợp đồng `SyscallEnv::invoke` + 36 tệp caller (`mem_bridge`, `user_run`, `ktest_*`) | 2–2,5 h | **cao** (deadlock nếu lấy khoá lồng) |
| B4 | `Retype` VM ở core + xoá `core_foreign.rs`/bảng thứ hai + revoke 4 loại | 2,5–3 h | cao |
| B5 | T1 + T2 + L1-medium | — | — |

⇒ **Không thể gói trong 3 h**; đề xuất tách `KT-417d` thành `KT-417d` (phần đã xong + spec) và
`KT-417e` (B1–B4) khi Agent 0 mở khoá — hoặc giữ mã `KT-417d` và cho Lead quyết định cắt nhỏ.

## 3. Bẫy đã gặp (ghi để lượt sau không lặp)

1. **R1 (≤1.500 dòng/tệp)**: `invoke.rs` vượt 1.500 sau khi thêm `pt_map/pt_unmap` ⇒ đã tách
   `mod tests` sang `v2/kernel/vifast-core/src/invoke_tests.rs` (`#[path]`, nội dung không đổi).
   Thêm bất kỳ hàm nào vào `invoke.rs` phải kiểm `wc -l` **trước** khi stage.
2. `ObjError` của object layer **không** có `MissingPageTable`/`Busy` ⇒ đã thêm hai biến thể
   (`abi_code` 12/14) ở `vm/mod.rs`; nếu ai đổi bảng mã ABI phải sửa cả `code_name`.
3. Bảng lá do caller sở hữu khi **cài** vào VSpace phải bị `teardown_vspace` **thu hồi cap**
   (`kill_cap`) — nếu không `check_objmem` báo `InvalidCapability` (cap trỏ object chết).
4. `vifast-plat-riscv64` **không** biên dịch được trên host macOS (`#[link_section=".corespace"]`
   + asm RISC-V) ⇒ T0 cho crate này là `cargo check --target riscv64gc-unknown-none-elf`
   (đã làm, xanh), **không** phải `v2 t0 -p vifast-plat-riscv64` (đỏ sẵn từ trước, không do task).
5. `api boundary (KT-241)` **luôn** fail trên cây bẩn (fail-closed) ⇒ chỉ xanh ở T1 sau commit.
