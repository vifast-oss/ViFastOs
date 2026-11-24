#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-76 — trích **nguyên văn** tiêu chí DOD từ hai nguồn **đọc-only**:
#   - `docs/v2/ViFastOs-v2-thiet-ke.md` §10.1 dòng K6 (cấm sửa)
#   - `docs/v1/KNOWN-ISSUES.md` bảy dòng bảng (cấm sửa)
#
# Vì sao có script này: `K6-DOD.md` phải chứa tiêu chí **nguyên văn**, nhưng tiêu chí có
# **chữ số** ("7 pha", "600 boot", "MAX_IO=256", "0/50" …) mà D2-110A/D2-112 **cấm** ngoại lệ
# chữ số trong tài liệu. Cách làm: tiêu chí nằm trong khối ```text scope=kt76-criteria.txt```
# và tệp đó **do script này sinh ra** từ nguồn đọc-only; `kt76-verify-numbers.sh` **sinh lại**
# rồi so **từng byte** ⇒ không có chữ số nào do agent viết tay.
#
# Dùng: python3 kt76_extract_criteria.py <repo-root>   # in ra stdout
import re
import sys

repo = sys.argv[1]

# --- 1) thiết kế §10.1 — đúng dòng `| K6 |` của bảng DÒNG LÕI (phải có đúng một).
# Phạm vi phải giới hạn trong §10.1: bảng thiết bị §6 cũng có dòng `| K6 | virtio-blk…`.
design = open(f"{repo}/docs/v2/ViFastOs-v2-thiet-ke.md", encoding="utf-8").read().splitlines()
start = next(i for i, ln in enumerate(design) if ln.startswith("### 10.1"))
end = next(i for i, ln in enumerate(design[start + 1:], start + 1) if ln.startswith("### 10.2"))
k6 = [
    ln
    for ln in design[start:end]
    if ln.startswith("| K6 |") and "Tương đương v1" in ln
]
if len(k6) != 1:
    raise SystemExit(f"kt76-extract-criteria: thấy {len(k6)} dòng '| K6 |' ở §10.1 (cần đúng 1)")
print("# Nguồn 1: docs/v2/ViFastOs-v2-thiet-ke.md §10.1 — dòng K6 (nguyên văn, đọc-only)")
print(k6[0])

# --- 2) sổ lỗi v1 — bảy dòng bảng `| 1 |` .. `| 7 |`
ki = open(f"{repo}/docs/v1/KNOWN-ISSUES.md", encoding="utf-8").read().splitlines()
rows = [ln for ln in ki if re.match(r"^\| [1-7] \|", ln)]
if len(rows) != 7:
    raise SystemExit(f"kt76-extract-criteria: thấy {len(rows)} dòng bảng KNOWN-ISSUES (cần 7)")
print("# Nguồn 2: docs/v1/KNOWN-ISSUES.md — bảy dòng bảng (nguyên văn, đọc-only)")
for row in rows:
    cells = [c.strip() for c in row.strip().strip("|").split("|")]
    # **ĐỦ SÁU CỘT** (reviewer KT-76 chỉ đúng: trước đây bỏ cột 4-5, mà cột 4 = "Ca kiểm tương ứng
    # trong v2" chính là cột nói **test nào** phải có — bỏ nó làm N1/N7 bị ghi N/A sai).
    # 0 = số thứ tự, 1 = lỗi v1, 2 = mô tả, 3 = tiêu chí đóng gốc (dossier), 4 = ca kiểm v2,
    # 5 = trạng thái.
    print(f"N{cells[0]} | " + " | ".join(cells[1:]))
