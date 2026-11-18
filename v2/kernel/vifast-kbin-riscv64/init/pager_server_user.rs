// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-70** — `pager_server` U-mode: pager + **page cache** của VFS tầng 1.
//!
//! Giao thức: interface `pager` (stub sinh từ `v2/lib/vifast-idl/idl/pager.vidl`, KT-69):
//! kernel gửi `fault(client,file,page,va)`; pager trả `served(client,page,slot,status)`, gửi
//! `ready` sau khi map pool, và `finish` khi nhận `stop`. Kernel giữ vai scheduler tuần tự.
//!
//! Mọi số đo ghi vào **khối stats** trong Frame chia sẻ (kernel đọc qua physmap); chương trình
//! **không** in số (D2-102). Không `unsafe`, không lệnh đặc quyền (D2-101).

#![no_std]
#![no_main]

extern crate vifast_abi;
extern crate vifast_idl;
extern crate vifast_user_abi;
extern crate vifast_vfs;

#[path = "pager_common.rs"]
mod common;

use common::{
    result, P_DISK0, P_EP_FAULT, P_EP_REPLY, P_POOL0, P_SELF, P_STATS, P_VSPACE, P_VS_A, P_VS_B,
    P_VS_C, P_VS_D, VA_DISK, VA_POOL, VA_RESULT, VA_STATS, CLIENTS, DISK_FRAMES, FLAGS_RU,
    FLAGS_RWU,
};
use vifast_vfs::SECTOR_SIZE as BLOCK_SIZE;
use vifast_idl::pager::{decode, encode_finish, encode_ready, encode_served, Request, METHOD_FAULT};
use vifast_user_abi::{
    call, exit, info, info_label, info_length, nb_recv, nb_send, note_received, peek_u64, poke_u64,
    put, set_recv_window, yield_now,
};
use vifast_vfs::{
    byte_at, err, file_pages, sector_page, Begin, BlockDev, FsError, PageSink, PagerCore,
    PagerStats, Stat, CACHE_CAPACITY, DISK_SECTORS, PAGE_SIZE,
};

/// VA "chưa map" trong bảng theo dõi mapping.
const NO_VA: u64 = u64::MAX;

/// Backend **RAM** của pager: 5 Frame 4 KiB kernel cấp, map ở [`VA_DISK`] (không `.bss` 20 KiB trong
/// ảnh U-mode — trang U-mode chỉ có 28 KiB từ `USER_BASE` tới guard).
struct MappedDisk;

impl BlockDev for MappedDisk {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        if lba >= DISK_SECTORS {
            return Err(FsError::Invalid);
        }
        let base = VA_DISK + u64::from(lba) * 512;
        let mut index = 0usize;
        while index < BLOCK_SIZE / 8 {
            let word = peek_u64(base + index as u64 * 8);
            out[index * 8..index * 8 + 8].copy_from_slice(&word.to_le_bytes());
            index += 1;
        }
        Ok(())
    }

    fn write(&mut self, lba: u32, data: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        if lba >= DISK_SECTORS {
            return Err(FsError::Invalid);
        }
        let base = VA_DISK + u64::from(lba) * 512;
        let mut index = 0usize;
        while index < BLOCK_SIZE / 8 {
            let word = u64::from_le_bytes(data[index * 8..index * 8 + 8].try_into().unwrap_or([0u8; 8]));
            poke_u64(base + index as u64 * 8, word);
            index += 1;
        }
        Ok(())
    }

    fn block_count(&self) -> u32 {
        DISK_SECTORS
    }
}

/// Nạp dữ liệu mẫu vào đĩa RAM **qua BlockDev** (chứng minh backend thay thế được: mọi thao tác
/// đọc/ghi của pager đi qua trait, không chạm trực tiếp).
fn seed_disk() {
    let mut dev = MappedDisk;
    for lba in 0..DISK_SECTORS {
        let mut sector = [0u8; BLOCK_SIZE];
        if let Some((file, page, sector_in_page)) = sector_page(lba) {
            let mut offset = 0usize;
            while offset < BLOCK_SIZE {
                sector[offset] = byte_at(file, page, sector_in_page as usize * BLOCK_SIZE + offset);
                offset += 1;
            }
        }
        let _ = dev.write(lba, &sector);
    }
}

/// Sink ghi sector vào **Frame pool** ở VA của pager.
struct FrameSink;

