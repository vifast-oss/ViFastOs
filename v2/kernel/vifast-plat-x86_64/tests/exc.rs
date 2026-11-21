// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho GDT/TSS/IDT + dữ liệu exception x86_64 — KT-261 (K5' bước 2).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test exc
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05) của KT-261: bốn mô-đun [`gdt`], [`tss`], [`idt`], [`exc`]
//! chưa tồn tại khi tệp này được viết, nên lượt chạy đầu **không biên dịch được** (đỏ đúng lý do).
//! Sau khi hiện thực, các khẳng định dưới đây chốt **giá trị tuyệt đối** mà asm của ảnh
//! `vifast-kbin-x86_64` dùng làm `const` operand — nếu ai đổi một bit, test đỏ **trước** khi có
//! ảnh boot.
//!
//! Nguồn giá trị:
//!
//! - **Intel SDM Vol 3A** §3.4.5 (segment descriptor), §3.5.1 (TSS descriptor 16 byte),
//!   §6.14 + Hình 6-8 (IDT gate 64-bit), §6.15 (`CR2` của `#PF`), Bảng 6-1 (vector + mã lỗi),
//!   §7.2.1 + Hình 7-11 (TSS 64-bit).
//! - **Intel SDM Vol 3A** §5.4.1 — nạp `DS` bằng selector ngoài giới hạn GDT ⇒ `#GP(selector)`.
//! - **Intel SDM Vol 3A** Bảng 6-1 — `INT3`/`INT n` lưu RIP của lệnh **kế tiếp**; fault (`#UD`,
//!   `#PF`, `#GP`) lưu RIP của **chính** lệnh gây lỗi. Đây là lý do handler resume khác nhau.

// Các khẳng định dưới đây **cố ý** so sánh hằng số: chúng chốt **giá trị tuyệt đối** mà asm của
// ảnh boot nạp. Clippy coi `assert!(HẰNG < HẰNG)` là "constant value"; đó chính là mục đích.
#![allow(clippy::assertions_on_constants)]

use vifast_plat_x86_64::exc::{
    has_error_code, name as exc_name, stub_slot, ERROR_CODE_VECTORS, FRAME_CS_OFFSET,
    FRAME_ERROR_OFFSET, FRAME_GPR_COUNT, FRAME_RFLAGS_OFFSET, FRAME_RIP_OFFSET,
    FRAME_VECTOR_OFFSET, KERNEL_CS, PF_ERR_INSTRUCTION, PF_ERR_PRESENT, PF_ERR_RESERVED,
    PF_ERR_USER, PF_ERR_WRITE, STUB_AC, STUB_BP, STUB_COUNT, STUB_CP, STUB_DB, STUB_DE, STUB_DF,
    STUB_GP, STUB_IPI, STUB_NP, STUB_PF, STUB_SPURIOUS, STUB_SS, STUB_SX, STUB_SYSCALL, STUB_TIMER,
    STUB_TS, STUB_UD, STUB_UNKNOWN, STUB_VC, VEC_AC, VEC_BP, VEC_CP, VEC_DB, VEC_DE, VEC_DF,
    VEC_GP, VEC_IPI, VEC_NP, VEC_PF, VEC_SPURIOUS, VEC_SS, VEC_SX, VEC_SYSCALL, VEC_TIMER, VEC_TS,
    VEC_UD, VEC_UNKNOWN, VEC_UNKNOWN_PROBE, VEC_VC,
};
use vifast_plat_x86_64::gdt::{
    entry as gdt_entry, gdt_descriptor, gdt_kernel_code, gdt_kernel_data, gdt_user_code,
    gdt_user_data, tss_descriptor, tss_descriptor_type, ACCESS_KERNEL_CODE, ACCESS_KERNEL_DATA,
    ACCESS_USER_CODE, ACCESS_USER_DATA, FLAGS_DATA, FLAGS_LONG_CODE, GDT_BYTES, GDT_ENTRIES,
    GDT_KERNEL_CODE, GDT_KERNEL_DATA, GDT_LIMIT, GDT_NULL, GDT_USER_CODE, GDT_USER_DATA,
    INDEX_KERNEL_CODE, INDEX_KERNEL_DATA, INDEX_NULL, INDEX_TSS, INDEX_USER_CODE, INDEX_USER_DATA,
    RPL_USER, SELECTOR_KERNEL_CODE, SELECTOR_KERNEL_DATA, SELECTOR_TSS, SELECTOR_USER_CODE,
    SELECTOR_USER_CODE_RPL3, SELECTOR_USER_DATA, SELECTOR_USER_DATA_RPL3, TSS_TYPE_AVAILABLE_64,
    TSS_TYPE_BUSY_64,
};
use vifast_plat_x86_64::idt::{
    entry as idt_entry, entry_at, entry_ist, entry_offset, entry_selector, entry_type_attr,
    gate_type_attr, GATE_INTERRUPT, GATE_INTERRUPT_USER, GATE_TRAP, IDT_BYTES, IDT_ENTRIES,
    IDT_LIMIT, IST_DOUBLE_FAULT,
};
use vifast_plat_x86_64::tss::{
    base as tss_base, check_distinct as tss_check_distinct, init as tss_init, io_map_base, ist,
    rsp0, TssError, TSS_BYTES, TSS_IO_MAP_BASE, TSS_IO_MAP_OFFSET, TSS_IST1_OFFSET, TSS_IST_COUNT,
    TSS_IST_STRIDE, TSS_LIMIT, TSS_RSP0_OFFSET, TSS_WORDS,
};

