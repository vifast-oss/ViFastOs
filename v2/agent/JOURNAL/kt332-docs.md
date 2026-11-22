# KT-332 — đồng bộ tài liệu công khai, cắt gọn `STATE.md`, dọn `BLOCKED.md`

**Ngày**: 2026-11-22 (Chu kỳ 2, Đợt 0). **Worktree**: `/tmp/kt332-s1`, nhánh `task/kt332-docs`,
base `main` @ `0809b93a`. **Sub-agent**: `docssync`.

**Đã đọc trước khi sửa**: `AGENTS.md`, `v2/AGENTS.md`, `v2/agent/DECISIONS.md`
(D2-229..D2-240, nhất là **D2-230**), `KE-HOACH-CHU-KY-2-2026-11-22.md` (mục `KT-332`),
`v2/docs/k6-test-invariants.md` (luật số đo E1), `v2/agent/STATE.md`, `v2/agent/BLOCKED.md`,
`README.md`, `v2/README.md`, `BAO-CAO-CUOI-TUAN-KT243-2026-11-22.md`.

**Ràng buộc đã giữ**: không sửa `v2/kernel/**`, `v2/lib/**`, `spec/`, `v2/bin/v2`,
`docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/v1/**`; không push,
không merge, không chạm `main`; **không chạy `qemu-system*`**; `git add` theo đường dẫn cụ thể;
không thêm số đo viết tay vào tài liệu công khai (chỉ journal này giữ số đo khảo sát).

---

## 1. Khảo sát — khẳng định cũ → thực tế → nguồn

| # | Tệp:dòng (cũ) | Khẳng định cũ | Thực tế sau `KT-243..KT-330` | Nguồn |
|---|---|---|---|---|
| 1 | `README.md:9`, `:79` | v2 có **15 syscall** (0–13 + Exit) | **16** syscall, dải **0..15**: 14 = `Exit`, 15 = `DebugGetInfo` (D2-29) | `spec/abi.toml` `[syscall]`; `docs/abi.md` "## Syscall (0..15)" |
| 2 | `README.md:11` | v2 "mới xong K0", "chưa có kernel để boot"; việc tiếp theo KT-03 rồi KT-07 | Cả **ba kiến trúc** boot trên QEMU; **K6** và **K7** ĐẠT; đang ở **Chu kỳ 2** (`KT-330..KT-348`) | khối `STATE.md`; `v2/agent/reports/K6-CLOSURE.md`; `DECISIONS.md` D2-143/D2-205 |
| 3 | `README.md:85` | v2 "**Chưa boot** (mới xong K0)" | Chạy 3 kiến trúc trên QEMU; K6/K7 ĐẠT | `STATE.md` dòng `AARCH64`/`X86_64`/`RISCV64`; `K6-CLOSURE.md` |
| 4 | `README.md:87` | "v2 làm riscv64 trước, hai cổng còn lại sau" | Cả ba cổng đã boot; `aarch64`/`x86_64` **chưa nối** `vifast-core` — việc của Chu kỳ 2 | `STATE.md` "Nợ mở"; `DECISIONS.md` D2-233 (KT-338b/340c) |
| 5 | `README.md:107` | "KT-01/KT-02 DONE; KT-03 tiếp theo; KT-07 mới tạo kernel" | K6/K7 ĐẠT; Chu kỳ 2 bốn đợt đang chạy | `QUEUE.md` (KT-330..KT-348); `K6-CLOSURE.md`; `BAO-CAO-QUA-DEM-2026-11-19.md` |
| 6 | `README.md:117-125` | `bin/` "sau KT-03", `kernel/` "sau KT-07"; có `viscript/` + `docs/viscript/` | `v2/bin/` có `v2`/`vifast-gen`/`vifast-idl`; `v2/kernel/` có `vifast-core` + 3 platform + 3 kbin; thêm `v2/sim/`, `v2/test/`, `v2/tools/`, `v2/docs/`, `spec/`, `review/`; **ViScript tách repo riêng** | `ls v2`; `v2/Cargo.toml`; `VISCRIPT.md` |
| 7 | `README.md:122,125,128,201` | Đường dẫn `viscript/` và `docs/viscript/...` | Không còn trong repo; dòng ViScript tách sang repo riêng | `VISCRIPT.md`; `ls docs` (không có `docs/viscript`) |
| 8 | `README.md:153` | Số v1 viết tay: "389 host test, 677 case âm, 3/3, treo ~0,3–1 %" | Giữ **định tính**, trỏ `docs/v1/` (QD §9 Q12: không số đo viết tay) | `docs/v1/README.md`, `docs/v1/KNOWN-ISSUES.md` |
| 9 | `README.md:155-163` | "v2 (đang xây, chưa boot)"; `v2/bin/v2` "sẽ có sau KT-03" | Lệnh cổng có thật: `check`, `gate --dev`, `gate --fast`, `status`, `check-rules`, `rules` | `v2/bin/v2` usage; `AGENTS.md` §6 |
| 10 | `README.md:214-215` | "v2 **sẽ** có bộ test riêng" (tương lai) | Đã có: sim tất định, mô hình tham chiếu, QEMU TAP, `v2 check`/`gate --fast` | `v2/bin/v2`; `AGENTS.md` §6.1–6.2 |
| 11 | `v2/README.md` (toàn bộ) | "Cây hiện có (**vòng 1**)"; `bin/` "hiện chưa có"; "L1 chưa bật cho tới KT-08"; `gate --fast` "chưa có" | Cây đầy đủ sau KT-330; `bin/` có 3 lệnh; L1 bật từ KT-08; T2 là `gate --fast` | `v2/bin/v2`; `v2/Cargo.toml`; `AGENTS.md` §6 |
| 12 | `BAO-CAO...md:7-13` | "88 task", nhưng chỉ liệt **vòng 1..18**; `HEAD cuối 2e8b67c8` | **25 vòng**: `KT-243..KT-329` = 87 task + `KT-330` = 88; HEAD cuối `0809b93a` | `git log`; `QUEUE.md`; `STATE.md` |
| 13 | `BAO-CAO...md` §5 | "Việc còn mở sau cuối tuần (cập nhật sau **vòng 7**)" — 14 mục, phần lớn đã đóng | Giữ câu chữ gốc + trạng thái cuối tuần từng mục (đóng/giảm/còn mở) | `QUEUE.md`, `DECISIONS.md` D2-205/D2-233..240, §3 báo cáo |

