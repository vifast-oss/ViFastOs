// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`SyscallEnv` của aarch64** (KT-338b) — phần **arch** của hợp đồng `vifast-core::syscall`
//! (D2-83: core không đọc/ghi bộ nhớ user, không giữ PTE/`satp`).
//!
//! Cùng khuôn `vifast-plat-riscv64::user::UserMemAccess` (KT-62 + KT-282 + KT-320), nhưng tối
//! giản đúng phạm vi ảnh này: **một** VSpace, **một** trang EL0, **một** trang stack user và
//! **một** trang IPC buffer cố định do kernel map trước khi `eret`.
//!
//! # Vì sao đọc được trang user mà **không** cần `SUM`-tương đương
//!
//! RISC-V phải bật `sstatus.SUM` (và tắt ngay) vì `SPP=1` ⇒ S-mode không đọc trang `U`. AArch64
//! không có luật đó ở EL1: `SCTLR_EL1.SPAN = 1` (đặt trong chuỗi `MMU_ENABLE_SEQUENCE`) nghĩa là
//! **không** mask `PSTATE.PAN` khi vào exception ⇒ EL1 đọc được trang `AP=EL0` bằng VA của
//! `TTBR0`. Ảnh này **không** bật `PSTATE.PAN` (bit 22) nên đường đọc là hợp lệ; nếu ai bật PAN,
//! lần đọc đầu tiên sẽ data-abort (`# trap: … ec=0x24`) chứ **không** im lặng trả 0.
//!
//! # Ba trang user được phép (fail-closed)
//!
//! [`crate::umode::EL0_CODE_VA`] (mã), [`crate::umode::EL0_STACK_VA`] (stack) và trang IPC buffer
//! do ảnh đăng ký bằng [`set_ipc_buffer_va`]. Mọi con trỏ khác ⇒ `NotMapped` **11** — không đọc
//! bừa (một con trỏ EL0 sai không được biến thành page fault S-mode làm chết máy).
//!
//! `unsafe` **không** có ở đây: mọi phép đọc đi qua [`crate::asm::read_u8`]/[`crate::asm::read_u64`]
//! (module duy nhất của crate được `allow(unsafe_code)`, D2-17).

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_core::ipc::{
    IpcConfigureParams, IpcExtraCaps, IpcHeader, IPC_BUFFER_CONFIGURE_OFFSET,
    IPC_BUFFER_CONFIGURE_WORDS, IPC_BUFFER_EXTRA_CAPS_OFFSET, IPC_BUFFER_EXTRA_CAPS_READ_WORDS,
    IPC_BUFFER_MR_OFFSET, IPC_HEADER_MAGIC, IPC_HEADER_WORDS, IPC_HEADER_W_INFO,
    IPC_HEADER_W_SEND_CAPS, IPC_HEADER_W_WINDOW, WORDS_IN_IPC_BUFFER,
};
use vifast_core::syscall::{SyscallEnv, VspaceOutcome, FEATURE_DEBUG_HALT, FEATURE_DEBUG_PUT_STR};
use vifast_core::{KError, ObjId};

use crate::umode::{EL0_CODE_VA, EL0_STACK_VA};
use crate::{asm, pl011};

/// `u64::MAX` = **chưa** map trang IPC buffer (mọi hook trả `None`).
const NO_BUFFER: u64 = u64::MAX;
/// `u32::MAX` = không có TCB nào chờ nạp điểm vào.
const NO_TCB: u32 = u32::MAX;

/// VA trang IPC buffer hiện hành (do ảnh đăng ký bằng [`set_ipc_buffer_va`] ngay trước `eret`).
static IPC_BUF_VA: AtomicU64 = AtomicU64::new(NO_BUFFER);

/// TCB đang chờ nạp điểm vào từ `Tcb.Resume` (D2-83: core giao `entry`, arch ghi `ELR_EL1`).
static RESUME_TCB: AtomicU32 = AtomicU32::new(NO_TCB);
/// Điểm vào mà `Tcb.Resume` đã giao cho arch.
static RESUME_PC: AtomicU64 = AtomicU64::new(0);

/// Đăng ký (hoặc xoá, `None`) trang IPC buffer mà hook `ipc_header`/`ipc_buffer_words` được đọc.
pub fn set_ipc_buffer_va(va: Option<u64>) {
    IPC_BUF_VA.store(va.unwrap_or(NO_BUFFER), Ordering::Release);
}

