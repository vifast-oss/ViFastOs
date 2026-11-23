// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-77c** — **FS + virtio-blk trong ảnh SẢN PHẨM** (D2-123.2): dịch vụ FS phía kernel của
//! runtime shell (`proc_rt`), để `ls`/`cat`/ghi tệp/đọc lại sau reboot chạy trên **ảnh sản phẩm**
//! — không chỉ trên ảnh test (`ktest_fs`/`ktest_vblk`/`ktest_persist`, KT-71/72/75).
//!
//! # Vì sao **không** kéo harness (`ktest`) vào ảnh sản phẩm
//!
//! Trước KT-77c feature `vblk` của crate arch khai `vblk = ["ktest"]` — **cùng dạng bẫy** KT-77b
//! đã sửa cho U-mode (`user = ["ktest"]`): bật virtio-blk cho ảnh sản phẩm là kéo `probe`/TAP
//! vào (D2-08, OD-K6-2 tiêu chí 6). Kiểm bằng mã: **không** tệp nào của driver
//! (`virtio_blk.rs`, `pci.rs`, `lib/vifast-virtio`) có `cfg(feature = "ktest")` ⇒ phụ thuộc
//! **chỉ là nhãn feature**, không phải mã test. Cách sửa giống `umode` của KT-77b:
//! `block = []` (kernel) + `vblk = ["block", "ktest"]` (ảnh test).
//!
//! # Hai phương tiện, một giao diện
//!
//! - **virtio-blk** (QEMU `-drive` + `-device virtio-blk-pci`): ổ **bền** — `Fs::fsync` gửi
//!   `T_FLUSH` xuống phương tiện, boot lại trên **cùng tệp đĩa** đọc được đúng byte đã ghi.
//! - **volume RAM** (PA [`RAM_VOL_PA`] qua physmap, đọc/ghi bằng wrapper **an toàn**
//!   `mmu::read_u64`/`write_u64`): ổ **bay hơi** khi QEMU không có đĩa — ảnh sản phẩm vẫn có FS
//!   dùng được và **in rõ** `volatile=1`. Không nhánh nào treo: lỗi PCI/thiết bị/format/mount
//!   đều thành **một dòng log** + reply lỗi cho shell.
//!
//! # `Fs` sống trong `.bss`, không trên stack
//!
//! `Fs` ≈ 13 KiB. Trả nó qua nhiều tầng theo giá trị làm **tràn guard page 64 KiB** (đo được:
//! `JOURNAL/kt77c-logs/kt77c-crash-stack.log`; KT-75 đã gặp đúng lỗi này và ghi ở
//! `ktest_persist.rs:915`). Vì vậy `Fs` nằm trong một ô `static` của crate arch
//! ([`vifast_plat_riscv64::static_slot::StaticSlot`], an toàn, `unsafe` nằm trong crate D2-17) và
//! module này chỉ chuyển **tham chiếu** — khung stack lớn nhất còn lại là một bản `Fs` tạm của
//! `Fs::mount` (không tránh được: API của `vifast-fs` trả theo giá trị).
//!
//! # Giao thức (không mở ABI mới)
//!
//! Dùng đúng hai syscall đã có (`NB_SEND`/`NB_RECV`) trên endpoint slot
//! [`EP_FS`](crate::proc_abi::EP_FS) và endpoint reply của procsrv — **không** thêm method vào
//! `spec/abi.toml`. Một request = **một** reply 4 MR word:
//!
//! | request (w0) | nghĩa | reply |
//! |---|---|---|
//! | `OP_PATH \| len<<40` + w1..w3 | **nối** tên đường dẫn (≤ [`MAX_PATH`] B); `len=0` = bắt đầu mới | `ST_OK` |
//! | `OP_LS \| index<<8` | entry thứ `index` của thư mục đang nạp | `ST_OK \| len<<8 \| kind<<16`, w1..w3 = tên, w0 bit 24.. = 0 |
//! | `OP_CAT \| offset<<8` | tối đa 24 B của tệp tại `offset` | `ST_OK \| len<<8`, w1..w3 = dữ liệu |
//! | `OP_WRITE \| offset<<8 \| len<<40` + w1..w3 | ghi (tạo tệp nếu chưa có) | `ST_OK \| n<<8` |
//! | `OP_TRUNC \| size<<8` | cắt tệp về `size` (giữ `write` idempotent) | `ST_OK` |
//! | `OP_STAT` | kiểu + cỡ của đường dẫn | `ST_OK \| kind<<8`, w1 = cỡ, w2 = ino |
//! | `OP_SYNC` | `Fs::fsync` → `flush` xuống phương tiện | `ST_OK` |
//!
//! Byte của tên/dữ liệu đi trong MR (24 B/request) chứ **không** qua Frame chia sẻ: shell U-mode đã
//! có sẵn đường `NBSend`/`NBRecv`, và `ls`/`cat`/`write` của một shell tối thiểu chỉ cần đường nhỏ
//! này. Module **không có `unsafe`** (physmap đi qua wrapper của crate arch).

