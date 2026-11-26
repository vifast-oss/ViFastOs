// SPDX-License-Identifier: MIT OR Apache-2.0
//! **GDT 64-bit per-hart + descriptor TSS** — KT-261 (K5' bước 2) + **KT-278** (per-hart).
//!
//! Tệp này là **dữ liệu + bảng**: hình học GDT (7 entry × 8 byte), mã hoá descriptor, pseudo-
//! descriptor `GDTR` và hàm [`install`] nạp bảng bằng `lgdt` + `ltr`. Asm đặc quyền nằm ở
//! [`crate::asm`] (D2-17); tệp này **không** `unsafe`.
//!
//! # Vì sao bảng nằm ở đây chứ không ở asm của ảnh
//!
//! Ảnh `vifast-kbin-x86_64` **không** được có `unsafe` (luật R5). Một `static mut` trong ảnh là
//! `unsafe` khi ghi ⇒ bảng phải nằm ở crate `kernel/vifast-plat-*` (D2-17). Ở đây bảng là
//! `[AtomicU64; 7]`: ghi bằng `store` **an toàn**, đọc bằng `load` **an toàn**, và layout của
//! `AtomicU64` **đúng bằng** `u64` (cùng cỡ 8, cùng căn 8) nên `lgdt` đọc được từng entry như
//! một `u64` thường. Không có bản thứ hai của con số nào: asm của ảnh dùng chính các hằng ở đây
//! làm `const` operand.
//!
//! # KT-278 — vì sao GDT là **per-hart**
//!
//! Descriptor TSS chứa **base** của vùng TSS. Nếu bốn hart dùng chung một bảng GDT thì chỉ có
//! **một** descriptor TSS ⇒ `ltr` trên hart 2 sẽ nạp **cùng** TSS với hart 1 (đúng nợ của
//! KT-276). Vì vậy mỗi hart có **bảng GDT riêng** ([`GDT`] là `[[AtomicU64; 7]; MAX_HARTS]`) và
//! **GDTR riêng** ([`GDTR`]) — `lgdt` của hart này không đụng hart khác, và descriptor TSS của
//! hart `i` trỏ [`crate::tss::base`]`(i)`.
//!
//! Bố cục **giống hệt nhau** ở mọi hart (null, kernel code/data, user code/data, TSS) nên
//! selector `0x08`/`0x10`/`0x28` có **cùng nghĩa** trên mọi hart — điều kiện để bảng IDT **dùng
//! chung** (mọi ô `selector = 0x08`) vẫn hợp lệ sau khi hart phụ `lgdt` bảng riêng của nó.
//!
//! # Bố cục (Intel SDM Vol 3A §3.4.5, §3.5.1)
//!
//! | # | Nội dung | Selector | Access | Flags |
//! |---|---|---|---|---|
//! | 0 | null (bắt buộc) | — | `0x00` | `0x0` |
//! | 1 | kernel code 64-bit | `0x08` | `0x9A` | `0x2` (`L=1`, `D/B=0`) |
//! | 2 | kernel data | `0x10` | `0x92` | `0x0` |
//! | 3 | user **data** | `0x18` (`0x1B` với RPL 3) | `0xF2` | `0x0` |
//! | 4 | user **code** 64-bit | `0x20` (`0x23` với RPL 3) | `0xFA` | `0x2` |
//! | 5–6 | **TSS** (descriptor hệ thống 16 byte) | `0x28` | `0x89` | `0x0` |
//!
//! Ở long mode, **base và limit của đoạn code/data bị bỏ qua** (chỉ `access` + `L` có nghĩa), nên
//! limit để 0 là đúng — asm của ảnh đã dùng đúng các giá trị này từ KT-257. Descriptor **TSS**
//! thì ngược lại: limit **có** nghĩa (cỡ vùng TSS) và base phải trỏ đúng TSS.
//!
//! # Vì sao TSS là **hai** entry
//!
//! Descriptor hệ thống 64-bit dài 16 byte (SDM Hình 7-4 "Format of TSS and LDT Descriptors in
//! 64-bit Mode"): 8 byte đầu là descriptor "cổ điển" (limit/base/type), 8 byte sau là
//! `base[63:32]` + 4 byte dự phòng. Vì vậy bảng có **7** entry cho **6** thứ.

