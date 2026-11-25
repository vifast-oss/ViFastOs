// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Dấu chân **tĩnh** của [`Machine`] — D2-84 pin core ≤ **196.608 B** (192 KiB) = trần ảnh
//! 262.144 B trừ 64 KiB headroom; vượt pin ⇒ **tối ưu** (giảm bảng/nhật ký), **không** nâng
//! pin.
//!
//! KT-59 dựng bảng 14 dòng; KT-60 thêm body Endpoint/Reply/Notification + pool message đang
//! chờ + bảng WaitOnAddress + biến thể `IpcArea` trong TCB; KT-63 thêm **receive window**
//! (`TcbData.recv_window`, 4 CPTR) + cap nguồn/ảnh chụp trong `Message` + 3 bộ đếm cap;
//! KT-64 thêm `slot_mem` + `harness_region` (thay `slot_pool`) + `tlb_pending[8]`; KT-312 thu hẹp
//! kiểu ba trường chỉ số của [`crate::objmeta::Meta`] (bảng **không** đổi số dòng). Hàm [`Machine::footprint`] trả
//! **phân bố byte** để test pin và KT-61 đối chiếu trước khi link; bất biến
//! `sum(bytes) + scalars + padding == size_of::<Machine>()` được test pin kiểm.

use core::mem::size_of;

use crate::machine::{
    Machine, MAX_CAPS, MAX_ENDPOINTS, MAX_HARTS, MAX_NOTIFICATIONS, MAX_OBJECTS,
    MAX_QUEUED_MESSAGES, MAX_REPLIES, MAX_SCHED_CONTEXTS, MAX_TCBS, MAX_UNTYPEDS,
};
use crate::objstate::{
    AddrWord, EndpointData, QueuedMsg, ReplyData, WaitNode, MAX_ADDR_WORDS, MAX_WAIT_NODES,
    WAITADDR_BUCKETS,
};
use crate::observer::{CoreObserver, TestObserver};
use crate::store::StaticStore;

/// Số thành phần của báo cáo [`Footprint`] (26 bảng/mảng; scalar + padding tính riêng).
///
/// **KT-405**: 29 → 26 — bốn bảng rời `sched[8]`/`mail[8]`/`published[8]`/`tlb_pending[8]` gộp
/// thành **một** `perhart[8]` ([`crate::PerHartState`]); ba bảng oracle vẫn là ba phần
/// (`obs.*`, `[0; 3]` với `NoopObserver`).
pub const FOOTPRINT_PARTS: usize = 26;

/// Phân bố byte **tĩnh** của [`Machine`] (D2-84). `bytes[i]` là `size_of::<[T; N]>()` của
/// từng bảng; `scalars` là tổng các trường đếm/cờ; `padding` là byte đệm/đuôi `repr(Rust)`:
/// `sum(bytes) + scalars + padding == total == size_of::<Machine>()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footprint {
    /// Tên 29 thành phần (cùng thứ tự `bytes`).
    pub names: [&'static str; FOOTPRINT_PARTS],
    /// Byte của từng thành phần.
    pub bytes: [usize; FOOTPRINT_PARTS],
    /// Tổng byte của các trường vô hướng.
    pub scalars: usize,
    /// Byte đệm (giữa các trường/đuôi struct).
    pub padding: usize,
    /// `size_of::<Machine>()`.
    pub total: usize,
}

/// **KT-405** — `footprint()` trên **kiểu cụ thể** `Machine<StaticStore, TestObserver>` để `Machine::footprint()`
/// (vị trí biểu thức, không chú thích kiểu) vẫn suy ra được `O` — cùng lý do như
/// [`Machine::new`]. Đường generic là [`Machine::footprint_of`].
impl Machine<StaticStore, TestObserver> {
    /// Báo cáo phân bố byte tĩnh của [`Machine`] (D2-84) — observer mặc định.
    pub fn footprint() -> Footprint {
        Self::footprint_of()
    }
}

