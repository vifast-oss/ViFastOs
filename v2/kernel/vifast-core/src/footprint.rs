// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Dấu chân **tĩnh** của [`Machine`] — D2-84 pin core ≤ **196.608 B** (192 KiB) = trần ảnh
//! 262.144 B trừ 64 KiB headroom; vượt pin ⇒ **tối ưu** (giảm bảng/nhật ký), **không** nâng
//! pin.
//!
//! KT-59 dựng bảng 14 dòng; KT-60 thêm body Endpoint/Reply/Notification + pool message đang
//! chờ + bảng WaitOnAddress + biến thể `IpcArea` trong TCB; KT-63 thêm **receive window**
//! (`TcbData.recv_window`, 4 CPTR) + cap nguồn/ảnh chụp trong `Message` + 3 bộ đếm cap;
//! KT-64 thêm `slot_mem` + `harness_region` (thay `slot_pool`) + `tlb_pending[8]`. Hàm [`Machine::footprint`] trả
//! **phân bố byte** để test pin và KT-61 đối chiếu trước khi link; bất biến
//! `sum(bytes) + scalars + padding == size_of::<Machine>()` được test pin kiểm.

use core::mem::size_of;

use crate::machine::{
    Machine, MAX_CAPS, MAX_ENDPOINTS, MAX_FINALISE, MAX_HARTS, MAX_NOTIFICATIONS, MAX_OBJECTS,
    MAX_QUEUED_MESSAGES, MAX_REPLIES, MAX_SCHED_CONTEXTS, MAX_TCBS, MAX_UNTYPEDS,
};
use crate::mailbox::HartMailbox;
use crate::objstate::{
    AddrWord, EndpointData, QueuedMsg, ReplyData, WaitNode, MAX_ADDR_WORDS, MAX_WAIT_NODES,
    WAITADDR_BUCKETS,
};
use crate::sched::{HartSched, Published};
use crate::tcb::TCB_STATE_COUNT;
use crate::tlb::TlbPending;

/// Số thành phần của báo cáo [`Footprint`] (29 bảng/mảng; scalar + padding tính riêng).
pub const FOOTPRINT_PARTS: usize = 29;

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

impl Machine {
    /// Báo cáo phân bố byte tĩnh của [`Machine`] (D2-84).
    pub fn footprint() -> Footprint {
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
            size_of::<[u32; TCB_STATE_COUNT * TCB_STATE_COUNT]>(),
            size_of::<[u32; TCB_STATE_COUNT * TCB_STATE_COUNT]>(),
            size_of::<[crate::machine::FinaliseEntry; MAX_FINALISE]>(),
            size_of::<[HartSched; MAX_HARTS]>(),
            size_of::<[HartMailbox; MAX_HARTS]>(),
            size_of::<[Published; MAX_HARTS]>(),
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
            size_of::<[TlbPending; MAX_HARTS]>(),
        ];
        // 20 × usize + 18 × u64 (9 bộ đếm cũ + 3 bộ đếm chuyển cap KT-63 + `tlb_seq`/
        // `tlb_local_flushes`/`tlb_remote_posts`/`tlb_serviced` của KT-64 + 2 bộ đếm extra cap
        // KT-309) + u16 (current_hart) + u32 (finalise_overflow) + bool.
        let scalars = 20 * size_of::<usize>()
            + 18 * size_of::<u64>()
            + size_of::<u16>()
            + size_of::<u32>()
            + size_of::<bool>();
        let total = size_of::<Machine>();
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
                "pair_hits[121]",
                "forbidden_hits[121]",
                "finalise_log[256]",
                "sched[8]",
                "mail[8]",
                "published[8]",
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
                "tlb_pending[8]",
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
    use std::println;

    #[test]
    fn in_bang_phan_bo_va_pin_d2_84() {
        let f = Machine::footprint();
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
