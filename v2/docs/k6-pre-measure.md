# KT-95 — kiểm trước khi đo (`pre-measure.sh`)

## Vì sao

Trong một phiên đã xảy ra **năm** lần cùng một lớp lỗi (D2-108A): đo một ảnh nhưng trích số từ ảnh
khác, hoặc đo ảnh build từ cây khác. **Mỗi lần** đều phát hiện **sau** khi đo xong — có lần sau khi
đã báo cáo.

Các phép kiểm cần thiết đã có rải rác (`build-rev.sh`, `pin-artifact.sh`, `git status`), nhưng không
ai chạy **đủ** trước khi đo. Script này gom chúng vào một chỗ.

## Dùng

```bash
bash v2/tools/pre-measure.sh <ảnh>
# PRE-MEASURE: OK sha256=… size=… head=… inputs=… pinned=…
```

Chạy **trước** mọi phép đo (soak, ma trận, persist, parity). Nếu nó từ chối, **không** đo.

## Sáu phép kiểm (theo thứ tự, dừng ở lỗi đầu tiên)

| Mã thoát | Lý do | Nghĩa |
|---|---|---|
| ba | `no-build-line` | ảnh không tự khai nguồn ⇒ không chứng minh được gì |
| bốn | `legacy-format` | ảnh định dạng **cũ** (`rev=`), không so được với cây hiện tại |
| bốn | `inputs-mismatch` | dấu vân tay ảnh **khác** cây hiện tại ⇒ ảnh build từ nội dung khác |
| năm | `image-dirty` | ảnh tự khai build từ cây bẩn |
| sáu | `not-pinned` | ảnh **chưa** nằm trong `v2/artifacts/<sha256>.bin` |
| bảy | `tree-dirty` | cây nguồn đang bẩn trong phạm vi build-inputs |
| tám | `qemu-orphans` | còn QEMU đang chạy ⇒ số đo sẽ nhiễu |
| tám | `no-pgrep` | không có `pgrep` ⇒ **không chứng minh được** "không có QEMU sót" (KT-100) |

Ngoài ra: `no-git` (bảy) khi không đọc được HEAD, `no-current-fingerprint` (bốn) khi không tính được
dấu vân tay cây hiện tại.

## Cờ

- `--quiet` — chỉ in dòng kết luận.
- `--allow-legacy` — chấp nhận ảnh định dạng cũ. **Chỉ** dùng khi đo lại một ảnh lịch sử có chủ ý;
  ảnh đó **không** chứng minh được nó khớp cây hiện tại.

## Ca kiểm (đã chạy)

| Ca | Kỳ vọng | Kết quả |
|---|---|---|
| ảnh mới `dd9f7050…`, cây sạch, đã ghim | `OK` | đạt |
| ảnh cũ `291502d2…` (định dạng `rev=`) | `REFUSE legacy-format` | đạt |
| ảnh cũ + `--allow-legacy` | `OK` | đạt |
| ảnh đã sửa một byte (sha lạ) | `REFUSE not-pinned` | đạt |
| cây bẩn (`v2/kernel`) | `REFUSE tree-dirty` | đạt |
| có QEMU đang chạy | `REFUSE qemu-orphans` | đạt |

## Chưa làm

- **Chưa** nối vào bốn runner (`ktest.sh`, `soak.sh`, `persist.sh`, `run.sh`). Nối vào là **đổi
  phương pháp đo** (thêm điều kiện từ chối) ⇒ cần Agent 0 duyệt. Hiện là quy ước: chạy tay trước
  khi đo.
