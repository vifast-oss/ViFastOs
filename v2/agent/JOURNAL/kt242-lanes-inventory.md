# KT-242 (P3) — inventory tài nguyên từng bước cổng

**Ngày**: 2026-11-20 (giờ máy Asia/Tokyo) · **Người làm**: Agent 2 luồng "Hạ tầng hiệu năng"
**HEAD lúc làm**: `21cc75cd` (worktree `/tmp/kt242-s3`, nhánh `task/kt242-lanes`)
**Máy**: 12 nhân (`hw.ncpu=12`), macOS, cargo/rustc `1.100.0` (ghim theo `v2/rust-toolchain.toml`).

Nhãn số liệu: mọi số trong tệp này là **`[ĐO KT-242]`** — đo trực tiếp bằng lệnh ghi kèm trong
`JOURNAL/kt242-lanes-timing.md`; không chép tay từ tài liệu.

---

## 0. Vì sao phải lập inventory

Cổng `v2/bin/v2` chạy **53 bước tuần tự** trên máy 12 nhân. Đo KT-110 (`real 407 s, user 277 s`)
cho thấy cổng chỉ dùng **~0,7 nhân**. Muốn chạy song song **mà không giảm sức kiểm**, phải biết
từng bước **đọc gì / ghi gì** để chỉ đưa bước **chỉ đọc** vào làn song song, và giữ **độc quyền**
cho bước ghi `target/`, sửa cây, QEMU, hoặc đo thời gian.

Ba tài nguyên tranh chấp thật trên máy này:

1. **`v2/target/`** (workspace host) và **`v2/kernel/target/`** (workspace kernel) — hai thư mục
   **khác nhau**; cargo khoá theo từng thư mục nên hai workspace *có thể* build song song, nhưng
   bước test nhạy thời gian thì không được chạy cạnh bất cứ thứ gì (D2-116/D2-141).
2. **Cây nguồn** — ba bước **sửa cây**: `api boundary` (chèn tệp probe vào crate thật),
   `selftest runner ktest` (phá rồi trả lại `v2/test/boot/expected.txt`), `selftest qemu guard`
   (ghi `v2/agent/JOURNAL/kt78-guard/`). Không được chạy checker trên cây đang bị sửa.
3. **`/tmp/<đường cố định>`** — nhiều script dùng chung một đường; hai cổng ở hai worktree (hoặc
   hai bước song song) ghi đè lên nhau. Xem §2.

---

## 1. Bảng inventory 53 bước

Cột **làn** là cách xếp của bản vá này (`main` = tuần tự, chạy song song với làn nền;
`A/B/C` = làn nền **chỉ đọc** của `check`; `D` = `selfbreak` — bước duy nhất của `gate --fast`
được chạy song song, và nó chạy **trong lượt `check`** (chỉ đọc cây thật); `sau` = độc quyền,
chạy sau khi mọi làn nền xong).

### 1.1 `check` (chạy trong cả T1 và T2) — 39 bước

