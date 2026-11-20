# KT-242 (P3) — làn song song + bảng thời gian cổng: đo trước/sau

**Ngày**: 2026-11-20 (giờ máy Asia/Tokyo) · **Người làm**: Agent 2 luồng "Hạ tầng hiệu năng"
**HEAD**: `21cc75cd` (worktree `/tmp/kt242-s3`, nhánh `task/kt242-lanes`) — **cùng cây** cho cả
trước và sau; bản vá chạm `v2/bin/v2` + 10 script đổi đường `/tmp` (xem
[`kt242-lanes-inventory.md`](kt242-lanes-inventory.md) §2).
**Máy**: 12 nhân (`hw.ncpu=12`), macOS, cargo/rustc `1.100.0` (ghim theo `v2/rust-toolchain.toml`).

Nhãn số liệu: mọi số dưới đây là **`[ĐO KT-242]`** — đo bằng `/usr/bin/time -p` bọc ngoài
`./v2/bin/v2 gate …`; log thô ở [`kt242-lanes-logs/`](kt242-lanes-logs/).

---

## 0. TL;DR

| Lượt | Wall trước | Wall sau | Giảm | CPU trước (user+sys) | CPU sau | Hệ số song song (CPU/wall) |
|---|---:|---:|---:|---:|---:|---:|
| `gate --dev` (T1) | **497,8 s** | **310,4 s** | **−187,4 s (−37,6 %)** | 649,7 s | 654,4 s | 1,30 → **2,11** |
| `gate --fast` (T2) | **722,8 s** | **494,5 s** | **−228,2 s (−31,6 %)** | 901,1 s | 910,8 s | 1,25 → **1,84** |

**CPU gần như không đổi** (+0,7 % / +1,1 %) ⇒ **cùng khối lượng việc**, chỉ đổi *đồng thời*.
Đó là bằng chứng mạnh nhất rằng **không bước nào bị bỏ, không ngưỡng nào bị nới**.

**`v2 check` (lệnh agent chạy nhiều nhất trong ngày)**: pha `check` **199,5 s → 72,5 s**
(−63,7 %; đo riêng `v2 check` sau bản vá: wall 72,5 s, CPU 222,0 s, 3 làn nền A/B/C, critical
path 71,0 s). Đây là con số quan trọng nhất cho vòng phát triển.

Số bước: T1 **46 bước OK + 8 bước T2-only bỏ qua** (trước: 44 + 8 — chênh do 2 bước cũ in
`OK` kèm đuôi nên không khớp mẫu đếm; **tập bước y hệt**, xem §5); T2 **53 bước OK** (trước 51,
cùng lý do). Tổng luôn là **54 bước**.

---

## 1. Cách đo và điều kiện máy

- Mỗi lượt: `/usr/bin/time -p bash ./v2/bin/v2 gate --dev|--fast`; ghi `real/user/sys` và
  wall nội bộ của cổng (`# gate-wall`). Hai số lệch < 1 s.
- **Tuần tự**: không chạy hai cổng cùng lúc. Trước: `--dev` rồi `--fast`; sau: `--dev` rồi `--fast`.
- **Cùng cây, cùng worktree, cùng toolchain, cùng `v2/target` + `v2/kernel/target` đã ấm.**
  Trước khi đo có **một cặp lượt khởi động** (`warm-dev`, `warm-fast`) để nạp cache build; số của
  cặp đó **không** dùng để so sánh (log ở `kt242-lanes-logs/warm-*.log`).
- **Máy có việc khác chạy song song** (agent khác trong cùng phiên). Vì vậy mỗi lượt ghi loadavg
  đầu→cuối, và kết luận dựa trên **cặp** lượt + **bảng từng bước**, không dựa vào một con số tổng.

| Lượt | loadavg đầu → cuối | wall | user | sys |
|---|---|---:|---:|---:|
| `before-dev` | 5,89 → 9,17 | 497,8 s | 537,74 s | 112,00 s |
| `after-dev` | 3,38 → 7,31 | 310,4 s | 538,07 s | 116,31 s |
| `before-fast` | 9,17 → 11,73 | 722,8 s | 643,28 s | 257,78 s |
| `after-fast` | 7,31 → 4,57 | 494,5 s | 647,91 s | 262,88 s |