// ---------------------------------------------------------------------------------------------
// GDT — 7 entry, selector, descriptor
// ---------------------------------------------------------------------------------------------

/// Hình học GDT: 7 entry × 8 byte, `limit` = byte cuối = 55.
#[test]
fn gdt_geometry_literal() {
    assert_eq!(GDT_ENTRIES, 7);
    assert_eq!(GDT_BYTES, 56);
    assert_eq!(GDT_LIMIT, 55);
    // `limit` là **byte cuối**, không phải số byte — sai một đơn vị là `lgdt` cắt mất entry TSS.
    assert_eq!(GDT_LIMIT as usize, GDT_BYTES - 1);
}

/// Chỉ số entry: null 0, kernel code 1, kernel data 2, user code 3, user data 4, TSS 5–6.
#[test]
fn gdt_indices_literal() {
    assert_eq!(INDEX_NULL, 0);
    assert_eq!(INDEX_KERNEL_CODE, 1);
    assert_eq!(INDEX_KERNEL_DATA, 2);
    assert_eq!(INDEX_USER_CODE, 3);
    assert_eq!(INDEX_USER_DATA, 4);
    assert_eq!(INDEX_TSS, 5);
    // Descriptor TSS chiếm **hai** entry ⇒ entry cuối là `INDEX_TSS + 1` = 6 < 7.
    assert_eq!(INDEX_TSS + 1, GDT_ENTRIES - 1);
}

/// Selector = chỉ số × 8; selector user dùng thật có RPL 3.
#[test]
fn gdt_selectors_literal() {
    assert_eq!(SELECTOR_KERNEL_CODE, 0x08);
    assert_eq!(SELECTOR_KERNEL_DATA, 0x10);
    assert_eq!(SELECTOR_USER_CODE, 0x18);
    assert_eq!(SELECTOR_USER_DATA, 0x20);
    assert_eq!(SELECTOR_TSS, 0x28);
    assert_eq!(RPL_USER, 3);
    assert_eq!(SELECTOR_USER_CODE_RPL3, 0x1B);
    assert_eq!(SELECTOR_USER_DATA_RPL3, 0x23);
    // Selector phải khớp chỉ số × 8 — asm của ảnh dùng hằng, không tự nhân.
    assert_eq!(SELECTOR_KERNEL_CODE as usize, INDEX_KERNEL_CODE * 8);
    assert_eq!(SELECTOR_KERNEL_DATA as usize, INDEX_KERNEL_DATA * 8);
    assert_eq!(SELECTOR_USER_CODE as usize, INDEX_USER_CODE * 8);
    assert_eq!(SELECTOR_USER_DATA as usize, INDEX_USER_DATA * 8);
    assert_eq!(SELECTOR_TSS as usize, INDEX_TSS * 8);
}

/// Bốn descriptor code/data: access byte + `L` đúng SDM; base/limit 0 (long mode bỏ qua).
#[test]
fn gdt_descriptors_literal() {
    assert_eq!(GDT_NULL, 0);
    assert_eq!(GDT_KERNEL_CODE, 0x0020_9A00_0000_0000);
    assert_eq!(GDT_KERNEL_DATA, 0x0000_9200_0000_0000);
    assert_eq!(GDT_USER_CODE, 0x0020_FA00_0000_0000);
    assert_eq!(GDT_USER_DATA, 0x0000_F200_0000_0000);
    // Hàm dựng phải trả đúng hằng (asm dùng hằng, không dùng hàm).
    assert_eq!(gdt_kernel_code(), GDT_KERNEL_CODE);
    assert_eq!(gdt_kernel_data(), GDT_KERNEL_DATA);
    assert_eq!(gdt_user_code(), GDT_USER_CODE);
    assert_eq!(gdt_user_data(), GDT_USER_DATA);
    // Access byte ở bit 40..47.
    assert_eq!(((GDT_KERNEL_CODE >> 40) & 0xFF) as u8, ACCESS_KERNEL_CODE);
    assert_eq!(((GDT_KERNEL_DATA >> 40) & 0xFF) as u8, ACCESS_KERNEL_DATA);
    assert_eq!(((GDT_USER_CODE >> 40) & 0xFF) as u8, ACCESS_USER_CODE);
    assert_eq!(((GDT_USER_DATA >> 40) & 0xFF) as u8, ACCESS_USER_DATA);
    // `L` (long mode) là bit 53; `D/B` bit 54 phải 0 khi `L=1`.
    assert_eq!((GDT_KERNEL_CODE >> 53) & 1, 1);
    assert_eq!((GDT_KERNEL_CODE >> 54) & 1, 0);
    assert_eq!((GDT_USER_CODE >> 53) & 1, 1);
    assert_eq!((GDT_USER_CODE >> 54) & 1, 0);
    // Flags nibble ở bit 52..55.
    assert_eq!(((GDT_KERNEL_CODE >> 52) & 0xF) as u8, FLAGS_LONG_CODE);
    assert_eq!(((GDT_KERNEL_DATA >> 52) & 0xF) as u8, FLAGS_DATA);
}

