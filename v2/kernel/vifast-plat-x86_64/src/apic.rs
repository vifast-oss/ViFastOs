// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Dữ liệu APIC + timer x86_64** — KT-266 (K5' bước 3).
//!
//! Tệp này **thuần dữ liệu** (không asm, không `unsafe`, không MMIO): nó chốt
//!
//! - **địa chỉ MMIO** mặc định của LAPIC (`0xFEE0_0000`) và IOAPIC (`0xFEC0_0000`);
//! - **offset thanh ghi** LAPIC (ID, version, TPR, EOI, SVR, LVT timer, initial/current count,
//!   divide) và **bit** của từng thanh ghi (SDM Vol 3A §10.4.1, §10.4.6, §10.5.4);
//! - **bảng chia** của LAPIC timer (mã 4 bit ↔ số chia, SDM Bảng 10-2) — mã **không** phải số;
//! - **thanh ghi IOAPIC** (`IOREGSEL`/`IOWIN`, `IOAPICID`, `IOAPICVER`, redirection table) và
//!   phép **mask** một redirection entry (bit 16, SDM §11.5.4);
//! - **PIT kênh 0** — nguồn thời gian tham chiếu 1,193182 MHz để **đo** `hz` của LAPIC timer
//!   (Intel 8254: mode 0, lệnh latch).
//!
//! # Vì sao tách khỏi asm
//!
//! Ảnh boot `vifast-kbin-x86_64` **không** được có `unsafe` (R5), nên mọi truy cập MMIO/MSR/cổng
//! I/O nằm ở [`crate::asm`] (crate `kernel/vifast-plat-*`, D2-17). Tệp này chỉ là **hợp đồng số
//! học**: test host `tests/apic.rs` chốt từng giá trị tuyệt đối, nên một bit sai đỏ **trước** khi
//! có ảnh boot — đúng bài học "xanh giả" của K5' (audit §5 R6).
//!
//! # Vì sao `hz` **không** hardcode
//!
//! Tần số bus APIC không được bảo đảm bởi đặc tả và QEMU TCG có thể chạy nhanh/chậm; vì vậy ảnh
//! **đo** `hz` bằng PIT: đếm số tick LAPIC trong một cửa sổ PIT đã biết. Các hàm
//! [`pit_elapsed_ticks`] và [`hz_from_pit`] là phần số học của phép đo đó (có test host, kể cả ca
//! chia 0).

// ---------------------------------------------------------------------------------------------
// Địa chỉ MMIO mặc định
// ---------------------------------------------------------------------------------------------

/// Địa chỉ MMIO mặc định của **LAPIC** — `IA32_APIC_BASE.BASE` sau reset (SDM Vol 3A §10.4.1).
pub const LAPIC_DEFAULT_BASE: u64 = 0xFEE0_0000;
/// Địa chỉ MMIO mặc định của **IOAPIC** (SDM Vol 3A §11.5.1).
pub const IOAPIC_DEFAULT_BASE: u64 = 0xFEC0_0000;

// ---------------------------------------------------------------------------------------------
// IA32_APIC_BASE (MSR 0x1B)
// ---------------------------------------------------------------------------------------------

/// `IA32_APIC_BASE.BSP` (bit 8) — `1` ⇔ CPU này là bootstrap processor.
pub const APIC_BASE_BSP: u64 = 1 << 8;
/// `IA32_APIC_BASE.EN` (bit 11) — `1` ⇔ APIC toàn cục được bật.
pub const APIC_BASE_ENABLE: u64 = 1 << 11;
/// Mặt nạ địa chỉ base của `IA32_APIC_BASE` (bit 12..51; SDM Hình 10-5).
pub const APIC_BASE_ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

// ---------------------------------------------------------------------------------------------
// Thanh ghi LAPIC (MMIO, 32-bit, căn 16 byte)
// ---------------------------------------------------------------------------------------------

