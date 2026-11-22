// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-339** — "transcript" kernel-side của pha shell (chỉ ảnh `ktest`): nhận **hash FNV-1a** của
//! từng lần `DebugPutStr` và bật cờ khi chuỗi in ra **đúng** một mốc của shell.
//!
//! Vì sao cần: TAP 43 phải chứng minh shell **in** prompt + `help` + `echo` + `exit` bằng dữ liệu
//! kernel đọc, không tin lời chương trình (D2-102) — và không phụ thuộc hàng `EP_EVENTS` (message
//! của TCB thoát bị `Machine::exit` dọn theo `unlink_tcb_queues`). Hàm ở đây **thuần safe Rust**,
//! không thêm `unsafe` ngoài `asm.rs` (D2-17); chỉ 4 cờ `AtomicU64`.
//!
//! Mốc khớp **nguyên một** lần gọi `put` (shell in mỗi mốc bằng một `put`), nên so `(len, hash)`
//! là đủ; `hash` 64-bit nên xác suất trùng lặp không đáng kể và **không** dùng để bỏ qua kiểm khác.

use core::sync::atomic::{AtomicU64, Ordering};

/// Hằng FNV-1a 64-bit (`[TK]`, cùng hằng của `vifast-traceformat`).
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// Nguyên tố FNV-1a 64-bit.
const FNV_PRIME: u64 = 0x100_0000_01b3;

/// `shell>\n` — prompt.
pub const F_PROMPT: u64 = 1 << 0;
/// `# shell: help cmds=` — lệnh nội trú `help` chạy.
pub const F_HELP: u64 = 1 << 1;
/// `# shell: echo ` — lệnh nội trú `echo` chạy.
pub const F_ECHO: u64 = 1 << 2;
/// `# shell: exit code=` — lệnh `exit` chạy.
pub const F_EXIT: u64 = 1 << 3;
/// Cả bốn mốc.
pub const F_ALL: u64 = F_PROMPT | F_HELP | F_ECHO | F_EXIT;

/// FNV-1a 64-bit của một chuỗi — `const fn` để tính hash mốc lúc biên dịch.
pub const fn fnv(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        index += 1;
    }
    hash
}

/// Cờ đã thấy (bit `F_*`).
static FLAGS: AtomicU64 = AtomicU64::new(0);

/// Bật cờ nếu `(len, hash)` khớp **nguyên** một mốc.
pub fn observe_hash(hash: u64, len: u64) {
    const MARKERS: [(u64, u64, u64); 4] = [
        (F_PROMPT, 7, fnv(b"shell>\n")),
        (F_HELP, 19, fnv(b"# shell: help cmds=")),
        (F_ECHO, 14, fnv(b"# shell: echo ")),
        (F_EXIT, 21, fnv(b"# shell: exit code=0\n")),
    ];
    let mut seen = 0u64;
    let mut index = 0;
    while index < MARKERS.len() {
        let (flag, marker_len, marker_hash) = MARKERS[index];
        if len == marker_len && hash == marker_hash {
            seen |= flag;
        }
        index += 1;
    }
    if seen != 0 {
        FLAGS.fetch_or(seen, Ordering::AcqRel);
    }
}

/// Cờ đã thấy tới thời điểm gọi.
pub fn flags() -> u64 {
    FLAGS.load(Ordering::Acquire)
}
