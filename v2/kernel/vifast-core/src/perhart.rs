// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`PerHartState`** — trạng thái **theo hart** tách khỏi sổ cái object (KT-405, D2-259 mục 6).
//!
//! Trước KT-405 bốn bảng `sched`/`published`/`mail`/`tlb_pending` nằm rời trong [`crate::Machine`]
//! — tức là **mọi** hart dùng chung một `IrqMcsLock<Machine>` (Big Kernel Lock) và không có
//! ranh giới dữ liệu nào để tách khoá theo hart. KT-405 gom chúng thành **một** khối
//! `[PerHartState; MAX_HARTS]`:
//!
//! | Trường | Vai trò | Ai ghi |
//! |---|---|---|
//! | `sched` | ready queue 256 mức + `current` của hart | hart đó (`dispatch`/`enqueue`/`remove`) |
//! | `published` | bản **công bố** cho hart khác **đọc** (thiết kế §4.16 luật 3) | hart đó |
//! | `mail` | hộp thư FIFO 8 message + nhật ký | mọi hart **post**, hart đích **rút** |
//! | `tlb_pending` | shootdown TLB đang chờ + tập hart phải ack | hart gọi + hart đích |
//!
//! Đây là **tiền đề dữ liệu** cho `KT-412`/`KT-413` (SMP 4 hart) và `KT-415` (đo tranh khoá):
//! khi mỗi hart sở hữu một `PerHartState`, khoá có thể hạ từ "toàn `Machine`" xuống "một hart"
//! mà **không** đổi ngữ nghĩa. KT-405 **chưa** đổi khoá (vẫn một `IrqMcsLock<Machine>` ở
//! `plat-*`) — chỉ tách **ranh giới dữ liệu** + accessor, đúng phạm vi "0 sửa API 3 arch".
//!
//! # Bất biến
//!
//! - `sched[h].current()` chỉ là TCB có `home == h` (KT-29).
//! - `published[h]` là **ảnh chụp** của `sched[h]` (`idle`/`cur_prio`), cập nhật ở
//!   [`crate::Machine::publish_hart`].
//! - `mail[h]` chỉ hart `h` rút (luật chống deadlock §4.10.3).
//! - `tlb_pending[h].active` ⇒ hart `h` còn phải `tlb_service` trước khi `dispatch`.

use crate::mailbox::HartMailbox;
use crate::sched::{HartSched, Published};
use crate::tlb::TlbPending;
use crate::trace::TraceRing;

/// Trạng thái **một hart** — xem tài liệu module.
#[derive(Debug, Clone, Copy)]
pub struct PerHartState {
    /// Ready queue 256 mức + TCB đang chạy của hart.
    pub(crate) sched: HartSched,
    /// Trường **công bố** cho hart khác đọc (thiết kế §4.16 luật 3).
    pub(crate) published: Published,
    /// Hộp thư FIFO theo hart (KT-22).
    pub(crate) mail: HartMailbox,
    /// Shootdown TLB đang chờ của hart (KT-64).
    pub(crate) tlb_pending: TlbPending,
    /// **KT-408** — kênh ring trace 32 B của hart (`D2-09`, luôn bật). **ZST** ⇒ 0 B: kho bản ghi
    /// nằm trong section NOLOAD `.corespace` (`crate::trace`), `size_of::<Machine>()` không đổi.
    pub(crate) trace: TraceRing,
}

impl PerHartState {
    /// Trạng thái rỗng của một hart (hằng — dùng cho `Machine::with_harts`).
    pub const fn new() -> Self {
        Self {
            sched: HartSched::new(),
            published: Published {
                idle: true,
                cur_prio: 0,
            },
            mail: HartMailbox::new(),
            tlb_pending: TlbPending::EMPTY,
            trace: TraceRing::new(),
        }
    }

    /// Khởi tạo **tại chỗ** trên `.bss` đã zero (KT-61): chỉ đặt trường **khác 0**
    /// (`idle = true`, `mail` có `Option<MailboxMsg>` với `None != 0`).
    pub(crate) fn reset(&mut self) {
        self.sched.reset();
        self.published = Published {
            idle: true,
            cur_prio: 0,
        };
        self.mail.reset();
        self.tlb_pending = TlbPending::EMPTY;
    }
}

impl Default for PerHartState {
    fn default() -> Self {
        Self::new()
    }
}