/// `DPL` nằm ở bit 45..46 của access byte: kernel 0, user 3.
#[test]
fn gdt_dpl_bits() {
    assert_eq!((GDT_KERNEL_CODE >> 45) & 0x3, 0);
    assert_eq!((GDT_KERNEL_DATA >> 45) & 0x3, 0);
    assert_eq!((GDT_USER_CODE >> 45) & 0x3, 3);
    assert_eq!((GDT_USER_DATA >> 45) & 0x3, 3);
    // `S = 1` (code/data, không phải system) ở bit 44.
    assert_eq!((GDT_KERNEL_CODE >> 44) & 1, 1);
    assert_eq!((GDT_USER_DATA >> 44) & 1, 1);
    // `P = 1` ở bit 47 — mọi descriptor phải present.
    assert_eq!((GDT_KERNEL_CODE >> 47) & 1, 1);
    assert_eq!((GDT_KERNEL_DATA >> 47) & 1, 1);
    assert_eq!((GDT_USER_CODE >> 47) & 1, 1);
    assert_eq!((GDT_USER_DATA >> 47) & 1, 1);
}

/// Bảng GDT tĩnh **per-hart**: 5 entry đầu đúng hằng, 2 entry TSS **bằng 0** trước `install`.
///
/// **KT-278**: mỗi hart một bảng riêng — kiểm **hart 0** và **hart 3** để chốt rằng bảng thứ tư
/// cũng được khởi tạo đúng (không phải "hart 0 đúng, còn lại rác").
#[test]
fn gdt_static_initial_contents() {
    for hart in [0usize, 3] {
        assert_eq!(gdt_entry(hart, INDEX_NULL), GDT_NULL);
        assert_eq!(gdt_entry(hart, INDEX_KERNEL_CODE), GDT_KERNEL_CODE);
        assert_eq!(gdt_entry(hart, INDEX_KERNEL_DATA), GDT_KERNEL_DATA);
        assert_eq!(gdt_entry(hart, INDEX_USER_CODE), GDT_USER_CODE);
        assert_eq!(gdt_entry(hart, INDEX_USER_DATA), GDT_USER_DATA);
        // Descriptor TSS do `install` điền; trước đó phải là 0 (không trỏ rác).
        assert_eq!(gdt_entry(hart, INDEX_TSS), 0);
        assert_eq!(gdt_entry(hart, INDEX_TSS + 1), 0);
        // Ngoài phạm vi trả 0, không panic.
        assert_eq!(gdt_entry(hart, GDT_ENTRIES), 0);
        assert_eq!(gdt_entry(hart, usize::MAX), 0);
    }
    // Hart ngoài `MAX_HARTS` trả 0 (không panic) — fail-closed ở tầng đọc.
    assert_eq!(gdt_entry(usize::MAX, INDEX_KERNEL_CODE), 0);
}

// ---------------------------------------------------------------------------------------------
// Descriptor TSS 16 byte
// ---------------------------------------------------------------------------------------------

/// Mã hoá descriptor TSS: từng byte đúng SDM Hình 7-4.
#[test]
fn tss_descriptor_layout_bytes() {
    // base = 0x0000_0000_1234_5678, limit = 0x67 (103) — dựng từng byte kỳ vọng.
    let base: u64 = 0x0000_0000_1234_5678;
    let limit: u32 = 0x67;
    let [low, high] = tss_descriptor(base, limit);
    let bytes = low.to_le_bytes();
    assert_eq!(bytes[0], 0x67); // limit[7:0]
    assert_eq!(bytes[1], 0x00); // limit[15:8]
    assert_eq!(bytes[2], 0x78); // base[7:0]
    assert_eq!(bytes[3], 0x56); // base[15:8]
    assert_eq!(bytes[4], 0x34); // base[23:16]
    assert_eq!(bytes[5], TSS_TYPE_AVAILABLE_64); // type_attr
    assert_eq!(bytes[6], 0x00); // limit[19:16] | flags
    assert_eq!(bytes[7], 0x12); // base[31:24]
    assert_eq!(high, 0x0000_0000); // base[63:32]
}

