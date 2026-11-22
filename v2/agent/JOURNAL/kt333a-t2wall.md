# KT-333a — Profile & hạ wall-time T2 (`gate --fast`) + CI host

**Chu kỳ 2 — Đợt 0.** Người làm: sub-agent `t2wall`. Ngày: 2026-11-22 (giờ máy Asia/Tokyo).

| | |
|---|---|
| Worktree | `/tmp/kt333a-s1` (nhánh `task/kt333a-t2wall`) |
| Base | `main` @ `3a184935` (`[KT-331] QUEUE DONE …`) |
| Máy | 12 nhân (`hw.ncpu=12`), macOS, Rust/cargo `1.100.0` (ghim `v2/rust-toolchain.toml`) |
| Lệnh đo | `./v2/bin/v2 gate --fast` (T2) và `./v2/bin/v2 check`, `export PATH="$HOME/.cargo/bin:$PATH"` |
| Nhãn số liệu | **`[ĐO KT-333a]`** — đo trực tiếp từ log cổng, không chép tay |

> **Tái dùng bằng chứng (§6.3)**: đây là **số đo mới** vì đầu vào là `v2/bin/v2` (bản vá này) và lần
> đo trước trong KT-242 (668 s) ở máy/phiên bản cổng khác (53 bước). Không tái dùng số cũ.

---

> ## ⚠ SỐ ĐO TRONG TỆP NÀY **KHÔNG** DÙNG LÀM BẰNG CHỨNG
> Ngày 2026-11-22, 15:31–15:45 có **hai `gate --fast` chạy song song trên cùng repo** (worktree này
> và checkout chính) ⇒ các bước git-guard/QEMU đỏ oan và thời gian bị phồng. Mọi con số wall/lane
> dưới đây là **quan sát thô** để thấy **cấu trúc** (chỗ nào cắt được), **không** phải bằng chứng
> nghiệm thu — xem **§10 "Sự cố đo song song 15:31–15:45"**. Lead sẽ đo lại **một mình** trên `main`.

## 0. TL;DR (**số thô, chưa hợp lệ — xem cảnh báo trên**)

| Chỉ số | Trước (thô) | Sau (thô) | Δ (thô) |
|---|---:|---:|---|
| **Wall `gate --fast`** | **905,3 s** | **507,9 s** | **−397,4 s (−43,9 %)** |
| CPU (user+sys) | 1.531,9 s | 1.416,7 s | −115,2 s |
| Hệ số song song (CPU/wall) | 1,69 nhân | 2,79 nhân | +1,10 |
| Số bước đăng ký / chạy | 69 / 69 | 69 / 69 | **0 bước bị bỏ** |
| Kết quả bước | 68 OK + 1 ĐỎ (flake D2-151) | **69/69 OK** | — |
| `v2 check` (pha 1) | ~116 s (tính từ log: 108,7 + 7,3) | **77,8 s** | −33 % |
| Cổng cuối | — | **KHÔNG đạt trong worktree này** (bị dừng theo yêu cầu lead; lead đo lại trên `main`) | — |

**68 hay 69?** Kế hoạch ghi "68 bước"; KT-331 (đã merge ở base `3a184935`) thêm bước
`selftest tên tệp NTFS (KT-331)` id **72** ⇒ T2 hiện có **69** bước. Log lượt trước có đúng **69**
dòng `# step-time`, tập **tên** trùng khít tập tên đăng ký hiện tại (`diff` rỗng — §5). Bản vá này
**không** thêm/bớt/đổi tên bước nào.

---

## 1. Phương pháp đo

1. Làm ấm cache (worktree mới, `v2/target` chưa có) bằng `cargo build --workspace`,
   `cargo clippy … -D warnings`, `cargo test --workspace --no-run`, `cargo test` workspace kernel
   `--no-run` ⇒ lượt đo không lẫn chi phí biên dịch **lần đầu** (ảnh QEMU vẫn được dựng trong lượt
   đo như mọi lần chạy cổng thật).
2. **Lượt TRƯỚC**: `./v2/bin/v2 gate --fast` trên mã **gốc** (`3a184935`), log
   `/tmp/kt333a-logs/run1-before.log`.
3. Vá **chỉ** `v2/bin/v2` (+ workflow CI) theo §4.
4. **Lượt SAU**: cùng lệnh, cùng máy, log `/tmp/kt333a-logs/run2-after.log`.
5. Bảng thời gian lấy từ dòng `# step-time <tên> <ms>` do chính cổng in; tên bước so khớp
   **theo tên** (không theo thứ tự hoàn thành).

**Điều kiện tải (trung thực)**: máy dùng chung với các sub-agent khác của Chu kỳ 2; `loadavg` lúc
chạy ~4–7 (12 nhân; xem `# infra before … loadavg_1m` trong log bước persist). Vì vậy **số tuyệt
đối** của từng bước bị phồng so với máy rảnh (ví dụ `QEMU x86_64` 103 s ở lượt trước, 112 s ở lượt
sau; D2-229 ghi baseline 828 s). Kết luận **wall-time** dựa trên **cùng điều kiện tải** và cấu trúc
làn, không dựa vào một bước đơn lẻ.

---

## 2. Bảng 69 bước: trước → sau

Cột **làn** = làn cũ → làn mới (in đậm nếu đã dời). **Độc quyền** = bước có bị buộc chạy một mình
không. `main` cũ chứa cả bước **chỉ đọc** (`fmt`, `luật cấu trúc`, `selftest build-inputs`) ⇒ chờ
`clippy`/`wake dep-info` (ghi `target/`) một cách **không cần thiết**.

