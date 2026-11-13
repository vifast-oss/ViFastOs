// SPDX-License-Identifier: MIT OR Apache-2.0
//! **MCS queue lock** — BKL dạng khoá xếp hàng của thiết kế §4.16 (S1, `:407`).
//!
//! ## Thuật toán
//!
//! MCS (Mellor-Crummey & Scott): người xin khoá chèn **node của mình** vào đuôi hàng; node
//! trước đó ghi liên kết tới node mới; khi mở khoá, chủ hiện tại trao khoá **đúng một** node
//! kế tiếp (FIFO theo thứ tự `tail.swap`) và người được trao tự quay vòng trên cờ `locked` của
//! node mình. Khác test-and-set, mỗi người chờ quay vòng trên **cache line của node mình**.
//!
//! ## Vì sao node nằm trong **pool cố định của khoá**
//!
//! MCS cổ điển đặt node trên **stack của người gọi**; trong Rust, [`Guard`] trả về theo giá trị
//! nên node sẽ bị **di chuyển** sau khi địa chỉ của nó đã được công bố cho node trước ⇒ con trỏ
//! treo, UB (K1C-PLAN §5 KT-30 rủi ro (a)). Vì vậy node nằm trong mảng **cố định**
//! `[Node; MAX_WAITERS]` của chính khoá: địa chỉ bất động suốt đời khoá, không cấp phát, không
//! `Pin`, không `Box`, và `Guard` chỉ giữ **chỉ số slot** (một `usize`) nên di chuyển vô hại.
//! Hàng đợi liên kết bằng **chỉ số slot + 1** (`0` = không có) — không có con trỏ thô trong
//! toàn bộ thuật toán.
//!
//! Đây là biến thể "node tĩnh theo CPU" quen thuộc của MCS trong kernel (mỗi hart một node
//! tĩnh); ở đây pool chung [`MAX_WAITERS`] slot để API `lock()` không cần tham số hart. `[TK]`.
//!
//! ## Bậc bộ nhớ
//!
//! `tail`/`head`/`next`/`locked` dùng `Ordering::SeqCst`: K1c **không** có mục tiêu hiệu năng
//! và cần một mô hình dễ chứng minh cho tính FIFO + mutual exclusion; tối ưu xuống
//! `AcqRel`/`Release` để dành cho mốc đo hiệu năng sau này (ghi trong `v2/docs/k1c-sync.md`).
//! Bitmap slot dùng `AcqRel`/`Release` vì chỉ bảo vệ quyền sở hữu slot, không tham gia chứng
//! minh FIFO.
//!
//! ## Bất biến
//!
//! (i) `tail == 0` ⇔ hàng rỗng (trừ khoảng chuyển tiếp một lệnh khi người vừa chiếm khoá chưa
//! kịp ghi `head`); (ii) mỗi slot có tối đa một chủ tại một thời điểm (bitmap); (iii) chủ hiện
//! tại là node ở **đầu** hàng (`head`); (iv) trao khoá theo đúng thứ tự chèn `tail.swap` ⇒ FIFO;
//! (v) không reentrant: xếp hàng lần hai khi đang giữ **không** được acquire.

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::Ordering::{AcqRel, Acquire, Release, SeqCst};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};

/// Số slot chờ đồng thời tối đa của [`McsLock`] (kích thước pool node, `[TK]`).
///
/// `MAX_HARTS` = 8 (thiết kế §4.16) cộng dư cho người gọi lồng nhau/ngoài hart; trần cứng của
/// bitmap slot là `usize::BITS` (64 trên hart 64-bit).
pub const MAX_WAITERS: usize = 16;

/// Bitmap `n` bit thấp bằng 1 (slot trống ban đầu). Hằng số, không cấp phát; `n >= usize::BITS`
/// được kẹp về toàn 1 để **không bao giờ** tràn phép dịch (test `free_mask_khong_tran`).
const fn free_mask(n: usize) -> usize {
    if n >= usize::BITS as usize {
        usize::MAX
    } else {
        (1usize << n) - 1
    }
}

/// Trần cứng: pool phải nằm trong bitmap slot (`usize`), kiểm ngay lúc biên dịch.
const _: () = assert!(MAX_WAITERS > 0 && MAX_WAITERS <= usize::BITS as usize);