/// `limit` lớn hơn 16 bit: 4 bit cao vào byte 6, không tràn sang `type_attr`.
#[test]
fn tss_descriptor_limit_high_nibble() {
    let [low, _] = tss_descriptor(0, 0x1_2345);
    let bytes = low.to_le_bytes();
    assert_eq!(bytes[0], 0x45);
    assert_eq!(bytes[1], 0x23);
    assert_eq!(bytes[6] & 0x0F, 0x1); // limit[19:16]
    assert_eq!(bytes[5], TSS_TYPE_AVAILABLE_64); // không bị đụng
}

/// Base 64-bit đầy đủ: nửa cao nằm ở `u64` thứ hai.
#[test]
fn tss_descriptor_base_high() {
    let base: u64 = 0xDEAD_BEEF_1234_5678;
    let [low, high] = tss_descriptor(base, 0x67);
    assert_eq!(high, 0xDEAD_BEEF);
    assert_eq!((low >> 16) & 0xFFFF, 0x5678);
    assert_eq!((low >> 32) & 0xFF, 0x34);
    assert_eq!((low >> 56) & 0xFF, 0x12);
}

/// Dạng **busy** (`0x8B`) khác dạng available (`0x89`) **đúng một byte** — CPU đổi sau `ltr`.
#[test]
fn tss_descriptor_busy_differs_in_type_byte() {
    let base = 0x0000_0000_0001_2345;
    let available = tss_descriptor(base, 0x67);
    let busy = tss_descriptor_type(base, 0x67, TSS_TYPE_BUSY_64);
    assert_eq!(TSS_TYPE_AVAILABLE_64, 0x89);
    assert_eq!(TSS_TYPE_BUSY_64, 0x8B);
    assert_eq!(available[1], busy[1]);
    assert_eq!(available[0] ^ busy[0], 0x02 << 40); // chỉ khác bit 1 của `type`
    assert_eq!(((busy[0] >> 40) & 0xFF) as u8, TSS_TYPE_BUSY_64);
}

/// `gdt_descriptor` là hàm dựng thuần: access/flags vào đúng vị trí bit.
#[test]
fn gdt_descriptor_places_bits() {
    assert_eq!(gdt_descriptor(0x9A, 0x2), 0x0020_9A00_0000_0000);
    assert_eq!(gdt_descriptor(0x00, 0x0), 0);
    assert_eq!(gdt_descriptor(0xFF, 0xF), 0x00F0_FF00_0000_0000);
}

// ---------------------------------------------------------------------------------------------
// TSS 64-bit
// ---------------------------------------------------------------------------------------------

/// Bố cục TSS theo SDM Hình 7-11: 104 byte, `RSP0` ở 4, `IST1` ở 0x24, `I/O map` ở 0x66.
#[test]
fn tss_layout_literal() {
    assert_eq!(TSS_BYTES, 104);
    assert_eq!(TSS_LIMIT, 103);
    assert_eq!(TSS_WORDS, 26);
    assert_eq!(TSS_RSP0_OFFSET, 4);
    assert_eq!(TSS_IST1_OFFSET, 0x24);
    assert_eq!(TSS_IST_STRIDE, 8);
    assert_eq!(TSS_IST_COUNT, 7);
    assert_eq!(TSS_IO_MAP_OFFSET, 0x66);
    assert_eq!(TSS_IO_MAP_BASE, 104);
    // `I/O map base` = limit + 1 ⇒ **không** có bitmap (mọi I/O từ CPL3 đều #GP).
    assert_eq!(TSS_IO_MAP_BASE, TSS_LIMIT + 1);
    // `IST7` phải kết thúc trước vùng dự phòng ở 0x5C.
    assert_eq!(TSS_IST1_OFFSET + (TSS_IST_COUNT - 1) * TSS_IST_STRIDE, 0x54);
    // Trường 64-bit phải căn **4** (điều kiện của `[AtomicU32; 26]`); trường 16-bit căn **2**.
    for offset in [TSS_RSP0_OFFSET, TSS_IST1_OFFSET] {
        assert_eq!(offset % 4, 0);
    }
    assert_eq!(TSS_IO_MAP_OFFSET % 2, 0);
    // `I/O map` nằm **sau** mọi trường khác, trong 2 byte cuối của TSS.
    assert_eq!(TSS_IO_MAP_OFFSET + 2, TSS_BYTES);
}

