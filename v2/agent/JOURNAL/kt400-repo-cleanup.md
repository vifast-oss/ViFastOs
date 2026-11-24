# KT-400 — Tinh gọn Kho dứt điểm (Đợt W1 / slot 1, `D2-259`)

**Ngày**: 2026-11-24 · **Nhánh**: `task/kt400-repo` (base `973c578e`, worktree `/private/tmp/vifast-wt/1`)
**Hợp đồng**: `v2/agent/QUEUE.md` dòng KT-400 + `KE-HOACH-KIEN-TRUC.md` §4 (KT-400) + `D2-259`.
**Trạng thái**: việc đã xong & đã kiểm T0/T1; **commit chờ Lead** (vifast-guard chặn agent — xem §7).

---

## 1. Tag `cycle1-raw-artifacts` — 914 tệp artifact thô `JOURNAL/`

Cây lưu trữ root commit `0c7a535c7bb091b2cc73b3e13d0ff2875b9cbc23` + annotated tag
**`cycle1-raw-artifacts`** (không nối lịch sử main, theo tiền lệ `cycle1-raw-logs`).

- Trước: `v2/agent/JOURNAL/` có **1.496** tệp theo dõi (339 `.md`, **1.157** không-`.md`).
- Lưu trữ: **914** tệp không-`.md`, tổng **2,14 MiB** blob.
- Giữ trên `main`: 339 `.md` + **242** tệp không-`.md` + `.gitkeep`:
  - 123 tệp là đích khối `scope=`/`scope-red=`/`scope-pending=` của `verify-numbers.py`
    (giữ luật 1/2/3/7 xanh; `ok=5 fail=0`),
  - 110 đích liên kết markdown từ tài liệu đang dùng,
  - 7 tệp `summary.txt`,
  - 3 script được harness nhắc đích danh (`kt77b-harness-scan.sh`, `kt78-collect-guard.sh`,
    `kt90-soak-product-image.sh`),
  - **không** tệp nào là `positive_log`/`negative_log` của `v2/test/rules/counters.toml`, không
    phải `[[legacy]]` của `numbers-policy.toml`, không phải raw log soak bị `.gitignore`.
- Khôi phục: `git checkout cycle1-raw-artifacts -- v2/agent/JOURNAL/`
  (từng tệp: `git checkout cycle1-raw-artifacts -- <đường-dẫn>`).

## 2. Tag `docs-bundles-archive` — 119 tệp `docs/bundles/` + lịch sử `review/`

Root commit `de977c38ab6f6afb7deeb1e47f4f8bb9533a0f89` + annotated tag
**`docs-bundles-archive`**: 90 tệp `docs/bundles/` (**11,0 MiB**) + 29 tệp lịch sử `review/`
(tổng 119 tệp, **11,88 MiB** blob).

**Bất biến đã kiểm bằng `ls`**: giữ nguyên trên `main` `review/agent0/GOI-ABI-V1_1.md`
(`spec/kernel.toml:19,84` ghim, `check_kernel.py` id 77), toàn bộ `docs/v1/` (106 tệp, đọc-only),
`docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`.

Khôi phục: `git checkout docs-bundles-archive -- docs/bundles/ review/`.

## 3. Gỡ cây mã v1 đóng băng

`git rm -r` 189 tệp: `crates/` 93, `legacy/` 27, `deploy/` 24, `python-model/` 19, `scripts/` 26
(+ `Cargo.lock` v1 ở gốc). Bản gốc bất biến ở tag **`v1-archive`** (`ca2eaf5`) — không tạo/xoá tag
nào của v1.

- **Trước khi xoá** đã `git grep` toàn repo: chỉ còn tham chiếu trong tài liệu lịch sử (v1/docs/v1,
  `docs/ViFastOs-agent-handoff.md`, `resources/brand/README.md`, `skills/*`) — **không** có mã
  thực thi, `.scn`, `v2/tools/`, `v2/test/` nào gọi tới; root `Cargo.toml` là nơi duy nhất khai báo
  workspace v1 (đã đổi).
- `Cargo.toml` gốc: `[workspace] members = []` + ghi chú "workspace duy nhất là `v2/Cargo.toml`;
  mọi lệnh cargo dùng `--manifest-path v2/Cargo.toml`". Kiểm: `cargo metadata --no-deps` ở gốc
  repo ⇒ rc 0, `packages=0`, `workspace_root=/private/tmp/vifast-wt/1`.
- Đường dẫn chết còn sót được **ghi chú khôi phục** thay vì sửa lịch sử: `README.md` (§3, §5, §6),
  `resources/brand/README.md`, `skills/vifast-parallel-gate/SKILL.md`. `docs/v1/` **không sửa**
  (đọc-only theo §4.7) dù còn câu chữ nhắc `docs/bundles/`/`review/src/`.
- Cắt kho: **2.898 → 1.678 tệp** (−1.220 net: xoá 1.223 tệp, thêm 3), **42,46 → ~25,4 MiB** (−40 %) nội dung theo dõi.

## 4. Fixture `case-r4-path-dep` (gốc rễ + cách vá)

Fixture cũ trỏ `path = "../../../../crates/vifast-abi"` — đường dẫn này **chưa từng tồn tại**
(nó giải ra `v2/test/crates/...`, không phải `crates/` ở gốc repo) và sau khi gỡ `crates/` thì
`T0` (`cargo clippy/test -p vifast-abi2`, chọn theo `Cargo.toml` bị chạm) **đỏ**:
`current package believes it's in a workspace when it's not`.

