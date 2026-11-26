// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **persist** riscv64 (KT-75, K6) — `test-persist` **7 pha** trên virtio-blk, mỗi pha
//! **ghi** dữ liệu rồi **thoát QEMU**; một tiến trình QEMU **mới** boot lại trên **cùng** ảnh đĩa
//! và **đọc lại kiểm từng byte** (nợ D2-111: “`flush` bền qua reboot”).
//!
//! # Kịch bản ở LBA 0
//!
//! Một pha gồm **hai** lần boot (ghi / đọc lại) ⇒ không vừa mô hình “mỗi ảnh một boot” của
//! `ktest.sh`; runner riêng `v2/test/boot/persist.sh` điều khiển **10 boot**. Mỗi boot nhận
//! **kịch bản** 32 B ở **LBA 0** — sector **dành riêng** của FS: `stage()` từ chối LBA 0,
//! `format` chỉ ghi 0 một lần, và `Fs::initialize` chỉ đọc LBA 0 khi superblock hỏng. Runner ghi
//! kịch bản bằng `dd` **trước** mỗi boot; kernel in lại **nguyên văn** những gì nó đọc (D2-102) ⇒
//! runner và kernel lệch nhau thì log sai ngay.
//!
//! # Bảy pha
//!
//! | Pha | Hành vi | Trạng thái sau pha |
//! |-----|---------|--------------------|
//! | 1 | tạo file mới + ghi | `/p1.txt` = payload pha 1 (512 B) |
//! | 2 | **ghi đè** (mở rộng) | `/p1.txt` = payload pha 2 (2048 B) |
//! | 3 | **append** tại EOF | `/p1.txt` = payload 2 + payload 3 (3072 B) |
//! | 4 | **truncate về 0** rồi ghi lại | `/p1.txt` = payload 4 (700 B) |
//! | 5 | **rename** | `/p5.bin` = payload 4; `/p1.txt` **vắng** |
//! | 6 | **xoá + tạo lại cùng tên** | `/p5.bin` = payload 6 (tên cũ sống lại); `/p1.txt` **vắng** |
//! | 7 | file lớn qua **double-indirect** | `/p7.bin` 144 block (73 728 B), block chỉ số 143 |
//!
//! Mỗi pha kết thúc bằng `Fs::fsync()` (commit + `flush` xuống thiết bị) rồi `halt`: QEMU thoát,
//! RAM mất. Pha đọc lại **chỉ** xanh khi từng byte trên **đĩa** khớp payload sinh lại trong boot
//! mới — không cache RAM nào giúp được (tiến trình khác, không gian địa chỉ khác). Nhật ký pha
//! `/pstate` (4 B/bản ghi) là **bằng chứng bền trên đĩa**: boot cuối tự đếm `phases=7 ok=7` từ
//! đĩa, không từ biến RAM.
//!
//! # Ca âm `flush`
//!
//! `PersistDev` là **cache ghi-bay-hơi mô phỏng** (RAM) giữa FS và virtio-blk: `write` chỉ vào
//! RAM, `flush` **đẩy** block bẩn xuống thiết bị rồi gửi `T_FLUSH` thật. QEMU ghi thẳng vào file
//! ảnh trên host nên **không** có cách nào để QEMU tự làm mất dữ liệu đã ghi; vì vậy “cắt điện”
//! được **đo** đúng bằng bất biến của cache: `pending_blocks > 0` lúc thoát ⇒ cache bay hơi ⇒
//! boot sau **không** thấy dữ liệu, và host kiểm thêm **sha256 ảnh đĩa không đổi**. Kịch bản
//! `mode=neg_write` bỏ **đúng một** hành vi: `flush()` **không** đẩy (đếm `flushes_ignored`),
//! mọi thứ khác giữ nguyên — đối chứng với 7 pha dương (cùng đường ghi, cùng thiết bị).
//!
//! # Cắt điện **tất định** giữa pha 7 (`KT-408d`, `Q7`)
//!
//! Kịch bản `mode=3` (`MODE_CRASH_MID_WRITE`) đọc lại pha 6 rồi ghi `/p7.bin` **từng sector
//! 512 B**; ngay trước transaction của sector thứ `crash_after` (byte `17..20`, `u32` LE) nó
//! **vũ trang** `PersistDev::crash_cut`, và `flush()` kế tiếp in
//! `# persist: crash_cut writes=N pending=P pushed=…` rồi `PLATFORM.halt(77)` **trước** `push_all`
//! ⇒ block bẩn trong [`CACHE_DATA`] **không** xuống đĩa. Vì FS commit trọn một sector mỗi
//! transaction, đĩa còn lại đúng **N sector bền** của `/p7.bin` + journal một transaction đã
//! commit-nhưng-chưa-dọn (boot sau replay được) — bất biến **không phụ thuộc thời gian/tải**, thay
//! cho poll `sleep 0.05` ở runner (nguồn flake `records=13`). Kịch bản cũ 32 B vẫn hợp lệ
//! (`crash_after = 0` ⇒ chỉ `mode=0..2`).
//!
//! # Cái KHÔNG có trong ảnh
//!
//! Không `unsafe` (MMIO ở `kernel/vifast-plat-riscv64`), không `#[ignore]`, không sửa
//! `vifast-fs`/`vifast-vfs`/`spec/`. SHA-256 tự cài trong tệp chỉ để **in số đo** (`sha_prefix=`);
//! host băm lại **độc lập** (`sha256sum`) trên byte đọc từ ảnh đĩa.

use core::fmt::Write;
use core::sync::atomic::{AtomicU8, Ordering};

