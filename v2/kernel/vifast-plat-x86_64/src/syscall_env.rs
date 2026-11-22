// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`SyscallEnv` của x86_64** (KT-340c) — phần **arch** của hợp đồng `vifast-core::syscall`
//! (D2-83: core không đọc/ghi bộ nhớ user, không giữ PTE/`CR3`).
//!
//! # Khuôn
//!
//! `vifast-plat-aarch64/src/syscall_env.rs` (KT-338b) + `vifast-plat-riscv64/src/user.rs`
//! (KT-62/KT-282/KT-320). Phạm vi đúng ảnh này: **một** `VSpace` động (hoặc cây user tĩnh của
//! `umode::run`), **một** trang mã, **một** trang stack và **một** trang IPC buffer do kernel map
//! trước khi `iretq`.
//!
//! # Vì sao đọc được trang user mà **không** cần `STAC`/`CLAC`
//!
//! RISC-V phải bật `sstatus.SUM`; x86_64 chỉ chặn supervisor đọc trang `U/S = 1` khi
//! **`CR4.SMAP = 1`** (SDM Vol 3A §4.6). Ảnh này **không** bật SMAP (`CR4` của đường boot và của
//! `kmain` chỉ có `PAE` và các bit mặc định — xem `run-x86_64.sh` dòng `# viFastOs … cr4=`), nên
//! CPL0 đọc trang user bằng VA trực tiếp là **hợp lệ phần cứng**. Nếu ai bật SMAP sau này, lần
//! đọc đầu tiên sẽ `#PF` (`err = 0x…`) chứ **không** im lặng trả 0 — fail-loud.
//!
//! # Ba vùng user được phép (fail-closed)
//!
//! [`crate::umode::VSPACE_CODE_VA`] (mã), [`crate::umode::VSPACE_STACK_VA`] (stack) và trang IPC
//! buffer do ảnh đăng ký bằng [`set_ipc_buffer_va`]. Mọi con trỏ khác ⇒ `NotMapped` **11** —
//! không đọc bừa (một con trỏ ring 3 sai không được biến thành page fault kernel làm chết máy).
//!
//! `unsafe` **không** có ở đây: mọi phép đọc đi qua [`crate::asm::read_u8`]/[`crate::asm::read_u64`]
//! (module duy nhất của crate được `allow(unsafe_code)`, D2-17).

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use vifast_core::ipc::{
    IpcConfigureParams, IpcExtraCaps, IpcHeader, IPC_BUFFER_CONFIGURE_OFFSET,
    IPC_BUFFER_CONFIGURE_WORDS, IPC_BUFFER_EXTRA_CAPS_OFFSET, IPC_BUFFER_EXTRA_CAPS_READ_WORDS,
    IPC_BUFFER_MR_OFFSET, IPC_HEADER_MAGIC, IPC_HEADER_WORDS, IPC_HEADER_W_INFO,
    IPC_HEADER_W_SEND_CAPS, IPC_HEADER_W_WINDOW, WORDS_IN_IPC_BUFFER,
};
use vifast_core::syscall::{SyscallEnv, VspaceOutcome, FEATURE_DEBUG_HALT, FEATURE_DEBUG_PUT_STR};
use vifast_core::{KError, ObjId};

use crate::{asm, com, umode};

/// `u64::MAX` = **chưa** map trang IPC buffer (mọi hook trả `None`).
const NO_BUFFER: u64 = u64::MAX;
/// `u32::MAX` = không có TCB nào chờ nạp điểm vào.
const NO_TCB: u32 = u32::MAX;

/// VA trang IPC buffer hiện hành (do ảnh đăng ký bằng [`set_ipc_buffer_va`] ngay trước `iretq`).
static IPC_BUF_VA: AtomicU64 = AtomicU64::new(NO_BUFFER);

/// TCB đang chờ nạp điểm vào từ `Tcb.Resume` (D2-83: core giao `entry`, arch ghi `RIP`).
static RESUME_TCB: AtomicU32 = AtomicU32::new(NO_TCB);
/// Điểm vào mà `Tcb.Resume` đã giao cho arch.
static RESUME_PC: AtomicU64 = AtomicU64::new(0);

