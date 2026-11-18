// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-70 — bố cục chung của pha pager: CSpace slot + VA + bảng trang client + khối kết quả.
//!
//! Ba chương trình U-mode dùng chung tệp này (mỗi ELF biên dịch riêng, `#[path]`), nên hằng
//! CSpace/VA/thứ tự trang là **một nguồn** — kernel rig ở `ktest_vfs.rs` chép đúng bảng này.
//!
//! Kernel test biên dịch cùng tệp (một số hằng chỉ dùng ở ảnh U-mode) ⇒ cho phép dead_code ở
//! phạm vi module này; ảnh U-mode vẫn dùng đúng các hằng của nó.
#![allow(dead_code)]

// VA của khối kernel cấp nằm **trong cùng 2 MiB block** với ảnh ELF (0x4000_0000..0x401F_FFFF) để
// pager/client không phải cấp thêm bảng trang (arena 64 KiB là tài nguyên hẹp — không nới ngưỡng).
/// VA khối **stats** trong VSpace pager (Frame kernel cấp).
pub const VA_STATS: u64 = 0x4003_0000;
/// VA **pool Frame** trong VSpace pager (`VA_POOL + slot * 4096`).
pub const VA_POOL: u64 = 0x4005_0000;
/// VA **đĩa RAM** (5 Frame 4 KiB liên tiếp = 40 sector) trong VSpace pager.
pub const VA_DISK: u64 = 0x4009_0000;
/// Số Frame 4 KiB của đĩa RAM = `DISK_SECTORS × 512 / 4096` (5 trang mẫu).
pub const DISK_FRAMES: u64 = 5;
/// VA khối **kết quả** của client (Frame kernel cấp) — trong block 0x4000_0000.
pub const VA_RESULT: u64 = 0x401E_0000;
// Trang của client được map **trong cùng 2 MiB block** với ảnh ELF (0x4000_0000..0x401F_FFFF) —
// không cần bảng trang mới (arena 64 KiB là tài nguyên hẹp, không nới ngưỡng).
/// Đáy vùng VA của **file 0** trong client A (trang `p` ⇒ `A_BASE + p*4096`).
pub const A_BASE: u64 = 0x4010_0000;
/// Đáy vùng VA của **file 0** trong client B.
pub const B_BASE: u64 = 0x4018_0000;
/// Đáy vùng VA của **file 1** trong client B (tách khỏi file 0).
pub const B_FILE1_BASE: u64 = 0x401B_0000;
/// VA **cố định** cho fault trang ngoài file của client B (page 99 không thể nhân `page*4096`).
pub const B_PROBE_VA: u64 = 0x401D_0000;
/// Đáy vùng VA của **client C** (KT-157) — 2 trang file 0, tách khỏi A/B và khỏi IPC buffer
/// (`0x4002_0000`), pool pager (`0x4005_0000`) và đĩa pager (`0x4009_0000`).
pub const C_BASE: u64 = 0x4006_0000;
/// Đáy vùng VA của **client D** (KT-157) — 2 trang file 0, tách khỏi C.
pub const D_BASE: u64 = 0x4008_0000;

/// Cờ `Frame.Map`: `R|U` (đọc được từ U-mode).
pub const FLAGS_RU: u64 = 1 | 8;
/// Cờ `Frame.Map`: `R|W|U` (khối stats/kết quả).
pub const FLAGS_RWU: u64 = 1 | 2 | 8;

// ---------------------------------------------------------------------------------------------
// CSpace (khớp `ktest_vfs.rs`)
// ---------------------------------------------------------------------------------------------

/// CSpace pager — 32 slot (slot 0 = cap **CNode** của chính pager, không phải TCB).
pub const P_SELF: u64 = 0;
/// Endpoint nhận `fault`/`stop` từ kernel.
pub const P_EP_FAULT: u64 = 1;
/// Endpoint gửi `ready`/`served`/`finish` về kernel.
pub const P_EP_REPLY: u64 = 2;
/// Cap VSpace của client A (pager map hộ).
pub const P_VS_A: u64 = 3;
/// Cap VSpace của client B.
pub const P_VS_B: u64 = 4;
/// Cap VSpace của client C (KT-157).
pub const P_VS_C: u64 = 16;
/// Cap VSpace của client D (KT-157).
pub const P_VS_D: u64 = 17;
/// Cap Frame pool đầu tiên (slot `P_POOL0 + i`).
pub const P_POOL0: u64 = 5;
/// Cap Frame stats.
pub const P_STATS: u64 = 9;
/// Cap VSpace của chính pager (map stats + pool + đĩa).
pub const P_VSPACE: u64 = 10;
/// Cap Frame đĩa RAM đầu tiên (slot `P_DISK0 + i`, 5 Frame 4 KiB).
pub const P_DISK0: u64 = 11;