use vifast_fs::{BlockDev, FormatOptions, Fs, FsError, BLOCK_SIZE, INODE_FILE};
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::pci;
use vifast_plat_riscv64::virtio_blk;
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};
use vifast_virtio::blk::BlkError;

static PLATFORM: Riscv64Virt = Riscv64Virt;

const CACHE_SLOTS: usize = 32;
const PLAN_LBA: u32 = 0;
const PLAN_MAGIC: &[u8; 4] = b"PL75";
const PLAN_VERSION: u8 = 1;
const FLAG_FORMAT: u8 = 1;
/// Chế độ của lần boot (byte 7 của kịch bản).
const MODE_NORMAL: u8 = 0;
const MODE_NEG_WRITE: u8 = 1;
const MODE_NEG_VERIFY: u8 = 2;
/// **KT-408d (Q7)** — cắt điện **tất định** giữa pha 7: halt **không flush** đúng sau
/// `crash_after` sector 512 B của `/p7.bin` đã xuống thiết bị (byte 17..20 = `u32` LE).
/// Tương thích ngược: kịch bản cũ 32 B có byte 17..20 = 0 ⇒ chỉ `mode=0..2` hợp lệ.
const MODE_CRASH_MID_WRITE: u8 = 3;
/// Offset `crash_after: u32` trong kịch bản 32 B (vùng dự phòng ngay sau `nonce: u64`).
const PLAN_CRASH_OFFSET: usize = 17;
const STATE_PATH: &[u8] = b"/pstate";
const NEG_PATH: &[u8] = b"/pneg.txt";
const PHASES: u8 = 7;
/// Cỡ payload pha 7: 144 block ⇒ block chỉ số 143 nằm **sau** 10 direct + 128 single-indirect
/// ⇒ mọi truy cập block đó **buộc** đi qua double-indirect (block 11 của inode).
const P7_BYTES: usize = 144 * BLOCK_SIZE;
/// Số sector 512 B của `/p7.bin` (đơn vị của `crash_after`).
const P7_BLOCKS: usize = P7_BYTES / BLOCK_SIZE;

// ------------------------------------------------------------------ payload sinh theo công thức
//
// D2-106: giá trị suy ra từ **công thức** in kèm đầu vào; cùng công thức được cài **độc lập** ở
// `v2/test/boot/persist_host.py` (host băm chéo).
const fn payload_byte(phase: u8, index: usize) -> u8 {
    ((phase as usize * 37 + index * 13 + (index / BLOCK_SIZE) * 101) & 0xff) as u8
}

const fn make_payload<const N: usize>(phase: u8) -> [u8; N] {
    let mut out = [0u8; N];
    let mut index = 0usize;
    while index < N {
        out[index] = payload_byte(phase, index);
        index += 1;
    }
    out
}

static P1: [u8; 512] = make_payload::<512>(1);
static P2: [u8; 2048] = make_payload::<2048>(2);
static P3: [u8; 1024] = make_payload::<1024>(3);
static P4: [u8; 700] = make_payload::<700>(4);
static P6: [u8; 1536] = make_payload::<1536>(6);
static P7: [u8; P7_BYTES] = make_payload::<P7_BYTES>(7);
static PNEG: [u8; 600] = make_payload::<600>(9);

// ------------------------------------------------------------------------------- SHA-256 (đo)
#[derive(Clone, Copy)]
struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    used: usize,
    len: u64,
}

