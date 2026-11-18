// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-70 — test host của **pager + page cache tầng 1**: hit/miss, LRU, backend RAM, stats.
//!
//! Viết **trước** phần cài (đỏ-trước D2-05): log `JOURNAL/kt70-red-host.log`.

use vifast_fs::{BlockDev, FsError, BLOCK_SIZE};
use vifast_vfs::{
    byte_at, file_pages, page_sector, Begin, DevRef, PageSink, PagerCore, RamDev, Stat, StatBlock,
    CACHE_CAPACITY, FILE0_PAGES, FILE1_PAGES, PAGE_SIZE, SECTORS_PER_PAGE,
};

/// Thiết bị RAM **một phiên bản** (mỗi test một `RamDev` mới) — không `unsafe`.
fn disk() -> RamDev {
    let dev = RamDev::new();
    dev.seed();
    dev
}

/// Sink ghi trang vào `Vec<u8>` (host) — U-mode cài bằng copy vào Frame.
#[derive(Default)]
struct VecSink {
    pages: [Vec<u8>; CACHE_CAPACITY],
}

impl PageSink for VecSink {
    fn write_sector(&mut self, slot: u8, sector_in_page: u32, data: &[u8; BLOCK_SIZE]) {
        let page = &mut self.pages[slot as usize];
        let off = sector_in_page as usize * BLOCK_SIZE;
        if page.len() < PAGE_SIZE {
            page.resize(PAGE_SIZE, 0);
        }
        page[off..off + BLOCK_SIZE].copy_from_slice(data);
    }
}

fn core() -> PagerCore<CACHE_CAPACITY, DevRef<'static>> {
    // `RamDev` có interior mutability (AtomicU64) nên `DevRef` chỉ cần `&'static`.
    static DEV: RamDev = RamDev::new();
    DEV.seed();
    PagerCore::new(DevRef(&DEV))
}

#[test]
fn hinh_hoc_trang_va_file_tu_nhat_quan() {
    // Trang = 8 sector 512 B; file 0 có 3 trang, file 1 có 2 trang (tổng 40 sector = 5 Frame).
    assert_eq!(PAGE_SIZE, 4096);
    assert_eq!(SECTORS_PER_PAGE, 8);
    assert_eq!(file_pages(0), Some(FILE0_PAGES));
    assert_eq!(file_pages(1), Some(FILE1_PAGES));
    assert_eq!(file_pages(2), None);
    assert_eq!(page_sector(0, 0), Some(0));
    assert_eq!(page_sector(0, 2), Some(16));
    assert_eq!(page_sector(1, 0), Some(24));
    assert_eq!(page_sector(0, 3), None);
    assert_eq!(page_sector(1, 2), None);
    // Mẫu dữ liệu phân biệt theo (file, page, offset) — cùng hàm cho cả pager lẫn client.
    assert_eq!(byte_at(0, 0, 0), 0);
    assert_ne!(byte_at(0, 1, 0), byte_at(1, 0, 0));
    assert_ne!(byte_at(0, 0, 1), byte_at(0, 1, 0));
}

#[test]
fn backend_ram_seed_va_doc_tung_sector() {
    let dev = disk();
    let mut sector = [0u8; BLOCK_SIZE];
    // File 0 trang 2 (sector 16..24), byte đầu sector 2 = byte_at(0,2,1024).
    BlockDev::read(&mut DevRef(&dev), 18, &mut sector).expect("đọc sector");
    assert_eq!(sector[0], byte_at(0, 2, 2 * BLOCK_SIZE));
    // File 1 trang 1 → sector 32..40 (sau 24 sector của file 0 + 8).
    BlockDev::read(&mut DevRef(&dev), 32, &mut sector).expect("đọc sector");
    assert_eq!(sector[7], byte_at(1, 1, 7));
}

#[test]
fn backend_ram_ghi_doc_lai_va_ngoai_dai_loi() {
    let dev = disk();
    let mut out = [0u8; BLOCK_SIZE];
    let mut data = [0u8; BLOCK_SIZE];
    data[5] = 0x5a;
    let mut writer = DevRef(&dev);
    writer.write(3, &data).expect("ghi sector");
    writer.read(3, &mut out).expect("đọc lại");
    assert_eq!(out[5], 0x5a);
    assert_eq!(writer.read(40, &mut out), Err(FsError::Invalid));
    assert_eq!(writer.write(40, &data), Err(FsError::Invalid));
    assert_eq!(DevRef::block_count(&writer), 40);
}

