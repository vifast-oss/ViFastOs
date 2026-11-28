# PHIẾU GỬI AGENT 0 — tối ưu thời gian build/test + nâng toolchain Rust

**Từ**: lead (agent điều phối phiên này).
**Ngày**: 2026-11-20, ~16:00 JST.
**Chỉ thị của người dùng**: "nâng cấp rust dự án lên bản mới nhất và tiếp tục xử lý việc build,
test và phát triển bị lâu"; sau đó: "viết tất cả báo cáo này ra 1 file để mình gửi cho agent0
phân tích quyết định".
**Phạm vi đã làm**: chỉ trong **worktree riêng** `/tmp/kt242-perf`, nhánh `task/kt242-perf`,
commit `226eae9a`. **Cây chung `main` không bị sửa** (chỉ dọn artifact — xem §6).
**Trạng thái**: **CHỜ AGENT 0 QUYẾT ĐỊNH** — chưa merge, chưa push.

Nhãn số liệu theo AGENTS.md §1: `[ĐO]` = đo trực tiếp hôm nay; `[ĐO-KT110]` = đo 2026-11-18
(`v2/agent/JOURNAL/kt110-perf.md`); `[ƯL]` = ước lượng từ số đo.

---

## 1. TL;DR — một khối

```text
VẤN ĐỀ      gate --fast 17m12s, check ~3,5 phút, máy 12 nhân chỉ dùng ~0,85 nhân (7%).
            Nút cổ chai KHÔNG phải L1 (0,9 s) và KHÔNG phải macOS — mà là chạy TUẦN TỰ.
ĐÃ LÀM      (a) nâng Rust 1.99.1 -> 1.100.0 (bản stable mới nhất) + vá 19 lint + vá 2 chỗ
                giả định bố cục artifact cũ;
            (b) L0 test chạy song song 6 luồng; tách fuzz kt29_smp thành 4 test;
            (c) find -prune trong luật cấu trúc; in thời gian từng bước;
            (d) dọn 630.150 tệp .o rác trong 3 target dir (không đụng mã).
KẾT QUẢ     gate --fast 1.032 s -> 629,8 s (-39%); L0 test ~290 s -> 76,1 s (-74%).
            Cổng trên worktree: 52/53 bước OK; bước đỏ duy nhất là artifact của worktree
            (git-guard b đòi refs/heads/main), không phải hồi quy.
CÒN LẠI      630 s chia: ktest 119 s · selfbreak 115 s · L0 76 s · họ wake-paths 88 s ·
            qemu guard 22 s · persist 27 s · build-inputs 19 s · K6 18 s · dep-info 15 s.
ĐỀ XUẤT     A gộp 3 lượt wake-paths (-58 s) · B selfbreak song song (-90 s) ·
            C chia cổng 3 làn (-150 s) · D tinh chỉnh L0 (-25 s) · E profile ảnh test (-60 s,
            RỦI RO CAO, cần ngài quyết). A+B+C+D: 630 s -> ~300 s.
CẦN DUYỆT   1) merge nhánh task/kt242-perf? 2) duyệt A/B/C/D? 3) E? 4) ảnh sản phẩm đổi sha
            vì toolchain mới => K6 phải đo lại (D2-191) — ngài quyết thứ tự.
```

---

## 2. Bối cảnh: vì sao "chậm" và "máy vẫn rảnh"

`[ĐO-KT110]` đo bằng `/usr/bin/time -p` trên chính máy này:

| Lần | real | user | sys | suy ra |
|---|---:|---:|---:|---|
| `gate --fast` lần 1 | 407,19 s | 277,19 s | 55,16 s | **0,82 nhân / 12** |
| `gate --fast` lần 2 | 370,98 s | 284,08 s | 51,01 s | **0,90 nhân / 12** |

Máy: **12 nhân** (2 Super + 4 Performance + 6 Efficiency), 32 GB RAM, macOS 27 arm64.
Tức là trong suốt 6–7 phút cổng chạy, **~93% máy rảnh**.

Nguyên nhân cấu trúc: `v2/bin/v2` là một pipeline **tuần tự** ~53 bước; `cargo test` chạy
**từng binary test một** (cargo không song song hoá giữa các binary); các checker Python là
**một tiến trình đơn luồng** mỗi cái.

