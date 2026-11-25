// SPDX-License-Identifier: MIT OR Apache-2.0
// **KT-404** (`D2-259`) — crate này **0 `unsafe`** ⇒ khoá bằng máy: mọi `unsafe`
// thêm vào (kể cả trong macro) là lỗi biên dịch, không chỉ là luật cấu trúc R5.
#![forbid(unsafe_code)]
//! `vifast-ring` — **ring 1 producer / 1 consumer** trên một `Frame` chia sẻ (KT-69).
//!
//! # Bất biến
//!
//! - Header cố định trong 4 KiB (`[`MAGIC`]`, version, `size` luỹ thừa 2, `mask`, `slot_bytes`,
//!   `head`, `tail`, `seq`). `head`/`tail` là bộ đếm **một chiều** (không reset khi wrap): slot
//!   của bộ đếm `n` là `PAYLOAD_OFF + (n & mask) * slot_bytes`.
//! - `send` chỉ ghi **một slot** (không copy cả buffer) rồi **publish** `head` bằng
//!   [`Mem::store_rel`] (Release); `recv` đọc `head` bằng [`Mem::acquire`] (Acquire) trước khi
//!   đọc slot ⇒ consumer **không** đọc slot producer đang ghi.
//! - `send` khi `head - tail == size` trả [`ERR_FULL`] (**không** chặn, **không** ghi đè);
//!   `recv` khi `head == tail` trả [`ERR_EMPTY`] (**không** chặn). Hai mã này là mã ABI hiện có
//!   — `NotEnoughMemory = 9` (hết chỗ) và `Busy = 14` (chưa có dữ liệu) — **không** thêm mã mới.
//! - Producer chỉ ghi `head`/`seq`, consumer chỉ ghi `tail` ⇒ không cần CAS/khoá, không `unsafe`.
//!
//! Crate **không** biết địa chỉ vật lý/ảo: mọi truy cập đi qua trait [`Mem`]. U-mode cài
//! `Mem` bằng shim `vifast_user_abi` (atomic Acquire/Release trên VA user); kernel cài bằng
//! physmap (`read_phys_u64`) để **đo độc lập**; host test cài bằng mảng `AtomicU64` (kể cả hai
//! thread thật). Nhờ vậy cùng một logic ring được kiểm ở cả ba tầng, không `unsafe` ở đây.
#![no_std]

/// Magic header: `b"KT69RING"` little-endian.
pub const MAGIC: u64 = 0x474E_4952_3936_544B;
/// Phiên bản layout header.
pub const VERSION: u64 = 1;
/// Số word của một message (slot tối thiểu).
pub const MSG_WORDS: usize = 4;
/// Số byte tối thiểu của một slot.
pub const MSG_BYTES: usize = MSG_WORDS * 8;
/// `send` khi ring đầy — mã ABI `NotEnoughMemory = 9` (không thêm mã mới).
pub const ERR_FULL: u32 = 9;
/// `recv` khi ring rỗng — mã ABI `Busy = 14` (cùng mã `NBRecv` rỗng, không thêm mã mới).
pub const ERR_EMPTY: u32 = 14;

/// Offset các trường header trong Frame.
pub const H_MAGIC: usize = 0x00;
/// Offset `version`.
pub const H_VERSION: usize = 0x08;
/// Offset `size` (sức chứa, luỹ thừa 2).
pub const H_SIZE: usize = 0x10;
/// Offset `mask` (`size - 1`).
pub const H_MASK: usize = 0x18;
/// Offset `slot_bytes`.
pub const H_SLOT_BYTES: usize = 0x20;
/// Offset `head` — producer ghi (Release), consumer đọc (Acquire).
pub const H_HEAD: usize = 0x28;
/// Offset `tail` — consumer ghi (Release), producer đọc (Acquire).
pub const H_TAIL: usize = 0x30;
/// Offset `seq` — số message đã publish (bằng `head`, giữ riêng theo yêu cầu header).
pub const H_SEQ: usize = 0x38;
/// Hết header (trước khối stats).
pub const HEADER_BYTES: usize = 0x40;
/// Offset khối quan sát (do chương trình U-mode ghi, kernel đọc).
pub const STATS_OFF: usize = 0x80;
/// Magic khối stats — chứng minh chương trình đã chạm đúng vùng.
pub const STATS_MAGIC: u64 = 0x5354_4154_5336_394B;
/// Offset payload slot 0.
pub const PAYLOAD_OFF: usize = 0x200;
/// Cỡ Frame mặc định của ring (một Frame 4 KiB).
pub const FRAME_BYTES: usize = 4096;
/// Số trường stats.
pub const STAT_COUNT: usize = 34;