/// `LAPIC_ID` — ID của LAPIC (bit 24..31).
pub const LAPIC_ID: u32 = 0x020;
/// `LAPIC_VERSION` — version (bit 0..7) + số LVT entry tối đa (bit 16..23).
pub const LAPIC_VERSION: u32 = 0x030;
/// `LAPIC_TPR` — task priority (bit 0..7); `0` ⇒ nhận mọi ưu tiên.
pub const LAPIC_TPR: u32 = 0x080;
/// `LAPIC_EOI` — ghi `0` để báo "đã xử lý xong" (SDM §10.6.1).
pub const LAPIC_EOI: u32 = 0x0B0;
/// `LAPIC_SVR` — spurious vector (bit 0..7) + APIC software enable (bit 8).
pub const LAPIC_SVR: u32 = 0x0F0;
/// `LAPIC_LVT_TIMER` — vector (bit 0..7), mask (bit 16), periodic (bit 17).
pub const LAPIC_LVT_TIMER: u32 = 0x320;
/// `LAPIC_LVT_LINT0` — chân ngắt ngoài 0 (thường nối 8259 PIC ở chế độ virtual wire).
pub const LAPIC_LVT_LINT0: u32 = 0x350;
/// `LAPIC_LVT_LINT1` — chân ngắt ngoài 1 (thường NMI).
pub const LAPIC_LVT_LINT1: u32 = 0x360;
/// `LAPIC_TIMER_INITIAL_COUNT` — nạp giá trị đếm đầu tiên (bắt đầu đếm).
pub const LAPIC_TIMER_INITIAL_COUNT: u32 = 0x380;
/// `LAPIC_TIMER_CURRENT_COUNT` — giá trị đếm hiện tại (chỉ đọc).
pub const LAPIC_TIMER_CURRENT_COUNT: u32 = 0x390;
/// `LAPIC_TIMER_DIVIDE` — mã chia 4 bit (SDM Bảng 10-2).
pub const LAPIC_TIMER_DIVIDE: u32 = 0x3E0;

/// Vị trí bit của ID trong `LAPIC_ID` (SDM Hình 10-6).
pub const LAPIC_ID_SHIFT: u32 = 24;
/// Mặt nạ byte version trong `LAPIC_VERSION`.
pub const LAPIC_VERSION_MASK: u32 = 0xFF;

/// `SVR.APIC_ENABLE` (bit 8) — bật APIC bằng phần mềm.
pub const SVR_APIC_ENABLE: u32 = 1 << 8;
/// Mặt nạ vector trong `SVR` (bit 0..7).
pub const SVR_VECTOR_MASK: u32 = 0xFF;

/// Mặt nạ vector trong `LVT_TIMER` (bit 0..7).
pub const LVT_VECTOR_MASK: u32 = 0xFF;
/// `LVT_TIMER.MASK` (bit 16) — `1` ⇔ timer bị che.
pub const LVT_MASKED: u32 = 1 << 16;
/// `LVT_TIMER.TIMER_MODE` (bit 17) — `1` ⇔ periodic, `0` ⇔ one-shot.
pub const LVT_TIMER_PERIODIC: u32 = 1 << 17;

/// Mã chia **1** (SDM Bảng 10-2: `1011`).
pub const TIMER_DIVIDE_BY_1: u32 = 0b1011;
/// Mã chia **2** (`0000`).
pub const TIMER_DIVIDE_BY_2: u32 = 0b0000;
/// Mã chia **4** (`0001`).
pub const TIMER_DIVIDE_BY_4: u32 = 0b0001;
/// Mã chia **8** (`0010`).
pub const TIMER_DIVIDE_BY_8: u32 = 0b0010;
/// Mã chia **16** (`0011`).
pub const TIMER_DIVIDE_BY_16: u32 = 0b0011;
/// Mã chia **32** (`1000`).
pub const TIMER_DIVIDE_BY_32: u32 = 0b1000;
/// Mã chia **64** (`1001`).
pub const TIMER_DIVIDE_BY_64: u32 = 0b1001;
/// Mã chia **128** (`1010`).
pub const TIMER_DIVIDE_BY_128: u32 = 0b1010;

/// ID của LAPIC từ giá trị `LAPIC_ID` (bit 24..31).
pub const fn lapic_id(raw: u32) -> u8 {
    ((raw >> LAPIC_ID_SHIFT) & 0xFF) as u8
}

/// Version của LAPIC từ giá trị `LAPIC_VERSION` (byte thấp).
pub const fn lapic_version(raw: u32) -> u8 {
    (raw & LAPIC_VERSION_MASK) as u8
}

/// Giá trị ghi vào `SVR`: vector spurious + **bật** APIC.
pub const fn svr_value(vector: u8) -> u32 {
    SVR_APIC_ENABLE | ((vector as u32) & SVR_VECTOR_MASK)
}

/// Giá trị ghi vào `LVT_TIMER`: vector + chế độ, **không** mask.
pub const fn lvt_timer_value(vector: u8, periodic: bool) -> u32 {
    ((vector as u32) & LVT_VECTOR_MASK) | if periodic { LVT_TIMER_PERIODIC } else { 0 }
}

/// `true` ⇔ `LVT_TIMER` đang bị che.
pub const fn lvt_is_masked(raw: u32) -> bool {
    raw & LVT_MASKED != 0
}