impl<S: crate::store::KernelStore, O: CoreObserver> Machine<S, O> {
    /// Báo cáo phân bố byte tĩnh của `Machine<O>` (D2-84) — đường **generic**.
    pub fn footprint_of() -> Footprint {
        // **KT-405** — ba phần oracle/test do observer sở hữu (`[0; 3]` với `NoopObserver`).
        let obs_parts = O::footprint_parts();
        let bytes = [
            size_of::<[crate::arena::Region; MAX_UNTYPEDS]>(),
            size_of::<[crate::machine::Meta; MAX_OBJECTS]>(),
            size_of::<[crate::machine::Cap; MAX_CAPS]>(),
            size_of::<crate::slotmem::SlotMem>(),
            size_of::<crate::arena::Region>(),
            size_of::<[crate::tcbdata::TcbData; MAX_TCBS]>(),
            size_of::<[u16; MAX_TCBS]>(),
            size_of::<[crate::tcbdata::ScData; MAX_SCHED_CONTEXTS]>(),
            size_of::<[u16; MAX_SCHED_CONTEXTS]>(),
            obs_parts[0],
            obs_parts[1],
            obs_parts[2],
            size_of::<[crate::perhart::PerHartState; MAX_HARTS]>(),
            size_of::<[EndpointData; MAX_ENDPOINTS]>(),
            size_of::<[u16; MAX_ENDPOINTS]>(),
            size_of::<[ReplyData; MAX_REPLIES]>(),
            size_of::<[u16; MAX_REPLIES]>(),
            size_of::<[crate::objstate::NotificationData; MAX_NOTIFICATIONS]>(),
            size_of::<[u16; MAX_NOTIFICATIONS]>(),
            size_of::<[QueuedMsg; MAX_QUEUED_MESSAGES]>(),
            size_of::<[u16; MAX_QUEUED_MESSAGES]>(),
            size_of::<[AddrWord; MAX_ADDR_WORDS]>(),
            size_of::<[WaitNode; MAX_WAIT_NODES]>(),
            size_of::<[u16; MAX_WAIT_NODES]>(),
            size_of::<[u16; WAITADDR_BUCKETS]>(),
            size_of::<[u16; WAITADDR_BUCKETS]>(),
        ];
        // 20 × usize + 18 × u64 (9 bộ đếm cũ + 3 bộ đếm chuyển cap KT-63 + `tlb_seq`/
        // `tlb_local_flushes`/`tlb_remote_posts`/`tlb_serviced` của KT-64 + 2 bộ đếm extra cap
        // KT-309) + u16 (current_hart) + u32 (`finalise_overflow`) + bool. **KT-405**: ba bảng
        // oracle (`pair_hits`/`forbidden_hits`/`finalise_log`) đã sang observer ⇒ `bytes` 29 → 26
        // dòng, nhưng **không** trường vô hướng nào rời `Machine` (`finalise_overflow` ở lại) ⇒
        // `scalars` **311 không đổi** (pin `kt60_core_parity` phía sim).
        let scalars = 20 * size_of::<usize>()
            + 18 * size_of::<u64>()
            + size_of::<u16>()
            + size_of::<u32>()
            + size_of::<bool>();
        let total = size_of::<Self>();
        let used: usize = bytes.iter().sum::<usize>() + scalars;
        Footprint {
            names: [
                "regions[16]",
                "objects[256]",
                "caps[512]",
                "slot_mem",
                "harness_region",
                "tcbs[64]",
                "tcb_free[64]",
                "scs[32]",
                "sc_free[32]",
                "obs.pair_hits[121]",
                "obs.forbidden_hits[121]",
                "obs.finalise_log[256]",
                "perhart[8]",
                "endpoints[16]",
                "endpoint_free[16]",
                "replies[32]",
                "reply_free[32]",
                "notifications[16]",
                "notification_free[16]",
                "ipc_msgs[24]",
                "ipc_msg_free[24]",
                "addr_words[64]",
                "wait_nodes[64]",
                "wait_free[64]",
                "wait_buckets[64]",
                "wait_buckets_tail[64]",
            ],
            bytes,
            scalars,
            padding: total.saturating_sub(used),
            total,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::MAX_SLOT_WORDS;
    use std::println;

    /// **KT-312** — `Meta` **không** được đệm thừa: ba trường chỉ số có miền giá trị **chứng
    /// minh được** nhỏ hơn nhiều so với `usize`:
    ///
    /// - `untyped` = chỉ số **object** của Untyped chứa object ⇒ `< MAX_OBJECTS = 256`;
    /// - `region` = chỉ số arena trong `Machine::regions` ⇒ `< MAX_UNTYPEDS = 16`;
    /// - `slots_meta` = số slot CNode ⇒ `≤ MAX_SLOT_WORDS = 4096` (harness ≤ `HARNESS_SLOT_WORDS`
    ///   = 1024, arena ≤ `REGION_SLOT_WORDS` = 192).
    ///
    /// `Option<usize>` chiếm **16 B** mỗi trường (không niche) ⇒ `Meta` **88 B**; kiểu hẹp
    /// `Option<u8>`/`Option<u16>` giữ **nguyên** ngữ nghĩa (`Some`/`None`, cùng giá trị) và cho
    /// `Meta` **48 B** ⇒ thu hồi **40 B × 256 = 10.240 B** trên `objects[256]`. D2-84: **tối ưu**
    /// chứ không nới pin.
    #[test]
    fn kt312_meta_khong_dem_thua() {
        // Miền giá trị của ba trường — tiền đề của việc thu hẹp kiểu.
        assert!(
            MAX_OBJECTS - 1 <= u8::MAX as usize,
            "untyped phải vừa u8: MAX_OBJECTS = {MAX_OBJECTS}"
        );
        assert!(
            MAX_UNTYPEDS - 1 <= u8::MAX as usize,
            "region phải vừa u8: MAX_UNTYPEDS = {MAX_UNTYPEDS}"
        );
        assert!(
            MAX_SLOT_WORDS <= u16::MAX as usize,
            "slots_meta phải vừa u16: MAX_SLOT_WORDS = {MAX_SLOT_WORDS}"
        );
        assert_eq!(
            size_of::<crate::machine::Meta>(),
            48,
            "Meta phải 48 B (ba trường chỉ số dùng kiểu hẹp, không Option<usize>)"
        );
        // **KT-311** (merge cùng vòng 20) thêm `TcbData.entry: u64` ⇒ `+8 × MAX_TCBS(64) = 512`
        // ⇒ trần của KT-312 là `184320 + 512 = 184832`. Trần **không** nới: nó vẫn chốt đúng
        // "thu hồi ≥ 10.200 B của KT-312" cộng phần tăng **có ghi** của KT-311.
        assert!(
            size_of::<Machine>() <= 184_832,
            "KT-312 + KT-311: size_of::<Machine>() = {} > 184832 (thu hồi < 10.200 B)",
            size_of::<Machine>()
        );
    }

    /// **KT-312** — giá trị **cực đại** của ba trường chỉ số vẫn **vừa** kiểu hẹp: 16 Untyped
    /// (chỉ số `0..=15`), object retype từ Untyped **cuối** (`untyped = 15`), CNode harness
    /// **1000 slot** (`slots_meta = 1000 > u8::MAX`). Nếu kiểu hẹp **cắt** giá trị thì
    /// `untyped_arena`/`object_bytes`/`cnode_slot_count` trả sai **ngay** — test này là chứng
    /// minh ngữ nghĩa, không chỉ chứng minh byte.
    #[test]
    fn kt312_meta_gia_tri_cuc_dai_van_vua_kieu_hep() {
        use crate::error::{ObjType, SlotRef};
        let mut m = Machine::<crate::store::StaticStore, crate::observer::TestObserver>::new();
        let root = m.add_cnode(MAX_UNTYPEDS);
        let mut last_untyped = 0;
        for i in 0..MAX_UNTYPEDS {
            let base = 0x1000_0000 + (i as u64) * 0x10_0000;
            let u = m.add_untyped(base, 1 << 16);
            let uc = m.root_cap(u);
            m.put_cap(root, i, uc).expect("cap untyped");
            last_untyped = u;
        }
        // `region = Some(15)` — chỉ số arena lớn nhất.
        assert_eq!(
            m.untyped_arena(last_untyped),
            Some((0x1000_0000 + 15 * 0x10_0000, 1 << 16)),
            "region = 15 phải đọc lại đúng (Option<u8>)"
        );
        // `untyped = Some(15)` — chỉ số Untyped lớn nhất.
        let cap = m
            .retype(SlotRef::new(root, MAX_UNTYPEDS - 1), ObjType::Tcb, 0, 0)
            .expect("retype TCB từ Untyped cuối");
        let obj = m.cap_obj(cap).expect("object của cap");
        let (off, len) = m
            .object_bytes(obj)
            .expect("object_bytes phải Some (untyped = 15)");
        assert!(len > 0, "len phải dương");
        assert!(
            off + len <= 1 << 16,
            "object phải nằm trong Untyped: off={off} len={len}"
        );
        // `slots_meta = 1000` — lớn hơn `u8::MAX` ⇒ chứng minh phải là `u16`, không phải `u8`.
        let big = m.add_cnode(1000);
        assert_eq!(m.cnode_slot_count(big), Ok(1000), "slots_meta = 1000");
        assert!(1000 > u8::MAX as usize, "ca này phải vượt u8");
    }

    /// **KT-405** — thân ABI **tại chỗ** phải vừa `spec/abi.toml:159` (`tcb = 512`,
    /// `endpoint = 64`, `sched_context = 256`, `notification = 64`, `reply = 64`) với 16 B
    /// header/đệm cho `UntypedStore` (KT-410). Đây là `const` assert ở `tcbdata.rs`/`objstate.rs`
    /// **cộng** phép đo ở đây (test bắt được cả khi ai đổi `repr`/thứ tự trường).
    #[test]
    fn kt405_than_abi_vua_spec_abi_toml_159() {
        let tcb_core = size_of::<crate::tcbdata::TcbCore>();
        let endpoint_core = size_of::<crate::objstate::EndpointCore>();
        let sc = size_of::<crate::tcbdata::ScData>();
        let notif = size_of::<crate::objstate::NotificationData>();
        let reply = size_of::<crate::objstate::ReplyData>();
        println!(
            "# kt405 abi: TcbCore={tcb_core} EndpointCore={endpoint_core} ScData={sc} \
             NotificationData={notif} ReplyData={reply} Meta={} Machine={} \
             TcbData={} TcbModel={} IpcArea={} EndpointData={} EndpointModel={} \
             MachineNoop={} TestObserver={}",
            size_of::<crate::machine::Meta>(),
            size_of::<Machine>(),
            size_of::<crate::tcbdata::TcbData>(),
            size_of::<crate::tcbdata::TcbModel>(),
            size_of::<crate::ipc::IpcArea>(),
            size_of::<crate::objstate::EndpointData>(),
            size_of::<crate::objstate::EndpointModel>(),
            size_of::<Machine<StaticStore, crate::observer::NoopObserver>>(),
            size_of::<crate::observer::TestObserver>(),
        );
        assert!(tcb_core <= 496, "TcbCore {tcb_core} > 496 (spec tcb = 512)");
        assert!(
            endpoint_core <= 48,
            "EndpointCore {endpoint_core} > 48 (spec endpoint = 64)"
        );
        assert!(sc <= 240, "ScData {sc} > 240 (spec sched_context = 256)");
        assert_eq!(notif, 32, "NotificationData phải 32 B");
        assert_eq!(reply, 8, "ReplyData phải 8 B");
        assert_eq!(size_of::<crate::machine::Meta>(), 48, "Meta phải 48 B");
        assert!(
            size_of::<Machine>() <= 184_832,
            "Machine {} > 184832 (KT-312)",
            size_of::<Machine>()
        );
    }

    #[test]
    fn in_bang_phan_bo_va_pin_d2_84() {
        let f = Machine::<crate::store::StaticStore, crate::observer::TestObserver>::footprint();
        let mut sum = 0usize;
        for i in 0..FOOTPRINT_PARTS {
            println!("{:28} {:>8}", f.names[i], f.bytes[i]);
            sum += f.bytes[i];
        }
        println!("{:28} {:>8}", "scalars", f.scalars);
        println!("{:28} {:>8}", "padding", f.padding);
        println!("{:28} {:>8}", "TOTAL", f.total);
        assert_eq!(sum + f.scalars + f.padding, f.total);
        // F5 reviewer-kt60 (D2-85c): padding ≤ 8 B — nếu lớn hơn, bảng đã bỏ sót trường và
        // tổng sẽ "đúng" một cách vô nghĩa.
        assert!(
            f.padding <= 8,
            "bang phan bo bo sot truong: padding {} B > 8",
            f.padding
        );
        assert!(f.total <= 196_608, "D2-84: {} > 196608", f.total);
    }
}