/// VA trang IPC buffer đã đăng ký (`None` = chưa map).
pub fn ipc_buffer_va() -> Option<u64> {
    let va = IPC_BUF_VA.load(Ordering::Acquire);
    (va != NO_BUFFER).then_some(va)
}

/// `true` ⇔ `[va, va+len)` nằm trọn trong một trong **ba** trang user đã đăng ký.
fn user_range_ok(va: u64, len: u64) -> bool {
    let Some(stop) = va.checked_add(len) else {
        return false;
    };
    let in_page = |base: u64| va >= base && stop <= base + 0x1000;
    if in_page(EL0_CODE_VA) || in_page(EL0_STACK_VA) {
        return true;
    }
    if let Some(buf) = ipc_buffer_va() {
        if in_page(buf) {
            return true;
        }
    }
    // **KT-339** — ảnh ELF K5 (`procsrv`/`shell`/`echo`) nằm ngoài ba trang cố định ở trên:
    // loader đăng ký **đúng** dải `[base, end)` của từng ảnh + stack + ring. Dải chưa đăng ký
    // vẫn `false` (fail-closed, cùng tinh thần KT-338b).
    crate::userload::registered_range_ok(va, len)
}

/// Đọc `words` word (8 B) tại `va` của **trang user đã đăng ký** — `None` ⇔ ngoài dải.
fn read_user_words(va: u64, words: &mut [u64]) -> Option<()> {
    let bytes = (words.len() as u64).checked_mul(8)?;
    if !user_range_ok(va, bytes) {
        return None;
    }
    for (index, slot) in words.iter_mut().enumerate() {
        *slot = asm::read_u64(va + (index as u64) * 8);
    }
    Some(())
}

/// `true` ⇔ dải `[va, va+len)` là trang user **đã đăng ký** (bằng chứng cho TAP).
pub fn user_addr_ok(va: u64, len: u64) -> bool {
    user_range_ok(va, len)
}

// ---------------------------------------------------------------------------------------------
// Tcb.Resume — điểm vào do core giao, arch giữ tới lần vào U-mode kế tiếp (D2-83)
// ---------------------------------------------------------------------------------------------

/// Giữ `entry` cho lần vào U-mode kế tiếp của **đúng** `tcb` (hook `SyscallEnv::tcb_resume`).
pub fn arm_resume(tcb: ObjId, entry: u64) {
    RESUME_PC.store(entry, Ordering::Release);
    RESUME_TCB.store(tcb, Ordering::Release);
}

/// Lấy + **xoá** điểm vào đã chờ cho `tcb` (`None` = không có, hoặc đang chờ TCB khác).
pub fn take_resume(tcb: ObjId) -> Option<u64> {
    if RESUME_TCB.load(Ordering::Acquire) != tcb {
        return None;
    }
    RESUME_TCB.store(NO_TCB, Ordering::Release);
    Some(RESUME_PC.swap(0, Ordering::AcqRel))
}

// ---------------------------------------------------------------------------------------------
// SyscallEnv
// ---------------------------------------------------------------------------------------------

/// Môi trường syscall của ảnh aarch64 — **zero-sized**, không giữ trạng thái riêng (trạng thái
/// nằm ở static kèm ghi chú, để không có hai nguồn sự thật).
pub struct Aarch64Env;

impl SyscallEnv for Aarch64Env {
    /// `DebugPutStr` (12) — đọc **đúng** `len` byte trong ba trang user đã đăng ký rồi in PL011.
    ///
    /// Trả `Ok(len)` khi in đủ; `NotMapped` **11** khi con trỏ ngoài dải (fail-closed — không đọc
    /// bừa để biến lỗi user thành page fault S-mode). Trần `MAX_DEBUG_PUT` (4096) do core kiểm
    /// **trước** khi gọi hook.
    fn put_str(&mut self, ptr: u64, len: u64) -> Result<u64, u64> {
        if len == 0 {
            return Ok(0);
        }
        if !user_range_ok(ptr, len) {
            return Err(u64::from(KError::NotMapped.id()));
        }
        // **KT-339** — transcript: hash FNV-1a của **đúng** các byte đã in (không đọc thêm gì).
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for index in 0..len {
            let byte = asm::read_u8(ptr + index);
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
            let _ = pl011::write_byte(byte);
        }
        #[cfg(feature = "ktest")]
        crate::k5cap::observe_hash(hash, len);
        Ok(len)
    }

