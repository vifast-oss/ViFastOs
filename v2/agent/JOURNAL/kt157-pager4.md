# KT-157 — N3 nợ: **bốn client mức kernel** cho pager

- **Nhánh**: `night2/L1-pager4` (worktree `/private/tmp/night2-L1-pager4`, tách từ `main` @ `6b2fdce7`).
- **Quyết định**: `D2-145` (Agent 1 quyết thay Agent 0, 2026-11-19) — đọc nguyên văn ở
  `v2/agent/DECISIONS.md:265` của worktree. Nợ `D2-136` mục (ii): "4 client mức kernel".
- **Ngân sách**: 3 h. Bắt đầu 03:50 JST 2026-11-19.
- **Commit trên nhánh** (không push, không merge): xem `git log -1` — `[KT-157] ...`.

---

## 0. Tóm tắt kết quả

| Vế | Nội dung | Kết quả |
|---|---|---|
| (1) | **Bốn** client U-mode trong ảnh `ktest-user`, mỗi client **CSpace + bảng trang riêng**, fault trên **VA riêng** (không chồng) | **ĐẠT** — A/B (KT-70) + C/D (mới), 5 VSpace cùng lúc |
| (2) | `DeliveryLedger`/`check_once` mở rộng thành sổ theo **`(client, va)`**, fail-closed | **ĐẠT** — `ktest_vfs_once.rs` |
| (3) | TAP 37 in `# pager: clients=4 deliveries=… faults=… served=… rejected=… ok=1`; đỏ nếu bất kỳ client/VA sai | **ĐẠT** — dòng evidence nguyên văn §2 |
| (4) | Đột biến "phát lại `Call` khi `Resume`" ⇒ TAP 37 **đỏ**, TAP 30..36 **vẫn xanh** | **ĐẠT** — log nguyên văn §3 |
| (5) | `./v2/bin/v2 check` PASS trên nhánh | **ĐẠT** — §4 |

**Tuyên bố**: nợ "4 client mức kernel" (D2-136 mục (ii) / D2-145) **đã đóng bằng ảnh test**, không
bằng lời: bốn tiến trình U-mode thật, mỗi cái một VSpace + CSpace, pager giao **đúng một lần** cho
**từng** `(client, va)`.

---

## 1. Thay đổi (vì sao từng chỗ)

### 1.1. Bốn client U-mode

- `init/pager_client_c_user.rs`, `init/pager_client_d_user.rs` (mới) + `init/pager_client_cd_common.rs`
  (mới): chương trình tối giản — **không** khối kết quả, **không** receive window; chỉ `Yield`,
  đọc 4096 byte mỗi trang, so `vifast_vfs::byte_at`, `Exit(0)` nếu đúng / `Exit(0x70+index)` nếu sai.
- `init/pager_common.rs`: `CLIENTS = 2 → 4`; thêm `P_VS_C = 16`, `P_VS_D = 17` (CSpace pager 32
  slot, 11..15 là đĩa); `C_BASE = 0x4006_0000`, `D_BASE = 0x4008_0000` (cùng 2 MiB block với ảnh
  ELF ⇒ **không** cấp thêm bảng trang); `C_TABLE = [(0,0),(0,1)]`, `D_TABLE = [(0,0),(0,1)]`
  (VA khác nhau nhờ base khác nhau).
- `init/pager_server_user.rs`: `vspace_cap()` map client 0..3 → `P_VS_A/B/C/D`.
- `src/ktest_vfs_rig.rs` (mới — tách khỏi `ktest_vfs.rs` để giữ **R1 ≤ 1500 dòng**): `build_rig`
  dựng **năm** VSpace (pager + A/B/C/D), 6 CNode, 6 TCB, 5 SC, 2 Endpoint; C/D **không** có khối
  kết quả; TCB/SC của C/D nằm trong **CNode của chính client** (slot 6/7) vì root CNode 32 slot đã
  bị các pha trước dùng tới slot 28 (`ktest_ring.rs:56-59`).