---

## 3. Đã làm gì (commit `226eae9a`, nhánh `task/kt242-perf`)

### 3.1 Nâng toolchain Rust 1.99.1 → 1.100.0

`rustup check` xác nhận **1.100.0 (0ef1bc386 2026-11-09) là bản stable mới nhất** tại thời điểm
này (nightly là 1.102.0-nightly). Đã cài đủ component + target bare-metal cho 1.100.0.

| Tệp | Thay đổi |
|---|---|
| `v2/rust-toolchain.toml` | `channel = "1.100.0"` |
| `v2/kernel/rust-toolchain.toml` | `channel = "1.100.0"` |
| `v2/Cargo.toml` | `rust-version = "1.100.0"` |
| `v2/tools/repro-build.sh` | `VIFAST_REPRO_TOOLCHAIN:-1.100.0` |
| `v2/tools/repro-build-windows.ps1` | `RUSTUP_TOOLCHAIN = "1.100.0"` |

**Hệ quả bắt buộc phải biết**: `v2/rust-toolchain.toml` nằm trong **build-inputs** (dấu vân tay
`71d32c46b1939fe3`, 234 mục). Đổi toolchain ⇒ **ảnh sản phẩm đổi sha256** ⇒ ảnh
`639ca6d1…` và bằng chứng **K6 6/6** trên ảnh đó **không còn dùng được** (D2-191 §4: đổi đầu
vào tạo ảnh thì phải build ảnh mới và đo lại K6). Đây là cái giá lớn nhất của việc nâng
toolchain và **cần ngài quyết** (xem §8, câu hỏi Q4).

### 3.2 Vá 19 lint mới của clippy 1.100 (`needless_range_loop`)

Đây đúng là lớp lỗi đã làm KT-190 phải **ghim** toolchain thay vì nâng. Nay đã vá tận gốc,
**không dùng `#[allow]`**, không đổi ngữ nghĩa:

| Tệp | Số lint | Cách vá |
|---|---:|---|
| `v2/lib/vifast-fs/src/dir.rs` | 4 | `for &lba in inode.blocks.iter().take(n)` |
| `v2/lib/vifast-fs/src/fsck.rs` | 4 | `iter_mut().take(end).skip(used)` — **giữ nguyên** hành vi khi `used > end` (không panic) |
| `v2/lib/vifast-fs/src/path.rs` | 2 | `for &lba in …` |
| `v2/lib/vifast-fs/src/tx.rs` | 2 | `for (index, &target) in self.targets.iter().take(count).enumerate()` |
| `v2/lib/vifast-fs/src/fs.rs` | 1 | `iter_mut().take(MAX_BLOCKS_PER_INODE).enumerate()` |
| `v2/lib/vifast-alloc/src/lib.rs` | 1 | `for &h in self.holes.iter().take(self.n_holes as usize)` |
| `v2/sim/vifast-sim/src/kobj.rs` | 2 | `hart_vspace.iter().enumerate()`, `published.iter().enumerate().take(n)` |
| `v2/sim/vifast-sim/tests/kt60_core_parity.rs` | 4 | `iter().take(n)` / `enumerate()` |

`cargo clippy --workspace --all-targets -- -D warnings` ⇒ **RC=0** trên 1.100.0.
`cargo fmt --all --check` ⇒ **sạch**.

### 3.3 Vá 2 chỗ giả định **bố cục artifact cũ** (phát hiện mới, quan trọng)

**cargo 1.100.0 đổi bố cục thư mục build**: binary test và tệp `.d` không còn ở
`<target>/debug/deps/` mà ở `<target>/debug/build/<pkg>/<hash>/out/`. Đã kiểm bằng crate tối giản:

```text
cargo 1.99.1: target/debug/deps/tiny-e0c7758806ad6493
cargo 1.100.0: target/debug/build/tiny/502a51ea0ce62bd7/out/tiny-502a51ea0ce62bd7
```

Hai chỗ trong repo giả định bố cục cũ, **đã vá**:

1. `v2/lib/vifast-objmem/tests/compile_fail.rs` — tìm rlib bằng `exe.parent()`. Nay tìm gốc
   `debug` (thư mục có `build/` hoặc `deps/`) rồi **quét đệ quy** ⇒ chạy đúng **cả hai** bố cục.
