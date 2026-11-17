# KT-110 — đo và cắt thời gian cổng (`check` / `gate --fast`)

**Ngày**: 2026-11-18 (giờ máy Asia/Tokyo) · **Người làm**: Agent 2 dòng OS · **Ngân sách**: 2 h
**HEAD lúc đo**: `19b496e1` (không đổi suốt phiên) · **Toolchain**: cargo/rustc `1.99.1`
**Phạm vi ghi**: chỉ `v2/agent/JOURNAL/kt110-*`. Không sửa mã, không commit, không `git add`.

Nhãn số liệu: mọi số dưới đây là **`[ĐO KT-110]`** — đo trực tiếp bằng lệnh ghi kèm, không phải
số chép tay từ tài liệu, không phải số do `nightly` sinh. Log thô đã lưu cạnh tệp này
(`kt110-*.log`); ai muốn trích số vào **báo cáo** (`v2/agent/reports/`) thì phải commit log trước
(luật 2 của KT-80).

---

## 0. TL;DR

| Câu hỏi | Trả lời `[ĐO KT-110]` |
|---|---|
| `check` mất bao lâu? | **49,4 s** và **55,6 s** (cache ấm); **77,2 s** khi `CARGO_TARGET_DIR` nguội |
| `gate --fast` mất bao lâu? | **407,2 s** và **371,0 s** |
| Nút cổ chai ở đâu? | **L0 test = 354,8 s / 317,3 s — 85,5–87,1 % của gate** |
| L1 (1.000 seed) có vượt 60 s? | **Không.** **0,92 s / 1,07 s** (cổng tự đo 753 / 899 ms). Ngưỡng §6 còn thừa ~59 s |
| Ba bước đắt nhất | L0 test **336 s** (TB) · luật máy K6 **17,0 s** · fixture báo cáo **14,5 s** |
| Ba đề xuất an toàn nhất | Song song binary L0 (**~250–285 s**) · tách `kt29_smp` (**~47 s**) · song song phép kiểm không-cargo (**~28 s**) |

**Kết luận quan trọng nhất**: mục tiêu "L1 ≤ 60 s" của `AGENTS.md` §6 **đã đạt từ lâu** (L1 < 1,1 s).
Thứ làm mỗi vòng lặp task chậm **không phải L1** mà là **L0 `cargo test --workspace`** — 100 binary
test chạy **lần lượt**, trong đó một binary (`kt29_smp`) chiếm 63–66 s.

---

## 1. Cách đo và trạng thái máy

- Bọc mỗi lệnh cổng bằng một runner gắn mốc thời gian **từng dòng** (`perl -MTime::HiRes`), nên
  thời gian từng bước là hiệu giữa dòng `--- <bước>` và dòng `--- <bước>: OK` — **không** thêm
  tiến trình `time` vào giữa các bước (không làm nhiễu số đo). Tổng kiểm chéo bằng `/usr/bin/time -p`.
- Mỗi lệnh chạy **hai lần** liên tiếp; thêm **một** lần `check` với `CARGO_TARGET_DIR` nguội
  (`/tmp/kt110/target-cold`) để tách "ấm/nguội".
- Máy: **12 core** (`hw.ncpu=12`), macOS. **Máy có việc khác chạy song song**: một agent đang
  build/test repo **ViScript** (`/Users/mps/dvchd/ViScript/target/...`) — **repo khác, target dir
  khác** nên **không** lẫn cache, chỉ tranh CPU. Vì vậy **không** kết luận từ một lần đo; mọi số
  dưới đây là cặp hai lần.

| Lần chạy | loadavg đầu | loadavg cuối | tổng |
|---|---|---|---|
| `check` lần 1 | 6,47 | 5,08 | **49,41 s** |
| `check` lần 2 | 4,99 | 8,14 | **55,59 s** |
| `check` nguội (`CARGO_TARGET_DIR=/tmp`) | 9,59 | 9,77 | **77,16 s** |
| `gate --fast` lần 1 | 7,37 | 6,24 | **407,19 s** |
| `gate --fast` lần 2 | 4,95 | 5,10 | **370,98 s** |