| # | Bước | Đọc | Ghi | Làn | Vì sao |
|---:|---|---|---|---|---|
| 1 | `fmt` | toàn bộ `*.rs` | — (`--check`) | main | chỉ đọc, nhưng rẻ; giữ ở main |
| 2 | `clippy` | cây nguồn + `v2/target` | **`v2/target/`** | main | ghi target ⇒ độc quyền với cargo khác |
| 3 | luật cấu trúc R1..R5 | `kernel lib sim user std boot test tools spec` | `mktemp` (`v2rules.*`) | main | tiến trình bash **mới** (KT-207: heap sạch sau clippy) |
| 4 | self-test luật | `v2/test/rules/fixtures/`, `v2/bin/v2` | `mktemp` | A | chỉ đọc |
| 5 | luật máy K6 | `v2/docs/`, `v2/agent/reports/`, `v2/agent/JOURNAL/` (log `scope=`), git | cache `$TMPDIR/kt80-ancestry-*.json` | A | chỉ đọc; cache chỉ là tối ưu wall |
| 6 | selftest build-inputs | cây nguồn | `mktemp` (`vbrev-selftest.*`) | B | chỉ đọc |
| 7 | selftest danh tính ảnh | `build-rev.sh`, `v2/artifacts/` | `mktemp` (`kt145-identity-selftest.*`) | C | chỉ đọc |
| 8 | selftest t26 | `t26.sh`, `expected.txt` | `mktemp` (`kt149-t26-selftest.*`) | C | chỉ đọc |
| 9 | ghim thuật toán | `v2/tools/algo-pin.txt`, `build-rev.sh` | — | C | chỉ đọc (`--update` mới ghi) |
| 10 | selftest ghim thuật toán | như #9 | `mktemp` | C | chỉ đọc |
| 11 | selftest shell-edge | `shell-edge.sh` | `mktemp` (`kt108-selftest.*`) | B | chỉ đọc |
| 12 | selftest budget | `budget.sh` | — (fixture trong heredoc) | B | chỉ đọc |
| 13 | selftest counter-evidence | `counter-evidence.sh` | `mktemp` (`kt80-ce.*`) | B | chỉ đọc |
| 14 | selftest task-target | `task-target.sh` | `mktemp` (`task-target-selftest.*`) | B | chỉ đọc |
| 15 | bảng đối chiếu số ↔ khối `scope=` | `v2/docs/`, `v2/agent/reports/`, `v2/agent/JOURNAL/`, git | **`v2/agent/JOURNAL/scope-table-skip-count.txt`** (repo) | sau | **ghi cây** ⇒ độc quyền |
| 16 | selftest bảng đối chiếu | như #15 | `tempfile` | B | chỉ đọc |
| 17 | selftest wiring | toàn bộ `v2/` (script) | `tempfile` (repo giả) | B | chỉ đọc |
| 18 | cổng selftest có cổng gọi | toàn bộ `v2/` | `tempfile` | B | chỉ đọc |
| 19 | wake paths | `v2/kernel/`, `v2/sim/` | — | C | chỉ đọc |
| 20 | selftest wake paths | như #19 | `tempfile` (`kt151-selftest-*`) | A | chỉ đọc |
| 21 | wake paths `--strict` | như #19 | — | B | chỉ đọc |
| 22 | wake theo dep-info | cây nguồn, `v2/target/**/*.d` | **`v2/target/`** (cargo check) + `.kt238-depinfo-stamp.json` | main | ghi target ⇒ độc quyền với cargo khác |
| 23 | selftest wake dep-info | như #22 | `tempfile` | C | chỉ đọc |
| 24 | api boundary | cây nguồn | **tệp probe trong crate thật** + `v2/target/` | sau | **sửa cây** ⇒ độc quyền tuyệt đối |
| 25 | test có răng | `v2/**/*.rs` (test) | — | B | chỉ đọc |
| 26 | selftest test có răng | như #25 | `tempfile` (`kt163-selftest-*`) | B | chỉ đọc |
| 27 | abi spec | `spec/abi.toml` | — | C | chỉ đọc |
| 28 | golden trace | `v2/test/trace/` | — | C | chỉ đọc |
| 29 | kịch bản `.scn` | `v2/test/scn/` | — | C | chỉ đọc |
| 30 | scn fixtures | `v2/test/scn/fixtures/` | **`/tmp/scn-fix-*.log`** (cố định) | C | chỉ đọc; đường `/tmp` đã đổi sang per-run |
| 31 | báo cáo R1-v2 | `v2/agent/reports/` | — | B | chỉ đọc |
| 32 | fixture âm của máy kiểm báo cáo | `v2/test/report/fixtures/` | **`/tmp/repfx.out`** (cố định) | C | chỉ đọc; đường `/tmp` đã đổi sang per-run |
| 33 | abi reserved | `spec/abi.toml` | — | C | chỉ đọc |
| 34 | abi fixtures | `v2/test/abi/fixtures/` | `mktemp` | C | chỉ đọc |
| 35 | gen-check | `spec/`, `v2/bin/vifast-gen` | — | C | chỉ đọc |
| 36 | gen fixtures | `v2/test/gen/fixtures/` | `mktemp` | C | chỉ đọc |
| 37 | idl-gen-check | `v2/lib/vifast-idl/idl/` | — | C | chỉ đọc |
| 38 | idl fixtures | `v2/test/gen/fixtures/` | `mktemp` | C | chỉ đọc |
| 39 | build (L0 compile) | cây nguồn | **`v2/target/`** | sau | ghi target ⇒ độc quyền |

### 1.2 `gate --fast` (T2) — 14 bước; `gate --dev` (T1) bỏ 8 bước harness

