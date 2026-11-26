# KT-417d (phần 2/2 — hotfix) — đúng 3 lỗi MAJOR của review W8a: F1/F2/F3

**Trạng thái: READY-FOR-COMMIT** (sub-agent `fix-kt417d` · ngân sách 2 h · không commit/push —
Lead commit hộ). Cùng task `KT-417d` (task **chưa DONE**), **không** phải task mới: chỉ vá 3 lỗi
MAJOR `F1`, `F2`, `F3` trong `/tmp/briefs/review-w8a.md`; F4/F5 **không** chạm (xem §6).

Đợt **W8a** · nhánh `task/kt417d-fix` · worktree `/private/tmp/vifast-wt/4` · nền/HEAD `afb2556b`
(`main`) · 2026-11-27 (giờ máy đo `Asia/Tokyo`, bắt đầu `02:27`, kết thúc `03:5x`).
`CARGO_TARGET_DIR=<repo>/target-task/slot-4` (build ấm của slot 4).

## 1. Phạm vi (đúng 3 lỗi, không mở rộng)

| Lỗi | Nội dung | Tệp sửa |
|---|---|---|
| **F2** | `PageTable.Map` không chặn `Frame ≠ 4 KiB` ⇒ cài bảng + cấp bảng mức 1 rồi `Corrupt` (rò 5), `Unmap` cùng bộ cap không đối xứng | `v2/kernel/vifast-core/src/vm/vspace.rs` (`map_page_table`) |
| **F1** | `NoSlots`/`Corrupt` rò ra ABI dưới mã **5** qua `mem.rs:152-157` (`.abi_code().unwrap_or(5)`) | `vifast-core/src/vm/mod.rs` (hàm thuần), `vifast-plat-riscv64/src/{mem_bridge.rs,user/mem.rs}` |
| **F3** | Q4-F6 chưa đóng: `errors` thiếu mã có thật, máy kiểm chỉ kiểm "tập con khác rỗng + pin 13" | `spec/abi.toml`, `v2/test/abi/check_abi.py` |

**Cứng đã tuân**: `spec/` chỉ chạm `spec/abi.toml` (0 chạm `spec/kernel.toml`); 0 chạm
`v2/bin/**`, `core_foreign.rs`/`teardown_vspace`, các tệp `KT-417c`, STATE/QUEUE/DECISIONS/AGENTS;
không nới ngưỡng, không `#[ignore]`, không bỏ ca; `Machine: Copy` + `forbid(unsafe_code)` +
0 `static` mới; `size_of::<Machine>() == 172424` (đo lại §5).

## 2. Đỏ-trước (log máy sinh trong `JOURNAL/kt417d-fix/`)

| # | Ca | Log | Kết quả **trước** khi vá |
|---|---|---|---|
| 1 | F2: `map_page_table` với Frame 2 MiB | `red-f2-frame-2mib.log` | exit **101**, `left: Corrupt / right: NotSupported` — đúng lỗi review |
| 2 | F1: mã ABI của `NoSlots`/`Corrupt` qua biểu thức `mem.rs` | `red-f1-noi-bo-ra-ma-5.log` | exit **101**, `left: 5 / right: 9` (NoSlots ⇒ 5) |
| 3 | F3: spec **thiếu** `MissingPageTable` trong `PageTable.Unmap.errors` | `red-f3-thieu-missingpagetable.log` | exit **0** = **xanh** (lỗ hổng: máy kiểm cũ bỏ qua) |

Ba log chứng minh lỗi **thật** trước bản vá; sau vá xanh lại (§5). Không retry lấy xanh.

## 3. Thay đổi

### F2 — `vm/vspace.rs::map_page_table`
- Thêm tiền điều kiện **cỡ Frame** ngay sau `frame_cap_full` (trước `install_leaf_table`, tức
  **trước mọi thay đổi**): `fr.size_bits != PAGE_SIZE_BITS[0]` ⇒ `ObjError::NotSupported`.