> Lưu ý về con số **107 s** của reviewer: `check` ấm ở đây chỉ **49–56 s**. Chênh lệch khớp với
> phần **nguội**: clippy 0,58 s → **12,69 s**, build 0,36 s → **8,19 s**, fmt 3,01 s → **10,75 s**.
> Một lần `check` nguội hoàn toàn (chưa có page cache của rustc) + máy bận cho ra 107 s là hợp lý.
> `check-rules` thì khớp: reviewer ~20 s, đo được **16,6–17,4 s**.

---

## 2. `./v2/bin/v2 check` — từng bước, hai lần đo

| # | Bước | Lần 1 (s) | Lần 2 (s) | Nguội (s) | % của check ấm |
|---:|---|---:|---:|---:|---:|
| 1 | `fmt` (`cargo fmt --all --check`) | 3,01 | 3,03 | 10,75 | 5,8 % |
| 2 | `clippy` (`--workspace --all-targets -D warnings`) | 0,58 | 0,75 | 12,69 | 1,3 % |
| 3 | luật cấu trúc R1..R5 (bash) | 9,12 | 9,91 | 9,16 | 18,1 % |
| 4 | self-test luật (6 ca TAP) | 0,74 | 0,71 | 0,75 | 1,4 % |
| 5 | **luật máy K6** (law 1,2,3,7,8) | **16,74** | **17,35** | 16,61 | **32,5 %** |
| 6 | abi spec | 0,22 | 0,17 | 0,36 | 0,4 % |
| 7 | golden trace | 0,11 | 0,10 | 0,09 | 0,2 % |
| 8 | kịch bản `.scn` | 0,12 | 0,15 | 0,13 | 0,3 % |
| 9 | scn fixtures (5 ca âm) | 0,70 | 0,70 | 0,86 | 1,3 % |
| 10 | báo cáo R1-v2 | 2,14 | 2,29 | 2,14 | 4,2 % |
| 11 | **fixture âm của máy kiểm báo cáo** (6 ca) | **13,23** | **15,84** | 13,11 | **27,7 %** |
| 12 | abi reserved | 0,12 | 0,18 | 0,11 | 0,3 % |
| 13 | abi fixtures (3 ca) | 0,66 | 1,11 | 0,58 | 1,7 % |
| 14 | gen-check | 0,14 | 0,29 | 0,16 | 0,4 % |
| 15 | gen fixtures (4 ca) | 0,84 | 1,45 | 0,86 | 2,2 % |
| 16 | idl-gen-check | 0,14 | 0,21 | 0,13 | 0,3 % |
| 17 | idl fixtures (2 ca) | 0,40 | 0,65 | 0,39 | 1,0 % |
| 18 | build (L0 compile) | 0,36 | 0,61 | 8,19 | 0,9 % |
| | **tổng (cộng các bước)** | **49,34** | **55,51** | **77,06** | 100 % |
| | **tổng `/usr/bin/time -p`** | **49,41** | **55,59** | **77,16** | |

### Biểu đồ phần trăm — `check` ấm (TB hai lần)

```text
luật máy K6 (law 1,2,3,7,8)  ████████████████        32,5 %
fixture báo cáo (6 ca)       ██████████████          27,7 %
luật cấu trúc R1..R5         █████████               18,1 %
fmt                          ███                      5,8 %
báo cáo R1-v2                ██                       4,2 %
gen fixtures                 █                        2,2 %
abi fixtures                 █                        1,7 %
self-test luật               █                        1,4 %
scn fixtures                 █                        1,3 %
clippy                       █                        1,3 %
idl fixtures                 ▏                        1,0 %
build (L0 compile)           ▏                        0,9 %
abi spec · golden · scn · abi reserved · gen-check · idl-gen-check   ~2,0 % (còn lại)
```