//! # Vì sao **user data đứng trước** user code (KT-417c)
//!
//! `SYSRET` **cứng** công thức selector (Intel SDM Vol 2B "SYSRET"): `CS = STAR[63:48] + 16` và
//! `SS = STAR[63:48] + 8`. Với `STAR[63:48] = 0x10` (kernel **data**) ⇒ cần `CS = 0x20` (user
//! code) và `SS = 0x18` (user data); tức **user data phải là entry ngay trước** user code.
//! Đó đúng là `[arch.x86_64.gdt.planned]` của `spec/kernel.toml`, và `check_kernel.py` đối chiếu
//! chéo: `sysret_ready = true` **bắt buộc** thứ tự hiện tại khớp `planned`.
//!
//! Đường `int 0x80` **không** phụ thuộc thứ tự này: nó nạp `CS`/`SS` từ **frame** do CPU đẩy
//! (selector nằm trong `CS`/`SS` của ring 3), nên hai cổng chạy song song trên cùng bảng.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::asm;
use crate::smp::MAX_HARTS;

/// Số entry của GDT: null + 2 kernel + 2 user + **2** cho descriptor TSS 16 byte.
pub const GDT_ENTRIES: usize = 7;
/// Cỡ bảng GDT tính bằng byte (7 × 8).
pub const GDT_BYTES: usize = GDT_ENTRIES * 8;
/// `GDTR.limit` = cỡ bảng − 1 (đặc tả `lgdt`: limit là **byte cuối**, không phải số byte).
pub const GDT_LIMIT: u16 = (GDT_BYTES - 1) as u16;

/// Chỉ số entry null (bắt buộc — nạp selector 0 phải #GP).
pub const INDEX_NULL: usize = 0;
/// Chỉ số entry kernel code 64-bit.
pub const INDEX_KERNEL_CODE: usize = 1;
/// Chỉ số entry kernel data.
pub const INDEX_KERNEL_DATA: usize = 2;
/// Chỉ số entry user **code** 64-bit — **entry 4** (sau user data) để `SYSRET` nạp đúng
/// `CS = STAR[63:48] + 16 = 0x20` (KT-417c).
pub const INDEX_USER_CODE: usize = 4;
/// Chỉ số entry user **data** — **entry 3** (ngay trước user code) để `SYSRET` nạp đúng
/// `SS = STAR[63:48] + 8 = 0x18` (KT-417c).
pub const INDEX_USER_DATA: usize = 3;
/// Chỉ số entry **đầu** của descriptor TSS (entry thứ hai là `INDEX_TSS + 1`).
pub const INDEX_TSS: usize = 5;

/// Selector kernel code (`INDEX_KERNEL_CODE × 8`, RPL 0).
pub const SELECTOR_KERNEL_CODE: u16 = (INDEX_KERNEL_CODE as u16) * 8;
/// Selector kernel data (`INDEX_KERNEL_DATA × 8`, RPL 0).
pub const SELECTOR_KERNEL_DATA: u16 = (INDEX_KERNEL_DATA as u16) * 8;
/// Selector user code (`INDEX_USER_CODE × 8`, RPL 0 — thêm [`RPL_USER`] khi dùng thật).
pub const SELECTOR_USER_CODE: u16 = (INDEX_USER_CODE as u16) * 8;
/// Selector user data (`INDEX_USER_DATA × 8`, RPL 0).
pub const SELECTOR_USER_DATA: u16 = (INDEX_USER_DATA as u16) * 8;
/// Selector TSS (`INDEX_TSS × 8`) — giá trị `ltr` nạp và `str` đọc lại.
pub const SELECTOR_TSS: u16 = (INDEX_TSS as u16) * 8;

/// `RPL` 3 — mức đặc quyền user (chưa dùng ở bước này: **không** U-mode).
pub const RPL_USER: u16 = 3;
/// Selector user code **dùng thật** (RPL 3) — `0x20 | 3 = 0x23` (KT-417c).
pub const SELECTOR_USER_CODE_RPL3: u16 = SELECTOR_USER_CODE | RPL_USER;
/// Selector user data **dùng thật** (RPL 3) — `0x18 | 3 = 0x1B` (KT-417c).
pub const SELECTOR_USER_DATA_RPL3: u16 = SELECTOR_USER_DATA | RPL_USER;

