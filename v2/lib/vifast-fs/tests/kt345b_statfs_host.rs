// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-345b** — đối chứng host cho `df` (`OP_STATFS`, `fsstat.vidl`).
//!
//! `proc_fs::FsSvc::statfs` chỉ **chuyển** `Fs::usage()` thành reply `[ST_OK, used, total,
//! block_size]`; ngữ nghĩa số block nằm ở `vifast-fs`. Tệp này khoá đúng hợp đồng mà **cả 3**
//! kiến trúc dùng chung, bằng **cùng** phép đo với ảnh sản phẩm:
//!
//! - `usage()` chỉ tính **vùng DỮ LIỆU** (`capacity − DATA_START`), không tính metadata;
//! - volume **RAM 96 block** (đúng `proc_fs::RAM_VOL_BLOCKS`, đường `--mode serial` không `-drive`)
//!   ⇒ `total = 66` block;
//! - seed `/README` + `/docs` + `/docs/note.txt` + `/docs/sub` + `/docs/sub/deep.txt` (đúng
//!   `proc_fs::seed`) ⇒ `used = 6` block (3 thư mục + 3 tệp) ⇒ `df` in `6/66`.
//!
//! Đột biến `mut-df-round-up` (làm tròn **lên** tổng block) làm số `total` khác 66 ⇒ parity đỏ;
//! ca này khoá con số nên không thể "sửa kỳ vọng cho xanh".

use std::vec;
use std::vec::Vec;

use vifast_fs::*;

/// Đĩa thô trong bộ nhớ (cùng khuôn `kt343_commands_host.rs`).
struct RamDisk {
    data: Vec<u8>,
}

impl RamDisk {
    fn new(blocks: u32) -> Self {
        Self {
            data: vec![0u8; blocks as usize * BLOCK_SIZE],
        }
    }
}

impl BlockDev for RamDisk {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        let start = lba as usize * BLOCK_SIZE;
        out.copy_from_slice(&self.data[start..start + BLOCK_SIZE]);
        Ok(())
    }
    fn write(&mut self, lba: u32, input: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        let start = lba as usize * BLOCK_SIZE;
        self.data[start..start + BLOCK_SIZE].copy_from_slice(input);
        Ok(())
    }
    fn block_count(&self) -> u32 {
        (self.data.len() / BLOCK_SIZE) as u32
    }
}

/// **Ca chính** — volume 96 block như `proc_fs::RAM_VOL_BLOCKS`; `usage()` = số block vùng dữ
/// liệu. Không seed gì ⇒ `(0, 66)`; sau đúng chuỗi seed của ảnh sản phẩm ⇒ `(6, 66)`.
#[test]
fn kt345b_statfs_la_vung_du_lieu_va_seed_sau_bang_6_66() {
    let blocks = 96u32;
    let mut dev = RamDisk::new(blocks);
    format_with_options(
        &mut dev,
        FormatOptions {
            total_blocks: blocks,
            ..FormatOptions::default()
        },
    )
    .expect("format");
    let mut fs = Fs::mount(dev).expect("mount");
    assert_eq!(fs.usage(), (0, 66), "vùng dữ liệu = 96 - DATA_START(30)");
    // Đúng thứ tự + nội dung `proc_fs::seed` (thư mục trước, tệp sau).
    let readme = fs.create(b"/README", 0, INODE_FILE).expect("create README");
    fs.write(readme, 0, b"vifast", 0).expect("write README");
    fs.create(b"/docs", 0, INODE_DIR).expect("mkdir docs");
    let note = fs
        .create(b"/docs/note.txt", 0, INODE_FILE)
        .expect("create note");
    fs.write(note, 0, b"ghi chu", 0).expect("write note");
    fs.create(b"/docs/sub", 0, INODE_DIR).expect("mkdir sub");
    let deep = fs
        .create(b"/docs/sub/deep.txt", 0, INODE_FILE)
        .expect("create deep");
    fs.write(deep, 0, b"deep", 0).expect("write deep");
    assert_eq!(
        fs.usage(),
        (6, 66),
        "3 thư mục + 3 tệp = 6 block; tổng vẫn 66 ⇒ df in 6/66"
    );
    // Block size là hằng của FS, không phải số đo tự do.
    assert_eq!(BLOCK_SIZE, 512);
}