**Đọc bảng**: 78 % thời gian `check` nằm ở **ba bước không liên quan tới Rust** (K6 32,5 % +
fixture báo cáo 27,7 % + luật cấu trúc 18,1 %). `clippy` + `build` chỉ **2,2 %** khi cache ấm.

---

## 3. `./v2/bin/v2 gate --fast` — tách L1 và phần còn lại

`gate --fast` = `check` (đầy đủ) → `cargo test --workspace` (**L0 test**) → `cargo test -p vifast-sim l1_` (**L1**).

| Phần | Lần 1 (s) | Lần 2 (s) | % lần 1 | % lần 2 |
|---|---:|---:|---:|---:|
| `check` (18 bước ở §2) | 51,42 | 52,57 | 12,6 % | 14,2 % |
| **L0 test** (`cargo test --workspace`) | **354,78** | **317,27** | **87,1 %** | **85,5 %** |
| **L1** (1.000 seed, lọc `l1_`) | **0,92** | **1,07** | **0,2 %** | **0,3 %** |
| **tổng** | **407,19** | **370,98** | 100 % | 100 % |

Cổng tự đo L1: `--- L1: OK (753 ms)` và `--- L1: OK (899 ms)`.

### Biểu đồ phần trăm — `gate --fast`

```text
L0 test  ███████████████████████████████████████████ 86,3 %
check    ███████                                     13,4 %
L1       ▏                                            0,3 %
```

**L1 không phải vấn đề.** 1.000 seed × 64 bước × 2 harness chỉ tốn **0,75–0,90 s** — cách ngưỡng
60 s của `AGENTS.md` §6 gần **60 lần**. Mọi đề xuất "giảm seed `--fast`" là **không cần thiết** và
sai hướng.

---

## 4. Bên trong L0 test — 100 binary, chạy **lần lượt**

`cargo test --workspace` chạy **tuần tự từng binary** (cargo không song song hoá giữa các binary).
Cộng thời gian cargo tự báo (`test result: ... finished in X.XXs`):

| Hạng | Binary | Lần 1 (s) | Lần 2 (s) | % của L0 (TB) |
|---:|---|---:|---:|---:|
| 1 | `tests/kt29_smp.rs` (13 test) | **63,25** | **66,44** | **19,3 %** |
| 2 | `tests/l1_k1c.rs` (1 test: `driver_smp_cung_seed_cung_trace`) | 28,23 | 29,06 | 8,5 % |
| 3 | `tests/kt11_diff_sim.rs` (1 test, crate `vifast-model`) | 26,59 | 18,19 | 6,7 % |
| 4 | `tests/kt23_k1b.rs` (9) | 15,23 | 20,61 | 5,3 % |
| 5 | `tests/kt39_wx.rs` (8) | 22,54 | 11,73 | 5,1 % |
| 6 | `tests/kt26_fault.rs` (24) | 15,18 | 13,97 | 4,3 % |
| 7 | `tests/kt24_ipc.rs` (13) | 10,15 | 17,78 | 4,2 % |
| 8 | `tests/kt25_ipc2.rs` (29) | 12,64 | 14,13 | 4,0 % |
| 9 | `tests/kt38_vspace.rs` (7) | 15,81 | 9,90 | 3,8 % |
| 10 | `tests/kt41_vmfault.rs` (9) | 10,60 | 11,37 | 3,3 % |
| 11 | `tests/kt36_frame.rs` (12) | 15,63 | 6,10 | 3,2 % |
| 12 | `tests/kt19_sched.rs` (10) | 10,94 | 10,75 | 3,2 % |
| 13 | `tests/kt27_notification.rs` (27) | 8,47 | 8,12 | 2,5 % |
| 14 | `tests/kt22_mailbox.rs` (10) | 7,81 | 7,62 | 2,3 % |
| 15 | `tests/kt21_donation.rs` (13) | 7,90 | 7,50 | 2,3 % |
| 16 | `tests/kt20_budget.rs` (12) | 6,96 | 7,05 | 2,1 % |
| 17 | `tests/kt37_pagetable.rs` (10) | 9,53 | 4,39 | 2,1 % |
| 18 | `tests/kt28_waitaddr.rs` (13) | 6,67 | 6,72 | 2,0 % |
| | 82 binary còn lại (cộng lại) | 21,4 | 20,0 | ~6 % |
| | **cộng thời gian test** | **315,5** | **290,9** | |
| | **hao phí khởi chạy** (L0 wall − cộng test) | **39,3** | **26,4** | **~10 %** |