**Tách phần do tải máy khỏi phần do làn** — dùng các bước **độc quyền** (không đổi cách chạy)
làm nhiệt kế:

| Bước độc quyền | before-dev | after-dev | before-fast | after-fast |
|---|---:|---:|---:|---:|
| `L0 test` | 117,3 s | 97,4 s | 89,0 s | 87,4 s |
| `QEMU smoke` (T1) | 128,7 s | 87,9 s | — | — |
| `selftest runner ktest` (T2) | — | — | 122,7 s | 122,0 s |
| `selftest qemu guard` | — | — | 22,1 s | 21,9 s |
| `cắt điện giữa lúc ghi` | — | — | 19,6 s | 20,1 s |
| `ca âm git-guard` ×3 | — | — | 73,9 s | 57,7 s |
| **tổng bước độc quyền** | **280,5 s** | **211,3 s** | **486,4 s** | **484,0 s** |

⇒ **T2: tải máy tương đương** (486,4 vs 484,0 s, lệch 0,5 %) ⇒ **−228 s của T2 gần như hoàn
toàn do làn song song**. **T1: lượt `after` nhẹ hơn ~25 %** (280,5 → 211,3 s) ⇒ trong −187 s có
**~69 s là do tải**, phần còn lại **~112 s là do làn** (khớp §4: pha `check` 199,5 → 87,6 s).

---

## 2. TRƯỚC — bảng thời gian từng bước

### TRƯỚC — `gate --dev` (T1): 32 bước có `# step-time`

| Bước | ms |
|---|---:|
| QEMU smoke (T1: 7 ảnh × 1 boot) | 128692 |
| L0 test | 117338 |
| selftest wake paths (KT-151) | 38222 |
| wake paths (KT-151: một cơ chế wake) | 27558 |
| wake paths --strict (KT-155: allowlist) | 27516 |
| luật máy K6 (KT-80: 1,2,3,7,8) | 19111 |
| selftest build-inputs (KT-128/134: a..h) | 18983 |
| wake theo dep-info (KT-217: lớp 2) | 15254 |
| selftest pre-measure (KT-95) | 11064 |
| L0 kernel workspace (KT-148) | 7990 |
| algo= nghiêm ngặt ở đường đo (KT-145) | 7791 |
| selftest pin-artifact (KT-84) | 7600 |
| selftest shell-edge (KT-108) | 4986 |
| api boundary (KT-241) | 4601 |
| clippy | 4527 |
| test có răng (KT-163) | 4128 |
| fmt | 3277 |
| build (L0 compile) | 1813 |
| selftest wiring (KT-134) | 1683 |
| selftest ghim thuật toán (KT-134) | 1664 |
| selftest wake dep-info (KT-217) | 1405 |
| selftest danh tính ảnh (KT-145: l1..l5) | 1053 |
| cổng selftest có cổng gọi (KT-134) | 999 |
| self-test luật | 841 |
| selftest test có răng (KT-163) | 555 |
| selftest t26 (KT-149: g1..g4) | 406 |
| selftest task-target (KT-84) | 384 |
| selftest bảng đối chiếu (KT-134) | 277 |
| bảng đối chiếu số ↔ khối scope= (KT-134) | 207 |
| selftest counter-evidence (KT-80) | 204 |
| ghim thuật toán dấu vân tay (KT-134) | 196 |
| selftest budget (KT-88) | 93 |
| **tổng các bước** | **460418** |

### TRƯỚC — `gate --fast` (T2): 39 bước có `# step-time`

