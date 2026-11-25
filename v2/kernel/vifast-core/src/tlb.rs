// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`TlbFlush` thật** — KT-64: core phát yêu cầu xả TLB tới **hart đang chạy** và **chờ đủ
//! ack trước khi cho TCB trên hart đó chạy lại**; một hart vẫn **luôn** flush cục bộ.
//!
//! # Vì sao phải có hook (D2-83)
//!
//! Core **không** sở hữu `Frame`/`PageTable`/`VSpace` và **không** chạm `sfence.vma` (D2-17:
//! `unsafe` chỉ ở `kernel/vifast-plat-*`). Object layer (KT-52) sinh
//! `FlushRequest { Page/Asid/All }` khi `map`/`unmap`/`protect`/teardown; arch (KT-51) có
//! `tlb::shootdown`/`active_on()` thật. Cầu nối **một chiều** ở đây là trait [`TlbEnv`]:
//! core quyết định **ai** phải flush, **thứ tự** và **chờ ack**; arch làm `sfence.vma` +
//! trả `active_on()`.
//!
//! # Ngữ nghĩa (giữ tinh thần sim KT-40 + KT-51, D2-78)
//!
//! - **Flush cục bộ luôn xảy ra** trên hart gọi, kể cả mô hình 1 hart (không có “flush hộ”).
//! - Chỉ gửi `MailboxMsg::TlbFlush` tới hart trong `active_on` **khác** hart gọi
//!   (`active_on & !hart_bit(caller)` — cùng công thức `tlb::targets`, D2-68/D2-78); hart
//!   online **ngoài** `active_on` không bị post/không bị chờ.
//! - **Anti-hang / không ack hộ:** hart đang chờ **chỉ** rút hộp thư của mình; ack của hart
//!   khác chỉ đến khi hart đó chạy `tlb_service` (D2-92b). Quá `MAILBOX_CAP + 2` vòng mà vẫn
//!   thiếu ack ⇒ `Busy` (không treo, không trả `Ok` giả; `pending` giữ nguyên). Đang có
//!   shootdown chưa xong ⇒ `begin` thứ hai trả `Busy` (không đè lệnh chưa ack).
//! - **Trước khi TCB khác chạy lại:** [`Machine::dispatch`] trên hart còn `TlbFlush` chưa xử lý
//!   trả `Busy`; hart phải gọi [`Machine::tlb_service`] (flush thật + ack) rồi mới `dispatch`.
//!   Đây là bất biến **core** (sim gộp TLB vào chính nó nên không có cửa này) — test core-only
//!   + TAP QEMU chứng minh bằng page fault thật sau `unmap`.
//!
//! # Không mô hình hoá phần cứng
//!
//! Ở đây **không** có mảng entry TLB, LRU, hit/miss: bộ nhớ đệm đó là của CPU. Đại lượng mô
//! hình được giữ đúng là `(seq, asid, va, scope)` + tập hart phải ack — đủ để chứng minh
//! “không còn trang cũ đọc được sau unmap” trên QEMU (TAP `# tlb:`).

use crate::error::KError;
use crate::machine::{Machine, MAX_HARTS};
use crate::mailbox::{FlushMsg, MailboxMsg, MAILBOX_CAP};
use crate::observer::CoreObserver;
use crate::store::KernelStore;

/// Phạm vi xả TLB — cùng ba mức của `vifast_plat_common::obj::FlushRequest`/`tlb::FlushScope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlbScope {
    /// `sfence.vma va, asid` — một VA (map/unmap/protect).
    Page {
        /// VA vừa đổi PTE.
        va: u64,
    },
    /// `sfence.vma zero, asid` — teardown/đổi root VSpace của một ASID.
    Asid,
    /// `sfence.vma` — mọi ASID (không có nhãn).
    All,
}

impl TlbScope {
    /// Mã ổn định (log/digest, không phải ABI).
    pub const fn code(self) -> u8 {
        match self {
            TlbScope::Page { .. } => 0,
            TlbScope::Asid => 1,
            TlbScope::All => 2,
        }
    }