#[test]
fn backend_ram_byte_moi_offset_khop_mau() {
    let dev = disk();
    let mut sector = [0u8; BLOCK_SIZE];
    let mut r = DevRef(&dev);
    for page in 0..FILE1_PAGES {
        let lba = page_sector(1, page).expect("sector");
        for s in 0..SECTORS_PER_PAGE {
            BlockDev::read(&mut r, lba + s, &mut sector).expect("đọc");
            for (i, byte) in sector.iter().enumerate() {
                assert_eq!(
                    *byte,
                    byte_at(1, page, s as usize * BLOCK_SIZE + i),
                    "file=1 page={page} sector={s} byte={i}"
                );
            }
        }
    }
}

#[test]
fn hit_khong_doc_backend() {
    let mut core = core();
    let mut sink = VecSink::default();
    let first = core.begin(0, 2);
    assert!(
        matches!(first, vifast_vfs::Begin::Ready { .. }),
        "lần đầu phải miss"
    );
    if let vifast_vfs::Begin::Ready { slot } = first {
        core.load(slot, 0, 2, &mut sink).expect("nạp");
    }
    let reads_trước = core.stats().reads;
    let sectors_trước = core.stats().sectors;
    match core.begin(0, 2) {
        vifast_vfs::Begin::Hit { slot } => assert_eq!(slot, 0, "trang duy nhất nằm slot 0"),
        other => panic!("phải hit, nhận {other:?}"),
    }
    assert_eq!(core.stats().reads, reads_trước, "hit KHÔNG đọc backend");
    assert_eq!(core.stats().sectors, sectors_trước, "hit KHÔNG đọc sector");
    assert_eq!(core.stats().hits, 1);
    assert_eq!(core.stats().misses, 1);
}

#[test]
fn miss_evict_lru_khi_vuot_nguong() {
    let mut core = core();
    let mut sink = VecSink::default();
    let keys = [(0u32, 0u32), (0, 1), (0, 2)];
    for (i, (file, page)) in keys.iter().enumerate() {
        match core.begin(*file, *page) {
            vifast_vfs::Begin::Ready { slot } => {
                assert_eq!(slot, i as u8, "slot cấp theo thứ tự");
                core.load(slot, *file, *page, &mut sink).expect("nạp");
            }
            other => panic!("lần đầu phải Ready, nhận {other:?}"),
        }
    }
    // Chạm lại trang 1 để nó thành mới nhất (LRU thật, không phải FIFO).
    assert!(matches!(core.begin(0, 1), vifast_vfs::Begin::Hit { .. }));
    // Trang thứ 4 vượt ngưỡng: phải trục xuất trang 0 (cũ nhất), không phải trang 1.
    match core.begin(1, 0) {
        vifast_vfs::Begin::Evict { slot, key, .. } => {
            assert_eq!(key, (0, 0), "LRU: trang 0 cũ nhất");
            assert_eq!(slot, 0);
            core.load(slot, 1, 0, &mut sink).expect("nạp sau trục xuất");
        }
        other => panic!("phải Evict, nhận {other:?}"),
    }
    assert_eq!(core.stats().evictions, 1);
    assert_eq!(core.stats().misses, 4);
    assert_eq!(core.stats().reads, 4);
    assert_eq!(core.stats().sectors, u64::from(4 * SECTORS_PER_PAGE));
    // Trang bị trục xuất quay lại ⇒ miss lại và nạp đúng dữ liệu (không mất dữ liệu).
    match core.begin(0, 0) {
        vifast_vfs::Begin::Evict { key, .. } => assert_eq!(key, (0, 2), "cũ nhất còn lại"),
        other => panic!("phải Evict, nhận {other:?}"),
    }
}

#[test]
fn du_lieu_nap_dung_mau_backend() {
    let mut core = core();
    let mut sink = VecSink::default();
    for (file, page) in [(0u32, 2u32), (1, 1)] {
        let slot = match core.begin(file, page) {
            vifast_vfs::Begin::Ready { slot } => slot,
            other => panic!("phải Ready, nhận {other:?}"),
        };
        core.load(slot, file, page, &mut sink).expect("nạp");
        let page_data = &sink.pages[slot as usize];
        assert_eq!(page_data.len(), PAGE_SIZE);
        for (off, byte) in page_data.iter().enumerate() {
            assert_eq!(
                *byte,
                byte_at(file, page, off),
                "file={file} page={page} off={off}"
            );
        }
    }
}