2. `v2/tools/check-wake-depinfo.py` — hard-code `debug/deps`. Nay thêm `depinfo_dir()` (ưu tiên
   `debug/deps` khi có, nếu không dùng `debug/build`) + `depinfo_glob()` (đệ quy khi là `build/`).
   Selftest **59/59 ok**, chạy thật `errors=0`.

Đã rà toàn bộ `v2/bin`, `v2/tools`, `v2/test`: **không còn chỗ nào** giả định `debug/deps`.

### 3.4 Tối ưu thời gian cổng

| Mã | Việc | Tệp |
|---|---|---|
| **P1** | `l0_test_parallel()`: build `--no-run --message-format=json`, lọc `profile.test`, chạy `xargs -P 6`, **cwd = thư mục gói** (như cargo), doc-test chạy riêng, binary nhạy thời gian (`kt30_stress`, `spsc`) chạy một mình | `v2/bin/v2` |
| **P2** | Tách `fuzz_smp_tat_dinh_bat_bien` thành 4 test `fuzz_smp_harts_{1,2,4,8}` (phủ **không đổi**: 1.000 seed × 64 op × 4 cấu hình, so trace 2 lượt/seed) | `v2/sim/vifast-sim/tests/kt29_smp.rs` |
| **P7** | `find -name target -prune` thay `-not -path '*/target/*'` (đã chứng minh **cùng tập tệp** trên 5 thư mục: kernel 171, lib 99, sim 71, test 96, tools 31) | `v2/bin/v2` |
| **P9** | In `# step-time <bước> <ms>` + bảng tổng cuối `check`/`gate` (tiền tố `#` nên bộ đọc log cũ bỏ qua) | `v2/bin/v2` |
| **P1b** | Tệp tạm `vifast_objmem_n9.o` thêm `std::process::id()` (hết đỏ oan khi chạy song song — đã tái hiện được) | `v2/lib/vifast-objmem/tests/compile_fail.rs` |

---

## 4. Số đo trước / sau

### 4.1 Tổng

| Lệnh | Trước `[ĐO]` | Sau `[ĐO]` | Cải thiện |
|---|---:|---:|---:|
| `gate --fast` | **1.032 s** (17m12s, `a2e18cdf`, 11:20 hôm nay) | **629,8 s** (10m30s, `226eae9a`) | **−39%** |
| `L0 test` | ~290 s (92 binary workspace chính + 19 binary workspace kernel; log cổng có 130 dòng `test result`, tổng 256,5 s test + ~33 s khởi chạy) | **76,1 s** (92 binary workspace chính, 0 đỏ) + 6,3 s kernel | **−74%** |
| `check` | ~206,7 s (đo độc lập trên `47ee7b92`) | ~190 s | −8% (mới chỉ `rules`) |

### 4.2 Cổng sau khi sửa — từng bước (tổng 578,5 s đo được)

| Bước | Thời gian | % |
|---|---:|---:|
| `selftest runner ktest` (build 7 ảnh release + QEMU) | **119,2 s** | 21% |
| `selfbreak` (6 lượt `verify-numbers` tuần tự) | **114,8 s** | 20% |
| `L0 test` (đã song song) | **76,1 s** | 13% |
| `selftest wake paths` | 37,5 s | 6% |
| `wake paths` | 25,3 s | 4% |
| `wake paths --strict` | 24,8 s | 4% |
| `selftest qemu guard` | 22,4 s | 4% |
| `cắt điện giữa lúc ghi` (QEMU) | 19,4 s | 3% |
| `selftest build-inputs` | 18,7 s | 3% |
| `luật máy K6` | 18,2 s | 3% |
| `wake theo dep-info` | 15,3 s | 3% |
| `ca âm git-guard song song` | 13,8 s | 2% |
| `ca âm git-guard` | 12,5 s | 2% |
| `selftest persist đỏ-trước` | 7,6 s | 1% |
| `selftest pre-measure` | 6,3 s | 1% |
| `L0 kernel workspace` | 6,3 s | 1% |
| `selftest pin-artifact` | 5,1 s | 1% |
| `clippy` | 4,7 s | 1% |
| `selftest shell-edge` | 4,5 s | 1% |
| `api boundary` | 4,1 s | 1% |
| `algo= nghiêm ngặt` | 3,7 s | 1% |
| `fmt` | 3,3 s | 1% |
| còn lại (~30 bước nhỏ) | ~40 s | 7% |