/// Một node trong hàng đợi MCS (nằm trong pool của khoá, **không** bao giờ di chuyển).
struct Node {
    /// Slot+1 của người kế tiếp; `0` = chưa có.
    next: AtomicUsize,
    /// `true` = đang chờ; node trước ghi `false` khi trao khoá.
    locked: AtomicBool,
    /// Ticket đơn điệu của lần xếp hàng (dùng cho [`McsLock::next_owner`]).
    owner: AtomicU32,
}

impl Node {
    const fn new() -> Self {
        Node {
            next: AtomicUsize::new(0),
            locked: AtomicBool::new(false),
            owner: AtomicU32::new(0),
        }
    }
}

/// Khoá xếp hàng MCS bảo vệ `T` (BKL của thiết kế §4.16).
///
/// ```
/// let lock = vifast_sync::McsLock::new(0u32);
/// {
///     let mut g = lock.lock();
///     *g += 1;
/// }
/// assert_eq!(*lock.try_lock().unwrap(), 1);
/// ```
pub struct McsLock<T> {
    /// Slot+1 của node cuối hàng; `0` = hàng rỗng.
    tail: AtomicUsize,
    /// Slot+1 của node đầu hàng (chủ hiện tại hoặc node vừa được trao); `0` = hàng rỗng.
    head: AtomicUsize,
    /// Bitmap slot trống: bit `i` = 1 ⇔ slot `i` chưa ai giữ.
    free: AtomicUsize,
    /// Số node đang trong hàng — **chẩn đoán**, không dùng để đồng bộ.
    queued: AtomicUsize,
    /// Cấp ticket đơn điệu cho mỗi lần xếp hàng.
    ticket: AtomicU32,
    nodes: [Node; MAX_WAITERS],
    value: UnsafeCell<T>,
}

// SAFETY: `McsLock` chỉ cho một luồng truy cập `value` tại một thời điểm (qua `Guard`), nên chia
// sẻ `&McsLock` giữa các luồng là an toàn khi `T: Send`; chuyển hẳn khoá sang luồng khác cũng
// an toàn với cùng điều kiện (mọi trạng thái nội bộ là atomic).
unsafe impl<T: Send> Send for McsLock<T> {}
// SAFETY: như trên — `Sync` cần `T: Send` vì `&McsLock` cho phép lấy `&mut T` qua `Guard`.
unsafe impl<T: Send> Sync for McsLock<T> {}

impl<T> McsLock<T> {
    /// Tạo khoá với giá trị ban đầu; dùng được trong `static`/`const` (không cấp phát).
    pub const fn new(value: T) -> Self {
        McsLock {
            tail: AtomicUsize::new(0),
            head: AtomicUsize::new(0),
            free: AtomicUsize::new(free_mask(MAX_WAITERS)),
            queued: AtomicUsize::new(0),
            ticket: AtomicU32::new(0),
            nodes: [const { Node::new() }; MAX_WAITERS],
            value: UnsafeCell::new(value),
        }
    }

    /// Số slot chờ đồng thời tối đa của khoá này.
    pub const fn capacity(&self) -> usize {
        MAX_WAITERS
    }

    /// Số node **đang trong hàng**, tính cả chủ hiện tại (node của chủ chỉ rời hàng khi mở khoá).
    ///
    /// Chỉ dùng cho **chẩn đoán/test** (K1C-PLAN §5 KT-30 DoD 2); không phải bất biến đồng bộ:
    /// với nhiều luồng có thể thấy trạng thái chuyển tiếp giữa hai lệnh.
    pub fn queue_len(&self) -> usize {
        self.queued.load(SeqCst)
    }

    /// Ticket của node **đầu hàng**: chủ hiện tại khi khoá đang giữ, hoặc node vừa được trao
    /// khoá; `None` khi hàng rỗng.
    ///
    /// Chỉ dùng cho **chẩn đoán/test** (DoD 2); ticket tăng đơn điệu theo thứ tự xếp hàng nên
    /// `next_owner()` cho thấy trao khoá có đúng FIFO hay không.
    pub fn next_owner(&self) -> Option<u32> {
        let head = self.head.load(SeqCst);
        if head == 0 {
            None
        } else {
            Some(self.nodes[head - 1].owner.load(SeqCst))
        }
    }

