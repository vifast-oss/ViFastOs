# KT-00 — Cổng: `step_l0_kernel` phủ `vifast-core` + `vifast-ktest` (D2-253/D2-262)

Người làm: Lead. Thời điểm: 2026-11-26, khung Cao điểm 2 → Thấp điểm (bàn giao tại `10:00 UTC`).
Nguồn yêu cầu: `D2-253` (quyền `v2/bin/**`), `D2-262` §1 (điểm mù 153 test lõi), `KT-408e` không liên quan.

## Vấn đề

`v2/Cargo.toml` đặt `exclude = ["kernel"]`, nên bước L0 của cổng chỉ chạy các crate `vifast-plat-*`
qua `v2/bin/lib/steps_gate.sh:step_l0_kernel`. Hai crate **`vifast-core` (137 test) và
`vifast-ktest` (16 test)** không bao giờ nằm trong T1/T2 — cổng xanh mà 153 test lõi không chạy.

## Thay đổi (1 tệp, 0 bước cổng mới)

[`v2/bin/lib/steps_gate.sh`](../../bin/lib/steps_gate.sh): thêm `-p vifast-core -p vifast-ktest`
vào đúng lệnh `cargo test --manifest-path v2/kernel/Cargo.toml` của `step_l0_kernel`, kèm chú thích
lý do. Không đổi thứ tự bước, không thêm làn, không đổi ngưỡng/thời gian bước.

## Bằng chứng

- **T1 lượt 1** (`gate --dev`, HEAD `4e41c6c`): đỏ **một** bước `luật máy K6` — `E1_SCOPE_UNTRACKED`:
  `v2/agent/reports/STATUS.md` trích log nightly chưa commit. Không liên quan bản vá
  (log: `target-task/peak2-20261126/t1-gate-dev.log`).
- **Khắc phục**: commit bằng chứng nightly trước báo cáo — `91060807` (3 nhật ký
  `v2/agent/JOURNAL/nightly/nightly-20261126-*` + `STATUS.md`).
- **Bản vá**: `297b89dc`.
- **T2** (`gate --fast`, HEAD `297b89dc`): **PASS**; bước `L0 kernel workspace (KT-148)` chạy thật
  `vifast-core` 137 + `vifast-ktest` 16 (log: `target-task/peak2-20261126/t2-gate-fast.log`).

## Bất biến đã giữ

Không sửa `spec/`, không sửa kỳ vọng harness, không nới ngưỡng, không `#[ignore]`, không retry lấy
xanh (D2-07), không CI. Commit qua `vifast-guard` với `VIFAST_ROLE=lead` (không `--no-verify`).

## Nợ / bàn giao

- `step_l0_kernel` **không** chạy trong T1 (`gate.sh:122` chỉ đăng ký ở `gate --fast`) ⇒ mọi thay đổi
  tệp này phải được chứng minh bằng **T2**, không đủ T1.
- Bằng chứng nightly của Peak 2 nằm ở `target-task/peak2-20261126/` + 3 thư mục
  `v2/agent/JOURNAL/nightly/nightly-20261126-*` (đã commit); soak aarch64/x86_64 **chưa** lấy được
  do lệch TAP 46/47↔50 — thuộc `KT-408e`.
