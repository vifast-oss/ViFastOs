// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-404c** — bảng artefact U-mode: **một** cơ chế cho ba `build.rs` của `vifast-kbin-*`.
//!
//! # Vì sao
//!
//! `KT-404` rút phần *máy móc* về crate này nhưng **ba** `build.rs` vẫn còn ba vòng lặp gần như
//! giống hệt nhau: mỗi vòng gọi [`write_linker_script`](crate::compile::write_linker_script), gọi
//! `$RUSTC` cho từng chương trình, rồi in `rerun-if-changed` cho nguồn của nó. Ba bản sao của
//! cùng một vòng lặp là lớp lỗi KT-346 (một bản sửa, hai bản còn lại vẫn sai).
//!
//! # Phân chia trách nhiệm (giữ đúng `KE-HOACH-KIEN-TRUC.md` dòng `KT-404`)
//!
//! * Crate này biết **cách** dựng artefact U-mode: `rlib` `--extern`, `.ld` sinh theo VA, cờ
//!   `-C link-arg` riêng của arch, `rerun-if-changed` cho từng tệp nguồn.
//! * `build.rs` của từng arch biết **cái gì**: nguồn nào, tên ELF nào, VA nào, nhúng `rlib` nào
//!   (bảng [`Program`] + [`Build`]) — kiến thức của arch **không** dời vào đây.
//!
//! # Bất biến (byte-identical với trước `KT-404c`)
//!
//! Mọi tham số của [`compile_user_elf`](crate::compile::compile_user_elf) được truyền **nguyên
//! giá trị** và **nguyên thứ tự** như ba vòng lặp cũ: cùng `cwd`, cùng đường dẫn nguồn (không
//! `canonicalize` — đường dẫn tuyệt đối lọt vào byte ELF là nợ P20/A1-184), cùng `--extern`, cùng
//! `link-arg`. Vì vậy ảnh và mọi artefact `$OUT_DIR` **không đổi một byte**.

use std::path::PathBuf;

use crate::compile::{self, LinkerScript};

/// Chỉ số nhóm `--extern` của một chương trình, trỏ vào [`Build::sets`].
pub type Set = usize;

/// Một chương trình U-mode cần dựng cho ảnh.
///
/// Dựng bằng [`Program::new`] (+ [`Program::at`] cho linker script sinh theo VA, [`Program::with`]
/// cho cờ rustc phụ) để bảng artefact trong `build.rs` đọc được như dữ liệu, không như mã.
#[derive(Clone, Copy)]
pub struct Program<'a> {
    /// Tên tệp nguồn trong [`Build::src_dir`].
    pub src: &'a str,
    /// Tên ELF trong `$OUT_DIR` (cũng là tên `include_bytes!` trong crate ảnh).
    pub out: &'a str,
    /// Nhóm `--extern` của chương trình này (`0` = không `--extern`).
    pub set: Set,
    /// VA liên kết — chỉ dùng khi [`Link::Generated`]; bỏ qua (giữ `0`) khi [`Link::Fixed`].
    pub base: u64,
    /// Cờ rustc phụ **riêng của chương trình này** (`--cfg shell_fs`, `--cfg rt_alloc`, …).
    pub extra: &'a [&'a str],
}

impl<'a> Program<'a> {
    /// Chương trình ở [`Set`] `set`, không cờ phụ, VA `0` (dùng [`Link::Fixed`]).
    pub const fn new(src: &'a str, out: &'a str, set: Set) -> Self {
        Self {
            src,
            out,
            set,
            base: 0,
            extra: &[],
        }
    }

    /// VA liên kết của chương trình (bắt buộc khi linker script sinh theo [`Link::Generated`]).
    pub const fn at(mut self, base: u64) -> Self {
        self.base = base;
        self
    }

    /// Cờ rustc phụ riêng của chương trình (`&[]` khi không cần).
    pub const fn with(mut self, extra: &'a [&'a str]) -> Self {
        self.extra = extra;
        self
    }
}