impl PageSink for FrameSink {
    fn write_sector(&mut self, slot: u8, sector_in_page: u32, data: &[u8; BLOCK_SIZE]) {
        let base = VA_POOL + u64::from(slot) * PAGE_SIZE as u64 + u64::from(sector_in_page) * 512;
        for (index, chunk) in data.chunks_exact(8).enumerate() {
            poke_u64(base + index as u64 * 8, u64::from_le_bytes(chunk.try_into().unwrap_or([0u8; 8])));
        }
    }
}

/// Ghi một bộ đếm trong khối stats.
fn st(stat: Stat, value: u64) {
    poke_u64(VA_STATS + stat.offset() as u64, value);
}

/// Cộng dồn một bộ đếm.
fn add(stat: Stat, delta: u64) {
    let value = peek_u64(VA_STATS + stat.offset() as u64).wrapping_add(delta);
    st(stat, value);
}

fn get(stat: Stat) -> u64 {
    peek_u64(VA_STATS + stat.offset() as u64)
}

/// Trạng thái pager (một tiến trình một luồng ⇒ không cần khoá).
struct Server {
    core: PagerCore<CACHE_CAPACITY, MappedDisk>,
    /// VA đã map cho từng `(slot, client)`; `NO_VA` = chưa.
    mapped: [[u64; CLIENTS as usize]; CACHE_CAPACITY],
    /// Khoá từng nạp (để đếm "nạp lại sau trục xuất").
    ever: [(u32, u32); 16],
    n_ever: usize,
    faults: u64,
    served: u64,
    rejected: u64,
    maps: u64,
    unmaps: u64,
    shared: u64,
    reloads: u64,
    /// Ca âm TAP 35 đã chạy (một lần, sau khi cả hai client map khối kết quả).
    neg_done: bool,
    done: bool,
}

impl Server {
    fn new() -> Self {
        Self {
            core: PagerCore::new(MappedDisk),
            mapped: [[NO_VA; CLIENTS as usize]; CACHE_CAPACITY],
            ever: [(0, 0); 16],
            n_ever: 0,
            faults: 0,
            served: 0,
            rejected: 0,
            maps: 0,
            unmaps: 0,
            shared: 0,
            reloads: 0,
            neg_done: false,
            done: false,
        }
    }

    fn vspace_cap(&self, client: u32) -> u64 {
        // KT-157: bốn client, mỗi client một VSpace riêng (A/B/C/D).
        match client {
            0 => P_VS_A,
            1 => P_VS_B,
            2 => P_VS_C,
            _ => P_VS_D,
        }
    }

    fn was_ever(&self, key: (u32, u32)) -> bool {
        let mut index = 0;
        while index < self.n_ever {
            if self.ever[index] == key {
                return true;
            }
            index += 1;
        }
        false
    }

    fn push_ever(&mut self, key: (u32, u32)) {
        if self.n_ever < self.ever.len() {
            self.ever[self.n_ever] = key;
            self.n_ever += 1;
        }
    }

    /// Map Frame `slot` vào VSpace client tại `va`; `Err(id)` = mã ABI **thật** của invocation
    /// (kernel in tên qua bảng spec — không thay bằng mã giả).
    fn map_into(&mut self, slot: u8, client: u32, va: u64) -> Result<(), u16> {
        let result = call(
            P_POOL0 + u64::from(slot),
            info(vifast_abi::FRAME_MAP, 0, false, 3),
            [self.vspace_cap(client), va, FLAGS_RU, 0],
        );
        if result[0] != 0 {
            return Err(result[0] as u16);
        }
        self.maps += 1;
        add(Stat::Maps, 1);
        self.mapped[slot as usize][client as usize] = va;
        Ok(())
    }

    /// Gỡ mapping Frame `slot` khỏi **mọi** client đang map (trước khi ghi đè dữ liệu).
    fn unmap_all(&mut self, slot: u8) {
        for client in 0..CLIENTS {
            let va = self.mapped[slot as usize][client as usize];
            if va == NO_VA {
                continue;
            }
            let result = call(
                P_POOL0 + u64::from(slot),
                info(vifast_abi::FRAME_UNMAP, 0, false, 2),
                [self.vspace_cap(client), va, 0, 0],
            );
            if result[0] == 0 {
                self.unmaps += 1;
                add(Stat::Unmaps, 1);
            }
            self.mapped[slot as usize][client as usize] = NO_VA;
        }
    }