/// Access byte: kernel code 64-bit — `P=1`, `DPL=0`, `S=1`, `type=0xA` (code/exec/read).
pub const ACCESS_KERNEL_CODE: u8 = 0x9A;
/// Access byte: kernel data — `P=1`, `DPL=0`, `S=1`, `type=0x2` (data/read/write).
pub const ACCESS_KERNEL_DATA: u8 = 0x92;
/// Access byte: user code 64-bit — `P=1`, `DPL=3`, `S=1`, `type=0xA`.
pub const ACCESS_USER_CODE: u8 = 0xFA;
/// Access byte: user data — `P=1`, `DPL=3`, `S=1`, `type=0x2`.
pub const ACCESS_USER_DATA: u8 = 0xF2;

/// Flags nibble: `L=1` (code 64-bit), `D/B=0`, `G=0`, `AVL=0` — bit 53 của descriptor.
pub const FLAGS_LONG_CODE: u8 = 0x2;
/// Flags nibble: không `L`, không `D/B`, `G=0` — data segment ở long mode.
pub const FLAGS_DATA: u8 = 0x0;

/// Type của descriptor TSS 64-bit **sẵn sàng**: `P=1`, `DPL=0`, `S=0`, `type=0x9`
/// ("64-bit TSS (Available)", SDM Bảng 3-2) ⇒ byte `0x89`.
pub const TSS_TYPE_AVAILABLE_64: u8 = 0x89;
/// Type của descriptor TSS 64-bit **đang bận**: `type=0xB` ("64-bit TSS (Busy)") ⇒ byte `0x8B`.
///
/// **CPU tự đổi** `0x89 → 0x8B` khi `ltr` nạp descriptor (SDM Vol 3A §7.2.2: "the TSS descriptor
/// is marked busy"). Vì vậy đọc lại GDT **sau** `ltr` phải thấy `0x8B` — đó là bằng chứng `ltr`
/// đã chạy thật, không chỉ là "đã gọi".
pub const TSS_TYPE_BUSY_64: u8 = 0x8B;

/// GDT entry 0: null descriptor (bắt buộc).
pub const GDT_NULL: u64 = 0;

/// Dựng descriptor code/data 64-bit từ `access` (bit 40..47) và `flags` (bit 52..55).
///
/// Base và limit đều 0: ở long mode phân đoạn bị bỏ qua (base = 0, limit không kiểm), nên
/// descriptor chỉ cần đúng `access` + `L`. Đây **đúng** giá trị asm của ảnh đã dùng từ KT-257.
pub const fn gdt_descriptor(access: u8, flags: u8) -> u64 {
    ((flags as u64) << 52) | ((access as u64) << 40)
}

/// GDT entry 1: kernel code 64-bit (`L=1`, `D/B=0`, access `0x9A`).
pub const fn gdt_kernel_code() -> u64 {
    gdt_descriptor(ACCESS_KERNEL_CODE, FLAGS_LONG_CODE)
}

/// GDT entry 2: kernel data (access `0x92`).
pub const fn gdt_kernel_data() -> u64 {
    gdt_descriptor(ACCESS_KERNEL_DATA, FLAGS_DATA)
}

/// GDT entry 4: user code 64-bit (`DPL=3`, `L=1`).
pub const fn gdt_user_code() -> u64 {
    gdt_descriptor(ACCESS_USER_CODE, FLAGS_LONG_CODE)
}

/// GDT entry 3: user data (`DPL=3`).
pub const fn gdt_user_data() -> u64 {
    gdt_descriptor(ACCESS_USER_DATA, FLAGS_DATA)
}

/// Hằng descriptor kernel code — asm của ảnh dùng làm `const` operand.
pub const GDT_KERNEL_CODE: u64 = gdt_kernel_code();
/// Hằng descriptor kernel data — asm của ảnh dùng làm `const` operand.
pub const GDT_KERNEL_DATA: u64 = gdt_kernel_data();
/// Hằng descriptor user code.
pub const GDT_USER_CODE: u64 = gdt_user_code();
/// Hằng descriptor user data.
pub const GDT_USER_DATA: u64 = gdt_user_data();

