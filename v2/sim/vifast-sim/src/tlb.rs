// SPDX-License-Identifier: MIT OR Apache-2.0
//! TLB **theo hart** của K2 — model-only `[TK]` (KT-40; K2-PLAN §3 KT-40 + §2 Q4/Q5; D2-65
//! Q4/Q5; `v2/docs/k2-vm.md` §14; thiết kế §4.10 `:353-366`, §9.5 `:799`).
//!
//! **K2 không phải kernel thật**: đây là **mô hình** TLB trong sim — không `sfence.vma`, không
//! IPI/TLB phần cứng, không MMU. "IPI" là một [`TlbFlush`] trong hộp thư mô hình; "ack" là bộ
//! đếm thế hệ (`delivered`/`acked`) trong mô hình, **không** atomic/thread (D2-49(f)).
//!
//! # Hình của TLB
//!
//! - Mỗi hart một [`HartTlb`] với **đúng** [`TLB_ENTRIES_PER_HART`] = **16** entry (K2-PLAN
//!   §3 KT-40). Entry khoá bằng [`TlbKey`] = `(generation, asid, va)` — `(asid, va)` là danh
//!   tính của một translation, còn `generation` là **thế hệ ASID** (Q4): khi cạn ASID,
//!   `generation += 1` và **mọi** entry thế hệ cũ thành vô hiệu (flush theo `gen`).
//! - `va` của khoá là **địa chỉ gốc** của mapping (`Mapping::va`), không phải VA tra cứu: một
//!   entry 2 MiB phủ cả 2 MiB, nên tra cứu/`flush` theo một VA bất kỳ **trong** khoảng đều phải
//!   khớp/khớp-phủ (Q4: "phải phủ cả entry lớn chứa `va`"). Xem [`TlbEntry::covers`].
//! - Thay trang: **LRU tất định theo chỉ số** (K2-PLAN: "16 entry, LRU tất định theo chỉ số") —
//!   chọn entry có `used` nhỏ nhất, hoà thì chỉ số nhỏ nhất; không `HashMap`, không thời gian
//!   thật, cùng dãy thao tác ⇒ cùng kết quả.
//! - `hit`/`miss`/`fill`/`evict`/`flush`/`invalidated` đếm được theo hart ([`TlbHartInfo`]).
//!
//! # Ack theo thế hệ (D2-60, Q5)
//!
//! Bộ đếm `delivered` = **số `TlbFlush` đã giao thành công** vào hộp thư của hart (`= số send
//! Ok`); `acked` = **`fetch_max`** của các `ack` đã xử lý. Vì hộp thư là FIFO và `ack` được
//! cấp tăng dần, `acked` là **tiền tố liên tục**: `delivered - acked` = số `TlbFlush` còn trong
//! hộp thư ([`crate::kobj::Machine::check_mailboxes`] kiểm đúng đẳng thức này). `acked ≠
//! delivered`: đã giao **không** có nghĩa đã ack — chỉ `acked == delivered` là xong.
//!
//! Kiểu này **không** dùng `std::sync`/atomic/unsafe: mô hình một luồng nên `fetch_max` là
//! `if seq > acked { acked = seq }` (đơn điệu, **không lùi**), biên dịch được dưới Miri.

use crate::pagetable::{MapAttrs, MapRights, Mapping};
use crate::vspace::HartMask;

/// Số entry TLB của **mỗi hart**: **16** — `[TK]`, K2-PLAN §3 KT-40 (đổi là đổi ngưỡng mô
/// hình, phải qua Agent 0).
pub const TLB_ENTRIES_PER_HART: usize = 16;

/// Khoá của một entry: `(generation, asid, va)`.
///
/// `generation` là **thế hệ ASID** của VSpace lúc nạp (Q4); `asid` là ASID trong thế hệ đó;
/// `va` là địa chỉ **gốc** của mapping (đã căn `1 << size_bits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TlbKey {
    /// Thế hệ ASID (của VSpace lúc nạp entry).
    pub generation: u64,
    /// ASID trong thế hệ.
    pub asid: u16,
    /// VA gốc của mapping.
    pub va: u64,
}

