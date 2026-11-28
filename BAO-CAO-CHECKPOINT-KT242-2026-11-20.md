# BÁO CÁO CHECKPOINT — KT-242 (thi hành quyết định Agent 0 ngày 2026-11-20)

**Từ**: lead (Agent 1).
**Ngày**: 2026-11-20, 16:40 JST.
**Chỉ thị**: quyết định Agent 0 trên `PHIEU-GUI-AGENT0-2026-11-20-PERF.md` — tách perf khỏi
toolchain, không merge nguyên `226eae9a`, duyệt A+B, C/P3 sau cô lập, D đo rồi chọn, chưa duyệt
E/nextest, nhóm toolchain phải xử lý artifact cũ + review indexing, rồi gate + ảnh + K6.

**Trạng thái một khối**

```text
ĐÃ MERGE        perf 417e2a18 -> main (đã push)
                toolchain 50958dd1 + fmt 45534b78 -> main (đã push)
                hồ sơ c6127fa0 -> main (đã push)  ← HEAD khi đóng hồ sơ
GATE PERF       PASS 53/53 bước, 702,07 s   (log v2/agent/JOURNAL/kt242-logs/kt242-gate-perf.log)
GATE TOOLCHAIN  PASS 53/53 bước, 669,07 s   (log v2/agent/JOURNAL/kt242-logs/kt242-gate-tc.log)
GATE MAIN       PASS 53/53 bước, 684,64 s   (log v2/agent/JOURNAL/kt242-logs/kt242-gate-main.log)
L0             92 binary, 789 test, 0 đỏ (jobs=6, threads=2)
ẢNH SẢN PHẨM   d915b977edbdae4661a3bb31e4aa7ec33f8e9d703bbba2c7f88069f200e541dc
               191 288 B, build=algo=kt134-v4 inputs=54f3b59fc08f3221 dirty=0
K6 ĐO LẠI      ktest 21/21 · persist PASS · conformance 61/61 · soak 1000/1000 · L1 400k 4/4
               trên ảnh d915b977… (inputs=54f3b59fc08f3221, dirty=0)
K6 HỢP ĐỒNG    C1..C8 phủ; N1..N7 và S1 GIỮ MỞ (không đo lại) — xem §4.1
P20             sha256 phụ thuộc thư mục build (5 lần build, 4 sha) — xem §4.2
```

---

## 1. Tách nhóm (đúng Q1)

`226eae9a` **không** được merge. Đã tách thành hai commit độc lập:

| Nhóm | Commit | Nội dung | Đổi ảnh sản phẩm? |
|---|---|---|---|
| **perf** | `ffe77527` | P1 runner L0, P2 tách fuzz, P7 prune, P9 đo thời gian, tệp tạm, fixture git-guard | **KHÔNG** |
| **toolchain** | `9d0b03fd` | Rust 1.100.0, 19 lint, bố cục artifact, dep-info | **CÓ** (rust-toolchain.toml ∈ build-inputs) |

Nhánh toolchain **đặt trên** nhánh perf (đúng thứ tự A0: tích hợp perf trước, rebase toolchain sau).

---

## 2. Nhóm perf — `ffe77527`

### 2.1 Gate: PASS 53/53

```text
v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
real 702.07   user 793.56   sys 266.74
# step-time-total   701817 ms  TỔNG gate --fast
```

**53/53 bước OK, 0 FAIL** — lần đầu cổng xanh **đầy đủ** trong worktree (bước `git-guard b` đã
được sửa fixture, xem §2.3).

### 2.2 Tám điều kiện A0 cho P1 — đã đáp ứng