/// `init` ghi `RSP0`/`IST1`/`I/O map base`; đọc lại đúng; `IST` ngoài phạm vi trả 0.
///
/// **KT-278**: TSS là **per-hart** — mỗi hart một vùng 104 byte riêng. Test ghi **hart 0** và
/// **hart 1** rồi đọc lại cả hai: nếu hai hart trỏ cùng một vùng thì khẳng định của hart 0 sẽ
/// thấy giá trị của hart 1 (đó chính là đột biến (iii) của KT-278, phát hiện bằng kiểm địa chỉ).
#[test]
fn tss_init_and_read_back() {
    let want_rsp0 = 0x0000_0000_0011_9000u64;
    let want_ist1 = 0x0000_0000_0010_9000u64;
    tss_init(0, want_rsp0, want_ist1);
    assert_eq!(rsp0(0), want_rsp0);
    assert_eq!(ist(0, 1), want_ist1);
    assert_eq!(io_map_base(0), TSS_IO_MAP_BASE);
    // `IST0` không tồn tại; `IST8` ngoài phạm vi ⇒ 0 (không đọc rác).
    assert_eq!(ist(0, 0), 0);
    assert_eq!(ist(0, TSS_IST_COUNT + 1), 0);
    // Các `IST` khác chưa nạp ⇒ 0.
    for n in 2..=TSS_IST_COUNT {
        assert_eq!(ist(0, n), 0);
    }
    // Ghi lại giá trị khác phải thay, không cộng dồn.
    tss_init(0, 0x2000, 0x3000);
    assert_eq!(rsp0(0), 0x2000);
    assert_eq!(ist(0, 1), 0x3000);
    // **Per-hart**: ghi hart 1 **không** đổi giá trị của hart 0.
    tss_init(1, 0x4000, 0x5000);
    assert_eq!(rsp0(1), 0x4000);
    assert_eq!(ist(1, 1), 0x5000);
    assert_eq!(rsp0(0), 0x2000);
    assert_eq!(ist(0, 1), 0x3000);
    // Địa chỉ hai vùng TSS **rời nhau** và cách nhau đúng `TSS_BYTES`.
    assert_ne!(tss_base(0), tss_base(1));
    assert_eq!(tss_base(1) - tss_base(0), TSS_BYTES as u64);
    // Kiểm địa chỉ: 4 hart của ảnh KT-278 phải **rời nhau**.
    let bases = [tss_base(0), tss_base(1), tss_base(2), tss_base(3)];
    assert_eq!(tss_check_distinct(&bases), Ok(()));
    // Đột biến (iii): hai hart **cùng** địa chỉ ⇒ `Duplicate` (ĐỎ ở tầng kiểm địa chỉ).
    assert_eq!(
        tss_check_distinct(&[bases[0], bases[1], bases[0], bases[3]]),
        Err(TssError::Duplicate { a: 0, b: 2 })
    );
    // Chồng lấn (cách nhau < 104 byte) cũng là lỗi — không chỉ trùng tuyệt đối.
    assert_eq!(
        tss_check_distinct(&[bases[0], bases[0] + 8]),
        Err(TssError::Overlap { a: 0, b: 1 })
    );
}

// ---------------------------------------------------------------------------------------------
// IDT — 256 ô × 16 byte
// ---------------------------------------------------------------------------------------------

/// Hình học IDT: 256 ô, 4096 byte, `limit` = 4095.
#[test]
fn idt_geometry_literal() {
    assert_eq!(IDT_ENTRIES, 256);
    assert_eq!(IDT_BYTES, 4096);
    assert_eq!(IDT_LIMIT, 4095);
    assert_eq!(IDT_LIMIT as usize, IDT_BYTES - 1);
    // Mỗi ô 16 byte ⇒ 256 ô phủ đúng 4 KiB.
    assert_eq!(IDT_ENTRIES * 16, IDT_BYTES);
}

/// Mã hoá một ô IDT: từng byte đúng SDM Hình 6-8.
#[test]
fn idt_entry_layout_bytes() {
    let offset: u64 = 0x0000_0000_1234_5678;
    let [low, high] = idt_entry(offset, SELECTOR_KERNEL_CODE, 0, GATE_INTERRUPT);
    let bytes = low.to_le_bytes();
    assert_eq!(bytes[0], 0x78); // offset[7:0]
    assert_eq!(bytes[1], 0x56); // offset[15:8]
    assert_eq!(bytes[2], 0x08); // selector[7:0]
    assert_eq!(bytes[3], 0x00); // selector[15:8]
    assert_eq!(bytes[4], 0x00); // IST | dự phòng
    assert_eq!(bytes[5], GATE_INTERRUPT); // type_attr
    assert_eq!(bytes[6], 0x34); // offset[23:16]
    assert_eq!(bytes[7], 0x12); // offset[31:24]
    assert_eq!(high, 0x0000_0000); // offset[63:32]
}