| Bước | ms |
|---|---:|
| selfbreak 5 luật K6 (KT-80) | 125883 |
| selftest runner ktest (KT-54) | 122712 |
| L0 test | 89003 |
| ca âm git-guard b (KT-85b) | 39782 |
| selftest wake paths (KT-151) | 38015 |
| wake paths --strict (KT-155: allowlist) | 28117 |
| wake paths (KT-151: một cơ chế wake) | 27183 |
| luật máy K6 (KT-80: 1,2,3,7,8) | 25965 |
| selftest qemu guard (KT-78) | 22053 |
| selftest build-inputs (KT-128/134: a..h) | 21629 |
| ca âm git-guard song song (R5) | 20867 |
| cắt điện giữa lúc ghi (KT-136) | 19604 |
| wake theo dep-info (KT-217: lớp 2) | 15494 |
| ca âm git-guard (KT-85) | 13209 |
| selftest persist đỏ-trước (KT-75) | 7766 |
| L0 kernel workspace (KT-148) | 7416 |
| selftest pre-measure (KT-95) | 7027 |
| clippy | 5804 |
| selftest shell-edge (KT-108) | 5565 |
| selftest pin-artifact (KT-84) | 5490 |
| algo= nghiêm ngặt ở đường đo (KT-145) | 5452 |
| fmt | 5102 |
| test có răng (KT-163) | 4598 |
| api boundary (KT-241) | 4428 |
| self-test luật | 1956 |
| selftest wiring (KT-134) | 1663 |
| selftest ghim thuật toán (KT-134) | 1560 |
| build (L0 compile) | 1502 |
| selftest wake dep-info (KT-217) | 1369 |
| selftest danh tính ảnh (KT-145: l1..l5) | 1141 |
| cổng selftest có cổng gọi (KT-134) | 1024 |
| selftest task-target (KT-84) | 425 |
| selftest t26 (KT-149: g1..g4) | 412 |
| selftest test có răng (KT-163) | 395 |
| selftest bảng đối chiếu (KT-134) | 274 |
| selftest counter-evidence (KT-80) | 226 |
| bảng đối chiếu số ↔ khối scope= (KT-134) | 214 |
| ghim thuật toán dấu vân tay (KT-134) | 207 |
| selftest budget (KT-88) | 96 |
| **tổng các bước** | **680628** |

---

## 3. SAU — bảng thời gian từng bước

### SAU — `gate --dev` (T1): 46 bước có `# step-time`

| Bước | ms |
|---|---:|
| L0 test | 97429 |
| QEMU smoke (T1: 7 ảnh × 1 boot) | 87946 |
| selftest wake paths (KT-151) | 56953 |
| wake paths (KT-151: một cơ chế wake) | 40313 |
| wake paths --strict (KT-155: allowlist) | 36637 |
| luật máy K6 (KT-80: 1,2,3,7,8) | 29018 |
| wake theo dep-info (KT-217: lớp 2) | 27868 |
| selftest build-inputs (KT-128/134: a..h) | 27520 |
| fixture âm của máy kiểm báo cáo | 17905 |
| luật cấu trúc R1..R5 | 15186 |
| selftest shell-edge (KT-108) | 7818 |
| algo= nghiêm ngặt ở đường đo (KT-145) | 7277 |
| selftest pre-measure (KT-95) | 6969 |
| L0 kernel workspace (KT-148) | 6890 |
| test có răng (KT-163) | 6313 |
| clippy | 5773 |
| selftest pin-artifact (KT-84) | 4835 |
| fmt | 4737 |
| api boundary (KT-241) | 4702 |
| selftest wiring (KT-134) | 2830 |
| báo cáo R1-v2 (Khối E) | 2772 |
| gen fixtures | 2348 |
| selftest danh tính ảnh (KT-145: l1..l5) | 2274 |
| build (L0 compile) | 2098 |
| selftest wake dep-info (KT-217) | 2066 |
| selftest ghim thuật toán (KT-134) | 1893 |
| self-test luật | 1579 |
| abi fixtures | 1347 |
| cổng selftest có cổng gọi (KT-134) | 1269 |
| selftest bảng đối chiếu (KT-134) | 1149 |
| L1 (sim 1.000 seed) | 952 |
| scn fixtures (KT-04c) | 950 |
| idl fixtures | 881 |
| selftest test có răng (KT-163) | 833 |
| selftest task-target (KT-84) | 584 |
| selftest t26 (KT-149: g1..g4) | 558 |
| idl-gen-check (KT-69: stub sinh từ .vidl) | 501 |
| gen-check (KT-05: code + docs sinh từ spec) | 463 |
| abi reserved (KT-04b) | 361 |
| selftest counter-evidence (KT-80) | 312 |
| ghim thuật toán dấu vân tay (KT-134) | 238 |
| bảng đối chiếu số ↔ khối scope= (KT-134) | 213 |
| kịch bản .scn (KT-04c) | 204 |
| abi spec | 171 |
| golden trace (D2-43) | 140 |
| selftest budget (KT-88) | 128 |
| **tổng các bước** | **521203** |