use core::fmt::Write;

use vifast_fs::{
    format_with_options, BlockDev, FormatOptions, Fs, FsError, BLOCK_SIZE, INODE_DIR, INODE_FILE,
};
use vifast_plat_common::PlatformWriter;
use vifast_plat_riscv64::static_slot::StaticSlot;
use vifast_plat_riscv64::{mmu, pci, virtio_blk, Riscv64Virt};
use vifast_virtio::blk::BlkError;

use crate::proc_abi::{
    OP_CAT, OP_FSCK, OP_LN, OP_LS, OP_MKDIR, OP_MV, OP_PATH, OP_PATH2, OP_RM, OP_STAT, OP_STATFS,
    OP_SYNC, OP_TRUNC, OP_WRITE, ST_END, ST_ERR, ST_OK,
};

/// Số byte tên/dữ liệu mang trong **một** reply (3 MR word).
pub const REPLY_BYTES: usize = 24;
/// Cỡ tối đa của đường dẫn kernel nhận (`OP_PATH` 24 B/lần, nối nhiều lần).
pub const MAX_PATH: usize = 48;
/// PA của **volume RAM** khi QEMU không có đĩa — ngoài mọi arena của boot (image `0x804e_8000`,
/// object layer `0x804f_8000`, U2 `0x8100_0000`, cửa sổ PA của object layer `0x8200_0000`); RAM
/// QEMU `virt` 128 MiB từ `0x8000_0000` nên `0x8300_0000` chắc chắn thuộc RAM và đã được physmap
/// phủ (`mmu::physmap_alias` trả `Some`).
pub const RAM_VOL_PA: u64 = 0x8300_0000;
/// Số block 512 B của volume RAM = 96 × 512 = **48 KiB** (`DATA_START` = 30 block metadata ⇒ 66
/// block dữ liệu). Đủ cho `/README` + kịch bản `ls`/`cat`/`write`; **không** nới `TOTAL_BLOCKS`
/// của FS và không đụng ngân sách `.data+.bss`.
pub const RAM_VOL_BLOCKS: u32 = 96;
/// Nội dung `/README` của volume mới format — ASCII, không số phiên bản (số đo thuộc tài liệu).
const README: &[u8] =
    b"ViFastOs v2 - FS tren anh san pham\nlenh: ls | cat <path> | write <path> <text>\n";
/// Nội dung `/docs/note.txt` của volume mới format — ASCII, **một** dòng, không số phiên bản.
/// **KT-158**: 16 B (không có `\n` cuối) để `cat` in đúng một dòng nội dung rồi dòng tổng kết.
const DOCS_NOTE: &[u8] = b"ghi chu cua docs";
/// Nội dung `/docs/sub/deep.txt` — **KT-183**: tệp ở **cấp sâu** (hai thư mục lồng nhau) để ca
/// `kt183-cd-sau` kiểm `cd /docs/sub` + `pwd` + `cat deep.txt` (đường **tương đối** trong thư mục
/// sâu) trên FS sản phẩm **nhiều cấp**. 4 B, không `\n` cuối (cùng quy ước `DOCS_NOTE`).
const DOCS_DEEP: &[u8] = b"deep";

/// `Fs` trên hai phương tiện sau một kiểu — kiểu **công khai** để `proc_rt` giữ tham chiếu.
pub type FsVol = Fs<Volume>;

/// Nguồn gốc volume, in ra log một lần lúc mở (bằng chứng "đĩa hay RAM").
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Medium {
    /// Không mở được (không có thiết bị **và** không dựng được volume RAM).
    None,
    /// virtio-blk qua `-drive` (bền qua reboot).
    Disk,
    /// Volume RAM ở [`RAM_VOL_PA`] (bay hơi khi QEMU thoát).
    Ram,
}

impl Medium {
    pub fn name(self) -> &'static str {
        match self {
            Medium::None => "none",
            Medium::Disk => "virtio-blk",
            Medium::Ram => "ram",
        }
    }
    pub fn volatile(self) -> u8 {
        u8::from(self == Medium::Ram)
    }
}

/// Thiết bị **virtio-blk** (MMIO/PCI ở crate arch, D2-17): block 512 B = một sector của QEMU.
struct DiskVol;

impl BlockDev for DiskVol {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        virtio_blk::read_sector(lba, out).map_err(map_err)
    }
    fn write(&mut self, lba: u32, data: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        virtio_blk::write_sector(lba, data).map_err(map_err)
    }
    fn block_count(&self) -> u32 {
        virtio_blk::capacity_sectors()
    }
    fn flush(&mut self) -> Result<(), FsError> {
        virtio_blk::flush().map_err(map_err)
    }
}

