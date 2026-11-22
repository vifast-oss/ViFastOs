// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-343** — đối chứng host cho 5 thao tác FS của 11 lệnh `deferred-K7`
//! (`mkdir`/`rm`/`mv`/`ln`/`fsck`).
//!
//! Vì sao có tệp này bên cạnh QEMU: dịch vụ FS của ảnh sản phẩm (`proc_fs.rs`) chỉ **chuyển**
//! opcode xuống `vifast-fs`; ngữ nghĩa thật (và **mã lỗi**) nằm ở crate này. Ca host khoá đúng
//! hợp đồng mà cả 3 kiến trúc dùng chung — cùng đầu vào phải cho cùng kết quả/mã lỗi, không phụ
//! thuộc arch:
//!
//! - `mkdir` đã có ⇒ `Exists` (6); thiếu thư mục cha ⇒ `NotFound` (5);
//! - `mkdir` rồi `unlink` thư mục **rỗng** ⇒ Ok; thư mục **còn con** ⇒ `NotEmpty` (9);
//! - `link` tăng `nlink`, `unlink` giảm; nguồn thiếu ⇒ `NotFound`; nguồn là thư mục ⇒ `IsDir` (8);
//! - `rename` đổi tên + **thay thế** đích (ngữ nghĩa POSIX của FS); nguồn thiếu ⇒ `NotFound`;
//! - `fsck` sạch sau chuỗi thao tác.
//!
//! Không sửa `vifast-fs`; thiết bị là `Vec<u8>` trong bộ nhớ; không `unsafe`.

use std::vec;
use std::vec::Vec;

use vifast_fs::*;

/// Đĩa thô trong bộ nhớ (cùng khuôn `t26_read_your_own_write.rs`).
struct RamDisk {
    data: Vec<u8>,
}

impl RamDisk {
    fn new() -> Self {
        Self {
            data: vec![0u8; TOTAL_BLOCKS as usize * BLOCK_SIZE],
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
        TOTAL_BLOCKS
    }
}

fn fresh() -> Fs<RamDisk> {
    let mut dev = RamDisk::new();
    format(&mut dev).expect("format");
    Fs::mount(dev).expect("mount")
}

/// **Ca chính**: đúng chuỗi lệnh mà `product-riscv64.sh` bơm vào shell sản phẩm, nhưng ở mức
/// thư viện — mỗi bước khoá cả kết quả **và** mã lỗi.
#[test]
fn kt343_mkdir_rm_mv_ln_fsck_contract() {
    let mut fs = fresh();

    // mkdir /paradigm — tạo một lần Ok, lần hai Exists(6).
    fs.mkdir(b"/parity", 0).expect("mkdir /parity");
    assert_eq!(fs.mkdir(b"/parity", 0), Err(FsError::Exists));
    // Thiếu thư mục cha ⇒ NotFound(5), không tự tạo cây.
    assert_eq!(fs.mkdir(b"/khong/con", 0), Err(FsError::NotFound));

    // write /parity/a.txt = "hi" (2 byte) — FS thật, không phải bảng trong bộ nhớ.
    let ino_a = fs.create(b"/parity/a.txt", 0, INODE_FILE).expect("create");
    fs.write(ino_a, 0, b"hi", 0).expect("write");
    fs.fsync().expect("fsync");
    assert_eq!(fs.info(ino_a).expect("info").size, 2);

    // rename: a.txt -> b.txt; tên cũ mất, tên mới cùng inode + nội dung.
    fs.rename(b"/parity/a.txt", b"/parity/b.txt", 0)
        .expect("rename");
    assert_eq!(fs.lookup(b"/parity/a.txt", 0), Err(FsError::NotFound));
    let ino_b = fs.lookup(b"/parity/b.txt", 0).expect("lookup b");
    assert_eq!(ino_b, ino_a);
    let mut buf = [0u8; 8];
    assert_eq!(fs.read(ino_b, 0, &mut buf, 0).expect("read b"), 2);
    assert_eq!(&buf[..2], b"hi");

    // hard link: b.txt + c.txt cùng inode, nlink 1 -> 2; unlink c giảm còn 1 và b vẫn đọc được.
    fs.link(b"/parity/b.txt", b"/parity/c.txt", 0)
        .expect("link");
    assert_eq!(fs.lookup(b"/parity/c.txt", 0).expect("lookup c"), ino_b);
    assert_eq!(fs.info(ino_b).expect("info link").nlink, 2);
    fs.unlink(b"/parity/c.txt", 0).expect("unlink c");
    assert_eq!(fs.lookup(b"/parity/c.txt", 0), Err(FsError::NotFound));
    assert_eq!(fs.info(ino_b).expect("info after unlink").nlink, 1);
    assert_eq!(fs.read(ino_b, 0, &mut buf, 0).expect("read b2"), 2);

    // rm thư mục còn con ⇒ NotEmpty(9); rm tệp rồi rm thư mục rỗng ⇒ Ok; lần hai ⇒ NotFound(5).
    assert_eq!(fs.unlink(b"/parity", 0), Err(FsError::NotEmpty));
    assert!(fs.unlink(b"/parity/b.txt", 0).is_ok());
    assert!(fs.unlink(b"/parity", 0).is_ok());
    assert_eq!(fs.unlink(b"/parity", 0), Err(FsError::NotFound));

    // Nguồn thiếu của mv/ln ⇒ NotFound(5); ln thư mục ⇒ IsDir(8) (liên kết cứng chỉ cho tệp).
    assert_eq!(
        fs.rename(b"/khong", b"/parity/y", 0),
        Err(FsError::NotFound)
    );
    assert_eq!(fs.link(b"/khong", b"/parity/x", 0), Err(FsError::NotFound));
    fs.mkdir(b"/dir", 0).expect("mkdir /dir");
    assert_eq!(fs.link(b"/dir", b"/dir2", 0), Err(FsError::IsDir));

    // fsck sạch sau mọi thao tác trên (đúng thứ tự shell sản phẩm: fsck cuối).
    fs.fsck().expect("fsck clean");
}

/// **Ca thay thế đích**: `rename` lên tệp đã có thay nội dung (POSIX), không trả `Exists` —
/// khác `link` (đích đã có ⇒ `Exists`). Hai ngữ nghĩa này dễ lẫn; ca này ghim chúng.
#[test]
fn kt343_rename_replaces_but_link_refuses_existing() {
    let mut fs = fresh();
    let a = fs.create(b"/a", 0, INODE_FILE).expect("create a");
    fs.write(a, 0, b"aaa", 0).expect("write a");
    let b = fs.create(b"/b", 0, INODE_FILE).expect("create b");
    fs.write(b, 0, b"bb", 0).expect("write b");

    fs.rename(b"/a", b"/b", 0).expect("rename replaces");
    let ino = fs.lookup(b"/b", 0).expect("lookup b");
    assert_eq!(ino, a, "rename thay thế đích: /b phải là inode của /a");
    assert_eq!(fs.lookup(b"/a", 0), Err(FsError::NotFound));
    assert_eq!(fs.info(ino).expect("info").size, 3);

    let c = fs.create(b"/c", 0, INODE_FILE).expect("create c");
    assert_eq!(fs.link(b"/c", b"/b", 0), Err(FsError::Exists));
    assert_eq!(fs.info(c).expect("info c").nlink, 1);
}
