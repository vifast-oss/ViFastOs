# KT-407 — Đại tu Tooling, Parity, `run-common.sh`, vá `guard_selftest` case 9 & Đồng bộ Lockfile (`B3`)

Người làm: **agent `kt407`** (slot 1, worktree `/private/tmp/vifast-wt/1`, nhánh
`task/kt407-tooling` từ `main` @ `16a03de2`), **phiên tiếp nối `kt407b`** (2026-11-25 14:55 JST).
Nguồn: `v2/agent/QUEUE.md` dòng KT-407; `D2-259`/`D2-260` mục 5; `KE-HOACH-KIEN-TRUC.md`
nút thắt 11 (`B3`) + dòng KT-407.

**Ranh giới**: 0 chạm `v2/kernel/`, 0 chạm `v2/user/`, 0 chạm `spec/`, 0 chạm `docs/`.
Không commit / không `git add` (hook `vifast-guard` chặn non-lead) — thay đổi để trong worktree.

## 0. Vào

- Phiên trước (`kt407`) dừng giữa chừng khi gọi AI; worktree đã có 15 tệp sửa + 16 tệp mới.
  Phiên này **không làm lại từ đầu**: đọc `git status`/`git diff` + journal, rồi (a) rà lại toàn bộ
  thay đổi, (b) vá thêm lỗi **`CARGO_TARGET_DIR`** (xem §7), (c) chạy T0/T1 + `.scn` conformance.
- `CARGO_TARGET_DIR=/Users/mps/dvchd/ViFastOs/target-task/slot-1`; mọi lệnh chạy trong worktree.
- Khoá máy đo: T2 của Lead giữ tới **15:05:44 JST** ⇒ trong lúc chờ chỉ đọc/viết mã; T0/T1/QEMU
  chỉ chạy sau khi `bash v2/tools/measure-lock.sh status` = **TRỐNG**.

## 1. Vá T-INFRA `guard_selftest.sh` case 9 (`run_infra_orphan`) — D2-260 mục 5

**Lỗi**: bản cũ so **cứng** `qemu_ppid1=1` toàn cục. Chỉ cần một làn QEMU song song của cổng đã
để lại QEMU mồ côi (hoặc máy đang có QEMU PPID 1 khác) là `qemu_ppid1=2` ⇒ ca ĐỎ oan dù
`qg_infra` vẫn kết luận đúng. Đây đúng chữ ký T-INFRA đã tái hiện 2 lượt T2 ngày 2026-11-25
(khuôn `6cf4640c` đã vá các ca guard/timeout, còn sót ca 9).

**Vá** (`v2/test/boot/guard_selftest.sh`): bốn bất biến **của ca này**, không so tổng toàn máy:

1. `before_ppid1="$(qemu_ppid1)"` đo **trước** khi tạo QEMU mồ côi;
2. PID do ca tạo có `ppid == 1` (đúng là QEMU mồ côi);
3. `verdict == KHONG_HOP_LE` **và** `after_ppid1 >= before_ppid1 + 1` (thay so cứng `=1`);
4. PID của ca **chết** sau `force_kill_pid` (`wait_gone_ms != 999999`) — khuôn `6cf4640c`.

Không nới ngưỡng: nhánh QEMU còn sống / verdict sai / ppid sai vẫn ĐỎ.

**Tự kiểm (đã chạy, §9)**: `--mode guard` 6/6 ca OK. **Tái hiện đúng T-INFRA**: dựng sẵn một QEMU
mồ côi (`ppid=1`) trước khi chạy ⇒ `baseline qemu_ppid1=1`, ca 9 vẫn **OK** với
`qemu_ppid1 1→2 (≥ +1)` — bản cũ sẽ ĐỎ oan (`qemu_ppid1=2`). Log:
`/tmp/kt407b/guard-selftest-orphan.log`.

## 2. Checker `B3` — đồng bộ 2 `Cargo.lock`

`v2/tools/check-lockfile-sync.py` (**mới**): hai workspace Cargo (`v2/Cargo.lock` và
`v2/kernel/Cargo.lock`) phải **cùng tập phiên bản + cùng nguồn** cho **mọi** crate dùng chung.
Fail-closed khi thiếu/rỗng/hỏng lockfile. Parser tối giản (không cần `tomllib`) ⇒ chạy được cả
`python3` 3.9 của máy đo lẫn `python3.12` của cổng.