    /// `WaitAddr`/`WakeAddr` (9/10) — ảnh này **chưa** cài: trả `NotMapped` **11** để core trả
    /// đúng mã lỗi ABI thay vì đoán `(ppn, offset)`. Bật khi có bảng trang per-TCB (Chu kỳ 3).
    fn resolve_addr(&mut self, _va: u64) -> Result<(u32, u32), u64> {
        Err(u64::from(KError::NotMapped.id()))
    }

    /// `ClockRead` (11) — đồng hồ **đơn điệu** của nền tảng: `CNTPCT_EL0`/`CNTFRQ_EL0` (generic
    /// timer của AArch64), **không** phải `rdtime` của RISC-V và không phải RTC.
    fn now_ns(&mut self) -> u64 {
        let freq = asm::read_cntfrq_el0();
        if freq == 0 {
            return 0;
        }
        let ticks = asm::read_cntpct_el0();
        (ticks / freq) * 1_000_000_000 + ((ticks % freq) * 1_000_000_000) / freq
    }

    /// `DebugGetInfo.features` — bitmap **runtime** của ảnh này: chỉ `DebugPutStr` (bit 0) +
    /// `DebugHalt` (bit 1). **Không** bật `WaitAddr` (hook trả 11) hay `MCS_YIELD` (không có
    /// scheduler U-mode) — quảng cáo tính năng mình không có là nói dối.
    fn features(&mut self) -> u64 {
        FEATURE_DEBUG_PUT_STR | FEATURE_DEBUG_HALT
    }

    /// `DebugGetInfo.num_harts` — **số hart nền tảng** (D2-88(a)), không phải số hart model: đọc
    /// số ảnh đã đo lúc `core_bridge::init` (`smpboot` mang hart lên trước khi dựng core).
    fn num_harts(&mut self) -> u64 {
        crate::core_bridge::harts() as u64
    }

    /// `Tcb.Resume` (12) — giữ `entry` để ảnh nạp `ELR_EL1` ở lần vào U-mode **kế tiếp** của
    /// đúng TCB (`take_resume` xoá — một lần). Ảnh **có** U-mode nên trả `true`.
    fn tcb_resume(&mut self, tcb: ObjId, entry: u64) -> bool {
        arm_resume(tcb, entry);
        true
    }

    /// `Tcb.Configure` tham số `vspace?` — **D2-242 nhóm 3 (3.1A)** trả ba trạng thái.
    ///
    /// Ảnh aarch64 của `KT-338b` có **một** VSpace (của ảnh đã nạp): `vspace = 0` (xoá/không
    /// đổi) là `Applied`; `vspace != 0` thì arch **không giữ được** danh tính ⇒
    /// [`VspaceOutcome::Failed`] với **`Busy` 14** — core **rollback** `TcbData.vspace` rồi trả
    /// đúng mã lỗi (fail-closed). Đây là cùng ngữ nghĩa "bảng khe đầy" mà riscv64 dùng; khác biệt
    /// **nền tảng**, không phải nới ngưỡng. `KT-339` (shell sản phẩm) sẽ mở bảng khe thật.
    fn tcb_vspace(&mut self, _tcb: ObjId, vspace: ObjId) -> VspaceOutcome {
        if vspace == 0 {
            VspaceOutcome::Applied
        } else {
            VspaceOutcome::Failed(KError::Busy)
        }
    }

    /// **KT-67b/KT-311** — header **IPC buffer** của TCB gọi, đọc tại trang đã đăng ký
    /// ([`ipc_buffer_va`]). `None` ⇔ chưa đăng ký hoặc magic 0 (ảnh không dùng header ⇒ hành vi
    /// cũ). Cùng layout 10 word + vùng extra caps (560) + vùng `Tcb.Configure` (608) như
    /// `vifast-core::ipc` — **không** chép layout, chỉ là bản đọc.
    fn ipc_header(&mut self, _tcb: ObjId) -> Option<IpcHeader> {
        let va = ipc_buffer_va()?;
        let mut words = [0u64; IPC_HEADER_WORDS];
        read_user_words(va, &mut words)?;
        if words[0] != IPC_HEADER_MAGIC {
            return None;
        }
        let mut extra_words = [0u64; IPC_BUFFER_EXTRA_CAPS_READ_WORDS];
        read_user_words(va + IPC_BUFFER_EXTRA_CAPS_OFFSET as u64, &mut extra_words)?;
        let mut configure_words = [0u64; IPC_BUFFER_CONFIGURE_WORDS];
        read_user_words(
            va + IPC_BUFFER_CONFIGURE_OFFSET as u64,
            &mut configure_words,
        )?;
        Some(IpcHeader {
            info: words[IPC_HEADER_W_INFO],
            recv_window: [
                words[IPC_HEADER_W_WINDOW],
                words[IPC_HEADER_W_WINDOW + 1],
                words[IPC_HEADER_W_WINDOW + 2],
                words[IPC_HEADER_W_WINDOW + 3],
            ],
            send_caps: [
                words[IPC_HEADER_W_SEND_CAPS],
                words[IPC_HEADER_W_SEND_CAPS + 1],
                words[IPC_HEADER_W_SEND_CAPS + 2],
                words[IPC_HEADER_W_SEND_CAPS + 3],
            ],
            extra: IpcExtraCaps::from_words(&extra_words),
            configure: IpcConfigureParams::from_words(&configure_words),
        })
    }