| # | Điều kiện A0 | Cách đáp ứng |
|---|---|---|
| 1 | Danh sách executable từ Cargo JSON **của lần build hiện tại**, không quét binary cũ | `cargo test --no-run --message-format=json` → lọc `profile.test` → `executable` |
| 2 | Đối chiếu **test được phát hiện/thực thi** trước–sau, không chỉ đếm binary | in `# L0: 92 binary, 789 test đã chạy, 0 đỏ`; 789 khớp con số nextest phát hiện độc lập |
| 3 | Giữ đúng cwd, biến môi trường, điều kiện runtime | runner `cd` vào **thư mục gói** (như cargo); đã tái hiện lỗi cũ khi thiếu cwd |
| 4 | Binary thất bại / không khởi chạy / bị signal ⇒ L0 đỏ | `.rc` riêng: 126 = không chạy được, 97 = không vào được thư mục gói, >128 = signal ⇒ đỏ |
| 5 | Không để pipeline/xargs che mã lỗi; xử lý đường dẫn có dấu cách | mã thoát ghi ra tệp `.rc`; danh sách **NUL-separated** + `xargs -0 -n 2` |
| 6 | Doc-test và workspace kernel vẫn chạy đầy đủ | `cargo test --workspace --doc` chạy riêng; kernel workspace là bước riêng của cổng |
| 7 | Có đường chạy tuần tự để đối chiếu/chẩn đoán | `V2_TEST_JOBS=1` hoặc `V2_TEST_SERIAL_ALL=1` |
| 8 | Giới hạn **tổng** mức song song, không chỉ số process | `threads = ncpu / jobs` (mặc định 12/6 = 2) ⇒ tổng ≤ 12 luồng |

### 2.3 Fixture git-guard b (A0 duyệt sửa)

`neg-test-kt85b.sh` clone repo rồi đòi `refs/heads/main`; chạy từ worktree nhánh phát triển thì
clone không có `main` ⇒ đỏ vì môi trường. Đã sửa: nếu clone thiếu `main` thì tạo
`main = <commit main của repo thật>`. **Vẫn kiểm đúng luật bảo vệ** (cherry-pick/merge/reset vào
chính `refs/heads/main`), không tắt guard, không ép nhánh phát triển tên `main`.
Kết quả: **7/7 ca PASS** (trước: đỏ ở ca đầu).

### 2.4 P2 — chứng minh tập seed/cấu hình bằng nhau

`fuzz_smp_tat_dinh_bat_bien` (1 test) → `fuzz_smp_harts_{1,2,4,8}` (4 test), mỗi test gọi
`fuzz_smp_mot_cau_hinh(harts)` với **cùng** `so_seed()` = 1.000 và `so_op()` = 64, vẫn so trace
hai lượt mỗi seed. Tập (seed × op × cấu hình) **không đổi**; chỉ chia nhỏ đơn vị chạy.

### 2.5 D — đo rồi chọn (A0: không mặc định jobs=10)

Cùng cây, cùng binary, chỉ đổi `jobs`/`threads` (789 test mỗi cấu hình, 0 đỏ):

| jobs | threads | thời gian |
|---:|---:|---:|
| **6** | **2** | **55,3 s** ← chọn |
| 6 | auto (12) | 56,4 s |
| 4 | 3 | 65,1 s |
| 8 | 1 | 72,0 s |
| 10 | 1 | 76,2 s |
| 12 | 1 | 78,1 s |

**Kết luận**: `jobs=6, threads=2` nhanh nhất **và** có trần tổng 12 luồng. Tăng jobs lên 10/12
**chậm hơn** — đúng như A0 lo. Mặc định giữ 6/2.

### 2.6 Tệp tạm (A0: PID chưa đủ)

`compile_fail.rs` dùng **thư mục tạm duy nhất theo lần gọi** (PID + đồng hồ nano) và
**cleanup trước mọi assert** — đỏ vì lý do gì cũng không để rác.

---

## 3. Nhóm toolchain — `9d0b03fd`

### 3.1 Gate: PASS 53/53

```text
v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
real 669.07   user 740.10   sys 252.55
# step-time-total   668808 ms  TỔNG gate --fast
```