- Cập nhật doc hàm: method chỉ mở đường **lá 4 KiB** do caller sở hữu; lá lớn vẫn đi `Frame.Map`.
- `unmap_page_table` **không cần sửa**: vì Map từ chối trước khi cài, `pt.owner` vẫn `VS_NONE` nên
  `Unmap` cùng bộ cap trả `InvalidCapability` (1) **ngay**, không để lại bảng/PTE/byte nửa vời —
  ca host dưới đây khẳng định bằng `used`/`counts().page_tables`/`walk` bất biến.
- Ca host mới `page_table_map_tu_choi_frame_khong_phai_4kib` (`vifast-plat-common/tests/obj.rs`):
  Frame 2 MiB ⇒ `NotSupported` (18), `untyped_stats(u0)` + `counts().page_tables` + `walk` **không
  đổi**, `Unmap` cùng bộ cap ⇒ `InvalidCapability`, `check_objmem` xanh.

### F1 — hàm thuần trong `vifast-core` + dùng ở cầu
- `ObjError::to_abi_code_for_invoke() -> u16` (`vifast-core/src/vm/mod.rs`): **toàn phần**,
  `NoSlots ⇒ NotEnoughMemory` **9**, lỗi nội bộ còn lại (`Corrupt` + mọi biến thể `None` thêm sau
  này) ⇒ `NotSupported` **18**; mọi biến thể có mã giữ nguyên `abi_code()` ⇒ **không có nhánh dự
  phòng nào trả 5**, nên F1 không thể tái diễn (mã **5 chỉ** đến từ `InvalidArgument`). Lý do chọn
  mã + căn cứ `spec/abi.toml [error]` ghi ngay doc-comment.
- `MemError::to_abi_code_for_invoke()` (`plat-riscv64/src/mem_bridge.rs`): ủy quyền `Obj` →
  hàm trên, `Core` → `KError::id()`.
- `user/mem.rs::invoke` thay hẳn `.abi_code().map(..).unwrap_or(InvalidArgument.id())` bằng
  `error.to_abi_code_for_invoke()` ⇒ **không còn `unwrap_or(5)`** trên đường trả mã.
