# KT-333b — `v2 nightly` thật + `deny.toml` (D2-21) + `THIRD-PARTY.md`

**Ngày**: 2026-11-22 · **Sub-agent**: `nightly` · **Nhánh**: `task/kt333b-nightly` ·
**Base**: `main` @ `3a184935` · **Ngân sách**: 3 h (Chu kỳ 2, Đợt 0)
**Nguồn ràng buộc**: `AGENTS.md` §6/§6.1/§6.3/§7, `v2/AGENTS.md`, **D2-21**, **D2-229**(§7),
**D2-235**(3)(5), `KE-HOACH-CHU-KY-2-2026-11-22.md` KT-333b, `docs/QUYET-DINH-sau-review.md` §9.3.
**Cấm chạm**: `v2/kernel/**`, `v2/lib/**`, `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`.

---

## 1. Khảo sát (số liệu đọc từ code, không viết tay)

### 1.1 `v2 nightly` trước task làm gì?

`v2/bin/v2` (HEAD `3a184935`) chỉ có **hai** nhánh (dòng 1246–1260 cũ):

- `nightly --only-miri` → `do_nightly_only_miri`: 24 lệnh `cargo +nightly miri test` trên tập
  chọn (`vifast-objmem`, `vifast-sim --lib`, kt16/kt09_untyped/kt09_revoke/kt10/kt17/kt18/kt19/
  kt20/kt21/kt22/kt24/kt25/kt26/kt27/kt28/kt29/kt36..kt42, `vifast-pte`, `vifast-sync`). Đầu tệp
  ghi rõ "nightly đầy đủ (L1 100k/mutation/soak) vẫn **CHƯA CÓ**".
- `nightly --l1 [--seeds N] [--smp a,b,c]` → `do_nightly_l1`: chạy **một** tiến trình
  `cargo test -p vifast-sim driver_smp -- --nocapture` với `VIFAST_SEEDS`/`VIFAST_SMP`
  (mặc định 10.000 seed × `1,2,4,8`).
- Mọi dạng khác (`v2 nightly`, `nightly --dry-run`, `--status`, `--selftest`, `--all`, `--soak`)
  đều **exit 2** ("nightly đầy đủ CHƯA CÓ").
- Không có manifest/bằng chứng máy đọc, không sinh `STATUS.md`.

### 1.2 `l1-tier.sh --tier full` chia lô thế nào?

`v2/tools/l1-tier.sh` (KT-242): ba mức `smoke|medium|full`; `full` = **100.000 seed × smp
1,2,4,8 = 400.000** (hợp đồng K6). Chia lô: `K = ceil(2*jobs / N_SMP)` lô mỗi cấu hình smp
(`K` kẹp `1..seeds`), chạy pool `jobs` tiến trình; mỗi lô nhận `VIFAST_SHARD=i/K` và
`VIFAST_SEEDS`/`VIFAST_SMP` — **không** đổi seed/`SO_OP`/bất biến/ngưỡng. Gộp: kiểm mỗi smp phủ
**đúng** `seeds` seed (không sót/trùng), XOR digest 16 hex, bảng `ipc/map/walk/unmap/fault/shoot`.
Ước lượng có sẵn trong `JOURNAL/kt242-l1-tiers.md`: **full [ƯL] 12–15 phút** (11 worker, máy rảnh).

### 1.3 `deny.toml` / dependency ngoài / allowlist

- `deny.toml`: **chưa có** ở gốc repo và ở `v2/` (kiểm `ls deny.toml v2/deny.toml`).
- `THIRD-PARTY.md`: **chưa có**.
- `cargo metadata --manifest-path v2/Cargo.toml`: **18 gói workspace** + `vifast-core`
  (path-dependency của `vifast-sim`, `exclude = ["kernel"]`) = **19 gói**; `v2/Cargo.lock` có
  **19** mục `[[package]]` và **0** dòng `source =` ⇒ **không có dependency registry/git nào**
  (repo chưa có phụ thuộc ngoài). Vì vậy allowlist chưa từng bị thử; cổng mới dựng để **chặn
  trước** dependency ngoài đầu tiên (đúng D2-37(c)).