### 3.2 Ảnh sản phẩm mới

```text
sha256   d915b977edbdae4661a3bb31e4aa7ec33f8e9d703bbba2c7f88069f200e541dc
kích cỡ  191 288 B        (ảnh cũ 639ca6d1…: 213 632 B)
build    algo=kt134-v4 inputs=54f3b59fc08f3221 dirty=0
HEAD     9d0b03fd0584
```

`build-inputs`: **234 mục**, dấu vân tay `54f3b59fc08f3221` (cũ: `43df126a4a6702ec`).
`REV-GATE: ACCEPT` ở cả hai chỗ (ktest và persist).

### 3.3 4.1 — dep-info gắn với artifact, không chọn thư mục theo tồn tại

Bỏ `depinfo_dir()` (ưu tiên `debug/deps` nếu tồn tại). Thay bằng `depinfo_dirs()`:
- lấy `dirname(artifact)` của **từng artifact cargo báo trong lần chạy này** ⇒ gắn với lần build
  và cấu hình đang kiểm;
- hai bố cục cùng tồn tại vẫn lấy đúng tệp của lần này;
- build đỏ (không artifact) ⇒ quét **cả hai** bố cục (superset, fail-closed), tầng gọi báo
  `E_DEPINFO_BUILD`;
- dấu lần build lưu **danh sách** thư mục (`STAMP_VERSION=2`); `--no-build` đọc thư mục **từ dấu**,
  không đoán; thiếu artifact/dấu ⇒ **thất bại rõ**, không PASS rỗng.

Selftest thêm ba ca: **(52)** hai bố cục cùng tồn tại, **(53)** build đỏ, **(54)** layout trộn.
Kết quả: **62/62 ok**. Chạy thật trên 1.100.0: `errors=0 warnings=0`, 20 artifact, 20 `.d`,
148 tệp `.rs`; `--no-build` cũng `errors=0`.

`compile_fail.rs`: tìm rlib bằng cách đi lên tới gốc `debug` rồi quét đệ quy — đúng cả hai bố cục.

### 3.4 4.2 — review từng chỗ indexing → iterator

| Chỗ | Bất biến | Cách xử lý |
|---|---|---|
| `fsck.rs:70` `used..end` | `used > end` ⇒ vòng rỗng | `take(end).skip(used)` — **giữ đúng** hành vi rỗng |
| `fsck.rs:199` `0..MAX_DIR_BLOCKS` (11) | mảng `[u32; 12]` | `take(11)` — không thể vượt |
| `fsck.rs:207` `0..direct.min(12)` | `.min(12)` | `take(...)` — không thể vượt |
| `fsck.rs:460` `0..blocks_used().min(11)` | `.min(11)` | `take(...)` — không thể vượt |
| `fs.rs:499` `0..12` | mảng `[u32; 12]` | `take(12)` — đúng độ dài |
| `tx.rs:58,213` `0..tx_count` | `tx_count <= MAX_TX_WRITES` **đã kiểm trước khi tăng** | `take(count)` |
| `alloc/lib.rs:408` `0..n_holes` | `n_holes <= MAX_HOLES` đã kiểm trước khi ghi | **slicing** `&holes[..n]` — từ chối vượt biên, không cắt ngắn |
| `dir.rs:471` `0..blocks_used()` | `blocks_used()` **kẹp** `min(12)` khi `INODE_DIR`; hàm chỉ chạy trên thư mục | **slicing** `&blocks[..used]` |
| `dir.rs:587,602,673`, `path.rs:243,275` | như trên (đọc/sửa entry của thư mục) | **slicing** `&blocks[..blocks_used()]` |
| `kobj.rs:6009` `0..n` | `sched` và `published` cùng dựng `(0..harts)` | **slicing** `&published[..n]` |
| `kobj.rs:3434` | `0..hart_vspace.len()` | `enumerate()` — đúng độ dài |
| `kt60_core_parity.rs` | mảng test cố định | `take` đúng độ dài |