const SHA_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Sha256 {
    const fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buf: [0u8; 64],
            used: 0,
            len: 0,
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for index in 0..16 {
            w[index] = u32::from_be_bytes([
                block[index * 4],
                block[index * 4 + 1],
                block[index * 4 + 2],
                block[index * 4 + 3],
            ]);
        }
        for index in 16..64 {
            let x = w[index - 15];
            let y = w[index - 2];
            let s0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            let s1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA_K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    fn update(&mut self, mut data: &[u8]) {
        self.len = self.len.wrapping_add(data.len() as u64);
        if self.used > 0 {
            let take = core::cmp::min(64 - self.used, data.len());
            self.buf[self.used..self.used + take].copy_from_slice(&data[..take]);
            self.used += take;
            data = &data[take..];
            if self.used == 64 {
                let block = self.buf;
                self.compress(&block);
                self.used = 0;
            }
        }
        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            self.compress(&block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.used = data.len();
        }
    }

    fn finish(mut self) -> [u8; 32] {
        let bits = self.len.wrapping_mul(8);
        let pad_len = if self.used < 56 {
            56 - self.used
        } else {
            120 - self.used
        };
        let mut pad = [0u8; 64];
        pad[0] = 0x80;
        self.update(&pad[..pad_len]);
        self.update(&bits.to_be_bytes());
        let mut out = [0u8; 32];
        for (index, word) in self.state.iter().enumerate() {
            out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

// --------------------------------------------------------------- thiết bị: cache bay hơi mô phỏng
//
// Cache nằm trong `.bss` dưới dạng `AtomicU8` (an toàn: không `unsafe`, `AtomicU8: Sync`), KHÔNG
// nằm trong struct — nếu nhét 16 KiB vào `PersistDev` thì `Fs::mount` sao chép `Fs` (13 KiB) kèm
// thiết bị 16 KiB và tràn stack 64 KiB (đã xảy ra thật: `PANIC: guard page fault` tại `memcpy`,
// `stval=0x80400000`).
const CACHE_BYTES: usize = CACHE_SLOTS * BLOCK_SIZE;
static CACHE_DATA: [AtomicU8; CACHE_BYTES] = [const { AtomicU8::new(0) }; CACHE_BYTES];

struct PersistDev {
    honor_flush: bool,
    count: usize,
    lbas: [u32; CACHE_SLOTS],
    /// Số block đã thật sự đẩy xuống virtio-blk.
    pushed: u32,
    /// Số lần `flush()` **được thi hành** (đẩy + `T_FLUSH`).
    flushes_done: u32,
    /// Số lần `flush()` **bị bỏ** (chỉ ở ca âm).
    flushes_ignored: u32,
    /// Số lần cache đầy (phải 0).
    overflow: u32,
    /// **KT-408d** — khi `Some(n)`: lần `flush()` **kế tiếp** là **cắt điện tất định**. In dấu
    /// `# persist: crash_cut writes=n pending=<pushed-dirty>` rồi `halt(77)` **trước** `push_all`
    /// ⇒ các block bẩn trong [`CACHE_DATA`] bay hơi đúng như mất điện, và trạng thái đĩa là
    /// **tiền tố xác định**: đúng `n` sector 512 B của `/p7.bin` đã bền.
    crash_cut: Option<u32>,
}

impl PersistDev {
    const fn new(honor_flush: bool) -> Self {
        Self {
            honor_flush,
            count: 0,
            lbas: [0; CACHE_SLOTS],
            pushed: 0,
            flushes_done: 0,
            flushes_ignored: 0,
            overflow: 0,
            crash_cut: None,
        }
    }

    /// **KT-408d** — vũ trang điểm cắt: `flush()` kế tiếp halt **không flush** (xem
    /// [`PersistDev::crash_cut`]). Chỉ `ktest_persist` gọi, đúng một lần, ngay trước transaction
    /// của sector `n` (0-based) để `n` sector đã bền là bất biến của phép đo.
    fn arm_crash_cut(&mut self, writes: u32) {
        self.crash_cut = Some(writes);
    }

    fn pending_blocks(&self) -> usize {
        self.count
    }

    fn slot_of(&self, lba: u32) -> Option<usize> {
        let mut index = 0;
        while index < self.count {
            if self.lbas[index] == lba {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn read_slot(&self, slot: usize, out: &mut [u8; BLOCK_SIZE]) {
        let base = slot * BLOCK_SIZE;
        for index in 0..BLOCK_SIZE {
            out[index] = CACHE_DATA[base + index].load(Ordering::Relaxed);
        }
    }

    fn write_slot(&mut self, slot: usize, data: &[u8; BLOCK_SIZE]) {
        let base = slot * BLOCK_SIZE;
        for index in 0..BLOCK_SIZE {
            CACHE_DATA[base + index].store(data[index], Ordering::Relaxed);
        }
    }

    fn push_all(&mut self) -> Result<(), FsError> {
        let mut index = 0;
        let mut block = [0u8; BLOCK_SIZE];
        while index < self.count {
            self.read_slot(index, &mut block);
            virtio_blk::write_sector(self.lbas[index], &block).map_err(map_err)?;
            self.pushed += 1;
            index += 1;
        }
        self.count = 0;
        Ok(())
    }
}

fn map_err(error: BlkError) -> FsError {
    if error == BlkError::OutOfRange {
        FsError::Invalid
    } else {
        FsError::Io
    }
}

impl BlockDev for PersistDev {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        if let Some(slot) = self.slot_of(lba) {
            self.read_slot(slot, out);
            return Ok(());
        }
        virtio_blk::read_sector(lba, out).map_err(map_err)
    }

    fn write(&mut self, lba: u32, data: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        if let Some(slot) = self.slot_of(lba) {
            self.write_slot(slot, data);
            return Ok(());
        }
        if self.count == CACHE_SLOTS {
            self.overflow += 1;
            return Err(FsError::Io);
        }
        self.lbas[self.count] = lba;
        self.write_slot(self.count, data);
        self.count += 1;
        Ok(())
    }

    fn block_count(&self) -> u32 {
        virtio_blk::capacity_sectors()
    }

    fn flush(&mut self) -> Result<(), FsError> {
        if !self.honor_flush {
            self.flushes_ignored += 1;
            return Ok(());
        }
        // **KT-408d (Q7)** — cắt điện TẤT ĐỊNH: halt **trước** `push_all` ⇒ mọi block bẩn
        // (journal + target của sector đang ghi dở) nằm lại trong `CACHE_DATA` RAM và bay hơi;
        // đĩa giữ đúng trạng thái đã flush của transaction trước. Không phụ thuộc thời gian,
        // không phụ thuộc tải máy: cùng `crash_after` ⇒ cùng byte trên đĩa.
        if let Some(writes) = self.crash_cut {
            let pending = self.count;
            let pushed = self.pushed;
            {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    w,
                    "# persist: crash_cut mode={MODE_CRASH_MID_WRITE} writes={writes} \
                     pending={pending} pushed={pushed} cache_slots={CACHE_SLOTS}"
                );
            }
            PLATFORM.halt(77)
        }
        self.push_all()?;
        self.flushes_done += 1;
        virtio_blk::flush().map_err(map_err)
    }
}

// ------------------------------------------------------------------------------- kịch bản
#[derive(Clone, Copy)]
struct Plan {
    verify: u8,
    write: u8,
    flags: u8,
    mode: u8,
    nonce: u64,
    /// **KT-408d** — chỉ dùng khi `mode == MODE_CRASH_MID_WRITE`: số sector 512 B của
    /// `/p7.bin` phải **bền** trước khi halt (byte 17..20, `u32` LE; kịch bản cũ = 0).
    crash_after: u32,
}

impl Plan {
    fn format(&self) -> bool {
        self.flags & FLAG_FORMAT != 0
    }
    fn honor_flush(&self) -> bool {
        self.mode != MODE_NEG_WRITE
    }
    /// **KT-408d** — điểm cắt điện tất định (`None` ở mọi chế độ khác).
    fn crash_cut(&self) -> Option<u32> {
        if self.mode == MODE_CRASH_MID_WRITE {
            Some(self.crash_after)
        } else {
            None
        }
    }
}

fn parse_plan(block: &[u8; BLOCK_SIZE]) -> Option<Plan> {
    if &block[..4] != PLAN_MAGIC || block[4] != PLAN_VERSION {
        return None;
    }
    let plan = Plan {
        verify: block[5],
        write: block[6],
        flags: block[7],
        mode: block[8],
        nonce: u64::from_le_bytes([
            block[9], block[10], block[11], block[12], block[13], block[14], block[15], block[16],
        ]),
        crash_after: u32::from_le_bytes([
            block[PLAN_CRASH_OFFSET],
            block[PLAN_CRASH_OFFSET + 1],
            block[PLAN_CRASH_OFFSET + 2],
            block[PLAN_CRASH_OFFSET + 3],
        ]),
    };
    if plan.verify > PHASES || plan.write > PHASES || plan.mode > MODE_CRASH_MID_WRITE {
        return None;
    }
    let has_action = plan.verify > 0 || plan.write > 0 || plan.mode != MODE_NORMAL;
    if !has_action {
        return None;
    }
    if plan.mode == MODE_CRASH_MID_WRITE {
        // Cắt điện tất định **chỉ** có nghĩa khi: đọc lại pha 6 rồi ghi pha 7, và điểm cắt nằm
        // giữa 1..(P7-1) sector (cắt sau sector 0 = chưa có tệp; cắt ở cuối = không còn gì để mất).
        if plan.verify != PHASES - 1 || plan.write != PHASES {
            return None;
        }
        if plan.crash_after == 0 || plan.crash_after as usize >= P7_BLOCKS {
            return None;
        }
        return Some(plan);
    }
    // Tương thích ngược: kịch bản `mode=0..2` cũ có 4 byte dự phòng = 0; byte khác 0 ⇒ hỏng.
    if plan.crash_after != 0 {
        return None;
    }
    if plan.mode != MODE_NORMAL && (plan.verify > 0 || plan.write > 0) {
        return None;
    }
    Some(plan)
}

// ------------------------------------------------------------------------- mô hình kỳ vọng
struct Seg {
    path: &'static [u8],
    offset: usize,
    data: &'static [u8],
}

static SEG_P1: [Seg; 1] = [Seg {
    path: b"/p1.txt",
    offset: 0,
    data: &P1,
}];
static SEG_P2: [Seg; 1] = [Seg {
    path: b"/p1.txt",
    offset: 0,
    data: &P2,
}];
static SEG_P3: [Seg; 2] = [
    Seg {
        path: b"/p1.txt",
        offset: 0,
        data: &P2,
    },
    Seg {
        path: b"/p1.txt",
        offset: 2048,
        data: &P3,
    },
];
static SEG_P4: [Seg; 1] = [Seg {
    path: b"/p1.txt",
    offset: 0,
    data: &P4,
}];
static SEG_P5: [Seg; 1] = [Seg {
    path: b"/p5.bin",
    offset: 0,
    data: &P4,
}];
static SEG_P6: [Seg; 1] = [Seg {
    path: b"/p5.bin",
    offset: 0,
    data: &P6,
}];
static SEG_P7: [Seg; 2] = [
    Seg {
        path: b"/p5.bin",
        offset: 0,
        data: &P6,
    },
    Seg {
        path: b"/p7.bin",
        offset: 0,
        data: &P7,
    },
];

fn segments(phase: u8) -> &'static [Seg] {
    match phase {
        1 => &SEG_P1,
        2 => &SEG_P2,
        3 => &SEG_P3,
        4 => &SEG_P4,
        5 => &SEG_P5,
        6 => &SEG_P6,
        _ => &SEG_P7,
    }
}

static ABSENT_5: [&[u8]; 1] = [b"/p1.txt"];
static ABSENT_NONE: [&[u8]; 0] = [];

fn absent_paths(phase: u8) -> &'static [&'static [u8]] {
    match phase {
        5..=7 => &ABSENT_5,
        _ => &ABSENT_NONE,
    }
}

fn expected_size(phase: u8, path: &[u8]) -> u32 {
    let mut size = 0usize;
    for seg in segments(phase) {
        if seg.path == path {
            size = size.max(seg.offset + seg.data.len());
        }
    }
    size as u32
}

// ------------------------------------------------------------------------------- tiện ích in
fn tap(number: usize, ok: bool, kind: &str, phase: Option<u8>, fails: &mut usize, why: &str) {
    let mut w = PlatformWriter(&PLATFORM);
    match (ok, phase) {
        (true, Some(phase)) => {
            let _ = writeln!(w, "ok {number} - {kind} {phase}");
        }
        (true, None) => {
            let _ = writeln!(w, "ok {number} - {kind}");
        }
        (false, Some(phase)) => {
            let _ = writeln!(w, "not ok {number} - {kind} {phase}: {why}");
            *fails += 1;
        }
        (false, None) => {
            let _ = writeln!(w, "not ok {number} - {kind}: {why}");
            *fails += 1;
        }
    }
}

fn sha_prefix(digest: &[u8; 32]) -> [u8; 8] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 8];
    for index in 0..4 {
        out[index * 2] = HEX[(digest[index] >> 4) as usize];
        out[index * 2 + 1] = HEX[(digest[index] & 0x0f) as usize];
    }
    out
}

/// Đọc + so **từng byte** một đoạn; trả `true` ⇔ khớp.
fn check_segment(
    fs: &mut Fs<PersistDev>,
    seg: &Seg,
    sha: &mut Sha256,
    mismatched: &mut usize,
) -> bool {
    let ino = match fs.lookup(seg.path, 0) {
        Ok(ino) => ino,
        Err(_) => {
            *mismatched += seg.data.len();
            sha.update(seg.data);
            return false;
        }
    };
    let mut good = true;
    let mut buffer = [0u8; BLOCK_SIZE];
    let mut done = 0usize;
    while done < seg.data.len() {
        let chunk = core::cmp::min(BLOCK_SIZE, seg.data.len() - done);
        match fs.read(ino, seg.offset + done, &mut buffer[..chunk], 0) {
            Ok(count) if count == chunk => {
                for (index, got) in buffer[..chunk].iter().enumerate() {
                    if *got != seg.data[done + index] {
                        *mismatched += 1;
                        good = false;
                    }
                }
                sha.update(&buffer[..chunk]);
            }
            _ => {
                *mismatched += chunk;
                good = false;
                sha.update(&seg.data[done..done + chunk]);
            }
        }
        done += chunk;
    }
    good
}

/// Ghi một bản ghi 4 B vào `/pstate` (append) — nhật ký pha **bền** để boot cuối tự đếm.
fn append_record(fs: &mut Fs<PersistDev>, phase: u8, action: u8, ok: bool) -> bool {
    let ino = match fs.lookup(STATE_PATH, 0) {
        Ok(ino) => ino,
        Err(_) => match fs.create(STATE_PATH, 0, INODE_FILE) {
            Ok(ino) => ino,
            Err(_) => return false,
        },
    };
    let size = match fs.info(ino) {
        Ok(info) => info.size as usize,
        Err(_) => return false,
    };
    let record = [phase, action, ok as u8, 0u8];
    fs.write(ino, size, &record, 0).is_ok()
}

/// Đếm nhật ký pha trên đĩa: `(phases_đủ, writes_ok, verifies_ok, records)`.
fn tally_state(fs: &mut Fs<PersistDev>) -> Option<(u8, u8, u8, usize)> {
    let ino = fs.lookup(STATE_PATH, 0).ok()?;
    let info = fs.info(ino).ok()?;
    let size = info.size as usize;
    if size == 0 || size > 512 || !size.is_multiple_of(4) {
        return None;
    }
    let mut buffer = [0u8; 512];
    let count = fs.read(ino, 0, &mut buffer[..size], 0).ok()?;
    if count != size {
        return None;
    }
    let mut seen_write = [false; 8];
    let mut seen_verify = [false; 8];
    let mut writes_ok = 0u8;
    let mut verifies_ok = 0u8;
    let mut index = 0usize;
    while index + 4 <= size {
        let phase = buffer[index] as usize;
        let action = buffer[index + 1];
        let ok = buffer[index + 2] == 1;
        if phase < seen_write.len() {
            if action == 1 && ok {
                if !seen_write[phase] {
                    writes_ok += 1;
                }
                seen_write[phase] = true;
            } else if action == 2 && ok {
                if !seen_verify[phase] {
                    verifies_ok += 1;
                }
                seen_verify[phase] = true;
            }
        }
        index += 4;
    }
    let mut phases = 0u8;
    for phase in 1..=PHASES as usize {
        if seen_write[phase] && seen_verify[phase] {
            phases += 1;
        }
    }
    Some((phases, writes_ok, verifies_ok, size / 4))
}

// ------------------------------------------------------------------------------ pha ĐỌC LẠI
#[inline(never)]
fn verify_phase(fs: &mut Fs<PersistDev>, phase: u8) -> bool {
    let mut sha = Sha256::new();
    let mut mismatched = 0usize;
    let mut bytes = 0usize;
    let mut size_bad = 0usize;
    let segs = segments(phase);
    for seg in segs {
        if let Ok(ino) = fs.lookup(seg.path, 0) {
            match fs.info(ino) {
                Ok(info) => {
                    if info.size != expected_size(phase, seg.path) || info.kind != INODE_FILE {
                        size_bad += 1;
                    }
                }
                Err(_) => size_bad += 1,
            }
        }
        bytes += seg.data.len();
        let _ = check_segment(fs, seg, &mut sha, &mut mismatched);
    }
    let mut absent_bad = 0usize;
    let mut absent_checked = 0usize;
    for path in absent_paths(phase) {
        absent_checked += 1;
        if fs.lookup(path, 0).is_ok() {
            absent_bad += 1;
        }
    }
    let mut files = 0usize;
    for (index, seg) in segs.iter().enumerate() {
        if segs[..index].iter().all(|other| other.path != seg.path) {
            files += 1;
        }
    }
    let prefix = sha_prefix(&sha.finish());
    let match_ok = mismatched == 0 && size_bad == 0 && absent_bad == 0;
    let recorded = append_record(fs, phase, 2, match_ok);
    let _ = fs.fsync();
    let pending = fs.dev().pending_blocks();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# persist: phase={phase} action=verify files={files} bytes={bytes} read={bytes} \
             absent={absent_checked} match={} sha_prefix={} mismatched={mismatched} \
             size_bad={size_bad} absent_bad={absent_bad} recorded={} pending_blocks={pending} \
             replayed={}",
            match_ok as u8,
            core::str::from_utf8(&prefix).unwrap_or("?"),
            recorded as u8,
            fs.replayed()
        );
    }
    match_ok && recorded && pending == 0
}

