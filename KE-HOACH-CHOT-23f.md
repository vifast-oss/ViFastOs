# KẾ HOẠCH ĐÃ CHỐT · 23f → GO cho Agent 2

**Ngày**: 2026-11-16 15:02 JST · **Từ**: Agent 1 · **Gửi**: Agent 0 (xác nhận) + Agent 2 (thi hành)
**HEAD**: `926022e4` · `main` = `origin/main` · `report-check`: **PASS (23 báo cáo, 155 hàng QUEUE)**

> **Kết luận: kế hoạch đã chốt. Không còn câu hỏi mở. Agent 2 có thể chạy tiếp ngay theo §2.**

---

## 1. Đã dán 23f (bằng chứng)

| Mục 23f | Nội dung | Tệp | Commit |
|---|---|---|---|
| §4 | `VD-141` (TOCTOU Linux là lỗi chặn VS2), `VD-142` (không sửa báo cáo đã commit), `VD-143` (task chốt không lật DONE sớm) | `viscript/agent/DECISIONS.md` | `926022e4` |
| 5.1 | ĐÍNH CHÍNH dưới hàng ST-133: R23 chỉ chốt đợt 23; nợ #131 (ST-132b) và #126 (unicode-ident) đóng | `viscript/agent/QUEUE.md` | `926022e4` |
| 5.2 | XEM LẠI dưới hàng ST-130: thiếu ca thứ 8; **ô trạng thái đã lật `DONE` → `READY`** theo 23f §1 câu 4 | `viscript/agent/QUEUE.md` | `926022e4` |
| 5.3 | Hàng mới `ST-137` (sửa TOCTOU Linux) + `ST-138` (R24 + chốt VS2), đặt sau cụm ST-136 | `viscript/agent/QUEUE.md` | `926022e4` |
| §6 | Đoạn giới hạn tạm thời ngay dưới gạch đầu dòng "Đường đọc" (FsGuard) | `viscript/agent/HOST-BAO-DAM.md` | `926022e4` |
| Push | `6f9148ad..926022e4` | `origin/main` | — |

Ghi chú nhỏ: dòng "CHỜ DUYỆT" cũ của ST-130 tôi **giữ nguyên** làm lịch sử (nội dung vẫn đúng cho phần đã làm). Nếu anh chỉ muốn thêm ghi chú mà **không** đổi ô trạng thái ST-130, báo để tôi trả lại `DONE`.

---

## 2. GO cho Agent 2 — thứ tự bắt buộc

```
ST-130 (ca thứ 8) → ST-132b → ST-134 → ST-135 → ST-137 → ST-136 → ST-138 (R24, chốt VS2)
```

Không đổi thứ tự: ST-137 trước ST-136 vì là lỗi chặn VS2; Windows đo sau cùng để số liệu phản ánh code cuối.

### ST-130 — ca thứ 8 (23b §2)
- Thêm vào `crates/vis-cli/tests/siet_caps_pkg.rs`: chương trình có **`use` module nội bộ + `use std.fs`**, **không** `pkg.*`, **không** khai `cap`, đọc một tệp ⇒ **chạy được, không VS0604**, cả **interp** và **`--vm --no-cache`**.
- Đỏ-trước nếu hiểu B; nếu đỏ vì hiểu B thì sửa trong ST-130 (>150 dòng ⇒ kế hoạch, không commit nửa vời).
- Đo lại: sem **447/22 không đổi**, diff 100k `lech=0`, gate PASS. Cập nhật dòng CHỜ DUYỆT. **Agent 1 duyệt** khi đủ.

### ST-132b — ca âm Landlock thứ 3 (23d §4.5)
- Trên `ps`: tiến trình con **`read_dir` thư mục ngoài phạm vi** ⇒ `PermissionDenied`/`EACCES`; có **đối chứng dương** `read_dir` trong phạm vi cùng ca; đỏ-trước (không `--sandbox` phải liệt kê được thư mục); 12 ca cũ vẫn xanh; log môi trường theo VD-136.

### ST-134 — vá runner + ca theo nền (VD-132, VD-136)
- **2a**: `rustup target` (wasm32) ghi vào **script tái lập trong repo** (`viscript/infra/linux/`), không cài tay; log digest/phiên bản.
- **2b**: sửa đúng **2 ca** theo VD-132 (grep prefix; `--sandbox-check`: macOS 3, Linux 0 + dòng ABI), **không** xoá/ignore/cfg-bỏ ca, mỗi nhánh khẳng định giá trị cụ thể, diff dán vào báo cáo.
- **2c**: điều tra ca (iv) `.vish` + `/tmp` trong **1 h**; kết luận 1 trong 3 loại (lỗi test / lỗi sản phẩm ≤150 dòng / >150 dòng hoặc chưa kết luận ⇒ kế hoạch).
- Liệt kê **đủ 6 ca đỏ Linux** của R23 (không phải 4), chỉ rõ ca nào là lỗi sản phẩm (dự kiến là ca TOCTOU — xử lý ở ST-137) và ca nào áp VD-132.