- `docs/QUYET-DINH-sau-review.md` §9.3 **nguyên văn**:
  > Allowlist: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, 0BSD, Zlib, Unicode-3.0,
  > Unicode-DFS-2016. Cấm MPL, GPL, LGPL.
  Ghi chú so với phiếu giao: phiếu liệt 8 mục (không có `0BSD`, `Unicode-DFS-2016`) và liệt
  `MIT OR Apache-2.0` như một mục. Bản dùng trong `deny.toml` = **đúng 9 mã QD §9.3**;
  `MIT OR Apache-2.0` là **biểu thức SPDX** (hợp lệ khi ít nhất một nhánh OR nằm trong allowlist),
  không phải mã riêng — thêm nó như mã riêng sẽ là bịa mã SPDX.

---

## 2. `v2 nightly` — cờ và hành vi

| Cờ | Hành vi | Exit |
|---|---|---|
| `--all` | L1-full hợp đồng K6 (`l1-tier.sh --tier full` = 400k) → Miri (tập KT-17) → đột biến (`selfbreak`, KT-80) → soak 3 arch (riscv64 1000×3, aarch64 1000×3, x86_64 600×3) | 0 xanh / 1 bước đỏ |
| `--l1 <seed>` | `< 10.000` ⇒ smp 1 (smoke); `≥ 10.000` ⇒ `1,2,4,8`; `≥ 100.000` ⇒ `l1-tier --tier full`. Tương thích cũ `--l1 --seeds N --smp a,b,c` | 0/1 |
| `--only-miri` | CHỈ Miri (không L1/mutation/soak) | 0/1 |
| `--soak` | soak ảnh sản phẩm 3 arch (D2-24: riscv64 1000, aarch64 1000; D2-149: x86_64 600) | 0/1 |
| `--status` | đọc `v2/agent/reports/STATUS.md` + manifest cục bộ mới nhất; **không** chạy lại | 0 đọc được / 1 hỏng |
| `--dry-run [cờ phạm vi]` | in kế hoạch: **bước + lệnh thật + seed + [ƯL] giây**; **không** chạy, không tạo thư mục bằng chứng | 0 |
| `--selftest` | **14 ca** (8 kế hoạch + 6 đọc/sinh trạng thái); không sim | 0/1 |
| (không cờ) | **lỗi dùng** (exit 2) — không có mặc định chạy (tránh vô tình chạy 4 tiếng) | 2 |

Không retry (D2-07): bước đầu tiên đỏ ⇒ dừng, ghi `result=FAIL`, vẫn sinh bằng chứng.