#[test]
fn ngoai_file_tra_ma_va_khong_doc_backend() {
    let mut core = core();
    let mut sink = VecSink::default();
    let reads = core.stats().reads;
    match core.begin(0, 99) {
        vifast_vfs::Begin::Reject { status } => {
            assert_eq!(status.id(), vifast_vfs::err::INVALID_ARGUMENT.id());
            assert_eq!(status.name(), "InvalidArgument");
        }
        other => panic!("phải Reject, nhận {other:?}"),
    }
    assert!(matches!(core.begin(2, 0), vifast_vfs::Begin::Reject { .. }));
    assert_eq!(
        core.stats().reads,
        reads,
        "trang ngoài file KHÔNG đọc backend"
    );
    assert_eq!(core.stats().rejects, 2, "đếm đủ hai ca từ chối");
    // Cache không bị bẩn bởi lần từ chối.
    assert_eq!(core.cache_entries(), 0);
    let _ = &mut sink;
}

#[test]
fn mot_ban_trong_cache_cho_moi_khoa() {
    let mut core = core();
    let mut sink = VecSink::default();
    let mut slots = [0u8; 3];
    for (i, (file, page)) in [(0u32, 1u32), (0, 2), (1, 0)].iter().enumerate() {
        let slot = match core.begin(*file, *page) {
            vifast_vfs::Begin::Ready { slot } => slot,
            other => panic!("phải Ready, nhận {other:?}"),
        };
        core.load(slot, *file, *page, &mut sink).expect("nạp");
        slots[i] = slot;
    }
    // Cùng khoá lần hai ⇒ hit đúng slot cũ (một bản, nhiều mapping do caller quyết định).
    match core.begin(0, 2) {
        vifast_vfs::Begin::Hit { slot } => assert_eq!(slot, slots[1], "hit đúng slot của khoá"),
        other => panic!("phải Hit, nhận {other:?}"),
    }
    assert_eq!(core.cache_entries(), 3);
    assert!(core.keys_unique(), "không có hai bản cho cùng (file,page)");
}

#[test]
fn stats_block_ghi_doc_qua_bang_tu() {
    let mut block = StatBlock::new();
    block.set(Stat::Magic, vifast_vfs::STATS_MAGIC);
    block.set(Stat::Hits, 7);
    block.set(Stat::Misses, 3);
    assert_eq!(block.get(Stat::Magic), vifast_vfs::STATS_MAGIC);
    assert_eq!(block.get(Stat::Hits), 7);
    assert_eq!(block.get(Stat::Misses), 3);
    // Mọi ô nằm trong một trang 4 KiB và không chồng nhau.
    let mut seen = [false; PAGE_SIZE / 8];
    for stat in Stat::ALL {
        let off = stat.offset();
        assert!(off + 8 <= PAGE_SIZE, "stat {stat:?} ngoài trang");
        assert!(!seen[off / 8], "stat {stat:?} trùng ô");
        seen[off / 8] = true;
    }
    assert_eq!(Stat::ALL.len(), Stat::COUNT);
}

#[test]
fn stats_phan_anh_bo_dem_do_duoc() {
    let mut core = core();
    let mut sink = VecSink::default();
    for (file, page) in [(0u32, 0u32), (0, 1), (0, 0), (1, 0)].iter() {
        match core.begin(*file, *page) {
            vifast_vfs::Begin::Ready { slot } => {
                core.load(slot, *file, *page, &mut sink).expect("nạp")
            }
            vifast_vfs::Begin::Hit { .. } => {}
            other => panic!("không mong {other:?}"),
        }
    }
    let stats = core.stats();
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 3);
    assert_eq!(stats.reads, 3);
    assert_eq!(stats.sectors, u64::from(3 * SECTORS_PER_PAGE));
    assert_eq!(stats.evictions, 0);
    assert_eq!(stats.rejects, 0);
    // `reads == misses` là **hệ quả** của bất biến (mỗi miss một lần đọc trang) — D2-106.
    assert_eq!(stats.reads, stats.misses);
}

#[test]
fn ram_dev_khong_unsafe_va_kich_thuoc_thiet_ke() {
    // Bất biến thiết kế: cache 3 trang (ngưỡng cố định, không nới), đĩa 40 sector (5 Frame).
    assert_eq!(CACHE_CAPACITY, 3);
    assert_eq!(vifast_vfs::DISK_SECTORS, 40);
    assert_eq!(
        FILE0_PAGES * SECTORS_PER_PAGE + FILE1_PAGES * SECTORS_PER_PAGE,
        40
    );
}