Hai điều đáng chú ý:

1. **`kt29_smp` = 63–66 s cho một binary.** Đó là fuzz `1.000 seed × 64 op × smp ∈ {1,2,4,8}`
   (`so_seed()`/`so_op()` trong `v2/sim/vifast-sim/tests/kt29_smp.rs:716-731`) — **một** hàm test
   tự lặp qua 4 giá trị smp, nên harness **không** có gì để chạy song song.
2. **~26–39 s chỉ để khởi chạy 100 binary** (mỗi binary ~0,3 s tiến trình + cargo chờ). Con số này
   biến mất nếu các binary được chạy song song.

---

## 5. Chỗ đắt **vô ích** (câu hỏi 2)

### 5.1 Bước chạy **lại** việc bước trước đã làm

| # | Việc trùng | Bằng chứng `[ĐO KT-110]` | Giá |
|---|---|---|---|
| A | `l1_1000_seed_tat_dinh` chạy **hai lần** trong **một** lần `gate --fast` | L0 chạy nó (`kt110-gate-r1.log:1209`, `finished in 0.34s`), rồi bước L1 lọc `l1_` chạy **lại đúng test đó** (0,92 s) | ~1,2 s |
| B | `driver_smp_cung_seed_cung_trace` (`l1_k1c`) **tưởng là không chạy trong gate** nhưng vẫn chạy | Chú thích tại `v2/sim/vifast-sim/tests/l1_k1c.rs:834-836` nói tên **cố ý không** bắt đầu bằng `l1_` để bộ lọc `l1_` của gate không chạy nó. Đúng — nhưng **L0 `cargo test --workspace` không lọc gì**, nên nó vẫn chạy với mặc định `1.000 seed × smp 1 × 2 lượt/seed` (`so_seed_hai_lan`, dòng 113-127). **ĐÍNH CHÍNH (review chéo)**: bản trước ghi `smp 1,2,4,8` — sai; `VIFAST_SMP` mặc định là `1` (`l1_k1c.rs:91`), còn `1,2,4,8` là mặc định của `v2 nightly --l1` (`v2/bin/v2:469`). Đo lại: mặc định **27,92 s**, `VIFAST_SMP=1,2,4,8` là **151,38 s** | **28,2–29,1 s** |
| C | `cargo build --workspace` (bước 18) là **tập con** của `cargo test --workspace` | Sau khi `cargo test` xong, `cargo build --workspace` chỉ mất **0,29–0,68 s** ("Finished in 0.29s") ⇒ test đã build xong lib+bin. Khi nguội, `build` một mình tốn **8,19 s** | 0,4–0,7 s (ấm) / **8,2 s** (nguội) |
| D | 6 fixture báo cáo **chạy lại toàn bộ** `check_report.py` 6 lần | `v2/test/report/run_fixtures.sh` gọi `check_report.py` 6 lần, mỗi lần đọc lại `QUEUE.md` (113 hàng) + git | 13,2–15,8 s |
| E | 25 subprocess Python cho 21 ca phá của luật 8 | `verify-numbers.py:1358` `exit_code_of()` spawn `sys.executable` **mỗi ca**; log cổng ghi `law_wall_ms={8:12474}` | 12,5 s |

### 5.2 Bước **build lại từ đầu** thay vì dùng cache

- **Không có bước nào build lại từ đầu khi cache ấm**: `clippy` 0,58–0,75 s, `build` 0,36–0,68 s —
  đều in "Finished ... in 0.3s", tức cargo thấy artifact còn hạn.