    /// **KT-282** — `MR4..MR63` trong IPC buffer tại **VA đã bind** (`va` do core truyền, lấy từ
    /// `Machine::tcb_ipc_buffer_binding`). `None` ⇔ VA **khác** trang đã đăng ký hoặc ngoài dải —
    /// fail-closed: core trả `InvalidArgument` 5 cho message `length > 4` thay vì đọc bừa.
    fn ipc_buffer_words(&mut self, _tcb: ObjId, va: u64) -> Option<[u64; WORDS_IN_IPC_BUFFER]> {
        let registered = ipc_buffer_va()?;
        if va != registered {
            return None;
        }
        let mut words = [0u64; WORDS_IN_IPC_BUFFER];
        read_user_words(va + IPC_BUFFER_MR_OFFSET as u64, &mut words)?;
        Some(words)
    }
}

// ---------------------------------------------------------------------------------------------
// Ghi hằng của buffer ra `# core ipc:` (bằng chứng layout khớp `spec/` — không chép tay)
// ---------------------------------------------------------------------------------------------

/// `(mr_offset, extra_offset, configure_offset)` — đọc từ **hằng của core** (sinh từ `spec/`),
/// in vào log để `check_abi.py`/runner đối chiếu được mà không cần tin lời văn.
pub const LAYOUT_EVIDENCE: (usize, usize, usize) = (
    IPC_BUFFER_MR_OFFSET,
    IPC_BUFFER_EXTRA_CAPS_OFFSET,
    IPC_BUFFER_CONFIGURE_OFFSET,
);

#[cfg(test)]
mod tests {
    use super::*;

    /// Dải user phải **fail-closed**: con trỏ ngoài ba trang đã đăng ký ⇒ `false`.
    #[test]
    fn dai_user_fail_closed() {
        assert!(user_range_ok(EL0_CODE_VA, 12));
        assert!(user_range_ok(EL0_STACK_VA, 8));
        // `EL0_CODE_VA + 0x1000` **chính là** `EL0_STACK_VA` (trang kế) — phải ĐẠT; ra ngoài cả
        // hai trang mới ĐỎ.
        assert!(user_range_ok(EL0_CODE_VA + 0x1000, 8));
        assert!(!user_range_ok(EL0_STACK_VA + 0x1000, 8));
        assert!(!user_range_ok(0, 8));
        assert!(!user_range_ok(u64::MAX - 4, 8));
    }

    /// Đăng ký/xoá trang IPC buffer: hook phải theo **đúng** trạng thái đăng ký.
    #[test]
    fn dang_ky_ipc_buffer() {
        set_ipc_buffer_va(None);
        assert_eq!(ipc_buffer_va(), None);
        assert!(!user_range_ok(0x7000_2000, 8));
        set_ipc_buffer_va(Some(0x7000_2000));
        assert_eq!(ipc_buffer_va(), Some(0x7000_2000));
        assert!(user_range_ok(0x7000_2000 + 0x80, 8));
        set_ipc_buffer_va(None);
    }

    /// `tcb_resume` giữ đúng **một** lần cho đúng TCB: `take_resume` xoá, TCB khác trả `None`.
    #[test]
    fn resume_tra_dung_tcb() {
        arm_resume(7, 0x1234);
        assert_eq!(take_resume(8), None);
        assert_eq!(take_resume(7), Some(0x1234));
        assert_eq!(take_resume(7), None);
    }

    /// Layout IPC buffer in ra log phải khớp `spec/abi.toml` `[ipc_buffer]` (80 / 560 / 608).
    #[test]
    fn layout_khop_spec() {
        assert_eq!(LAYOUT_EVIDENCE, (80, 560, 608));
    }
}
