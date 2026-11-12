// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-T26 — đèn host cho read-your-own-write của `vifast-fs`, theo cấu hình tái hiện **256 byte/lần**
//! (`MAX_IO` của v1 lúc phát hiện T26; xem `v2/docs/kt-t26-analysis.md`).
//!
//! Dùng thiết bị **đang ship** `CachedDev<D>` (giữ bất biến write-ahead: block dữ liệu xuống trước
//! commit record 29) thay vì cache tự chế, và đèn chẩn đoán `lost_writes()` mà crate đã có cho T26.
//!
//! Ba lớp đo:
//! 1. `ryow_1000_seeds` — ghi `3 × 256` byte, có **kẻ thứ ba ghi xen vào** (writer khác + xoá cache),
//!    đọc lại trong cùng mount phải khớp; `lost_writes()` phải = 0.
//! 2. `ryow_across_remount_1000_seeds` — `fsync` rồi mount lại từ **ảnh RAW** (không cache) phải khớp.
//! 3. `harness_catches_silent_write_loss` (**ca âm**) — thiết bị **mất ghi im lặng** (write trả `Ok`)
//!    phải làm phép đo **đỏ**; nếu không, đèn không có tác dụng.

use std::vec;
use std::vec::Vec;

use vifast_fs::*;

/// Cấu hình tái hiện T26: 256 byte mỗi lần vào/ra (D-04 giữ nguyên, không nới).
const MAX_IO: usize = 256;
const SEEDS: u64 = 1_000;
/// Tự bảo vệ: đổi cấu hình tái hiện là đổi phép đo.
const _: () = assert!(MAX_IO == 256);

/// Đĩa thô trong bộ nhớ (tầng dưới của `CachedDev`).
struct RawDisk {
    data: Vec<u8>,
}

impl RawDisk {
    fn new() -> Self {
        Self {
            data: vec![0u8; TOTAL_BLOCKS as usize * BLOCK_SIZE],
        }
    }

    fn from_raw(data: Vec<u8>) -> Self {
        Self { data }
    }

    fn snapshot(&self) -> Vec<u8> {
        self.data.clone()
    }
}

impl BlockDev for RawDisk {
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

/// **Mất ghi im lặng**: `write` trả `Ok` nhưng không ghi — đúng kiểu lỗi T26 (không phải lỗi I/O).
/// Chỉ mất ở vùng dữ liệu (≥ `DATA_START`); metadata vẫn ghi để fs mount được.
struct SilentLossDisk {
    inner: RawDisk,
}

impl BlockDev for SilentLossDisk {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        self.inner.read(lba, out)
    }
    fn write(&mut self, lba: u32, input: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        if lba >= DATA_START {
            return Ok(()); // mất im lặng
        }
        self.inner.write(lba, input)
    }
    fn block_count(&self) -> u32 {
        self.inner.block_count()
    }
}

/// Bộ sinh tất định (không thêm crate — D2-21).
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(6364136223846793005).wrapping_add(1))
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
}

fn payload(seed: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(3 * MAX_IO);
    let mut rng = Lcg::new(seed ^ 0x7ee5);
    for _ in 0..(3 * MAX_IO) {
        out.push((rng.next() & 0xff) as u8);
    }
    out
}

/// Kẻ thứ ba **ghi** xen vào (D-03): writer khác ghi block lạ + xoá cache, tạo áp lực đuổi khối bẩn
/// đúng lúc fs vừa ghi.
fn interleave<D: BlockDev>(dev: &mut CachedDev<D>, rng: &mut Lcg) {
    for _ in 0..rng.below(3) {
        match rng.below(3) {
            0 => {
                // Writer khác ghi vào vùng dữ liệu xa (không phải file của ta).
                let lba = DATA_START + 200 + rng.below(64) as u32;
                let block = [0xa5u8; BLOCK_SIZE];
                let _ = dev.inner_mut().write(lba, &block);
            }
            1 => dev.clear(),
            _ => {
                let mut verify = [0u8; BLOCK_SIZE];
                let lba = DATA_START + 300 + rng.below(64) as u32;
                let _ = dev.inner_mut().read(lba, &mut verify);
            }
        }
    }
}

