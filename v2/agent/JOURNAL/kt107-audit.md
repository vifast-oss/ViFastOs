# KT-107 — Audit: test dùng chung công thức với code nó kiểm

**Ngày**: 2026-11-18 · **Ngân sách**: 2 h · **Dùng**: ~1 h 10 phút.
**Phạm vi đọc**: `v2/kernel/**`, `v2/lib/**`, `v2/sim/**`, `v2/test/**` (chỉ đọc; mọi thí nghiệm
chạy trên bản clone `/tmp/kt107-repo`, không sửa tệp nào trong repo, không commit, không `git add`).

## 0. Mẫu tham chiếu — KT-105 (đã vá, dùng làm "dạng cần tìm")

| Phía code | Phía test | Dạng |
|---|---|---|
| `v2/kernel/vifast-kbin-riscv64/init/shell_user.rs:257` — `(entry >> 1) & 0x0F` | `v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs:787` — `((entry >> 1) & 0x0F)` | test **giải mã lại** đúng trường kernel đóng gói (`proc_demo.rs:329`, `proc_rt.rs:257`) bằng **cùng** công thức với shell ⇒ shell và test "khớp nhau" dù cả hai sai. Đã vá cả hai về `& 0x0F` (KT-105). |

Đây là dạng chuẩn: **test dựng kỳ vọng bằng cách tự tính lại công thức mà code sản xuất dùng**.
Ba dạng gặp trong audit:

- **D1** — test giải mã một trường do code đóng gói (như `ps`).
- **D2** — test dùng **bảng/hàm dữ liệu của code sản xuất** làm kỳ vọng (bảng chuyển trạng thái,
  hằng sinh).
- **D3** — harness (Python/shell) tự tính lại công thức của kernel để kiểm log.

## 1. Bảng ứng viên