/// Mốc thô của PIT kênh 2 cho [`SyscallEnv::now_ns`] (đếm lùi, 16 bit).
static CLOCK_LAST: AtomicU64 = AtomicU64::new(0);
/// Tổng số count PIT đã trôi qua (bộ cộng dồn bọc).
static CLOCK_COUNTS: AtomicU64 = AtomicU64::new(0);

/// Đăng ký (hoặc xoá, `None`) trang IPC buffer mà hook `ipc_header`/`ipc_buffer_words` được đọc.
pub fn set_ipc_buffer_va(va: Option<u64>) {
    IPC_BUF_VA.store(va.unwrap_or(NO_BUFFER), Ordering::Release);
}

/// VA trang IPC buffer đã đăng ký (`None` = chưa map).
pub fn ipc_buffer_va() -> Option<u64> {
    let va = IPC_BUF_VA.load(Ordering::Acquire);
    (va != NO_BUFFER).then_some(va)
}

/// Số dải user **động** được đăng ký (KT-341: ảnh ELF của shell/procsrv + stack + ring).
pub const MAX_USER_RANGES: usize = 8;
/// Dải `[base, base+len)` đã map cho U-mode — nguồn sự thật cho hook đọc bộ nhớ user.
static RANGE_BASE: [AtomicU64; MAX_USER_RANGES] = [const { AtomicU64::new(0) }; MAX_USER_RANGES];
static RANGE_LEN: [AtomicU64; MAX_USER_RANGES] = [const { AtomicU64::new(0) }; MAX_USER_RANGES];
/// Số dải đã đăng ký (cấp phát tuyến tính, chỉ hart 0 lúc nạp).
static RANGE_COUNT: AtomicU32 = AtomicU32::new(0);

/// Đăng ký dải user `[base, base+len)` (loader/stack/ring gọi trước khi `iretq`).
/// Trả `false` nếu bảng đầy hoặc dải rỗng — caller coi là lỗi nạp (fail-closed).
pub fn register_user_range(base: u64, len: u64) -> bool {
    if len == 0 || base == 0 {
        return false;
    }
    let index = RANGE_COUNT.load(Ordering::Acquire) as usize;
    if index >= MAX_USER_RANGES {
        return false;
    }
    RANGE_BASE[index].store(base, Ordering::Release);
    RANGE_LEN[index].store(len, Ordering::Release);
    RANGE_COUNT.store((index + 1) as u32, Ordering::Release);
    true
}

/// Xoá bảng dải động (dùng cho test host / một lần nạp mới ở ảnh).
pub fn reset_user_ranges() {
    let count = RANGE_COUNT.swap(0, Ordering::AcqRel) as usize;
    let mut index = 0;
    while index < count && index < MAX_USER_RANGES {
        RANGE_BASE[index].store(0, Ordering::Release);
        RANGE_LEN[index].store(0, Ordering::Release);
        index += 1;
    }
}

/// `true` ⇔ `[va, va+len)` nằm trọn trong **một** dải user đã đăng ký.
///
/// Fail-closed: tràn số, đảo dải, hoặc chỉ **một phần** ngoài dải đều trả `false`. Ba vùng của
/// pha core KT-340c (`VSPACE_CODE_VA`/`VSPACE_STACK_VA`/IPC buffer) vẫn được nhận **không** cần
/// đăng ký; ảnh nạp ELF (KT-341) đăng ký thêm dải thật của chương trình.
fn user_range_ok(va: u64, len: u64) -> bool {
    let Some(stop) = va.checked_add(len) else {
        return false;
    };
    let in_page = |base: u64| va >= base && stop <= base.saturating_add(0x1000);
    if in_page(umode::VSPACE_CODE_VA) || in_page(umode::VSPACE_STACK_VA) {
        return true;
    }
    if let Some(buf) = ipc_buffer_va() {
        if in_page(buf) {
            return true;
        }
    }
    let count = RANGE_COUNT.load(Ordering::Acquire) as usize;
    let mut index = 0;
    while index < count && index < MAX_USER_RANGES {
        let base = RANGE_BASE[index].load(Ordering::Acquire);
        let range_len = RANGE_LEN[index].load(Ordering::Acquire);
        if base != 0 && va >= base && stop <= base.saturating_add(range_len) {
            return true;
        }
        index += 1;
    }
    false
}