/// CSpace client — 16 slot.
pub const C_SELF: u64 = 0;
/// Cap Frame kết quả.
pub const C_RESULT: u64 = 1;
/// Cap VSpace của chính client.
pub const C_VSPACE: u64 = 2;

/// Số client dùng pager (KT-157: **4** — A/B/C/D, mỗi client một VSpace + CSpace riêng).
pub const CLIENTS: u32 = 4;

// ---------------------------------------------------------------------------------------------
// Bảng trang client (thứ tự **chính xác** — kernel rig dùng cùng bảng để quy VA → (file,page))
// ---------------------------------------------------------------------------------------------

/// Client A: 3 trang file 0 (làm đầy cache 3), rồi nạp lại p0 (sau khi B trục xuất) và p2
/// (sau khi B trục xuất tiếp).
pub const A_TABLE: [(u32, u32); 5] = [(0, 0), (0, 1), (0, 2), (0, 0), (0, 2)];
/// Client B: trang 2 file 0 (**hit cache** — cùng PA với client A), trang 0/1 file 1 (miss),
/// trang 2 file 0 lần nữa (**hit lần hai** sau khi A nạp lại), rồi 1 trang **ngoài file**.
pub const B_TABLE: [(u32, u32); 5] = [(0, 2), (1, 0), (1, 1), (0, 2), (0, 99)];
/// Client C (KT-157): 2 trang file 0 tại VA riêng `C_BASE` — mỗi trang fault **đúng một** lần.
pub const C_TABLE: [(u32, u32); 2] = [(0, 0), (0, 1)];
/// Client D (KT-157): 2 trang file 0 tại VA riêng `D_BASE` — mỗi trang fault **đúng một** lần.
/// Dùng page 0/1 (khác **VA** với C nhờ `D_BASE` khác) để `page_of` chỉ cần một dải liên tục.
pub const D_TABLE: [(u32, u32); 2] = [(0, 0), (0, 1)];

/// VA của một trang trong bảng của client A.
pub const fn a_va(page: u32) -> u64 {
    A_BASE + page as u64 * 4096
}

/// VA của một trang trong bảng của client C.
pub const fn c_va(page: u32) -> u64 {
    C_BASE + page as u64 * 4096
}

/// VA của một trang trong bảng của client D.
pub const fn d_va(page: u32) -> u64 {
    D_BASE + page as u64 * 4096
}

/// VA của một trang trong bảng của client B; trang ngoài file dùng VA cố định `B_PROBE_VA`.
pub const fn b_va(file: u32, page: u32) -> u64 {
    if file == 1 {
        B_FILE1_BASE + page as u64 * 4096
    } else if page >= vifast_vfs::FILE0_PAGES {
        B_PROBE_VA
    } else {
        B_BASE + page as u64 * 4096
    }
}

// ---------------------------------------------------------------------------------------------
// Khối kết quả (Frame riêng của từng client; kernel đọc qua physmap)
// ---------------------------------------------------------------------------------------------

/// Magic khối kết quả client A.
pub const RESULT_MAGIC_A: u64 = 0x4b54_3730_434c_4941; // "KT70CLIA"
/// Magic khối kết quả client B.
pub const RESULT_MAGIC_B: u64 = 0x4b54_3730_434c_4942; // "KT70CLIB"
/// Offset các trường trong khối kết quả (u64).
pub mod result {
    /// Magic (khác nhau theo client).
    pub const MAGIC: u64 = 0;
    /// Số trang kiểm đúng.
    pub const OK_PAGES: u64 = 1;
    /// Số trang có byte sai.
    pub const BAD_PAGES: u64 = 2;
    /// Số byte đúng.
    pub const OK_BYTES: u64 = 3;
    /// Số byte sai.
    pub const BAD_BYTES: u64 = 4;
    /// Trang cuối đã kiểm (file).
    pub const LAST_FILE: u64 = 5;
    /// Trang cuối đã kiểm (page).
    pub const LAST_PAGE: u64 = 6;
    /// Offset sai đầu tiên (`u64::MAX` nếu không có).
    pub const FIRST_BAD: u64 = 7;
    /// `1` ⇔ đã chạy hết bảng (client A) — client B không đặt vì dừng ở ca lỗi.
    pub const DONE: u64 = 8;
    /// Số lần `Yield` (nhịp scheduler) — bằng chứng xen kẽ.
    pub const YIELDS: u64 = 9;
}