/// Trường của khối quan sát. **Sở hữu một bên** (không tranh chấp): server ghi
/// `ServerSteps/SentOk/ProdSum/Full*/SyncMsgs/AttachMsgs/LabelsSeen/Wrap`; client ghi
/// `ClientSteps/RecvOk/ConsSum/Empty*/FirstSeq/LastSeq/PeakUsed/Dups/Lost/BadSlot/HdrErr/
/// AttachCaps/FinishMsgs/ClientDone/SyncAcks`. `Magic` do server ghi lúc `init`. TAP **không**
/// dùng lời khai của chương trình cho hình học ring: kernel đọc header qua physmap (D2-102).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Stat {
    /// `[`STATS_MAGIC`]` — U-mode ghi sau khi mở ring.
    Magic = 0,
    /// Số bước scheduler server đã chạy.
    ServerSteps,
    /// Số bước scheduler client đã chạy.
    ClientSteps,
    /// Số message `send` thành công (server).
    SentOk,
    /// Số message `recv` thành công (client).
    RecvOk,
    /// Số lần `send` gặp đầy (server đếm).
    FullErr,
    /// Số lần `recv` gặp rỗng (client đếm).
    EmptyErr,
    /// Mã lỗi **thật** lần `send` đầy (phải bằng [`ERR_FULL`]).
    FullCode,
    /// Mã lỗi **thật** lần `recv` rỗng **đầu tiên** (phải bằng [`ERR_EMPTY`]).
    EmptyCode,
    /// Sequence đầu tiên client nhận.
    FirstSeq,
    /// Sequence cuối cùng client nhận.
    LastSeq,
    /// Số vòng wrap **quan sát được** (`head / size` khi kết thúc).
    Wrap,
    /// `1` ⇔ client kiểm từng message đúng thứ tự, không mất/nhân đôi.
    OrderOk,
    /// Số lần header/`recv` lỗi bất thường (không đọc được slot, mã rỗng lệch) — phải 0.
    HdrErr,
    /// Số message `attach` đã gửi/nhận.
    AttachMsgs,
    /// Số message `sync` đã gửi/nhận.
    SyncMsgs,
    /// Số message `finish` đã gửi/nhận.
    FinishMsgs,
    /// Bitmask method id đã thấy (bit `n` = method `n`).
    LabelsSeen,
    /// `1` ⇔ server đã xong (gửi reply `finish`).
    ServerDone,
    /// `1` ⇔ client đã xong (nhận reply `finish`).
    ClientDone,
    /// Tổng giá trị payload server đã gửi.
    ProdSum,
    /// Tổng giá trị payload client đã nhận.
    ConsSum,
    /// `peek_used` lớn nhất client quan sát (`head - tail`) — bằng chứng ring từng đầy.
    PeakUsed,
    /// Số message trùng sequence client phát hiện (phải 0).
    Dups,
    /// Số message thiếu trong dãy sequence (phải 0).
    Lost,
    /// Số slot sai checksum (phải 0).
    BadSlot,
    /// `head` lúc `send` gặp đầy lần đầu.
    FullHead,
    /// `tail` lúc `recv` gặp rỗng lần đầu.
    EmptyTail,
    /// Số cap client **nhận thật** trong `attach` (phải 1).
    AttachCaps,
    /// `1` ⇔ hai bên đều `Ring::open`/`check` xanh.
    RingCheck,
    /// Sức chứa slot hai bên đọc được.
    Slots,
    /// `slot_bytes` hai bên đọc được.
    SlotBytes,
    /// Số message `sync` client đã nhận reply.
    SyncAcks,
    /// Mã lỗi **thật** lần `recv` rỗng **cuối cùng** (Dbl kiểm với `EmptyCode` đầu).
    EmptyCodeLast,
}

impl Stat {
    /// Offset tuyệt đối trong Frame.
    pub const fn offset(self) -> usize {
        STATS_OFF + (self as usize) * 8
    }
}