### ST-135 — chạy lại đầy đủ trên Linux
- `cargo test --workspace`, `gate --fast`, host suite trên `ps`; số đo đặt cạnh macOS; còn đỏ thì liệt kê, **không sửa** trong task này.

### ST-137 — sửa rò rỉ TOCTOU Linux (VD-141)
- **Bước 0 (≤45 phút, bắt buộc)**: `JOURNAL/st137-phantich.md` chỉ ra **cửa sổ thật** (chuỗi bước theo dòng code, bước nào Linux khác macOS, 1 lần lọt tái hiện có log). Không đoán rồi sửa.
- **Hướng ưu tiên** (không `unsafe`, không crate mới): sau khi mở handle, lấy đường dẫn thật qua `/proc/self/fd/<fd>` (`AsRawFd`), kiểm đường dẫn đó nằm trong vùng được cấp, **đọc từ chính handle**; `/proc` lỗi/không mount ⇒ **từ chối** (đóng an toàn), không quay về cách cũ. Đường ghi giữ giới hạn đã ghi.
- Hướng khác (`openat2`/`O_NOFOLLOW` qua libc) cần crate/`unsafe` ⇒ không làm VS2; nếu Bước 0 chứng minh hướng ưu tiên không đóng được ⇒ **dừng, kế hoạch, BLOCKED**.
- **Tiêu chí**: đỏ-trước **798/50 000** trên `ps` có log → sau sửa **0/50 000** trên `ps` **và** macOS; ca `/proc` không đọc được ⇒ từ chối; host suite **54/54 cả hai nền**; diff 100k `lech=0`; không `unsafe`, không crate mới.

### ST-136 — đo trên Windows (psvn), chỉ đo (VD-136, VD-140)
- Chỉ đo: `cargo test --workspace`, host suite, gate nếu chạy được; **không** sửa code/kỳ vọng/`cfg` cho Windows.
- `core.autocrlf=false`, target dir riêng, nguồn đúng hash đang đo; log môi trường đủ mục VD-136.
- Cập nhật **Rust toolchain đúng phiên bản `ps`** qua rustup trong `viscript/infra/windows/`; **Build Tools chỉ cập nhật khi build lỗi do bản cũ**, có log lỗi trước; **không reboot** — cần reboot ⇒ dừng, BLOCKED-ENV; không cài gì khác. Log phiên bản **trước/sau**.
- Kết quả: mỗi ca lệch ghi tên ca, đầu ra mong đợi, đầu ra Windows, nhóm nguyên nhân **phỏng đoán** (đường dẫn / CRLF / tiến trình / quyền / khác).

### ST-138 — R24 + chốt VS2 (VD-131, VD-142, VD-143)
- Đầu báo cáo: **danh sách 6 hàng phụ thuộc kèm trạng thái** (ST-130, ST-132b, ST-134, ST-135, ST-136, ST-137).
- Mục **"Đính chính R23"** (VD-142): từng điểm câu sai / câu đúng / căn cứ.
- Đủ 5 câu bắt buộc VD-131; 8 hạng mục theo 3 trạng thái; Linux và Windows **mỗi nền một mục số đo**; soak 10/10; `check_report` PASS.
- Hạng mục 4 chỉ được ghi "đóng" khi **ST-137 APPROVED**.

---

## 3. Vai Agent 1 (tôi làm gì, khi nào)

- **ST-130**: khi có ca thứ 8 xanh (8 `#[test]`, sem 447 không đổi) ⇒ tôi đối chiếu và dán APPROVED theo mẫu 23d §4.4, ghi "Agent 1 đối chiếu".
- **ST-132b**: khi có log `ps` (read_dir ngoài `EACCES` + đối chứng dương) ⇒ tôi dán APPROVED.
- **ST-137**: khi có log `ps` **0/50 000** + ca `/proc` + host 54/54 hai nền ⇒ tôi dán APPROVED; sau đó hạng mục 4 đủ điều kiện đóng.
- **ST-138**: tôi đối chiếu đủ 6 hàng phụ thuộc rồi dán APPROVED cho R24; **không** tự lật trạng thái.
- Tôi **không** sửa mã/test/R23; không chạy cổng trùng với Agent 2.

---

## 4. Trạng thái kỹ thuật lúc chốt

- HEAD **`926022e4`**; 23f đã push; `git status` sạch cho 3 tệp tôi chạm.
- `report-check`: **PASS (23 báo cáo, 155 hàng QUEUE)** — chạy trong `viscript/`.
- Không có tiến trình `cargo`/`vis-dev`/`vis-linux` đang chạy lúc 15:01 (Agent 2 đang chờ, đúng như anh nói).
- R23 vẫn **bất biến** (VD-142): mọi đính chính nằm ở QUEUE hôm nay và mục "Đính chính R23" của R24.
