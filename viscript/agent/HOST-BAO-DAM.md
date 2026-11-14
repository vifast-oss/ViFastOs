# Mức bảo đảm host của ViScript (ST-73, VD-107)

Tài liệu **ngắn** về cái gì **thật sự** được bảo đảm trên từng nền. Cập nhật: 2026-11-14.

## Bảng nền tảng ↔ bảo đảm

| Nền tảng | Cưỡng chế **runtime** (ST-72b) | Landlock (kernel) | Bằng chứng |
|---|---|---|---|
| macOS (máy phát triển) | **Có** — `FsGuard`/`NetGuard`, `VS0604`, `--grant`, đã đo 8 ca interp+VM | **Không có** (không hỗ trợ) | `vis run --sandbox-check` in `landlock: không hỗ trợ trên macos`, exit 3 |
| Linux | **Có** — cùng mã `vis-cli` (chưa đo trên máy Linux) | **Chỉ probe** — đọc `/sys/kernel/security/lsm`, **chưa** cưỡng chế bằng kernel | `cargo check --target x86_64-unknown-linux-gnu` xanh (biên dịch chéo, **chưa chạy**) |
| Windows / wasm | Chưa có host tương ứng | Không áp dụng | Chưa đo |

## Cách đọc `vis run --sandbox-check`

In ra **stdout**, **không** chạy chương trình:

```
landlock: không hỗ trợ trên macos     # hoặc: landlock: có/không (LSM: …) trên Linux
cưỡng chế runtime: bật|tắt            # bật khi `main` khai ≥1 `cap[...]` (ST-72b)
cap điều kiện: fs,net                 # loại quyền đang được cưỡng chế
```

Mã thoát: `0` = Linux có `landlock`; `3` = không có / nền khác; `4` = Linux không đọc được tệp LSM.

## Cấm ngôn từ

- **Sai**: “đã cưỡng chế bằng Landlock”, “Landlock đã bảo vệ chương trình”, “cách ly kernel đã bật”.
- **Đúng**: “**cưỡng chế ở tầng runtime** (ST-72b)”; “**probe** Landlock (ST-73) — chỉ đọc, chưa cưỡng chế”.
- `vis-host-linux` hiện là **khung probe**; nó **không** gọi syscall và **không** hạn chế quyền.

## Điều kiện để có Landlock thật (VD-107 (4))

Cần **một** trong hai, kèm runner Linux để **đo**:

1. Agent 0 duyệt crate `landlock` (MIT/Apache) vào allowlist; hoặc
2. Agent 0 duyệt `unsafe` **đóng** trong `vis-host-linux` + ca âm.

Hiện cả hai đều **chưa** được duyệt (R2 cấm `unsafe`, allowlist crate đóng).