| # | Bước | Trước (s) | Làn cũ | Độc quyền (cũ) | Sau (s) | Làn mới | Độc quyền (mới) |
|---:|---|---:|---|---|---:|---|---|
| 1 | fmt | 5.7 | main | tuần tự (chỉ đọc — **chờ vô ích**) | 10.7 | **main→F** | song song |
| 2 | clippy | 19.4 | main | tuần tự (ghi target/build) | 7.4 | main | tuần tự (ghi target/build) |
| 3 | luật cấu trúc R1..R5 + tên tệp NTFS (KT-331) | 20.8 | main | tuần tự (chỉ đọc — **chờ vô ích**) | 29.0 | **main→F** | song song |
| 4 | self-test luật | 1.5 | A | song song (chỉ đọc) | 1.6 | A | song song |
| 5 | luật máy K6 (KT-80: 1,2,3,7,8) | 37.3 | A | song song (chỉ đọc) | 34.8 | **A→D** | song song |
| 6 | selftest build-inputs (KT-128/134: a..h) | 27.8 | main | tuần tự (chỉ đọc — **chờ vô ích**) | 25.8 | **main→D** | song song |
| 7 | selftest danh tính ảnh (KT-145: l1..l5) | 8.0 | C | song song (chỉ đọc) | 10.4 | **C→E** | song song |
| 8 | selftest t26 (KT-149: g1..g4) | 0.5 | C | song song (chỉ đọc) | 0.7 | **C→F** | song song |
| 9 | ghim thuật toán dấu vân tay (KT-134) | 0.2 | C | song song (chỉ đọc) | 0.3 | **C→F** | song song |
| 10 | selftest ghim thuật toán (KT-134) | 2.0 | C | song song (chỉ đọc) | 2.3 | **C→F** | song song |
| 11 | selftest shell-edge (KT-108) | 7.9 | B | song song (chỉ đọc) | 9.2 | B | song song |
| 12 | selftest budget (KT-88) | 0.1 | B | song song (chỉ đọc) | 0.4 | **B→F** | song song |
| 13 | selftest counter-evidence (KT-80) | 0.3 | B | song song (chỉ đọc) | 0.6 | **B→F** | song song |
| 14 | selftest task-target (KT-84) | 0.6 | B | song song (chỉ đọc) | 0.7 | **B→F** | song song |
| 15 | selftest ba mức L1 (KT-242) | 0.8 | B | song song (chỉ đọc) | 0.6 | **B→F** | song song |
| 16 | bảng đối chiếu số ↔ khối scope= (KT-134) | 0.2 | sau | độc quyền toàn cục | 0.2 | sau | tuần tự một mình (D2-151) |
| 17 | selftest bảng đối chiếu (KT-134) | 0.9 | B | song song (chỉ đọc) | 0.4 | **B→F** | song song |
| 18 | selftest wiring (KT-134) | 4.8 | B | song song (chỉ đọc) | 3.6 | **B→E** | song song |
| 19 | cổng selftest có cổng gọi (KT-134) | 3.3 | B | song song (chỉ đọc) | 2.2 | **B→E** | song song |
| 20 | wake paths (KT-151: một cơ chế wake) | 62.3 | C | song song (chỉ đọc) | 62.2 | C | song song |
| 21 | selftest wake paths (KT-151) | 66.4 | A | song song (chỉ đọc) | 68.5 | A | song song |
| 22 | wake paths --strict (KT-155: allowlist) | 59.6 | B | song song (chỉ đọc) | 55.8 | B | song song |
| 23 | wake theo dep-info (KT-217: lớp 2) | 35.0 | main | tuần tự (ghi target/build) | 33.8 | main | tuần tự (ghi target/build) |
| 24 | selftest wake dep-info (KT-217) | 2.7 | C | song song (chỉ đọc) | 1.9 | C | song song |
| 25 | api boundary (KT-241) | 5.5 | sau | độc quyền toàn cục | 4.2 | sau | tuần tự một mình (D2-151) |
| 26 | test có răng (KT-163) | 7.3 | B | song song (chỉ đọc) | 5.9 | **B→E** | song song |
| 27 | selftest test có răng (KT-163) | 0.6 | B | song song (chỉ đọc) | 0.6 | **B→F** | song song |
| 28 | abi spec | 0.3 | C | song song (chỉ đọc) | 0.2 | **C→F** | song song |
| 29 | golden trace (D2-43) | 0.3 | C | song song (chỉ đọc) | 0.2 | **C→F** | song song |
| 30 | kịch bản .scn (KT-04c) | 0.3 | C | song song (chỉ đọc) | 0.2 | **C→F** | song song |
| 31 | scn fixtures (KT-04c) | 1.0 | C | song song (chỉ đọc) | 1.1 | **C→F** | song song |
| 32 | báo cáo R1-v2 (Khối E) | 3.2 | B | song song (chỉ đọc) | 5.1 | **B→E** | song song |
| 33 | fixture âm của máy kiểm báo cáo | 20.5 | C | song song (chỉ đọc) | 24.0 | **C→E** | song song |
| 34 | abi reserved (KT-04b) | 0.2 | C | song song (chỉ đọc) | 0.2 | **C→F** | song song |
| 35 | abi fixtures | 0.7 | C | song song (chỉ đọc) | 0.9 | **C→F** | song song |
| 36 | gen-check (KT-05: code + docs sinh từ spec) | 0.4 | C | song song (chỉ đọc) | 0.3 | **C→F** | song song |
| 37 | gen fixtures | 1.2 | C | song song (chỉ đọc) | 1.2 | **C→F** | song song |
| 38 | idl-gen-check (KT-69: stub sinh từ .vidl) | 0.4 | C | song song (chỉ đọc) | 0.2 | **C→F** | song song |
| 39 | idl fixtures | 0.6 | C | song song (chỉ đọc) | 0.6 | **C→F** | song song |
| 40 | build (L0 compile) | 1.6 | sau | độc quyền toàn cục | 1.4 | sau | tuần tự một mình (D2-151) |
| 41 | L0 test | 87.8 | main | tuần tự (ghi target/build) | 104.0 | main | tuần tự (ghi target/build) |
| 42 | L0 kernel workspace (KT-148) | 19.8 | main | tuần tự (ghi target/build) | 10.3 | main | tuần tự (ghi target/build) |
| 43 | L1 (sim 1.000 seed) | 1.0 | main | tuần tự (ghi target/build) | 1.0 | main | tuần tự (ghi target/build) |
| 44 | selftest runner ktest (KT-54) | 182.6 | sau | độc quyền toàn cục | 220.4 | **sau→sau2** | song song |
| 45 | selftest qemu guard (KT-78) | 20.5 | sau | độc quyền toàn cục | 22.3 | **sau→sau2** | song song |
| 46 | selftest pre-measure (KT-95) | 7.8 | main | tuần tự (đo) | 7.5 | main | tuần tự (đo) |
| 47 | selftest pin-artifact (KT-84) | 5.9 | main | tuần tự (đo) | 5.2 | main | tuần tự (đo) |
| 48 | algo= nghiêm ngặt ở đường đo (KT-145) | 4.3 | main | tuần tự (đo) | 4.7 | main | tuần tự (đo) |
| 49 | selftest persist đỏ-trước (KT-75) | 7.7 | sau | độc quyền toàn cục | 12.3 | sau | tuần tự một mình (D2-151) |
| 50 | cắt điện giữa lúc ghi (KT-136) | 26.9 | sau | độc quyền toàn cục | 22.4 | sau | tuần tự một mình (D2-151) |
| 51 | selfbreak 5 luật K6 (KT-80) | 156.8 | D | song song (chỉ đọc) | 154.7 | **D→sau3** | song song |
| 52 | ca âm git-guard (KT-85) | 20.3 | sau | độc quyền toàn cục | 18.8 | **sau→sau4** | song song |
| 53 | ca âm git-guard song song (R5) | 16.3 | sau | độc quyền toàn cục | 22.2 | **sau→sau4** | song song |
| 54 | ca âm git-guard b (KT-85b) | 31.7 | sau | độc quyền toàn cục | 50.7 | **sau→sau4** | song song |
| 55 | selftest validator net (KT-245) | 12.2 | B | song song (chỉ đọc) | 13.6 | **B→D** | song song |
| 56 | QEMU aarch64 (K4' bước 3) | 67.7 | sau | độc quyền toàn cục | 77.9 | **sau→sau5** | song song |
| 57 | QEMU virtio-net (KT-245/246) | 58.6 | sau | độc quyền toàn cục | 74.9 | **sau→sau5** | song song |
| 58 | kiểm máy --cfg test (KT-256) | 20.3 | main | tuần tự (ghi target/build) | 18.9 | main | tuần tự (ghi target/build) |
| 59 | QEMU x86_64 (K5' bước 1) | 103.0 | sau | độc quyền toàn cục | 112.2 | **sau→sau6** | song song |
| 60 | QEMU virtio-input (KT-255) | 6.5 | sau | độc quyền toàn cục | 36.0 | **sau→sau6** | song song |
| 61 | selftest validator input (KT-255) | 2.6 | B | song song (chỉ đọc) | 3.1 | B | song song |
| 62 | selftest validator cfg-test (KT-256) | 0.2 | B | song song (chỉ đọc) | 0.8 | B | song song |
| 63 | QEMU virtio-gpu (KT-263) | 10.6 | sau | độc quyền toàn cục | 11.2 | **sau→sau6** | song song |
| 64 | selftest validator gpu (KT-263) | 23.6 | B | song song (chỉ đọc) | 26.0 | **B→C** | song song |
| 66 | selftest soak aarch64 (KT-265) | 65.8 | B | song song (chỉ đọc) | 98.5 | B | song song |
| 67 | selftest budget aarch64 (KT-265) | 0.1 | B | song song (chỉ đọc) | 0.2 | **B→D** | song song |
| 69 | wake dep-info --product-fs (KT-267) | 44.9 | B | song song (chỉ đọc) | 74.6 | **B→C** | song song |
| 71 | selftest soak x86_64 (KT-293) | 35.3 | B | song song (chỉ đọc) | 82.7 | **B→D** | song song |
| 72 | selftest tên tệp NTFS (KT-331) | 4.3 | B | song song (chỉ đọc) | 3.7 | **B→E** | song song |

> Đọc bảng: tổng thời gian **công** (sum các bước) lượt sau (1.700 s) **lớn hơn** lượt trước
> (1.458 s) — bình thường: các bước chạy chen nhau nên mỗi bước chậm hơn dưới tải, nhưng **wall**
> giảm vì chồng lấn. Đại lượng cần giảm là wall, không phải sum.

---

## 3. Critical path & ba–năm nguyên nhân lớn nhất

### 3.1 Lượt TRƯỚC (905,3 s)

| Pha | Cấu trúc | Thời gian |
|---|---|---:|
| 1 — `check` | `max(main 108,7; A 105,2; B 93,7; C 101,8; **D 156,8**) + sau 7,3` | **164,1 s** |
| 2 — `gate` | `max(main 146,9; **B 184,7**) + sau 552,4` | **737,1 s** |
| Tổng | | **901,2 s** + ~4 s overhead ≈ 905,3 s |

**Năm nguyên nhân lớn nhất** `[ĐO KT-333a]`:

1. **Đuôi tuần tự 552,4 s (61 % wall)**: 12 bước QEMU/persist/git-guard/sửa cây **xếp hàng** trong
   một làn `sau`, dù tài nguyên phần lớn **rời nhau**: `selftest runner ktest` 182,6 s,
   `QEMU x86_64` 103,0 s, `QEMU aarch64` 67,7 s, `QEMU virtio-net` 58,6 s,
   `ca âm git-guard b` 31,7 s, `cắt điện` 26,9 s, `qemu guard` 20,5 s, `git-guard` 20,3 s…
2. **`selfbreak` 156,8 s nằm trọn trên critical path pha 1**: nó **chỉ đọc** (băm 4 đường
   nguồn + dựng bản copy) nhưng phải xong mới cho pha 2 chạy; `main`/A/B/C xong ở ~109 s.
3. **Pha 2, làn B = 184,7 s**: 8 selftest validator **chỉ đọc** dồn một làn
   (66 = 65,8 s; 69 = 44,9 s; 71 = 35,3 s; 64 = 23,6 s; 55 = 12,2 s) ⇒ vượt `main` (146,9 s).
4. **Pha 1 chỉ có 3 làn nền cho ~300 s công chỉ-đọc** (A 105,2; B 93,7; C 101,8) trong khi máy 12
   nhân, và `main` (108,7 s) ôm 3 bước **chỉ đọc** (fmt 5,7; luật 20,8; build-inputs 27,8).
5. **QEMU nhạy tải**: cùng một bước `QEMU x86_64` đo 103 s dưới tải ngoài (máy rảnh thường ~15–20 s)
   ⇒ phải **giới hạn** QEMU đồng thời (≤ 3, AGENTS §9) chứ không phải tuần tự hoá tất cả; đồng thời
   bước **nhạy tải nhất** (`cắt điện` — D2-151) phải có **cửa sổ yên tĩnh** riêng.

### 3.2 Bước **chờ làn khác không cần thiết** (trả lời trực tiếp câu hỏi)

| Bước | Chờ ai (lượt trước) | Vì sao không cần |
|---|---|---|
| `fmt`, `luật cấu trúc R1..R5`, `selftest build-inputs` | xếp sau `clippy`/`wake dep-info` trong `main` (ghi `target/`) | chỉ **đọc** cây; không ghi `target/`, không sửa cây |
| `selfbreak` | cả pha 2 (`L0`, `L1`, toàn bộ đuôi) | chỉ đọc 4 đường nguồn **rời** mọi bước khác (xem §4) |
| `selftest validator *` (55/61/62/64/66/67/69/71) | nhau trong **một** làn B | độc lập nhau, chỉ đọc |
| `selftest runner ktest` (44) | 11 bước đuôi khác (và ngược lại) | tài nguyên rời, trừ 44→45 (cùng ảnh) — xem §4 |
| `QEMU x86_64/input/gpu`, `aarch64/net`, `git-guard`, `persist` | 44 và nhau | logdir/target/cổng riêng; chỉ 56↔57 chung cổng 45678, 49→50 chung logdir |

### 3.3 Lượt SAU (507,9 s) — critical path mới

| Pha | Cấu trúc | Thời gian |
|---|---|---:|
| 1 — `check` | `max(main 41,2; **A 70,1**; B 65,0; C 64,1; D 60,5; E 54,9; F 57,4) + sau 5,8` | **76,3 s** |
| 2 — `gate` | `max(main 151,6; B 102,4; C 100,6; D 96,5) + [sau 40,6 + max(sau2 242,7; sau3 154,7; sau4 91,7; sau5 152,9; sau6 159,4)]` | **429,7 s** |
| Tổng | | **506,0 s** (wall 507,9 s) |

Critical path **còn lại** = **`selftest runner ktest` (44) 220,4 s** trong làn `sau2` (sau nó là
`selftest qemu guard` 22,3 s phụ thuộc cùng ảnh). Kế đến: `L0 test` 104,0 s + phần `main` pha 2
(151,6 s), rồi các làn QEMU 152–159 s (bị phồng do chạy song song — đúng mục đích). **Không thể**
đưa 44 sang pha 1/2 vì (a) nó sửa `expected.txt` + QEMU/build, (b) chạy cạnh `L0 test` sẽ làm đỏ
oan binary nhạy thời gian (D2-116/D2-141) — xem §8.

---

## 4. Thay đổi (chỉ `v2/bin/v2`; không đổi nội dung phép kiểm)

### 4.1 Mô hình làn mới

* `main` — **chỉ** bước ghi `target/` (clippy, wake theo dep-info, L0, L0 kernel, L1) và bước **đo**
  (pre-measure, pin-artifact, algo-strict, `--cfg test`). Giữ tuần tự (D2-116/D2-141).
* `A..F` — 6 làn nền **chỉ đọc** ở pha `check` (trước: 3 làn A/B/C + D cho selfbreak).
* `sau` — **pha đuôi tuần tự, chạy một mình** (shell chính): `49 selftest persist đỏ-trước` →
  `50 cắt điện giữa lúc ghi`. Đây là **cửa sổ yên tĩnh** cố ý cho D2-151 (xem §8).
* `sau2..sau6` — **pha đuôi song song**: mỗi làn tuần tự, các làn chạy đồng thời, tài nguyên rời
  nhau; **QEMU đồng thời ≤ 3** (`sau2`, `sau5`, `sau6`) — đúng trần AGENTS §9.

### 4.2 Vì sao từng nhóm dời làn là **an toàn** (bước chỉ đọc, không tranh chấp)

| Nhóm | Làn cũ → mới | Chứng cứ an toàn |
|---|---|---|
| `1 fmt`, `3 luật cấu trúc`, `6 selftest build-inputs` | main → F/F/D | chỉ đọc cây + `mktemp`; `fmt --check` không ghi; luật/build-inputs không ghi `target/` (KT-242 inventory §1). `main` chỉ còn bước ghi `target/`. |
| `5 luật máy K6` | A → D | chỉ đọc `v2/docs`, `reports`, `JOURNAL` + git; cache `$TMPDIR` (không đụng cây). |
| các `selftest *` nhỏ (7–39) | C/B → E/F | chỉ đọc + `mktemp`/`V2_RUN_TMP`; 6 làn cân theo thời gian đo (mỗi làn 40–70 s). |
| `20/21/22 wake paths` (62/69/56 s) | C/A/B giữ, tách mỗi bước một làn | ba checker **độc lập**, chỉ đọc; trước đây 20 (62 s) và 21 (66 s) chạy song song đã ổn định. |
| `23 wake theo dep-info` | main (giữ) | **ghi `v2/target/`** (cargo check) ⇒ tuần tự với clippy (đúng hàng đợi `main`). |
| `16 scope table` (ghi `JOURNAL/scope-table-skip-count.txt`), `25 api boundary` (chèn tệp probe vào crate), `40 build` (ghi `target/`) | sau (giữ) | **sửa cây/ghi target** ⇒ phải sau mọi làn đọc; giữ trong pha đuôi tuần tự ngắn (5,8 s). |
| `44 selftest runner ktest` + `45 selftest qemu guard` | sau → sau2 (cùng làn, 44 trước) | 44 **sửa `v2/test/boot/expected.txt` ~1 s** (trả lại + sha256); **không** runner nào khác đọc tệp này (đã `grep` 8 runner QEMU: chỉ `ktest.sh`/`selftest.sh`). 45 dùng **cùng ảnh sản phẩm** `vifast-kbin-riscv64` do 44 dựng ở rev-gate ⇒ bắt buộc 44 trước 45, khác làn sẽ hỏng. |
| `51 selfbreak` | D → sau3 | chỉ đọc: băm `v2/bin/v2`, `v2/tools/verify-numbers.py`, `v2/docs/k6-machine-rules.md`, `v2/test/rules/**`; dựng copy bằng `git ls-files`/`git show` (đọc index/HEAD worktree chính). **Không** bước đuôi nào ghi 4 đường đó (git-guard chỉ `git add` trong **worktree tạm**, `neg-test.sh:25`); chạy được khi 3 QEMU + git-guard đang chạy (đo thật lượt sau: PASS). |
| `52/53/54 git-guard` | sau → sau4 (một làn) | tạo `git worktree add` + nhánh tạm + `git init --bare` trong `mktemp`; cùng làn để **không** hai tiến trình cùng sửa worktree/branch/`worktree list` (script tự so `WT_COUNT`, `INDEX_BEFORE/AFTER`). |
| `56 aarch64` + `57 virtio-net` | sau → sau5 (một làn, 56 trước) | **cùng bind `127.0.0.1:45678`** (`run-aarch64.sh:211`, `net.sh:110`) ⇒ bắt buộc **tuần tự trong một làn**; đã kiểm không script đuôi nào khác bind cổng này. |
| `59 x86_64`, `60 input`, `63 gpu` | sau → sau6 | logdir `$V2_RUN_TMP` riêng, ảnh/bin riêng, không bind cổng host; cargo tự khoá `kernel/target` nên build xen kẽ **không** đua tệp. |
| `55/61/62/64/66/67/69/71 selftest validator` | B/C → B/C/D (chia 3 làn) | thuần kiểm **validator** (không QEMU thật, không ghi cây): net/input/cfg-test/gpu/aarch64-soak/budget/x86_64-soak. Chỉ đọc + `mktemp`. |

### 4.3 Gộp bước?

**Không gộp** bước nào: các cặp "cùng chương trình" thực sự khác **chế độ/đầu vào** và mỗi lần gọi
kiểm một bất biến riêng — `check-algo-pin.sh --strict` vs `--selftest` (9/10), `git-guard/neg-test.sh`
vs `--selftest-parallel` (52/53), `persist.sh --red-selftest` vs `--crash` (49/50),
`soak-*` selfcheck vs soak thật. Gộp sẽ **mất phép kiểm**, trái yêu cầu "không mất phép kiểm" ⇒
giữ nguyên từng lời gọi, chỉ đổi **làn**. Số phép kiểm vì vậy **không đổi** (69 bước, cùng tên).

### 4.4 Chống bỏ bước **âm thầm** (D2-235(5))

* `lane_step` ghi nhận id đã thấy; **trùng id ⇒ `lane_run` fail-closed ngay**, in
  `--- LỖI: trùng id lane_step:<id> …` và trả 1 **trước khi chạy bất kỳ bước nào**.
* Hằng số **đếm bước** (fail-closed): `CHECK_STEPS_EXPECTED=41`, `GATE_FAST_EXTRA_EXPECTED=28`
  (T2 = 69), T1 = 69 + 1 (QEMU smoke) = 70; lệch ⇒ cổng ĐỎ trước khi chạy.
* Cuối log có dòng `# gate-steps registered=… ran=… skipped=… failed=…` và
  `# gate-phase pha1=… pha2=…`, `# gate-critical-path` **đo được** (trước đây không có).

---

## 5. Đo lại — chứng minh **không mất bước**

### 5.1 Số liệu cổng (nguyên văn từ log)

```
# TRƯỚC: # gate-wall 905251 ms (905.3 s)   # gate-cpu user=1106.263 s sys=425.634 s total=1531.897 s
#        # gate-lane-time main=255633 ms D=156797 ms A=105217 ms C=101814 ms B=278520 ms sau=559617 ms
#        # gate-critical-path 838137 ms (chuỗi dài nhất: làn B + sau)
#        --- cắt điện giữa lúc ghi (KT-136): FAIL  (D2-151, xem §5.3)

# SAU:   # gate-wall 507936 ms (507.9 s)   # gate-cpu user=996.071 s sys=420.657 s total=1416.728 s
#        # gate-lanes 6 làn nền (F A D E B C) + main + làn đuôi tuần tự (sau) + 5 làn đuôi song song
#        # gate-lane-time main=192768 ms F=51340 ms A=70088 ms D=156995 ms E=54916 ms B=167377 ms
#                          C=164673 ms sau=40628 ms sau2=242707 ms sau3=154661 ms sau4=91726 ms
#                          sau5=152851 ms sau6=159401 ms
#        # gate-tail 283335 ms (sau=40628 ms + làn song song dài nhất sau2=242707 ms)
#        # gate-phase pha1=76311 ms pha2=429653 ms (tổng 505964 ms)
#        # gate-steps registered=69 ran=69 skipped=0 failed=0
#        v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
```

### 5.2 Không mất bước — `diff` danh sách **tên**

* Tập tên **đăng ký trong mã** (T2: 41 bước `check` + 28 bước `gate`) = **69**.
* Tập tên trong log **lượt trước** = **69**; log **lượt sau** = **69**.
* `diff` tập tên (đã sắp xếp) mã-hiện-tại ↔ log-trước = **RỖNG**; **69 = 69 = 69**; id **không
  trùng** (kiểm `grep -n "lane_step" v2/bin/v2` trên **cả** `do_gate_fast` **và** `do_gate_dev`:
  70 dòng đăng ký — 69 T2 + 1 T1 `QEMU smoke` id 70 — id đôi một khác nhau).
* Lượt sau in `# gate-steps registered=69 ran=69 skipped=0 failed=0` ⇒ **69/69 bước OK**.

### 5.3 Lượt TRƯỚC có 1 bước đỏ — **flake D2-151 đã biết**, không phải hồi quy

Lượt trước chạy dưới tải của các sub-agent khác (`loadavg ≈ 7`), đỏ **đúng một** bước `50`:

```
FAIL crash-host kt75-disk.img: get /pstate: 13 bản ghi (cần 11 hoặc 12 sau crash giữa pha 7)
```

Đây **chính xác** chữ ký **D2-151** (đã ghi 2026-11-19; gặp lại ở KT-264 phụ lục): ca này `kill`
QEMU theo **dòng serial**; host chậm ⇒ lệnh kill rơi **sau** khi khách ghi xong pha 7 ⇒ `13` bản
ghi. Bằng chứng trong log: `/p7.bin` đủ `73728 B`, `partial=0`, `committed=1` — pha 7 **đã xong**,
toàn vẹn dữ liệu không bị vi phạm. Theo AGENTS §4.5, chạy lại **một lần** chỉ để **phân loại**:

```
$ bash v2/test/boot/persist.sh --crash
# crash: đã giết group pid=6043 ngay sau dòng verify pha 6 (đang ghi pha 7)
# persist.sh: PASS          (19,8 s, exit 0)
```

⇒ Bước 50 **không hồi quy**; lượt sau bước 50 chạy **PASS** (22,4 s) và bản vá này **không sửa**
`v2/test/boot/persist.sh` hay đầu vào FS. Vì D2-151 nhạy tải, bản vá giữ `49/50` trong **cửa sổ yên
tĩnh** `sau` (không chạy cạnh QEMU nào khác) — xem §8.

---

## 6. Đột biến — chứng minh cơ chế đếm/không-trùng-id hoạt động

### (i) Cố tình đặt **trùng** `lane_step` id

* **Mã mới**, đổi id bước 72 → 17 (17 đã tồn tại, trong `do_check`):

```
$ ./v2/bin/v2 check
--- LỖI: trùng id lane_step:17 — bước cùng id thứ hai sẽ bị bỏ qua im lặng (D2-235(5)); sửa id rồi chạy lại
exit=1
```

* **Mã cũ** (`3a184935`), cùng đột biến ⇒ **PASS nhưng bỏ bước im lặng**. Bằng chứng đo:

```
$ ./v2/bin/v2 check        # mã cũ, lane_step B 6 "selftest tên tệp NTFS (KT-331)"
v2 check: PASS (L1 chạy trong gate --fast)
$ ls $LANE_DIR/*.rc | wc -l     -> 40       (đăng ký 41 bước `check`)
$ ls $LANE_DIR/72.log           -> KHÔNG có
# hai tên khác nhau in CÙNG một thời gian: 21149 ms
#   selftest build-inputs (KT-128/134: a..h)      21149 ms
#   selftest tên tệp NTFS (KT-331)                21149 ms
```

⇒ Bước `selftest tên tệp NTFS` **không hề chạy**, nhưng log vẫn in `OK` vì `lane_report` phát lại
theo id và `lane_done` thấy `.rc` của bước trước. **Đây đúng là lỗi D2-235(5) mà bản vá chặn.**

### (ii) Bỏ một bước

* Xoá dòng `lane_step E 72 …` (mã mới):

```
$ ./v2/bin/v2 check
--- LỖI: check đăng ký 40 bước, kỳ vọng 41 — cập nhật CHECK_STEPS_EXPECTED nếu thêm/bớt bước có chủ ý
exit=1
```

⇒ Cổng **ĐỎ** ngay, không chạy bước nào; muốn bỏ bước phải sửa hằng số **có chủ ý** (review thấy
trong diff), không thể bỏ âm thầm. Đã khôi phục mã gốc sau đột biến (`bash -n` OK, 70 dòng
`lane_step`, `diff` tên rỗng).

---

## 7. CI host (`.github/workflows/v2-host-ci.yml`)

Chạy trên `ubuntu-24.04` (Rust 1.100.0 ghim + rustfmt/clippy, Python ≥ 3.11 có `tomllib`,
`fetch-depth: 0` cho luật K6 ancestry):

1. `cargo fmt --manifest-path v2/Cargo.toml --all --check`
2. `cargo clippy --manifest-path v2/Cargo.toml --workspace --all-targets -- -D warnings`
3. `./v2/bin/v2 check` (fmt/lint + R1..R5/E1 + checker + selftest + L0 compile)
4. `cargo test --workspace` trong `v2/` (L0 host: mọi binary test + doc-test)

**QEMU KHÔNG chạy trên CI** — ghi rõ trong workflow: T1/T2/nightly/soak cần
`qemu-system-{riscv64,aarch64,x86_64}` + OpenSBI `fw_dynamic` + ảnh kernel và ngưỡng D2-72/D2-74
(`-icount shift=6`, 20 s/boot) trên máy thật; CI **không thay** T2. Lý do kỹ thuật: `selfbreak.sh`
dùng `shasum` (macOS) và các bước QEMU cần ảnh — nên CI chỉ chạy phần host.

---

## 8. Rủi ro & việc chưa làm

1. **D2-151 (cắt điện)**: đã cách ly `49/50` vào làn `sau` **chạy một mình** (40,6 s). Đánh đổi
   ~35 s wall để **không** tăng xác suất flake do chính cổng gây tải. Tải **ngoài** (sub-agent
   khác) vẫn có thể làm đỏ oan — khi đó phân loại theo AGENTS §4.5, **không** retry để lấy xanh.
2. **Trần QEMU = 3**: `sau2` (44/45), `sau5` (56/57), `sau6` (59/60/63). Đúng AGENTS §9; nếu sau này
   thêm làn QEMU thứ 4 phải **không** vượt trần — cân nhắc trước khi thêm bước.
3. **Bước 44 (220 s) vẫn là critical path** của lượt sau. Muốn giảm nữa phải **tách** phép kiểm
   của `selftest.sh`/`ktest.sh` (build 8 ảnh + 1 boot + 21 ca) hoặc rút gọn build — việc này **đổi
   nội dung phép kiểm/harness**, ngoài phạm vi KT-333a; đề xuất mở task riêng nếu cần.
4. **Hằng số đếm bước**: thêm/bớt bước hợp lệ **bắt buộc** cập nhật
   `CHECK_STEPS_EXPECTED`/`GATE_FAST_EXTRA_EXPECTED` (và T1 nếu cần) — đây là **tripwire có chủ ý**
   (D2-235(5), AGENTS §6.1).
5. **6 làn nền** có thể nhiều hơn số nhân trên máy nhỏ; T2 vẫn chỉ chạy trên máy đo 12 nhân
   (AGENTS §6.1), T1/CI không đổi hành vi. Không nới ngưỡng nào (timeout, seed, số ca).
6. **Chưa làm**: (a) chưa tách bước 44; (b) chưa đo lại trên máy **rảnh hoàn toàn** (máy dùng
   chung, xem §1) — khi bàn giao lead nên chạy final T2 trên máy rảnh để có số sạch; (c) chưa chạy
   thử workflow GitHub (không có quyền/không cần trong vòng task) — chỉ kiểm YAML cú pháp.

**Không sửa** `v2/kernel/**`, `v2/lib/**`, `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`.
Cây sau commit: sạch (log `kt78-guard` sinh bởi bước 45 đã xoá trước khi commit).

---

## 9. Tệp thay đổi

| Tệp | Thay đổi |
|---|---|
| `v2/bin/v2` | làn `main`/`A..F`/`sau`/`sau2..sau6` + cân tải theo số đo; fail-closed trùng id + hằng số đếm bước + `# gate-steps`/`# gate-phase`/`# gate-critical-path`; chú thích an toàn từng nhóm dời làn. **Không** đổi thân hàm bước nào (nguyên văn lệnh kiểm). |
| `.github/workflows/v2-host-ci.yml` | CI host fmt/clippy/`v2 check`/`cargo test`, ghi rõ **không QEMU**. |
| `v2/agent/JOURNAL/kt333a-t2wall.md` | nhật ký này (bảng 69 bước, critical path, nguyên nhân, trước/sau, đột biến, rủi ro). |

Log thô (không commit — ngoài cây): `/tmp/kt333a-logs/run1-before.log`, `run2-after.log`,
`mut-dup-new.log`, `mut-drop-new.log`, `mut-dup-old.log`, `reclass-crash.log`, `check-final.log`.

---

## 10. Sự cố đo song song 15:31–15:45 (làm **mất hiệu lực** số đo — đọc trước khi trích dẫn)

### 10.1 Chuyện gì xảy ra

Trong lúc worktree này chạy **final T2** (`/tmp/kt333a-logs/run4-final.log`, bắt đầu 15:25:32), lead
khởi động **một `gate --fast` thứ hai** trên **cùng repo** ở checkout chính
(`/Users/mps/dvchd/ViFastOs`, log `/tmp/t2-r26.log`, bắt đầu ~15:33). Hai cổng chồng nhau
15:31–15:45. Hệ quả:

* Cổng của lead: `v2 gate --fast: FAIL` (771,8 s, RC=1).
* Cổng của tôi (run4): **5 bước đỏ oan** — `selftest qemu guard (KT-78)`,
  `ca âm git-guard (KT-85)`, `ca âm git-guard song song (R5)`, `ca âm git-guard b (KT-85b)`,
  `QEMU x86_64 (K5' bước 1)`; wall 751,6 s.

### 10.2 Nguyên nhân (bằng chứng trong log)

1. **git-guard dùng chung `.git`**: mỗi ca tạo/xoá `git worktree` + nhánh tạm trong `.git` của repo;
   hai cổng cùng lúc ⇒ `KẾT LUẬN: FAIL — số worktree KHÁC trước (77 → 78)` (run4, bước 52) và
   `not ok - selftest-parallel: lần 1 exit 1 (mong 0)` (bước 53). Đây là **tranh chấp tài nguyên
   git**, không phải logic sai.
2. **QEMU chồng tải + logdir cố định dùng chung**: `run-x86_64.sh`/`run-aarch64.sh` mặc định
   `LOGDIR=${TMPDIR}/k5-x86_64` / `k4-aarch64` (**không** theo `V2_RUN_TMP`) ⇒ hai cổng ghi cùng
   thư mục log/ảnh; `guard_selftest` thấy `guard-timeout: timeout kích hoạt nhưng còn 0 QEMU
   (baseline=1)` và `infra-orphan … qemu_ppid1=1 … verdict=KHONG_HOP_LE` vì QEMU của cổng kia.
   `QEMU x86_64` chỉ đạt `30/32 kiểm sạch` do boot dưới tải chồng.
3. Đây **đúng** là lý do luật vận hành "QEMU tuần tự — không chạy 2 `gate --fast` cùng lúc"
   (AGENTS §9) và tinh thần **D2-116/D2-151**: đo dưới tải chồng thì **không kết luận**.

### 10.3 Kết luận & hiệu lực số đo

* **KHÔNG** dùng cặp số **905,3 s → 507,9 s** (và mọi bảng làn ở §3/§5) làm **bằng chứng** nghiệm
  thu. Chúng chỉ là **quan sát thô** về cấu trúc (chỗ nghẽn, tỉ lệ song song), đo trong điều kiện
  tải chồng và có 1 lượt đỏ do flake D2-151.
* Số **hợp lệ** phải do **một mình lead** đo lại trên `main` (máy rảnh, `qemu_foreign=0`), theo
  đúng D2-116/D2-151.
* Phần **không phụ thuộc wall-time** vẫn giữ nguyên hiệu lực: **69 = 69 bước, `diff` tên rỗng**
  (§5.2), **đột biến** trùng-id/bỏ-bước đều bị chặn (§6), **CI host** (§7), và **mã nguồn phép
  kiểm không đổi** (chỉ đổi làn).
* Bài học vận hành: trước khi chạy T2 phải kiểm `pgrep -f 'v2/bin/v2 gate'` **trên mọi worktree**
  và `ps aux | grep '[q]emu-system-' | wc -l` = 0; hai cổng cùng repo là **cấm** (git common dir).

---

## 11. LEad đo lại **một mình** trên `main` (số hợp lệ duy nhất)

Sau khi máy về 0 QEMU (agent `t2wall` đã `kill` đúng PID gate của nó, **không** `pkill` toàn cục),
lead chạy **một** `./v2/bin/v2 gate --fast` trên `main`:

```text
v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
# step-time-total   558875 ms  TỔNG gate --fast (T2)
```

- **PASS 72/72 bước OK, 0 FAIL** (69 bước của KT-333a + 3 bước 73/74/75 của KT-333b).
- **558,9 s**, so với các lượt T2 sạch trước đó trên cùng máy: `765,8 s` (`d9aa1f1a`), `771,8 s`
  (lượt đỏ do đo song song), `820,1 s` (`bfbfafd5`), `828,1 s` (`98ee4886`).
  ⇒ **giảm ~27–33 %** so với ba lượt sạch gần nhất. Đây **là** con số dùng được (máy rảnh, không
  QEMU lạ; `qemu_foreign=0`).
- Con số `905,3 → 507,9 s` (`§0`) **bị vô hiệu** như `§10` đã ghi — **không** trích dẫn.

### 11.1 Sửa kèm khi gộp (cảnh báo §9 của sub-agent, lead áp dụng)

1. `CHECK_STEPS_EXPECTED` **41 → 44** (id 1..40 + 72 + 73/74/75) — nếu không, cổng đỏ ngay
   (đã xảy ra thật: `--- LỖI: check đăng ký 47 bước, kỳ vọng 44` khi lead gộp **trùng** ba dòng
   73/74/75; đã xoá bản trùng).
2. `lane_fails 72` → **`lane_fails 999`** ở `do_gate_fast` **và** `do_gate_dev`: dải cũ **không**
   đếm id 73–75 ⇒ ba bước mới đỏ có thể **xanh oan**. Đã sửa cả hai chỗ.
3. Ba bước mới xếp: 73 `main` (đọc `Cargo.lock` + ghi `THIRD-PARTY.md`), 74/75 làn `B` (chỉ đọc).

### 11.2 Bằng chứng độc lập (không phụ thuộc wall-time)

- **69 bước của KT-333a**: `diff` danh sách tên bước (mã ↔ log trước ↔ log sau) **rỗng**.
- Đột biến **trùng id** ⇒ `--- LỖI: trùng id lane_step:17` exit 1; mã cũ **PASS im lặng** với
  chỉ 40 `.rc` và không có `72.log` ⇒ chứng minh cơ chế chống trùng id có răng.
- Đột biến **bỏ bước** ⇒ `--- LỖI: check đăng ký 40 bước, kỳ vọng 41` exit 1 (fail-closed).