**Bằng chứng mỗi lần chạy thật** (máy sinh, không viết tay):
- `target-task/nightly/<run-id>/` (gitignored): `manifest.tsv`, `plan.tsv`, `step-*.log` thô;
- `v2/agent/JOURNAL/nightly/<run-id>/`: `manifest.tsv` + `summary.log` (nhỏ, commit được);
- `v2/agent/reports/STATUS.md`: sinh lại mỗi lần chạy; **mọi số đo** nằm trong **một** khối
  ```` ```text scope=nightly/<run-id>/summary.log ```` trích **nguyên văn** từ `summary.log`
  (luật máy 1 / KT-80) — prose chỉ có `PASS`/`FAIL` và `v2`, không có số đo viết tay (QD Q12).

**Ca tự kiểm `--selftest` (14/14 PASS)** — dương: kế hoạch `--all` đủ 6 bước + lệnh
`soak.sh 1000 3`; `--l1 1000` có `VIFAST_SEEDS=1000` + smp 1; `--l1 100000` dùng
`l1-tier.sh --tier full`; `--dry-run` trần = kế hoạch `--all` và **không tạo bằng chứng**;
`STATUS.md` tất định (2 lần render cùng byte); khối `scope=` trích nguyên văn + prose không số;
`--status` đọc PASS; chưa có bằng chứng ⇒ exit 0 + nói rõ. Âm: `--l1 0` ⇒ exit 2; `--l1 abc` ⇒
exit 2; cờ lạ ⇒ exit 2; sửa 1 dòng log tóm tắt ⇒ `STATUS_NOT_VERBATIM` (exit 1); `STATUS.md`
mất khối `scope=` ⇒ `STATUS_MALFORMED` (exit 1).

---

## 3. `deny.toml` + cổng giấy phép + `THIRD-PARTY.md` (D2-21)

`deny.toml` (gốc repo, cargo-deny dùng được cùng tệp):
- `[licenses].allow` = **MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, 0BSD, Zlib,
  Unicode-3.0, Unicode-DFS-2016** (nguyên văn QD §9.3; `exceptions = []`).
- `[sources]`: `unknown-registry = "deny"`, `unknown-git = "deny"`,
  `allow-registry = ["https://github.com/rust-lang/crates.io-index"]`, `allow-git = []`.

`v2/tools/check-licenses.py` (bước **73** của `v2 check`; D2-37(c) yêu cầu bước này nằm trong
`v2 check` **trước** dependency ngoài đầu tiên):
1. **giấy phép** — parser SPDX nhỏ (OR/AND/WITH/ngoặc): hợp lệ nếu ít nhất một nhánh OR thuộc
   allowlist; thiếu giấy phép ⇒ `E_LICENSE_MISSING`; sai cú pháp ⇒ `E_LICENSE_EXPR`;
2. **nguồn** — registry phải khớp **đúng** URL `allow-registry` (chuẩn hoá `registry+`/`sparse+`;
   crates.io git-index ↔ sparse-index là **cùng registry**); `git+…` phải khớp `allow-git`
   (rỗng = cấm) ⇒ `E_SOURCE_REGISTRY_DENIED`/`E_SOURCE_GIT_DENIED`;
3. **khớp lock** — tập `(name, version)` của `v2/Cargo.lock` phải **bằng đúng** metadata
   (`cargo metadata --locked`) ⇒ `E_LOCK_MISMATCH`;
4. **`THIRD-PARTY.md`** — sinh từ metadata (tên · phiên bản · giấy phép · nguồn), sắp xếp tất
   định; `--check` so byte, lệch ⇒ `E_THIRD_PARTY_DRIFT`. `--generate` từ chối ghi khi còn vi phạm.

Kết quả trên cây này (`v2 licenses --check`):
- **quét 19 gói** (19 nội bộ/workspace + **0** registry/git), **0 vi phạm**;
- `THIRD-PARTY.md`: **32 dòng** (đúng 4 cột), sinh bằng `v2 licenses --generate`;
- tự kiểm `check-licenses.py --selftest`: **9/9 PASS** — âm đỏ đúng mã: `GPL-3.0-only` ⇒
  `E_LICENSE_NOT_ALLOWED`, thiếu license ⇒ `E_LICENSE_MISSING`, registry lạ ⇒
  `E_SOURCE_REGISTRY_DENIED`, git dep ⇒ `E_SOURCE_GIT_DENIED`, lock lệch ⇒ `E_LOCK_MISMATCH`,
  thiếu `deny.toml` ⇒ `E_DENY_MISSING`, lệch `THIRD-PARTY.md` ⇒ `E_THIRD_PARTY_DRIFT`.

---

## 4. Đo (máy sinh)

| Phép đo | Kết quả | Nguồn |
|---|---|---|
| `v2 nightly --l1 1000` (driver `l1_k1c`, smp 1) | **PASS**, 1000 lượt, driver **68 s**, tổng lệnh **69,4 s** (gồm build) | `target-task/nightly/nightly-20261122-055211-3a184935/step-1-l1.log`; `v2/agent/JOURNAL/nightly/…/summary.log` |
| `v2 nightly --selftest` | **PASS 14/14**, 2,0 s | log cổng |
| `v2 licenses --selftest` | **PASS 9/9** | log cổng |
| `v2 nightly --dry-run` (--all) | 6 bước, tổng **[ƯL]** 89 phút 55 giây | stdout |
| `v2 check` (sau khi stage) | **PASS**, `# gate-wall 86049 ms (86,0 s)` | `target-task/kt333b-gates/check.log` |

Lần chạy L1-1000 ghi `dirty=1` (đúng: chạy trước commit); `HEAD=3a184935`. Đây là bằng chứng
"chạy thật" tối thiểu theo phiếu giao; **không** chạy L1-full/soak/Miri trong vòng task
(AGENTS §6: nightly + soak độc quyền QEMU; phiếu giao yêu cầu chỉ dựng + `--l1 1000`).

---

## 5. Cổng