Vá: thêm `[workspace]` rỗng cho fixture + stub cục bộ
`v2/test/fixtures/case-r4-path-dep/crates/vifast-abi-stub/` (SPDX, 0 dòng phụ thuộc) để
path-dependency vẫn **chứa `crates/`** ⇒ R4 (D2-26) vẫn bắt ca âm, mà crate biên dịch được.
`v2/test/rules/run.sh` đổi nhãn case 4 thành "path-dependency sang cây v1 (crates/)".

## 5. Lệnh kiểm (đã chạy trên cây sau khi sửa)

| # | Lệnh | Exit | Kết luận |
|---|---|---:|---|
| 1 | `python3.12 v2/test/kernel/check_kernel.py` | 0 | `kernel-check: PASS (37 giá trị đối chiếu theo tên hằng)` |
| 2 | `python3.12 v2/test/kernel/check_kernel.py --selftest` | 0 | `8/8 ca đúng như kỳ vọng` |
| 3 | `python3.12 v2/tools/verify-numbers.py` | 0 | `ok=5 fail=0 wall_ms=17213` |
| 4 | `bash v2/test/rules/run.sh` | 0 | `6/6 case đúng kỳ vọng` (R4 vẫn ĐỎ đúng ca âm) |
| 5 | `bash v2/bin/v2 t0` | 0 | `T0: PASS (2 crate)` — gồm fixture R4 + stub |
| 6 | `cargo metadata --no-deps` (gốc repo) | 0 | `packages=0` — workspace v1 không còn |
| 7 | `bash v2/bin/v2 gate --dev` (**T1**) | 0 | `v2 gate --dev: PASS (T1 — KHÔNG phải full gate)`, tổng `707823 ms`; `L1 (sim 1.000 seed): OK`; `QEMU smoke (T1: 7 ảnh × 1 boot): OK`; `QEMU aarch64: OK`; `QEMU x86_64: OK` |
| 8 | `bash v2/bin/v2 check` (sau khi thêm biên bản này) | 0 | `v2 check: PASS`; `verify-numbers: ok=5 fail=0`; `kernel-check: PASS` |

**T-INFRA (lượt T1 đầu)**: chạy với `CARGO_TARGET_DIR=/Users/mps/.../target-task/slot-1` (cache ấm
dùng chung) ⇒ 1 bước đỏ `selftest wake dep-info (KT-217)`: `FAIL (51) ... E_DEPINFO_OUTSIDE
/private/tmp/vifast-wt/1/v2/lib/vifast-traceformat/src/lib.rs` (`69/70`). Nguyên nhân **môi trường**:
ca 51 của `--selftest` dựng repo giả trong `mktemp -d` rồi chạy `--no-build`, nhưng
`cargo metadata` lấy `CARGO_TARGET_DIR` tuyệt đối ⇒ quét `.d` của cây thật, nằm ngoài gốc giả.
Tái hiện: `env -u CARGO_TARGET_DIR python3.12 v2/tools/check-wake-depinfo.py --selftest` ⇒
`70/70 ok`. Không sửa checker (không nới ngưỡng); T1 #7 chạy lại với `CARGO_TARGET_DIR` **không
đặt** (cửa sổ `gate --dev` không đặt biến này) ⇒ PASS.

## 6. Không chạy

`gate --fast` (T2) và `nightly`/soak — theo hợp đồng chỉ Lead chạy T2 lúc merge. Không retry cổng
để lấy xanh; lượt chạy lại T1 ở trên là chạy đúng môi trường sau khi phân loại T-INFRA (D2-07 cho
phép chạy lại 1 lần để phân loại).

## 7. Vì sao chưa có commit trên nhánh (bàn giao Lead)

`vifast-guard/pre-commit` fail-closed khi `VIFAST_ROLE` ≠ `lead` (AGENTS.md §4/§7, `D2-123`);
agent **không được** tự đặt biến, **không** dùng `--no-verify`. Vì vậy:
toàn bộ thay đổi đã **stage** trong index của worktree (1.223 xoá + 7 tệp sửa + 3 tệp mới), 2 tag
annotated đã tạo (tag không đi qua hook). Lead commit bằng
`VIFAST_ROLE=lead git -C /private/tmp/vifast-wt/1 commit ...` (kế hoạch 6 commit:
`/tmp/kt400-commit-plan.sh`), rồi chạy T2 `gate --fast` trên ứng viên tích hợp.

## 8. Nợ / rủi ro cho Lead

1. **`Cargo.lock` gốc đã gỡ** ⇒ repo còn **1** `Cargo.lock` (`v2/Cargo.lock`). Checker `B3`
   "đồng bộ 2 `Cargo.lock`" của KT-407 cần đọc lại bối cảnh (1 lock là trạng thái đích sau gỡ v1).
2. `docs/v1/` (đọc-only) còn câu chữ nhắc `docs/bundles/`, `review/src/` — không sửa được theo
   §4.7; nội dung lịch sử, khôi phục bằng tag.
3. `review/work/`, `review/video/` vẫn là thư mục **cục bộ bị `.gitignore`** (không theo dõi,
   không xoá) — `.gitignore` giữ nguyên chủ đích D2-259.
4. Tag trỏ **root commit tách rời** lịch sử (đúng tiền lệ `cycle1-raw-logs`): merge nhánh không
   kéo theo tệp lưu trữ; `git checkout <tag> -- <path>` vẫn khôi phục được.
5. Máy đo: worktree còn `v2/target/` (~994 MB, bị ignore) do lượt T1; xoá được bằng
   `rm -rf v2/target` nếu cần đĩa cho T2.