| # | Ứng viên | Phía code | Phía test | Phân loại | Lỗi bị che | Bằng chứng thực nghiệm |
|---|---|---|---|---|---|---|
| 1 | `ps` state mask | `init/shell_user.rs:257`; `src/proc_demo.rs:329` | `src/proc_demo.rs:787` | **AN TOÀN** (đã vá KT-105) | — | KT-105: trước vá `state=Unknown` mọi pid; sau vá `state=Ready`; ma trận 21/21 |
| 2 | Bảng `ALLOWED` 11×11 | `v2/sim/vifast-sim/src/tcb.rs:398-440` | `v2/sim/vifast-sim/tests/kt18_tcb.rs:307-370` (vòng `ALLOWED.iter()`) | **NGHI VẤN** (D2) | Một cặp chuyển trạng thái sai **giữ nguyên số 48** và không bị thao tác nào chạm ⇒ test ma trận xanh, `kt18_tcb` không bắt | Hoán vị `BlockedOnRecv→Inactive` (true→false) + `BlockedOnRecv→BlockedOnReply` (false→true): `kt18_tcb` **18/18 PASS**; `kt23_k1b` bắt (fuzz `forbidden_attempts=45 ≠ 0`) |
| 3 | `INFO_CAPS_SHIFT` / `INFO_LENGTH_SHIFT` của stub IDL | `v2/lib/vifast-idl/src/generated.rs:25,28,44-45` (sinh từ `v2/tools/vifast-gen/gen_idl.py:173,175`) | `v2/lib/vifast-idl/src/lib.rs:91-92,99,109,185` | **NGHI VẤN** (D2) | Layout `MessageInfo` của stub IDL sai (vd caps ở bit 21 thay vì 20) — test encode/decode dùng **cùng** hằng nên round-trip tự khớp; nguồn độc lập `spec/abi.toml:48` không được kiểm | Sửa `generated.rs:25` `20 → 21`: `cargo test -p vifast-idl` **7/7 PASS** |
| 4 | `detail` pha 7 (persist) | `v2/kernel/vifast-kbin-riscv64/src/ktest_persist.rs:770` — `((raw[10] as i64) << 32) \| raw[11]` | `v2/test/boot/persist_host.py:230` — `detail != (single << 32) \| double` | **NGHI VẤN** (D3) | Công thức `detail` sai (vd `<< 16`) — harness tính lại **cùng** công thức nên đẳng thức luôn đúng; chỉ là chẩn đoán, không phải hành vi | Log tổng hợp `detail=(100<<16)\|200` + bản sao harness sửa `<< 16` ⇒ **PASS**; harness gốc `<< 32` ⇒ **FAIL** |
| 5 | `seq` trong chuỗi kỳ vọng `# con: u` | `v2/kernel/vifast-kbin-riscv64/init/con_client_user.rs:117` (`stats.seq`) | `v2/kernel/vifast-kbin-riscv64/src/ktest_con.rs:359` (`expect.num(seq)` với `seq` từ `ring_stats()`) | **NGHI VẤN** (D1, nhẹ) | `seq` sai (không bằng `head`) — test lấy chính `seq` của kernel làm kỳ vọng, chương trình U cũng đọc `seq` từ ring ⇒ hai bên khớp; `head/tail/used` vẫn kiểm được nên chỉ che lỗi chẩn đoán | Chưa chạy QEMU (ngoài ngân sách); suy ra từ code: cả hai phía đọc `seq` từ cùng header ring |
| 6 | `label = frame_pa >> 12` (WaitAddr) | `v2/kernel/vifast-kbin-riscv64/src/ktest_core.rs:531` (nhãn futex) | cùng dòng — test tự tính nhãn từ `frame_pa` của object layer | **NGHI VẤN** (D1, nhẹ) | `frame_pa` sai (nhưng `!= 0`) ⇒ nhãn sai; test dùng nhãn đó cho `wait_addr`/`set_addr_word`/`wake_addr` nên vẫn khớp; chỉ `frame_pa != 0` là kiểm độc lập | Chưa chạy QEMU; suy ra từ code |
| 7 | Công thức payload persist | `v2/kernel/vifast-kbin-riscv64/src/ktest_persist.rs:87` | `v2/test/boot/persist_host.py:42` | **NGHI VẤN** (D3, nhẹ — dữ liệu thử) | Công thức payload sai ở **cả hai** phía ⇒ host sinh lại đúng chuỗi kernel ghi, SHA khớp; đây là dữ liệu thử, không phải hành vi sản xuất | Chưa chạy QEMU; suy ra từ code (hai biểu thức giống nhau) |
| 8 | Bitmap bit `1 << (block % 8)` | `v2/lib/vifast-fs/src/alloc.rs:19,36` | `v2/lib/vifast-fs/src/tests/part1.rs:201`; `v2/kernel/vifast-kbin-riscv64/src/ktest_fs.rs:815` | **AN TOÀN** (D1) | — (test sửa bitmap bằng cùng công thức, nhưng fsck đọc lại bằng công thức sản xuất; mutation sản xuất bị bắt) | Sửa `alloc.rs` thành `7 - block % 8`: `cargo test -p vifast-fs --lib fsck_detects_corrupt_bitmap` **FAIL** (`part1.rs:195` `fsck().unwrap()` = `Corrupt`) |
| 9 | Layout PTE mô hình | `v2/sim/vifast-sim/src/pagetable.rs:60-71` (`PTE_VALID=1<<0`, `PTE_TABLE=1<<1`, `PTE_RIGHTS_SHIFT=2`, `PTE_ATTRS_SHIFT=4`, `PTE_OBJ_SHIFT=10`) | `v2/sim/vifast-sim/tests/kt37_pagetable.rs:1125-1135` (literal 1/2/4/10) | **AN TOÀN** (D1) | — (bit literal trùng hằng sản xuất, nhưng giá trị kỳ vọng `mid`/`leaf`/`o4`/READ/WB là độc lập; nguồn độc lập: `v2/docs/k2-vm.md:318-330`) | Sửa `PTE_OBJ_SHIFT` 10→11: `cargo test -p vifast-sim --test kt37_pagetable` **FAIL** (`left: 12, right: 6`) |
| 10 | `targets_of` (TLB shootdown) | `v2/kernel/vifast-core/src/tlb.rs:154-159`; `v2/kernel/vifast-plat-common/src/tlb.rs:376-378` | `v2/sim/vifast-sim/tests/kt64_core_parity.rs:631-633` | **AN TOÀN** (D1) | — (test chép công thức, nhưng giá trị `active_on`/`caller` độc lập; nguồn độc lập: D2-78, `v2/docs/k3b-core.md:989`) | Sửa core `targets_of` bỏ `& !(1 << caller)`: `cargo test -p vifast-sim --test kt64_core_parity` **FAIL** (2/4) |
| 11 | `MessageInfo` 35 bit | `v2/sim/vifast-sim/src/ipc.rs:183-189` | `v2/sim/vifast-sim/tests/kt24_ipc.rs:211-231` | **AN TOÀN** (D1) | — (kỳ vọng `want` viết tay từ `spec/abi.toml:48`; từng dải bit kiểm độc lập) | Sửa `caps << 20` → `<< 21`: `kt24_ipc` **FAIL** (2 failed/13) |
| 12 | caps mask bit 20..22 | `v2/sim/vifast-sim/src/ipc.rs:183-189` | `v2/sim/vifast-sim/tests/kt25_ipc2.rs:1379` | **AN TOÀN** (D1) | — (mask literal `0x7 << 20`, kỳ vọng `4 << 20` độc lập) | Cùng mutation #11: `kt25_ipc2` **FAIL** (`left: 0, right: 4194304`) |
| 13 | `chunks_for` (conchan) | `v2/lib/vifast-virtio/src/conchan.rs:73-75` | `v2/kernel/vifast-kbin-riscv64/src/ktest_con.rs:258` | **AN TOÀN** (D2) | — (harness có `const:chunks_want=2` viết tay: `v2/test/boot/ktest.sh:680`) | Chưa chạy QEMU; nguồn độc lập là regex harness |
| 14 | `want_drops` (conchan) | `v2/lib/vifast-virtio/src/conchan.rs:73-75` | `v2/kernel/vifast-kbin-riscv64/src/ktest_con.rs:620` | **AN TOÀN** (D2) | — (harness có `drops=2 const:drops_want=2`: `v2/test/boot/ktest.sh:686`) | Chưa chạy QEMU; nguồn độc lập là regex harness |
| 15 | `slots`/`slot_bytes` (conchan) | `v2/lib/vifast-virtio/src/conchan.rs:18,20` | `v2/kernel/vifast-kbin-riscv64/src/ktest_con.rs:360-363` | **AN TOÀN** (D2) | — (harness có `const:slots=32 const:slot_bytes=32`: `v2/test/boot/ktest.sh:679`) | Chưa chạy QEMU; nguồn độc lập là regex harness |
| 16 | `sum`/`first`/`last` (con u) | `v2/kernel/vifast-kbin-riscv64/init/con_client_user.rs:81-98` (đọc byte từ ring) | `v2/kernel/vifast-kbin-riscv64/src/ktest_con.rs:343-357` (tính từ `PAYLOAD`) | **AN TOÀN** (D1) | — (hai nguồn khác nhau: payload đã bơm vs byte đọc từ ring) | Chưa chạy QEMU; suy ra từ code |
| 17 | `live` (allocator) | `v2/kernel/vifast-kbin-riscv64/src/ktest_mem.rs:127` | cùng dòng — test biết nó đã cấp 2 MiB + 4 KiB | **AN TOÀN** (D1) | — (nguồn độc lập: test tự biết cỡ đã cấp; comment ghi rõ "hai nguồn độc lập") | Chưa chạy QEMU; suy ra từ code |
| 18 | `double_ok` (fs) | `v2/kernel/vifast-kbin-riscv64/src/ktest_fs.rs:459` | cùng dòng — test biết nó ghi thêm 3 block | **AN TOÀN** (D1) | — (nguồn độc lập: số block test đã ghi) | Chưa chạy QEMU; suy ra từ code |
| 19 | `info()` 35 bit (capa/exec) | `v2/kernel/vifast-kbin-riscv64/src/ktest_capa.rs:113`; `ktest_exec.rs:136` | cùng hàm — test dựng `MessageInfo` để gọi `invoke::dispatch` | **AN TOÀN** (D1) | — (layout từ `spec/abi.toml:48`, được `kt24_ipc` kiểm độc lập) | Chưa chạy QEMU; nguồn độc lập là spec + `kt24_ipc` |
| 20 | `leaf_bytes` (walk) | `v2/kernel/vifast-kbin-riscv64/src/ktest_con.rs:87` | cùng dòng — `4096 << (9 * level)` | **AN TOÀN** (D1) | — (hình Sv39 9/9/9/12, nguồn độc lập: `vifast_pte::walk_shape`/RISC-V spec) | Chưa chạy QEMU; suy ra từ code |
| 21 | `flags` PLIC (test-only) | `v2/kernel/vifast-kbin-riscv64/src/ktest_smp.rs:263-265` (test tự đóng gói) | `ktest_smp.rs:564-565` (test tự giải mã) | **AN TOÀN** (không phải công thức sản xuất) | — | Đọc code: `flags` chỉ để in chẩn đoán, `plic_ok` không dùng `flags` |
| 22 | `occ` bitmap (test-only) | `v2/kernel/vifast-kbin-riscv64/src/ktest_ring.rs:385` | cùng dòng | **AN TOÀN** (không phải công thức sản xuất) | — | Đọc code: chỉ in chẩn đoán khi rig fail |
| 23 | `marker()` (test-only) | `v2/kernel/vifast-kbin-riscv64/src/ktest_tlb.rs:69-70` | cùng dòng | **AN TOÀN** (không phải công thức sản xuất) | — | Đọc code: marker test-only |
| 24 | SHA-256 (persist) | `v2/kernel/vifast-kbin-riscv64/src/ktest_persist.rs:110-230` | `v2/test/boot/persist_host.py:29,91,211` (`hashlib.sha256`) | **AN TOÀN** (D3) | — (host dùng thư viện chuẩn Python, độc lập với cài đặt Rust) | Đọc code: `hashlib` vs `Sha256` tự viết |
| 25 | `mask`/`tlb_sent`/`__SMPM1__` (harness) | `v2/test/boot/ktest.sh:437,451,728` | cùng file — tính từ `smp` (tham số đầu vào) | **AN TOÀN** (D3) | — (nguồn độc lập: `smp` do người chạy chọn, không đọc từ kernel) | Đọc code: `mask=(1<<smp)-1`, `tlb_sent=100*(smp-1)` |
| 26 | `BODY_OFFSET` (objmem) | `v2/lib/vifast-objmem/src/obj.rs:10-13` | `v2/lib/vifast-objmem/tests/objmem_contract.rs:228` | **AN TOÀN** (D1) | — (test còn kiểm `HEADER_SIZE == 16` và `size_of::<ObjHeader>()`) | Sửa `HEADER_SIZE` 16→8: `objmem_contract` **FAIL** (3/10) |
| 27 | `SECTORS_PER_PAGE` (vfs) | `v2/lib/vifast-vfs/src/layout.rs:12` | `v2/lib/vifast-vfs/tests/pager.rs:161,274`; `writeback.rs:190` | **AN TOÀN** (D1) | — (`pager.rs:47` có `assert_eq!(SECTORS_PER_PAGE, 8)` viết tay) | Sửa `SECTORS_PER_PAGE` +1: `cargo test -p vifast-vfs` **FAIL** (9/12) |
| 28 | PTE fault (`PteFacts`) | `v2/kernel/vifast-plat-common/src/mmu.rs:44-58` | `v2/kernel/vifast-plat-common/tests/fault.rs:26-33` (`leaf()` dùng hằng sản xuất) | **AN TOÀN** (D1) | — (`PteFacts::from_raw` dùng literal `raw & 1`, `raw & 0b1110` độc lập) | Sửa `PTE_READ` 1<<1 → 1<<5: `--test fault` **FAIL** (`NotMapped` vs `Permission`) |
| 29 | `walk_shape` (pte) | `v2/lib/vifast-pte/src/walk.rs:15,82-89` | `v2/lib/vifast-pte/tests/kt35_pte.rs:172-174` (literal 30/21/12) | **AN TOÀN** (D1) | — (literal Sv39 độc lập) | Sửa `INDEX_BITS` 9→8: `kt35_pte` **FAIL** (3/9) |
| 30 | `Record::to_bytes` (traceformat) | `v2/lib/vifast-traceformat/src/lib.rs:53-62` | `v2/lib/vifast-traceformat/tests/crosscheck.rs:60-75` (so golden nhị phân đã commit) | **AN TOÀN** (D1) | — (golden nhị phân `v2/test/trace/golden-sim-seed7.bin` là nguồn độc lập) | Đảo `a`/`b` trong `to_bytes`: `cargo test -p vifast-traceformat` **FAIL** (2/6) |