- Test host: `ptmap_tests::loi_noi_bo_qua_cau_invoke_khong_phai_ma_5` (2 mã) +
  `ptmap_tests::ma_qua_cau_toan_tap_chi_invalidargument_ra_5` (đủ **16** biến thể `ObjError`: có
  mã ABI thì giữ nguyên; nội bộ không rò 5; **chỉ** `InvalidArgument` cho 5 — bất biến "không còn
  `unwrap_or(5)`" ở mức kiểu).

### F3 — spec + máy kiểm
- `spec/abi.toml` (`[method_signature]` 2 chữ ký VM) — tập `errors` chốt **suy từ đường mã thật**:

| Chữ ký | Tập `errors` chốt | Thêm so với trước |
|---|---|---|
| `PageTable.Map` | InvalidCapability, WrongObjectType, InsufficientRights, WxViolation, InvalidArgument, Alignment, NotEnoughMemory, AlreadyMapped, Busy, **NotSupported** | `NotSupported` 18 (F2 + F1 `Corrupt`) |
| `PageTable.Unmap` | InvalidCapability, WrongObjectType, InsufficientRights, Alignment, **MissingPageTable**, NotMapped, **NotEnoughMemory**, **NotSupported** | `MissingPageTable` 12 + 2 mã của F1 (`unmap` trả `NoSlots`/`Corrupt`) |

  Căn cứ từng mã: `invoke.rs::{pt_map,pt_unmap,aux_cap,map_flags}` (1/2/3/5) → `mem_bridge`
  (1/18 nhánh method lạ) → `vspace.rs::{map_page_table,install_leaf_table,map}` (4/5/6/9/10/14/18,
  F2 ⇒ 18) và `{unmap_page_table,leaf_table_pa,missing_tables,unmap}` (1/2/6/9/11/12/18). Ghi chú
  `note` của 2 chữ ký nêu tiền điều kiện 4 KiB + ánh xạ F1.
- `v2/test/abi/check_abi.py`: **thêm pin chính xác tập** cho 2 chữ ký (`F3:` thiếu/thừa ⇒ ĐỎ),
  giữ nguyên pin 13 + các bất biến F6 cũ; **3 ca âm selftest mới** (thiếu `MissingPageTable`,
  thiếu `NotSupported`, thừa `Busy`) ⇒ selftest **23 → 26** ca; cập nhật **neo** của ca âm F6
  "mã lạ" (chuỗi `errors` của Map đã dài thêm, chỉ đổi neo — không đổi kỳ vọng).
- `vifast-gen --check` + `docs/abi.md`: **không cần** sinh lại (`gen.py` không phát `errors` vào
  `generated.rs`/`docs/abi.md`) — kiểm bằng máy: `gen-check: PASS` (§5).

## 4. Vì sao không đưa thêm mã vào `errors` (chống "thừa mã")

`ObjError::RangeError` (19) **chỉ** phát từ `Machine::cap/object` khi chỉ số **ngoài bảng**; qua
cầu, mọi `CapId` do `mem_bridge` giải mã đều được kiểm `cap_kind/cap_key` trước (`mem_bridge.rs`
PageTable branch) và sai ⇒ `InvalidCapability` 1. Vì vậy 19 **không lọt** ra U-mode trên 2 chữ ký
này ⇒ **không** khai (khai thừa cũng là lỗi theo bất biến F3 mới). `HasChildren`/`WxViolation`…
cũng không có nhánh trả trên `Unmap` ⇒ không khai.

## 5. Bằng chứng máy sinh (T0 — **không** phải cổng)

- `cargo test -p vifast-plat-common`: **obj.rs 21 passed** (trước 20; +1 ca F2), các bộ khác xanh
  (`t0.log`).
- `cargo test -p vifast-core`: **149 passed** (trước 147; +2 ca F1) — gồm pin cỡ
  `# kt417b cap: Cap=32 caps[512]=16384 Machine=172424` (**không đổi**), `MachineNoop=159160`.
- `cargo fmt --all --check`: **PASS**; `clippy -p vifast-core --all-targets -D warnings` +
  `clippy -p vifast-plat-common --all-targets -D warnings`: **0 cảnh báo**.
- Cross-target: `cargo check --target riscv64gc-unknown-none-elf -p vifast-plat-riscv64 -p
  vifast-kbin-riscv64` **0 lỗi**; `cargo clippy --target riscv64gc-unknown-none-elf -p
  vifast-plat-riscv64 --lib -D warnings` **0 cảnh báo**; `--target aarch64-unknown-none -p
  vifast-kbin-aarch64` + `--target x86_64-unknown-none -p vifast-kbin-x86_64` **0 lỗi** (core dùng
  chung 3 arch).
- Máy kiểm (`checkers.log`): `abi-check: PASS`; `--selftest: 26/26`; `gen-check: PASS`;
  `kernel-check: PASS (49)`; `rules: PASS (v2)` + `test/rules/run.sh 6/6`.
- **Không** chạy T1/T2/nightly/soak (theo brief: T1 chỉ sau khi Lead commit, cây sạch; T2/nightly
  là việc Lead ở giờ Cao điểm). QEMU: 0 tiến trình do lượt này tạo.

## 6. Rủi ro / nợ — phần **không** thuộc lượt này

1. **F4 (MAJOR, review W8a)** — `teardown_vspace` giết cap bảng lá của caller trong khi cây cap
   core vẫn giữ cap cùng `key`: **không** sửa ở đây (quyết định thiết kế, Lead/Agent 0). Đây là
   chỗ chặn B4 và nguy cơ double-free khi xoá `core_foreign.rs` ⇒ phải chốt **trước** `KT-417e`.
2. **F5 (MAJOR)** — 22 host test `namesrv`/`page` không cổng nào chạy: việc của Lead (`v2/bin/**`).
3. **M1/M2/M3/M4** của review **không** đụng (ngoài 3 lỗi được giao); M2 (aliasing bảng) và M1
   (rò bảng trung gian khi unmap) vẫn là nợ `KT-417e`; M3 (`[limits.plat]` mất máy kiểm) nên đóng
   trước B1.
4. `PageTable.{Map,Unmap}` vẫn **chưa có TAP QEMU** (nợ đã ghi ở journal phần 1/2) — bằng chứng
   lượt này là **test host** + cross-target; thêm TAP sẽ đổi `TAP_TESTS` parity (`KT-408e`).
5. F1 ở tầng **cầu** chỉ kiểm được bằng hàm thuần host + cross-target compile; nhánh QEMU thật
   (`mem.rs`) sẽ được phủ khi pager U-mode có đường gọi (Chặng 3) — đã nêu trong doc-comment.

## 7. Tệp staged + commit đề xuất (Lead commit hộ, **không** push từ sub-agent)

`spec/abi.toml`, `v2/kernel/vifast-core/src/vm/mod.rs`,
`v2/kernel/vifast-core/src/vm/vspace.rs`, `v2/kernel/vifast-core/src/ptmap_tests.rs`,
`v2/kernel/vifast-plat-common/tests/obj.rs`,
`v2/kernel/vifast-plat-riscv64/src/mem_bridge.rs`,
`v2/kernel/vifast-plat-riscv64/src/user/mem.rs`, `v2/test/abi/check_abi.py`,
`v2/agent/JOURNAL/kt417d-hotfix-f1f2f3.md`, `v2/agent/JOURNAL/kt417d-fix/**`.

Đề xuất: `[KT-417d] hotfix F1/F2/F3: chặn Frame ≠4 KiB bằng NotSupported, ánh xạ NoSlots/Corrupt
qua cầu (9/18, hết unwrap_or(5)), pin đúng tập errors PageTable.{Map,Unmap}`

## 8. T1 — `./v2/bin/v2 gate --dev` (cây **sạch** @ `e65e42a4`, Lead commit hộ)

- Lệnh: `cd /private/tmp/vifast-wt/4 && ./v2/bin/v2 gate --dev` (nhánh `task/kt417d-fix`,
  `CARGO_TARGET_DIR=<repo>/target-task/slot-4`). Một lượt, **không retry**.
- Log: `v2/agent/JOURNAL/kt417d-fix/t1-clean.log` (3.488 dòng) · 17:40:37Z → **17:53:28Z**.
- Kết quả (nguyên văn):
```
# gate-steps registered=82 ran=68 skipped=14 failed=0
# gate-wall 769603 ms (769.6 s)
# gate-cpu user=1086.553 s sys=362.425 s total=1448.978 s
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
T1-EXIT=0
```
- Các bước then chốt: `v2 check: PASS` · `rules: PASS` · `abi-check: PASS` ·
  `reserved-check: PASS` · `gen-check: PASS` · `api boundary (KT-241): OK` · `L0 test: OK` ·
  `L0 kernel workspace (KT-148): OK` · `L1: OK (898 ms)` · `# expected[ktest]: plan=74 ok=74
  ten-TAP=74` · **QEMU smoke (T1: 7 ảnh × 1 boot): OK** với 9 dòng `# ktest* … PASS`
  (74/74, 4/4, 38/38, 10/10, 6/6, 5/5, 6/6, 11/11, 3/3; `# ktest.sh: PASS`) ·
  14 bước `BỎ QUA (T1 — chỉ chạy ở T2)` đúng thiết kế (T1 **không** phải full gate).
- Duy nhất một dòng `not ok - selftest: netsrv ca âm có dòng PANIC 'xong' … (validator rỗng)`
  là **có sẵn** ở log T1 phần 1/2 trên `main` (cùng nội dung, `kt417d-vm-store/t1-clean.log`
  dòng 2616), thuộc `net.sh --red-selftest` (`34 kiểm, 0 lỗi`; bước `selftest validator net
  (KT-245): OK`) ⇒ **không** do bản vá, không phải bước đỏ.
- Không gặp `# net: dns error=Timeout`, không `T-INFRA`, không timeout hạ tầng; chạy song song
  `KT-417c` nhưng wall 769,6 s cùng bậc với phần 1/2 (675,4 s). **Không** chạy T2/nightly/soak,
  **không** push/merge.