/// Một seed: tạo `/d/f`, ghi 3 khúc 256 byte, đọc lại (cùng mount). Trả `Err(mô tả)` nếu vi phạm.
fn ryow_seed<D: BlockDev>(seed: u64, mut dev: CachedDev<D>) -> Result<(), String> {
    let mut rng = Lcg::new(seed);
    format(&mut dev).map_err(|e| format!("format: {e:?}"))?;
    let mut fs = Fs::mount(dev).map_err(|e| format!("mount: {e:?}"))?;
    fs.mkdir(b"/d", 0).map_err(|e| format!("mkdir: {e:?}"))?;
    let ino = fs
        .create(b"/d/f", 0, INODE_FILE)
        .map_err(|e| format!("create: {e:?}"))?;

    let data = payload(seed);
    for (i, chunk) in data.chunks(MAX_IO).enumerate() {
        let n = fs
            .write(ino, i * MAX_IO, chunk, 0)
            .map_err(|e| format!("write[{i}]: {e:?}"))?;
        if n != chunk.len() {
            return Err(format!("write[{i}] ngắn: {n} != {}", chunk.len()));
        }
        // Kẻ thứ ba chen ngay **sau từng** write.
        interleave(fs.dev_mut(), &mut rng);

        // Đọc lại ngay (qua cache) — T26: phải thấy đúng cái mình vừa ghi.
        let mut out = vec![0u8; data.len()];
        let n = fs
            .read(ino, 0, &mut out, 0)
            .map_err(|e| format!("read[{i}]: {e:?}"))?;
        if out[..n] != data[..n] {
            return Err(format!(
                "read-your-own-write sai sau write[{i}] (seed {seed}) — kẻ chen giữa vừa tác động"
            ));
        }
    }

    if fs.dev().lost_writes() != 0 {
        return Err(format!(
            "CachedDev.lost_writes() = {} (T26: ghi xong đọc lại khác)",
            fs.dev().lost_writes()
        ));
    }
    Ok(())
}

#[test]
fn ryow_1000_seeds() {
    for seed in 0..SEEDS {
        if let Err(msg) = ryow_seed(seed, CachedDev::new(RawDisk::new())) {
            panic!("seed {seed}: {msg}");
        }
    }
}

#[test]
fn ryow_across_remount_1000_seeds() {
    for seed in 0..SEEDS {
        let mut rng = Lcg::new(seed ^ 0xbeef);
        let mut dev = CachedDev::new(RawDisk::new());
        format(&mut dev).expect("format");
        let mut fs = Fs::mount(dev).expect("mount");
        fs.mkdir(b"/d", 0).expect("mkdir");
        let ino = fs.create(b"/d/f", 0, INODE_FILE).expect("create");

        let data = payload(seed);
        for (i, chunk) in data.chunks(MAX_IO).enumerate() {
            fs.write(ino, i * MAX_IO, chunk, 0).expect("write");
            interleave(fs.dev_mut(), &mut rng);
        }

        // RAW: flush xong lấy ảnh đĩa, mount lại không cache.
        fs.fsync().expect("fsync");
        let raw = fs.dev().inner().snapshot();
        let mut fs2 = Fs::mount(CachedDev::new(RawDisk::from_raw(raw))).expect("mount lại từ RAW");
        let ino2 = fs2.lookup(b"/d/f", 0).expect("lookup sau mount lại");
        let mut out = vec![0u8; data.len()];
        let n = fs2.read(ino2, 0, &mut out, 0).expect("read sau mount lại");
        assert_eq!(n, data.len(), "seed {seed}: read sau mount lại ngắn");
        assert_eq!(out, data, "seed {seed}: RAW read khác write");
    }
}

/// **Ca âm**: thiết bị mất ghi im lặng ⇒ phép đo **phải** báo lỗi. Nếu test này đỏ thì đèn hỏng.
#[test]
fn harness_catches_silent_write_loss() {
    let dev = CachedDev::new(SilentLossDisk {
        inner: RawDisk::new(),
    });
    let res = ryow_seed(0, dev);
    assert!(
        res.is_err(),
        "mất ghi im lặng mà phép đo vẫn xanh ⇒ đèn T26 vô dụng"
    );
    let msg = res.unwrap_err();
    assert!(
        msg.contains("read-your-own-write") || msg.contains("lost_writes"),
        "đèn báo sai kênh: {msg}"
    );
}