Nối cổng: `v2/bin/lib/steps_check.sh` (`step_lockfile_sync` + `step_selftest_lockfile_sync`),
`v2/bin/lib/check.sh` (bước **80/81**), `v2/bin/lib/consts.sh` (`CHECK_STEPS_EXPECTED` 44 → **47**;
`gate --fast` 75 → **78**).

## 3. `run-common.sh` — `rc=126` + `log_inventory` + 1 `trap EXIT` + `monitor-sendkey.py`

`v2/test/boot/lib/run-common.sh` (**mới**) — hấp thu nợ `KT-348a`/`KT-348b`:

- `run_common_trap <hàm dọn>` — **một** bộ trap `EXIT/INT/TERM/HUP`; gọi lại chỉ **thay** hàm
  dọn, không chồng trap (bản cũ đặt 3 lần ⇒ chỉ trap cuối chạy — KT-348b).
- `run_common_log_inventory <n> <dir>` — đếm **từng slot** log boot: có mặt + khác rỗng, nêu tên
  tệp + lý do từ `results.txt` (KT-348: 7 boot 814..823 mất tệp vì `ENOSPC` mà không ai đếm).
- `run_common_sendkey_start/wait` — bọc `lib/monitor-sendkey.py`: nối HMP **trước** khi khách in
  `# input: ready`, gửi trên kết nối đã mở sẵn; lỗi ⇒ **`rc=126`** có tên (`RUN_COMMON_KEYS_ERR`),
  phân biệt `124` (treo) / `125` (không khởi động được).
- `--selftest` **11 ca** thuần shell, **không QEMU**; nối cổng bước **82**.

Áp dụng:

| Runner | Thay đổi |
|---|---|
| `soak-aarch64.sh` | source `run-common.sh`; `soak_a64_log_inventory` → gọi `run_common_log_inventory` (giữ tên hàm cho 48 ca selftest); 3 dòng `trap` → `run_common_trap _soak_a64_cleanup` |
| `soak-x86_64.sh` | source; **xoá bộ trap trùng** ở chỗ probe (bản cũ đặt y hệt 2 lần ⇒ trap sau ghi đè trap trước — đúng lớp KT-348b); `run_common_trap _soak_x64_cleanup` |
| `soak.sh` | source; 4 dòng `trap` → `run_common_trap _soak_rv_cleanup` |
| `run-aarch64.sh` | source; **bỏ `send_keys` cũ** (chờ `# input: ready` bằng `grep` rồi mới khởi động `python3` — nợ KT-348b); `boot_full` dùng `run_common_sendkey_start` **ngay sau `qg_start`** + `run_common_sendkey_wait`; thêm `rc=126` có tên; **sửa trap `SOCKDIR` bị `qg_init` ghi đè** (đặt trước `qg_init` ⇒ rò rỉ thư mục socket) bằng **một** `run_common_trap _run_a64_cleanup` sau `qg_init` |

## 4. Tách `check-wake-paths.py` (11.046 dòng) → package `v2/tools/checkers/wake/`

| Tệp | Dòng | Nội dung |
|---|---:|---|
| `checkers/wake/core.py` | 4.789 | hằng số, lexer/token-tree, phân tích Rust, luật `wake`, quét v2/v1 |
| `checkers/wake/selftest.py` | 6.076 | **461 ca** (nguyên văn; chỉ đổi 3 đường dẫn `__file__` + 2 vòng quét call-site sang `core.py`) |
| `checkers/wake/cli.py` | 135 | `main()` + argparse (default root sửa `parents[4]` cho đúng gốc repo) |
| `checkers/wake/__main__.py` | 9 | `python -m checkers.wake` |
| `v2/tools/check-wake-paths.py` | 43 | **shim**: re-export **toàn bộ** namespace `core` (kể cả private) để `check-wake-depinfo.py` nạp như module — không chép luật; xử lý `--selftest` tường minh (KT-134) |

`check-selftest-wiring.py`: thêm `cli.py`/`selftest.py` vào `NGOAI_LE` (mô-đun của package, cổng
gọi qua shim — shim **vẫn** bị kiểm bình thường).

## 5. Tách `kt{58,59,60}_core_parity.rs` (9.229 dòng) theo miền

| Gốc | Tệp mới | Dòng |
|---|---|---:|
| `kt58_core_parity.rs` (2.688) | `kt58_core_parity/main.rs` + `scripts.rs` | 1.005 + 1.691 |
| `kt59_core_parity.rs` (2.262) | `kt59_core_parity/main.rs` + `tests.rs` | 1.443 + 827 |
| `kt60_core_parity.rs` (4.279) | `kt60_core_parity/main.rs` + `tests_ipc.rs` + `tests_kt63.rs` | 1.997 + 1.233 + 1.063 |