    /// Tên ổn định.
    pub const fn name(self) -> &'static str {
        match self {
            TlbScope::Page { .. } => "Page",
            TlbScope::Asid => "Asid",
            TlbScope::All => "All",
        }
    }
}

/// Hook arch cho `TlbFlush` — **arch cài** (`kernel/vifast-plat-riscv64`); core không `unsafe`.
///
/// Hợp đồng:
/// - [`TlbEnv::active_on`] trả bitmask hart **đang chạy** VSpace bị đổi (`tlb::active_on()` của
///   KT-51); hart gọi **phải** nằm trong mask, nếu không `begin`/`flush` trả
///   `IllegalOperation`.
/// - [`TlbEnv::flush_local`] thực thi `sfence.vma` **thật** trên hart đang chạy — arch được
///   phép `debug_assert!(hart == hart_id hiện tại)`; mô hình gọi nó cho hart đích **chỉ** khi
///   “hart đó chạy” ([`Machine::tlb_service`]); `wait` **không** flush hộ.
///   **Nếu đường multi-hart lọt vào kernel hiện tại** (`active_on` có hart khác) thì `assert`
///   này + `flush_local` sẽ nổ/`sfence` sai hart ⇒ lỗi lập trình, không phải “im lặng đúng”.
pub trait TlbEnv {
    /// Bitmask hart đang chạy VSpace bị đổi (`active_on`).
    fn active_on(&self) -> u64;
    /// Thế hệ ASID hiện tại (mang theo trong `FlushMsg`; không dùng để quyết định).
    fn generation(&self) -> u64;
    /// Xả TLB **thật** trên `hart` theo phạm vi.
    fn flush_local(&mut self, hart: u16, scope: TlbScope, asid: u16, va: u64);
}

/// Một shootdown **đang chờ** của một hart (`[TK]`, cùng vai trò `TlbPending` của sim KT-40).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TlbPending {
    /// Có shootdown đang chờ trên hart này.
    pub(crate) active: bool,
    /// Số hiệu toàn cục (1-based).
    pub(crate) seq: u64,
    /// Phạm vi cần xả.
    pub(crate) scope: TlbScope,
    /// ASID.
    pub(crate) asid: u16,
    /// VA (với `Page`).
    pub(crate) va: u64,
    /// Với **hart gọi**: bitmask hart đích phải ack; với hart đích: 0.
    pub(crate) targets: u64,
}

impl TlbPending {
    /// Mốc rỗng.
    pub(crate) const EMPTY: Self = Self {
        active: false,
        seq: 0,
        scope: TlbScope::All,
        asid: 0,
        va: 0,
        targets: 0,
    };
}

/// Kết quả một [`Machine::tlb_shootdown_begin`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlbShootdownReport {
    /// Số hiệu shootdown (1-based, tăng đơn điệu).
    pub seq: u64,
    /// `active_on` lúc bắt đầu.
    pub active_on: u64,
    /// Bitmask hart **phải ack** (khác hart gọi).
    pub targets: u64,
}

/// Kết quả [`Machine::tlb_flush`] — sau khi **đã chờ đủ** ack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlbFlushReport {
    /// Số hiệu shootdown.
    pub seq: u64,
    /// `active_on` lúc bắt đầu.
    pub active_on: u64,
    /// Bitmask hart đã phải ack.
    pub targets: u64,
    /// Bitmask hart đã ack (bằng `targets` khi `Ok`).
    pub acked: u64,
    /// `true` ⇔ hart gọi đã flush cục bộ (luôn `true`: kể cả 1 hart).
    pub local: bool,
}

/// `true` ⇔ `hart` nằm trong `mask` (hart ngoài dải `MAX_HARTS` ⇒ `false`).
pub(crate) const fn in_mask(mask: u64, hart: usize) -> bool {
    hart < MAX_HARTS && (mask & (1u64 << hart)) != 0
}