// ------------------------------------------------------------------------------ pha GHI
#[inline(never)]
fn write_phase(fs: &mut Fs<PersistDev>, phase: u8, crash: Option<u32>) -> bool {
    let before = virtio_blk::stats();
    let mut wrote = 0usize;
    let mut ok = true;
    let mut detail = 0i64;
    // P7: hai **thành phần** của `detail` để in cả công thức (D2-106b); pha khác = `None`.
    let mut indirect: Option<(u32, u32)> = None;
    match phase {
        1 => match fs.create(b"/p1.txt", 0, INODE_FILE) {
            Ok(ino) => match fs.write(ino, 0, &P1, 0) {
                Ok(count) => wrote = count,
                Err(_) => ok = false,
            },
            Err(_) => ok = false,
        },
        2 => match fs.lookup(b"/p1.txt", 0) {
            Ok(ino) => match fs.write(ino, 0, &P2, 0) {
                Ok(count) => wrote = count,
                Err(_) => ok = false,
            },
            Err(_) => ok = false,
        },
        3 => match fs.lookup(b"/p1.txt", 0) {
            Ok(ino) => {
                let offset = match fs.info(ino) {
                    Ok(info) => info.size as usize,
                    Err(_) => 0,
                };
                detail = offset as i64;
                match fs.write(ino, offset, &P3, 0) {
                    Ok(count) => wrote = count,
                    Err(_) => ok = false,
                }
            }
            Err(_) => ok = false,
        },
        4 => match fs.lookup(b"/p1.txt", 0) {
            Ok(ino) => {
                if fs.truncate(ino, 0, 0).is_err() {
                    ok = false;
                }
                match fs.write(ino, 0, &P4, 0) {
                    Ok(count) => wrote = count,
                    Err(_) => ok = false,
                }
            }
            Err(_) => ok = false,
        },
        5 => {
            if fs.rename(b"/p1.txt", b"/p5.bin", 0).is_err() {
                ok = false;
            }
        }
        6 => {
            // Xoá rồi **tạo lại cùng tên** (`/p5.bin` mới) — kiểm cả xoá bền lẫn cấp lại tên cũ.
            if fs.unlink(b"/p5.bin", 0).is_err() {
                ok = false;
            }
            match fs.create(b"/p5.bin", 0, INODE_FILE) {
                Ok(ino) => match fs.write(ino, 0, &P6, 0) {
                    Ok(count) => wrote = count,
                    Err(_) => ok = false,
                },
                Err(_) => ok = false,
            }
        }
        _ => match fs.create(b"/p7.bin", 0, INODE_FILE) {
            Ok(ino) => {
                if let Some(after) = crash {
                    // **KT-408d (Q7)** — cắt điện tất định. `write()` của FS commit trọn **một**
                    // sector 512 B mỗi vòng, nên ghi từng sector cho ta điểm cắt là **số sector
                    // đã bền**, không phụ thuộc thời gian/tải máy. Vũ trang ngay TRƯỚC sector
                    // thứ `after`: `flush()` kế tiếp (mở transaction của sector đó) in
                    // `crash_cut` rồi `halt(77)` **trước** `push_all` ⇒ đĩa giữ đúng `after`
                    // sector của `/p7.bin`; phần còn lại + journal của transaction dở bay hơi.
                    let target = after as usize;
                    let mut done = 0usize;
                    while done < P7_BLOCKS {
                        if done == target {
                            fs.dev_mut().arm_crash_cut(after);
                        }
                        let start = done * BLOCK_SIZE;
                        match fs.write(ino, start, &P7[start..start + BLOCK_SIZE], 0) {
                            Ok(count) => wrote += count,
                            Err(_) => {
                                ok = false;
                                break;
                            }
                        }
                        done += 1;
                    }
                } else {
                    match fs.write(ino, 0, &P7, 0) {
                        Ok(count) => {
                            wrote = count;
                            if let Some(raw) = fs.inode_blocks_raw(ino) {
                                // block 10 = single-indirect, block 11 = double-indirect (VF04).
                                detail = ((raw[10] as i64) << 32) | raw[11] as i64;
                                indirect = Some((raw[10], raw[11]));
                            }
                        }
                        Err(_) => ok = false,
                    }
                }
            }
            Err(_) => ok = false,
        },
    }
    // Nhật ký pha ghi **sau** dữ liệu của pha (thứ tự thấy được trong `writes`).
    let recorded = append_record(fs, phase, 1, ok);
    let fsync_ok = fs.fsync().is_ok();
    let after = virtio_blk::stats();
    let pending = fs.dev().pending_blocks();
    let flushes = after.flushes - before.flushes;
    let durable = pending == 0 && flushes > 0 && fs.dev().flushes_ignored == 0;
    let phase_ok = ok && recorded && fsync_ok && durable;
    let mut sha = Sha256::new();
    match phase {
        1 => sha.update(&P1),
        2 => sha.update(&P2),
        3 => sha.update(&P3),
        4 => sha.update(&P4),
        5 => {}
        6 => sha.update(&P6),
        _ => sha.update(&P7),
    }
    let prefix = sha_prefix(&sha.finish());
    {
        let dev = fs.dev();
        let mut w = PlatformWriter(&PLATFORM);
        let _ = write!(
            w,
            "# persist: phase={phase} action=write wrote={wrote} recorded={} fsync={} \
             durable={} flush_honored={} flushes_done={} flushes_ignored={} \
             device_flushes={flushes} pending_blocks={pending} pushed={} overflow={} detail={detail}",
            recorded as u8,
            fsync_ok as u8,
            durable as u8,
            dev.honor_flush as u8,
            dev.flushes_done,
            dev.flushes_ignored,
            dev.pushed,
            dev.overflow
        );
        // D2-106b: `detail` của pha bảy là **suy ra** từ hai block — in cả công thức + hai thành phần.
        if let Some((single, double)) = indirect {
            let _ = write!(w, " (=single({single})<<32|double({double}))");
        }
        let _ = writeln!(
            w,
            " sha_prefix={} errors={}",
            core::str::from_utf8(&prefix).unwrap_or("?"),
            after.errors
        );
    }
    phase_ok
}