    /// **TAP 35** — hai ca âm đo **mã lỗi thật** (không đoán, không bỏ qua):
    /// (a) `Frame.Map` với aux = **CNode cap** của pager (slot `P_SELF`) — sai loại cap ⇒
    ///     `WrongObjectType` (core kiểm loại cap phụ trước object layer);
    /// (b) `Frame.Map` vào VA **đã có mapping** trong VSpace client B (khối kết quả) —
    ///     `AlreadyMapped`; PTE cũ **không** bị ghi đè (kernel chứng minh PA khối kết quả không đổi).
    fn negatives_once(&mut self) {
        if self.neg_done {
            return;
        }
        self.neg_done = true;
        let aux = call(
            P_POOL0,
            info(vifast_abi::FRAME_MAP, 0, false, 3),
            [P_SELF, VA_POOL, FLAGS_RWU, 0],
        );
        st(Stat::NegAux, aux[0]);
        let over = call(
            P_POOL0,
            info(vifast_abi::FRAME_MAP, 0, false, 3),
            [P_VS_B, VA_RESULT, FLAGS_RU, 0],
        );
        st(Stat::NegOver, over[0]);
    }

    /// Gửi `served`; `status_id = 0` ⇔ thành công (mọi mã lỗi khác lấy từ [`err`]).
    fn send_served(&mut self, client: u32, page: u32, slot: u8, status_id: u16) {
        let (info_word, mr) = encode_served(client, page, u32::from(slot), u32::from(status_id));
        let _ = nb_send(P_EP_REPLY, info_word, mr);
    }

    fn reject_status(&mut self, client: u32, page: u32, status: vifast_vfs::Status) {
        self.reject_code(client, page, status.id());
    }

    fn reject_code(&mut self, client: u32, page: u32, status_id: u16) {
        self.rejected += 1;
        add(Stat::Rejected, 1);
        st(Stat::LastRejectClient, u64::from(client));
        st(Stat::LastRejectStatus, u64::from(status_id));
        self.send_served(client, page, 0, status_id);
    }

    fn publish(&self) {
        let stats: PagerStats = self.core.stats();
        st(Stat::Hits, stats.hits);
        st(Stat::Misses, stats.misses);
        st(Stat::Evictions, stats.evictions);
        st(Stat::Reads, stats.reads);
        st(Stat::Sectors, stats.sectors);
        st(Stat::Served, self.served);
        st(Stat::Entries, stats.entries as u64);
        st(Stat::UniqueKeys, if self.core.keys_unique() { stats.entries as u64 } else { 0 });
        st(Stat::Faults, self.faults);
        st(Stat::Rejected, self.rejected);
        st(Stat::Maps, self.maps);
        st(Stat::Unmaps, self.unmaps);
        st(Stat::Clients, u64::from(CLIENTS));
        st(Stat::SharedHits, self.shared);
        st(Stat::Reloads, self.reloads);
    }

    fn fault(&mut self, client: u32, file: u32, page: u32, va: u64) {
        self.negatives_once();
        self.faults += 1;
        add(Stat::Faults, 1);
        if client >= CLIENTS {
            self.reject_status(client, page, err::INVALID_ARGUMENT);
            return;
        }
        let outcome = self.core.begin(file, page);
        match outcome {
            Begin::Reject { status } => self.reject_status(client, page, status),
            Begin::Hit { slot } => {
                let mut shared = false;
                for other in 0..CLIENTS {
                    if other != client && self.mapped[slot as usize][other as usize] != NO_VA {
                        shared = true;
                    }
                }
                match self.map_into(slot, client, va) {
                    Ok(()) => {
                        self.served += 1;
                        add(Stat::Served, 1);
                        if shared {
                            self.shared += 1;
                            add(Stat::SharedHits, 1);
                        }
                        self.send_served(client, page, slot, 0);
                    }
                    Err(code) => self.reject_code(client, page, code),
                }
            }
            Begin::Ready { slot } => self.load_and_map(slot, client, file, page, va),
            Begin::Evict {
                slot,
                key,
                dirty: _,
            } => {
                add(Stat::Evictions, 1);
                // Gỡ mapping cũ **trước** khi ghi đè Frame (client khác không thấy dữ liệu mới
                // qua mapping cũ).
                self.unmap_all(slot);
                let _ = key;
                self.load_and_map(slot, client, file, page, va)
            }
        }
    }