/// `true` ⇔ dải `[va, va+len)` là trang user **đã đăng ký** (bằng chứng cho TAP).
pub fn user_addr_ok(va: u64, len: u64) -> bool {
    user_range_ok(va, len)
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

// ---------------------------------------------------------------------------------------------
// **KT-341** — capture byte `DebugPutStr` (kernel đếm, không tin lời U-mode — D2-102)
// ---------------------------------------------------------------------------------------------

/// Số mẫu chuỗi cần nhận diện (prompt + help + echo + dòng exit).
const CAPTURE_PATTERNS: usize = 4;
const PATTERN_PROMPT: usize = 0;
const PATTERN_HELP: usize = 1;
const PATTERN_ECHO: usize = 2;
const PATTERN_EXIT: usize = 3;

/// Các mẫu **nguyên văn** do `shell.rs` in ra (cùng nguồn 3 arch).
const CAPTURE_PATTERN_BYTES: [&[u8]; CAPTURE_PATTERNS] = [
    b"shell>\n",
    b"# shell: help cmds=6",
    b"# shell: echo same-source",
    b"# shell: exit code=0",
];

static CAPTURE_ON: AtomicBool = AtomicBool::new(false);
static CAPTURE_PUTS: AtomicU64 = AtomicU64::new(0);
static CAPTURE_BYTES: AtomicU64 = AtomicU64::new(0);
static CAPTURE_HASH: AtomicU64 = AtomicU64::new(FNV_OFFSET);
/// Độ dài prefix đã khớp của từng mẫu (0..len).
static CAPTURE_MATCH: [AtomicU32; CAPTURE_PATTERNS] =
    [const { AtomicU32::new(0) }; CAPTURE_PATTERNS];
/// Đã thấy trọn mẫu `i` chưa (bit `i`).
static CAPTURE_DONE: AtomicU32 = AtomicU32::new(0);

/// FNV-1a 64 offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64 prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Bắt đầu một lần đo: xoá bộ đếm + bộ khớp mẫu.
pub fn capture_reset() {
    CAPTURE_PUTS.store(0, Ordering::Release);
    CAPTURE_BYTES.store(0, Ordering::Release);
    CAPTURE_HASH.store(FNV_OFFSET, Ordering::Release);
    CAPTURE_DONE.store(0, Ordering::Release);
    for slot in CAPTURE_MATCH.iter() {
        slot.store(0, Ordering::Release);
    }
    CAPTURE_ON.store(true, Ordering::Release);
}

/// Tắt capture (không đếm thêm byte).
pub fn capture_off() {
    CAPTURE_ON.store(false, Ordering::Release);
}

/// Cập nhật bộ khớp mẫu với **một** byte vừa in (khớp prefix đơn giản — mẫu không chồng nhau).
fn capture_byte(byte: u8) {
    if !CAPTURE_ON.load(Ordering::Acquire) {
        return;
    }
    let hash = CAPTURE_HASH.load(Ordering::Relaxed);
    CAPTURE_HASH.store(
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME),
        Ordering::Relaxed,
    );
    CAPTURE_BYTES.fetch_add(1, Ordering::Relaxed);
    let mut done = CAPTURE_DONE.load(Ordering::Relaxed);
    let mut index = 0;
    while index < CAPTURE_PATTERNS {
        let pattern = CAPTURE_PATTERN_BYTES[index];
        let mut len = CAPTURE_MATCH[index].load(Ordering::Relaxed) as usize;
        if len >= pattern.len() {
            len = 0;
        }
        if byte == pattern[len] {
            len += 1;
        } else {
            len = usize::from(byte == pattern[0]);
        }
        if len == pattern.len() {
            done |= 1u32 << index;
            len = 0;
        }
        CAPTURE_MATCH[index].store(len as u32, Ordering::Relaxed);
        index += 1;
    }
    CAPTURE_DONE.store(done, Ordering::Relaxed);
}

