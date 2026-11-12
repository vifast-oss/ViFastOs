// SPDX-License-Identifier: MIT OR Apache-2.0
// PORTED-FROM: crates/vifast-fs/src/lib.rs@ca2eaf5
//! ViFastOs FS v3: a fixed-size, journaled filesystem for user-space VFS.
//!
//! Port từ v1 (D2-26): cùng API và cùng hành vi, chỉ **tách tệp** để thoả luật
//! cấu trúc v2 "tệp trong `lib/` không quá 1.500 dòng" (D2-06). Một tệp của v1
//! (4.543 dòng) được chia thành: `consts`, `fs` (types + block cache + crc +
//! encode + format + phần lõi của `Fs`), `tx` (journal/stage/commit + fsync),
//! `alloc` (bitmap/inode), `path` (tra đường dẫn), `dir` (thao tác thư mục/tệp)
//! và `tests` (3 tệp). Không đổi ngữ nghĩa, không đổi API công khai.
//!
//! The filesystem is deliberately small: no allocation, 64 inodes, and one
//! 16-block write-ahead journal.  New `VF04` images use 10 direct blocks plus
//! one single-indirect and one double-indirect block per file (up to 8.4 MiB
//! logically, bounded by the 4 MiB image).  Legacy `VF03` images still mount
//! with their original 11-direct + single-indirect geometry (69 KiB files).
//! A large write is split into per-block atomic transactions rather than
//! claiming whole-file atomicity.
#![no_std]
#![forbid(unsafe_code)]

mod alloc;
mod consts;
mod dir;
mod fs;
mod path;
mod tx;

#[cfg(test)]
mod tests;

pub use consts::*;
pub use fs::*;
pub use tx::DirEntryFull;