    /// Xin khoá, **chờ** tới khi được trao (FIFO). Trả [`Guard`]; `Drop` của guard mở khoá.
    ///
    /// Không reentrant: gọi `lock()` lần hai trên cùng luồng khi đang giữ sẽ **treo** (đúng
    /// nghĩa khoá không reentrant; dùng `try_lock()` nếu cần kiểm).
    pub fn lock(&self) -> Guard<'_, T> {
        let (slot, acquired) = self.enqueue();
        if !acquired {
            self.wait_handoff(slot);
        }
        Guard {
            lock: self,
            slot,
            _not_send: PhantomData,
        }
    }

    /// Thử lấy khoá **không chờ**: `Some(guard)` nếu lấy được, `None` nếu đang bị giữ/đang có
    /// người xếp hàng. Không reentrant (đang giữ ⇒ `None`).
    pub fn try_lock(&self) -> Option<Guard<'_, T>> {
        // Chỉ claim slot khi còn trống — không chờ slot (giữ đúng nghĩa "try").
        let slot = self.try_claim_slot()?;
        let node = &self.nodes[slot];
        let me = slot + 1;
        // Khởi tạo node **trước** khi công bố nó qua `tail` (người kế nhiệm có thể ghi `next`).
        node.next.store(0, SeqCst);
        node.locked.store(false, SeqCst);
        if self.tail.compare_exchange(0, me, SeqCst, SeqCst).is_err() {
            // Có chủ/người xếp hàng ⇒ nhả slot, không tiêu ticket.
            self.release_slot(slot);
            return None;
        }
        node.owner.store(self.ticket.fetch_add(1, SeqCst), SeqCst);
        self.head.store(me, SeqCst);
        self.queued.fetch_add(1, SeqCst);
        Some(Guard {
            lock: self,
            slot,
            _not_send: PhantomData,
        })
    }

    // ------------------------------------------------------------ nội bộ

    /// Claim một slot trống; `None` nếu pool đã cạn (không chờ).
    fn try_claim_slot(&self) -> Option<usize> {
        let mut free = self.free.load(Acquire);
        loop {
            if free == 0 {
                return None;
            }
            let bit = free.trailing_zeros() as usize;
            let mask = 1usize << bit;
            match self
                .free
                .compare_exchange_weak(free, free & !mask, AcqRel, Acquire)
            {
                Ok(_) => return Some(bit),
                Err(actual) => free = actual,
            }
        }
    }

    /// Claim một slot, chờ tới khi có (tiến trình được bảo đảm vì slot chỉ bị giữ bởi người
    /// đang giữ khoá/đang xếp hàng, và người giữ khoá luôn mở khoá được).
    fn claim_slot(&self) -> usize {
        loop {
            if let Some(slot) = self.try_claim_slot() {
                return slot;
            }
            core::hint::spin_loop();
        }
    }

    /// Trả slot về pool.
    fn release_slot(&self, slot: usize) {
        self.free.fetch_or(1usize << slot, Release);
    }

    /// Chèn node vào đuôi hàng; trả `(slot, đã_acquire_ngay)` — **không** chờ trao khoá.
    fn enqueue(&self) -> (usize, bool) {
        let slot = self.claim_slot();
        let node = &self.nodes[slot];
        node.owner.store(self.ticket.fetch_add(1, SeqCst), SeqCst);
        node.next.store(0, SeqCst);
        node.locked.store(true, SeqCst);
        let me = slot + 1;
        let prev = self.tail.swap(me, SeqCst);
        if prev == 0 {
            // Hàng rỗng: node này là chủ ngay.
            node.locked.store(false, SeqCst);
            self.head.store(me, SeqCst);
            self.queued.fetch_add(1, SeqCst);
            (slot, true)
        } else {
            // Nối vào node trước; node trước sẽ ghi `locked = false` khi trao khoá.
            self.nodes[prev - 1].next.store(me, SeqCst);
            self.queued.fetch_add(1, SeqCst);
            (slot, false)
        }
    }

    /// Chờ tới khi node `slot` được trao khoá (node trước ghi `locked = false`).
    fn wait_handoff(&self, slot: usize) {
        let node = &self.nodes[slot];
        while node.locked.load(SeqCst) {
            core::hint::spin_loop();
        }
    }

    /// Mở khoá đang giữ ở `slot`: trao cho node kế tiếp nếu có, nếu không thì làm rỗng hàng.
    fn release(&self, slot: usize) {
        let node = &self.nodes[slot];
        let me = slot + 1;
        let mut next = node.next.load(SeqCst);
        if next == 0 {
            // Chưa thấy người kế tiếp: thử làm rỗng `tail`.
            if self.tail.compare_exchange(me, 0, SeqCst, SeqCst).is_ok() {
                // Hàng thật sự rỗng. `head` chỉ bị xoá nếu vẫn là node này (một người vừa
                // acquire có thể đã ghi `head` của nó — CAS thất bại thì giữ nguyên).
                self.head.compare_exchange(me, 0, SeqCst, SeqCst).ok();
                self.queued.fetch_sub(1, SeqCst);
                self.release_slot(slot);
                return;
            }
            // Người kế tiếp đã `swap` vào `tail` nhưng chưa kịp nối `next`; chờ liên kết.
            loop {
                next = node.next.load(SeqCst);
                if next != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
        }
        let next_slot = next - 1;
        self.head.store(next, SeqCst);
        self.nodes[next_slot].locked.store(false, SeqCst);
        self.queued.fetch_sub(1, SeqCst);
        self.release_slot(slot);
    }

    // ------------------------------------------------------------ API test nội bộ

    /// **Nội bộ crate** (`pub(crate)`, K1C-PLAN §5 KT-30 DoD 2): xếp hàng **không chờ** — mô
    /// phỏng một luồng khác xin khoá trên cùng luồng thật. Trả `(slot, đã_acquire_ngay)`.
    ///
    /// **Không** `pub`: gọi `test_release` trên slot không sở hữu từ code ngoài crate (safe)
    /// sẽ tạo hai `Guard` cùng lúc ⇒ alias `&mut T` = UB. Chỉ test trong crate dùng.
    #[cfg(test)]
    pub(crate) fn test_enqueue(&self) -> (usize, bool) {
        self.enqueue()
    }

    /// **Nội bộ crate**: node `slot` đã được trao khoá chưa.
    #[cfg(test)]
    pub(crate) fn test_woken(&self, slot: usize) -> bool {
        !self.nodes[slot].locked.load(SeqCst)
    }

    /// **Nội bộ crate** (`cfg(test)`): mở khoá đang giữ ở `slot` (dùng sau
    /// [`McsLock::test_enqueue`] hoặc [`Guard::slot`]). Chỉ test trong crate gọi — xem cảnh báo
    /// ở [`McsLock::test_enqueue`].
    #[cfg(test)]
    pub(crate) fn test_release(&self, slot: usize) {
        self.release(slot);
    }
}