**Cách ghi đúng (A0 §4)**: các chỗ đổi sang slicing **giữ hành vi từ chối vượt biên**, không âm
thầm cắt ngắn — nhưng **không** ghi "panic giống hệt": slicing panic **trước khi vòng lặp bắt
đầu**, còn indexing cũ có thể đã làm một phần việc rồi mới panic. Khác biệt đó **chỉ có ý nghĩa**
ở các vòng có tác dụng phụ (`write_dir_entry`, `remove_dir_entry` ghi thiết bị); ở đó nhánh vượt
biên **không tới được** vì `blocks_used()` kẹp `min(MAX_BLOCKS_PER_INODE)` cho `INODE_DIR`. Các
chỗ còn lại là bất biến đã chứng minh trong code (const bằng đúng độ dài mảng, `tx_count` kiểm
trước khi tăng, `n_holes` kiểm trước khi ghi, `sched`/`published` cùng `harts`). Không dùng
`#[allow]`. `clippy -D warnings` RC=0.

---

## 4. K6 trên ứng viên — đã đo xong

| Hạng mục | Kết quả | Ghi chú |
|---|---|---|
| `ktest.sh 1 matrix` | **21/21 boot sạch** | 7 ảnh × smp 1/2/4, 2 m 10 s |
| `persist.sh` | **PASS** | 10 boot, 7/7 pha, mỗi pha 2 tiến trình khác nhau |
| `conformance/run.sh --boot --smp 2` | **PASS** | 61/61 dòng trace khớp, 5/5 ca |
| `soak.sh 1000 3` | **PASS — 1000/1000 boot, 0 lỗi** | wall 240 s, `BUILD_ID=eca79baef309297c`, sha khớp `d915b977…` |
| L1 400k seed (4 tiến trình song song) | **PASS 4/4** | smp 1/2/4/8: 1501 / 1626 / 1774 / 2088 s, 0 lỗi |

**Đã đo lại đủ năm hạng mục K6 trên ảnh mới** (`d915b977…`): ktest 21/21, persist PASS,
conformance PASS, soak 1000/1000, L1 400k PASS. **Kết luận K6 ĐẠT/không là của Agent 0** — tôi chỉ
báo số. Bằng chứng K6 của ảnh cũ `639ca6d1…` giữ nguyên giá trị lịch sử cho ảnh cũ, **không**
chuyển sang ảnh mới (đúng A0 §5).

### 4.1 Ánh xạ "năm hạng mục đo" ↔ hợp đồng K6 hiện hành (A0 yêu cầu)

Hợp đồng K6 hiện hành (`v2/agent/reports/K6-DOD.md`) **không** đếm bằng năm hạng mục. Nó gồm:

- **C1..C8** — tám hạng mục đo được của dòng K6 (VFS journal, fsck, rename, truncate,
  double-indirect, pager nhiều client, exec chain, test-persist bảy pha);
- **N1..N7** — bảy lỗi v1 trong `docs/v1/KNOWN-ISSUES.md`;
- **S1** — "tương đương v1 ở mức người dùng" (phát hiện của KT-76, **không** có trong §10.1).

Ánh xạ lần đo này:

| Hạng mục tôi đo | Bao phủ | Không bao phủ |
|---|---|---|
| `ktest.sh 1 matrix` 21/21 | **C1..C7** — chạy lại đúng bộ `expected.txt` của 7 ảnh ktest (journal/fsck/rename/truncate/double-indirect/pager/exec) | — |
| `persist.sh` PASS | **C8** — bảy pha, mỗi pha hai tiến trình khác nhau | — |
| `conformance --boot --smp 2` | tương đương trace sim ↔ QEMU (61/61 dòng) | — |
| `soak.sh 1000 3` | độ ổn định boot (K3) | — |
| L1 400k seed | tính tất định của sim (bốn cấu hình smp) | — |

