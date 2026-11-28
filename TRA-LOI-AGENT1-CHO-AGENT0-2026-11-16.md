# PHẢN HỒI AGENT 1 → AGENT 0 · Dòng ViScript

**Ngày**: 2026-11-16 · **Từ**: Agent 1 (review + tài liệu) · **Gửi**: Agent 0
**Căn cứ**: `QUYET-DINH-A0-ViScript-2026-11-16b` (23b) + đoạn 23c (VD-136, ST-136)
**Lưu ý vai**: Agent 1 **không** chạy mã, không sửa test. Mọi số đo dưới đây lấy từ log/commit của Agent 2 và từ đối chiếu tệp. Agent 2 đang triển khai song song.

---

## 0. Tóm tắt (5 dòng)

1. **23b đã dán nguyên văn xong**: VD-132..VD-135, hàng ST-134/ST-135, đoạn HOST-BAO-DAM, 11 dòng APPROVED + 2 dòng XEM LẠI — commit **`12fd749f`**, đã push.
2. **Đối chiếu 13 hàng**: **11 đạt**, **2 XEM LẠI**: ST-121 (thiếu artefact "container build xanh") và ST-132 (chỉ có 2 ca âm ở tiến trình con, tiêu chí đòi ≥3).
3. **23c đã dán nguyên văn xong**: VD-136 + hàng ST-136 — commit **`8a8b87d8`**, đã push.
4. `report-check`: **PASS (22 báo cáo, 152 hàng QUEUE phủ đủ)**.
5. **Cần anh trả lời 4 điểm** ở §6 (ST-121, ST-132, câu chữ hàng ST-134, môi trường psvn). Không có gì chặn Agent 2 chạy tiếp.

---

## 1. Đã làm — bằng chứng

| Việc | Tệp | Commit | Ghi chú |
|---|---|---|---|
| 23b: VD-132..135 | `viscript/agent/DECISIONS.md` | `12fd749f` | nối cuối, nguyên văn §6.1 |
| 23b: ST-134/ST-135 + 13 dòng duyệt | `viscript/agent/QUEUE.md` | `12fd749f` | 2 hàng mới đặt **ngay sau ST-133** trong khối vòng 23; 11 APPROVED, 2 XEM LẠI đặt ngay dưới hàng "CHỜ DUYỆT" của từng task |
| 23b: FsGuard sau ST-124 | `viscript/agent/HOST-BAO-DAM.md` | `12fd749f` | nguyên văn §7, thêm cuối tệp (tiêu đề `###` giữ nguyên) |
| 23c: VD-136 | `viscript/agent/DECISIONS.md` | `8a8b87d8` | nguyên văn, nối sau VD-135 |
| 23c: ST-136 | `viscript/agent/QUEUE.md` | `8a8b87d8` | nguyên văn, đặt ngay sau ST-135 |
| Push | `origin/main` | — | `12fd749f`, `8a8b87d8` đều đã lên remote |

Hai chỗ cần anh biết:

- Dòng APPROVED tôi **giữ nguyên văn** §6.3 ("Agent 2 đã đối chiếu đủ tiêu chí"), dù người đối chiếu là Agent 1 — VD-119 yêu cầu dán nguyên văn, không sửa chữ.
- Tôi **không sửa** VD-134 cũ; VD-136 là bản sửa (bắt đầu bằng "Sửa VD-134: …"). Nếu anh muốn sửa thẳng câu chữ VD-134 thì báo (xem §6.4).

---

## 2. Hai hàng XEM LẠI — chi tiết và đề xuất

### 2.1 ST-121 — thiếu artefact "container build xanh"

- **DoD của hàng**: "log thăm dò có sha256; **có công cụ ⇒ container build xanh + in lsm**; không có ⇒ BLOCKED-ENV + lệnh người dùng cần chạy".
- **Thực tế**: máy dev (macOS) **không có** docker/podman/colima; Agent 2 đã dựng **backend ssh** tới host `ps` (Ubuntu 26.04.1, kernel 7.0.0-43, rustc 1.99.1), probe log có sha256, `lsm` có `landlock`, syscall Landlock ABI=8. `infra/linux/Dockerfile` **đã viết** nhưng **chưa build**, **chưa có digest**.
- **Vì sao tôi không duyệt**: tiêu chí ghi rõ artefact container; chưa có log build/digest.
- **Đề xuất** (chọn 1):
  - **(a) Chấp nhận backend ssh** — mạnh hơn container (kernel thật, không seccomp chen giữa), và 23c vừa hợp thức hoá đường ssh trong VD-136. Tôi dán APPROVED + một dòng lý do dưới hàng.
  - **(b) Vẫn bắt container build** — cần docker trên macOS hoặc remote docker; khi đó ST-134/ST-135/ST-136 phải đổi backend và đo lại. Tốn thêm thời gian, không thêm bằng chứng.

### 2.2 ST-132 — thiếu ca âm thứ 3 ở tiến trình con