/// Vector trong `LVT_TIMER`.
pub const fn lvt_vector(raw: u32) -> u8 {
    (raw & LVT_VECTOR_MASK) as u8
}

/// `true` ⇔ hai ảnh `LAPIC_TIMER_CURRENT_COUNT` chứng minh timer **periodic** đang chạy.
///
/// # Vì sao **không** so `count1 < count0` (KT-297)
///
/// Ảnh đặt `LVT_TIMER` **periodic** ([`LVT_TIMER_PERIODIC`], bit 17): counter đếm lùi từ
/// `INITIAL_COUNT` rồi **nạp lại**, nên giữa hai lần đọc nó có thể **quay vòng** và
/// `count1 > count0` là hợp lệ. KT-293 soak 600 boot bắt **4 boot `runs=0` giả** vì phép kiểm
/// one-shot cũ (`count1 < count0`) gặp quay vòng — nguyên văn:
/// `count0=317000 count1=803001`, `233000→718001`, `969000→975002`, `18003→472004`.
///
/// Bằng chứng "timer chạy" là counter **đổi giá trị** trong một cửa sổ có trần (ảnh chờ tới khi
/// đổi, không đọc hai lần liền nhau), **và** cả hai ảnh nằm trong dải kiến trúc
/// `0..=initial`:
///
/// - `0` là ảnh chụp hiếm khi counter vừa hết hạn, chưa nạp lại — vẫn hợp lệ nếu ảnh kia khác;
/// - `0xFFFF_FFFF` (không có APIC / MMIO không map) và mọi giá trị `> initial` ⇒ **fail-closed**.
///
/// Hàm **thuần** để test host chốt được (D2-05); ảnh chỉ đọc MMIO rồi gọi hàm này.
pub const fn lapic_timer_runs(count0: u32, count1: u32, initial: u32) -> bool {
    count0 != count1 && count0 <= initial && count1 <= initial
}

/// Số chia từ **mã** 4 bit; mã không dùng ⇒ `None` (không đoán bừa).
pub const fn timer_divide_value(code: u32) -> Option<u32> {
    match code {
        TIMER_DIVIDE_BY_1 => Some(1),
        TIMER_DIVIDE_BY_2 => Some(2),
        TIMER_DIVIDE_BY_4 => Some(4),
        TIMER_DIVIDE_BY_8 => Some(8),
        TIMER_DIVIDE_BY_16 => Some(16),
        TIMER_DIVIDE_BY_32 => Some(32),
        TIMER_DIVIDE_BY_64 => Some(64),
        TIMER_DIVIDE_BY_128 => Some(128),
        _ => None,
    }
}