- **Khi nguội** (đo bằng `CARGO_TARGET_DIR=/tmp/kt110/target-cold`): `clippy` **12,69 s**,
  `build` **8,19 s**, `fmt` **10,75 s** ⇒ `check` 49,4 s → **77,2 s**. Đây là lý do con số 107 s
  của reviewer cao hơn: mỗi vòng task sửa mã ⇒ clippy/build phải làm lại phần bị đổi.
- **`fmt` 3,0 s (ấm) / 10,8 s (nguội)** cho 17 crate: rustfmt spawn theo crate, không có cache dùng
  chung. Không có cách "dùng cache" cho `--check`; chỉ có thể chạy song song theo crate.

### 5.3 `CARGO_TARGET_DIR` — có chỗ nào **không** dùng chung không?

- **Trong đường cổng: KHÔNG.** Cả 5 lệnh cargo (`fmt`, `clippy`, `build`, `test --workspace`,
  `test -p vifast-sim`) đều dùng `--manifest-path v2/Cargo.toml` và **không** đặt
  `CARGO_TARGET_DIR` ⇒ cùng một target dir `v2/target` (5,0 GB). `.cargo/config.toml` ở gốc repo
  chỉ đặt `RUST_MIN_STACK`, **không** đặt `target-dir`.
- **Có một cây target thứ hai `v2/kernel/target` (1,2 GB)** vì `v2/kernel` là **workspace riêng**
  (`exclude = ["kernel"]` trong `v2/Cargo.toml`). Cổng **không** build kernel nên **không** dùng
  cây này — nhưng bước luật cấu trúc vẫn **đi qua** nó (cùng `v2/target`) khi `find`.
- **Hệ quả phụ (không phải lỗi hiệu năng, là lỗ hổng phủ)**: vì `kernel` bị `exclude`, `cargo fmt
  --all` và `cargo clippy --workspace` **không** chạm `v2/kernel/**`. Ghi nhận, **không** đề xuất
  thêm kiểm (thêm kiểm = chậm hơn, ngoài phạm vi task này).

### 5.4 Vì sao luật cấu trúc tốn 9–10 s (không phải vì `target/`)

Giả thuyết ban đầu "find đi qua 404.196 tệp trong `v2/target`" **sai**. Đo tách:

| Thành phần | Thời gian `[ĐO KT-110]` |
|---|---:|
| `find` đi qua **toàn bộ** cây (kể cả `v2/target` + `v2/kernel/target`), có `-not -path '*/target/*'` | **0,34 s** |
| cùng `find` nhưng `-name target -prune` | 0,03 s |
| **R1** (`wc -l` **mỗi tệp** trong `kernel/`+`lib/`) | **2,58 s** |
| **R3** (`head -5` **+** `grep` **mỗi tệp** = 2 tiến trình/tệp, 356 tệp) | **3,09 s** |
| **R5** (`grep` **mỗi tệp** `.rs`) | **1,90 s** |
| R2 + R4 (một `grep -rn`, vài `Cargo.toml`) | ~0,2 s |
| **bản gộp R3 một lượt `grep`** (cùng vị từ) | **0,51 s** |

⇒ 9,1 s là **~7,6 s tiền spawn tiến trình cho từng tệp**, không phải I/O cây `target/`.

---

## 6. Đề xuất cắt thời gian (xếp theo tiết kiệm, **không** bỏ phép kiểm nào)

Quy ước cột: **(a)** tiết kiệm ước lượng `[ƯL]` từ số đo §2–§5 · **(b)** rủi ro **mất phủ** ·
**(c)** có **đổi ngữ nghĩa cổng** không (nếu **có** ⇒ **bắt buộc Agent 0 duyệt**).

### P1 — Chạy **song song các binary test** của L0 (thay vì cargo chạy lần lượt)

