# STATE — ViFastOs v2 (vòng 1)

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review).

```
HEAD          : ca2eaf5
BUILD_ID      : —
Task          : —
Bước          : —
Nhánh         : main
Cổng gần nhất : —
Việc dở       : —
```

## Ghi chú

- **L1 chưa bật tới KT-08** (chưa có sim). Từ giờ tới đó `./v2 gate --fast` chỉ gồm fmt, clippy, luật cấu trúc, L0.
- `./v2 check` / `./v2 gate --fast` **chưa tồn tại** cho tới khi KT-03 xong ⇒ ghi "cổng chưa có" vào ô `Cổng gần nhất`, không tự tạo cổng ngoài task.
- Task `REVIEW` (KT-04d) **không** được `agentctl next` trả về; chỉ mở khi có dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `QUEUE.md`.
- Tag v1: `v1-archive` trên `ca2eaf5` (KT-01). **Không** có `v1-final`; v1 không soak lại.
- Nhánh làm việc: `main`. Mỗi commit `[KT-##]`; chạm `kernel/`, `lib/` phải xanh `--fast` (D2-05).

## Lịch sử trạng thái (mới nhất ở trên)

| Ngày | Task | Bước | HEAD | BUILD_ID | Cổng | Ghi chú |
|---|---|---|---|---|---|---|
| 2026-11-12 | — | khởi tạo | `ca2eaf5` | — | — | bàn giao vòng 1: 17 dòng task, 5 READY (KT-01, KT-02, KT-04a, KT-T26, KT-R1), KT-04d REVIEW |