| # | Bước | Đọc | Ghi | Làn | Vì sao |
|---:|---|---|---|---|---|
| 40 | L0 test | cây nguồn, `v2/target` | **`v2/target/`** + `mktemp` (`v2l0.*`) | main | ghi target; chứa binary **nhạy thời gian** (`kt30_stress` biên 3 s) ⇒ không chạy cạnh cargo khác |
| 41 | L0 kernel workspace | `v2/kernel/` | **`v2/kernel/target/`** | main | target **khác** `v2/target`, nhưng vẫn là cargo nặng ⇒ giữ tuần tự (xem §3) |
| 42 | L1 (1.000 seed) | `v2/sim` | **`v2/target/`** | main | ghi target |
| 43 | selftest runner ktest | `expected.txt`, `ktest.sh` | **`expected.txt`** (phá rồi trả lại) + QEMU + `$LOGDIR` | sau | **sửa cây** + QEMU ⇒ độc quyền |
| 44 | selftest qemu guard | `qemu_guard.sh` | **`v2/agent/JOURNAL/kt78-guard/`** (repo) + QEMU | sau | **ghi cây** + QEMU ⇒ độc quyền |
| 45 | selftest pre-measure | `v2/artifacts/` | `mktemp` (`PRE_MEASURE_ART_DIR`) | main | **đường đo** ⇒ không đặt cạnh làn nền |
| 46 | selftest pin-artifact | `v2/artifacts/` | `mktemp` (`pin-selftest.*`) | main | **đường đo** |
| 47 | `algo=` nghiêm ngặt ở đường đo | toàn bộ `v2/` | `mktemp` (`kt145-algo-strict.*`) | main | **đường đo** |
| 48 | selftest persist đỏ-trước | ảnh kernel | **`/tmp/kt75-persist`** + QEMU | sau | QEMU ⇒ độc quyền |
| 49 | cắt điện giữa lúc ghi | ảnh kernel, ảnh đĩa | **`/tmp/kt75-persist`** + QEMU | sau | QEMU ⇒ độc quyền |
| 50 | selfbreak 5 luật K6 | cây thật (**băm trước/sau**), git index | `mktemp` (`kt80-selfbreak.*`) | D | **chỉ đọc** cây thật (D2-113F) ⇒ chạy song song với làn kiểm tĩnh; **không** đặt cạnh L0 |
| 51 | ca âm git-guard | repo git | **worktree + nhánh tạm** | sau | sửa git ⇒ độc quyền |
| 52 | ca âm git-guard song song (R5) | repo git | **2 worktree + 2 nhánh tạm** | sau | sửa git ⇒ độc quyền |
| 53 | ca âm git-guard b | repo git | **`/tmp/kt85bneg.$$`** + worktree/nhánh | sau | sửa git ⇒ độc quyền |
| 54 | QEMU smoke (chỉ T1) | ảnh kernel | **`/tmp/k3-ktest`** + QEMU | sau | QEMU ⇒ độc quyền |

---

## 2. Đường `/tmp` cố định đã loại

Cơ chế: `v2/bin/v2` tạo **một** thư mục `V2_RUN_TMP` (`mktemp -d "${TMPDIR:-/tmp}/v2run.XXXXXX"`)
cho **mỗi lần chạy cổng** và `export` nó; mọi script con đổi đường cố định sang
`${V2_RUN_TMP:-${TMPDIR:-/tmp}}/<tên>`. Chạy script **rời** (không qua cổng) vẫn dùng
`${TMPDIR:-/tmp}` như cũ ⇒ không đổi hành vi cũ, chỉ thêm tính cô lập khi có cổng.

Đã kiểm **không có script nào khác đọc** các đường này trước khi đổi (`grep -rn` toàn repo; chỉ
còn chú thích và log lịch sử trong `JOURNAL/`).

| Tệp | Đường cố định cũ | Đường mới | Ai đọc? |
|---|---|---|---|
| `v2/test/scn/run_fixtures.sh` | `/tmp/scn-fix-$dir.log` | `${V2_RUN_TMP:-…}/scn-fix-$dir.log` | không ai (chỉ ghi log) |
| `v2/test/report/run_fixtures.sh` | `/tmp/repfx.out` | `${V2_RUN_TMP:-…}/repfx.out` | không ai |
| `v2/test/boot/ktest.sh` | `/tmp/k3-ktest` (LOGDIR) | `${V2_RUN_TMP:-…}/k3-ktest` | `conformance/run.sh` qua `KT54_LOGDIR` ⇒ đổi **cùng biểu thức** |
| `v2/test/boot/ktest.sh` | `/tmp/k3-ktest-build{,-vblk,-con,-exec,-user}.log` | `$LOGDIR/k3-ktest-build*.log` | không ai |
| `v2/test/boot/persist.sh` | `/tmp/kt75-persist` | `${V2_RUN_TMP:-…}/kt75-persist` | không ai (JOURNAL cũ chỉ trích log) |
| `v2/test/boot/run.sh` | `/tmp/k3-run`, `/tmp/k3-product-build.log`, `/tmp/k3-product-boot-$i.log` | `$LOGDIR/…` | không ai |
| `v2/test/boot/t26.sh` | `/tmp/kt149-t26` | `${V2_RUN_TMP:-…}/kt149-t26` | không ai |
| `v2/test/boot/shell-edge.sh` | `/tmp/kt108-shell-edge` | `${V2_RUN_TMP:-…}/kt108-shell-edge` | không ai |
| `v2/test/boot/soak.sh` | `/tmp/k3-soak`, `/tmp/kt57-soak-build.log` | `$LOGDIR/…` | không ai |
| `v2/test/conformance/run.sh` | `/tmp/k3-kt55`, đọc `/tmp/k3-ktest` | `${V2_RUN_TMP:-…}/k3-kt55`, đọc cùng biểu thức với `ktest.sh` | — |
| `v2/test/parity/run.sh` | `/tmp/kt77a-parity` | `${V2_RUN_TMP:-…}/kt77a-parity` | không ai |
| `v2/tools/git-guard/neg-test-kt85b.sh` | `/tmp/kt85bneg.$$` | `${V2_RUN_TMP:-…}/kt85bneg.$$` | không ai |