### 4.3 L0 chi tiết

```text
# L0: biên dịch xong sau 10.607 ms
# L0: 92 binary, 0 đỏ, 72.767 ms (jobs=6, threads=auto)
```

Trước P1: `cargo test --workspace` chạy **tuần tự từng binary** (92 binary workspace chính +
19 binary workspace kernel), ~290 s. Sau: 92 binary workspace chính chạy **6 luồng** = 76,1 s,
cộng bước kernel riêng 6,3 s. Số binary **không đổi** — P2 chỉ tách 1 test thành 4 test trong
**cùng** một binary.

### 4.4 Trạng thái cổng trên worktree

**52/53 bước OK**. Bước đỏ: `ca âm git-guard b (KT-85b)` — **không phải hồi quy**: test này
clone repo rồi đòi `refs/heads/main`, nhưng worktree đang đứng trên nhánh `task/kt242-perf`
nên clone không có `main` (`fatal: ambiguous argument 'refs/heads/main'`). Trên `main` bước này
xanh (đã xanh trong log `kt239-gate-pass.log`).

---

## 5. Thí nghiệm nextest (người dùng hỏi)

Đã cài `cargo-nextest 0.9.152` và đo trên **cùng một cây** (worktree, 1.100.0):

| Cách chạy | Kết quả | Thời gian chạy |
|---|---|---:|
| `cargo nextest run --workspace` | 789 test, 788 pass, 1 fail (bố cục artifact) | **67,1 s** |
| `xargs -P 6` trên danh sách binary (cách của P1) | 92 binary | **53,9 s** |

**Kết luận: nextest KHÔNG nhanh hơn** với workload này. Lý do: các test ở đây là **CPU-bound**
và critical path là **test dài nhất** (`kt11_diff_sim` 43,6 s một test), không phải lịch trình.
nextest có lợi thế khác (timeout từng test, output JUnit) nhưng **không mua được tốc độ**, lại
thêm một công cụ phải ghim phiên bản và **không chạy doc-test** (19 suite phải chạy riêng).
⇒ **Đề xuất: giữ cách không phụ thuộc (xargs -P), không thêm nextest vào cổng.**

---

## 6. Dọn dữ liệu rác (đã làm trên cây chung — không đụng mã)

| Thư mục | Tệp `.o` rác đã xoá |
|---|---:|
| `v2/target/debug/deps` | 397.851 |
| `target/debug/deps` (v1) | 213.811 |
| `v2/kernel/target/debug/deps` | 18.488 |
| **Tổng** | **630.150** |

Đây là các tệp `*.rcgu.o` còn lại của những lần build bị kill giữa chừng (ngày 13–16/11), cargo
không tham chiếu tới. Hệ quả đo được: `find` toàn cây **4,82 s → ~0,1 s**; `v2/target` từ
442.781 tệp còn ~45.000. Đã thêm `.metadata_never_index` vào `target/`, `v2/target/`,
`v2/kernel/target/` để Spotlight không index 442k tệp.

**Không xoá gì khác**: không `cargo clean`, không đụng `incremental/`, không đụng mã nguồn.

---

## 7. Phân tích báo cáo "macOS spawn chậm hơn Linux" (người dùng nhờ phân tích)

Một agent khác đã đo và kết luận: spawn trên macOS chậm 3–7× so với Linux, nhưng chỉ chiếm
6–9% của `check`, nên chuyển Linux chỉ cứu vài giây.

**Đồng ý** với số đo và với kết luận "đừng chuyển OS":

- `rules` của tôi đo **10,5 s**, của họ **10,6 s** — khớp.
- Spawn chỉ chiếm ~6–9% của `check` — hợp lý.

**Nhưng cần bổ sung ba điểm**, vì nếu chỉ đọc báo cáo đó sẽ kết luận sai là "không cải thiện được":