**Kích thước `STATE.md` trước khi sửa**: 343.850 B (**335,79 KiB**), 4.088 dòng — vượt xa mục
tiêu **< 30 KiB**. **`BLOCKED.md`**: 50.968 B (**49,77 KiB**), 532 dòng; các mục đã đóng vẫn nằm
lẫn với mục mở.

---

## 2. `STATE.md` — tách lịch sử

- Tạo `v2/agent/archive/STATE-chu-ky-1.md`: header lưu trữ + **nguyên văn** phần lịch sử
  (dòng 41..4088 của tệp cũ = **4.048 dòng**). Kiểm bằng Python: `archive.endswith(history) == True`.
- Bỏ **một** dòng rào mã (fence) treo ở đầu phần lịch sử gốc (dòng 40 cũ — mở mà không đóng) để
  tệp hiển thị được; **không** mất dòng chữ nào (`endswith` ở trên đã chứng minh).
- `STATE.md` mới: 43 dòng, **3.665 B (3,58 KiB)** — dưới trần 30 KiB rất xa.
- **Khối trạng thái hiện tại giữ nguyên tuyệt đối**: sha256 của 34 dòng khối (dòng 6..39 cũ so với
  dòng 10..43 mới) **giống nhau**: `626a8e14eaf236f96173f131d0c825fe0859834155fe1384c028417ed1d2e42d`
  (`cmp` trả `SAME_BLOCK`). Hai dòng mốc `=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật … ===`
  và `=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===` **không đổi định dạng**.
- Thêm trước khối 4 dòng hướng dẫn (nguồn trạng thái duy nhất; lịch sử ở archive; không đổi định
  dạng hai dòng mốc) — **không** đặt gì sau khối.

## 3. `BLOCKED.md` — dọn mục đã đóng

- Tạo `v2/agent/archive/BLOCKED-chu-ky-1.md`: header lưu trữ + **nguyên văn** toàn bộ tệp cũ
  (532 dòng). `archive.endswith(old) == True`.