/// Báo cáo capture của lần đo (kernel đọc — bằng chứng TAP 44/45).
#[derive(Clone, Copy, Debug)]
pub struct CaptureReport {
    /// Số lần `DebugPutStr` thành công.
    pub puts: u64,
    /// Tổng số byte đã in.
    pub bytes: u64,
    /// FNV-1a 64 của toàn bộ byte đã in.
    pub hash: u64,
    /// Đã thấy prompt `shell>`.
    pub prompt: bool,
    /// Đã thấy `# shell: help cmds=6`.
    pub help: bool,
    /// Đã thấy `# shell: echo same-source`.
    pub echo: bool,
    /// Đã thấy `# shell: exit code=0`.
    pub exit_line: bool,
}

/// Đọc báo cáo capture hiện tại.
pub fn capture_report() -> CaptureReport {
    let done = CAPTURE_DONE.load(Ordering::Acquire);
    CaptureReport {
        puts: CAPTURE_PUTS.load(Ordering::Acquire),
        bytes: CAPTURE_BYTES.load(Ordering::Acquire),
        hash: CAPTURE_HASH.load(Ordering::Acquire),
        prompt: done & (1 << PATTERN_PROMPT) != 0,
        help: done & (1 << PATTERN_HELP) != 0,
        echo: done & (1 << PATTERN_ECHO) != 0,
        exit_line: done & (1 << PATTERN_EXIT) != 0,
    }
}

// ---------------------------------------------------------------------------------------------
// Tcb.Resume — điểm vào do core giao, arch giữ tới lần vào ring 3 kế tiếp (D2-83)
// ---------------------------------------------------------------------------------------------

/// Giữ `entry` cho lần vào ring 3 kế tiếp của **đúng** `tcb` (hook `SyscallEnv::tcb_resume`).
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

/// Môi trường syscall của ảnh x86_64 — **zero-sized**, không giữ trạng thái riêng (trạng thái
/// nằm ở static kèm ghi chú, để không có hai nguồn sự thật).
pub struct X86_64Env;

impl SyscallEnv for X86_64Env {
    /// `DebugPutStr` (12) — đọc **đúng** `len` byte trong ba trang user đã đăng ký rồi in COM1.
    ///
    /// Trả `Ok(len)` khi in đủ; `NotMapped` **11** khi con trỏ ngoài dải (fail-closed — không đọc
    /// bừa để biến lỗi user thành page fault kernel). Trần `MAX_DEBUG_PUT` (4096) do core kiểm
    /// **trước** khi gọi hook. Khoá COM1 cho **cả** chuỗi (KT-316: một `DebugPutStr` không được
    /// để hart khác xen ký tự vào giữa).
    fn put_str(&mut self, ptr: u64, len: u64) -> Result<u64, u64> {
        if len == 0 {
            return Ok(0);
        }
        if !user_range_ok(ptr, len) {
            return Err(u64::from(KError::NotMapped.id()));
        }
        let _guard = com::COM1.lock();
        for index in 0..len {
            let byte = asm::read_u8(ptr + index);
            asm::com1_write_byte(byte);
            capture_byte(byte);
        }
        if CAPTURE_ON.load(Ordering::Acquire) {
            CAPTURE_PUTS.fetch_add(1, Ordering::Relaxed);
        }
        Ok(len)
    }

    /// `WaitAddr`/`WakeAddr` (9/10) — ảnh này **chưa** cài: trả `NotMapped` **11** để core trả
    /// đúng mã lỗi ABI thay vì đoán `(ppn, offset)`. Bật khi có bảng trang per-TCB (Chu kỳ 3).
    fn resolve_addr(&mut self, _va: u64) -> Result<(u32, u32), u64> {
        Err(u64::from(KError::NotMapped.id()))
    }

    /// `ClockRead` (11) — đồng hồ **đơn điệu** của nền tảng: PIT **kênh 2** (cùng nguồn
    /// `timebase::PitClock` mà driver mạng dùng), không phải TSC (chưa hiệu chuẩn) và không phải
    /// RTC. Bộ cộng dồn xử lý **bọc** 16 bit, nên giá trị không giảm.
    fn now_ns(&mut self) -> u64 {
        let raw = asm::read_pit2_latch() as u64;
        let last = CLOCK_LAST.swap(raw, Ordering::Relaxed);
        let delta = if raw <= last {
            last - raw
        } else {
            last + 0x1_0000 - raw
        };
        let counts = CLOCK_COUNTS.fetch_add(delta, Ordering::Relaxed) + delta;
        crate::timebase::elapsed_us(counts).saturating_mul(1000)
    }