- `src/ktest_vfs.rs`: `page_of`/`va_of` thêm nhánh client 2/3; `deliver_and_pump` chọn
  `tcb_c/tcb_d` + `core_vs_c/core_vs_d`; `run_cd_client` chạy C/D sau kịch bản A/B.

### 1.2. Sổ theo `(client, va)`

`src/ktest_vfs_once.rs`: `DeliveryLedger` giữ `entries: [(u8 client, u64 va, u64 count); 32]`;
`record(client, va)`, `delivered_to(client, va)`, `distinct_clients()`. `check_once` **fail-closed**:
sổ rỗng / `faults == 0` / `expected` rỗng / một cặp không có trong sổ / `deliveries != planned` /
`faults != planned` / `served + rejected != faults` ⇒ `false`. TAP 37 còn đòi
`distinct_clients() == 4`, `c_exit == 0`, `d_exit == 0`, `cd_bad == 0`.

### 1.3. Tài nguyên (đo được, không nới ngưỡng cấm)

- `vifast-plat-riscv64/src/obj.rs`: `OBJ_ARENA_BYTES 64 KiB → 96 KiB`. **Lý do đo được**: 5 VSpace
  × 3 PageTable 4 KiB = 60 KiB chỉ riêng bảng trang + metadata CNode/TCB/SC/Endpoint ≈ 10 KiB ⇒
  64 KiB **không đủ** (rig fail `NotEnoughMemory`). Đây là **dung lượng bộ nhớ**, không phải ngưỡng
  thời gian/seed/`MAX_OBJECTS`; `MAX_OBJECTS = 96` **giữ nguyên**.
- `vifast-plat-riscv64/src/user.rs`: `MAX_PROCS 4 → 6` (5 ô dùng + 1 dự phòng). Dung lượng bảng dải
  (test-only), không phải `MAX_TCBS` (giữ 64).
- `vifast-plat-riscv64/src/init_pages.rs` + `user.rs`: `load_init_slot_with_stack` (tách hàm
  `map_user_stack` sang `init_pages.rs` để giữ R1) — C/D dùng **1 trang stack** (chương trình không
  đệ quy). **Lý do đo được**: `MAX_MAPPINGS = 64`; 8 trang stack cho C/D đẩy tổng mapping lên 74 ⇒
  `Frame.Map` trả `NoSlots` (đo bằng debug tạm: `# dbg: map fail va=0x40094000 err=NoSlots`).
  Với 1 trang: `maps=60` ≤ 64.
- `src/ktest_user.rs`: `loaded.release()` **trước** pha pager — trả VSpace + 12 Frame + 3 PageTable
  của `init` (đã `Exited`, TAP 1..6 đã đo xong) để `MAX_OBJECTS = 96` đủ cho 5 VSpace. Không đổi
  TAP nào; `# capa: u0 truoc rig` (không nằm trong `expected.txt`) in trước khi release.

### 1.4. Harness

- `v2/test/boot/ktest.sh`: hai regex evidence của ảnh `ktest-user` —
  `const:clients=2 → const:clients=4`; dòng `# pager: once …` → `# pager: clients=4 …`.
- `v2/test/boot/expected.txt`: ghi chú KT-157 (số TAP **vẫn 37**, không thêm TAP).

---

## 2. Bằng chứng XANH (nguyên văn)

Lệnh:

```bash
cd /private/tmp/night2-L1-pager4
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 \
  --bin vifast-kbin-ktest-user --features ktest,user \
  --target riscv64gc-unknown-none-elf --release
bash /tmp/kt157-boot-user.sh /tmp/kt157-green-smp1.log 1
bash v2/test/boot/ktest.sh --check-log ktest-user /tmp/kt157-green-smp1.log 1
```

Dòng TAP 37 xanh (log `/tmp/kt157-green-smp1.log`, dòng 257):

```text
# pager: clients=4 deliveries=14 faults=14 served=13 rejected=1 expected_va=11 distinct_va=11 ok=1
ok 37 - pager giao dung mot lan
```

`--check-log`:

```text
ok - check-log ktest-user smp=1 (plan=37 ok=37, 64 evidence) log /tmp/kt157-green-smp1.log
# check-log: PASS
```

TAP 30..36 xanh cùng log:

```text
ok 30 - pager fault den server va nap trang
ok 31 - pager hit khong doc backend
ok 32 - pager eviction LRU khong mat du lieu
ok 33 - pager hai client chia se mot trang
ok 34 - pager page ngoai file tra ma
ok 35 - pager ca am quyen va khong ghi de
ok 36 - pager bat bien reads==misses
```

**Đọc số**: `deliveries=14` = 5 (A) + 5 (B) + 2 (C) + 2 (D); `faults=14` pager nhận đúng 14;
`served=13` + `rejected=1` = 14; `expected_va=11` cặp `(client,va)` phân biệt (A 3 + B 4 + C 2 +
D 2); `distinct_va=11` sổ có đúng 11 cặp; `clients=4` sổ có đủ bốn client.

---

## 3. Bằng chứng ĐỎ dưới đột biến (nguyên văn)

Đột biến **tạm thời** trong `deliver_and_pump` (`ktest_vfs.rs`): sau khi nhận `served` khớp trang,
kernel **gửi lại** đúng message `pager.fault` vừa phục vụ (một lần, `client=0,file=0,page=0`),
**không** ghi vào sổ. Đã khôi phục sau khi đo (không commit).

```bash
cargo build … --bin vifast-kbin-ktest-user --features ktest,user --release
bash /tmp/kt157-boot-user.sh /tmp/kt157-red-mutation-smp1.log 1
```

Log `/tmp/kt157-red-mutation-smp1.log`:

```text
ok 30 - pager fault den server va nap trang
ok 31 - pager hit khong doc backend
ok 32 - pager eviction LRU khong mat du lieu
ok 33 - pager hai client chia se mot trang
ok 34 - pager page ngoai file tra ma
ok 35 - pager ca am quyen va khong ghi de
ok 36 - pager bat bien reads==misses
# pager: clients=4 deliveries=14 faults=15 served=13 rejected=2 expected_va=11 distinct_va=11 ok=0
not ok 37 - pager giao dung mot lan: message bi giao/xu ly lap, hoac so dem theo (client,va) khong doc duoc
```

**Đọc số**: TAP 30..36 **vẫn xanh** ⇒ đỏ **quy được** về TAP 37. `deliveries=14` (kernel ghi sổ 14)
nhưng `faults=15` (pager **nhận** 15) ⇒ **1 message bị giao lặp**; `rejected` 1 → 2. Checker bắt
đúng chỗ.

---

## 4. `./v2/bin/v2 check` PASS

```text
--- fmt: OK
--- clippy: OK
--- luật cấu trúc: OK
...
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
```

---

## 5. Còn thiếu / hạn chế

- **Không** chạy `gate --fast` (luật đêm D2-148: sub-agent chỉ `v2 check` + test đúng phạm vi).
- **Không** chạy `ktest.sh` đầy đủ matrix 1/2/4 cho cả 7 ảnh trong ngân sách này — chỉ boot
  `ktest-user` `-smp 1` (log §2) + `--check-log`. `-smp 2/4` không đổi đường pager (một hart chạy
  U-mode tuần tự), nhưng **chưa đo**; lead nên chạy `bash v2/test/boot/ktest.sh 1 matrix` sau merge.
- C/D dùng **1 trang stack** (không phải 8 như A/B) — chương trình tối giản, không đệ quy; nếu sau
  này C/D cần stack lớn hơn thì phải nâng `MAX_MAPPINGS` (hiện 64, đã dùng 60) — **chưa** làm vì
  không cần cho phép đo này.
- `OBJ_ARENA_BYTES` 64 → 96 KiB và `MAX_PROCS` 4 → 6 là **thay đổi dung lượng bộ nhớ/danh sách**
  (không phải ngưỡng thời gian/seed/`MAX_OBJECTS`); ghi ở §1.3 để reviewer đối chiếu.