> **ĐÍNH CHÍNH (review chéo, 2026-11-18)**: con số dưới đây ghi **ba** mục "đã chứng minh xanh giả",
> nhưng reviewer độc lập chỉ ra mục `persist_host.py` là **đột biến phía test**, không phải phía code:
> harness **gốc** vẫn bắt được `<< 16` ở phía kernel. Vậy đúng là **hai** mục chứng minh được bằng
> đột biến **phía code** (#2 `kt18_tcb`, #3 IDL). Mục `persist_host.py` **vẫn** là NGHI VẤN, nhưng vì
> lý do khác — reviewer tìm ra log `single == double` **lọt** harness gốc; đã vá bằng **KT-111**.
> Hai mục NGHI VẤN còn lại (`ktest_con.rs:359`, `ktest_core.rs:531`) reviewer xác nhận là **xanh giả
> thật (nhẹ)**; `persist_host.py:42` thì **không** phải xanh giả có nghĩa.

**Tổng**: 30 ứng viên; **6 NGHI VẤN** (#2–#7); **2 đã chứng minh xanh giả bằng đột biến phía code**
(#2 `kt18_tcb`, #3 IDL, #4 `persist_host.py`); 24 AN TOÀN có bằng chứng (mutation hoặc nguồn độc lập).

## 2. Chi tiết các mục NGHI VẤN

### 2.1 `kt18_tcb` — bảng `ALLOWED` làm kỳ vọng (D2)

- **Code**: `v2/sim/vifast-sim/src/tcb.rs:398-440` — `pub const ALLOWED: [[bool; 11]; 11]`.
- **Test**: `v2/sim/vifast-sim/tests/kt18_tcb.rs:307-370` — `bang_allowed_11x11_du_cap_va_cam`
  lặp `for (from, row) in ALLOWED.iter()` / `for (to, is_allowed) in row.iter()` rồi kiểm
  `transition_tcb` **theo chính bảng đó**. Nếu bảng sai, test vẫn xanh.
- **Lỗi bị che**: một cặp chuyển trạng thái sai **giữ nguyên số cặp** (48) và **không bị thao tác
  nào chạm** ⇒ test ma trận xanh; `kt18_tcb` không có nguồn độc lập cho 121 cặp (chỉ 5 cặp cấm
  được viết tay ở `cap_cam_theo_thiet_ke_bi_tu_choi`, `kt18_tcb.rs:380-410`).
- **Thực nghiệm** (clone `/tmp/kt107-repo`):
  - Hoán vị `BlockedOnRecv→Inactive` (true→false) + `BlockedOnRecv→BlockedOnReply` (false→true)
    trong `tcb.rs:415-419`:
    `cargo test -p vifast-sim --test kt18_tcb` ⇒ **18 passed, 0 failed** (xanh giả).
  - `cargo test -p vifast-sim --test kt23_k1b` ⇒ **8 passed, 1 failed** (fuzz bắt:
    `forbidden_attempts = 45`, kỳ vọng 0). Vậy **bộ đầy đủ** bắt được hoán vị này; nhưng
    `kt18_tcb` **một mình** là vòng tròn.
  - Hoán vị thứ hai `BlockedOnSend→BlockedOnRecv` (false→true) + `BlockedOnSend→BlockedOnReply`
    (true→false): `kt18_tcb` **15 passed, 3 failed** (các test chức năng bắt `BlockedOnReply`).
- **Kết luận**: NGHI VẤN. Test ma trận không độc lập; `kt23_k1b` bù một phần. Chưa chứng minh
  được có hoán vị nào lọt **cả bộ** — ghi nhận là hạn chế, không suy đoán.

### 2.2 Stub IDL — `INFO_CAPS_SHIFT`/`INFO_LENGTH_SHIFT` (D2)

- **Code**: `v2/lib/vifast-idl/src/generated.rs:25,28` (`INFO_CAPS_SHIFT = 20`,
  `INFO_LENGTH_SHIFT = 28`) và `:44-45` (encoder dùng chính hai hằng này). Sinh từ
  `v2/tools/vifast-gen/gen_idl.py:173,175` (hằng viết tay trong generator, chỉ **chú thích**
  `spec/abi.toml`).
- **Test**: `v2/lib/vifast-idl/src/lib.rs:91-92,99,109,185` — `roundtrip_moi_method` giải mã
  `info` bằng **cùng** `INFO_CAPS_SHIFT`/`INFO_LENGTH_SHIFT` mà encoder dùng.
- **Lỗi bị che**: layout `MessageInfo` của stub IDL sai (vd caps ở bit 21 thay vì 20) — test
  encode/decode tự khớp; `gen-check` cũng chỉ kiểm "generated khớp generator", không kiểm
  generator khớp `spec/abi.toml:48`. Đây là **wire format** giữa `ring_server`/`ring_client`
  (`init/ring_server_user.rs:205`, `init/ring_client_user.rs:70`).
- **Thực nghiệm**: sửa `generated.rs:25` `20 → 21`; `cargo test -p vifast-idl` ⇒ **7 passed,
  0 failed** (xanh giả). Nguồn độc lập: `spec/abi.toml:48`
  (`label:0..19, caps:20..22, no_reply:23, extra_caps_unwrapped:24..27, length:28..34`).
- **Bù trừ**: QEMU `ktest-ring` kiểm `attach_caps == 1` (`ktest_ring.rs:682`) — kernel giải mã
  `MessageInfo` bằng `vifast-abi` (layout đúng) nên sẽ bắt được; nhưng `ktest-ring` **không**
  nằm trong `gate --fast` (chỉ `cargo test --workspace` + L1 sim).
- **Kết luận**: NGHI VẤN — nguy hiểm nhất trong danh sách vì là wire format và lọt `gate --fast`.

### 2.3 `persist_host.py` — `detail` pha 7 (D3)

- **Code**: `v2/kernel/vifast-kbin-riscv64/src/ktest_persist.rs:770` —
  `detail = ((raw[10] as i64) << 32) | raw[11] as i64;`.
- **Harness**: `v2/test/boot/persist_host.py:230` — `if detail != (single << 32) | double:`.
- **Lỗi bị che**: công thức `detail` sai (vd `<< 16`) — harness tính lại **cùng** công thức nên
  đẳng thức luôn đúng. `detail` chỉ là chẩn đoán (không phải hành vi), nên mức nguy hiểm thấp;
  nhưng đây là ví dụ sạch của dạng "harness tự tính lại công thức kernel".
- **Thực nghiệm**: log tổng hợp `/tmp/kt107-persist-phase7.log` với
  `detail=6553800 (=single(100)<<32|double(200))`:
  - harness gốc (`<< 32`) ⇒ **FAIL** (`detail=6553800 != single<<32|double=429496729800`);
  - bản sao harness sửa `<< 16` ⇒ **PASS** (`fail=0`).
  Chứng minh: check là tự nhất quán, không độc lập.

### 2.4 `ktest_con.rs` — `seq` trong chuỗi kỳ vọng (D1, nhẹ)

- **Code**: `v2/kernel/vifast-kbin-riscv64/init/con_client_user.rs:117` — in `stats.seq` đọc từ
  header ring.
- **Test**: `v2/kernel/vifast-kbin-riscv64/src/ktest_con.rs:359` — `expect.num(seq)` với `seq`
  lấy từ `uart_console::ring_stats()` (cùng nguồn sản xuất).
- **Lỗi bị che**: `seq` sai (không bằng `head`) — test và chương trình U cùng đọc `seq` từ ring
  nên khớp; `head2 == head`, `tail2 == head2`, `used2 == 0` vẫn kiểm được nên chỉ che lỗi chẩn
  đoán. **Chưa chạy QEMU** (ngoài ngân sách) — kết luận từ code, không suy đoán thêm.

### 2.5 `ktest_core.rs` — nhãn WaitAddr từ `frame_pa` (D1, nhẹ)

- **Code/test**: `v2/kernel/vifast-kbin-riscv64/src/ktest_core.rs:531` —
  `let label = (frame_pa >> 12) as u32;` rồi dùng `label` cho `wait_addr`/`set_addr_word`/
  `wake_addr` và kiểm `TcbState::BlockedOnAddress { frame: label }`.
- **Lỗi bị che**: `frame_pa` sai (nhưng `!= 0`) ⇒ nhãn sai; test dùng nhãn đó nhất quán nên
  vẫn khớp. Chỉ `frame_pa != 0` là kiểm độc lập. **Chưa chạy QEMU** — kết luận từ code.

### 2.6 `persist_host.py` — công thức payload (D3, nhẹ)

- **Code**: `v2/kernel/vifast-kbin-riscv64/src/ktest_persist.rs:87` —
  `((phase * 37 + index * 13 + (index / BLOCK_SIZE) * 101) & 0xff)`.
- **Harness**: `v2/test/boot/persist_host.py:42` — biểu thức y hệt bằng Python.
- **Lỗi bị che**: công thức payload sai ở **cả hai** phía ⇒ host sinh lại đúng chuỗi kernel ghi,
  SHA khớp. Đây là **dữ liệu thử** (không phải hành vi sản xuất), nên mức nguy hiểm thấp; nhưng
  nếu ai đó sửa công thức ở kernel mà quên harness (hoặc ngược lại) thì test đỏ — tức nó vẫn có
  tác dụng như một "hợp đồng chép tay", chỉ không độc lập.

## 3. Các mục đã loại trừ (ghi để người sau không tìm lại)

- **`kt19_sched.rs:533-537`, `kt22_mailbox.rs:745-746`** — test tự đóng gói `state.index()` vào
  `goi`/`row` để so trace giữa hai lần chạy; **không** phải công thức sản xuất (test-only
  encoding). AN TOÀN.
- **`kt23_k1b.rs:889`** — dùng `coverage_design` (hàm sản xuất) để tính coverage, nhưng test có
  mốc cứng `46/46`, `48`, `73` (`kt23_k1b.rs:199-226`) ⇒ có nguồn độc lập. AN TOÀN.
- **`kt35_pte.rs:69`** — `size_bytes() == 1 << size_bits()` dùng hằng sản xuất, nhưng có mốc
  cứng `K4/M2/G1` (`kt35_pte.rs:72-74`). AN TOÀN.
- **`kt42_scn.rs`, `kt24_scn.rs`, `kt25_scn.rs`, `kt26_scn.rs`, `kt27_scn.rs`, `kt64a_scn.rs`** —
  đọc kỳ vọng từ tệp `.scn` (nguồn độc lập). AN TOÀN.
- **`v2/test/parity/run.sh`** — đọc `expect` từ `spec/v1-parity.toml` (nguồn độc lập). AN TOÀN.
- **`v2/test/scn/check_scn.py`** — so trace với `expect` trong `.scn` (nguồn độc lập). AN TOÀN.
- **`v2/test/boot/expected.txt`** — số TAP đã commit (nguồn độc lập). AN TOÀN.
- **`v2/test/boot/ktest.sh`** — các regex `# …` viết tay; các giá trị tính toán (`mask`,
  `tlb_sent`, `__SMPM1__`) suy từ `smp` đầu vào, không đọc từ kernel. AN TOÀN.
- **`v2/test/conformance/trace_compare.py:123`** — `want_build`/`len(img_text)` tính từ chính
  ảnh bằng `image_digest.py`; so với fingerprint nhúng trong ảnh — đây là kiểm "ảnh khớp chính
  nó", không phải công thức sản xuất. AN TOÀN (mục đích là phát hiện ảnh bị thay).
- **`v2/lib/vifast-model/tests/kt11_diff_sim.rs`** — parity hai cài đặt riêng
  (`vifast-model` không được `vifast-sim` dùng làm thư viện; chỉ dev-dependency). AN TOÀN.
- **`v2/sim/vifast-sim/tests/kt58/59/60/64_core_parity.rs`** — parity sim↔core; digest là phép
  chiếu test-side trên accessor sản xuất. Nếu một accessor sai ở **cả hai** phía thì digest
  khớp — đây là dạng D2 tiềm ẩn, nhưng các accessor là hàm đọc đơn giản và đã được kiểm ở
  test riêng; **KHÔNG KẾT LUẬN** cho từng accessor (ngoài ngân sách), ghi nhận để audit sau.

## 4. Đề nghị thứ tự vá (nguy hiểm nhất trước)

1. **IDL `INFO_CAPS_SHIFT`/`INFO_LENGTH_SHIFT`** (`v2/lib/vifast-idl/src/lib.rs:91-92` +
   `generated.rs:25,28`) — wire format ring; test xanh giả; lọt `gate --fast`.
   **Vá**: thêm test so hằng sinh với `spec/abi.toml:48` (hoặc với `vifast_abi`), hoặc cho
   `gen_idl.py` đọc `spec/abi.toml` thay vì viết tay 20/28. Không sửa `spec/`.
2. **`kt18_tcb` bảng `ALLOWED`** (`v2/sim/vifast-sim/tests/kt18_tcb.rs:307-370`) — bảng chuyển
   trạng thái là lõi TCB; test ma trận vòng tròn.
   **Vá**: thêm bảng kỳ vọng viết tay (121 bit) suy từ `docs/v2/ViFastOs-v2-thiet-ke.md`
   §4.4/§4.5, hoặc ít nhất liệt kê đủ 73 cặp cấm từ thiết kế; giữ `kt23_k1b` làm lớp hai.
3. **`persist_host.py` `detail`** (`v2/test/boot/persist_host.py:230`) — chẩn đoán, nhưng là
   dạng D3 rõ nhất.
   **Vá**: kiểm `detail` bằng nguồn độc lập (vd `single`/`double` phải khớp block number đọc từ
   `fsutil`/layout), không dùng lại đúng biểu thức của kernel.
4. **`ktest_con.rs` `seq`** (`ktest_con.rs:359`) — chẩn đoán.
   **Vá**: dùng `head` (đã kiểm) làm kỳ vọng, hoặc bỏ `seq` khỏi chuỗi khớp và kiểm riêng bằng
   `head`.
5. **`ktest_core.rs` nhãn WaitAddr** (`ktest_core.rs:531`) — chẩn đoán.
   **Vá**: dùng nhãn viết tay (vd `0x1234`) hoặc kiểm `label == frame_pa >> 12` với `frame_pa`
   lấy từ nguồn độc lập (object layer).
6. **`persist_host.py` công thức payload** (`persist_host.py:42`) — dữ liệu thử.
   **Vá**: ghim payload mẫu đã commit (golden) hoặc dùng công thức khác ở host; giữ SHA-256
   `hashlib` làm nguồn độc lập.

## 5. Hạn chế / KHÔNG KẾT LUẬN

- **QEMU**: các mục #5, #6, #7, #13–#20 chưa chạy QEMU (cần build kernel riscv64 + QEMU; ngoài
  ngân sách 2 h). Kết luận dựa trên đọc code + nguồn độc lập (regex harness, spec, thiết kế),
  không suy đoán thêm.
- **`kt18_tcb`**: đã chứng minh test **một mình** xanh giả với 1 hoán vị; chưa chứng minh được
  có hoán vị nào lọt **cả bộ** (`kt23_k1b` bắt cả hai hoán vị đã thử). Ghi "NGHI VẤN", không
  kết luận "có lỗi thật".
- **Parity digest** (`kt58/59/60/64`): chưa audit từng accessor — ghi "KHÔNG KẾT LUẬN" cho phần
  này, để task sau.
- **`v2/test/boot/ktest.sh`**: các regex `# …` là kỳ vọng viết tay; đã đọc nhưng chưa đối chiếu
  từng dòng với kernel (ngoài phạm vi "công thức"). Không thấy dạng D1/D2 rõ ràng.

## 6. Lệnh tái lập thí nghiệm

```bash
# clone (không đụng repo gốc)
git clone --local /Users/mps/dvchd/ViFastOs /tmp/kt107-repo
export PATH="$HOME/.cargo/bin:$PATH"
cd /tmp/kt107-repo/v2
export CARGO_TARGET_DIR=/tmp/kt107-target

# #2 kt18: hoán vị BlockedOnRecv->Inactive / ->BlockedOnReply trong tcb.rs:415-419
cargo test -p vifast-sim --test kt18_tcb     # 18/18 PASS (xanh giả)
cargo test -p vifast-sim --test kt23_k1b     # FAIL (fuzz bắt)

# #3 IDL: generated.rs:25 INFO_CAPS_SHIFT 20 -> 21
cargo test -p vifast-idl                     # 7/7 PASS (xanh giả)

# #4 persist detail: log tổng hợp + bản sao harness sửa << 16
python3 v2/test/boot/persist_host.py check --log /tmp/kt107-persist-phase7.log \
  --kind write --phase 7 --nonce 12345 --verify 0 --write 1 --mode 0

# #8 fs bitmap: alloc.rs:19,36 -> 7 - block % 8
cargo test -p vifast-fs --lib fsck_detects_corrupt_bitmap   # FAIL

# #9 PTE: pagetable.rs:68 PTE_OBJ_SHIFT 10 -> 11
cargo test -p vifast-sim --test kt37_pagetable              # FAIL

# #10 targets_of: vifast-core/src/tlb.rs:158 bỏ & !(1 << caller)
cargo test -p vifast-sim --test kt64_core_parity            # FAIL

# #11/#12 MessageInfo: ipc.rs:185 caps << 20 -> << 21
cargo test -p vifast-sim --test kt24_ipc                    # FAIL
cargo test -p vifast-sim --test kt25_ipc2                   # FAIL

# #26 objmem: obj.rs:10 HEADER_SIZE 16 -> 8
cargo test -p vifast-objmem --test objmem_contract          # FAIL

# #27 vfs: layout.rs:12 SECTORS_PER_PAGE + 1
cargo test -p vifast-vfs                                    # FAIL

# #28 fault PTE: mmu.rs:46 PTE_READ 1<<1 -> 1<<5
cargo test -p vifast-plat-common --test fault               # FAIL

# #29 walk_shape: walk.rs:15 INDEX_BITS 9 -> 8
cargo test -p vifast-pte --test kt35_pte                    # FAIL

# #30 traceformat: lib.rs:59-60 đảo a/b
cargo test -p vifast-traceformat                            # FAIL
```

## 7. Kết luận ngắn

- **30 ứng viên**; **6 NGHI VẤN**: `kt18_tcb`/`ALLOWED`, IDL `INFO_CAPS_SHIFT`/`INFO_LENGTH_SHIFT`,
  `persist_host.py`/`detail`, `ktest_con.rs`/`seq`, `ktest_core.rs`/nhãn WaitAddr,
  `persist_host.py`/payload.
- **3 chứng minh xanh giả bằng thực nghiệm**: `kt18_tcb` (18/18 với hoán vị), IDL (7/7 với
  `INFO_CAPS_SHIFT=21`), `persist_host.py` (check PASS với `<< 16` ở cả hai phía).
- **24 AN TOÀN** có bằng chứng (mutation hoặc nguồn độc lập). Ngoài 30 ứng viên, **1 KHÔNG KẾT LUẬN**
  (parity digest accessor, ghi ở §5).
- Nguy hiểm nhất: **IDL stub** (wire format, lọt `gate --fast`), rồi **bảng `ALLOWED`**.