**Giữ mở, không đổi cách đếm**: **N1..N7** và **S1** **không** được đo lại trong đợt này. Trạng
thái cũ của chúng trong `K6-DOD.md` **giữ nguyên** (N4/N6 ĐẠT; N1/N2/N3/N7 **KHÔNG ĐẠT**; N5
không áp dụng; **S1 KHÔNG ĐẠT** — ảnh sản phẩm không có shell/U-mode). Vì vậy báo cáo này **không**
tuyên bố "K6 đạt toàn bộ hợp đồng"; nó chỉ nói **năm hạng mục đo lại trên ảnh mới đều đạt**.

### 4.2 Tái lập byte: đường build **chuẩn** tái lập được, build tay thì không (nợ P20)

Cùng commit + cùng `build-inputs` (`54f3b59fc08f3221`), bốn lần build:

| Lần build | Thư mục | Đường | sha256 |
|---|---|---|---|
| Trong cổng (16:28) | `/tmp/kt242-tc` | `ktest.sh` rev-gate | `d915b977…` |
| Build tay (17:47) | `/tmp/kt242-tc` | `cargo build` | `b77fff0d…` |
| Worktree sạch | `/tmp/kt242-check` | `cargo build` | `d0eeacab…` |
| **Đo lại K6 (18:00)** | `/tmp/kt242-tc` | `ktest.sh` rev-gate | **`d915b977…`** ← trùng lần đầu |
| **Cây chung sau merge (18:50)** | `/Users/mps/dvchd/ViFastOs` | `ktest.sh` rev-gate | **`746acb05…`** |

**Kết luận (P20)**: sha256 **phụ thuộc thư mục build**, không chỉ nội dung. Cùng
`build-inputs=54f3b59fc08f3221`, cùng dòng `# build: algo=kt134-v4 inputs=… dirty=0`, nhưng bốn
thư mục/đường build cho bốn sha. Trong **cùng một thư mục** thì tái lập được (`d915b977` hai lần).
Nguyên nhân khả dĩ: `-C metadata` của cargo băm đường dẫn tuyệt đối của package.

**Hệ quả cho bằng chứng**: sha256 **không** dùng được làm danh tính bền xuyên thư mục; danh tính
bền là dòng `# build: algo=… inputs=… dirty=…` (giống nhau ở **cả năm** lần build). Bằng chứng K6
dưới đây neo vào **`inputs=54f3b59fc08f3221` + `algo=kt134-v4` + `dirty=0`**, đo trên ảnh
`d915b977…` (thư mục `/tmp/kt242-tc`). **Không** tuyên bố đã đo trên ảnh `746acb05…` của cây
chung — muốn thế phải chạy lại 5 hạng mục ở cây chung (nợ P20 phải xử lý trước nếu muốn sha làm
danh tính).

### 4.3 Đo lại K6 trên ảnh `d915b977…` (sau khi sửa chọn rlib)

| Hạng mục | Kết quả | Thời gian |
|---|---|---:|
| `ktest.sh 1 matrix` | **PASS 21/21** | 137 s |
| `persist.sh` | **PASS** (7/7 pha) | 24 s |
| `conformance --boot --smp 2` | **PASS** (61/61 dòng) | 74 s |
| `soak.sh 1000 3` | **PASS 1000/1000, 0 lỗi** | 251 s |
| L1 400k (4 tiến trình song song) | **PASS 4/4** (1506 / 1633 / 1782 / 2088 s) | 2088 s |

Cuối chuỗi, script in lại danh tính: `sha256=d915b977…`, `# build: algo=kt134-v4
inputs=54f3b59fc08f3221 dirty=0`, `HEAD=00b82391`. Tức **cùng ảnh, cùng đầu vào** với lần A0 đã
nhận — chỉ khác commit (thêm bản sửa chọn rlib, không nằm trong đầu vào tạo ảnh).

