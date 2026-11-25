// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-404** (`B4`, D2-259) — crate **host** dùng chung cho ba `build.rs` của
//! `vifast-kbin-{riscv64,aarch64,x86_64}`.
//!
//! # Vì sao có crate này
//!
//! `KT-346` dời userland sản phẩm sang `v2/user/`; ba `build.rs` phình lên
//! `547 + 857 + 492 = 1.896` LOC với **15 hàm gần như giống hệt nhau**:
//!
//! | Khối | LOC mỗi bản | Lặp |
//! |---|---:|---:|
//! | `build_identity` + `git_*` + `identity_watch_paths` + `is_algo`/`is_inputs16`/`normalize_rev` | ~230 | **3×** |
//! | `remap_flags` | ~30 | **3×** |
//! | `compile_rlib`/`build_rlib_at` | ~70 | **3×** |
//! | `compile_user_elf`/`build_user_elf_at` | ~80 | **3×** |
//! | `write_linker_script` | ~25 | **2×** |
//!
//! Ba bản chỉ khác nhau **3 dòng** (tên crate arch trong `identity_watch_paths` + `v2/user` trong
//! pathspec `git status` của `riscv64`). Sửa một lỗi ở một bản là hai bản còn lại vẫn sai — đúng
//! lớp lỗi KT-76/KT-89 (ảnh mang danh tính cũ) và A1-184 (đường dẫn tuyệt đối lọt vào byte ảnh).
//!
//! # Phạm vi (cố ý hẹp)
//!
//! Crate này **chỉ**:
//!   1. [`identity::build_identity`] — tính + nhúng danh tính bản dựng (`rev`/`dirty`/`inputs`/
//!      `algo`) vào ảnh, **một** nguồn sự thật cho cả 3 arch;
//!   2. [`compile`] — gọi `$RUSTC` (cargo cấp) biên dịch `rlib` + ELF U-mode từ nguồn trong
//!      `v2/user/` (và `init/` của từng ảnh ktest) rồi trả đường dẫn cho `build.rs` nhúng;
//!   3. [`user`] (**KT-404c**) — cơ chế dựng **một bảng** artefact U-mode: ba `build.rs` khai
//!      `Program`/`Build` (dữ liệu của arch) và gọi [`user::build`], không tự viết vòng lặp;
//!   4. [`cargo`] (**KT-404c**) — ba khối khai báo `cargo:` mà cả ba `build.rs` đều phải in
//!      (`feature`, `link_kernel`, `rerun_if_changed`).
//!
//! Nó **không** biết ảnh nào cần artefact nào: danh sách artefact là kiến thức **của từng arch**
//! nên vẫn nằm trong `build.rs` tương ứng (đúng `KE-HOACH-KIEN-TRUC.md` dòng `KT-404`: build.rs
//! chỉ còn khai báo + nhúng).
//!
//! # Bất biến
//!
//! * **0 phụ thuộc ngoài** (D2-21 allowlist giấy phép) và `#![forbid(unsafe_code)]`.
//! * Mọi lần gọi `$RUSTC` **phải** truyền [`compile::remap_flags`] — nếu không, đường dẫn tuyệt
//!   đối của thư mục nguồn lọt vào byte ELF ⇒ cùng commit ở hai thư mục cho hai ảnh khác nhau
//!   (A1-184, nợ P20/`N-7`).
//! * `KT-404c` **không** đổi một byte đầu ra nào: `user::build` truyền nguyên giá trị + thứ tự
//!   tham số của ba vòng lặp cũ cho [`compile::compile_user_elf`].
#![forbid(unsafe_code)]

pub mod cargo;
pub mod compile;
pub mod identity;
pub mod user;