// =============================================================================================
// KT-150 (N3) — checker "giao đúng một lần" + ca **4 client** (mức host trên `PagerCore`).
//
// Vì sao ở mức host: 4 client ở mức **kernel** đòi thêm VSpace/CSpace/VA base + bảng trang
// (`init/pager_common.rs` đang có `CLIENTS = 2`, `A_BASE`/`B_BASE`/`B_FILE1_BASE`/`B_PROBE_VA`)
// ⇒ đổi bố cục bộ nhớ của ảnh `ktest-user`. D2-130 mục (b) cấm đổi ABI/bố cục ⇒ làm ở host và
// ghi nợ "4 client mức kernel" trong `v2/agent/JOURNAL/kt150-n3.md`.
//
// Checker ở đây **fail-closed**: sổ rỗng / thiếu vế / đếm không được ⇒ `Err` (ĐỎ), không "bỏ qua".
// =============================================================================================

/// Số client của ca 4 client (host). Mức kernel vẫn 2 (`init/pager_common.rs:CLIENTS`).
const CLIENTS4: usize = 4;

/// Một message `pager.fault` — đúng 4 trường của `v2/lib/vifast-idl/idl/pager.vidl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fault {
    client: u8,
    file: u32,
    page: u32,
    va: u64,
}

/// Sổ ghi giao/nhận — **nguồn duy nhất** của checker (không suy từ bộ đếm của pager).
#[derive(Default)]
struct Ledger {
    /// Kernel **định** giao: mỗi phần tử = một fault theo bảng của client.
    intended: Vec<Fault>,
    /// Kernel **đã** giao: mỗi phần tử = một message `pager.fault` thật sự gửi đi.
    delivered: Vec<Fault>,
    /// Pager **đã** trả: mỗi phần tử = một message `pager.served` `(client, page, slot, status)`.
    served: Vec<(u8, u32, u8, u16)>,
}

/// Checker "giao đúng một lần" — **fail-closed**.
///
/// Đỏ khi: sổ rỗng (không đếm được) · số message giao ≠ số fault trong kế hoạch · số message
/// pager trả ≠ số message đã giao (xử lý lặp) · **một địa chỉ** được giao khác số lần kế hoạch ·
/// có message ngoài kế hoạch.
fn check_delivered_once(ledger: &Ledger) -> Result<(), String> {
    if ledger.intended.is_empty() {
        return Err("kế hoạch rỗng — không đếm được".to_string());
    }
    if ledger.delivered.is_empty() {
        return Err("không giao message nào — không đếm được".to_string());
    }
    if ledger.served.is_empty() {
        return Err("pager không trả message nào — không đếm được".to_string());
    }
    if ledger.delivered.len() != ledger.intended.len() {
        return Err(format!(
            "giao {} message cho {} fault — phải bằng nhau",
            ledger.delivered.len(),
            ledger.intended.len()
        ));
    }
    if ledger.served.len() != ledger.delivered.len() {
        return Err(format!(
            "pager trả {} message cho {} message đã giao — mỗi request đúng 1 lần",
            ledger.served.len(),
            ledger.delivered.len()
        ));
    }
    // Đếm **theo từng địa chỉ**: mỗi `(client, va)` đúng số lần kế hoạch (1 với fault thường,
    // 2 với trang nạp lại sau trục xuất — số lấy từ bảng, không viết tay).
    for want in &ledger.intended {
        let planned = ledger.intended.iter().filter(|f| *f == want).count();
        let got = ledger.delivered.iter().filter(|f| *f == want).count();
        if got != planned {
            return Err(format!(
                "addr=0x{:x} client={} giao {got} lần, kế hoạch {planned} lần",
                want.va, want.client
            ));
        }
    }
    for got in &ledger.delivered {
        if !ledger.intended.contains(got) {
            return Err(format!("giao message ngoài kế hoạch: {got:?}"));
        }
    }
    Ok(())
}