/// Một entry TLB: translation đã cache + thông tin để kiểm TLB-safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlbEntry {
    /// Khoá `(generation, asid, va)`.
    pub key: TlbKey,
    /// PA của mô hình (= `FrameMeta.pa` lúc nạp).
    pub pa: u64,
    /// `12` / `21` / `30` — cỡ lá.
    pub size_bits: u8,
    /// Frame nguồn (để oracle đối chiếu cây).
    pub frame: u32,
    /// Quyền **của mapping** lúc nạp (không suy từ cap).
    pub rights: MapRights,
    /// Attrs **của mapping** lúc nạp.
    pub attrs: MapAttrs,
    /// Dấu LRU (số thứ tự chạm cuối; tăng đơn điệu).
    used: u64,
}

impl TlbEntry {
    /// VA kết thúc (không gồm) của khoảng entry phủ.
    pub const fn end_va(self) -> u64 {
        self.key.va + (1u64 << self.size_bits)
    }

    /// Entry có phủ `va` không: `key.va <= va < end_va` (VA tra cứu **trong** khoảng).
    pub const fn covers(self, va: u64) -> bool {
        va >= self.key.va && va < self.end_va()
    }

    /// Ảnh chụp translation trả cho người gọi.
    pub const fn translation(self, from_tlb: bool) -> TlbTranslation {
        TlbTranslation {
            va: self.key.va,
            pa: self.pa,
            size_bits: self.size_bits,
            frame: self.frame,
            rights: self.rights,
            attrs: self.attrs,
            from_tlb,
        }
    }
}

/// Phạm vi của một lệnh flush (mô hình; ba mức của Q4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlbFlushScope {
    /// **Một trang**: mọi entry của `(generation, asid)` phủ `va` (kể cả entry lớn chứa `va`).
    Page {
        /// VA nằm trong khoảng entry cần xoá.
        va: u64,
    },
    /// **Một ASID**: mọi entry của `(generation, asid)` (teardown VSpace).
    Asid,
    /// **Một thế hệ**: mọi entry có `generation` **cũ hơn** mốc (cạn ASID — Q4: "flush toàn bộ
    /// một lần theo `gen`").
    Generation,
}

impl TlbFlushScope {
    /// Tên để in log/assert.
    pub const fn name(self) -> &'static str {
        match self {
            TlbFlushScope::Page { .. } => "Page",
            TlbFlushScope::Asid => "Asid",
            TlbFlushScope::Generation => "Generation",
        }
    }
}

/// Message `TlbFlush` — **kind duy nhất** KT-40 thêm vào hộp thư mô hình (D2-65 Q5: nới
/// D2-59; `VmKick`/`PowerReq` vẫn để K3).
///
/// Thay mặt cho "IPI + handler xả TLB" trên phần cứng thật. `ack` do **kernel** gán khi giao
/// message (= `delivered` của hart đích sau khi tăng); người gọi `post_mail` **không** tự đặt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlbFlush {
    /// Số hiệu shootdown toàn cục (1-based, tăng đơn điệu) — để log/đối chiếu.
    pub seq: u64,
    /// Thế hệ ack **của hart đích** (= số `TlbFlush` đã giao cho hart này, 1-based) — do kernel
    /// gán; `ack(seq) = fetch_max`.
    pub ack: u64,
    /// Thế hệ ASID của VSpace lúc phát.
    pub generation: u64,
    /// ASID của VSpace lúc phát.
    pub asid: u16,
    /// VA gốc của mapping bị đổi PTE (xem [`TlbFlushScope::Page`]).
    pub va: u64,
}

/// Ảnh chụp một translation trả từ TLB/walk (test + harness).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlbTranslation {
    /// VA gốc của mapping.
    pub va: u64,
    /// PA của mô hình.
    pub pa: u64,
    /// Cỡ lá (`12` / `21` / `30`).
    pub size_bits: u8,
    /// Frame nguồn.
    pub frame: u32,
    /// Quyền của mapping.
    pub rights: MapRights,
    /// Attrs của mapping.
    pub attrs: MapAttrs,
    /// `true` ⇔ lấy từ TLB (hit); `false` ⇔ miss rồi walk + nạp.
    pub from_tlb: bool,
}

/// Ảnh chụp một TLB hart (test/đo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TlbHartInfo {
    /// Hart.
    pub hart: u16,
    /// Sức chứa ([`TLB_ENTRIES_PER_HART`]).
    pub capacity: usize,
    /// Số entry đang sống.
    pub entries: usize,
    /// Số lần tra cứu trúng.
    pub hits: u64,
    /// Số lần tra cứu trượt.
    pub misses: u64,
    /// Số lần nạp entry từ walk.
    pub fills: u64,
    /// Số entry bị đuổi vì đầy (LRU).
    pub evictions: u64,
    /// Số lệnh flush đã áp lên hart này.
    pub flushes: u64,
    /// Tổng số entry bị vô hiệu bởi flush/clear.
    pub invalidated: u64,
    /// Số `TlbFlush` đã giao vào hộp thư hart này (`= số send Ok`).
    pub delivered: u64,
    /// `fetch_max` của các `ack` đã xử lý (`acked <= delivered`).
    pub acked: u64,
}