- **(a) Tiết kiệm `[ƯL]`: ~250–285 s mỗi `gate`.** L0 317–355 s → **~65–75 s** (chặn dưới là
  `kt29_smp` 63–66 s). `gate --fast` 407 s → **~150 s**.
  *Bằng chứng*: 6 binary vừa (tổng tuần tự **46,2 s**) chạy song song còn **12,8 s** (3,6×, chỉ 6
  luồng trên 12 core). Cộng thời gian test = 315,5 s / 290,9 s; CPU `user` của cả gate = 277 s ⇒
  trần lý thuyết ~26 s nếu 12 luồng.
- **(b) Mất phủ: KHÔNG.** Cùng binary, cùng test, cùng assert, cùng seed. Rủi ro *khác* (không
  phải mất phủ) là **đua trên đường dẫn tạm dùng chung**. Đã rà: **không** tệp test nào trong
  `v2/sim/vifast-sim/tests/*.rs` và `v2/lib/*/tests/*.rs` ghi `/tmp` cố định; chỉ
  `v2/lib/vifast-objmem/tests/compile_fail.rs` dùng temp dir. Vẫn phải rà lại khi làm thật.
- **(c) Đổi ngữ nghĩa cổng: CÓ (mức cơ chế)** — thứ tự/đồng thời, định dạng log, cách gộp lỗi
  thay đổi ⇒ **Agent 0 duyệt**. Không đổi *cái gì* được kiểm.
- *Ghi chú công cụ*: `cargo-nextest` **chưa cài** trên máy (`command -v cargo-nextest` → không có).
  Cách không thêm phụ thuộc: `cargo test --no-run --message-format=json` lấy đường dẫn binary rồi
  chạy bằng `xargs -P <N>` (BSD `xargs` có sẵn). Cách này **không** cần crate mới (D2-21).

### P2 — Tách fuzz `kt29_smp` thành **4 test theo smp** (1 / 2 / 4 / 8)

- **(a) Tiết kiệm `[ƯL]`: ~47 s mỗi `gate`.** 63–66 s → **~17 s**. Đây là binary dài nhất, nên
  **sau P1 nó thành critical path** — P1 mà không P2 thì L0 không xuống dưới ~65 s.
- **(b) Mất phủ: KHÔNG.** Vẫn `1.000 seed × 64 op × 4 giá trị smp`, vẫn kiểm `check_sched`/
  `check_harts`/`check_ipc` sau **mỗi** bước, vẫn "cùng seed ⇒ cùng trace". Chỉ đổi cách chia
  việc giữa các luồng của harness.
- **(c) Đổi ngữ nghĩa cổng: KHÔNG** (vẫn là L0, cùng ngưỡng). Nhưng là **sửa mã test** ⇒ cần task
  riêng + Agent 0 (Agent 2 không tự sửa mã trong task đo này).

### P3 — Chạy **song song nhóm phép kiểm không-cargo** trong `check`

Nhóm này **độc lập, chỉ đọc**: luật cấu trúc, self-test luật, K6, abi spec/reserved/fixtures,
golden, scn + fixtures, báo cáo + fixtures, gen-check + fixtures, idl-gen-check + fixtures.

- **(a) Tiết kiệm `[ƯL]`: ~28 s mỗi `check` và mỗi `gate`.** Nhóm = **~48 s**; chạy song song còn
  **~18–20 s** (chặn bởi K6 17 s). `check` 52 s → **~24 s**.
- **(b) Mất phủ: KHÔNG.** Cùng lệnh, cùng mã thoát, cùng TAP. **Rủi ro bắt buộc phải vá trước**:
  `v2/test/report/run_fixtures.sh:11` ghi **`/tmp/repfx.out` cố định** và
  `v2/test/scn/run_fixtures.sh:15` ghi `/tmp/scn-fix-$dir.log` ⇒ song song hoá mà không đổi sang
  thư mục tạm riêng thì **hai ca ghi đè nhau** (có thể làm cổng xanh giả). Đây là điều kiện tiên
  quyết, không phải tuỳ chọn.
- **(c) Đổi ngữ nghĩa cổng: CÓ (mức cơ chế)** ⇒ **Agent 0 duyệt**.