Giữ nguyên tên target Cargo (`tests/<tên>/main.rs` ⇒ target `<tên>`), giữ nguyên số test
(4 + 8 + 18). `check-test-teeth.py`: chuyển `fn run` sang `tests_ipc.rs` (trợ giúp **cùng tệp**,
không phải bản đồ toàn cây) + cập nhật khoá allowlist `let _ =` sang tệp mới.

## 6. `.scn` conformance

- **Sim**: `bash v2/test/conformance/run.sh --sim-only` ⇒ `# trace sim: OK (61 dòng, 5 ca —
  22:16 23:19 24:7 25:7 26:12)`, `scn-check: PASS (27 tệp, 0 blocked, 16 nhóm)`.
- **QEMU (riscv64)**: `bash v2/test/conformance/run.sh --boot` ⇒
  `ok - trace-compare: 61/61 dòng khớp, 5/5 ca (22:16 23:19 24:7 25:7 26:12), build=857c0c5479093cbe`.
- **3 arch**: hiện **chỉ riscv64** có runner conformance (`ktest.sh`); `aarch64`/`x86_64` chưa có
  ảnh `ktest` + runner `.scn` (nợ cấu trúc thuộc **KT-411** — crate `vifast-ktest` generic 3 arch).
  Không thể thêm runner 2 arch kia mà 0 chạm `v2/kernel/` ⇒ ghi rõ là **còn nợ**, không tuyên bố
  "3 arch PASS".

## 7. Vá `CARGO_TARGET_DIR` cho **mọi** runner QEMU (phát hiện khi chạy T1)

**Triệu chứng**: T1 đầu tiên (có `CARGO_TARGET_DIR=…/slot-1`) ĐỎ **2 bước QEMU**:
`run-aarch64.sh`/`run-x86_64.sh` báo `không thấy ảnh sản phẩm …/v2/kernel/target/…` trong khi cargo
đặt ảnh ở `$CARGO_TARGET_DIR/<target>/release`. Đây là **ĐỎ giả** đã ghi nhận ở
`JOURNAL/kt403-ratchet.md` §4 ("Mục (i) thuộc phạm vi KT-407 (đồng bộ runner), chưa sửa ở đây").

**Vá** (cùng khuôn `product-aarch64.sh`/`product-x86_64.sh` đã có):
`IMG_DIR="${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET/release"` — **no-op** khi biến không
đặt (đường `main`/T2 giữ nguyên), đúng khi worktree dùng chung target dir của slot.

| Tệp | Đường ảnh |
|---|---|
| `run-aarch64.sh`, `run-x86_64.sh`, `run.sh` | `IMAGE` |
| `ktest.sh` | `IMG_DIR` (9 ảnh ktest) |
| `soak.sh`, `soak-aarch64.sh`, `soak-x86_64.sh` | `IMAGE`/`PRODUCT_IMAGE` |
| `product-riscv64.sh` | `IMG_DIR` |
| `gpu.sh`, `gpu-x86_64.sh`, `input.sh`, `net.sh`, `persist.sh`, `guard_selftest.sh` | `IMAGE`/`NETSRV_IMAGE`/`PRODUCT_IMAGE` |
| `v2/test/conformance/run.sh` | `KT_IMG` (trace-compare) |

Sau vá: T1 **PASS** (lần 2), gồm `QEMU aarch64: OK`, `QEMU x86_64: OK`, `QEMU smoke: OK`.

## 8. Bằng chứng selftest (T0)