/// Guard giữ khoá; `Drop` mở khoá (trao cho node kế tiếp theo FIFO).
pub struct Guard<'a, T> {
    lock: &'a McsLock<T>,
    slot: usize,
    /// `Guard` **không** `Send`: miền tới hạn gắn với luồng đã lấy khoá (như `MutexGuard`).
    _not_send: PhantomData<*const ()>,
}

impl<T> Guard<'_, T> {
    /// **Nội bộ crate**: chỉ số slot của node đang giữ khoá (chỉ test trong crate dùng — nếu
    /// `pub`, code ngoài crate có thể `test_release` slot người khác và tạo hai guard cùng lúc).
    #[cfg(test)]
    pub(crate) fn slot(&self) -> usize {
        self.slot
    }
}

impl<T> Deref for Guard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: `Guard` là bằng chứng duy nhất cho quyền truy cập `value`; nó không `Clone`,
        // không `Send`, và chỉ tồn tại từ lúc acquire tới `Drop` (mở khoá).
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: như `Deref`; `&mut self` bảo đảm không có `&` nào khác đang sống từ guard này.
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.lock.release(self.slot);
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for Guard<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&**self, f)
    }
}

#[cfg(test)]
mod tests {
    //! Test **single-thread** dùng API nội bộ (`pub(crate)`) — FIFO, không reentrant, pool cạn.
    //! Chạy dưới Miri như mọi test của lib (`v2 miri -p vifast-sync`).

    use super::*;

    #[test]
    fn free_mask_khong_tran() {
        assert_eq!(free_mask(0), 0);
        assert_eq!(free_mask(1), 1);
        assert_eq!(free_mask(MAX_WAITERS), (1usize << MAX_WAITERS) - 1);
        assert_eq!(free_mask(usize::BITS as usize), usize::MAX);
        assert_eq!(free_mask(usize::BITS as usize + 8), usize::MAX);
        assert_eq!(
            free_mask(64),
            usize::MAX,
            "n >= 64 (trên hart 64-bit) không tràn"
        );
    }