/// **Mã** 4 bit từ số chia; số chia ngoài bảng ⇒ `None`.
pub const fn timer_divide_code(divide: u32) -> Option<u32> {
    match divide {
        1 => Some(TIMER_DIVIDE_BY_1),
        2 => Some(TIMER_DIVIDE_BY_2),
        4 => Some(TIMER_DIVIDE_BY_4),
        8 => Some(TIMER_DIVIDE_BY_8),
        16 => Some(TIMER_DIVIDE_BY_16),
        32 => Some(TIMER_DIVIDE_BY_32),
        64 => Some(TIMER_DIVIDE_BY_64),
        128 => Some(TIMER_DIVIDE_BY_128),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// IOAPIC
// ---------------------------------------------------------------------------------------------

/// `IOREGSEL` — ghi chỉ số thanh ghi IOAPIC (offset 0x00).
pub const IOAPIC_REGSEL: u32 = 0x00;
/// `IOWIN` — đọc/ghi dữ liệu thanh ghi đã chọn (offset 0x10).
pub const IOAPIC_WIN: u32 = 0x10;
/// Chỉ số thanh ghi `IOAPICID`.
pub const IOAPIC_ID_REG: u32 = 0x00;
/// Chỉ số thanh ghi `IOAPICVER`.
pub const IOAPIC_VER_REG: u32 = 0x01;
/// Chỉ số thanh ghi **đầu** của redirection table (`IOREDTBL0`).
pub const IOAPIC_REDTBL_BASE: u32 = 0x10;
/// Mỗi redirection entry chiếm **hai** thanh ghi 32-bit (low, high).
pub const IOAPIC_REDTBL_STRIDE: u32 = 2;
/// Số redirection entry tối đa theo đặc tả (24 pin; `IOAPICVER` cho biết số thật).
pub const IOAPIC_MAX_REDIRECTIONS: u32 = 24;
/// `IOREDTBL.MASK` (bit 16 của thanh ghi low) — `1` ⇔ ngắt bị che.
pub const IOAPIC_REDIR_MASKED: u32 = 1 << 16;

/// ID của IOAPIC từ `IOAPICID` (bit 24..27).
pub const fn ioapic_id(raw: u32) -> u8 {
    ((raw >> 24) & 0xF) as u8
}

/// Version của IOAPIC từ `IOAPICVER` (byte thấp).
pub const fn ioapic_version(raw: u32) -> u8 {
    (raw & 0xFF) as u8
}

/// Số redirection entry từ `IOAPICVER`: bit 16..23 **+ 1** (SDM Hình 11-7).
pub const fn ioapic_redirection_count(raw: u32) -> u32 {
    ((raw >> 16) & 0xFF) + 1
}

/// Bật bit mask của một redirection entry (thanh ghi low), **không** đụng bit khác.
pub const fn ioapic_redir_low_masked(low: u32) -> u32 {
    low | IOAPIC_REDIR_MASKED
}

/// `true` ⇔ redirection entry đang bị che.
pub const fn ioapic_redir_is_masked(low: u32) -> bool {
    low & IOAPIC_REDIR_MASKED != 0
}

/// Chỉ số thanh ghi **low** của redirection entry `pin`; `pin >= 24` ⇒ `None`.
pub const fn ioapic_redtbl_reg(pin: u32) -> Option<u32> {
    if pin < IOAPIC_MAX_REDIRECTIONS {
        Some(IOAPIC_REDTBL_BASE + pin * IOAPIC_REDTBL_STRIDE)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------------
// 8259 PIC — phải **che** khi chuyển sang APIC (KT-266)
// ---------------------------------------------------------------------------------------------
//
// Trên PC, 8259 PIC nối vào `LAPIC.LINT0` (virtual wire). BIOS đặt vector IRQ0 = `0x08` — trùng
// `#DF`! Nếu không che PIC, ngắt PIT (IRQ0) được giao **thẳng** cho CPU với vector 8 và ảnh chết
// trong `#DF` ngay sau `sti` (đo được ở lần chạy đầu KT-266). Che cả hai PIC bằng `0xFF` là bước
// chuẩn khi dùng IOAPIC/LAPIC.

/// Cổng dữ liệu (IMR) của PIC1 (master, IRQ0..7).
pub const PIC1_DATA: u16 = 0x21;
/// Cổng dữ liệu (IMR) của PIC2 (slave, IRQ8..15).
pub const PIC2_DATA: u16 = 0xA1;
/// Giá trị IMR: che **toàn bộ** 8 IRQ của một PIC.
pub const PIC_MASK_ALL: u8 = 0xFF;

// ---------------------------------------------------------------------------------------------
// PIT kênh 0 — nguồn thời gian tham chiếu để **đo** `hz`
// ---------------------------------------------------------------------------------------------

/// Cổng dữ liệu PIT kênh 0.
pub const PIT_CH0_DATA: u16 = 0x40;
/// Cổng lệnh PIT (dùng chung ba kênh).
pub const PIT_CH0_CMD: u16 = 0x43;
/// Tần số PIT: 1,193182 MHz (Intel 8254).
pub const PIT_FREQUENCY_HZ: u32 = 1_193_182;
/// Divisor lớn nhất của PIT (16-bit) — mode 0 với divisor này đếm ~54,9 ms.
pub const PIT_MAX_DIVISOR: u16 = 0xFFFF;
/// Lệnh: kênh 0, lobyte/hibyte, **mode 0** (interrupt on terminal count), nhị phân.
pub const PIT_CMD_CH0_MODE0_LOHI: u8 = 0x30;
/// Lệnh: kênh 0, lobyte/hibyte, **mode 2** (rate generator — tự nạp lại, nguồn tick), nhị phân.
pub const PIT_CMD_CH0_MODE2_LOHI: u8 = 0x34;
/// Lệnh: **latch** counter kênh 0 (đọc ổn định trong lúc đang đếm).
pub const PIT_CMD_LATCH_CH0: u8 = 0x00;

/// Cổng dữ liệu PIT **kênh 2** (loa) — dùng làm **đồng hồ đo** độc lập với kênh 0 (nguồn tick).
pub const PIT_CH2_DATA: u16 = 0x42;
/// Lệnh: kênh 2, lobyte/hibyte, **mode 2** (rate generator), nhị phân.
pub const PIT_CMD_CH2_MODE2_LOHI: u8 = 0xB4;
/// Lệnh: **latch** counter kênh 2.
pub const PIT_CMD_LATCH_CH2: u8 = 0x80;
/// Cổng điều khiển loa: bit 0 = gate kênh 2, bit 1 = dữ liệu loa.
pub const PIT_SPEAKER_PORT: u16 = 0x61;
/// Bit gate kênh 2 — phải `1` thì kênh 2 mới đếm.
pub const PIT_SPEAKER_GATE2: u8 = 1 << 0;
/// Bit dữ liệu loa — giữ `0` để không bật loa.
pub const PIT_SPEAKER_DATA: u8 = 1 << 1;

/// Số tick PIT đã trôi qua giữa hai lần đọc counter (modulo 65536).
pub const fn pit_elapsed_ticks(start: u16, end: u16) -> u32 {
    (start.wrapping_sub(end)) as u32
}

/// `true` ⇔ counter PIT đọc **ngay sau** khi nạp mode 2 khớp `divisor` đã nạp.
///
/// # Vì sao cần kiểm này (KT-297)
///
/// Counter mode 2 đếm lùi từ `divisor`; đọc ngay sau khi nạp (vài µs) nên giá trị còn ≈
/// `divisor`. Dải chấp nhận `[divisor/2, divisor]`:
///
/// - cận trên `divisor`: counter không thể lớn hơn giá trị vừa nạp;
/// - cận dưới `divisor/2`: đọc trễ tới nửa chu kỳ (5 ms ở 100 Hz) vẫn chấp nhận.
///
/// Dưới TCG, tốc độ tick **giao được** bị chặn trên bởi độ trễ đánh thức `hlt` (~20 ms) nên
/// `hz` đo được **bão hoà** ~35..55 Hz cho cả nguồn 100 Hz lẫn 1.000 Hz. Kiểm divisor trực tiếp
/// là lớp bắt **đột biến `wrong-hz`** (đổi divisor) mà phép đo `hz` không phân biệt được.
pub const fn pit_divisor_ok(counter0: u16, divisor: u16) -> bool {
    counter0 as u32 >= divisor as u32 / 2 && counter0 as u32 <= divisor as u32
}

/// `hz = ticks × 1_193_182 / pit_ticks` — chia 0 ⇒ `0` (fail-closed, không panic trong ảnh).
pub const fn hz_from_pit(ticks: u32, pit_ticks: u32) -> u32 {
    if pit_ticks == 0 {
        return 0;
    }
    ((ticks as u64 * PIT_FREQUENCY_HZ as u64) / pit_ticks as u64) as u32
}

// ---------------------------------------------------------------------------------------------
// Khoảng `hz` hợp lệ của nguồn tick 100 Hz — KT-297
// ---------------------------------------------------------------------------------------------

/// Sàn `hz` **giao được** của nguồn tick PIT 100 Hz dưới TCG.
///
/// Danh định là 100 Hz; dưới TCG QEMU giao tick **không đều** và tốc độ **giao được** bị chặn
/// trên bởi độ trễ đánh thức `hlt` (~20 ms). KT-297 đo trên cửa sổ cố định 500 ms: 29..51 Hz
/// (p50 37). Sàn 20 Hz là ~1,5× dưới mức thấp nhất đo được — đủ rộng cho host bận, vẫn bắt
/// nguồn tick **quá chậm**.
pub const HZ_MIN: u32 = 20;
/// Trần `hz` **giao được** — 500 Hz, rộng hơn nhiều mức đo được (max 51 Hz). Đây là **chặn
/// trên an toàn** (giá trị vô lý), **không** phải phép kiểm divisor: dưới TCG tốc độ giao được
/// **bão hoà** ở ~55 Hz nên một nguồn 1.000 Hz vẫn đo ra ~55 Hz. Việc bắt nguồn sai nhịp nằm ở
/// [`pit_divisor_ok`] (đọc lại divisor từ counter PIT).
pub const HZ_MAX: u32 = 500;

/// `true` ⇔ `hz` **giao được** nằm trong khoảng hợp lệ [`HZ_MIN`]..=[`HZ_MAX`].
///
/// # Vì sao thêm kiểm này (KT-297)
///
/// Trước KT-297 cổng chỉ đòi `hz > 0`, nên một nguồn tick **quá chậm** (hoặc giá trị vô lý)
/// vẫn xanh miễn là còn tick. Đây là **kiểm đúng đắn** (chặt hơn phép kiểm cũ), khoảng chốt từ
/// số đo thật của soak — **không** phải nới ngưỡng. Nguồn tick **sai divisor** được bắt riêng
/// bằng [`pit_divisor_ok`] vì `hz` giao được bão hoà dưới TCG.
pub const fn hz_ok(hz: u32) -> bool {
    hz >= HZ_MIN && hz <= HZ_MAX
}
