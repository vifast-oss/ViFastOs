# Báo cáo đêm 17→18/11/2026 — dòng OS (Agent 1)

**Trạng thái**: HEAD `d2fbe4f7`, cây sạch, `v2 check` PASS, năm luật đều ok, không commit nào chưa push.
**Bối cảnh**: dòng ViScript đã tách sang repo riêng; Agent 0 offline; Agent 1 làm trong phạm vi D2-121.

## 1. Việc đã xong (mười hai task)

| Task | Nội dung | Kết quả |
|---|---|---|
| KT-92 | Đóng **A1-184**: `build.rs` truyền `--remap-path-prefix` cho cả ba chỗ gọi `$RUSTC` | hai đường dẫn khác nhau ⇒ **cùng sha**; đối chứng âm ⇒ khác sha |
| KT-93 | Script đo/build vào build-inputs | sửa `build-rev.sh` ⇒ dấu vân tay **đổi** |
| KT-84 | `task-target.sh` + `pin-artifact.sh` (ghim, `--verify`, `--selftest`) | sáu ca tự kiểm PASS |
| KT-95 | `pre-measure.sh` — sáu phép kiểm trước khi đo | sáu ca kiểm đã chạy |
| KT-96 | `soak.sh --image` — soak ảnh có sẵn | **từ chối** ảnh có shell (0/N sẽ là giả) |
| KT-97 | Kiểm lại sau thay đổi | ma trận **hai mươi mốt trên hai mươi mốt**, persist PASS, parity sáu trên sáu |
| KT-98 | `--selftest` cho hai công cụ | **bảy đột biến, bảy trên bảy bị bắt** |
| KT-99 | Soak trên ảnh canonical mới | **một nghìn trên một nghìn**, wall năm trăm ba mươi ba giây |
| KT-100 | Vá **bốn lỗ fail-open** do reviewer độc lập phát hiện | `missing-input`, `no-pgrep`, `--allow-unverified`, soak chỉ nhận timeout |
| KT-101 | Conformance trên HEAD | `trace-compare` **sáu mươi mốt trên sáu mươi mốt** dòng khớp sim ↔ QEMU |
| KT-102 | `v2 status [--json]` | một màn hình trạng thái |
| KT-103/104/105 | ba lỗi shell tìm bằng thử biên | xem §3 |

## 2. Hai việc giấy tờ đã mở đường cho K6

- **A1-179 XONG**: năm mươi tám số của `k6-exec`/`k6-persist`/`K6-DOD` ra khỏi prose ⇒ `pre_existing` bằng không,
  mọi tài liệu K6 qua luật một **không cần miễn trừ**.
- **KT-83 XONG**: bảng bảy lỗi v1 với `file:dòng`; kèm **đính chính ba chỗ sai** trong `K6-DOD.md`
  (hai dẫn lệch dòng, và N5 phải là **HOÃN K5′** không phải "N/A").

## 3. ba lỗi thật trong shell (tìm bằng thử biên, không phải đọc code)

| Lỗi | Triệu chứng | Vì sao không test nào bắt |
|---|---|---|
| **KT-103** | dòng dài hơn `MAX_LINE` in **một cảnh báo cho mỗi byte** ⇒ ngập log | không có ca kiểm dòng dài |
| **KT-104** | `cat`/`write` không tham số in `# shell: ?` (như lệnh lạ) | parity chỉ gửi lệnh có tham số |
| **KT-105** | `ps` in `state=Unknown` cho **mọi** tiến trình | **test dùng chung lỗi với shell** |

**KT-105 là bài học quan trọng nhất**: `proc_demo.rs` dựng dòng kỳ vọng bằng **cùng** mask sai
(`& 0x1F`) ⇒ test và shell "khớp nhau" mà cả hai đều sai. Khi vá shell, ma trận **mười tám trên hai mươi mốt đỏ** — đó
là lúc lỗi lộ ra. Nếu chỉ sửa test cho xanh thì đã **che** lỗi.

## 4. Ảnh canonical mới nhất

Ảnh `34ee7a25…`, dấu vân tay `be9689a5c032cf69`, cây sạch, `rev=244f06d11777`. Dòng `REPRO` nguyên
văn: `v2/agent/JOURNAL/kt105-journal.md` (mục "Ảnh canonical mới nhất").

Đã ghim; parity sáu trên sáu; persist PASS; ma trận hai mươi mốt trên hai mươi mốt. **Chưa** soak một nghìn trên ảnh này (soak gần nhất
là `dd9f7050…`, trước KT-103/104/105).

## 5. Cần Agent 0 quyết

| STT | Việc | Ghi chú |
|---|---|---|
| một | **Tiêu chí bốn** dùng số nào — `K6-CLOSURE-DRAFT.md` §2, phương án (a) hay (b) | lead đề xuất **(b)** |
| hai | **KT-91** (ba lệnh `time`/`uptime`/`jobs`) | đổi `# shell: help cmds=` ⇒ phải sửa kỳ vọng harness |
| ba | **B6** — build-inputs có gồm `v2/agent/**` không | A1-188 đang tạm loại trừ |
| bốn | **KT-82a** — số đo viết bằng **chữ** để qua luật một | giữ thông tin nhưng **luật không kiểm máy được**; có làm lại bằng khối `scope=` không |
| năm | Thêm ca kiểm `ps` **không** in `state=Unknown` | đụng kỳ vọng harness (parity hoặc TAP) |
| sáu | Xác nhận **A1-188…A1-193** | các quyết định thay mặt trong đêm |

## 6. Việc còn lại (không chờ A0)

- Soak một nghìn trên `34ee7a25…` (ảnh mới nhất).
- Nối `pre-measure.sh` vào bốn runner (đổi phương pháp đo ⇒ cần A0).
- KT-81 (thiết bị giả nghiêm ngặt) — theo D2-119 làm **sau** khi K6 đóng.