    /// `DebugGetInfo.features` — bitmap **runtime** của ảnh này: chỉ `DebugPutStr` (bit 0) +
    /// `DebugHalt` (bit 1). **Không** bật `WaitAddr` (hook trả 11) hay `MCS_YIELD` (chưa có
    /// scheduler lấy từ model) — quảng cáo tính năng mình không có là nói dối.
    fn features(&mut self) -> u64 {
        FEATURE_DEBUG_PUT_STR | FEATURE_DEBUG_HALT
    }

    /// `DebugGetInfo.num_harts` — **số hart nền tảng** (D2-88(a)), không phải số hart model.
    fn num_harts(&mut self) -> u64 {
        crate::core_bridge::harts() as u64
    }

    /// `Tcb.Resume` (12) — giữ `entry` để ảnh nạp `RIP` ở lần vào ring 3 **kế tiếp** của đúng TCB
    /// (`take_resume` xoá — một lần). Ảnh **có** ring 3 nên trả `true`.
    fn tcb_resume(&mut self, tcb: ObjId, entry: u64) -> bool {
        arm_resume(tcb, entry);
        true
    }

    /// `Tcb.Configure` tham số `vspace?` — **D2-242 nhóm 3 (3.1A)** trả ba trạng thái.
    ///
    /// Ảnh x86_64 của `KT-340c` có `VSpace` **động** (KT-340b) nhưng **chưa** có bảng "một
    /// `VSpace` cho mỗi TCB" — cùng giới hạn mà aarch64 KT-338b ghi: `vspace = 0` (xoá/không đổi)
    /// là `Applied`; `vspace != 0` thì arch **không giữ được danh tính** ⇒
    /// [`VspaceOutcome::Failed`] với **`Busy` 14** — core **rollback** `TcbData.vspace` rồi trả
    /// đúng mã lỗi (fail-closed, ca (c) đo điều này). `KT-341` (shell sản phẩm) sẽ mở bảng khe
    /// thật; đây là **cùng ngữ nghĩa** với riscv64 chứ không phải nới ngưỡng.
    fn tcb_vspace(&mut self, _tcb: ObjId, vspace: ObjId) -> VspaceOutcome {
        if vspace == 0 {
            VspaceOutcome::Applied
        } else {
            VspaceOutcome::Failed(KError::Busy)
        }
    }

    /// **KT-67b/KT-311** — header **IPC buffer** của TCB gọi, đọc tại trang đã đăng ký
    /// ([`ipc_buffer_va`]). `None` ⇔ chưa đăng ký hoặc magic 0 (ảnh không dùng header ⇒ hành vi
    /// cũ). Layout 10 word + vùng extra caps (560) + vùng `Tcb.Configure` (608) **lấy từ**
    /// `vifast-core::ipc` — không chép số.
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
        set_ipc_buffer_va(None);
        assert!(user_range_ok(umode::VSPACE_CODE_VA, 12));
        assert!(user_range_ok(umode::VSPACE_STACK_VA, 8));
        assert!(!user_range_ok(0, 8));
        assert!(!user_range_ok(u64::MAX - 4, 8));
        // Trang kế sau stack **không** đăng ký (guard của bố cục VSpace).
        assert!(!user_range_ok(umode::VSPACE_STACK_VA + 0x1000, 8));
    }

    /// Đăng ký/xoá trang IPC buffer: hook phải theo **đúng** trạng thái đăng ký.
    #[test]
    fn dang_ky_ipc_buffer() {
        set_ipc_buffer_va(None);
        assert_eq!(ipc_buffer_va(), None);
        set_ipc_buffer_va(Some(0x0010_0005_0000));
        assert_eq!(ipc_buffer_va(), Some(0x0010_0005_0000));
        assert!(user_range_ok(0x0010_0005_0000 + 0x80, 8));
        set_ipc_buffer_va(None);
    }

    /// `tcb_resume` giữ đúng **một** lần cho đúng TCB: `take_resume` xoá, TCB khác `None`.
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