/// Pager + kernel giả lập **đúng giao thức** `pager.vidl` (`fault` → `served`), có sổ ghi.
struct PagerSession<const N: usize> {
    core: PagerCore<N, DevRef<'static>>,
    sink: VecSink,
    ledger: Ledger,
    /// Đột biến "phát lại Call khi Resume": số lần giao **thêm** cho mỗi fault (0 = không).
    replay_on_resume: u8,
    /// VA đã map cho từng `(slot, client)` — như `Server::mapped` của pager U-mode.
    mapped: [[Option<u64>; CLIENTS4]; N],
    /// Số lần một fault được phục vụ bằng Frame đã map cho client **khác**.
    shared: u64,
    /// Số byte dữ liệu sai khi đối chiếu `byte_at` **tại lúc phục vụ**.
    data_bad: u64,
    /// Bitmask client đã được phục vụ ít nhất một lần.
    clients_seen: u8,
}

impl<const N: usize> PagerSession<N> {
    fn new() -> Self {
        static DEV: RamDev = RamDev::new();
        DEV.seed();
        Self {
            core: PagerCore::new(DevRef(&DEV)),
            sink: VecSink::default(),
            ledger: Ledger::default(),
            replay_on_resume: 0,
            mapped: [[None; CLIENTS4]; N],
            shared: 0,
            data_bad: 0,
            clients_seen: 0,
        }
    }

    /// Kernel giao một fault: ghi kế hoạch, gửi message, (đột biến) phát lại khi Resume.
    fn fault(&mut self, client: u8, file: u32, page: u32, va: u64) -> u16 {
        let request = Fault {
            client,
            file,
            page,
            va,
        };
        self.ledger.intended.push(request);
        let status = self.send(request);
        let mut replay = 0u8;
        while replay < self.replay_on_resume {
            let _ = self.send(request);
            replay += 1;
        }
        status
    }

    /// Một lần **gửi** message `pager.fault` rồi nhận `pager.served`.
    fn send(&mut self, request: Fault) -> u16 {
        self.ledger.delivered.push(request);
        let (slot, status) = self.serve(request);
        self.ledger
            .served
            .push((request.client, request.page, slot, status));
        status
    }

    /// Pager xử lý **một** request: `begin` → (nạp) → map → `served`.
    fn serve(&mut self, request: Fault) -> (u8, u16) {
        match self.core.begin(request.file, request.page) {
            Begin::Reject { status } => (0, status.id()),
            Begin::Hit { slot } => {
                if self.mapped_elsewhere(slot, request.client) {
                    self.shared += 1;
                }
                self.map_into(slot, request);
                self.validate(slot, request);
                (slot, 0)
            }
            Begin::Ready { slot } => {
                self.core
                    .load(slot, request.file, request.page, &mut self.sink)
                    .expect("nạp trang");
                self.map_into(slot, request);
                self.validate(slot, request);
                (slot, 0)
            }
            Begin::Evict { slot, .. } => {
                // Gỡ mapping cũ **trước** khi ghi đè Frame (client khác không thấy dữ liệu mới
                // qua mapping cũ) — đúng thứ tự của `pager_server_user.rs`.
                self.unmap_all(slot);
                self.core
                    .load(slot, request.file, request.page, &mut self.sink)
                    .expect("nạp trang");
                self.map_into(slot, request);
                self.validate(slot, request);
                (slot, 0)
            }
        }
    }

    fn mapped_elsewhere(&self, slot: u8, client: u8) -> bool {
        let mut other = 0usize;
        while other < CLIENTS4 {
            if other != client as usize && self.mapped[slot as usize][other].is_some() {
                return true;
            }
            other += 1;
        }
        false
    }

    fn map_into(&mut self, slot: u8, request: Fault) {
        self.mapped[slot as usize][request.client as usize] = Some(request.va);
        self.clients_seen |= 1 << request.client;
    }

    fn unmap_all(&mut self, slot: u8) {
        let mut client = 0usize;
        while client < CLIENTS4 {
            self.mapped[slot as usize][client] = None;
            client += 1;
        }
    }

    /// Đối chiếu **toàn bộ** trang trong Frame với mẫu `byte_at` — ngay tại lúc phục vụ.
    fn validate(&mut self, slot: u8, request: Fault) {
        let page = &self.sink.pages[slot as usize];
        if page.len() != PAGE_SIZE {
            self.data_bad += PAGE_SIZE as u64;
            return;
        }
        for (offset, byte) in page.iter().enumerate() {
            if *byte != byte_at(request.file, request.page, offset) {
                self.data_bad += 1;
            }
        }
    }

    fn clients_seen(&self) -> u8 {
        self.clients_seen.count_ones() as u8
    }
}