/// Volume **RAM** trong PA, đọc/ghi qua physmap bằng wrapper an toàn của crate arch
/// (`mmu::physmap_alias` + `read_u64`/`write_u64`) — module này **không** có `unsafe`.
struct RamVol {
    /// VA physmap của block 0.
    base: u64,
    blocks: u32,
}

impl RamVol {
    fn new(pa: u64, blocks: u32) -> Option<Self> {
        let base = mmu::physmap_alias(pa)?;
        // 8 B cuối của volume phải còn trong dải RAM mà physmap phủ.
        mmu::physmap_alias(pa + u64::from(blocks) * BLOCK_SIZE as u64 - 8)?;
        Some(Self { base, blocks })
    }
}

impl BlockDev for RamVol {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        if lba >= self.blocks {
            return Err(FsError::Invalid);
        }
        let base = self.base + u64::from(lba) * BLOCK_SIZE as u64;
        let mut index = 0usize;
        while index < BLOCK_SIZE / 8 {
            let word = mmu::read_u64(base + index as u64 * 8).to_le_bytes();
            out[index * 8..index * 8 + 8].copy_from_slice(&word);
            index += 1;
        }
        Ok(())
    }
    fn write(&mut self, lba: u32, data: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        if lba >= self.blocks {
            return Err(FsError::Invalid);
        }
        let base = self.base + u64::from(lba) * BLOCK_SIZE as u64;
        let mut index = 0usize;
        while index < BLOCK_SIZE / 8 {
            let word = u64::from_le_bytes(
                data[index * 8..index * 8 + 8]
                    .try_into()
                    .unwrap_or([0u8; 8]),
            );
            mmu::write_u64(base + index as u64 * 8, word);
            index += 1;
        }
        Ok(())
    }
    fn block_count(&self) -> u32 {
        self.blocks
    }
}

/// Hai phương tiện sau **một** kiểu `BlockDev` (`Fs<Volume>` thay vì hai `Fs<_>`).
pub enum Volume {
    Disk(DiskVol),
    Ram(RamVol),
}

impl BlockDev for Volume {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        match self {
            Volume::Disk(dev) => dev.read(lba, out),
            Volume::Ram(dev) => dev.read(lba, out),
        }
    }
    fn write(&mut self, lba: u32, data: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        match self {
            Volume::Disk(dev) => dev.write(lba, data),
            Volume::Ram(dev) => dev.write(lba, data),
        }
    }
    fn block_count(&self) -> u32 {
        match self {
            Volume::Disk(dev) => dev.block_count(),
            Volume::Ram(dev) => dev.block_count(),
        }
    }
    fn flush(&mut self) -> Result<(), FsError> {
        match self {
            Volume::Disk(dev) => dev.flush(),
            Volume::Ram(_) => Ok(()),
        }
    }
}

fn map_err(error: BlkError) -> FsError {
    if error == BlkError::OutOfRange {
        FsError::Invalid
    } else {
        FsError::Io
    }
}

/// Mã số của lỗi FS trả cho shell (số nhỏ, cố định — shell in `code=`, không cần bảng chữ).
fn err_code(error: FsError) -> u64 {
    match error {
        FsError::Io => 1,
        FsError::Invalid => 2,
        FsError::NoSpace => 3,
        FsError::NoInode => 4,
        FsError::NotFound => 5,
        FsError::Exists => 6,
        FsError::NotDir => 7,
        FsError::IsDir => 8,
        FsError::NotEmpty => 9,
        FsError::NoAccess => 10,
        FsError::NameTooLong => 11,
        FsError::BadFd => 12,
        FsError::TooManyWrites => 13,
        FsError::Corrupt => 14,
    }
}

/// Kết quả [`open`]: **tham chiếu** tới `Fs` trong ô static + trạng thái giao thức (nhỏ).
pub struct Open {
    pub fs: Option<&'static mut FsVol>,
    pub svc: FsSvc,
}

/// Trạng thái giao thức + số đo (evidence). **Không** chứa `Fs` (xem đầu tệp).
pub struct FsSvc {
    medium: Medium,
    /// Lý do phải dùng volume RAM (rỗng khi có đĩa) — in trong log, **không** đoán.
    why: &'static str,
    path: [u8; MAX_PATH],
    path_len: usize,
    /// **KT-343** — đường dẫn **thứ hai** cho `mv`/`ln` (nguồn ở `path`, đích ở `path2`).
    path2: [u8; MAX_PATH],
    path2_len: usize,
    pub ops_ls: u64,
    pub ops_cat: u64,
    pub ops_write: u64,
    pub ops_sync: u64,
    pub errors: u64,
    pub flushes: u64,
}

impl FsSvc {
    pub fn medium(&self) -> Medium {
        self.medium
    }