| Lệnh | Kết quả |
|---|---|
| `./v2/bin/v2 t0` | **PASS** (1 crate `vifast-sim`: fmt + clippy + 67 unit + toàn bộ integration) |
| `python3.12 v2/tools/check-lockfile-sync.py --selftest` | **10/10 ok** |
| `python3.12 v2/tools/check-lockfile-sync.py --root .` | **PASS** — 14 crate dùng chung cùng phiên bản + nguồn |
| `bash v2/test/boot/lib/run-common.sh --selftest` | **11/11 ok** |
| `python3.12 v2/tools/check-wake-paths.py --selftest` | **461/461 ok** |
| `python3.12 v2/tools/check-wake-paths.py --combined` | `errors=0 warnings=0` |
| `python3.12 v2/tools/check-wake-depinfo.py --root .` | `errors=0 warnings=0` (25,5 s) |
| `python3.12 v2/tools/check-wake-depinfo.py --selftest` | **70/70 ok** |
| `bash v2/test/boot/soak-aarch64.sh --selftest` | **48/48** |
| `bash v2/test/boot/soak-x86_64.sh --selftest` | **58/58** |
| `bash v2/test/boot/ktest.sh --selftest-stack` | **5/5** |
| `python3.12 v2/tools/check-test-teeth.py` | `errors=0 warnings=0` |
| `python3.12 v2/tools/check-test-teeth.py --selftest` | **40/40 ok** |
| `python3.12 v2/tools/check-selftest-wiring.py --selftest` | **9/9 ok** |
| `python3.12 v2/tools/check-selftest-wiring.py --standalone-red` | PASS (44 script, 42 có cổng, 2 ngoại lệ, 0 mồ côi) |
| `cargo test -p vifast-sim --test kt58/kt59/kt60_core_parity` | **4/4 + 8/8 + 18/18** |
| `cargo fmt --all --check` | PASS |
| `bash v2/bin/v2 rules --root v2` | PASS (1700 tên tracked) |
| `bash v2/test/boot/guard_selftest.sh --mode guard` | **6/6 OK** (case 9 `infra-orphan` OK) |
| ↳ tái hiện T-INFRA (dựng sẵn 1 QEMU `ppid=1`) | **6/6 OK**, case 9 `qemu_ppid1 1→2 (≥ +1)` |

## 9. T0 / T1 / T2

- **T0** (`./v2/bin/v2 t0`): **PASS** — `v2 t0: PASS (1 crate; **T0** — KHÔNG phải cổng; …)`.
- **T1** (`./v2/bin/v2 gate --dev`, có `CARGO_TARGET_DIR=…/slot-1`):
  **PASS** — `# gate-steps registered=79 ran=65 skipped=14 failed=0`,
  `# step-time-total 1065836 ms TỔNG gate --dev (T1)`,
  `v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; …)`.
  Log: `/tmp/kt407b/t1-gate-dev-2.log`. (Lượt T1 đầu — trước vá §7 — ĐỎ đúng 2 bước QEMU
  `aarch64`/`x86_64` vì `CARGO_TARGET_DIR`; log `/tmp/kt407b/t1-gate-dev.log`.)
- **T2** (`./v2/bin/v2 gate --fast`): **Lead** chạy trên ứng viên tích hợp (D2-256) — chưa chạy
  trong worktree này.
- **`.scn` conformance**: sim PASS + QEMU riscv64 PASS (xem §6); 2 arch còn lại **còn nợ** (KT-411).

## 10. Còn nợ / ghi chú

- **T2** chưa chạy trong worktree (Lead gộp lô + chạy trên ứng viên tích hợp).
- **`.scn` conformance 3 arch**: mới đủ **riscv64** (sim + QEMU). `aarch64`/`x86_64` cần crate
  `vifast-ktest` generic + runner (KT-411) — ngoài phạm vi "0 chạm `v2/kernel/`" của task.
- **Tổ chức `v2/tools/{checkers,build,ops,crates}/`** (mục 1 của QUEUE): mới mở `checkers/` cho
  `wake/`; **chưa** di chuyển 30 tệp còn lại — đúng kế hoạch `KE-HOACH-KIEN-TRUC.md` dòng KT-407
  ("Lead cập nhật caller `v2/bin/lib/` lúc merge"). Di chuyển cơ học 30 tệp mà không có caller sẽ
  làm T1 đỏ; để Lead quyết thứ tự.
- **`not ok - selftest: netsrv ca âm có dòng PANIC 'xong' …`** trong `net.sh --red-selftest`:
  **nền có sẵn** (đã ghi ở `JOURNAL/kt406-uart-irq.md` §5 mục 2; `net.sh` không bị task này sửa;
  script exit 0 nên bước cổng vẫn OK).
- **Phạm vi ghi**: mục (3) tách parity bắt buộc chạm `v2/sim/vifast-sim/tests/` — ngoài danh
  sách `v2/tools/`/`v2/test/boot/`/`v2/bin/lib/` của chỉ đạo, nhưng đúng nguyên văn QUEUE
  ("0 chạm `v2/kernel/` hay `v2/user/`"). Đã giữ 0 chạm kernel/user.
- **`v2/test/conformance/run.sh`** cũng được vá `CARGO_TARGET_DIR` (§7) — ngoài `v2/test/boot/`
  nhưng cùng lớp lỗi và cần để chạy `.scn` conformance trong worktree.