/// Dựng **descriptor TSS 16 byte** (hai `u64`) từ `base` và `limit`.
///
/// Bố cục (SDM Hình 7-4, little-endian):
///
/// ```text
///   byte 0..1   limit[15:0]
///   byte 2..3   base[15:0]
///   byte 4      base[23:16]
///   byte 5      type_attr = 0x89
///   byte 6      limit[19:16] (bit 0..3) | flags (bit 4..7)
///   byte 7      base[31:24]
///   byte 8..11  base[63:32]
///   byte 12..15 dự phòng (0)
/// ```
///
/// `limit` là **byte cuối** của vùng TSS (103 với TSS 104 byte), không phải số byte.
pub const fn tss_descriptor(base: u64, limit: u32) -> [u64; 2] {
    tss_descriptor_type(base, limit, TSS_TYPE_AVAILABLE_64)
}

/// Như [`tss_descriptor`] nhưng chỉ định **byte `type_attr`** — dùng để dựng dạng **busy**
/// ([`TSS_TYPE_BUSY_64`]) mà CPU đặt sau `ltr`, và để test host chốt cả hai dạng.
pub const fn tss_descriptor_type(base: u64, limit: u32, type_attr: u8) -> [u64; 2] {
    let low = ((limit & 0xFFFF) as u64)
        | ((base & 0xFFFF) << 16)
        | (((base >> 16) & 0xFF) << 32)
        | ((type_attr as u64) << 40)
        | ((((limit >> 16) & 0xF) as u64) << 48)
        | (((base >> 24) & 0xFF) << 56);
    let high = (base >> 32) & 0xFFFF_FFFF;
    [low, high]
}

/// Giải mã **base** từ hai `u64` của descriptor TSS 16 byte — phép nghịch của
/// [`tss_descriptor_type`].
///
/// Dùng để hart phụ đọc lại **địa chỉ TSS mà CPU thật sự nạp** (`ltr`) từ bảng GDT của chính nó,
/// thay vì tin giá trị nó vừa ghi: đó là "kiểm địa chỉ" của đột biến (iii) KT-278.
pub const fn tss_base_of(low: u64, high: u64) -> u64 {
    (low >> 16) & 0xFFFF
        | (((low >> 32) & 0xFF) << 16)
        | (((low >> 56) & 0xFF) << 24)
        | ((high & 0xFFFF_FFFF) << 32)
}

/// Giải mã **byte `type_attr`** từ `u64` đầu của descriptor TSS — dùng để hart phụ chứng minh
/// `ltr` đã chạy thật (CPU đổi `0x89` → `0x8B`).
pub const fn tss_type_of(low: u64) -> u8 {
    ((low >> 40) & 0xFF) as u8
}

/// Bảng GDT **thật** của ảnh — **mỗi hart một bảng** 7 entry, ghi/đọc bằng atomic (an toàn).
///
/// Entry 5–6 (TSS) bằng 0 cho tới khi [`install`] điền; `lgdt` nạp cả bảng nên entry rác **không**
/// bao giờ được `ltr` dùng trước khi điền.
static GDT: [[AtomicU64; GDT_ENTRIES]; MAX_HARTS] = [const {
    [
        AtomicU64::new(GDT_NULL),
        AtomicU64::new(GDT_KERNEL_CODE),
        AtomicU64::new(GDT_KERNEL_DATA),
        AtomicU64::new(GDT_USER_DATA),
        AtomicU64::new(GDT_USER_CODE),
        AtomicU64::new(0),
        AtomicU64::new(0),
    ]
}; MAX_HARTS];

/// Pseudo-descriptor `GDTR` **per-hart** (10 byte có nghĩa, đệm cho tròn 12): `limit` 16-bit +
/// `base` 64-bit.
///
/// Dùng `[AtomicU32; 3]` thay vì `#[repr(C)] struct { limit: u16, base: u64 }` vì `repr(C)` sẽ
/// **chèn 6 byte đệm** giữa hai trường (base thành offset 8), trong khi CPU đọc base ở offset
/// **2**. Ba `u32` cho đúng layout đặc tả mà vẫn ghi được bằng atomic an toàn.
///
/// **Per-hart** vì `sgdt`/`lgdt` của hart này không được ghi vào vùng nhớ hart khác đang dùng.
static GDTR: [[AtomicU32; 3]; MAX_HARTS] =
    [const { [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)] }; MAX_HARTS];