1. **141 s checker Python KHÔNG phải spawn-bound.** Tôi profile `check-wake-paths.py`
   (`cProfile`): **677 tệp**, **5,7 triệu** lần `re.Pattern.match`, **11,7 triệu** lần
   `list.append`, **15,2 triệu** lần `isinstance`, **0 lời gọi `subprocess`**. Đây là một Rust
   tokenizer viết bằng Python, chạy **3 lượt** trên cùng một cây. Chuyển sang Linux **không
   nhanh hơn một giây nào** ở phần này.
2. **Báo cáo đó chỉ phân tích `check` (~3,5 phút), chưa phân tích `gate --fast` (10,5 phút).**
   Phần lớn thời gian nằm ở `ktest` (119 s), `selfbreak` (115 s), `L0` (76 s) — không phải checker.
3. **Vấn đề không phải OS mà là tuần tự hoá.** Cùng máy 12 nhân, chỉ cần dùng nhiều hơn 1 nhân
   là đã hơn 10 lần.

| Phương án | Tiết kiệm `check` | Tiết kiệm `gate` |
|---|---:|---:|
| Chuyển sang Linux | ~10–13 s (5–6%) | ~10–13 s (2%) |
| Song song hoá + gộp lượt (A–D dưới) | ~140 s (68%) | ~320 s (51%) |

---

## 8. Đề xuất cải thiện tiếp (cần ngài quyết)

| # | Việc | Tiết kiệm `[ƯL]` | Mất phủ | Rủi ro | Cần duyệt |
|---|---|---:|---|---|---|
| **A** | **Gộp 3 lượt wake-paths**: tokenize **một lần**, đánh giá 3 chế độ (thường / `--strict` / selftest) | **−58 s** | KHÔNG | thấp — cùng phép kiểm, cùng ca | Agent 0 (đổi cơ chế checker) |
| **B** | **`selfbreak` chạy 5 luật song song** thay vì 6 lượt `verify-numbers` tuần tự (mỗi luật đã có thư mục `case-<id>` riêng) | **−90 s** | KHÔNG | thấp | Agent 0 |
| **C** | **Chia cổng thành 3 làn song song** cho các bước độc lập (checker, selftest không-QEMU); giữ tuần tự cho QEMU/persist/git-guard/rev-gate | **−150 s** | KHÔNG | trung bình — thứ tự log đổi, phải sửa `/tmp/repfx.out`, `/tmp/scn-fix-*` sang thư mục riêng trước | Agent 0 |
| **D** | **L0: `jobs` 6 → 10**; tách `kt11_diff_sim` (43,6 s một test) thành nhiều test | **−25 s** | KHÔNG | thấp | Agent 0 (D là sửa test) |
| **E** | **Ảnh TEST dùng profile nhanh hơn** (`opt-level=1`, `lto=false`, `codegen-units=16`) thay vì profile sản phẩm | **−60 s** | KHÔNG | **CAO** — đổi kích thước ảnh, đụng ngưỡng `data+bss ≤ 262144`; **không** áp cho ảnh sản phẩm | Agent 0 |
| **P3** | Song song hoá nhóm checker không-cargo trong `check` | `check` 206 → ~70 s | KHÔNG | thấp | Agent 0 |

**Tổng A+B+C+D: 630 s → ~300 s (5 phút).** Thêm P3: `check` ~70 s.

### Vì sao A và B là hai việc nên làm trước

- **A**: ba lượt wake-paths (25,3 + 24,8 + 37,5 = **87,6 s**) chạy **cùng một bộ tokenizer** trên
  **cùng một cây**. Tokenize một lần rồi đánh giá ba chế độ là **cùng phép kiểm**, không bỏ ca nào.
- **B**: `selfbreak` (114,8 s) = 6 lượt `verify-numbers`, mỗi lượt ~19 s, trong khi 5 luật
  **độc lập** và mỗi luật đã có thư mục riêng. Chạy song song 5 luồng ⇒ ~25 s.

---

## 9. Câu hỏi cần ngài quyết