/// Ca âm: **ghi mà không flush** (thiết bị mô phỏng bỏ `flush`) rồi thoát ⇒ cache RAM mất.
#[inline(never)]
fn negative_write(fs: &mut Fs<PersistDev>) -> bool {
    let before = virtio_blk::stats();
    let mut wrote = 0usize;
    let mut ok = true;
    match fs.create(NEG_PATH, 0, INODE_FILE) {
        Ok(ino) => match fs.write(ino, 0, &PNEG, 0) {
            Ok(count) => wrote = count,
            Err(_) => ok = false,
        },
        Err(_) => ok = false,
    }
    let _ = fs.fsync();
    let after = virtio_blk::stats();
    let pending = fs.dev().pending_blocks();
    let flushes = after.flushes - before.flushes;
    let ignored = fs.dev().flushes_ignored;
    let honor = fs.dev().honor_flush;
    let pushed = fs.dev().pushed;
    let overflow = fs.dev().overflow;
    // Kỳ vọng ca âm: ghi **thành công** nhưng KHÔNG có gì bền (pending>0, không `T_FLUSH`).
    let lost = pending > 0 && flushes == 0 && ignored > 0 && !honor && pushed == 0;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# persist: phase=neg action=noflush_write wrote={wrote} ok={} flush_honored={} \
             flushes_ignored={ignored} device_flushes={flushes} pending_blocks={pending} \
             pushed={pushed} overflow={overflow} bytes_lost={} (=pending({pending})*{BLOCK_SIZE})",
            ok as u8,
            honor as u8,
            pending * BLOCK_SIZE
        );
    }
    ok && lost
}