/// Bảng của 4 client (host): làm đầy cache 3, hit chéo client, trục xuất LRU, nạp lại, từ chối.
const TABLES4: [&[(u32, u32)]; CLIENTS4] = [
    &[(0, 0), (0, 1), (0, 2)],
    &[(0, 2), (1, 0), (1, 1)],
    &[(0, 0), (0, 2), (1, 0)],
    &[(0, 2), (1, 0), (0, 99)],
];

/// VA của một trang trong bảng client `client` (4 vùng rời nhau, không đè ảnh ELF).
fn va4(client: usize, file: u32, page: u32) -> u64 {
    0x4010_0000
        + client as u64 * 0x0008_0000
        + u64::from(file) * 0x0004_0000
        + u64::from(page) * 4096
}

#[test]
fn bon_client_giao_dung_mot_lan_va_chia_se_frame() {
    let mut session = PagerSession::<CACHE_CAPACITY>::new();
    let mut faults = 0u64;
    let mut rejected = 0u64;
    for (client, table) in TABLES4.iter().enumerate() {
        for (file, page) in table.iter() {
            let status = session.fault(client as u8, *file, *page, va4(client, *file, *page));
            faults += 1;
            if *page == 99 {
                assert_eq!(
                    status,
                    vifast_vfs::err::INVALID_ARGUMENT.id(),
                    "trang ngoài file phải bị từ chối"
                );
                rejected += 1;
            } else {
                assert_eq!(status, 0, "client={client} file={file} page={page}");
            }
        }
    }
    // (a) checker "giao đúng một lần" — fail-closed, không "bỏ qua".
    check_delivered_once(&session.ledger).expect("mỗi fault phải được giao đúng một lần");
    assert_eq!(session.ledger.delivered.len() as u64, faults);
    assert_eq!(session.ledger.served.len() as u64, faults);
    // (b) ca `clients=4`: bốn client **thật sự** được phục vụ, không phải khai báo.
    assert_eq!(
        session.clients_seen(),
        CLIENTS4 as u8,
        "đủ 4 client được phục vụ"
    );
    assert_eq!(
        session.shared, 3,
        "ba lần hit chéo client (một Frame, nhiều mapping)"
    );
    assert_eq!(session.data_bad, 0, "không byte nào sai so với mẫu backend");
    let stats = session.core.stats();
    assert_eq!(stats.rejects, rejected, "đếm đủ ca từ chối");
    assert_eq!(stats.evictions, 5, "trục xuất LRU thật khi vượt ngưỡng 3");
    assert_eq!(stats.reads, stats.misses, "mỗi miss đúng một lần đọc trang");
    assert!(stats.entries <= CACHE_CAPACITY, "cache không vượt ngưỡng");
    assert_eq!(
        stats.entries as u64,
        stats.misses - stats.evictions,
        "entries = miss - evict"
    );
    assert!(session.core.keys_unique(), "một bản cho mỗi (file,page)");
}

#[test]
fn ca_am_phat_lai_call_khi_resume_thi_checker_do() {
    // Ca âm **của chính harness** (luật N4/T-IPC): checker phải bắt được lỗi trước khi dùng nó
    // để tuyên bố. Đột biến "phát lại Call khi Resume" ⇒ cùng một fault bị giao **hai** lần.
    let mut session = PagerSession::<CACHE_CAPACITY>::new();
    session.replay_on_resume = 1;
    let _ = session.fault(0, 0, 0, va4(0, 0, 0));
    let error = check_delivered_once(&session.ledger).expect_err("phát lại ⇒ checker phải ĐỎ");
    assert!(
        error.contains("giao 2 message cho 1 fault"),
        "lý do đỏ phải nêu số message: {error}"
    );
    // Đối chứng: cùng một fault, KHÔNG phát lại ⇒ xanh.
    let mut clean = PagerSession::<CACHE_CAPACITY>::new();
    let _ = clean.fault(0, 0, 0, va4(0, 0, 0));
    check_delivered_once(&clean.ledger).expect("không phát lại ⇒ xanh");
}

#[test]
fn checker_fail_closed_khi_so_rong() {
    // Thiếu dòng / không đếm được ⇒ ĐỎ, không được coi là "bỏ qua".
    let empty = Ledger::default();
    assert!(check_delivered_once(&empty).is_err(), "sổ rỗng phải ĐỎ");
    let mut only_intended = Ledger::default();
    only_intended.intended.push(Fault {
        client: 0,
        file: 0,
        page: 0,
        va: 0x4010_0000,
    });
    assert!(
        check_delivered_once(&only_intended).is_err(),
        "có kế hoạch mà không giao được message nào phải ĐỎ"
    );
}