| # | Câu hỏi | Phương án |
|---|---|---|
| **Q1** | Có merge nhánh `task/kt242-perf` (commit `226eae9a`) vào `main` không? | (a) merge cả gói (toolchain + perf); (b) **tách**: merge P1/P2/P7/P9 trước, toolchain sau; (c) chưa merge, chờ đo lại |
| **Q2** | Có duyệt A/B/C/D để làm tiếp không? | (a) duyệt A+B ngay (dễ, −148 s); (b) duyệt cả A–D; (c) chỉ A |
| **Q3** | Có duyệt **E** (profile ảnh test) không? | (a) có, kèm đo lại ngưỡng `data+bss`; (b) không, giữ profile sản phẩm cho ảnh test |
| **Q4** | **Nâng toolchain làm đổi sha ảnh sản phẩm** ⇒ K6 phải đo lại (D2-191). Thứ tự nào? | (a) nâng toolchain → build ảnh mới → đo lại K6 **trước** khi merge; (b) merge trước, đo K6 sau; (c) **hoãn** nâng toolchain, giữ 1.99.1 cho ảnh sản phẩm, chỉ dùng 1.100.0 cho dev |
| **Q5** | Có giữ việc dọn 630k tệp `.o` (đã làm trên cây chung) không? | Đã làm rồi; nếu ngài muốn hoàn tác thì chỉ cần build lại (cargo tự sinh lại cái cần) |

**Khuyến nghị của tôi**: **Q1 = (b) tách** — phần perf (P1/P2/P7/P9) không đổi ảnh sản phẩm,
merge được ngay; phần toolchain để riêng vì nó kéo theo K6. **Q2 = (b)** duyệt cả A–D.
**Q3 = (b)** chưa duyệt E. **Q4 = (a)** đo lại K6 trước khi merge toolchain.

---

## 10. Bằng chứng (tệp và lệnh)

| Tệp | Nội dung |
|---|---|
| `/tmp/kt242-perf` | worktree nhánh `task/kt242-perf`, commit `226eae9a` |
| `/Users/mps/dvchd/kt242-perf/kt242-perf.patch` | bản vá 340 dòng (bản trước khi commit) |
| `/Users/mps/dvchd/kt242-perf/TRANG-THAI.md` | trạng thái + số đo trung gian |
| `/Users/mps/dvchd/kt242-perf/do-check-khong-cargo.log` | thời gian từng bước không-cargo của `check` |
| `/Users/mps/dvchd/kt242-perf/do-gate-selftest.log` | thời gian từng selftest chỉ-có-trong-gate |
| `/tmp/kt242-gate-1100b.log` | **log cổng đầy đủ** trên `226eae9a` (52/53 OK, có `# step-time`) |
| `/tmp/kt242-clippy-1100d.log` | clippy 1.100.0 `-D warnings` ⇒ RC=0 |
| `/tmp/kt242-nextest-1.log` | thí nghiệm nextest (789 test, 67,1 s) |
| `v2/agent/JOURNAL/kt110-perf.md` | phân tích KT-110 (18/11) — 9 đề xuất, **chưa có task nào thi hành** |

Lệnh tái lập:

```bash
# đo cổng có thời gian từng bước
cd /tmp/kt242-perf && ./v2/bin/v2 gate --fast 2>&1 | tee /tmp/gate.log
grep '^# step-time ' /tmp/gate.log | sort -k3 -rn | head -20

# đo L0 riêng (không chạy cả cổng)
V2_TEST_JOBS=6 ./v2/bin/v2 gate --fast   # xem dòng "# L0: ..."

# kiểm toolchain
rustc +1.100.0 -V && cargo +1.100.0 clippy --version
```

---

## 11. Điều **không** làm (đúng ràng buộc)

- **Không** sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOS-v2-thiet-ke.md`,
  `docs/v1/`.
- **Không** nới ngưỡng nào (`MAX_IO`, `MAX_TCBS`, `MAX_OBJECTS`, `KERNEL_STATIC_BUDGET`,
  `data+bss ≤ 262144`, seed L1).
- **Không** thêm `#[ignore]`, `#[should_panic]`, không tắt test, không bỏ ca.
- **Không** dùng `#[allow(clippy::…)]` để né lint — 19 lint được vá tận gốc.
- **Không** thêm crate phụ thuộc; **không** đưa nextest vào cổng.
- **Không** commit/push lên `main`; mọi thay đổi nằm trên nhánh `task/kt242-perf`.
- **Không** đụng cây chung ngoài việc dọn artifact (§6).