### P4 — Bỏ `cargo build --workspace` khỏi **đường `gate --fast`** (giữ nguyên trong `check`)

- **(a) Tiết kiệm `[ƯL]`: 0,4–0,7 s khi ấm; ~8 s khi nguội** (đo được 8,19 s).
- **(b) Mất phủ: KHÔNG.** `cargo test --workspace` biên dịch lib + bin rồi mới chạy; `build` là
  tập con thực sự (bằng chứng: sau `test`, `build` chỉ còn 0,29–0,68 s).
- **(c) Đổi ngữ nghĩa cổng: KHÔNG đổi cái được kiểm**, nhưng đụng thứ tự cổng ⇒ nên **Agent 0**
  xác nhận. `check` chạy **một mình** vẫn phải giữ `build` (nó không có bước test).

### P5 — Song song hoá 25 subprocess của **luật 8** trong `verify-numbers.py`

- **(a) Tiết kiệm `[ƯL]`: ~9 s mỗi `check`/`gate`.** 12,5 s → **~3,5 s**.
  *Bằng chứng*: 8 lần `--law 1` tuần tự **9,88 s** → song song **2,65 s** (3,7×).
- **(b) Mất phủ: KHÔNG.** 21 ca phá + 4 đối chứng dương, mỗi ca đã có thư mục riêng
  `case-<id>` (`verify-numbers.py:1384-1389`) nên **không** đua.
- **(c) Đổi ngữ nghĩa cổng: KHÔNG** (cùng ca, cùng mã lỗi kỳ vọng). Sửa công cụ ⇒ Agent 0.

### P6 — Song song hoá 6 fixture của **máy kiểm báo cáo**

- **(a) Tiết kiệm `[ƯL]`: ~11 s mỗi `check`/`gate`.** 13,2–15,8 s → **~2,6–4,2 s**.
  *Bằng chứng*: tuần tự **22,86 s** → song song **4,20 s** (5,4×).
- **(b) Mất phủ: KHÔNG.** **ĐÍNH CHÍNH (review chéo)**: điều kiện `/tmp/repfx.out` **không** thuộc
  P3 — `v2/bin/v2` gọi **mỗi** script fixture đúng một lần, hai script ghi **hai tệp khác nhau** nên
  P3 không va chạm. Điều kiện đó thuộc **P6** (song song sáu ca *bên trong* `run_fixtures.sh`).
- **(c) Rủi ro phải ghi trước khi bật**: có **ba** chỗ phụ thuộc **đồng hồ thực** —
  `v2/lib/vifast-sync/tests/kt30_stress.rs:21` (biên ba giây), `v2/lib/vifast-ring/tests/spsc.rs:218`
  (hạn mười giây), `v2/lib/vifast-objmem/tests/compile_fail.rs:74` (tên tệp cố định trong `temp_dir`).
  Đo ở mười hai lần quá tải: còn dư năm đến tám lần ⇒ rủi ro **thấp nhưng có thật**; cần chính sách
  `T-INFRA` trước khi bật P1.
- **(c) Đổi ngữ nghĩa cổng: KHÔNG.**

### P7 — Gộp R1/R3/R5 thành **một lượt** quét thay vì spawn mỗi tệp

- **(a) Tiết kiệm `[ƯL]`: ~8 s mỗi `check`/`gate`.** 9,1–9,9 s → **~1,5 s**.
  *Bằng chứng*: R1 2,58 s + R3 3,09 s + R5 1,90 s; bản gộp R3 một lượt `grep` = **0,51 s**.
- **(b) Mất phủ: KHÔNG** (cùng vị từ). **Cẩn thận**: R3 phải kiểm "trong **5 dòng đầu**" ⇒ dùng
  `grep -n` rồi lọc số dòng ≤ 5; dùng `grep -l` là **mất** ràng buộc vị trí (thành nới lỏng).
- **(c) Đổi ngữ nghĩa cổng: KHÔNG.**

