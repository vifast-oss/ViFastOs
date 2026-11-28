# KT-417d phần còn lại — CHỐT 4 TASK CON + KHỐI DÁN `QUEUE.md` (trinh sát, chỉ đọc)

Ngày: 2026-11-28 21:0x JST · người trinh sát: `prep-tech-b` (sub-agent READ-ONLY của Lead) · **0 sửa repo, 0 build/test/QEMU/cổng, 0 git ghi** · máy đang chạy `mutants --all`.
Nguồn: `/Users/mps/dvchd/ViFastOs` **`main @82502f7d`** (đo trực tiếp), `/tmp/briefs/{kt417e-split,kt417e-dev}.md`, `v2/agent/QUEUE.md:479`, `git show 2fc15e0e --stat`, handoff `v2/agent/JOURNAL/kt417d-handoff-con-ho-lai.md`.
Nhãn: số đếm/dòng = `[V1:ĐO]` (đếm trên cây); số byte suy ra từ `size_of` cấu trúc = `[ƯL]`; số giờ = `[ƯL]`.

---

## 0. Xác nhận bản đề xuất — 8/9 điểm đúng, **1 điểm sai (chặn `f`)**

| # | Điều đề xuất nói | Đo trên `main @82502f7d` | Kết luận |
|---|---|---|---|
| 1 | 155 điểm `self.<bảng>` → `self.t.*` (mod 86 / vspace 37 / check 32) | **đúng 155** (86 / 37 / 32 / arena 0; `objects` 52, `untypeds` 39, `mappings` 15, `caps` 14, `mem` 8, `n_untypeds` 7, `n_flushes` 7, `next_asid` 5, `next_id` 4, `flushes` 4) | ✅ |
| 2 | `aobj::machine()` 105 điểm / 23 tệp | **105 điểm / 22 tệp** (`plat-common/tests/obj.rs` đã hết) | ✅ số điểm, ✏️ 22 tệp |
| 3 | `impl SyscallEnv` 16 impl / 15 tệp | **18 impl / 16 tệp** (+`ktest_capa.rs::CapaEnv`, +`configure_tests.rs::HookEnv`) | ✏️ **cập nhật DoD của `g`** |
| 4 | 10 vị trí pin cỡ `Machine` ở 4 tệp runner | **đúng 10**: `run-aarch64.sh:2447,2448`; `run-x86_64.sh:1990,1991`; `ktest.sh:608,621,654,839` (`parts=26` **chỉ** `:608`); `lib/budget-common.sh:54,303` | ✅ |
| 5 | `soak-*.sh` không pin cỡ | **0 hit `172424`** trong `v2/test/boot/soak*` | ✅ (f **không** chạm soak) |
| 6 | `[limits.plat]` đã xoá, ratchet `E_LIMITS_PLAT`, 0 chạm `spec/` | `check_kernel.py:296-302` ratchet có thật; `check_kernel.py` chỉ kiểm hằng **được khai** trong `source` (không quét `MAX_*` lạ) ⇒ hằng bảng VM đặt ở `store.rs` **không** cần spec | ✅ **0 chạm `spec/` cho cả 4** |
| 7 | `vm/mod.rs` 1.285 dòng ⇒ phải tách trước | **1.316 dòng** (đã +31 ở `2fc15e0e`) ⇒ +250 = **1.566 > 1.500**, `v2/bin/lib/util.sh:145` chặn R1 | ✅ nhấn mạnh: tách `vm/objects.rs` **trước** |
| 8 | `KT-417c` còn là chủ `run-x86_64.sh` ⇒ `f` phải chờ | **`KT-417c` ĐÃ MERGE vào `main`**: `075b0639` (feature) + `82502f7d` (hotfix X1/X2/X3), `spec/kernel.toml:123 sysret_ready = true`, `plat-x86_64/src/gdt.rs` đã `planned`. QUEUE:480 còn ghi `BLOCKED` = **lệch sổ** | ✏️ **`f`/`g` hết xung đột với `KT-417c`** |
| 9 | `size_of::<Machine>()` 172.424, headroom 12.408 B | 172.424 ✅ nhưng còn **2 pin chặt hơn mà đề xuất bỏ sót**: `footprint.rs:334-338` `total ≤ 184_832 − 12_288 = **172.544**` (slack **120 B**) và `kt408_tests.rs:105` `assert_eq!(…, **172_424**)` | ❌ **`f` bất khả thi nếu không tái neo 2 pin này** |