### SAU — `gate --fast` (T2): 53 bước có `# step-time`

| Bước | ms |
|---|---:|
| selfbreak 5 luật K6 (KT-80) | 141281 |
| selftest runner ktest (KT-54) | 121963 |
| L0 test | 87441 |
| selftest wake paths (KT-151) | 51083 |
| wake paths (KT-151: một cơ chế wake) | 45877 |
| wake paths --strict (KT-155: allowlist) | 43205 |
| luật máy K6 (KT-80: 1,2,3,7,8) | 33656 |
| ca âm git-guard b (KT-85b) | 31066 |
| selftest build-inputs (KT-128/134: a..h) | 30483 |
| fixture âm của máy kiểm báo cáo | 21916 |
| selftest qemu guard (KT-78) | 21872 |
| cắt điện giữa lúc ghi (KT-136) | 20115 |
| wake theo dep-info (KT-217: lớp 2) | 19595 |
| luật cấu trúc R1..R5 | 17947 |
| ca âm git-guard song song (R5) | 14004 |
| ca âm git-guard (KT-85) | 12663 |
| selftest persist đỏ-trước (KT-75) | 9533 |
| selftest shell-edge (KT-108) | 9403 |
| clippy | 7702 |
| L0 kernel workspace (KT-148) | 6579 |
| selftest pre-measure (KT-95) | 6371 |
| test có răng (KT-163) | 6246 |
| algo= nghiêm ngặt ở đường đo (KT-145) | 5839 |
| selftest pin-artifact (KT-84) | 5242 |
| fmt | 4676 |
| api boundary (KT-241) | 4162 |
| báo cáo R1-v2 (Khối E) | 3172 |
| cổng selftest có cổng gọi (KT-134) | 2619 |
| selftest wiring (KT-134) | 2436 |
| selftest wake dep-info (KT-217) | 2334 |
| selftest ghim thuật toán (KT-134) | 2257 |
| scn fixtures (KT-04c) | 2168 |
| selftest danh tính ảnh (KT-145: l1..l5) | 1525 |
| build (L0 compile) | 1524 |
| selftest bảng đối chiếu (KT-134) | 1404 |
| self-test luật | 1332 |
| gen fixtures | 1118 |
| L1 (sim 1.000 seed) | 987 |
| selftest test có răng (KT-163) | 867 |
| abi fixtures | 758 |
| selftest t26 (KT-149: g1..g4) | 620 |
| selftest task-target (KT-84) | 592 |
| idl fixtures | 468 |
| golden trace (D2-43) | 401 |
| ghim thuật toán dấu vân tay (KT-134) | 323 |
| kịch bản .scn (KT-04c) | 321 |
| selftest counter-evidence (KT-80) | 316 |
| abi spec | 310 |
| gen-check (KT-05: code + docs sinh từ spec) | 240 |
| bảng đối chiếu số ↔ khối scope= (KT-134) | 200 |
| idl-gen-check (KT-69: stub sinh từ .vidl) | 192 |
| abi reserved (KT-04b) | 163 |
| selftest budget (KT-88) | 119 |
| **tổng các bước** | **808686** |

---

## 4. Critical path

**Trước** (tuần tự 100 %): critical path = **tổng mọi bước** = **460,4 s** (T1) / **680,6 s** (T2)
— cộng thêm ~37 s / ~42 s chi phí cố định (khởi động cargo, các bước chưa có `# step-time` cũ).