    #[test]
    fn fifo_theo_thu_tu_xep_hang() {
        let lock = McsLock::new(0u32);
        let guard = lock.lock(); // ticket 0 — "luồng chính"
        let s0 = guard.slot();
        assert_eq!(lock.next_owner(), Some(0));
        assert_eq!(lock.queue_len(), 1);

        // Hai "luồng" xếp hàng theo thứ tự ticket 1 rồi 2; cả hai đều **chưa** được trao khoá.
        let (s1, got1) = lock.test_enqueue();
        assert!(!got1, "đã có chủ ⇒ người thứ hai phải xếp hàng");
        assert_eq!(lock.next_owner(), Some(0), "đầu hàng vẫn là chủ hiện tại");
        assert_eq!(lock.queue_len(), 2);

        let (s2, got2) = lock.test_enqueue();
        assert!(!got2);
        assert_eq!(lock.queue_len(), 3);
        assert_ne!(s0, s1);
        assert_ne!(s1, s2);
        assert_ne!(s0, s2);
        assert!(!lock.test_woken(s1));
        assert!(!lock.test_woken(s2));

        // Mở khoá của chủ: trao đúng cho người xếp hàng **đầu tiên** (FIFO).
        drop(guard);
        assert!(lock.test_woken(s1), "FIFO: ticket 1 được trao trước");
        assert!(!lock.test_woken(s2));
        assert_eq!(lock.next_owner(), Some(1));
        assert_eq!(lock.queue_len(), 2);

        lock.test_release(s1);
        assert!(
            lock.test_woken(s2),
            "FIFO: ticket 2 chỉ được trao sau ticket 1"
        );
        assert_eq!(lock.next_owner(), Some(2));
        assert_eq!(lock.queue_len(), 1);

        lock.test_release(s2);
        assert_eq!(lock.queue_len(), 0);
        assert_eq!(lock.next_owner(), None);
    }

    #[test]
    fn khong_reentrant() {
        // Khoá **không** reentrant: xếp hàng lần hai trên cùng một luồng **không** được acquire
        // ngay mà phải chờ lần giữ đầu mở khoá (gọi `lock()` thật sẽ treo — đúng nghĩa).
        let lock = McsLock::new(7u8);
        let guard = lock.lock();
        assert_eq!(*guard, 7);

        let (slot, acquired) = lock.test_enqueue();
        assert!(!acquired, "không reentrant: lần xếp hàng thứ hai phải chờ");
        assert!(lock.try_lock().is_none(), "try_lock cũng phải trả None");
        assert!(!lock.test_woken(slot));
        assert_eq!(lock.queue_len(), 2);

        drop(guard);
        assert!(lock.test_woken(slot), "chỉ sau khi mở khoá mới được trao");
        lock.test_release(slot);
        assert_eq!(lock.queue_len(), 0);
        assert_eq!(lock.next_owner(), None);
    }

    #[test]
    fn pool_can_muoi_sau_slot_thi_try_lock_none_va_khong_panic() {
        let lock = McsLock::new(());
        let (s0, acquired) = lock.test_enqueue();
        assert!(acquired);
        let mut slots = [0usize; MAX_WAITERS];
        slots[0] = s0;
        for (i, slot) in slots.iter_mut().enumerate().skip(1) {
            let (claimed, acquired) = lock.test_enqueue();
            assert!(!acquired, "slot {i}: đã có chủ nên phải xếp hàng");
            *slot = claimed;
        }
        assert_eq!(lock.queue_len(), MAX_WAITERS, "pool đã cạn");
        // Pool cạn: try_lock **không** chờ slot, trả None (không panic, không kẹt).
        assert!(lock.try_lock().is_none(), "hết slot ⇒ try_lock None");
        assert_eq!(
            lock.queue_len(),
            MAX_WAITERS,
            "try_lock thất bại không đổi hàng"
        );

        // Mở khoá lần lượt theo FIFO: mỗi lần trao đúng node kế tiếp, cuối cùng trả hết slot.
        for (i, slot) in slots.iter().enumerate() {
            if i > 0 {
                assert!(
                    lock.test_woken(*slot),
                    "slot {i} phải được trao sau slot {}",
                    i - 1
                );
            }
            lock.test_release(*slot);
        }
        assert_eq!(lock.queue_len(), 0);
        assert_eq!(lock.next_owner(), None);
        let guard = lock.try_lock();
        assert!(guard.is_some(), "pool hồi phục sau khi mở hết khoá");
        drop(guard);
        assert_eq!(lock.queue_len(), 0);
    }
}