/// Ca âm: boot **mới** phải thấy `/pneg.txt` **vắng**, P7 còn nguyên, nhật ký pha đủ 7.
#[inline(never)]
fn negative_verify(fs: &mut Fs<PersistDev>) -> bool {
    let absent = fs.lookup(NEG_PATH, 0).is_err();
    let mut sha = Sha256::new();
    let mut mismatched = 0usize;
    for seg in &SEG_P7 {
        let _ = check_segment(fs, seg, &mut sha, &mut mismatched);
    }
    let (phases, writes_ok, verifies_ok, records) = tally_state(fs).unwrap_or((0, 0, 0, 0));
    let prefix = sha_prefix(&sha.finish());
    let ok = absent && mismatched == 0 && phases == PHASES;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# persist: phase=neg action=verify_absent absent={} p7_match={} mismatched={mismatched} \
             phases={phases} writes_ok={writes_ok} verifies_ok={verifies_ok} records={records} \
             sha_prefix={}",
            absent as u8,
            (mismatched == 0) as u8,
            core::str::from_utf8(&prefix).unwrap_or("?")
        );
    }
    ok
}

/// Báo cáo tổng — đếm từ **nhật ký pha trên đĩa**, không từ biến RAM.
#[inline(never)]
fn report_total(fs: &mut Fs<PersistDev>) {
    let (phases, writes_ok, verifies_ok, records) = tally_state(fs).unwrap_or((0, 0, 0, 0));
    let mut w = PlatformWriter(&PLATFORM);
    let _ = writeln!(
        w,
        "# persist: phases={phases} ok={phases} writes_ok={writes_ok} verifies_ok={verifies_ok} \
         records={records} const:phases_want={PHASES} const:records_want={}",
        PHASES * 2
    );
    let _ = writeln!(
        w,
        "# persist: total_ok={}",
        (phases == PHASES && records as u8 == PHASES * 2) as u8
    );
}