/// Địa chỉ (VA = PA, identity map) bảng GDT của `hart` — asm của ảnh và `lgdt` dùng cùng giá trị.
///
/// `hart >= MAX_HARTS` là lỗi lập trình ⇒ panic fail-loud.
pub fn base(hart: usize) -> u64 {
    assert!(hart < MAX_HARTS, "hart {hart} ngoài MAX_HARTS");
    GDT[hart].as_ptr() as u64
}

/// Đọc entry `index` của GDT hart `hart` (0..[`GDT_ENTRIES`]); ngoài phạm vi trả 0.
pub fn entry(hart: usize, index: usize) -> u64 {
    if hart >= MAX_HARTS {
        return 0;
    }
    match GDT[hart].get(index) {
        Some(slot) => slot.load(Ordering::Relaxed),
        None => 0,
    }
}

/// Ghi `limit`/`base` vào pseudo-descriptor `GDTR` của hart `hart` (chưa nạp — [`install`] mới
/// `lgdt`).
fn set_gdtr(hart: usize, base: u64, limit: u16) {
    let word0 = (limit as u32) | (((base & 0xFFFF) as u32) << 16);
    let word1 = ((base >> 16) & 0xFFFF_FFFF) as u32;
    let word2 = ((base >> 48) & 0xFFFF) as u32;
    GDTR[hart][0].store(word0, Ordering::Relaxed);
    GDTR[hart][1].store(word1, Ordering::Relaxed);
    GDTR[hart][2].store(word2, Ordering::Relaxed);
}

/// Đọc lại `GDTR` của hart `hart` bằng `sgdt` — bằng chứng "GDT đã nạp thật", không chỉ tin
/// `lgdt`.
///
/// Trả `(base, limit)`. Ghi vào chính [`GDTR`] của hart rồi đọc lại ba `u32` (cùng layout ở
/// [`set_gdtr`]).
pub fn read_gdtr(hart: usize) -> (u64, u16) {
    assert!(hart < MAX_HARTS, "hart {hart} ngoài MAX_HARTS");
    asm::sgdt(GDTR[hart].as_ptr() as u64);
    let word0 = GDTR[hart][0].load(Ordering::Relaxed);
    let word1 = GDTR[hart][1].load(Ordering::Relaxed);
    let word2 = GDTR[hart][2].load(Ordering::Relaxed);
    let limit = (word0 & 0xFFFF) as u16;
    let base = (((word0 >> 16) & 0xFFFF) as u64)
        | ((word1 as u64) << 16)
        | (((word2 & 0xFFFF) as u64) << 48);
    (base, limit)
}

/// Nạp GDT **thật** (7 entry) của hart `hart` + descriptor TSS trỏ `tss_base`, rồi `ltr` selector
/// TSS.
///
/// Thứ tự là hợp đồng: điền descriptor TSS **trước** `lgdt` (nếu không, `ltr` đọc entry rác), và
/// `ltr` **sau** `lgdt` (nếu không, `ltr` đọc bảng cũ). `CS`/`DS` đang dùng (`0x08`/`0x10`) trỏ
/// entry **không đổi** giữa GDT tạm của `_start`/trampoline và bảng này, nên `lgdt` giữa chừng an
/// toàn.
///
/// **KT-278**: mỗi hart gọi hàm này với `hart` của **chính nó** và `tss_base = tss::base(hart)` —
/// descriptor TSS của hart này không bao giờ trỏ TSS của hart khác (trừ đột biến `same-tss`).
pub fn install(hart: usize, tss_base: u64) {
    assert!(hart < MAX_HARTS, "hart {hart} ngoài MAX_HARTS");
    let [low, high] = tss_descriptor(tss_base, crate::tss::TSS_LIMIT as u32);
    GDT[hart][INDEX_TSS].store(low, Ordering::Relaxed);
    GDT[hart][INDEX_TSS + 1].store(high, Ordering::Relaxed);
    set_gdtr(hart, base(hart), GDT_LIMIT);
    asm::lgdt(GDTR[hart].as_ptr() as u64);
    asm::ltr(SELECTOR_TSS);
}