**Sau**: `main` và các làn nền chạy **đồng thời**, rồi `sau` chạy tiếp. Cổng in:

```
# gate-wall <ms>            tổng thời gian thực
# gate-cpu user=… sys=…     CPU thật (times builtin, shell + mọi con đã wait)
# gate-lanes N làn nền (…) + main + sau
# gate-lane-time main=… sau=… A=… B=… C=… D=…
# gate-critical-path <ms> (chuỗi dài nhất: làn <X> + sau)
```

`gate-critical-path` = `max(main, làn nền dài nhất) + sau` — **cận trên** của chuỗi dài nhất
(main và làn nền chạy chồng nhau; `sau` nối tiếp). Số đo được:

| Lượt sau | main | làn nền dài nhất | sau | critical path (cổng in) | wall |
|---|---:|---:|---:|---:|---:|
| `after-dev` | 205,4 s | **A = 87,6 s** | 95,0 s | **300,4 s** | 309,6 s |
| `after-fast` | 192,9 s | **D = 141,3 s** (`selfbreak`) | 237,1 s | **430,0 s** | 493,4 s |

Chuỗi phụ thuộc thật (đọc từ bảng bước):

- **T1**: `check` (pha 1) = **87,6 s** (làn A: `selftest wake paths` 57,0 + `K6` 29,0)
  → `L0 test` 97,4 + `L0 kernel` 6,9 + `L1` 1,0 + `pre-measure` 7,0 + `pin-artifact` 4,8 +
  `algo-strict` 7,3 = **124,4 s** → `sau` (QEMU smoke 87,9 + `api boundary` 4,3 + `build` 1,5)
  = **93,9 s**. Tổng **305,9 s** ≈ wall 309,6 s.
- **T2**: `check` = **141,3 s** (làn D: `selfbreak` — chạy song song với 3 làn checker)
  → `main` cổng = **112,4 s** → `sau` = **237,2 s** (`selftest runner ktest` 122,0 +
  `qemu guard` 21,9 + `persist đỏ-trước` 9,5 + `cắt điện` 20,1 + git-guard 57,7 + `api boundary`
  4,3 + `build` 1,5). Tổng **490,9 s** ≈ wall 493,4 s.

**Nút cổ chai còn lại (T2)**: `sau` = **237 s** — toàn bộ là QEMU (`ktest` 122 + `guard` 22 +
`persist` 30) + git-guard (58). Đây là các bước **bắt buộc độc quyền** (§5), không phải chỗ cắt
được bằng làn.

---

## 5. Làn nào song song, làn nào độc quyền

| Làn | Bước | Vì sao an toàn |
|---|---|---|
| `main` (tuần tự, chạy **chồng** với làn nền) | 1 `fmt`, 2 `clippy`, 3 luật R1..R5, 6 `selftest build-inputs`, 22 `wake theo dep-info`, 40 `L0 test`, 41 `L0 kernel`, 42 `L1`, 45 `pre-measure`, 46 `pin-artifact`, 47 `algo-strict` | Bước ghi `target/` hoặc bước **đo** — chạy một mình trong làn của nó |
| `A` (nền, chỉ đọc) | 4 `self-test luật`, 5 `luật máy K6`, 20 `selftest wake paths` | Không ghi `target/`, không sửa cây, không QEMU |
| `B` (nền, chỉ đọc) | 11 `shell-edge`, 12 `budget`, 13 `counter-evidence`, 14 `task-target`, 16 `selftest bảng đối chiếu`, 17 `wiring`, 18 `cổng selftest có cổng gọi`, 21 `wake paths --strict`, 25 `test có răng`, 26 `selftest test có răng`, 31 `báo cáo` | như trên |
| `C` (nền, chỉ đọc) | 7 `danh tính ảnh`, 8 `t26`, 9 `ghim thuật toán`, 10 `selftest ghim`, 19 `wake paths`, 23 `selftest wake dep-info`, 27–30, 32–38 (abi/scn/report/gen/idl) | như trên |
| `D` (nền, chỉ đọc) | 50 `selfbreak 5 luật K6` | Băm cây thật trước/sau để chứng minh **0 tệp bị sửa** (D2-113F); dựng bản copy trong `mktemp`. Chạy **trong pha `check`**, không đặt cạnh `L0` |
| `sau` (độc quyền) | 15 `bảng đối chiếu scope=`, 24 `api boundary`, 39 `build`, 43 `selftest runner ktest`, 44 `qemu guard`, 48 `persist đỏ-trước`, 49 `cắt điện giữa lúc ghi`, 51–53 git-guard, 54 `QEMU smoke` | **Sửa cây** (probe `api boundary`, `expected.txt` của `selftest runner ktest`, `JOURNAL/kt78-guard/`), **QEMU**, hoặc **sửa git** (worktree/nhánh) |