- Chuyển **43 mục đã đóng**: bảng REVIEW vòng 1 (7 hàng + ghi chú O1), bảng BLOCKED theo
  `PATCH-BRIEF` §1 (10 hàng), 16 mục §3 (`YC-18`, `Q-01`, `YC-01..06`, ghi chú `KT-04d`,
  `YC-11..17`), §4 dòng ViScript, §5 `KT-67`, và 7 mục `KT-74` / `KT-75` / `KT-76`+S1 / `KT-78` /
  `KT-77a` / ghi chú `KT-80` / vá `KT-77a` sau review.
- `BLOCKED.md` mới (50 dòng, 4.315 B) giữ:
  - **§1 mục BLOCKED đang mở: không có** (sau KT-330).
  - **§2 nợ có chủ kế thừa** (9 mục): `CNode.Mutate/Rotate`, `CNode.Insert`, `Tcb.Exec(path)`,
    ca lỗi-muộn + SMP của `exec`, `ping`/`ifconfig` trong shell sản phẩm, đọc DTB `aarch64`,
    `x86_64` `VSpace`/W^X/TLB, nợ P20, số TAP mặc định hard-code.
  - **§3 giới hạn có văn bản** (KT-75, KT-78, tái lập K6).
  - **§4 chuyển tiếp theo lịch** (`KT-345a`, các task `BLOCKED` theo DAG).
  - **§5 trỏ archive** (không xoá thông tin).

## 4. `README.md` / `v2/README.md`

- `README.md`: sửa 10 điểm ở bảng §1 (dòng 9, 11, 79, 85, 87, 106-108, 117-128, 153, 155-163,
  201, 214-215). **Không thêm số đo**; số duy nhất được sửa là hằng thiết kế **16 syscall** (từ
  `spec/abi.toml`, không phải số đo) và **bỏ** các số v1 viết tay (389/677/0,3–1 %) — thay bằng
  trỏ `docs/v1/`.
- `v2/README.md`: viết lại (3.178 B) theo cây thật sau KT-330, lệnh cổng thật, ba tầng kiểm, và
  ghi rõ **lệch đã biết** `v2/kernel/Cargo.toml` còn `rust-version = "1.99.1"` (KT-334 đồng bộ,
  D2-231). Không nêu TAP/thời gian đo.

## 5. `BAO-CAO-CUOI-TUAN-KT243-2026-11-22.md`

- **Đầu trang**: thay khối "88 task" chỉ liệt vòng 1..18 bằng **25 vòng đầy đủ** (`KT-243..KT-329`
  = 87 task) + **KT-330** (khép soak vòng 25, Đợt 0 Chu kỳ 2) = **88 task**; HEAD cuối
  `0809b93a` (đã push). Giữ nguyên dòng "Người thực hiện/Hạn/Kế hoạch".
- **§5**: đổi tiêu đề thành "cập nhật sau vòng 25"; giữ **nguyên câu chữ** 14 mục (lịch sử) và
  thêm trạng thái cuối tuần cho từng mục: **ĐÓNG** 1, 5, 6, 7, 11, 12; **ĐÓNG phần lớn** 2, 3,
  10, 13; **CÒN MỞ** 4 (P20), 9 (đã lỗi thời — ảnh sản phẩm đổi), 14 (giảm). Không thêm số đo mới.
- **§3.x giữ nguyên** làm lịch sử (không sửa).

## 6. Cổng

| Lệnh | Kết quả | Ghi chú |
|---|---|---|
| `./v2/bin/v2 check-rules` | **PASS** — `# verify-numbers: ok=5 fail=0` (21 ca self-test) | không thêm số đo vào tài liệu đích |
| `./v2/bin/v2 check` | **PASS** | 40/40 bước T0 (fmt, clippy, R1..R5, checker, L0); **không chạy QEMU** |

Không chạy `qemu-system*` (máy soak độc quyền của lead). Không chạy `gate --fast` (T2) — ngoài
phạm vi KT-332 và cần máy đo.

## 7. Commit

`[KT-332] ...` trên nhánh `task/kt332-docs` (chỉ commit; **không** push/merge). `git status` sạch
sau commit. `git diff main --stat -- v2/kernel v2/lib spec v2/bin/v2` = **rỗng**.