**Không đổi** (cố ý):

- `v2/test/boot/guard_selftest.sh`: `LOGDIR` mặc định `v2/agent/JOURNAL/kt78-guard` — **bằng
  chứng** phải nằm trong repo (đã commit log), không phải `/tmp`. Bước này ở làn `sau`.
- `v2/tools/verify-numbers.py`: cache `$TMPDIR/kt80-ancestry-<hash>.json` — đã per-root, đọc lỗi
  JSON thì bỏ qua (cache chỉ là tối ưu wall), không tham gia quyết định đúng/sai.
- `v2/test/boot/selftest.sh`, `shell-edge.sh --selftest`, `t26.sh --selftest`, `build-rev.sh
  --selftest`, `build-identity.sh --selftest`, `check-algo-pin.sh`, `task-target.sh --selftest`,
  `counter-evidence.sh`, `algo-strict.sh`, `pin-artifact.sh --selftest`, `selfbreak.sh`,
  `check-wake-paths.py --selftest`, `check-test-teeth.py --selftest`, `check-selftest-wiring.py`,
  `check-scope-table.py --selftest`, `gen/abi/idl fixtures`: **đã** dùng `mktemp -d` riêng cho
  từng ca ⇒ không có đường cố định để đổi.

---

## 3. Bước **không** song song hoá — và lý do

| Bước | Lý do giữ tuần tự |
|---|---|
| `L0 test` (#40) | Chứa binary **nhạy thời gian** (`kt30_stress` biên 3 s, `spsc` hạn 10 s). Chính `l0_test_parallel` đã tách chúng ra chạy **một mình** vì "dưới tải chúng đỏ oan" (D2-116/D2-141). Đặt cargo/selfbreak cạnh L0 là tái diễn đúng lỗi đó. |
| `L0 kernel workspace` (#41) | Cargo nặng, dùng gần hết 12 nhân; chạy cạnh L0 (6 job × 2 thread = 12 thread) sẽ **oversubscribe** và làm chính L0 chậm/flaky. Target dir khác (`v2/kernel/target`) nên **không** xung đột tệp — chỉ xung đột CPU. |
| `L1` (#42) | Cùng `v2/target` với L0 ⇒ cargo khoá tuần tự; thêm nữa L1 là phép đo seed. |
| `selftest pre-measure` / `pin-artifact` / `algo=` (#45–47) | Là **đường đo** (KT-95/KT-145): chạy dưới tải thì số đo bị nhiễu. Giữ ở `main`, không đặt cạnh làn nền. |
| QEMU: `selftest runner ktest`, `selftest qemu guard`, `persist --red-selftest`, `persist --crash`, `QEMU smoke` (#43, #44, #48, #49, #54) | QEMU + `-icount` là phép đo thời gian thật; AGENTS.md §9 giới hạn 3 QEMU song song và cấm `pkill` toàn cục. Thêm nữa #43 **sửa** `expected.txt`, #44 **ghi** `JOURNAL/kt78-guard/`. |
| git-guard ×3 (#51–53) | Tạo/xoá **worktree + nhánh** trên checkout chính và tự kiểm bất biến "HEAD/index/số worktree không đổi" ⇒ phải chạy một mình. |
| `api boundary` (#24) | Chèn tệp probe vào **crate thật** rồi `cargo check`; checker khác đọc cây trong lúc đó sẽ thấy tệp probe ⇒ sai. |
| `bảng đối chiếu scope=` (#15) | Ghi `v2/agent/JOURNAL/scope-table-skip-count.txt` trong repo (mốc xu hướng) ⇒ không chạy cạnh bước đọc `JOURNAL/`. |

---

## 4. Bất biến của bản vá

1. **Cùng tập bước, cùng ngưỡng, cùng số ca.** Không bước nào bị bỏ; T1 vẫn bỏ **đúng 8** bước
   harness qua `V2_T2_ONLY_STEPS` (danh sách không đổi).
2. **Log phát lại theo số thứ tự cố định**, không theo thứ tự hoàn thành ⇒ hai lần chạy so sánh
   được từng dòng.
3. **Mã thoát** = có bước nào đỏ hay không (như cũ). `gate --fast` vẫn dừng sau `check` nếu
   `check` đỏ.
4. **Không chạy checker trên cây đang bị sửa**: mọi bước sửa cây nằm ở làn `sau`, chạy **sau khi**
   mọi làn nền đã join.