/// Bitmask hart phải ack: `active_on` trừ hart gọi — **đúng** công thức `tlb::targets` (D2-78).
pub(crate) const fn targets_of(active_on: u64, caller: usize) -> u64 {
    if caller >= MAX_HARTS {
        return active_on;
    }
    active_on & !(1u64 << caller)
}

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
    /// Số hiệu shootdown gần nhất đã cấp.
    pub fn tlb_seq(&self) -> u64 {
        self.tlb_seq
    }

    /// Số lần flush **cục bộ** đã thực thi (mọi hart; gồm cả hart đích khi phục vụ).
    pub fn tlb_local_flushes(&self) -> u64 {
        self.tlb_local_flushes
    }

    /// Số `TlbFlush` đã post **remote** (không tính flush cục bộ).
    pub fn tlb_remote_posts(&self) -> u64 {
        self.tlb_remote_posts
    }

    /// Số lần một hart đích đã phục vụ (`tlb_service`) — dấu ack thật.
    pub fn tlb_serviced(&self) -> u64 {
        self.tlb_serviced
    }

    /// Shootdown đang chờ của `hart` (`Some(seq)`), hoặc `None`.
    pub fn tlb_pending(&self, hart: u16) -> Result<Option<u64>, KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        let p = self.harts[hart as usize].tlb_pending;
        Ok(if p.active { Some(p.seq) } else { None })
    }

    /// **Bắt đầu** một shootdown: flush cục bộ **ngay** rồi post `TlbFlush` tới mọi hart trong
    /// `active_on` khác hart gọi; **không** chờ (xem [`Machine::tlb_shootdown_wait`]).
    ///
    /// Lỗi: `InvalidArgument` (hart ngoài dải), `IllegalOperation` (hart gọi **không** thuộc
    /// `active_on` — không được flush “hộ” VSpace khác), `Busy` (đang có shootdown chưa ack
    /// hoặc hộp thư hart đích đầy — **trước** mọi post nên không nửa chừng).
    pub fn tlb_shootdown_begin(
        &mut self,
        env: &mut dyn TlbEnv,
        scope: TlbScope,
        asid: u16,
        va: u64,
    ) -> Result<u64, KError> {
        let hart = self.current_hart;
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        let active_on = env.active_on();
        if !in_mask(active_on, hart as usize) {
            return Err(KError::IllegalOperation);
        }
        let targets = targets_of(active_on, hart as usize);
        // Đang có shootdown chưa xong (của hart gọi hoặc của một hart đích) ⇒ Busy, **không**
        // flush/post gì (giữ nguyên tử).
        for (h, p) in self.harts[..self.n_harts]
            .iter()
            .map(|h| &h.tlb_pending)
            .enumerate()
        {
            if p.active && (h == hart as usize || (targets & (1u64 << h)) != 0) {
                return Err(KError::Busy);
            }
        }
        // Hộp thư hart đích phải còn chỗ **trước** khi post (không nửa chừng).
        for (h, mb) in self.harts[..self.n_harts]
            .iter()
            .map(|h| &h.mail)
            .enumerate()
        {
            if (targets & (1u64 << h)) != 0 && mb.is_full() {
                return Err(KError::Busy);
            }
        }
        // Một hart vẫn phải flush **cục bộ** — luôn, trước khi báo cho hart khác.
        env.flush_local(hart, scope, asid, va);
        self.tlb_local_flushes = self.tlb_local_flushes.saturating_add(1);
        let seq = self.tlb_seq.saturating_add(1);
        self.tlb_seq = seq;
        let generation = env.generation();
        for h in 0..self.n_harts {
            if (targets & (1u64 << h)) == 0 {
                continue;
            }
            let msg = MailboxMsg::TlbFlush(FlushMsg {
                seq,
                ack: seq,
                generation,
                asid,
                va,
            });
            let _ = self.harts[h].mail.try_push(msg);
            self.harts[h].tlb_pending = TlbPending {
                active: true,
                seq,
                scope,
                asid,
                va,
                targets: 0,
            };
            self.tlb_remote_posts = self.tlb_remote_posts.saturating_add(1);
        }
        if targets != 0 {
            self.harts[hart as usize].tlb_pending = TlbPending {
                active: true,
                seq,
                scope,
                asid,
                va,
                targets,
            };
        }
        Ok(seq)
    }

    /// **Chờ đủ ack** của shootdown hart `hart` (idempotent: không có gì chờ ⇒ `Ok`).
    ///
    /// Mỗi vòng hart chờ **chỉ** rút hộp thư **của mình** (luật chống deadlock §4.10.3 của sim);
    /// **không** flush/ack hộ hart khác — ack của hart đích chỉ đến khi hart đó chạy
    /// [`Machine::tlb_service`] (D2-92b: đừng tự ack hộ). Quá `MAILBOX_CAP + 2` vòng ⇒ `Busy`:
    /// **không** treo, **không** `Ok` giả; `pending` giữ nguyên (fail-closed).
    pub fn tlb_shootdown_wait(&mut self, _env: &mut dyn TlbEnv, hart: u16) -> Result<(), KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        let p = self.harts[hart as usize].tlb_pending;
        if !p.active {
            return Ok(());
        }
        let max_rounds = MAILBOX_CAP as u64 + 2;
        let mut rounds = 0u64;
        loop {
            let mut missing = 0u64;
            for (h, hp) in self.harts[..self.n_harts]
                .iter()
                .map(|h| &h.tlb_pending)
                .enumerate()
            {
                if (p.targets & (1u64 << h)) != 0 && hp.active && hp.seq == p.seq {
                    missing += 1;
                }
            }
            if missing == 0 {
                self.harts[hart as usize].tlb_pending.active = false;
                return Ok(());
            }
            // Hart chờ tự rút hộp thư **của mình** — ack của hart khác có thể đang nằm đó.
            let _ = self.process_mail(hart);
            rounds += 1;
            if rounds >= max_rounds {
                return Err(KError::Busy);
            }
        }
    }

    /// Một cửa: begin + wait, trả báo cáo (flush cục bộ đã xảy ra **trước** khi post).
    ///
    /// **Điều kiện:** dùng được khi `targets == 0` (kernel hiện tại — `active_on` giao hart đang
    /// chạy core) hoặc khi caller **đã** cho các hart đích chạy [`Machine::tlb_service`]. Nếu
    /// không, `wait` trả `Busy` thay vì treo (D2-92b — ack của hart khác không được tự sinh).
    pub fn tlb_flush(
        &mut self,
        env: &mut dyn TlbEnv,
        scope: TlbScope,
        asid: u16,
        va: u64,
    ) -> Result<TlbFlushReport, KError> {
        let hart = self.current_hart;
        let active_on = env.active_on();
        let targets = targets_of(active_on, hart as usize);
        let seq = self.tlb_shootdown_begin(env, scope, asid, va)?;
        self.tlb_shootdown_wait(env, hart)?;
        Ok(TlbFlushReport {
            seq,
            active_on,
            targets,
            acked: targets,
            local: true,
        })
    }

    /// **Hart `hart` phục vụ** `TlbFlush` đang chờ của mình: flush thật qua `env` rồi ack.
    ///
    /// Trả số shootdown đã phục vụ (0/1). Đây là cửa **duy nhất** để hart khác chạy lại sau
    /// flush: [`Machine::dispatch`] từ chối khi còn pending (xem tài liệu module).
    pub fn tlb_service(&mut self, env: &mut dyn TlbEnv, hart: u16) -> Result<usize, KError> {
        if hart as usize >= self.n_harts {
            return Err(KError::InvalidArgument);
        }
        let p = self.harts[hart as usize].tlb_pending;
        if !p.active {
            return Ok(0);
        }
        env.flush_local(hart, p.scope, p.asid, p.va);
        self.tlb_local_flushes = self.tlb_local_flushes.saturating_add(1);
        self.harts[hart as usize].tlb_pending.active = false;
        self.tlb_serviced = self.tlb_serviced.saturating_add(1);
        // Rút message `TlbFlush` vừa post khỏi hộp thư của hart (đã xử lý).
        let _ = self.process_mail(hart);
        Ok(1)
    }

    /// `true` ⇔ hart còn `TlbFlush` chưa phục vụ — [`Machine::dispatch`] trả `Busy` khi vậy.
    pub(crate) fn tlb_blocks_dispatch(&self, hart: u16) -> bool {
        hart as usize >= self.n_harts || self.harts[hart as usize].tlb_pending.active
    }
}
