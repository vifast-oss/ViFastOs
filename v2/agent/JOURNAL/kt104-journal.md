# KT-104 — shell: `cat`/`write` không tham số báo đúng "thiếu đường dẫn"

**Ngày**: 2026-11-18 · **Ngân sách**: 20 phút · **Dùng**: ~10 phút.

## Lỗi (tìm bằng thử biên)

`fs_dispatch` chỉ khớp `cat ` / `write ` **có dấu cách**. Gõ trần `cat` hoặc `write` rơi xuống nhánh
"lệnh lạ" ⇒ in `# shell: ?` — người dùng hiểu nhầm là shell **không biết** lệnh, trong khi nó biết
và chỉ thiếu tham số.

## Vá

Thêm hai nhánh `line == b"cat"` / `line == b"write"` ⇒ in `# shell: thieu duong dan`.

## Bằng chứng

```text
# trước:  cat → "# shell: ?"      write → "# shell: ?"
# sau:    cat → "# shell: thieu duong dan"   write → "# shell: thieu duong dan"
# ls (không tham số) vẫn đúng như cũ: "# shell: ls n=1 path=/"
```

## Thử biên khác (đã chạy, **không** tìm thấy lỗi)

| Lệnh | Kết quả |
|---|---|
| `cat /khongco` | `# shell: fs loi code=5` (không tìm thấy) — đúng |
| `ls /khongco` | `# shell: fs loi code=5` — đúng |
| `write /a xin chao` rồi `cat /a` | `write /a bytes=8 sync=ok` → `cat ok bytes=8 first=xin chao` — đúng |
| `ls` với đường dẫn rất dài | `# shell: fs loi code=5` — đúng, không tràn |
| dòng dài hơn `MAX_LINE` | một cảnh báo (KT-103), dòng bị cắt, không tràn buffer |

## Ghi chú

Không đụng `CMDS` ⇒ `# shell: help cmds=` giữ nguyên.