| Lệnh | Kết quả |
|---|---|
| `./v2/bin/v2 check` | **PASS** (86,0 s; gồm 3 bước mới: 73 giấy phép **265 ms**, 74 selftest giấy phép **456 ms**, 75 selftest nightly **2.423 ms** — đều OK) |
| `./v2/bin/v2 nightly --dry-run` | **PASS** (in 6 bước + lệnh thật + [ƯL] 89 phút 55 giây; không chạy, không tạo bằng chứng) |
| `./v2/bin/v2 nightly --status` | **PASS** (đọc bằng chứng L1-1000 vừa chạy; `kết quả: PASS`) |
| `./v2/bin/v2 nightly --selftest` | **PASS 14/14** (2 s; 9 ca dương + 5 ca âm) |
| `./v2/bin/v2 nightly --l1 1000` | **PASS** (68 s driver) |
| `./v2/bin/v2 licenses --check` | **PASS** (19 gói, 0 vi phạm, `THIRD-PARTY.md` khớp) |
| `./v2/bin/v2 licenses --selftest` | **PASS 9/9** |
| `./v2/bin/v2 gate --fast` (T2) | **KHÔNG chạy trong vòng task** — làn QEMU (`sau`) độc quyền theo phiếu giao/AGENTS §6; ảnh hưởng của task chỉ là gate/checker + tool mới, không chạm kernel/lib ⇒ T2 để lead chạy trên ứng viên tích hợp |

---

## 6. `lane_step` — chống trùng (D2-235(5))

Đã kiểm **trên cả `do_gate_fast` và `do_gate_dev`** (dev gọi lại fast + id 70):
`grep -o "lane_step [A-Za-z]* [0-9]*" v2/bin/v2 | awk '{print $3}' | sort -n | uniq -d`
→ **rỗng**. Dải id hiện tại: 1..64, 66, 67, 69, 70, 71 (fast), 72 (check–NTFS), **73/74/75
(mới, check)**. Tổng **73** dòng `lane_step`; 3 bước mới chỉ nằm trong `do_check` (dùng chung
cho `check`, `gate --fast`, `gate --dev` nên không cần khai lại).

---

## 7. Sai lệch / nợ (ghi rõ, không giấu)

1. **Bước "đột biến" của `--all` dùng `selfbreak.sh` (KT-80)** — kiểm đột biến 5 luật K6, chạy
   được ngay. `v2 mutants` (thiết kế §9.13) **chưa tồn tại**, nên chưa có bộ đột biến mã nguồn
   tự động; nhãn bước ghi rõ "`v2 mutants` §9.13 chưa có". Đề xuất: mở task riêng cho
   `v2/test/mutants.toml` + `v2 mutants` trước khi KT-348 cần bằng chứng mutation đầy đủ.
2. **`--all` chưa được chạy** (hàng giờ + QEMU độc quyền) — đúng phiếu giao. Kế hoạch + lệnh +
   ước lượng đã kiểm bằng `--selftest`/`--dry-run`; lần chạy L1-1000 chứng minh đường thực thi
   (manifest → STATUS.md → `--status`).
3. **T2 chưa chạy** — lý do ở §5; cần lead chạy `gate --fast` trên ứng viên tích hợp (D2-235(2)).
4. `STATUS.md` đang là bằng chứng của lần chạy `--l1 1000` (chưa phải nightly đầy đủ); khi
   `--all` chạy xong, cùng lệnh sẽ ghi đè bằng bằng chứng đầy đủ hơn.
5. **Chưa commit được từ sub-agent**: hook `vifast-guard` (D2-123) đòi `VIFAST_ROLE=lead` cho
   **mọi** commit; AGENTS §7 cấm agent tự đặt biến/`--no-verify`. Toàn bộ tệp của task đã
   **stage** sẵn trên nhánh `task/kt333b-nightly`; lead chạy
   `VIFAST_ROLE=lead git -C <worktree> commit -m "[KT-333b] …"` rồi merge theo D2-235(1).

---

## 8. Tệp đã chạm (không chạm kernel/lib/spec/agent điều khiển)

Thêm: `deny.toml`, `THIRD-PARTY.md`, `v2/tools/check-licenses.py`, `v2/tools/nightly-report.py`,
`v2/agent/reports/STATUS.md`, `v2/agent/JOURNAL/nightly/<run-id>/{summary.log,manifest.tsv}`,
`v2/agent/JOURNAL/kt333b-nightly.md`.
Sửa: `v2/bin/v2` (nightly + licenses + 3 bước cổng + usage).
**Không** sửa: `v2/kernel/**`, `v2/lib/**`, `spec/**`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`.