---

## 5. Số đo — nói rõ điều kiện (đúng A0 §7)

| Lần chạy | Cây | Toolchain | Thời gian | Ghi chú |
|---|---|---|---:|---|
| Baseline `a2e18cdf` | main | 1.99.1 | **1.032 s** | 52 bước, `STATE.md` |
| Gộp `226eae9a` | worktree | 1.100.0 | 629,8 s | **không dùng** — gộp cả hai nhóm |
| **Perf `ffe77527`** | worktree | 1.99.1 | **702,1 s** | 53 bước, build lạnh (đổi toolchain) |
| **Toolchain `9d0b03fd`** | worktree | 1.100.0 | **669,1 s** | 53 bước, build lạnh |

**Không quy toàn bộ mức giảm cho riêng perf**: lần gộp 629,8 s có toolchain mới và target ấm;
lần perf 702,1 s có build lạnh. Cặp gần nhất là **baseline 1.032 s → perf 702,1 s** (cùng 1.99.1)
= **mức giảm quan sát được ~32%** — **không** phải benchmark đồng nhất: baseline có **52** bước,
lần perf có **53** bước (thêm bước `git-guard b` chạy đủ 7 ca thay vì đỏ sớm), và điều kiện
target khác nhau. Cặp **perf 702,1 s → toolchain 669,1 s** (cùng nhánh, khác toolchain) = −4,7%,
cũng chỉ là quan sát.

Phần **chưa phân bổ**: `# step-time-total` chỉ cộng các bước có `run_step`; các bước inline
(`rules`, `abi spec`, `golden`, `scn`, `report`, `gen`, `idl`) **chưa** được đo — chênh lệch
giữa `real` và tổng bước là phần này.

---

## 6. Việc còn lại

1. ~~soak 1.000 boot~~ — **PASS 1000/1000**.
2. ~~L1 400k seed~~ — **PASS 4/4**.
3. ~~merge/push perf~~ — `417e2a18` trên `origin/main`.
4. ~~khép chọn rlib + gate ứng viên cuối~~ — `00b82391`, gate 53/53 trên nhánh và trên cây chung.
5. ~~merge/push toolchain~~ — `50958dd1` + `45534b78` trên `origin/main`.
6. **Kết luận K6** — **chờ Agent 0** (tôi chỉ báo số, không tự kết luận).
7. **P20** — sha256 phụ thuộc thư mục build; muốn dùng sha làm danh tính thì phải xử lý nợ này.
8. **C/P3** — chưa làm, chờ bước cô lập tài nguyên (A0 §3).
9. **E, nextest** — không làm (A0 §6).

---

## 7. Tệp bằng chứng

| Tệp | Nội dung |
|---|---|
| `v2/agent/JOURNAL/kt242-logs/kt242-gate-perf.log` | gate 53/53 trên `ffe77527` (702 s) |
| `v2/agent/JOURNAL/kt242-logs/kt242-gate-tc.log` | gate 53/53 trên `9d0b03fd` (669 s) |
| `v2/agent/JOURNAL/kt242-logs/kt242-gate-main.log` | gate 53/53 trên cây chung sau merge (685 s) |
| `v2/agent/JOURNAL/kt242-logs/kt242-soak.log` | soak 1.000 boot, 0 lỗi |
| `v2/agent/JOURNAL/kt242-logs/kt242-l1-smp{1,2,4,8}.log` | L1 400k, bốn cấu hình |
| `v2/agent/JOURNAL/kt242-logs/l0-bench.sh` | script đo D (6 cấu hình) |
| `/tmp/kt242-k6b/*.log` | đo lại K6 trên ảnh `d915b977…` (ktest/persist/conformance/soak/L1) |
| `/tmp/kt242-tc` | worktree nhánh `task/kt242-toolchain` |
| `/tmp/kt242-perf` | worktree nhánh `task/kt242-perf` |