/// Ảnh chụp một shootdown (đang chờ hoặc đã xong) — nguồn của bất biến
/// `Ok ⇒ acked == targets`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlbShootdownInfo {
    /// Số hiệu shootdown toàn cục.
    pub seq: u64,
    /// Thế hệ ASID của VSpace lúc phát.
    pub generation: u64,
    /// ASID của VSpace lúc phát.
    pub asid: u16,
    /// VA gốc bị đổi PTE.
    pub va: u64,
    /// Mask hart **phải** ack (`= active_on` lúc phát).
    pub targets: HartMask,
    /// Mask hart **đã** ack (tính từ `acked` của từng hart).
    pub acked: HartMask,
    /// Số `TlbFlush` đã giao cho các hart đích (**trừ** hart gọi — nó flush cục bộ + ack ngầm).
    pub messages: u32,
    /// Số vòng của vòng chờ (chẩn đoán deadlock).
    pub rounds: u64,
    /// `true` ⇔ ack đủ (`acked == targets`) — chỉ khi đó `unmap`/`protect` mới trả `Ok`.
    pub completed: bool,
}

/// TLB của **một** hart.
#[derive(Debug)]
pub(crate) struct HartTlb {
    entries: Vec<TlbEntry>,
    tick: u64,
    pub(crate) hits: u64,
    pub(crate) misses: u64,
    pub(crate) fills: u64,
    pub(crate) evictions: u64,
    pub(crate) flushes: u64,
    pub(crate) invalidated: u64,
    /// Số `TlbFlush` đã giao thành công vào hộp thư của hart này.
    pub(crate) delivered: u64,
    /// `fetch_max` của các `ack` đã xử lý (không bao giờ lùi).
    pub(crate) acked: u64,
}

