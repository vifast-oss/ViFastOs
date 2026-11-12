# STATE — ViScript (vòng 1)

Người ghi: **Agent 2** dòng ViScript. Cập nhật **sau mỗi bước**.
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

- Giai đoạn đầu (`ST-01` chưa xong) **chưa có** `./vis-dev check` / `./vis-dev gate --fast` ⇒ ô `Cổng gần nhất` ghi "cổng chưa có".
- `ST-02a` **DONE** (APPROVED 2026-11-12 Agent 0). **ST-04 mở ngay**; 60 golden token là test đỏ đầu tiên của ST-04.
- Việc làm được ngay, không chờ duyệt: `ST-01`, `ST-02b`, `ST-10`; `ST-03` mở khi `ST-01` xong.
- Vòng 1 **không** có interpreter/kiểm kiểu; dừng ở lexer → parser/CST → `CmdCST` → `vis-fmt` → runner `--check` cú pháp.
- Commit `[ST-##]`; SPDX `#` cho `.vis .vish .vidl .toml`, `//` cho Rust; mọi `unsafe` có case âm; tệp ≤ 1.500 dòng.
- Ưu tiên hai dòng: OS trước (D2-20); dòng này chạy khi `v2/agent/QUEUE.md` không còn task `READY`, ST-02a đã DONE; ST-04 mở ngay.

## Lịch sử trạng thái (mới nhất ở trên)

| Ngày | Task | Bước | HEAD | BUILD_ID | Cổng | Ghi chú |
|---|---|---|---|---|---|---|
| 2026-11-12 | — | khởi tạo | `ca2eaf5` | — | — | bàn giao vòng 1: 10 task, 3 READY (ST-01, ST-02b, ST-10), ST-02a DONE; ST-04 READY |
