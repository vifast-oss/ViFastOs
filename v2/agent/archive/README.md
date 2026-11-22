# Kho lưu trữ `v2/agent/` — raw `.log` và phiếu/báo cáo cũ

Thư mục này là **chỉ mục** của những gì đã được đưa ra khỏi cây làm việc nhưng **không xoá hẳn**:
dữ liệu thô (raw `.log`) và phiếu/báo cáo cũ của các vòng trước. Mọi mục đều **khôi phục được
bằng một lệnh** (xem §2), nên không có bằng chứng nào bị mất.

| Mục | Ở đâu | Task | Ngày |
|---|---|---|---|
| Phiếu/báo cáo cũ ở gốc repo (20 tệp) | `v2/agent/archive/reports/` | `KT-333e` | 2026-11-23 |
| Raw `.log` Chu kỳ 1/2 (7.546 tệp) | tag `cycle1-raw-logs` | `KT-333e` | 2026-11-23 |
| `.log` **chưa theo dõi** của `kt78-guard` (36 tệp) | tag `cycle1-raw-logs-untracked` | `KT-333f` | 2026-11-23 |

## 1. Nhánh/tag lưu trữ raw `.log`

Cả hai tag đều trỏ tới **root commit** (cây riêng, **không** nối lịch sử `main`), nên `main` không
bị kéo theo hàng nghìn tệp log; muốn lấy lại thì `git checkout <tag> -- <đường-dẫn>`.

| Tag (annotated) | Commit | Số tệp | Nội dung | Kích thước | Task |
|---|---|---:|---|---:|---|
| `cycle1-raw-logs` | `948fb372` | **7.546** | raw `.log` Chu kỳ 1/2 nằm ngoài mọi khối `scope=`/`scope-red=` (đo: 159 khối / 124 tên / 119 tệp `.log` nguồn — đã kiểm **0** tên không phân giải) | — | `KT-333e` (D2-247 mục 3b) |
| `cycle1-raw-logs-untracked` | `9d7916db` | **36** | `.log` **chưa theo dõi** dưới `v2/agent/JOURNAL/kt78-guard/` — transcript của bước T2 *"selftest qemu guard (KT-78)"* (`v2/test/boot/guard_selftest.sh`), sinh 2026-11-21 (19) và 2026-11-22 (17) | 126.380 B (0,12 MiB) | `KT-333f` |

**Vì sao 36 tệp đó không gộp vào `cycle1-raw-logs`**: chúng **không có** trong tag cũ — so **theo
đường dẫn** 0/36 và so **theo nội dung** (git blob hash với 6.930 hash phân biệt của tag) cũng
**0/36**; chúng sinh **sau** `KT-333e` và chưa từng được theo dõi Git. Vì vậy phải lưu trữ ở tag
thứ hai trước khi xoá (không thể "xoá vì đã có trong tag cũ").

## 2. Khôi phục (một lệnh)

```bash
# Toàn bộ raw .log Chu kỳ 1/2 (7.546 tệp)
git checkout cycle1-raw-logs -- v2/agent/JOURNAL/

# Toàn bộ 36 log guard-selftest (KT-333f)
git checkout cycle1-raw-logs-untracked -- v2/agent/JOURNAL/kt78-guard/

# Một tệp bất kỳ
git checkout cycle1-raw-logs-untracked -- v2/agent/JOURNAL/kt78-guard/selftest-20261121T030552Z.log

# Liệt kê / kiểm nội dung mà không cần checkout
git ls-tree -r --name-only cycle1-raw-logs-untracked
git cat-file blob cycle1-raw-logs-untracked:<đường-dẫn> | shasum -a 256
```

Hai tag hiện là **ref cục bộ** (chưa push): `git push origin cycle1-raw-logs cycle1-raw-logs-untracked`.

Sau khi khôi phục, các tệp `.log` trả về sẽ ở trạng thái **đã theo dõi** (vì `git checkout <tag>`
ghi cả vào index); muốn để chúng là tệp chưa theo dõi thì `git rm --cached -- <đường-dẫn>`.
Bản sao lưu **byte-identical** đã được kiểm: `git archive <tag> | tar -x` rồi so `sha256`
**36/36** khớp (xem `JOURNAL/kt333f-logs.md` §4).

## 3. Còn lại trong cây (không lưu trữ, có lý do)

- **222** `.log` **đang theo dõi** dưới `v2/agent/JOURNAL/` — giữ nguyên theo `KT-333e` §3.2:
  **119** là nguồn của khối `scope=`/`scope-red=`, **99** là đích liên kết markdown của tài liệu
  đang dùng, **2** log `positive_log`/`negative_log` của `counters.toml`, **6** log của
  `numbers-policy.toml`, cộng 1 log được giữ vì báo cáo đã lưu trữ trỏ tới.
- **5.625** `.log` **không theo dõi** và **đã bị `.gitignore` phủ** (`v2/agent/JOURNAL/**/run-*/`,
  `soak/`, `parity/`, `persist/`, `ktest-matrix/`; xem `.gitignore` dòng 19–25) — 25,08 MiB, phần
  lớn là `boot-*.log` của các lượt soak. **Không** đưa vào Git: D2-230 **cấm** commit raw
  `boot-*.log` của soak (chỉ commit tóm tắt `summary.txt`/bảng/sha256), và D2-230(1) nói **không
  xoá lịch sử `JOURNAL/` cũ khi chưa có quyết định riêng**. Chúng cũng **không** làm `git status`
  ồn vì đã ignored.
- Chính sách đang áp: **D2-230** + **D2-245**(1) — mỗi task chỉ commit **một** tệp
  `JOURNAL/kt<n>-<ten>.md`; **không** tạo thư mục `kt*-logs/*.log` mới, trừ khi tệp nằm trong
  khối `scope=` **bắt buộc** của `v2/tools/verify-numbers.py`.

## 4. Phiếu/báo cáo cũ

Xem `v2/agent/archive/reports/README.md` — 20 tệp (19 phiếu/báo cáo `.md` + 1 tổng hợp `.txt`) do
`KT-333e` chuyển từ gốc repo, tên tệp **giữ nguyên**, liên kết markdown đã sửa tiền tố.
