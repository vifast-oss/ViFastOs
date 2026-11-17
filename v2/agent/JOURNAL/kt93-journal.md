# KT-93 — đóng khe tin-cậy: đưa script đo/build vào dấu vân tay build-inputs

**Ngày**: 2026-11-17 · **Ngân sách**: 45 phút · **Dùng**: ~15 phút.

## Khe hở (review verify-kt89 nêu)

`VBREV_INPUT_ROOTS` chỉ gồm `v2/kernel v2/lib v2/bin`; `VBREV_INPUT_FILES` chỉ gồm
`v2/Cargo.toml v2/Cargo.lock v2/rust-toolchain.toml`. Ba tệp **quyết định phép đo và byte ảnh**
không nằm trong dấu vân tay:

| Tệp | Vì sao phải nằm trong dấu vân tay |
|---|---|
| `v2/test/boot/build-rev.sh` | **định nghĩa** dấu vân tay — đổi nó là đổi phép đo |
| `v2/tools/repro-build.sh` | đặt `RUSTFLAGS` (`--remap-path-prefix`, `-C strip=symbols`) ⇒ đổi byte ảnh |
| `v2/tools/rustc-canon.rs` | wrapper rustc canonical ⇒ đổi byte ảnh |

Hệ quả trước khi vá: đổi **định nghĩa** dấu vân tay mà dấu vân tay **không đổi** ⇒ cổng tự tính lại
bằng chính script mới nên tự nhất quán, chỉ còn tin cậy ở review. Đây là khe **tin-cậy**, không
phải build-input bị giấu — nhưng vẫn là khe.

## Vá

Thêm ba tệp vào `VBREV_INPUT_FILES`. Danh sách build-inputs: **260 → 263** mục; dấu vân tay
`ba0b394538234864` → `64e2a74e4f7255da`.

## Bằng chứng (clone `/tmp/kt93`)

```text
fp0 (HEAD + build-rev.sh moi, chua commit)          = 64e2a74e4f7255da  dirty=1
sua build-rev.sh, CHUA commit                        = 64e2a74e4f7255da  dirty=1   (dirty bat, fp dung)
commit thay doi trong build-rev.sh                   = 9a723fb345ba7137  dirty=0   (fp DOI — khe da dong)
revert commit do                                     = 64e2a74e4f7255da  dirty=0   (fp quay ve)
```

Trước KT-93, dòng thứ ba sẽ **không đổi** (đó chính là khe). Nay đổi.

## Hệ quả

- Mọi thay đổi `build-rev.sh`/`repro-build.sh`/`rustc-canon.rs` **buộc** build lại ảnh trước khi đo,
  nếu không cổng từ chối `inputs-mismatch`. Đúng ý D2-127.
- Dấu vân tay đổi ⇒ ảnh phát hành cũ `291502d2…` **không còn** khớp HEAD; mọi phép đo mới phải
  build lại. Đây là hệ quả **có chủ ý**, không phải hồi quy.
