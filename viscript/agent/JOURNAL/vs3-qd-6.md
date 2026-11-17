<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
> **CẬP NHẬT 17/11/2026 (VD-180 + VD-181) — thay QĐ-6 mục 1, 2 ở dưới.** Bản đầy đủ sẽ do **ST-178** viết lại. Chốt hiện hành:
> - **Mô hình phi tập trung kiểu Go**, không kho tập trung: tên gói = **đường dẫn** `vifast.org/vis/<tên>` (hoặc `github.com/…`, domain riêng); `vis.toml`: `tên_ngắn = "đường_dẫn@semver"`; phiên bản = **tag git semver**, major ≥ 2 có `/v2`; **MVS**, không bộ giải ràng buộc; `pkg update X` chỉ X + phụ thuộc bắt buộc (#142).
> - **Tin cậy**: `vis.lock` = **sha256 cây tệp chuẩn hoá**; **`sum.vifast.org`** = sổ hash chỉ ghi thêm (repo git commit ký, khoá ghim); lần đầu khớp sổ, sau khớp lock; **VS0707** = hash không khớp; **không** lệnh bỏ qua, chỉ `VIS_SUMDB=off` (cảnh báo); `VIS_PRIVATE` cho gói riêng.
> - **Dịch vụ**: `sum.vifast.org`, `proxy.vifast.org` (ngang hàng, **không** `sum.vis.`); proxy **chỉ thiết kế**, `VIS_PROXY=direct` mặc định. `vis.vifast.org` **không** làm tiền tố module; `viscript.org` chỉ thương hiệu.
> - **Traefik không phải hợp đồng**: nội dung meta tag ở `vifast.org/vis/<tên>` là hợp đồng; tên **bất biến** sau gói đầu tiên.
> - **Ca đỏ-trước bắt buộc**: tag đổi nội dung ⇒ VS0707; MVS `v1.2`/`v1.4` ⇒ chọn `v1.4`; vanity `vifast.org/vis/…` ⇒ tải đúng; offline + cache ⇒ build chạy.

# VS3-QĐ-6 · Gói: registry mạng + chữ ký + GC
**Trạng thái**: CHỜ AGENT 0 DUYỆT (VD-148) · **Phụ thuộc**: QĐ-3 (TLS) · **Mở đường cho**: —

## 1. Mục tiêu đo được
1. `pkg add` tải gói từ **registry HTTPS** (server thử trong fixtures) và **xác minh chữ ký** (Ed25519) + sha nội dung.
2. Chữ ký sai/chứng chỉ sai ⇒ **từ chối**, không ghi vào `.vis/pkg/`.
3. `pkg gc` xoá gói không còn trong `vis.lock`, **không** xoá gói đang dùng; chạy 2 lần ⇒ lần 2 không đổi.
4. `vis.lock` ghi **đủ** (tên, phiên bản, sha256, khoá ký) — test round-trip offline.

## 2. Hiện trạng (dẫn chứng)
- `crates/vis-cli/src/pkg.rs` + `crates/vis-cli/src/pkg/resolver.rs`, `semver.rs`.
- `crates/vis-cli/src/modules.rs:5-6` — nạp gói từ `.vis/pkg/` theo `vis.lock`, kiểm **sha** lúc nạp.
- Nợ: **#142** (`pkg update` ghim mọi gói khác), **#143** (NamePolicy::Strict — VS3), **#125** (mạng/chữ ký VS3).

## 3. Bảng phụ thuộc (Agent 1 xác minh bằng `cargo metadata` khi ghim)
| Crate | Giấy phép (khai) | Lý do | Ghi chú |
|---|---|---|---|
| `ed25519-dalek` | BSD-3-Clause (**kiểm lại**) | chữ ký gói | có unsafe bên trong |
| `sha2` | MIT/Apache-2.0 (**kiểm lại**) | sha nội dung | — |
| (TLS từ QĐ-3) | — | tải registry | — |

## 4. Chia task ≤ 3 h
| Task | Nội dung | Điều kiện xong | Phụ thuộc |
|---|---|---|---|
| VS3-6a | Client registry HTTPS + server thử fixtures | tải gói + ghi `.vis/pkg/` offline | QĐ-3 |
| VS3-6b | Chữ ký Ed25519 + `vis.lock` mở rộng | chữ ký sai ⇒ từ chối; lock round-trip | 6a |
| VS3-6c | `pkg gc` + test 2 lần idempotent | ca: xoá đúng, không xoá gói đang dùng | 6a |
| VS3-6d | #142 (chính sách update) + tài liệu | Agent 0 chốt chính sách; doc cập nhật | 6b |

## 5. Ca đỏ-trước
Registry trả gói có chữ ký sai ⇒ `VS0701` (hoặc mã riêng) + **không** ghi; `gc` idempotent.

## 6. Cổng
`gate --fast` + test offline (server thử) + `cargo-deny` licenses/advisories.

## 7. Rủi ro
Khoá ký/rotating; registry giả; offline; supply-chain (phải có `cargo-deny` từ QĐ-3 trước).
