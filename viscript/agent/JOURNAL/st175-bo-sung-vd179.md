<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# ST-175 · BỔ SUNG THEO VD-179 §2 — phần ViScript trong cây `ps` bẩn
**Làm trong lúc đóng băng** (chỉ đọc, không gate). Thời điểm: 17/11/2026 ~15:32 JST.

## a. Crate ViScript — lấy bằng `cargo metadata` (không viết tay)
`cargo metadata --manifest-path /root/vifast-linux/viscript/Cargo.toml --no-deps` ⇒ **8 package**, tất cả dưới `viscript/`:
`vis-diag` · `vis-syntax` · `vis-cli` · `vis-bytecode` · `vis-types` · `vis-fmt` · `vis-interp` · `vis-vm`
(đường dẫn manifest: `viscript/crates/<tên>/Cargo.toml`).

## b. Đếm trong `git status --short` của `/root/vifast-linux` (2338 mục)
| Nhóm | Số mục | Ghi chú |
|---|---|---|
| Toàn cây | **2338** = **2297 xoá (D)** + **25 sửa (M)** + **16 untracked (??)** | |
| **`viscript/`** | **17** = **0 xoá** + **3 sửa** + **14 untracked** | 3 sửa: `agent/HOST-BAO-DAM.md`, `crates/vis-cli/src/main.rs`, `crates/vis-cli/src/quyen.rs` — đều là **bản ViScript tự sửa SAU ST-167** (ST-173/174), không phải xoá; 14 untracked: log `st167-*`/`st174-*` + `crates/vis-cli/src/quyen_st174.rs` + `target/` |
| **`crates/` (gốc)** | **93** — **toàn bộ là dòng OS** (`crates/vifast-abi`, `vifast-acpi`, `vifast-arch-*`, …), **không** phải crate ViScript | 93 tệp bị xoá thuộc OS line, không thuộc 8 package ở (a) |
| Các crate ViScript (8) | **0 tệp bị xoá**, **0 tệp sửa** ngoài 3 tệp kể trên | tính theo tiền tố `viscript/crates/` |

⇒ **Phần ViScript KHÔNG có tệp bị xoá.** “93 tệp bị xoá ở `crates/`” là **dòng OS**, không phải mã ViScript (mã ViScript nằm ở `viscript/crates/`, không phải `crates/`).

## c. Reflog `ps` — HEAD lúc chạy ba gate
| Gate | HEAD ghi trong log | Reflog `ps` | Ghi chú |
|---|---|---|---|
| ST-161 | `0df923c2` (header `st161-gate-ps.log`) | `0df923c2 HEAD@{2026-11-17 10:06:15}` commit `[ST-162]` | khớp |
| ST-166 | `6f8f63a5` (header `st166-sau-ps.log`) | có trong reflog trước 12:35 (OS line) | khớp mốc thời gian |
| ST-167 | `795df046` (header `st167-soak-ps.log`) | `795df046 HEAD@{2026-11-17 13:00:04}` commit `[KT-77b]` | khớp |

**Mã ViScript KHÁC nhau giữa ba gate** (đúng lịch sử, không phải lỗi): `git rev-parse <HEAD>:viscript/crates` =
`c8102181…` (ST-161) → `1a576761…` (ST-166) → `df4da818…` (ST-167); RIÊNG `viscript/crates/vis-cli/src/quyen.rs` +305 dòng trong khoảng đó (ST-163 chẩn đoán `#146`, ST-165/166 sửa `quyen.rs`).

## Kết luận (theo VD-179 §2)
- **Phần ViScript sạch theo kiểm hồi tố**: **0 xoá** trong `viscript/`; 3 tệp sửa hiện tại là **bản ViScript tự sửa sau ST-167** (ST-173/174), không phải nhiễu từ cây bẩn; 93 tệp xoá ở `crates/` là **dòng OS**.
- **Ba gate giữ được**, nhưng chỉ ở mức **“sạch theo kiểm hồi tố”** (dựa trên: 0 xoá ở `viscript/` + HEAD khớp reflog + 3 tệp sửa có mốc sau gate) — **không** chứng minh được cây làm việc sạch tại đúng phút chạy gate (log chỉ ghi **commit**, không ghi trạng thái cây).
- **Mã ViScript khác nhau giữa ba gate**, nên R29 **phải dùng gate mới trên clone sạch** (VD-179 §3) làm số ps chuẩn; số cũ chỉ là tham chiếu.

## Đề xuất ghi vào R29
> Ba gate `ps` cũ (ST-161/166/167) chạy trên cây bẩn **của dòng OS**; phần ViScript sạch theo kiểm hồi tố (0 xoá ở `viscript/`, HEAD khớp reflog, 3 tệp sửa có mốc sau gate) ⇒ giữ làm tham chiếu, **không** dùng làm số chuẩn. Số ps chuẩn của R29 = gate một lần trên **clone sạch mới** (VD-179 §3).
