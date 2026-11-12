# Bản đồ tài liệu ViFastOs

Tài liệu trong repo chia **3 loại**. Trước khi đọc hoặc sửa, xác định đúng loại
để không lấy thiết kế cũ làm mô tả hiện trạng.

## 1. Tài liệu SỐNG (đọc trước, phải luôn khớp code)

| File | Nội dung |
|---|---|
| [`../README.md`](../README.md) | Giới thiệu + trạng thái tóm tắt cho người mới |
| [`00-hien-trang.md`](00-hien-trang.md) | **Trang trạng thái chuẩn**: cái gì chạy được, số liệu, việc còn mở |
| [`01-tong-quan.md`](01-tong-quan.md) | Mục tiêu, hai hướng sản phẩm, phạm vi |
| [`02-kien-truc.md`](02-kien-truc.md) | Kiến trúc thật: 25 crate, `Platform`/`Arch`, luồng điển hình |
| [`05-port-rust-va-kiem-thu.md`](05-port-rust-va-kiem-thu.md) | Build, host test, cổng QEMU, quy ước cập nhật |
| [`06-lo-trinh.md`](06-lo-trinh.md) | Lộ trình + mốc có tiêu chí đo |
| [`10-multi-arch.md`](10-multi-arch.md) | Ranh giới portable ↔ BSP của 3 port |
| [`11-hien-tai/`](11-hien-tai/00-index.md) | 18 tài liệu thành phần theo subsystem (ABI, object, IPC, VM, VFS, runtime, arch, test, virtio, pager, IOMMU, runbook…) |
| [`../ViFastOs-architecture-summary.txt`](../ViFastOs-architecture-summary.txt) | Bản tổng hợp **tự sinh** bởi `scripts/gen-docs.py` — không sửa tay |

Quy ước: mọi thay đổi hành vi phải cập nhật các file trên **trong cùng commit**.
`scripts/check-docs.py` + `gen-*.py --check` là cổng chặn.

## 2a. Thiết kế v2 đang thi công (nguồn: QD, 2026-11-12)

| Tệp | Nội dung |
|---|---|
| [`v2/ViFastOs-v2-thiet-ke.md`](v2/ViFastOs-v2-thiet-ke.md) | thiết kế v2: ABI 15 syscall + 19 lỗi, K0→K0.5→K6, D2-01..24 |
| [`viscript/ViScript-thiet-ke.md`](viscript/ViScript-thiet-ke.md) | ViScript/ViShell: lexer 2 chế độ, Phụ lục A EBNF tự chứa, ST-01..10 |
| [`viscript/SPEC-lex.md`](viscript/SPEC-lex.md) | đặc tả từ vựng (token, escape, thụt lề, chế độ lệnh, mã lỗi VS01xx) |
| [`QUYET-DINH-sau-review.md`](QUYET-DINH-sau-review.md) | quyết định của Agent 0 cho vòng vá — **nguồn sự thật**, cấm sửa |
| [`DUYET-AGENT0-vong-va.md`](DUYET-AGENT0-vong-va.md) | duyệt + errata E1–E5/S1–S11/L1–L8/Q1–Q7; cách ghi D2-25 và APPROVED |

Điều khiển: [`../v2/agent/QUEUE.md`](../v2/agent/QUEUE.md), [`../viscript/agent/QUEUE.md`](../viscript/agent/QUEUE.md); lưu trữ v1 ở [`v1/`](v1/).

## 2. Tài liệu THIẾT KẾ THAM CHIẾU (có thể lệch code)

Mô tả thiết kế đầy đủ từ giai đoạn mô hình Python (CDT, CNode nhiều cấp,
`vifast-sim`, profile manifest, IOMMU thật, compositor…). Nhiều phần **chưa được
port** sang kernel hiện tại.

- [`01-tong-quan.md`](01-tong-quan.md) phần quyết định lịch sử (đã cập nhật lại phạm vi)
- [`03-thanh-phan/`](03-thanh-phan/) — 9 tài liệu thành phần (object, cap, IPC, sched, VM, IRQ/DMA/IOMMU, fault, boot, ABI)
- [`04-diem-chu-y.md`](04-diem-chu-y.md) — danh mục điểm dễ sai / đánh đổi
- [`07-thuat-ngu.md`](07-thuat-ngu.md) — thuật ngữ
- [`08-giai-thich-cho-sinh-vien-nam-nhat.md`](08-giai-thich-cho-sinh-vien-nam-nhat.md) — giải thích nhập môn
- [`design/`](design/) — quyết định thiết kế chuyên sâu
- [`ViFastOs-agent-handoff.md`](ViFastOs-agent-handoff.md) — bàn giao giữa các phiên agent

> Mỗi file trong nhóm này có banner "thiết kế tham chiếu" ở đầu. Khi mâu thuẫn
> với code, **code + `00-hien-trang.md`** là chuẩn.

## 3. Tài liệu LỊCH SỬ (không sửa)

Ảnh chụp tại thời điểm viết: báo cáo vòng, audit, bug, decisions, bundles, soak.

- [`reviews/`](reviews/) — báo cáo từng vòng (r2xx–r3xx), `progress-g1.md`
- [`audits/`](audits/) — audit bảo mật/kiến trúc
- [`bugs/`](bugs/) — hồ sơ lỗi
- [`v1/README.md`](v1/README.md) — **kho lưu trữ v1** (tag `v1-archive`, đọc-only): báo cáo phiên, nhật ký quyết định, soak
- [`v1/decisions/log.md`](v1/decisions/log.md) — nhật ký quyết định (v1, lưu trữ)
- [`bundles/`](bundles/) — bundle bằng chứng đã ký (SHA-256)
- [`v1/overnight-log.md`](v1/overnight-log.md), [`v1/agent2-report-*.md`](v1/) — log phiên agent (v1, lưu trữ)
- [`../legacy/`](../legacy/), [`../python-model/`](../python-model/) — mô hình Python gốc

## 4. Thêm tài liệu mới

1. Nếu là **mô tả hiện trạng** → sửa file sống tương ứng, không tạo file mới.
2. Nếu là **báo cáo một vòng** → `docs/reviews/ViFastOs-report-<vòng>.md`.
3. Nếu là **quyết định** → quyết định v1: thêm mục vào `docs/v1/decisions/log.md` (kèm ngày, lý do); quyết định v2: `v2/agent/DECISIONS.md` (Agent 0 ghi), quyết định ViScript: `viscript/agent/DECISIONS.md`.
4. Cập nhật [`00-hien-trang.md`](00-hien-trang.md) nếu thay đổi ảnh hưởng trạng thái.