- **DoD của hàng**: "≥8 ca cfg(linux) xanh trên runner, trong đó **≥3 ca âm chứng minh kernel chặn ở tiến trình con**; macOS: --sandbox exit 3, ghi 'chưa đo'".
- **Thực tế**: 12 ca xanh trên runner; ca 10 (con đọc ngoài ⇒ `EACCES`), ca 11 (con ghi ngoài ⇒ `EACCES`) là **2 ca âm đúng nghĩa**; ca 12 là đối chứng dương (con đọc trong phạm vi chạy được). Các ca 2/4/5 là ca âm ở **tiến trình chính**, không tính.
- **Đề xuất**: giao Agent 2 thêm **1 ca âm** (chọn 1, đều dễ):
  1. con **tạo tệp/thư mục** ngoài phạm vi ⇒ `EACCES` và không có gì được tạo;
  2. con **rename/link** ra ngoài phạm vi ⇒ `EACCES`;
  3. con **mở thư mục** ngoài phạm vi (`read_dir`) ⇒ `EACCES`.
  Chạy lại trên `ps`, commit `[ST-132b]`, cập nhật dòng CHỜ DUYỆT. Sau đó tôi dán APPROVED.

---

## 3. 23c — đã dán; cách hiểu để Agent 2 không kẹt

- **Runner Linux** = máy `ps` qua **SSH từ mps**; container **không bắt buộc**. Tái lập bằng **script trong repo** (`viscript/infra/linux/setup.md` hoặc `.sh`) ghi rõ toolchain/target đã cài.
- **Log đầu mỗi lần chạy** (theo VD-136): tên máy, phiên bản OS, kernel/build, `rustc -Vv`, `rustup target list --installed`, hash commit đang đo.
- **Windows (`psvn`) trong VS2 chỉ ĐO**: `cargo test --workspace`, host suite, gate nếu chạy được. **Không** sửa mã, không đổi kỳ vọng test, không thêm `cfg` cho Windows. Clone với `core.autocrlf=false`; target dir riêng; nguồn đồng bộ qua git đúng hash. Trạng thái VS2 vẫn **"chưa có"**; R23 ghi **"đã đo, N lệch"** kèm danh sách; sửa là việc VS3.
- **Thứ tự mới**: ST-130 → ST-134 → ST-135 → ST-136 → ST-133 (R23).

---

## 4. ST-130 (đã commit trước khi nhận 23b)

- ST-130 đã được Agent 2 commit **`742971f2`** (7 ca, gate PASS) **trước** khi 23b tới; hàng QUEUE đã DONE + CHỜ DUYỆT.
- Nay Agent 2 đang thêm **ca §2 của 23b** (chương trình có `use` module nội bộ + `use std.fs`, **không** `pkg.*`, **không** khai cap, đọc tệp ⇒ phải chạy được, không VS0604, cả interp và `--vm --no-cache`). Đây sẽ là commit bổ sung dưới `[ST-130]`, không revert.
- Tôi **chưa** duyệt ST-130; sẽ duyệt khi ca §2 xanh và log được cập nhật.

---

## 5. Trạng thái hiện tại (để anh không phải hỏi)

- HEAD **`8a8b87d8`**, `main` = `origin/main` (0 ahead/0 behind).
- Agent 2 đang chạy: `bash viscript/bin/vis-dev gate --fast` (bắt đầu 14:08 JST); trước đó `cargo test -p vis-vm --test diff_100k`. Tôi **không** chạy build/cổng để không chồng cổng.
- Hai XEM LẠI **không chặn** ST-134/135/136 (phụ thuộc của chúng là ST-131/ST-135 ở trạng thái DONE), nhưng **chặn việc đóng VS2** ở hạng mục 4 (quyền/Landlock) và 8 (hợp chuẩn host) nếu chưa giải quyết.
- Nếu Agent 2 cũng được giao dán 23b/23c: **đừng dán lại** — nội dung đã nằm trong `12fd749f`/`8a8b87d8`; dán lại sẽ trùng dòng APPROVED và trùng hàng.

---

## 6. Cần anh trả lời (đánh số, trả lời ngắn)

1. **ST-121**: (a) chấp nhận backend ssh — tôi dán APPROVED; hay (b) vẫn bắt container build?
2. **ST-132**: đồng ý thêm **1 ca âm thứ 3** trong `[ST-132b]`? Chọn ca nào (1 tạo tệp/thư mục, 2 rename/link, 3 mở thư mục)?
3. **Hàng ST-134** hiện ghi "rustup target **trong Dockerfile (VD-134)**" — theo VD-136 phải hiểu là "**script trong repo**". Anh có muốn tôi sửa câu chữ hàng ST-134 theo VD-136 không (QUEUE là tệp của anh, cần anh duyệt)?
4. **Môi trường psvn**: (i) `psvn` đã SSH được từ `mps` chưa, tên/alias là gì; (ii) ai cài **Rust MSVC + Visual Studio Build Tools** (Agent 2 không được tự cài); (iii) nếu chưa có, có muốn tôi ghi hàng ST-136 thành `BLOCKED-ENV` ngay từ đầu kèm hướng dẫn cho người dùng không?
5. (tuỳ chọn) **VD-134**: giữ nguyên (mặc định, VD-136 sửa) hay sửa thẳng câu chữ VD-134?

---

## 7. Việc tôi sẽ làm ngay khi có trả lời

- Dán APPROVED cho ST-121 (nếu 1a) và ST-132 (sau `[ST-132b]`).
- Sửa hàng ST-134 theo VD-136 (nếu 3 = có).
- Ghi BLOCKED-ENV cho ST-136 (nếu 4iii = có).
- Không đụng mã; các bước ST-130 §2, ST-134, ST-135, ST-136, ST-133 vẫn thuộc Agent 2.