impl HartTlb {
    /// TLB rỗng của một hart.
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::with_capacity(TLB_ENTRIES_PER_HART),
            tick: 0,
            hits: 0,
            misses: 0,
            fills: 0,
            evictions: 0,
            flushes: 0,
            invalidated: 0,
            delivered: 0,
            acked: 0,
        }
    }

    /// Số entry đang sống.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Duyệt entry (oracle) — không đổi LRU.
    pub(crate) fn entries(&self) -> impl Iterator<Item = &TlbEntry> {
        self.entries.iter()
    }

    /// Ảnh chụp cho test/đo.
    pub(crate) fn info(&self, hart: u16) -> TlbHartInfo {
        TlbHartInfo {
            hart,
            capacity: TLB_ENTRIES_PER_HART,
            entries: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
            fills: self.fills,
            evictions: self.evictions,
            flushes: self.flushes,
            invalidated: self.invalidated,
            delivered: self.delivered,
            acked: self.acked,
        }
    }

    fn find_index(&self, generation: u64, asid: u16, va: u64) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.key.generation == generation && e.key.asid == asid && e.covers(va))
    }

    /// Tra cứu **không** đổi LRU/không đếm (dùng cho test/oracle/oracle `tlb_lookup`).
    pub(crate) fn peek(&self, generation: u64, asid: u16, va: u64) -> Option<TlbEntry> {
        self.find_index(generation, asid, va)
            .map(|i| self.entries[i])
    }

    /// Tra cứu của đường dịch: cập nhật LRU + đếm `hit`/`miss`.
    pub(crate) fn lookup(&mut self, generation: u64, asid: u16, va: u64) -> Option<TlbEntry> {
        match self.find_index(generation, asid, va) {
            Some(i) => {
                self.hits += 1;
                self.tick += 1;
                let used = self.tick;
                let entry = &mut self.entries[i];
                entry.used = used;
                Some(*entry)
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    /// Nạp translation vừa walk vào TLB với khoá `(generation, asid)` của VSpace đang nạp;
    /// đầy ⇒ đuổi entry **LRU** (hoà: chỉ số nhỏ nhất).
    pub(crate) fn fill_keyed(&mut self, generation: u64, asid: u16, m: Mapping) -> TlbEntry {
        let entry = TlbEntry {
            key: TlbKey {
                generation,
                asid,
                va: m.va,
            },
            pa: m.pa,
            size_bits: m.size_bits,
            frame: m.frame,
            rights: m.rights,
            attrs: m.attrs,
            used: 0,
        };
        self.insert(entry)
    }

    fn insert(&mut self, mut entry: TlbEntry) -> TlbEntry {
        self.fills += 1;
        self.tick += 1;
        entry.used = self.tick;
        if self.entries.len() < TLB_ENTRIES_PER_HART {
            self.entries.push(entry);
            return entry;
        }
        // Đầy: chọn LRU (used nhỏ nhất; hoà ⇒ chỉ số nhỏ nhất) — tất định.
        let mut victim = 0usize;
        for (i, e) in self.entries.iter().enumerate().skip(1) {
            if e.used < self.entries[victim].used {
                victim = i;
            }
        }
        self.entries[victim] = entry;
        self.evictions += 1;
        entry
    }

    /// Áp một lệnh flush (Q4) — trả số entry bị vô hiệu.
    pub(crate) fn flush(&mut self, generation: u64, asid: u16, scope: TlbFlushScope) -> usize {
        self.flushes += 1;
        let truoc = self.entries.len();
        match scope {
            TlbFlushScope::Page { va } => self.entries.retain(|e| {
                !(e.key.generation == generation && e.key.asid == asid && e.covers(va))
            }),
            TlbFlushScope::Asid => self
                .entries
                .retain(|e| !(e.key.generation == generation && e.key.asid == asid)),
            TlbFlushScope::Generation => self.entries.retain(|e| e.key.generation >= generation),
        }
        let n = truoc - self.entries.len();
        self.invalidated += n as u64;
        n
    }

    /// Xoá **toàn bộ** entry (hart offline — mô hình không giữ TLB của hart đã tắt).
    pub(crate) fn clear(&mut self) -> usize {
        self.flushes += 1;
        let n = self.entries.len();
        self.entries.clear();
        self.invalidated += n as u64;
        n
    }

    /// Giao một `TlbFlush`: tăng `delivered`, trả **thế hệ ack** vừa cấp.
    pub(crate) fn deliver(&mut self) -> u64 {
        self.delivered += 1;
        self.delivered
    }

    /// Ack tới thế hệ `seq` — `fetch_max` (D2-60): trả số ack **mới** ghi nhận, **không lùi**
    /// (`ack(3)` rồi `ack(1)` ⇒ vẫn 3).
    pub(crate) fn ack(&mut self, seq: u64) -> u64 {
        let prev = self.acked;
        if seq > self.acked {
            self.acked = seq;
        }
        self.acked - prev
    }

    /// **test-only** — gieo một entry "không thể sinh qua public op" để chứng minh oracle
    /// [`crate::kobj::Machine::check_tlb`] **không rỗng** (cùng mẫu KT-36/KT-39: gieo trạng thái
    /// hỏng rồi bắt checker trả lỗi).
    #[cfg(test)]
    pub(crate) fn seed_entry_for_test(&mut self, entry: TlbEntry) {
        self.entries.push(entry);
        self.fills += 1;
    }

    /// **test-only** — xem/sửa thẳng hàng entry để dựng/khôi phục trạng thái gieo hỏng.
    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut Vec<TlbEntry> {
        &mut self.entries
    }

    /// Bất biến rẻ: không vượt sức chứa, `acked <= delivered`.
    pub(crate) fn invariant(&self) -> bool {
        self.entries.len() <= TLB_ENTRIES_PER_HART && self.acked <= self.delivered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pagetable::Mapping;

    fn m(va: u64, size_bits: u8) -> Mapping {
        Mapping {
            va,
            pa: 0x1000 * va.max(1),
            size_bits,
            frame: 1,
            rights: MapRights::READ | MapRights::WRITE,
            attrs: MapAttrs::WB,
        }
    }

    #[test]
    fn lru_tat_dinh_va_du_16_entry() {
        assert_eq!(TLB_ENTRIES_PER_HART, 16);
        let mut t = HartTlb::new();
        for i in 0..16u64 {
            t.fill_keyed(0, 0, m(0x1000 * (i + 1), 12));
        }
        assert_eq!(t.len(), 16);
        assert_eq!(t.fills, 16);
        assert_eq!(t.evictions, 0);
        // Chạm trang 0 ⇒ MRU. Nạp trang 17 ⇒ đuổi trang 1 (LRU cũ nhất), không đuổi trang 0.
        assert!(t.lookup(0, 0, 0x1000).is_some());
        t.fill_keyed(0, 0, m(0x1000 * 17, 12));
        assert_eq!(t.len(), 16);
        assert_eq!(t.evictions, 1);
        assert!(
            t.peek(0, 0, 0x1000).is_some(),
            "LRU không được đuổi trang vừa chạm"
        );
        assert!(
            t.peek(0, 0, 0x2000).is_none(),
            "LRU phải đuổi entry cũ nhất (chỉ số nhỏ nhất khi hoà)"
        );
        assert!(t.peek(0, 0, 0x11000).is_some());
        assert!(t.invariant());
    }

    #[test]
    fn hit_miss_cong_don_va_peek_khong_doi_lru() {
        let mut t = HartTlb::new();
        assert!(t.lookup(0, 7, 0x4000).is_none());
        assert_eq!((t.misses, t.hits), (1, 0));
        t.fill_keyed(0, 7, m(0x4000, 12));
        assert!(t.lookup(0, 7, 0x4fff).is_some(), "VA trong khoảng 4 KiB");
        assert_eq!((t.misses, t.hits), (1, 1));
        // Sai asid/gen ⇒ miss (không dùng nhầm translation).
        assert!(t.lookup(0, 8, 0x4000).is_none());
        assert!(t.lookup(1, 7, 0x4000).is_none());
        let truoc = t.len();
        let _ = t.peek(0, 7, 0x4000);
        assert_eq!(t.len(), truoc, "peek không đổi bảng");
        assert!(t.invariant());
    }

    #[test]
    fn flush_page_phu_entry_lon_asid_va_generation() {
        let mut t = HartTlb::new();
        // Entry 2 MiB ở 2 MiB (cấp 1) + hai entry 4 KiB của hai ASID.
        t.fill_keyed(0, 0, m(1 << 21, 21));
        t.fill_keyed(0, 0, m(0x1000, 12));
        t.fill_keyed(0, 1, m(0x2000, 12));
        // Flush theo trang **bên trong** entry lớn ⇒ phải xoá entry lớn (Q4).
        assert_eq!(
            t.flush(
                0,
                0,
                TlbFlushScope::Page {
                    va: (1 << 21) + 0x1000
                }
            ),
            1
        );
        assert!(t.peek(0, 0, 1 << 21).is_none(), "entry lớn phủ va phải mất");
        assert!(t.peek(0, 0, 0x1000).is_some());
        assert!(t.peek(0, 1, 0x2000).is_some());
        // Flush ASID chỉ đụng đúng `(gen, asid)`.
        assert_eq!(t.flush(0, 0, TlbFlushScope::Asid), 1);
        assert!(t.peek(0, 0, 0x1000).is_none());
        assert!(t.peek(0, 1, 0x2000).is_some());
        // Generation: entry thế hệ **cũ hơn** mất, entry thế hệ hiện tại còn.
        t.fill_keyed(1, 2, m(0x3000, 12));
        assert_eq!(t.flush(1, 0, TlbFlushScope::Generation), 1);
        assert!(t.peek(0, 1, 0x2000).is_none(), "thế hệ 0 < 1 ⇒ mất");
        assert!(t.peek(1, 2, 0x3000).is_some(), "thế hệ 1 còn");
        assert_eq!(t.flushes, 3);
        assert_eq!(t.invalidated, 3);
        assert!(t.invariant());
    }

    #[test]
    fn ack_fetch_max_khong_lui_va_delivered_khac_acked() {
        let mut t = HartTlb::new();
        assert_eq!(t.deliver(), 1);
        assert_eq!(t.deliver(), 2);
        assert_eq!(t.deliver(), 3);
        assert_eq!((t.delivered, t.acked), (3, 0), "giao chưa phải ack");
        assert_eq!(t.ack(2), 2);
        assert_eq!((t.delivered, t.acked), (3, 2));
        assert_eq!(t.ack(1), 0, "ack cũ không lùi (fetch_max)");
        assert_eq!(t.acked, 2);
        assert_eq!(t.ack(3), 1);
        assert_eq!(t.acked, 3);
        assert!(t.invariant());
    }

    #[test]
    fn clear_xoa_het_va_dem_invalidated() {
        let mut t = HartTlb::new();
        t.fill_keyed(0, 0, m(0x1000, 12));
        t.fill_keyed(0, 1, m(0x2000, 12));
        assert_eq!(t.clear(), 2);
        assert_eq!(t.len(), 0);
        assert!(t.invariant());
    }
}