/// Dựng FS (một object `Fs` tại một thời điểm) + chạy các hành động của **lần boot này**.
///
/// Tách khỏi `rust_main` để khung stack lớn nhất chỉ chứa `Fs` (≈13 KiB) — nhét thêm 16 KiB
/// cache vào cùng khung đã tràn **guard page** 64 KiB (`PANIC: guard page fault` tại `memcpy`).
#[inline(never)]
fn run_actions(plan: Plan) -> usize {
    let mut fails = 0usize;
    // ---- dựng FS (format khi kịch bản yêu cầu) — MỘT object `Fs` tại một thời điểm
    let mut dev = PersistDev::new(plan.honor_flush());
    let mut formatted = false;
    if plan.format() {
        match vifast_fs::format_with_options(&mut dev, FormatOptions::default()) {
            Ok(()) => formatted = true,
            Err(error) => {
                let mut out = PlatformWriter(&PLATFORM);
                let _ = writeln!(out, "# persist: format err={error:?}");
                tap(1, false, "persist format", None, &mut fails, "format hỏng");
                PLATFORM.halt(4)
            }
        }
    }
    let mut fs = match Fs::mount(dev) {
        Ok(fs) => fs,
        Err(error) => {
            let mut out = PlatformWriter(&PLATFORM);
            let _ = writeln!(out, "# persist: mount err={error:?} formatted={formatted}");
            tap(1, false, "persist mount", None, &mut fails, "mount hỏng");
            PLATFORM.halt(4)
        }
    };
    {
        let mut out = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            out,
            "# persist: fs mounted=1 formatted={} replayed={} capacity={} pending_blocks={}",
            formatted as u8,
            fs.replayed(),
            fs.dev().block_count(),
            fs.dev().pending_blocks()
        );
    }

    // ---- pha ĐỌC LẠI (boot mới) rồi pha GHI kế tiếp
    if plan.mode == MODE_NEG_WRITE {
        let ok = negative_write(&mut fs);
        tap(
            1,
            ok,
            "persist noflush write",
            None,
            &mut fails,
            "ghi không flush lại bền (sai)",
        );
    } else if plan.mode == MODE_NEG_VERIFY {
        let ok = negative_verify(&mut fs);
        tap(
            1,
            ok,
            "persist noflush verify",
            None,
            &mut fails,
            "ca âm: dữ liệu không flush lại BỀN, hoặc P7/nhật ký pha hỏng",
        );
    } else {
        let mut index = 1usize;
        if plan.verify > 0 {
            let phase = plan.verify;
            let ok = verify_phase(&mut fs, phase);
            tap(
                index,
                ok,
                "persist verify phase",
                Some(phase),
                &mut fails,
                "byte đọc lại LỆCH / băm lệch / nhật ký pha hỏng",
            );
            index += 1;
            if phase == PHASES {
                report_total(&mut fs);
            }
        }
        if plan.write > 0 {
            let phase = plan.write;
            let ok = write_phase(&mut fs, phase, plan.crash_cut());
            tap(
                index,
                ok,
                "persist write phase",
                Some(phase),
                &mut fails,
                "pha ghi không bền (pending>0 / không có T_FLUSH / lỗi FS)",
            );
            let _ = ok;
        }
    }
    fails
}