/// Truy cập bộ nhớ của Frame — cài đặt theo tầng (user atomic / kernel physmap / host test).
pub trait Mem {
    /// Đọc thường (không ràng buộc thứ tự).
    fn load(&self, offset: usize) -> u64;
    /// Ghi thường.
    fn store(&self, offset: usize, value: u64);
    /// Đọc **Acquire** — thấy mọi ghi trước `store_rel` của bên kia.
    fn acquire(&self, offset: usize) -> u64;
    /// Ghi **Release** — mọi ghi trước đó thấy được sau `acquire` của bên kia.
    fn store_rel(&self, offset: usize, value: u64);
}

impl<T: Mem + ?Sized> Mem for &T {
    fn load(&self, offset: usize) -> u64 {
        (**self).load(offset)
    }
    fn store(&self, offset: usize, value: u64) {
        (**self).store(offset, value)
    }
    fn acquire(&self, offset: usize) -> u64 {
        (**self).acquire(offset)
    }
    fn store_rel(&self, offset: usize, value: u64) {
        (**self).store_rel(offset, value)
    }
}

/// Hình học ring đọc từ header (đã kiểm).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    /// Sức chứa (slot).
    pub size: u32,
    /// `size - 1`.
    pub mask: u32,
    /// Byte mỗi slot.
    pub slot_bytes: u32,
}

/// Trạng thái header đo được.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    /// `head` (tổng đã publish).
    pub head: u64,
    /// `tail` (tổng đã tiêu thụ).
    pub tail: u64,
    /// `seq`.
    pub seq: u64,
    /// Slot đang dùng (`head - tail`).
    pub used: u64,
    /// Sức chứa.
    pub size: u32,
    /// Byte mỗi slot.
    pub slot_bytes: u32,
}

/// Ring trên một vùng nhớ 4 KiB.
pub struct Ring<M: Mem> {
    mem: M,
}

impl<M: Mem> Ring<M> {
    /// Bọc vùng nhớ (chưa kiểm header — gọi [`Self::open`] hoặc [`Self::init`]).
    pub const fn new(mem: M) -> Self {
        Self { mem }
    }

    /// Khởi tạo header (producer, trước khi consumer chạy). Magic ghi **cuối** (Release) để
    /// consumer thấy header đầy đủ hoặc chưa thấy gì.
    pub fn init(&self, size: u32, slot_bytes: u32) -> Result<Geometry, u32> {
        let geometry = checked_geometry(size, slot_bytes)?;
        self.mem.store(H_VERSION, VERSION);
        self.mem.store(H_SIZE, u64::from(size));
        self.mem.store(H_MASK, u64::from(geometry.mask));
        self.mem.store(H_SLOT_BYTES, u64::from(slot_bytes));
        self.mem.store(H_HEAD, 0);
        self.mem.store(H_TAIL, 0);
        self.mem.store(H_SEQ, 0);
        self.mem.store(Stat::Magic.offset(), STATS_MAGIC);
        self.mem.store_rel(H_MAGIC, MAGIC);
        Ok(geometry)
    }

    /// Mở/kiểm header. `Err(ERR_EMPTY)` = chưa init (magic 0), lỗi khác = header hỏng.
    pub fn open(&self) -> Result<Geometry, u32> {
        if self.mem.acquire(H_MAGIC) != MAGIC {
            return Err(ERR_EMPTY);
        }
        if self.mem.load(H_VERSION) != VERSION {
            return Err(ERR_FULL);
        }
        let size = self.mem.load(H_SIZE) as u32;
        let slot_bytes = self.mem.load(H_SLOT_BYTES) as u32;
        let geometry = checked_geometry(size, slot_bytes)?;
        if self.mem.load(H_MASK) != u64::from(geometry.mask) {
            return Err(ERR_FULL);
        }
        Ok(geometry)
    }

    /// Gửi **một** slot; đầy ⇒ [`ERR_FULL`], không ghi đè, không chặn.
    pub fn send(&self, message: &[u64; MSG_WORDS]) -> Result<(), u32> {
        let geometry = self.open()?;
        let head = self.mem.load(H_HEAD);
        let tail = self.mem.acquire(H_TAIL);
        if head.wrapping_sub(tail) >= u64::from(geometry.size) {
            return Err(ERR_FULL);
        }
        let base = slot_offset(geometry, head);
        for (index, word) in message.iter().enumerate() {
            self.mem.store(base + index * 8, *word);
        }
        // Release: slot hoàn chỉnh trước khi head tăng.
        self.mem.store_rel(H_HEAD, head.wrapping_add(1));
        self.mem.store(H_SEQ, head.wrapping_add(1));
        Ok(())
    }