    fn load_and_map(&mut self, slot: u8, client: u32, file: u32, page: u32, va: u64) {
        let key = (file, page);
        if self.was_ever(key) {
            self.reloads += 1;
            add(Stat::Reloads, 1);
        } else {
            self.push_ever(key);
        }
        let mut sink = FrameSink;
        if let Err(status) = self.core.load(slot, file, page, &mut sink) {
            self.reject_status(client, page, status);
            return;
        }
        self.publish();
        match self.map_into(slot, client, va) {
            Ok(()) => {
                self.served += 1;
                add(Stat::Served, 1);
                self.send_served(client, page, slot, 0);
            }
            Err(code) => self.reject_code(client, page, code),
        }
        self.publish();
    }
}

/// Mã lỗi "không có" = 0 trong khối stats — in qua `err::OK.id()` nếu cần.
const _: u16 = 0;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Window 4 ô phân biệt (KT-69): apply_ipc_header kiểm ở mọi lần gửi.
    set_recv_window([12, 13, 14, 15]);
    // 1) Map khối stats + pool Frame vào VSpace của chính pager (invocation Frame.Map).
    let mapped_stats = call(
        P_STATS,
        info(vifast_abi::FRAME_MAP, 0, false, 3),
        [P_VSPACE, VA_STATS, FLAGS_RWU, 0],
    );
    if mapped_stats[0] != 0 {
        exit(0x50 + mapped_stats[0])
    }
    for slot in 0..CACHE_CAPACITY as u64 {
        let mapped = call(
            P_POOL0 + slot,
            info(vifast_abi::FRAME_MAP, 0, false, 3),
            [P_VSPACE, VA_POOL + slot * PAGE_SIZE as u64, FLAGS_RWU, 0],
        );
        if mapped[0] != 0 {
            exit(61)
        }
    }
    // Đĩa RAM: 5 Frame 4 KiB, map R|W|U rồi seed qua `BlockDev` (không .bss trong ảnh).
    for index in 0..DISK_FRAMES {
        let mapped = call(
            P_DISK0 + index,
            info(vifast_abi::FRAME_MAP, 0, false, 3),
            [P_VSPACE, VA_DISK + index * PAGE_SIZE as u64, FLAGS_RWU, 0],
        );
        if mapped[0] != 0 {
            exit(63)
        }
    }
    seed_disk();
    // 2) Magic/version/hình học thiết kế + `ready`.
    st(Stat::Magic, vifast_vfs::STATS_MAGIC);
    st(Stat::Version, vifast_vfs::STATS_VERSION);
    st(Stat::Capacity, CACHE_CAPACITY as u64);
    st(Stat::PageSize, PAGE_SIZE as u64);
    st(Stat::SectorSize, BLOCK_SIZE as u64);
    st(Stat::Clients, u64::from(CLIENTS));
    let (ready_info, ready_mr) = encode_ready();
    if nb_send(P_EP_REPLY, ready_info, ready_mr)[0] != 0 {
        exit(62)
    }
    put(b"U: pager up\n");

    let mut server = Server::new();
    loop {
        if server.done {
            break;
        }
        let mut handled = 0;
        while handled < 4 {
            let answer = nb_recv(P_EP_FAULT);
            if answer[0] != 0 {
                break;
            }
            note_received(answer[2]);
            let label = info_label(answer[2]) as u32;
            let length = info_length(answer[2]) as u8;
            let mr = [answer[3], answer[4], answer[5], answer[6]];
            match decode(label, length, &mr) {
                Some(Request::Fault {
                    client,
                    file,
                    page,
                    va,
                }) => {
                    let _ = METHOD_FAULT;
                    server.fault(client, file, page, va);
                }
                Some(Request::Stop) => {
                    let stats = server.core.stats();
                    let (finish_info, finish_mr) = encode_finish(
                        server.faults as u32,
                        stats.hits as u32,
                        stats.misses as u32,
                    );
                    let _ = nb_send(P_EP_REPLY, finish_info, finish_mr);
                    server.publish();
                    server.done = true;
                }
                _ => {}
            }
            handled += 1;
        }
        server.publish();
        yield_now();
    }
    // Linger: message `finish` vừa đẩy vào hàng Endpoint; `Exit` của sender finalise IPC và có
    // thể huỷ message còn trong hàng (D2-25) — nhường 4 lượt cho kernel NBRecv.
    let mut linger = 0u64;
    while linger < 4 {
        yield_now();
        linger += 1;
    }
    exit(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(99)
}

/// Giữ `result` module trong biên dịch (bố cục khối kết quả dùng chung với client/kernel).
const _RESULT_LAYOUT: u64 = result::DONE;
/// `file_pages` cũng là nguồn của pager (validate nằm trong `PagerCore::begin`).
const _FILES: u32 = vifast_vfs::FILE_COUNT;
