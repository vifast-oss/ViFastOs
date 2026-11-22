# Kho lưu trữ phiếu/báo cáo cũ ở gốc repo

**Nguồn**: các tệp này trước đây nằm **ở gốc repo** `ViFastOs/` và đã được **chuyển nguyên văn**
(tên tệp giữ nguyên) vào thư mục này bởi task **`KT-333e`** ngày **2026-11-23** (D2-247 mục 3b).

**Lý do**: dọn gốc repo (22 phiếu/báo cáo của các vòng trước nằm rải ở gốc), để gốc repo chỉ còn
tài liệu **đang dùng** và để `v2 gate --fast` pre-flight không phải cảnh báo vì tệp lạ ở gốc.

**Đã chuyển (20 tệp)** — 19 phiếu/báo cáo `.md` + 1 tệp tổng hợp kiến trúc `.txt`:

| Nhóm | Tệp |
|---|---|
| Báo cáo tuần/đêm/thi hành | `BAO-CAO-CUOI-TUAN-KT242-2026-11-20.md`, `BAO-CAO-DEM-2026-11-19-VONG2.md`, `BAO-CAO-DEM-2026-11-20.md`, `BAO-CAO-QUA-DEM-2026-11-19.md`, `BAO-CAO-THI-HANH-LUOT4-2026-11-18.md` |
| Phiếu gửi Agent 0 | `PHIEU-GUI-AGENT0-2026-11-19.md`, `PHIEU-GUI-AGENT0-2026-11-19-VONG2..VONG7.md` (6), `PHIEU-GUI-AGENT0-2026-11-20-VONG8.md`, `PHIEU-GUI-AGENT0-2026-11-21-KT241.md` |
| Phiếu trình / trả lời | `PHIEU-TRINH-AGENT0-2026-11-18-C1-C7.md`, `PHIEU-TRA-LOI-AGENT0-2026-11-18-C1-C7.md`, `PHIEU-TRA-LOI-AGENT0-2026-11-18-LUOT2.md`, `PHIEU-TRA-LOI-AGENT0-2026-11-18-LUOT3.md` |
| Phiếu ngăn API | `PHIEU-NGAN-API-2026-11-20.md` |
| Tổng hợp kiến trúc (v1) | `ViFastOs-architecture-summary.txt` |

**KHÔNG** chuyển (vẫn ở gốc repo, đang dùng): `BAO-CAO-CUOI-TUAN-KT243-2026-11-22.md` (báo cáo
đang dùng của lead), `KE-HOACH-*`, `AGENTS.md`, `README.md`, và các tệp hạ tầng khác ở gốc.

## Tham chiếu & khôi phục

- Tên tệp **không đổi**, nên mọi tham chiếu dạng **chữ** (không phải liên kết) trong
  `v2/agent/{QUEUE,DECISIONS}.md`, `JOURNAL/`… vẫn truy được: tệp cùng tên nay nằm ở
  `v2/agent/archive/reports/`. Các tệp đó là **bản ghi lịch sử**, không sửa lại.
- Liên kết markdown trong chính các tệp đã chuyển được sửa tiền tố (`v2/agent/…` → `../../…`)
  để không chết; liên kết trong `README.md` gốc trỏ tới đường dẫn mới.
- **Không** tệp nào trong **19 tệp `[[legacy]]`** của `v2/test/rules/numbers-policy.toml` nằm ở
  gốc repo (0/19) ⇒ không đụng ghim sha256 nào. `verify-numbers.py` chỉ đọc
  `v2/agent/reports/*.md` + `v2/docs/k6-*.md` nên thư mục này **không** vào bảng `scope=`.
- Khôi phục: `git log --follow -- v2/agent/archive/reports/<tên-tệp>` rồi
  `git mv v2/agent/archive/reports/<tên-tệp> <tên-tệp>` (commit chuyển là commit của `KT-333e`).