    /// Nhận **một** slot; rỗng ⇒ [`ERR_EMPTY`], không chặn.
    pub fn recv(&self, out: &mut [u64; MSG_WORDS]) -> Result<(), u32> {
        let geometry = self.open()?;
        let tail = self.mem.load(H_TAIL);
        let head = self.mem.acquire(H_HEAD);
        if head == tail {
            return Err(ERR_EMPTY);
        }
        let base = slot_offset(geometry, tail);
        for (index, word) in out.iter_mut().enumerate() {
            *word = self.mem.load(base + index * 8);
        }
        // Release: đọc xong slot trước khi tail tăng (producer được ghi đè).
        self.mem.store_rel(H_TAIL, tail.wrapping_add(1));
        Ok(())
    }

    /// Đọc header (không kiểm magic — dùng để in bằng chứng cả khi hỏng).
    pub fn stats(&self) -> Stats {
        let size = self.mem.load(H_SIZE) as u32;
        let slot_bytes = self.mem.load(H_SLOT_BYTES) as u32;
        let head = self.mem.load(H_HEAD);
        let tail = self.mem.load(H_TAIL);
        Stats {
            head,
            tail,
            seq: self.mem.load(H_SEQ),
            used: head.wrapping_sub(tail),
            size,
            slot_bytes,
        }
    }

    /// Bất biến header: magic/version/hình học/`head >= tail`/`used <= size`/payload trong Frame.
    pub fn check(&self) -> bool {
        if self.mem.load(H_MAGIC) != MAGIC || self.mem.load(H_VERSION) != VERSION {
            return false;
        }
        let size = self.mem.load(H_SIZE) as u32;
        let slot_bytes = self.mem.load(H_SLOT_BYTES) as u32;
        let Ok(geometry) = checked_geometry(size, slot_bytes) else {
            return false;
        };
        if self.mem.load(H_MASK) != u64::from(geometry.mask) {
            return false;
        }
        let stats = self.stats();
        stats.head >= stats.tail
            && stats.used <= u64::from(geometry.size)
            && stats.seq == stats.head
            && stats.size == size
            && stats.slot_bytes == slot_bytes
    }

    /// Đọc một trường khối quan sát.
    pub fn stat(&self, stat: Stat) -> u64 {
        self.mem.load(stat.offset())
    }

    /// Ghi một trường khối quan sát (mỗi trường một bên sở hữu).
    pub fn set_stat(&self, stat: Stat, value: u64) {
        self.mem.store(stat.offset(), value);
    }

    /// Cộng dồn một trường (một bên sở hữu ⇒ đọc-sửa-ghi tuần tự).
    pub fn add_stat(&self, stat: Stat, delta: u64) {
        self.set_stat(stat, self.stat(stat).wrapping_add(delta));
    }
}

fn checked_geometry(size: u32, slot_bytes: u32) -> Result<Geometry, u32> {
    if size == 0 || !size.is_power_of_two() {
        return Err(ERR_FULL);
    }
    if slot_bytes < MSG_BYTES as u32 || !slot_bytes.is_multiple_of(8) {
        return Err(ERR_FULL);
    }
    let payload_end = PAYLOAD_OFF + (size as usize) * (slot_bytes as usize);
    if payload_end > FRAME_BYTES {
        return Err(ERR_FULL);
    }
    Ok(Geometry {
        size,
        mask: size - 1,
        slot_bytes,
    })
}

fn slot_offset(geometry: Geometry, counter: u64) -> usize {
    PAYLOAD_OFF + ((counter & u64::from(geometry.mask)) as usize) * (geometry.slot_bytes as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_rejects_bad_shapes() {
        assert!(checked_geometry(8, 32).is_ok());
        assert!(checked_geometry(0, 32).is_err());
        assert!(checked_geometry(6, 32).is_err());
        assert!(checked_geometry(8, 24).is_err());
        assert!(
            checked_geometry(256, 32).is_err(),
            "payload phải nằm trong 4 KiB"
        );
    }

    #[test]
    fn stat_offsets_trong_khoi_quan_sat() {
        assert_eq!(Stat::Magic.offset(), STATS_OFF);
        assert_eq!(
            Stat::EmptyCodeLast.offset(),
            STATS_OFF + (STAT_COUNT - 1) * 8,
            "enum phải đúng STAT_COUNT"
        );
        assert!(Stat::SyncAcks.offset() < Stat::EmptyCodeLast.offset());
        assert!(Stat::EmptyCodeLast.offset() + 8 <= PAYLOAD_OFF);
    }
}