/// Offset 64-bit đầy đủ: nửa cao ở `u64` thứ hai; khứ hồi mã hoá/giải mã.
#[test]
fn idt_entry_offset_roundtrip() {
    let offset: u64 = 0xFFFF_8000_DEAD_BEEF;
    let [low, high] = idt_entry(offset, SELECTOR_KERNEL_CODE, 0, GATE_INTERRUPT);
    assert_eq!(high, 0xFFFF_8000);
    assert_eq!(entry_offset(low, high), offset);
    assert_eq!(entry_selector(low), SELECTOR_KERNEL_CODE);
    assert_eq!(entry_ist(low), 0);
    assert_eq!(entry_type_attr(low), GATE_INTERRUPT);
}

/// `IST` nằm ở bit 32..34 của `u64` đầu — `#DF` dùng `IST1`.
#[test]
fn idt_entry_ist_bits() {
    assert_eq!(IST_DOUBLE_FAULT, 1);
    let [low, _] = idt_entry(
        0x1000,
        SELECTOR_KERNEL_CODE,
        IST_DOUBLE_FAULT,
        GATE_INTERRUPT,
    );
    assert_eq!((low >> 32) & 0x7, 1);
    assert_eq!(entry_ist(low), 1);
    // Ba bit, không tràn sang `type_attr` (byte 5).
    let [low7, _] = idt_entry(0x1000, SELECTOR_KERNEL_CODE, 7, GATE_INTERRUPT);
    assert_eq!(entry_ist(low7), 7);
    assert_eq!(entry_type_attr(low7), GATE_INTERRUPT);
    // `IST = 0` nghĩa là "không dùng IST" (stack hiện tại).
    let [low0, _] = idt_entry(0x1000, SELECTOR_KERNEL_CODE, 0, GATE_INTERRUPT);
    assert_eq!(entry_ist(low0), 0);
}

/// Ba loại gate: interrupt `0x8E`, trap `0x8F`, interrupt gọi được từ CPL3 `0xEE`.
#[test]
fn idt_gate_types_literal() {
    assert_eq!(GATE_INTERRUPT, 0x8E);
    assert_eq!(GATE_TRAP, 0x8F);
    assert_eq!(GATE_INTERRUPT_USER, 0xEE);
    // `P = 1` ở bit 7 của `type_attr` — mọi gate phải present.
    for gate in [GATE_INTERRUPT, GATE_TRAP, GATE_INTERRUPT_USER] {
        assert_eq!(gate >> 7, 1);
    }
    // `DPL` ở bit 5..6: kernel 0, user 3.
    assert_eq!((GATE_INTERRUPT >> 5) & 0x3, 0);
    assert_eq!((GATE_INTERRUPT_USER >> 5) & 0x3, 3);
    // `S = 0` (system) ở bit 4.
    assert_eq!((GATE_INTERRUPT >> 4) & 1, 0);
}

/// **KT-270**: chỉ vector `0x80` (syscall) có gate `DPL = 3`; mọi vector khác `DPL = 0`.
///
/// Đây là điều kiện để ring 3 gọi được `int 0x80` mà **không** gọi được `int 3`/`int 0x0e` (một
/// ring 3 gọi `int 3` với gate `DPL = 0` sẽ `#GP` — đúng hành vi mong muốn).
#[test]
fn idt_gate_type_attr_only_syscall_is_user() {
    assert_eq!(gate_type_attr(VEC_SYSCALL), GATE_INTERRUPT_USER);
    for vec in [0u8, 1, 3, 6, 8, 13, 14, 32, 0x30, 0xFE, 255] {
        assert_eq!(gate_type_attr(vec), GATE_INTERRUPT, "vector {vec}");
    }
    // `DPL` của gate syscall là 3, của mọi gate khác là 0.
    assert_eq!((gate_type_attr(VEC_SYSCALL) >> 5) & 0x3, 3);
    assert_eq!((gate_type_attr(3) >> 5) & 0x3, 0);
}

/// Bảng IDT tĩnh: mọi ô **rỗng** (`P = 0`) trước `install` — không có ô rác trỏ vào đâu.
#[test]
fn idt_static_initial_zero() {
    for vec in [0usize, 3, 8, 13, 14, 255] {
        assert_eq!(entry_at(vec), [0, 0]);
    }
    // Ngoài phạm vi trả `[0, 0]`, không panic.
    assert_eq!(entry_at(IDT_ENTRIES), [0, 0]);
    assert_eq!(entry_at(usize::MAX), [0, 0]);
}

// ---------------------------------------------------------------------------------------------
// Vector + mã lỗi + slot stub
// ---------------------------------------------------------------------------------------------