pub fn run() -> ! {
    PLATFORM.put_str("TAP version 13\n");
    PLATFORM.set_trap_vector(trap_vector());
    let _ = read_trap_vector();
    let _mmu = mmu::init();
    let mut fails = 0usize;

    // ---- thiết bị (PCI ECAM + virtio-blk)
    let mut w = PlatformWriter(&PLATFORM);
    let pci_report = match pci::init() {
        Ok(report) => report,
        Err(fail) => {
            let _ = writeln!(
                w,
                "# persist: pci err={} stage={}",
                fail.error.name(),
                fail.stage
            );
            let _ = writeln!(w, "1..1");
            tap(
                1,
                false,
                "persist device",
                None,
                &mut fails,
                "không init được PCI",
            );
            PLATFORM.halt(2)
        }
    };
    let blk = match virtio_blk::init(&pci_report) {
        Ok(report) => report,
        Err(stage) => {
            let _ = writeln!(w, "# persist: blk init_fail={stage}");
            let _ = writeln!(w, "1..1");
            tap(
                1,
                false,
                "persist device",
                None,
                &mut fails,
                "không init được virtio-blk",
            );
            PLATFORM.halt(2)
        }
    };
    let mut plan_block = [0u8; BLOCK_SIZE];
    if virtio_blk::read_sector(PLAN_LBA, &mut plan_block).is_err() {
        let _ = writeln!(w, "# persist: plan read lba={PLAN_LBA} err=Io");
        let _ = writeln!(w, "1..1");
        tap(
            1,
            false,
            "persist plan",
            None,
            &mut fails,
            "không đọc được sector kịch bản",
        );
        PLATFORM.halt(3)
    }
    let plan = match parse_plan(&plan_block) {
        Some(plan) => plan,
        None => {
            let _ = writeln!(
                w,
                "# persist: plan invalid magic={} ver={} verify={} write={} flags={} mode={}",
                core::str::from_utf8(&plan_block[..4]).unwrap_or("?"),
                plan_block[4],
                plan_block[5],
                plan_block[6],
                plan_block[7],
                plan_block[8]
            );
            let _ = writeln!(w, "1..1");
            tap(
                1,
                false,
                "persist plan",
                None,
                &mut fails,
                "kịch bản ở LBA 0 không hợp lệ",
            );
            PLATFORM.halt(3)
        }
    };
    let actions = (plan.verify > 0) as usize
        + (plan.write > 0) as usize
        + (plan.mode != MODE_NORMAL && plan.mode != MODE_CRASH_MID_WRITE) as usize;
    let _ = writeln!(w, "1..{actions}");
    let _ = writeln!(
        w,
        "# persist: plan magic={} ver={} nonce={} verify={} write={} flags={} mode={} \
         crash_after={} disk_sectors={} const:slots={CACHE_SLOTS}",
        core::str::from_utf8(PLAN_MAGIC).unwrap_or("?"),
        PLAN_VERSION,
        plan.nonce,
        plan.verify,
        plan.write,
        plan.flags,
        plan.mode,
        plan.crash_after,
        blk.capacity
    );

    let action_fails = run_actions(plan);
    let fails = fails + action_fails;
    let stats = virtio_blk::stats();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# persist: stats requests={} reads={} writes={} flushes={} completions={} errors={} \
             notifies={} id_mismatch={} timeouts={} capacity={} fails={fails}",
            stats.requests,
            stats.reads,
            stats.writes,
            stats.flushes,
            stats.completions,
            stats.errors,
            stats.notifies,
            stats.id_mismatch,
            stats.timeouts,
            blk.capacity
        );
        let _ = writeln!(w, "# persist: halt code=0 fails={fails} actions={actions}");
    }
    PLATFORM.halt(if fails == 0 { 0 } else { 1 })
}