    /// In **một** dòng tổng kết (bằng chứng) trước khi `halt`.
    pub fn report(&self, platform: &Riscv64Virt, replies: u64) {
        let mut w = PlatformWriter(platform);
        let _ = writeln!(
            w,
            "# product fs: medium={} ls={} cat={} write={} sync={} errors={} replies={} dev_reads={} dev_writes={} dev_flushes={}",
            self.medium.name(),
            self.ops_ls,
            self.ops_cat,
            self.ops_write,
            self.ops_sync,
            self.errors,
            replies,
            virtio_blk::stats().reads,
            virtio_blk::stats().writes,
            self.flushes
        );
    }
}

/// Quyết định phương tiện **trước** khi mount (khung nhỏ — không `Fs` nào ở đây).
enum Prepared {
    /// Có thiết bị virtio-blk; `need_format` ⇔ superblock không hợp lệ.
    Disk { need_format: bool },
    /// Không có đĩa (hoặc lỗi): dùng volume RAM; `why` ghi lý do.
    Ram { why: &'static str },
    /// Không dựng được phương tiện nào.
    Nothing { why: &'static str },
}

/// Đọc superblock của đĩa để biết có phải format không (`VF04`/`VF03` ở block 1).
fn disk_needs_format() -> bool {
    let mut block = [0u8; BLOCK_SIZE];
    if virtio_blk::read_sector(1, &mut block).is_err() {
        return true;
    }
    &block[..4] != b"VF04" && &block[..4] != b"VF03"
}

fn prepare() -> Prepared {
    let report = match pci::init() {
        Ok(report) => report,
        Err(_) => {
            return Prepared::Ram {
                why: "khong-co-pci",
            }
        }
    };
    if let Err(reason) = virtio_blk::init(&report) {
        return Prepared::Ram { why: reason };
    }
    let sectors = virtio_blk::capacity_sectors();
    if sectors < vifast_fs::DATA_START + 2 {
        return Prepared::Ram { why: "dia-qua-nho" };
    }
    Prepared::Disk {
        need_format: disk_needs_format(),
    }
}

/// Mở FS vào **ô static** `slot` (một lần cho cả đời ảnh) rồi in dòng trạng thái.
///
/// Không panic/không treo ở mọi nhánh: không có thiết bị ⇒ volume RAM; không có RAM ⇒ `fs=None`
/// (shell in `fs khong san sang`) — ca âm 5(a)/(c) đều có đường ra.
pub fn open(platform: &Riscv64Virt, slot: &'static StaticSlot<FsVol>) -> Open {
    let prepared = prepare();
    let (medium, why, mounted): (Medium, &'static str, Option<&'static mut FsVol>) = match prepared
    {
        Prepared::Disk { need_format } => {
            let mut volume = Volume::Disk(DiskVol);
            if need_format {
                let _ = format_with_options(&mut volume, FormatOptions::default());
            }
            match slot.init_with(move || Fs::mount(volume).ok()) {
                Some(fs) => (Medium::Disk, "", Some(fs)),
                None => (Medium::None, "mount-loi", None),
            }
        }
        Prepared::Ram { why } => match RamVol::new(RAM_VOL_PA, RAM_VOL_BLOCKS) {
            Some(dev) => {
                let mut volume = Volume::Ram(dev);
                let options = FormatOptions {
                    total_blocks: RAM_VOL_BLOCKS,
                    ..FormatOptions::default()
                };
                if format_with_options(&mut volume, options).is_err() {
                    (Medium::None, "ram-format-loi", None)
                } else {
                    match slot.init_with(move || Fs::mount(volume).ok()) {
                        Some(fs) => (Medium::Ram, why, Some(fs)),
                        None => (Medium::None, "ram-mount-loi", None),
                    }
                }
            }
            None => (Medium::None, "physmap-khong-phu", None),
        },
        Prepared::Nothing { why } => (Medium::None, why, None),
    };
    let mut mounted = mounted;
    // `/README` + `/docs/note.txt` bảo đảm có ở cả hai phương tiện (`create` trả `Exists` nếu đã
    // có — bỏ qua; **KT-158** thêm thư mục con `/docs`).
    if let Some(fs) = mounted.as_mut() {
        seed(fs);
    }
    let blocks = mounted
        .as_ref()
        .map(|fs| fs.dev().block_count())
        .unwrap_or(0);
    let (used, total) = mounted.as_ref().map(|fs| fs.usage()).unwrap_or((0, 0));
    let svc = FsSvc {
        medium,
        why,
        path: [0u8; MAX_PATH],
        path_len: 0,
        path2: [0u8; MAX_PATH],
        path2_len: 0,
        ops_ls: 0,
        ops_cat: 0,
        ops_write: 0,
        ops_sync: 0,
        errors: 0,
        flushes: 0,
    };
    {
        let mut w = PlatformWriter(platform);
        let _ = writeln!(
            w,
            "# product fs: medium={} blocks={} used={} total={} volatile={} why={} const:ram_pa=0x{:x} const:ram_blocks={} const:reply_bytes={} const:path_max={}",
            svc.medium.name(),
            blocks,
            used,
            total,
            svc.medium.volatile(),
            if svc.why.is_empty() { "ok" } else { svc.why },
            RAM_VOL_PA,
            RAM_VOL_BLOCKS,
            REPLY_BYTES,
            MAX_PATH
        );
    }
    Open { fs: mounted, svc }
}

/// Tạo `/README` + `/docs/note.txt` + `/docs/sub/deep.txt` trên volume **mới format** + `fsync`
/// (để `cat` có nội dung ở cả hai phương tiện). Lỗi ở đây **không** làm mất FS: chỉ bỏ qua (dòng
/// `used=` đã in).
///
/// **KT-158** (nợ KT-137, D2-138): trước đây chỉ seed `/README` ⇒ FS sản phẩm chỉ có **gốc** `/`,
/// nên D2-138 ghi hạn chế "`cd` vào **thư mục con** chưa kiểm được" và đột biến "`pwd` in hằng `/`"
/// **không phân biệt được**. Thêm `/docs` (thư mục) + `/docs/note.txt` (tệp) cho ca `kt137-cdpwd`
/// một thư mục con **thật**: `cd /docs` ⇒ `pwd` in `/docs`, `ls` trong `/docs` thấy `note.txt`,
/// `cat /docs/note.txt` đọc đúng nội dung — nhờ vậy đột biến `pwd` in hằng `/` **phải ĐỎ**.
///
/// **KT-183** (D2-164): `/docs` mới là **một** cấp; thêm `/docs/sub` (thư mục) + `/docs/sub/deep.txt`
/// (tệp) để FS sản phẩm có **nhiều cấp** thật — ca `kt183-cd-sau` kiểm `cd /docs/sub` ⇒ `pwd` in
/// `/docs/sub`, `cat deep.txt` (tương đối) đọc đúng, `cd ../..` ⇒ `/`, `cd /docs/sub/..` ⇒ `/docs`
/// (chuẩn hoá `..`), `cd ../../..` từ `/docs/sub` **kẹp** ở `/` (không âm). Trước KT-183 không có
/// thư mục sâu nên các vế "nhiều cấp"/"kẹp gốc" **không kiểm được**.
///
/// Không đổi `TOTAL_BLOCKS` của FS và không đụng ngân sách `.data+.bss`: năm inode (README, docs,
/// note.txt, sub, deep.txt) + bốn block dữ liệu nằm trong volume 96 block sẵn có
/// (`RAM_VOL_BLOCKS`).
fn seed(fs: &mut FsVol) {
    if let Ok(ino) = fs.create(b"/README", 0, INODE_FILE) {
        let _ = fs.write(ino, 0, README, 0);
        let _ = fs.fsync();
    }
    // **KT-158**: thư mục trước, tệp sau — `create` đòi thư mục cha đã có (`resolve_parent`).
    // `create` trả `Exists` khi đã có (đĩa đã seed lần trước) ⇒ bỏ qua, giữ nguyên dữ liệu người dùng.
    if fs.create(b"/docs", 0, INODE_DIR).is_ok() {
        let _ = fs.fsync();
    }
    if let Ok(ino) = fs.create(b"/docs/note.txt", 0, INODE_FILE) {
        let _ = fs.write(ino, 0, DOCS_NOTE, 0);
        let _ = fs.fsync();
    }
    // **KT-183**: cấp sâu — cùng thứ tự "thư mục trước, tệp sau"; `Exists` ⇒ bỏ qua (đĩa đã seed).
    if fs.create(b"/docs/sub", 0, INODE_DIR).is_ok() {
        let _ = fs.fsync();
    }
    if let Ok(ino) = fs.create(b"/docs/sub/deep.txt", 0, INODE_FILE) {
        let _ = fs.write(ino, 0, DOCS_DEEP, 0);
        let _ = fs.fsync();
    }
}

// ---------------------------------------------------------------------------------------------
// **KT-347 (U1a)** — API **phía kernel** đọc **cả** một tệp vào buffer (dùng cho `exec` đọc ELF
// từ FS). Đây **không** phải op mới của giao thức IPC: `OP_CAT` vẫn 24 B/lần cho shell; `exec`
// là kernel-mediated (D2-96) nên kernel đọc thẳng `Fs` — không cần vòng request/reply 4 word.
//
// Fail-closed: tệp lớn hơn buffer ⇒ `Invalid` (caller trả lỗi có mã, **không** cắt bớt im lặng);
// đường dẫn không có ⇒ `NotFound`; thư mục ⇒ `IsDir`.
pub fn read_file(fs: &mut FsVol, path: &[u8], buf: &mut [u8]) -> Result<usize, FsError> {
    let ino = fs.lookup(path, 0)?;
    let info = fs.info(ino)?;
    if info.kind == INODE_DIR {
        return Err(FsError::IsDir);
    }
    let size = info.size as usize;
    if size > buf.len() {
        return Err(FsError::Invalid);
    }
    if size == 0 {
        return Ok(0);
    }
    fs.read(ino, 0, &mut buf[..size], 0)
}

impl FsSvc {
    /// Một request của shell → **một** reply 4 word. Không bao giờ panic: mọi lỗi thành `ST_ERR`.
    pub fn request(&mut self, fs: &mut FsVol, words: [u64; 4]) -> [u64; 4] {
        let head = words[0];
        let op = head & 0xff;
        let arg = (head >> 8) & 0xffff_ffff;
        let len = ((head >> 40) & 0xff) as usize;
        match op {
            OP_PATH => self.load_path(&words, len),
            OP_LS => self.ls(fs, arg as usize),
            OP_CAT => self.cat(fs, arg as usize),
            OP_WRITE => self.write(fs, arg as usize, len, [words[1], words[2], words[3]]),
            OP_TRUNC => self.trunc(fs, arg as usize),
            OP_STAT => self.stat(fs),
            OP_SYNC => self.sync(fs),
            // **KT-343** — 5 thao tác FS của 11 lệnh `deferred-K7`; `OP_PATH2` nạp đích.
            OP_PATH2 => self.load_path2(&words, len),
            OP_MKDIR => self.mkdir(fs),
            OP_RM => self.rm(fs),
            OP_MV => self.mv(fs),
            OP_LN => self.ln(fs),
            OP_FSCK => self.fsck(fs),
            OP_STATFS => self.statfs(fs),
            _ => self.fail(FsError::Invalid),
        }
    }

    fn fail(&mut self, error: FsError) -> [u64; 4] {
        self.errors += 1;
        [ST_ERR | (err_code(error) << 8), 0, 0, 0]
    }

    /// Nối tối đa `len` B của tên từ 3 MR word vào buffer đường dẫn: `len == 0` = **bắt đầu mới**
    /// (shell gửi reset rồi các khối 24 B). Không cấp phát.
    fn load_path(&mut self, words: &[u64; 4], len: usize) -> [u64; 4] {
        if len == 0 {
            self.path_len = 0;
            return [ST_OK, 0, 0, 0];
        }
        let room = MAX_PATH.saturating_sub(self.path_len);
        // **KT-118** (review toàn dự án): `len` đến từ `head >> 40` của **request U-mode** (tối đa
        // 255) và trước đây chỉ kẹp theo `room` (48) ⇒ `len = 25` làm `words[1 + 24/8] = words[4]`
        // **tràn mảng** `[u64; 4]` ⇒ PANIC kernel (đã dựng lại: `PANIC: index out of bounds: the
        // len is 4 but the index is 4`). Nay kẹp thêm theo số byte **thật** chở trong 3 MR word.
        let len = core::cmp::min(len, core::cmp::min(room, REPLY_BYTES));
        let start = self.path_len;
        let mut index = 0usize;
        while index < len {
            let word = words[1 + index / 8];
            self.path[start + index] = (word >> ((index % 8) * 8)) as u8;
            index += 1;
        }
        self.path_len = start + len;
        [ST_OK, len as u64, 0, 0]
    }

    /// **Bản sao** đường dẫn đang nạp (phải tuyệt đối `/`); shell luôn gửi dạng `/x`.
    ///
    /// Sao chép ra `[u8; MAX_PATH]` (48 B trên stack) để không giữ borrow của `self` trong lúc
    /// mượn `*fs`.
    fn take_path(&self) -> Option<([u8; MAX_PATH], usize)> {
        if self.path_len == 0 || self.path[0] != b'/' {
            return None;
        }
        let mut copy = [0u8; MAX_PATH];
        copy[..self.path_len].copy_from_slice(&self.path[..self.path_len]);
        Some((copy, self.path_len))
    }

    /// **KT-343** — nạp đường dẫn **thứ hai** (đích của `mv`/`ln`); cùng bố cục 24 B/lần với
    /// [`Self::load_path`], nhưng ghi vào `path2`. `len == 0` = bắt đầu mới.
    fn load_path2(&mut self, words: &[u64; 4], len: usize) -> [u64; 4] {
        if len == 0 {
            self.path2_len = 0;
            return [ST_OK, 0, 0, 0];
        }
        let room = MAX_PATH.saturating_sub(self.path2_len);
        // Cùng chặn tràn như `load_path` (KT-118): `len` do U-mode gửi, tối đa 3 MR word.
        let len = core::cmp::min(len, core::cmp::min(room, REPLY_BYTES));
        let start = self.path2_len;
        let mut index = 0usize;
        while index < len {
            let word = words[1 + index / 8];
            self.path2[start + index] = (word >> ((index % 8) * 8)) as u8;
            index += 1;
        }
        self.path2_len = start + len;
        [ST_OK, len as u64, 0, 0]
    }

    /// **KT-343** — bản sao đường dẫn thứ hai (phải tuyệt đối `/`), cùng khuôn [`Self::take_path`].
    fn take_path2(&self) -> Option<([u8; MAX_PATH], usize)> {
        if self.path2_len == 0 || self.path2[0] != b'/' {
            return None;
        }
        let mut copy = [0u8; MAX_PATH];
        copy[..self.path2_len].copy_from_slice(&self.path2[..self.path2_len]);
        Some((copy, self.path2_len))
    }

    fn ls(&mut self, fs: &mut FsVol, index: usize) -> [u64; 4] {
        self.ops_ls += 1;
        let Some((path, path_len)) = self.take_path() else {
            return self.fail(FsError::Invalid);
        };
        let root = match fs.lookup(&path[..path_len], 0) {
            Ok(ino) => ino,
            Err(e) => return self.fail(e),
        };
        match fs.read_dir_entry_full(root, index) {
            Ok(None) => [ST_END, 0, 0, 0],
            Ok(Some((ino, name, len, kind))) => {
                // Ô TRỐNG trả `ino == 0` (shell bỏ qua, đi tiếp index).
                let name_len = core::cmp::min(len, REPLY_BYTES);
                // `ino` đi trong byte cao của **word 0** (≤ 64 ⇒ 6 bit) — nếu để `ino` ở `words[1]`
                // thì các byte tên OR vào đó sẽ hỏng tên (`p.txt` in thành `s.txt` khi ino = 3:
                // đo được ở boot KT-77c đầu tiên, xem `JOURNAL/kt77c-logs/kt77c-boot-disk-1.log`).
                let mut words = [
                    ST_OK
                        | ((name_len as u64) << 8)
                        | ((kind as u64) << 16)
                        | (u64::from(ino) << 24),
                    0,
                    0,
                    0,
                ];
                let mut i = 0usize;
                while i < name_len {
                    words[1 + i / 8] |= u64::from(name[i]) << ((i % 8) * 8);
                    i += 1;
                }
                words
            }
            Err(e) => self.fail(e),
        }
    }

    fn cat(&mut self, fs: &mut FsVol, offset: usize) -> [u64; 4] {
        self.ops_cat += 1;
        let Some((path, path_len)) = self.take_path() else {
            return self.fail(FsError::Invalid);
        };
        let ino = match fs.lookup(&path[..path_len], 0) {
            Ok(ino) => ino,
            Err(e) => return self.fail(e),
        };
        let info = match fs.info(ino) {
            Ok(info) => info,
            Err(e) => return self.fail(e),
        };
        if offset >= info.size as usize {
            return [ST_END, u64::from(info.size), 0, 0];
        }
        let mut buf = [0u8; REPLY_BYTES];
        let count = match fs.read(ino, offset, &mut buf, 0) {
            Ok(n) => n,
            Err(e) => return self.fail(e),
        };
        let mut words = [ST_OK | ((count as u64) << 8), 0, 0, 0];
        let mut i = 0usize;
        while i < count {
            words[1 + i / 8] |= u64::from(buf[i]) << ((i % 8) * 8);
            i += 1;
        }
        words
    }

    fn write(&mut self, fs: &mut FsVol, offset: usize, len: usize, data: [u64; 3]) -> [u64; 4] {
        self.ops_write += 1;
        let Some((path, path_len)) = self.take_path() else {
            return self.fail(FsError::Invalid);
        };
        let len = core::cmp::min(len, REPLY_BYTES);
        let ino = match fs.lookup(&path[..path_len], 0) {
            Ok(ino) => ino,
            Err(FsError::NotFound) => match fs.create(&path[..path_len], 0, INODE_FILE) {
                Ok(ino) => ino,
                Err(e) => return self.fail(e),
            },
            Err(e) => return self.fail(e),
        };
        let mut buf = [0u8; REPLY_BYTES];
        let mut i = 0usize;
        while i < len {
            buf[i] = (data[i / 8] >> ((i % 8) * 8)) as u8;
            i += 1;
        }
        match fs.write(ino, offset, &buf[..len], 0) {
            Ok(n) => [ST_OK | ((n as u64) << 8), 0, 0, 0],
            Err(e) => self.fail(e),
        }
    }

    fn trunc(&mut self, fs: &mut FsVol, size: usize) -> [u64; 4] {
        let Some((path, path_len)) = self.take_path() else {
            return self.fail(FsError::Invalid);
        };
        let ino = match fs.lookup(&path[..path_len], 0) {
            Ok(ino) => ino,
            Err(e) => return self.fail(e),
        };
        match fs.truncate(ino, size, 0) {
            Ok(()) => [ST_OK, 0, 0, 0],
            Err(e) => self.fail(e),
        }
    }

    fn stat(&mut self, fs: &mut FsVol) -> [u64; 4] {
        let Some((path, path_len)) = self.take_path() else {
            return self.fail(FsError::Invalid);
        };
        let ino = match fs.lookup(&path[..path_len], 0) {
            Ok(ino) => ino,
            Err(e) => return self.fail(e),
        };
        match fs.info(ino) {
            Ok(info) => [
                ST_OK | ((info.kind as u64) << 8),
                u64::from(info.size),
                u64::from(ino),
                0,
            ],
            Err(e) => self.fail(e),
        }
    }

    /// `fsync`: commit journal + **flush thiết bị** (ranh giới bền của KT-75, dùng lại nguyên).
    fn sync(&mut self, fs: &mut FsVol) -> [u64; 4] {
        self.ops_sync += 1;
        if let Err(e) = fs.fsync() {
            return self.fail(e);
        }
        self.flushes += 1;
        [ST_OK, 0, 0, 0]
    }

    // -----------------------------------------------------------------------------------------
    // **KT-343** — 5 thao tác FS còn thiếu (`mkdir`/`rm`/`mv`/`ln`/`fsck`). Mọi lỗi đi qua
    // [`Self::fail`] nên shell nhận **cùng** bảng `code=` như các thao tác cũ (một hợp đồng).
    // -----------------------------------------------------------------------------------------

    /// `mkdir <path>` — `Fs::mkdir`; ino không trả (shell chỉ cần thành công/thất bại).
    fn mkdir(&mut self, fs: &mut FsVol) -> [u64; 4] {
        let Some((path, path_len)) = self.take_path() else {
            return self.fail(FsError::Invalid);
        };
        match fs.mkdir(&path[..path_len], 0) {
            Ok(_) => [ST_OK, 0, 0, 0],
            Err(e) => self.fail(e),
        }
    }

    /// `rm <path>` — `Fs::unlink` (thư mục phải rỗng; FS trả `NotEmpty` nếu không).
    fn rm(&mut self, fs: &mut FsVol) -> [u64; 4] {
        let Some((path, path_len)) = self.take_path() else {
            return self.fail(FsError::Invalid);
        };
        match fs.unlink(&path[..path_len], 0) {
            Ok(()) => [ST_OK, 0, 0, 0],
            Err(e) => self.fail(e),
        }
    }

    /// `mv <src> <dst>` — `Fs::rename(src, dst)`; đích tồn tại thì **thay thế** (ngữ nghĩa FS).
    fn mv(&mut self, fs: &mut FsVol) -> [u64; 4] {
        let (Some((src, src_len)), Some((dst, dst_len))) = (self.take_path(), self.take_path2())
        else {
            return self.fail(FsError::Invalid);
        };
        match fs.rename(&src[..src_len], &dst[..dst_len], 0) {
            Ok(()) => [ST_OK, 0, 0, 0],
            Err(e) => self.fail(e),
        }
    }

    /// `ln <src> <dst>` — `Fs::link(src, dst)` (liên kết cứng; đích phải chưa có).
    fn ln(&mut self, fs: &mut FsVol) -> [u64; 4] {
        let (Some((src, src_len)), Some((dst, dst_len))) = (self.take_path(), self.take_path2())
        else {
            return self.fail(FsError::Invalid);
        };
        match fs.link(&src[..src_len], &dst[..dst_len], 0) {
            Ok(()) => [ST_OK, 0, 0, 0],
            Err(e) => self.fail(e),
        }
    }

    /// `fsck` — `Fs::fsck` kiểm toàn vẹn (superblock, inode, block, liên kết thư mục).
    fn fsck(&mut self, fs: &mut FsVol) -> [u64; 4] {
        match fs.fsck() {
            Ok(()) => [ST_OK, 0, 0, 0],
            Err(e) => self.fail(e),
        }
    }

    /// **KT-345b** — `OP_STATFS` (`fsstat.vidl`): `[ST_OK, used, total, block_size]`. `used`/`total`
    /// là số **block vùng dữ liệu** đúng như `Fs::usage()` (bỏ superblock/bitmap/inode/journal),
    /// nên `df` in được `used/total` khớp khả năng cấp phát thật. Đột biến `mut-df-round-up` làm
    /// tròn **lên** tổng số block ⇒ parity đỏ (ca đỏ-trước ở `spec/v1-parity.toml`).
    fn statfs(&mut self, fs: &mut FsVol) -> [u64; 4] {
        let (used, total) = fs.usage();
        #[cfg(feature = "mut-df-round-up")]
        let total = (total + 7) & !7;
        [ST_OK, u64::from(used), u64::from(total), BLOCK_SIZE as u64]
    }
}