/// Vector của các exception bước này xử lý (SDM Bảng 6-1).
#[test]
fn exception_vectors_literal() {
    assert_eq!(VEC_DE, 0);
    assert_eq!(VEC_DB, 1);
    assert_eq!(VEC_BP, 3);
    assert_eq!(VEC_UD, 6);
    assert_eq!(VEC_DF, 8);
    assert_eq!(VEC_TS, 10);
    assert_eq!(VEC_NP, 11);
    assert_eq!(VEC_SS, 12);
    assert_eq!(VEC_GP, 13);
    assert_eq!(VEC_PF, 14);
    assert_eq!(VEC_AC, 17);
    assert_eq!(VEC_CP, 21);
    assert_eq!(VEC_VC, 29);
    assert_eq!(VEC_SX, 30);
    // Sentinel của `isr_unknown` không trùng vector thật nào (0..31) và không phải 0.
    // **KT-266**: dời từ 255 sang 254 vì 255 là vector **thật** của spurious (có stub riêng).
    assert_eq!(VEC_UNKNOWN, 0xFE);
    assert!(VEC_UNKNOWN > 31);
    assert_ne!(VEC_UNKNOWN, VEC_SPURIOUS as u64);
    // Hai vector **ngắt** của KT-266.
    assert_eq!(VEC_TIMER, 32);
    assert_eq!(VEC_SPURIOUS, 255);
    // KT-270: vector `int 0x80` — software interrupt, ngoài dải exception.
    assert_eq!(VEC_SYSCALL, 0x80);
    assert!(VEC_SYSCALL > 31);
    assert_ne!(VEC_SYSCALL, VEC_TIMER);
    assert_ne!(VEC_SYSCALL, VEC_SPURIOUS);
}

/// Tập vector **có mã lỗi** đúng 10 vector của SDM Bảng 6-1.
#[test]
fn error_code_vectors_literal() {
    assert_eq!(ERROR_CODE_VECTORS.len(), 10);
    assert_eq!(
        ERROR_CODE_VECTORS,
        [8u8, 10, 11, 12, 13, 14, 17, 21, 29, 30]
    );
    // `has_error_code` phải khớp **chính xác** tập trên cho cả 256 vector.
    for vec in 0..=255u8 {
        let want = ERROR_CODE_VECTORS.contains(&vec);
        assert_eq!(has_error_code(vec), want, "vector {vec}");
    }
    // Vài vector **không** có mã lỗi — stub phải đẩy `0` giả.
    for vec in [0u8, 1, 3, 6, 7, 16, 18, 19, 20, 31] {
        assert!(!has_error_code(vec), "vector {vec} không có mã lỗi");
    }
}

/// Ánh xạ vector → slot stub: 14 vector có stub riêng, còn lại về `STUB_UNKNOWN`.
#[test]
fn stub_slot_mapping() {
    assert_eq!(stub_slot(VEC_DE), STUB_DE);
    assert_eq!(stub_slot(VEC_DB), STUB_DB);
    assert_eq!(stub_slot(VEC_BP), STUB_BP);
    assert_eq!(stub_slot(VEC_UD), STUB_UD);
    assert_eq!(stub_slot(VEC_DF), STUB_DF);
    assert_eq!(stub_slot(VEC_GP), STUB_GP);
    assert_eq!(stub_slot(VEC_PF), STUB_PF);
    assert_eq!(stub_slot(VEC_TS), STUB_TS);
    assert_eq!(stub_slot(VEC_NP), STUB_NP);
    assert_eq!(stub_slot(VEC_SS), STUB_SS);
    assert_eq!(stub_slot(VEC_AC), STUB_AC);
    assert_eq!(stub_slot(VEC_CP), STUB_CP);
    assert_eq!(stub_slot(VEC_VC), STUB_VC);
    assert_eq!(stub_slot(VEC_SX), STUB_SX);
    // KT-266: timer 32 và spurious 255 có stub riêng.
    assert_eq!(stub_slot(VEC_TIMER), STUB_TIMER);
    assert_eq!(stub_slot(VEC_SPURIOUS), STUB_SPURIOUS);
    // KT-270: `int 0x80` có stub riêng (không đi qua `isr_unknown`).
    assert_eq!(stub_slot(VEC_SYSCALL), STUB_SYSCALL);
    // **KT-278**: vector IPI `0x30` có stub riêng; vector lạ của ca `isr_unknown` là `0x31`.
    assert_eq!(stub_slot(VEC_IPI), STUB_IPI);
    assert_eq!(stub_slot(VEC_UNKNOWN_PROBE), STUB_UNKNOWN);
    // Mọi vector khác (kể cả 2, 4, 5, 7, 9, 15, 16, 31, 0x31, 0xFE) về stub chung.
    for vec in [2u8, 4, 5, 7, 9, 15, 16, 18, 19, 20, 31, 0x31, 0xFE] {
        assert_eq!(stub_slot(vec), STUB_UNKNOWN, "vector {vec}");
    }
    // Đếm: đúng 18 vector có stub riêng (14 exception + timer + spurious + syscall + IPI).
    let special = (0..=255u8)
        .filter(|v| stub_slot(*v) != STUB_UNKNOWN)
        .count();
    assert_eq!(special, 18);
}