### P8 — Bỏ lần chạy **trùng** `l1_1000_seed_tat_dinh` (L0 đã chạy, bước L1 chạy lại)

- **(a) Tiết kiệm `[ƯL]`: ~1,2 s** (0,34 s trong L0 + 0,92 s ở bước L1). **Thấp.**
- **(b) Mất phủ: trong cùng một lần `gate` thì KHÔNG** (vẫn chạy đúng 1 lần). Nhưng nếu ai chạy
  riêng L0 (`cargo test --workspace`) thì **mất** phủ L1.
- **(c) Đổi ngữ nghĩa cổng: CÓ** ⇒ Agent 0. **Khuyến nghị: KHÔNG làm** — 1,2 s không đáng đánh
  đổi một phép kiểm đang chạy ở hai tầng.

### P9 — (ghi nhận, không đề xuất) `fmt` 3,0 s / 10,8 s nguội

`cargo fmt --all --check` spawn rustfmt theo crate, không có cache. Cách duy nhất nhanh hơn là
chạy song song theo crate — nhưng `fmt` chỉ 5,8 % của `check` ấm và **không** nằm trong critical
path sau P1/P3. **Không đáng làm.**

---

## 7. Ước lượng tổng hợp nếu làm P1+P2+P3+P5+P6+P7

| Lệnh | Hiện tại (ấm) | Sau đề xuất `[ƯL]` | Ghi chú |
|---|---:|---:|---|
| `check` | 49,4–55,6 s | **~10–12 s** | nhóm không-cargo 48 s → ~4 s (P3+P5+P6+P7); cargo 4 s |
| `gate --fast` | 371–407 s | **~35–40 s** | L0 336 s → ~25 s (P1+P2); L1 1 s |
| L1 (1.000 seed) | 0,92–1,07 s | **0,92–1,07 s** | **không đụng** — đã dưới ngưỡng 60 s rất xa |

Con số 35–40 s đưa `gate --fast` về cùng bậc với `check` hiện nay, tức mỗi vòng lặp task nhanh
hơn **~10 lần** mà **không** bỏ phép kiểm nào.

---

## 8. Điều **không** làm (đúng ràng buộc task)

- **Không** sửa tệp nào trong repo (chỉ ghi `v2/agent/JOURNAL/kt110-*`); **không** commit,
  **không** `git add`. Mọi thí nghiệm chạy trong `/tmp/kt110` hoặc chỉ đọc.
- **Không** chạy `nightly`, **không** soak, **không** ma trận.
- **Không** đề xuất bỏ phép kiểm nào, **không** đề xuất nới ngưỡng/giảm seed. Mọi đề xuất là
  **làm nhanh hơn** hoặc **chạy song song**.
- **Không** kết luận từ một lần đo: mỗi lệnh chạy hai lần, ghi cả hai + `loadavg` (§1). Máy **có**
  việc khác (agent ViScript) nên số tuyệt đối có thể cao hơn máy rảnh; **tỉ lệ** giữa các bước
  ổn định qua cả hai lần (K6 32,5 %, fixture báo cáo 27,7 %, luật cấu trúc 18,1 %; L0 85–87 %).
- **Không** sửa `v2/bin/v2`, `verify-numbers.py`, `run_fixtures.sh` hay bất kỳ test nào — P1..P7
  là **đề xuất**, cần task riêng và (với P1/P3/P4/P8) chữ ký Agent 0.

## 9. Tệp bằng chứng

| Tệp | Nội dung |
|---|---|
| `kt110-check-r1.log`, `kt110-check-r2.log` | `check` hai lần, mốc thời gian từng dòng + `loadavg` |
| `kt110-check-cold.log` | `check` với `CARGO_TARGET_DIR=/tmp/kt110/target-cold` (nguội) |
| `kt110-gate-r1.log`, `kt110-gate-r2.log` | `gate --fast` hai lần, đủ 100 binary test |
| `kt110-parse.py` | bộ tách "mốc dòng → thời gian từng bước" dùng cho mọi bảng trên |