**Giữ tuần tự vì không an toàn** (đã ghi lý do trong inventory §3): `L0 test` (chứa
`kt30_stress` biên 3 s — "chạy một mình" theo D2-116/D2-141), `L0 kernel workspace` (cargo nặng,
oversubscribe 12 nhân), `L1` (cùng `v2/target`), ba bước **đường đo**, toàn bộ QEMU, git-guard.

---

## 6. Rủi ro còn lại

1. **Tải máy ngoài tầm kiểm soát.** Phiên này có agent khác chạy test song song; T1 lượt `after`
   nhẹ hơn ~25 % (bảng nhiệt kế §1). Vì vậy **T2 là số đáng tin hơn** (tải tương đương), còn T1
   nên đọc là "−37,6 % đo được, trong đó ~112 s do làn và ~69 s do tải".
2. **`selfbreak` chậm đi khi chạy song song**: 125,9 s (tuần tự) → **141,3 s** (làn D). Vẫn lời
   ~126 s vì nó biến mất khỏi chuỗi tuần tự, nhưng nếu sau này thêm bước nặng vào làn D thì
   pha `check` sẽ dài ra.
3. **Cân làn phụ thuộc tải**: A/B/C đang 87,6 / 72,6 / 60,6 s (T1) — lệch ~27 s. Trên máy bận
   hơn, làn nặng nhất có thể vượt `main` và thành critical path. Cân lại bằng cách đổi cột làn
   trong bảng `lane_step` (một dòng mỗi bước), không cần sửa logic.
4. **`gate --fast` không còn dừng ngay ở bước đỏ đầu tiên**: mọi bước vẫn chạy rồi cổng trả 1
   (trước: `|| return 1` dừng ngay). Đổi để giữ **cùng tập bước** khi có lỗi; cổng đỏ vẫn đỏ,
   chỉ tốn thêm thời gian và in nhiều bằng chứng hơn.
5. **`check-selftest-wiring.py` bắt nhầm tên hàm**: hàm bước nào **kết thúc** bằng `selftest`
   bị coi là "script tự kiểm mồ côi" (đã xảy ra thật với `step_*_selftest`). Đã đổi tên thành
   `step_selftest_*`; **đừng** đặt tên hàm bước kết thúc bằng `selftest`.
6. **`V2_RUN_TMP` không tự dọn**: mỗi lượt cổng để lại một `v2run.*` trong `$TMPDIR` (log QEMU
   là bằng chứng). Chưa có chính sách xoá — cần thì thêm sau, đừng xoá trong lúc cổng chạy.
7. **`ktest-proc` từng đỏ oan dưới tải** (một lần trong phiên: 5/10 TAP `shell`/`procsrv`), chạy
   lại **7/7 boot sạch**. Đây là **flake có sẵn** của ảnh `ktest-proc` dưới tải, không do bản vá
   (bản vá không chạm kernel); ghi `T-INFRA` nếu gặp lại.
8. **`selftest runner ktest` (T2) = 122 s** — bước đắt nhất trong `sau`, vì nó boot **7 ảnh**.
   Cắt được bằng cách dùng lại log của `QEMU smoke`, nhưng như vậy **giảm sức kiểm** ⇒ không làm.