/// 18 slot là **18 giá trị phân biệt** — không hai vector nào dùng chung stub riêng.
#[test]
fn stub_slots_are_distinct() {
    let slots = [
        STUB_UNKNOWN,
        STUB_DE,
        STUB_DB,
        STUB_BP,
        STUB_UD,
        STUB_DF,
        STUB_GP,
        STUB_PF,
        STUB_TS,
        STUB_NP,
        STUB_SS,
        STUB_AC,
        STUB_CP,
        STUB_VC,
        STUB_SX,
        STUB_TIMER,
        STUB_SPURIOUS,
        STUB_SYSCALL,
        STUB_IPI,
    ];
    assert_eq!(slots.len(), STUB_COUNT);
    for (i, a) in slots.iter().enumerate() {
        for (j, b) in slots.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "slot {i} trùng slot {j}");
            }
        }
        assert!(*a < STUB_COUNT, "slot {i} ngoài phạm vi");
    }
}

/// Bố cục frame: 15 GPR (120 byte) rồi vector/mã lỗi/rip/cs/rflags — asm dùng chính các hằng này.
#[test]
fn exception_frame_offsets_literal() {
    assert_eq!(FRAME_GPR_COUNT, 15);
    assert_eq!(FRAME_VECTOR_OFFSET, 120);
    assert_eq!(FRAME_ERROR_OFFSET, 128);
    assert_eq!(FRAME_RIP_OFFSET, 136);
    assert_eq!(FRAME_CS_OFFSET, 144);
    assert_eq!(FRAME_RFLAGS_OFFSET, 152);
    // Mỗi trường cách nhau 8 byte, đúng thứ tự CPU + stub đẩy.
    assert_eq!(FRAME_VECTOR_OFFSET, FRAME_GPR_COUNT * 8);
    assert_eq!(FRAME_ERROR_OFFSET, FRAME_VECTOR_OFFSET + 8);
    assert_eq!(FRAME_RIP_OFFSET, FRAME_ERROR_OFFSET + 8);
    assert_eq!(FRAME_CS_OFFSET, FRAME_RIP_OFFSET + 8);
    assert_eq!(FRAME_RFLAGS_OFFSET, FRAME_CS_OFFSET + 8);
    // `CS` của kernel = selector kernel code.
    assert_eq!(KERNEL_CS, SELECTOR_KERNEL_CODE as u64);
    assert_eq!(KERNEL_CS, 0x08);
}

/// Bit mã lỗi `#PF` (SDM Bảng 6-1 / §6.15).
#[test]
fn page_fault_error_bits_literal() {
    assert_eq!(PF_ERR_PRESENT, 1 << 0);
    assert_eq!(PF_ERR_WRITE, 1 << 1);
    assert_eq!(PF_ERR_USER, 1 << 2);
    assert_eq!(PF_ERR_RESERVED, 1 << 3);
    assert_eq!(PF_ERR_INSTRUCTION, 1 << 4);
    // Năm bit rời nhau, không chồng.
    let all = PF_ERR_PRESENT | PF_ERR_WRITE | PF_ERR_USER | PF_ERR_RESERVED | PF_ERR_INSTRUCTION;
    assert_eq!(all, 0x1F);
}

/// Tên vector để in log — phủ đúng các vector bước này dùng.
#[test]
fn exception_names() {
    assert_eq!(exc_name(VEC_BP as u64), "bp");
    assert_eq!(exc_name(VEC_UD as u64), "ud");
    assert_eq!(exc_name(VEC_DF as u64), "df");
    assert_eq!(exc_name(VEC_GP as u64), "gp");
    assert_eq!(exc_name(VEC_PF as u64), "pf");
    assert_eq!(exc_name(VEC_UNKNOWN), "unknown");
    assert_eq!(exc_name(VEC_SYSCALL as u64), "syscall");
    // **KT-278**: vector IPI có tên riêng; vector lạ của ca `isr_unknown` vẫn là "other".
    assert_eq!(exc_name(VEC_IPI as u64), "ipi");
    assert_eq!(exc_name(VEC_UNKNOWN_PROBE as u64), "other");
    // Vector không có tên riêng vẫn có nhãn, không panic.
    assert_eq!(exc_name(2), "other");
}