/// Nguồn của linker script dùng để liên kết các chương trình của **một** ảnh.
pub enum Link<'a> {
    /// Một tệp `.ld` **có sẵn** trong cây nguồn (riscv64: `init/init.ld` — giữ `.stack` NOLOAD).
    Fixed(&'a std::path::Path),
    /// Sinh `$OUT_DIR/<out>.ld` cho **từng** chương trình từ một [`LinkerScript`] mẫu, ở VA
    /// [`Program::base`] (aarch64, x86_64: mỗi ảnh một VA tuyệt đối riêng).
    Generated(LinkerScript<'a>),
}

/// Đặc tả một lượt dựng artefact U-mode của một ảnh.
pub struct Build<'a> {
    /// Target triple của lượt build (`riscv64gc-unknown-none-elf`, …).
    pub target: &'a str,
    /// Thư mục làm việc của `$RUSTC` (giữ nguyên như trước `KT-404c`: riscv64 dùng thư mục crate
    /// ảnh, aarch64/x86_64 dùng `$OUT_DIR`).
    pub cwd: &'a std::path::Path,
    /// Thư mục chứa nguồn các chương trình (`init/` của ảnh, hoặc `v2/user/programs/`).
    pub src_dir: &'a std::path::Path,
    pub link: Link<'a>,
    /// Cờ rustc phụ **chung** cho mọi chương trình của lượt này (`--cfg mut_info_x1`, …).
    pub extra: &'a [&'a str],
    /// Cờ `-C link-arg=…` riêng của arch (`--no-relax`, `-zmax-page-size=0x1000`, …).
    pub link_args: &'a [&'a str],
    /// Các nhóm `--extern` (mỗi nhóm là `(tên crate, đường dẫn rlib)`), trỏ tới bởi
    /// [`Program::set`].
    pub sets: &'a [&'a [(&'a str, &'a std::path::Path)]],
    pub programs: &'a [Program<'a>],
}

/// Dựng **mọi** chương trình trong `spec`, trả đường dẫn ELF theo đúng thứ tự `spec.programs`.
///
/// Với mỗi chương trình: sinh linker script (nếu [`Link::Generated`]), gọi `$RUSTC` qua
/// [`compile::compile_user_elf`], rồi khai báo `rerun-if-changed` cho tệp nguồn của nó.
pub fn build(spec: &Build<'_>) -> Vec<PathBuf> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let mut elfs = Vec::with_capacity(spec.programs.len());
    for program in spec.programs {
        let script = match &spec.link {
            Link::Fixed(path) => path.to_path_buf(),
            Link::Generated(spec) => {
                let path = out_dir.join(format!("{}.ld", program.out));
                compile::write_linker_script(&path, program.base, spec);
                path
            }
        };
        let source = spec.src_dir.join(program.src);
        // Cờ phụ của chương trình = cờ chung của lượt + cờ riêng của nó, **đúng thứ tự** này
        // (trước `KT-404c` mỗi nơi truyền một trong hai; cộng dồn cho cùng dòng lệnh `$RUSTC`).
        let mut extra: Vec<&str> = spec.extra.to_vec();
        extra.extend_from_slice(program.extra);
        elfs.push(compile::compile_user_elf(
            spec.target,
            spec.cwd,
            &source,
            program.out,
            &script,
            spec.sets[program.set],
            &extra,
            spec.link_args,
        ));
        println!("cargo:rerun-if-changed={}", source.display());
    }
    elfs
}

/// Một `rlib` cần dựng trước khi liên kết chương trình U-mode.
///
/// `externs` trỏ tới các dòng **đứng trước** nó trong cùng bảng [`rlibs`] (cùng thứ tự dựng) —
/// như vậy bảng vừa là dữ liệu vừa là thứ tự phụ thuộc, không cần tên biến trung gian.
pub struct Rlib<'a> {
    /// `--crate-name` (và tiền tố `lib<name>.rlib` trong `$OUT_DIR`).
    pub name: &'a str,
    /// Nguồn, tương đối với thư mục crate ảnh (`dir` của [`rlibs`]).
    pub src: &'a str,
    /// Các `--extern (tên, chỉ số dòng)` lấy từ bảng này.
    pub externs: &'a [(&'a str, usize)],
    /// Cờ rustc phụ (`--cfg rt_alloc`, `--check-cfg=…`).
    pub extra: &'a [&'a str],
}

impl<'a> Rlib<'a> {
    /// `rlib` không `--extern`, không cờ phụ.
    pub const fn new(name: &'a str, src: &'a str, externs: &'a [(&'a str, usize)]) -> Self {
        Self {
            name,
            src,
            externs,
            extra: &[],
        }
    }

    /// `rlib` có cờ rustc phụ.
    pub const fn with(mut self, extra: &'a [&'a str]) -> Self {
        self.extra = extra;
        self
    }
}

/// Dựng các `rlib` U-mode theo **một** bảng, trả đường dẫn theo đúng thứ tự bảng.
///
/// Đường dẫn nguồn là `dir.join(src)` — **không** `canonicalize` (A1-184/P20).
pub fn rlibs(dir: &std::path::Path, target: &str, specs: &[Rlib<'_>]) -> Vec<PathBuf> {
    let mut libs: Vec<PathBuf> = Vec::with_capacity(specs.len());
    for spec in specs {
        let externs: Vec<(&str, &std::path::Path)> = spec
            .externs
            .iter()
            .map(|(name, index)| (*name, libs[*index].as_path()))
            .collect();
        libs.push(compile::compile_rlib(
            target,
            spec.name,
            &dir.join(spec.src),
            &externs,
            spec.extra,
        ));
    }
    libs
}