### 0.1 Vì sao `f` **không thể** né (bằng số)

`f` đưa bảng VM **vào** `Machine<S,O>` (D2-262 Q2 cấm global `static`). Byte tối thiểu phải thêm, kể cả **không** có metadata object:

| Thành phần | Byte | Cơ sở |
|---|---:|---|
| `VmUntypeds<4>` (`Untyped` = 8 + 96 + 128 + 12 → canh 8) | 992 | `vm/arena.rs:19-36` + `vifast-alloc/src/lib.rs:102-111` ⇒ `Untyped` **248 B** (không phải ~240), `FrameArena` **128 B** (không phải 127) |
| `VmMappings<64>` (`Mapping` 16 B) | 1.024 | `vm/mod.rs:560-568` |
| `VmFlushes<16>` (`Flush` 16 B) | 256 | `vm/mod.rs:626-631` |
| bộ đếm `n_untypeds/n_flushes/next_id/next_asid` | 12 | `vm/mod.rs:643-655` |
| **tối thiểu** | **2.284** | ⇒ 174.708 B **> 172.544** |

Ba phương án metadata VM (đều **vượt** 172.544, đều **dưới** trần 184.832):

| Phương án | Thêm | Tổng | Slack vs 184.832 | Đánh đổi |
|---|---:|---:|---:|---|
| (a) bảng rời `VmObjMeta[96]` (`Object` 36 B) | 3.456 | **178.164** | 6.668 | giữ `Meta == 48` (3 pin), **nhưng** chỉ 96/256 object được là VSpace/PageTable/Frame |
| (a') bảng rời `VmObjMeta[256]` | 9.216 | **183.924** | **908** | đủ 256 object, nhưng sát trần (<1 KiB) |
| (b) nhét trường VM vào `Meta` 48 → 72 B | 6.144 | **180.852** | 3.980 | đủ 256 object + thoáng, **nhưng** phải sửa **3 pin `Meta == 48`** (`footprint.rs:270`, `kt408_tests.rs:97`, `store.rs:682`) |

⇒ `f` **buộc** phải: `footprint.rs:335` → `total ≤ 184_832` (giữ chứng minh thu hồi 12.288 B ở `assert_eq!(size_of::<[Cap; 512]>(), 32 × 512 = 16.384 B)` đã có ở `footprint.rs:329`), và `kt408_tests.rs:105` → số **mới đo** (đúng tiền lệ `KT-417b` đã đổi pin 184.712 → 172.424). Đây **không** phải nới trần: trần `184.832` (KT-312) và `196.608` (D2-84) giữ nguyên.

---

## 1. Bốn task con — tệp chính, chồng tệp, phụ thuộc, `spec/`

Thứ tự **bắt buộc tuần tự** `e → f → g → h` (một lane). Chồng tệp chỉ tồn tại **giữa hai task liền kề có bàn giao khai báo**; **không** cặp nào được chạy song song.

### KT-417e — B2 · "Động cơ VM trên bảng rời" · **3 h** `[ƯL]` · phụ thuộc `KT-417d` (đã ở `main`) · **0 chạm `spec/`**
Tệp chính: `vifast-core/src/vm/{mod,vspace,check,arena}.rs` (sửa) + `vm/{tables,arch,view,objects}.rs` (**mới**); `plat-riscv64/src/arch_vm.rs` (**mới**) + **1 dòng** `mod arch_vm;` trong `plat-riscv64/src/lib.rs`; `plat-common/tests/vm_engine.rs` (**mới**, 6–8 ca).
Không chạm: `store.rs`, `machine.rs`, `footprint.rs`, `kt408_tests.rs`, `invoke.rs`, `syscall.rs`, `core_bridge.rs`, `plat-riscv64/src/obj.rs`, `plat-common/tests/obj.rs`, `spec/`, mọi runner/`ktest.sh`.
Đo được: 155 điểm dịch; `size_of::<Machine>()` **vẫn 172_424**; mọi `vm/*` < 1.500 dòng; 20 ca `tests/obj.rs` + 8 ca `ptmap_tests.rs` còn xanh.

### KT-417f — B1 · "Bảng VM vào `KernelStore` + tái neo 2 pin byte + 10 vị trí pin runner" · **2 h** `[ƯL]` (đề xuất cũ 1.5 h — nay +0.5 vì 2 pin byte) · phụ thuộc `KT-417e` · **0 chạm `spec/`**
Tệp chính: `vifast-core/src/{store,machine,footprint}.rs` + **`vifast-core/src/kt408_tests.rs`** (pin cỡ chính xác, đề xuất cũ bỏ sót); `v2/test/boot/{ktest.sh,run-aarch64.sh,run-x86_64.sh,lib/budget-common.sh}` (**10 vị trí**, `budget-common.sh:54` sửa **trước**).
Không chạm: `vm/**` (chỉ đọc `VmTables` của `e`), `invoke.rs`, `syscall.rs`, caller, `spec/`, `soak-*`.
Đo được: `# core: link … parts=30 sum=<mới> sum_ok=1`; 10 vị trí pin khớp; `check_kernel.py` xanh (`E_LIMITS_PLAT` không kích hoạt); `Machine<UntypedStore>` vẫn > `Machine<StaticStore>`.

### KT-417g — B3 · "Hợp đồng `SyscallEnv::invoke` + 105 điểm caller" · **3 h** `[ƯL]` · phụ thuộc `KT-417f` · **0 chạm `spec/`**
Tệp chính: `vifast-core/src/{syscall,invoke}.rs`; `plat-{riscv64,aarch64,x86_64}/src/core_bridge.rs`; `plat-aarch64/src/syscall_env.rs` + `plat-x86_64/src/syscall_env.rs` + `plat-riscv64/src/user/mem.rs` (**18 impl / 16 tệp**, đo lại); **22 tệp / 105 điểm** `aobj::machine()` (`mem_bridge` 21, `ktest_ring` 15, `ktest_capa` 10, …).
Không chạm: `vm/**`, `store.rs`/`machine.rs`/`footprint.rs`/`kt408_tests.rs`, `core_foreign.rs`, `spec/`, runner.
Đo được: `grep -rn "aobj::machine()" v2/` = **0**; 18 impl cập nhật; ca host "invoke trong `with_machine` không lấy lại khoá"; QEMU smoke không treo.

### KT-417h — B4 + mục (5) · "Xoá bảng thứ hai + `core_foreign.rs` + `Retype` VM ở core + `Revoke` 4 loại" · **3 h** `[ƯL]` · phụ thuộc `KT-417g` · **0 chạm `spec/`**
Tệp chính: `vifast-core/src/vm/**` (xoá `Machine<M>`/`mem`/bảng), `core_foreign.rs` (**xoá tệp** 85 dòng), `objmeta.rs`, `invoke.rs`, `invoke_tests.rs`, `lib.rs`; `plat-riscv64/src/obj.rs` (xoá `static mut MACHINE:94-101`); `kbin-riscv64/src/ktest_inv.rs` (TAP + ca âm).
Không chạm: `store.rs`/`machine.rs`/`footprint.rs` (chốt ở `f`), `syscall.rs` + caller (chốt ở `g`), `spec/`, runner (TAP đếm **chỉ** nếu chốt được số với `KT-419`).
Đo được: `grep "core_foreign|foreign_key|add_foreign_object"` = 0; `static mut MACHINE` = 0; ca `revoke_untyped_thu_hoi_4_loai`; `mut-state-before-cap` vẫn KILLED.

### 1.1 Bảng chồng tệp (tệp chính)

| | `e` | `f` | `g` | `h` |
|---|:--:|:--:|:--:|:--:|
| `e` — `vm/*` (sửa+tạo), `plat-riscv64/{arch_vm.rs,lib.rs}`, `plt-common/tests/vm_engine.rs` | — | 0 | 0 | ⚠ `vm/**` (bàn giao `h` xoá bảng thứ hai) |
| `f` — `store/machine/footprint/kt408_tests`, 4 tệp runner | 0 | — | 0 | 0 |
| `g` — `syscall/invoke`, `core_bridge` 3 arch, 3 `syscall_env`/`mem.rs`, 22 tệp caller | 0 | 0 | — | ⚠ `invoke.rs` + `lib.rs` (bàn giao) |
| `h` — `vm/**`, `core_foreign`, `objmeta`, `invoke`, `ktest_inv`, `plat-riscv64/obj.rs` | ⚠ | 0 | ⚠ | — |

⇒ **0 % chồng giữa mọi cặp có thể song song**; hai cặp `e↔h` / `g↔h` là **bàn giao tuần tự khai báo** (1 chủ/lượt), không phải song song. So với 5 task W8: `e` = 0 % với **toàn bộ**; `f` = 0 % nay (KT-417c đã merge); `g`/`h` chỉ còn va `KT-419`/`KT-419b`/`KT-420b` ở `invoke.rs` ⇒ **không** chạy đồng thời.

### 1.2 Tổng ngân sách
3 + 2 + 3 + 3 = **11 h** `[ƯL]` dev (+T2 Lead). Dự phòng `KT-417h2` 1,5 h (chỉ bằng chứng `Revoke` + TAP) nếu `h` chạm 2 h 45 ⇒ worst-case 12,5 h.

---

## 2. KHỐI DÁN ĐƯỢC cho `v2/agent/QUEUE.md`

> Dán **5 dòng** dưới đây vào bảng chính, **thay thế** dòng 479 (`KT-417d`) và **thêm sau nó** 4 dòng mới (giữ đúng 6 cột hiện có). Dán **sau** khi Agent 0 ký `APPROVED`; nếu chưa ký thì đổi ô trạng thái `READY` của `KT-417e` thành `BLOCKED`.

```markdown
| KT-417d | **P0** **Chặng 2 (W8a) — Hợp nhất 1 Cây Cap/MDB bước 2, PHẦN 1/2: `PageTable.{Map,Unmap}` thật + xoá `[limits.plat]`/shim `plat-common::obj` + đóng `Q4` F6** | KT-417b | 3 h | DONE | **(DONE 2026-11-27 `2fc15e0e`, đã ở `main` — PARTIAL 1/2: 3/5 mục xong; phần còn lại KHÔNG còn thuộc `KT-417d`, đã tách `KT-417e/f/g/h`)** `PageTable.{Map,Unmap}` cài thật (`vm/vspace.rs::map_page_table/unmap_page_table` + `LeafInstall` rollback + dispatch `invoke.rs` + `mem_bridge`); gỡ `unsupported`/`install_at` + `check_abi.py` đảo chiều; xoá hẳn `[limits.plat]` (54→49 giá trị) + ratchet mới `E_LIMITS_PLAT`; đóng `Q4` F6 (`errors=[...]` cho 13 `[method_signature]` + 4 ca âm); xoá shim `plat-common::obj`, 30 caller dùng thẳng `vifast_core::vm`. **Còn lại → `e` (bảng rời `VmTables`/`VmView`/`ArchVmOps`), `f` (bảng VM vào `KernelStore` + pin cỡ), `g` (hợp đồng `SyscallEnv` + 105 caller), `h` (xoá bảng thứ hai + `core_foreign.rs` + `Revoke` 4 loại)** — 0 mục nào chạm `spec/`. |
| KT-417e | **P0** **Chặng 2 (W8a') — KT-417d phần 2a: Động cơ VM trên **bảng rời** (`VmTables`/`VmView`/`ArchVmOps`) — **chưa** vào `KernelStore`** | KT-417d | 3 h | READY | **(Lead tách 2026-11-28, chờ `APPROVED`; 0 % chồng tệp toàn bộ W8a–W8d; 0 chạm `spec/`)** (1) tách `vm/mod.rs` **1.316 dòng** → **mới** `vm/objects.rs` **trước** khi thêm dòng (R1 1.500, `v2/bin/lib/util.sh:145`); (2) `VmTables<MAP,FLUSH,UNT>` giữ **cả** bảng lẫn bộ đếm (`n_untypeds`/`n_flushes`/`next_id`/`next_asid`) + `VmUntypeds` (`FrameArena`) + `VmObjMeta` **bảng rời** (giữ `Meta == 48 B`); (3) trait `ArchVmOps` (`pte_load/pte_store/flush/physmap`) + 2 impl: host `Ram` và **tệp mới** `plat-riscv64/src/arch_vm.rs` cho `PhysmapMem` (1 dòng `mod`; **không** sửa `obj.rs`); (4) dịch **155 điểm** `self.<bảng>` → `self.t.*` (mod 86 / vspace 37 / check 32) + `self.mem` → `self.arch`; (5) **mới** `plat-common/tests/vm_engine.rs` 6–8 ca (map→walk→unmap qua bảng rời; `flush` qua `ArchVmOps`; ca âm `pte_store=false` ⇒ `ObjError::Corrupt`; const-assert `≤ 4096`). **0 chạm**: `store.rs`, `machine.rs`, `footprint.rs`, `kt408_tests.rs`, `invoke.rs`, `syscall.rs`, `core_bridge.rs`, `obj.rs`, `plat-common/tests/obj.rs`, **`spec/`**, mọi runner/`ktest.sh`. Xong: `size_of::<Machine>()` **vẫn 172.424**; mọi `vm/*` < 1.500 dòng; 20 ca `tests/obj.rs` + 8 ca `ptmap_tests.rs` còn xanh; T0 `cargo check --target riscv64gc-unknown-none-elf` (plat-riscv64 **không** build host macOS). |
| KT-417f | **P0** **Chặng 2 (W8a') — KT-417d phần 2b: Bảng VM thành **kiểu liên kết hạng nhất** của `KernelStore` + tái neo 2 pin byte + 10 vị trí pin runner (`parts` 26→30)** | KT-417e | 2 h | BLOCKED | **(Lead tách 2026-11-28, chờ `APPROVED`; 0 chạm `spec/`)** (1) `store.rs`: `type Vm` + `VM_EMPTY` cho **cả** `StaticStore` (`VmTables<64,16,4>`) và `UntypedStore` (`VmTables<256,32,16>`), giữ hằng `MAX_*` bảng VM ở `store.rs` (**không** `machine.rs` — `[limits.core]` trỏ `machine.rs#MAX_*`); (2) `machine.rs`: field `vm: S::Vm` + const-assert `Copy`/cỡ; (3) `footprint.rs`: `FOOTPRINT_PARTS` 26→30 + 4 tên/4 bytes, canh `padding ≤ 8 B`; (4) **tái neo 2 pin byte** (bắt buộc, xem §0.1): `footprint.rs:335` `total ≤ 184832−12288` → `total ≤ 184832` (**giữ** chứng minh thu hồi 12.288 B ở `assert_eq!(size_of::<[Cap; 512]>(), 32 × 512 = 16.384 B)`), và `kt408_tests.rs:105` `assert_eq!(…, 172_424)` → cỡ **mới đo**; trần `184.832` (KT-312) / `196.608` (D2-84) **không nới**; (5) **10 vị trí pin** (4 tệp, `[V1:ĐO]`): `lib/budget-common.sh:54,303` (SSOT — sửa **trước**), `ktest.sh:608,621,654,839` (`parts=26`→30 **chỉ** `:608`), `run-aarch64.sh:2447,2448`, `run-x86_64.sh:1990,1991`. **0 chạm**: `vm/**`, `invoke.rs`, `syscall.rs`, caller, `spec/`, `soak-*` (đo: 0 hit `172424`). Xong: `# core: link … parts=30 sum_ok=1` trên QEMU; `Machine<UntypedStore>` vẫn > `Machine<StaticStore>`; `check_kernel.py` xanh. |
| KT-417g | **P0** **Chặng 2 (W8c) — KT-417d phần 2c: Hợp đồng `SyscallEnv::invoke` + **105 điểm / 22 tệp** caller về **một** cây cap của core (chống deadlock `with_machine`)** | KT-417f | 3 h | BLOCKED | **(Lead tách 2026-11-28, chờ `APPROVED`; 0 chạm `spec/`; KHÔNG chạy đồng thời `KT-419`/`419b`/`420b` — cùng `invoke.rs`, 1 chủ/lượt)** (1) `syscall.rs`: trait generic theo `S: KernelStore`, hook nhận `&mut S::Vm` (**không** lấy lại khoá); (2) `invoke.rs`: 8 chỗ `env: &mut impl SyscallEnv` (`:398,544,702,1066,1189,1223,1259,1298` `[V1:ĐO]`); (3) `core_bridge.rs` 3 arch (`machine_mut`/`vm`); (4) cập nhật **18 impl / 16 tệp** (`[V1:ĐO]`, gồm `plat-aarch64/src/syscall_env.rs:128`, `plat-x86_64/src/syscall_env.rs:323`, `plat-riscv64/src/user/mem.rs:21`, `ktest_{capa,exec,user}.rs`, 6 tệp test `vifast-core`, `syscall.rs::HeaderEnv`, `sim/tests/kt62_syscall.rs`, `plat-riscv64-logic/tests/user.rs`); (5) **105 điểm / 22 tệp** `aobj::machine()` → `with_machine(\|m\| …)` hoặc `env` (`mem_bridge` 21, `ktest_ring` 15, `ktest_capa` 10, `ktest_vfs` 7, `ktest_inv` 7, `ktest_capx` 6, `ktest_mem` 6, `ktest_vfs_rig` 6, …; 5 chỗ khai **kiểu** `Machine<PhysmapMem>`/`ObjMac` phải đổi sang view). **Đỏ-trước**: ca host "invoke **trong** `with_machine` **không** lấy lại khoá" (bản cũ ⇒ đỏ có tên). Xong: `grep "aobj::machine()"` = **0**; T0 3 crate xanh; QEMU smoke **0 treo** (`with_machine` không re-entrant — `mem_bridge.rs:123+127`, `313+327` là 2 cặp gọi liên tiếp phải vá). |
| KT-417h | **P0** **Chặng 2 (W8d) — KT-417d phần 2d: Xoá **bảng thứ hai** + `core_foreign.rs` + `Retype` VM ở core (qua `ArchVmOps`) + chứng minh `Revoke` 1 `Untyped` thu hồi **4 loại** object** | KT-417g | 3 h | BLOCKED | **(Lead tách 2026-11-28, chờ `APPROVED`; 0 chạm `spec/`; dự phòng `KT-417h2` 1,5 h nếu chạm 2 h 45)** (1) `Retype` VM chuyển từ plat sang **core** (`retype_page_table/vspace/frame`), lấy PA/zero bảng **qua `ArchVmOps`** để giữ `forbid(unsafe_code)`; (2) xoá `vm::Machine<M>`/`mem`/bảng + **`git rm core_foreign.rs`** (85 dòng) + `static mut MACHINE` `plat-riscv64/src/obj.rs:94-101` + 20 tệp còn tham chiếu `core_foreign`/`foreign_key`; (3) `CNode.Revoke` 4 loại: thêm `Tcb` vào vòng cuối `revoke_untyped` (68 dòng) + ca host `revoke_untyped_thu_hoi_4_loai` (1 `Untyped` → `TCB`+`VSpace`+`Frame`+`PageTable` → `Revoke` ⇒ `objects/caps/mappings/flushes`=0, `total_used()==0`, `check_caps`/`check_bytes`/`check_objmem` xanh) + ca âm bỏ `kill_cap` ⇒ `InvalidCapability` + **1 TAP** `ok N - revoke: 4 loai` (**chốt số TAP với `KT-419` trước**, `TAP_TESTS=50`); (4) `mut-state-before-cap` vẫn **KILLED**. **0 chạm**: `store.rs`/`machine.rs`/`footprint.rs`/`kt408_tests.rs` (chốt ở `f`), `syscall.rs`+caller (chốt ở `g`), `spec/`, runner (trừ số TAP đã chốt). Xong: `grep "core_foreign\|add_foreign_object\|foreign_key"` = **0**; `static mut MACHINE` = **0**; `cargo test -p vifast-core` xanh; T1 xanh; T2 do Lead. |
```

---

## 3. Câu hỏi **duy nhất** cho Agent 0 (mở khoá `f`, từ đó mở `g`/`h`)

> **Ngân sách byte của `Machine` sau khi hợp nhất: cho phép tái neo 2 pin byte và chọn chỗ đặt metadata VM theo phương án nào?**
> Cụ thể — (i) **tái neo** `footprint.rs:335` `total ≤ 184832−12288 (=172.544)` → `total ≤ 184832`, giữ nguyên chứng minh thu hồi 12.288 B ở `assert_eq!(size_of::<[Cap; 512]>(), 32 × 512 = 16.384 B)`, và `kt408_tests.rs:105` `assert_eq!(…, 172_424)` → cỡ mới đo (**trần 184.832 / 196.608 không nới**); (ii) chọn **(a)** `VmObjMeta[96]` bảng rời — giữ `Meta == 48`, tổng **178.164 B**, **nhưng** chỉ 96/256 object được là VSpace/PageTable/Frame; **(a')** `VmObjMeta[256]` — đủ 256 object, tổng **183.924 B**, slack **908 B**; **(b)** mở `Meta` 48 → 72 B — đủ 256 object, tổng **180.852 B**, nhưng phải sửa **3 pin `Meta == 48`** (`footprint.rs:270`, `kt408_tests.rs:97`, `store.rs:682`).
> **Không trả lời được ⇒ `f` bất khả thi**: bảng VM tối thiểu +2.284 B ⇒ 174.708 B > 172.544, mà giữ bảng ngoài `Machine` bằng `static` là vi phạm `D2-262 Q2`.

*(Ba việc còn lại **không** cần hỏi vì đã có câu trả lời trong sổ: cấp mã `KT-417e/f/g/h` hợp regex `^KT-[0-9]+[a-z]?$`; **0 chạm `spec/`** cho cả 4 — đã kiểm `check_kernel.py` chỉ đọc hằng khai trong `source` + `E_LIMITS_PLAT` chỉ cấm `[limits.plat]` quay lại, và `Untyped.Retype`/`CNode.Revoke` đã `frozen` trong `spec/abi.toml:228-229`; thứ tự `g`/`h` ↔ `KT-419`/`419b`/`420b` tuân `D2-262` Q2 + luật 1 chủ/`invoke.rs`.)*
