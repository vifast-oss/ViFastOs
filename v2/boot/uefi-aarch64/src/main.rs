// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-414b** — ảnh UEFI aarch64 (`PE/COFF`, entry `efi_main`): chụp `GetMemoryMap` **ngay trước**
//! `ExitBootServices`, kiểm **fail-closed có tên**, dựng **`BootInfo` v1 nguồn `Uefi`** (D2-260)
//! + bản ghi bàn giao `UefiHandoffV1`, in bằng chứng `# uefi:`/`# bootinfo:` ra `ConOut`.
//!
//! Sau đó thoát dịch vụ boot và báo kết quả cho runner bằng **ARM semihosting**
//! (`SYS_EXIT_EXTENDED`).
//!
//! Thứ tự cố định (không đổi để log so sánh được giữa các lần chạy — **cùng** thứ tự với
//! `v2/boot/uefi-x86_64/src/main.rs`, KT-414):
//!
//! 1. `# uefi: app=…` + firmware + chế độ build (naive/fault);
//! 2. `GetMemoryMap` (hỏi cỡ → đọc vào đệm 64 KiB tĩnh);
//! 3. tiêm lỗi nếu `VIFAST_UEFI_FAULT` (chỉ harness) → **kiểm** (`memmap::validate`);
//! 4. `# uefi: memmap=ok …` + `# uefi: cfgtable …` (RSDP/DTB từ `EFI_CONFIGURATION_TABLE`);
//! 5. `# uefi: dtb …` — kiểm header DTB **khi firmware khai** (`dtb_pa != 0`), fail-closed
//!    `bad-dtb`; AAVMF đo được `dtb_pa=0` (cấp ACPI) ⇒ `check=none`;
//! 6. dựng `BootInfo` v1 `source=uefi` + `validate()` **fail-closed theo nguồn** rồi in
//!    `# bootinfo:` **nguyên văn** bằng `BootInfo::write_log` của kernel;
//! 7. dựng + `verify_for(ARCH_AARCH64)` bản ghi bàn giao (`rsdp_pa`/`dtb_pa`), in địa chỉ/kích
//!    thước/checksum + **bất biến trang bàn giao** (reserved/copy khi nhảy `e_entry`);
//! 8. `GetMemoryMap` làm mới `MapKey` → `ExitBootServices` (thử lại khi `EFI_INVALID_PARAMETER`);
//! 9. sau khi thoát: `# uefi: after-ebs=ok` qua **semihosting** (ConOut đã chết) → thoát QEMU
//!    bằng `SYS_EXIT_EXTENDED` với `rc = 33` (cùng thang `rc` với x86_64).
//!
//! Đường lỗi: in `# uefi: fail-closed err=<mã> code=<mã thoát>` rồi thoát QEMU `rc = 65/67` —
//! **không** gọi `ExitBootServices` khi bảng nhớ hỏng **hoặc** `BootInfo` không dựng được (ví dụ
//! bảng nhớ hợp lệ nhưng không còn vùng RAM usable nào).
//!
//! # Khác x86_64 (KT-414) — và vì sao
//!
//! | Việc | x86_64 | aarch64 |
//! |---|---|---|
//! | Báo kết quả sau `ExitBootServices` | cổng I/O `isa-debug-exit` (`0x501`) | **semihosting** `SYS_EXIT_EXTENDED` (`hlt #0xF000`) |
//! | Bằng chứng sau `ExitBootServices` | `isa-debugcon` (`0xE9`) | **semihosting** `SYS_WRITE0` (QEMU: stderr) |
//! | `arch` trong bản ghi | `ARCH_X86_64` | `ARCH_AARCH64` |
//! | `BootArch` của `BootInfo` | `X86_64` | `Aarch64` |
//! | DTB | không có (`dtb_pa=0`) | kiểm header khi firmware khai |
//!
//! Bốn mô-đun `efi`/`memmap`/`handoff`/`console` **không** có bản riêng: chúng được `#[path]`
//! include từ `v2/boot/uefi-x86_64/src/` (xem `src/lib.rs`).

#![no_std]
#![no_main]

use vifast_boot_uefi_aarch64::bootinfo::BootInfo;
use vifast_boot_uefi_aarch64::console::{
    line, ConOutWriter, EXIT_FAIL_EBS, EXIT_FAIL_MAP, EXIT_OK,
};
use vifast_boot_uefi_aarch64::efi::{
    find_config_table, BootServices, MemoryDescriptor, SimpleTextOutput, SystemTable,
    ACPI_10_TABLE_GUID, ACPI_20_TABLE_GUID, DTB_TABLE_GUID, EFI_BUFFER_TOO_SMALL,
    EFI_INVALID_PARAMETER, EFI_SUCCESS,
};
use vifast_boot_uefi_aarch64::handoff::{
    UefiHandoffV1, ARCH_AARCH64, HANDOFF_EMPTY, HANDOFF_RESERVED_INVARIANT,
};
use vifast_boot_uefi_aarch64::memmap::{self, MapInfo, MemMapStats, UefiError};
use vifast_boot_uefi_aarch64::semihost;

/// Tên lỗi tiêm lúc build (`""` = ảnh sản phẩm). Xem `build.rs`.
const FAULT: &str = env!("VIFAST_UEFI_FAULT");
/// Tái hiện hành vi **trước khi vá** (bỏ kiểm bảng nhớ) — chỉ dùng để chứng minh ca âm đỏ.
const NAIVE: bool = cfg!(uefi_naive);

/// Đệm bảng nhớ (tĩnh, không cấp phát): AAVMF ~60 mô tả × 48 B ≈ 3 KiB; 64 KiB là dư.
const MAP_CAP: usize = 64 * 1024;
/// Số lần thử tối đa khi `ExitBootServices` trả `EFI_INVALID_PARAMETER`.
const EBS_TRIES: u8 = 8;

static mut MAP_BUF: [u8; MAP_CAP] = [0; MAP_CAP];
static mut HANDOFF: UefiHandoffV1 = HANDOFF_EMPTY;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // SAFETY: semihosting còn sống sau ExitBootServices; panic handler không được trả về.
    unsafe {
        semihost::write0("# uefi: panic");
    }
    // SAFETY: thoát QEMU; không trả về.
    unsafe { semihost::exit_qemu(EXIT_FAIL_MAP) }
}

/// In một dòng `ConOut` (chỉ trước `ExitBootServices`).
macro_rules! ulog {
    ($out:expr, $($arg:tt)*) => {
        // SAFETY: `out` là ConOut của firmware; mọi lời gọi đều nằm trước ExitBootServices.
        unsafe { line($out, core::format_args!($($arg)*)) }
    };
}

/// `GetMemoryMap` một lần: đệm tĩnh, lặp khi `EFI_BUFFER_TOO_SMALL`.
fn get_memory_map(bs: *mut BootServices, cap: usize) -> Result<MapInfo, UefiError> {
    let mut size = cap;
    let mut key = 0usize;
    let mut desc_size = 0usize;
    let mut desc_version = 0u32;
    let mut tries = 0u8;
    loop {
        // SAFETY: đệm tĩnh 64 KiB; `bs` là BootServices hợp lệ trước ExitBootServices.
        let buf = core::ptr::addr_of_mut!(MAP_BUF) as *mut u8;
        // SAFETY: `buf` đủ `size` byte (== cap) và `bs` hợp lệ.
        let status = unsafe {
            ((*bs).get_memory_map)(
                &mut size,
                buf.cast::<MemoryDescriptor>(),
                &mut key,
                &mut desc_size,
                &mut desc_version,
            )
        };
        if status == EFI_SUCCESS {
            return Ok(MapInfo {
                map_size: size,
                desc_size,
                desc_version,
                map_key: key,
                capacity: cap,
            });
        }
        if status == EFI_BUFFER_TOO_SMALL {
            if size > cap {
                return Err(UefiError::MapTruncated);
            }
            tries += 1;
            if tries <= EBS_TRIES {
                continue;
            }
        }
        return Err(UefiError::MapQuery);
    }
}

/// In bằng chứng bảng nhớ đã kiểm (dòng ổn định `# uefi:`; phần `# bootinfo:` do
/// [`print_bootinfo`] in **nguyên văn** bằng `BootInfo::write_log` của kernel).
fn print_memmap(out: *mut SimpleTextOutput, info: &MapInfo, st: &MemMapStats) {
    ulog!(
        out,
        "# uefi: memmap=ok count={} desc_size={} desc_version={} map_size=0x{:x} sum=0x{:x}",
        st.count,
        info.desc_size,
        info.desc_version,
        info.map_size,
        st.sum
    );
    ulog!(
        out,
        "# uefi: memmap: n_usable={} usable_bytes=0x{:x} highest_phys=0x{:x} n_runtime={} n_acpi={} n_reserved={} attr_odd={} types=0x{:x}",
        st.n_usable,
        st.usable_bytes,
        st.highest_phys,
        st.n_runtime,
        st.n_acpi,
        st.n_reserved,
        st.n_attr_odd,
        st.type_mask
    );
}

/// In `BootInfo::write_log` **nguyên văn** — cùng định dạng `# bootinfo:` với 2 arch còn lại
/// (không chép chuỗi định dạng ở boot stub).
fn print_bootinfo(out: *mut SimpleTextOutput, info: &BootInfo) {
    let mut w = ConOutWriter::new(out);
    let _ = info.write_log(&mut w);
    // SAFETY: ConOut còn hiệu lực (mọi lời gọi đều trước ExitBootServices).
    unsafe { w.flush() };
}

/// Đọc `EFI_CONFIGURATION_TABLE` → `(rsdp_pa, dtb_pa)`; **0** nếu firmware không khai.
///
/// Trước `ExitBootServices` mọi vùng firmware được identity-map ⇒ VA = PA. RSDP ưu tiên bảng
/// ACPI 2.0, fallback ACPI 1.0 (firmware cũ); DTB lấy theo `DTB_TABLE_GUID` — AAVMF
/// (edk2-stable202408) **không** cấp FDT config table (nó cấp ACPI) nên `dtb_pa=0` là **đúng**,
/// không phải lỗi.
fn config_tables(st: *mut SystemTable) -> (u64, u64) {
    // SAFETY: `st` do firmware truyền, hợp lệ trước ExitBootServices.
    let (ptr, n) = unsafe { ((*st).configuration_table, (*st).number_of_table_entries) };
    if ptr.is_null() || n == 0 {
        return (0, 0);
    }
    // SAFETY: firmware khai `n` phần tử `EFI_CONFIGURATION_TABLE` tại `ptr`.
    let tables = unsafe { core::slice::from_raw_parts(ptr, n) };
    let rsdp = {
        let acpi20 = find_config_table(tables, &ACPI_20_TABLE_GUID);
        if acpi20 != 0 {
            acpi20
        } else {
            find_config_table(tables, &ACPI_10_TABLE_GUID)
        }
    };
    (rsdp, find_config_table(tables, &DTB_TABLE_GUID))
}

/// Kiểm header DTB mà firmware khai (`dtb_pa`): magic `0xd00dfeed` (big-endian) + `totalsize`.
///
/// `dtb_pa == 0` ⇒ firmware không khai (AAVMF đo được: 0) — **không** phải lỗi, in `check=none`.
/// Có DTB mà header hỏng ⇒ `Err("bad-dtb")` (fail-closed, **không** `ExitBootServices`).
fn check_dtb(out: *mut SimpleTextOutput, dtb_pa: u64) -> Result<(), &'static str> {
    if dtb_pa == 0 {
        ulog!(
            out,
            "# uefi: dtb pa=0x0 check=none (firmware khong khai FDT)"
        );
        return Ok(());
    }
    // SAFETY: firmware khai DTB tại PA; trước ExitBootServices mọi vùng firmware identity-map.
    let (magic, totalsize) = unsafe {
        let p = dtb_pa as usize as *const u8;
        (
            u32::from_be_bytes([
                core::ptr::read(p),
                core::ptr::read(p.add(1)),
                core::ptr::read(p.add(2)),
                core::ptr::read(p.add(3)),
            ]),
            u32::from_be_bytes([
                core::ptr::read(p.add(4)),
                core::ptr::read(p.add(5)),
                core::ptr::read(p.add(6)),
                core::ptr::read(p.add(7)),
            ]),
        )
    };
    let res = vifast_boot_uefi_aarch64::dtb_header_ok(magic, totalsize);
    ulog!(
        out,
        "# uefi: dtb pa=0x{:x} magic=0x{:08x} totalsize={} check={}",
        dtb_pa,
        magic,
        totalsize,
        if res.is_ok() { "ok" } else { "FAIL" }
    );
    res
}

/// Dựng, `verify_for(ARCH_AARCH64)` và đặt bản ghi bàn giao vào `static`; in địa chỉ/kích
/// thước/checksum + **bất biến trang bàn giao** (reserved/copy khi nhảy `e_entry`).
fn install_handoff(
    out: *mut SimpleTextOutput,
    info: &MapInfo,
    st: &MemMapStats,
    rsdp_pa: u64,
    dtb_pa: u64,
) {
    let mut rec = UefiHandoffV1::build_for(ARCH_AARCH64, st, info, rsdp_pa, dtb_pa);
    rec.seal();
    match rec.verify_for(ARCH_AARCH64) {
        Ok(()) => {}
        Err(e) => fail(out, "bad-handoff", EXIT_FAIL_MAP, Some(e)),
    }
    // SAFETY: ghi vào `static` của ảnh; không ai đọc đồng thời (single-thread boot).
    let ptr = core::ptr::addr_of_mut!(HANDOFF);
    // SAFETY: `ptr` hợp lệ, `rec` là bản ghi đã seal.
    unsafe { ptr.write(rec) };
    let pa = ptr as usize as u64;
    ulog!(
        out,
        "# uefi: handoff magic=0x{:016x} version={} arch={} va=0x{:x} size={} n_ram={} rsdp_pa=0x{:x} dtb_pa=0x{:x} checksum=0x{:08x}",
        rec.magic,
        rec.version,
        rec.arch,
        pa,
        rec.size_bytes,
        rec.n_ram,
        rec.rsdp_pa,
        rec.dtb_pa,
        rec.checksum
    );
    ulog!(out, "# uefi: handoff verify=ok checksum=ok");
    // **Bất biến trang bàn giao** (D2-260): ảnh `.efi` nằm trong `EfiLoaderCode/Data` — sau
    // `ExitBootServices` vùng đó được nhả. Kernel **phải** giữ `[pa, pa + size_bytes)` reserved
    // hoặc chép bản ghi ra trang riêng **trước** khi cấp phát RAM. In ra để runner đối chiếu.
    ulog!(
        out,
        "# uefi: handoff reserved pa=0x{:x} bytes={} pages={} invariant={}",
        pa,
        rec.size_bytes,
        rec.reserved_pages(pa),
        HANDOFF_RESERVED_INVARIANT
    );
}

/// Đường lỗi duy nhất: in lý do **có tên** + thoát QEMU mã lỗi (KHÔNG gọi `ExitBootServices`).
fn fail(
    out: *mut SimpleTextOutput,
    name: &'static str,
    code: u32,
    detail: Option<&'static str>,
) -> ! {
    let detail = detail.unwrap_or("-");
    ulog!(
        out,
        "# uefi: fail-closed err={} detail={} code=0x{:x}",
        name,
        detail,
        code
    );
    ulog!(
        out,
        "# uefi: memmap=FAIL err={} exitbootservices=not-called",
        name
    );
    ulog!(
        out,
        "# uefi: exit=0x{:x} (semihosting SYS_EXIT_EXTENDED ⇒ rc {})",
        code,
        semihost::qemu_rc(code)
    );
    // SAFETY: semihosting còn sống sau ExitBootServices; sau đó thoát QEMU (không trả về).
    unsafe {
        semihost::write0("# uefi: fail-closed");
        semihost::exit_qemu(code)
    }
}

fn main_impl(image: usize, st: *mut SystemTable) -> ! {
    // SAFETY: `st` do firmware truyền, hợp lệ trước ExitBootServices.
    let (out, bs, revision, n_tables) = unsafe {
        (
            (*st).con_out,
            (*st).boot_services,
            (*st).hdr.revision,
            (*st).number_of_table_entries,
        )
    };
    ulog!(
        out,
        "# uefi: app=vifast-boot-aarch64 v=1 target=aarch64-unknown-uefi"
    );
    ulog!(
        out,
        "# uefi: firmware rev=0x{:08x} tables={} naive={} fault={}",
        revision,
        n_tables,
        u8::from(NAIVE),
        if FAULT.is_empty() { "none" } else { FAULT }
    );

    // (2) Chụp bảng nhớ.
    let mut info = match get_memory_map(bs, MAP_CAP) {
        Ok(i) => i,
        Err(e) => fail(out, e.name(), e.exit_val(), None),
    };
    ulog!(
        out,
        "# uefi: memmap=query desc_size={} desc_version={} map_size=0x{:x} key=0x{:x}",
        info.desc_size,
        info.desc_version,
        info.map_size,
        info.map_key
    );

    // Bằng chứng thô: 8 mô tả đầu (loại/trang/thuộc tính) — đối chiếu với firmware thật.
    {
        let p = core::ptr::addr_of_mut!(MAP_BUF) as *const u8;
        let n = info.count().min(8);
        for i in 0..n {
            // SAFETY: `i < count` ⇒ `i*desc_size + 40 <= map_size <= 64 KiB`.
            let d = unsafe {
                core::ptr::read_unaligned(p.add(i * info.desc_size).cast::<MemoryDescriptor>())
            };
            ulog!(
                out,
                "# uefi: d[{}] ty={} phys=0x{:x} pages={} attr=0x{:x}",
                i,
                d.ty,
                d.physical_start,
                d.number_of_pages,
                d.attribute
            );
        }
    }

    // (3) Tiêm lỗi (harness) + kiểm fail-closed.
    if !FAULT.is_empty() {
        // SAFETY: đệm tĩnh 64 KiB; `info` khớp lần GetMemoryMap vừa ghi.
        let ptr = core::ptr::addr_of_mut!(MAP_BUF) as *mut u8;
        let injected = unsafe { memmap::apply_fault(ptr, &mut info, FAULT) };
        ulog!(
            out,
            "# uefi: fault={} injected={} (bang nho bi lam hong co chu dich - ca am)",
            FAULT,
            u8::from(injected)
        );
    }

    let stats: Option<MemMapStats> = if NAIVE {
        // Bản "trước khi vá": bỏ toàn bộ phép kiểm, vẫn thoát dịch vụ boot. Chỉ dùng để chứng
        // minh ca âm là **đỏ** trên bản chưa vá (xem run.sh --all-cases).
        ulog!(
            out,
            "# uefi: memmap=ok count={} naive=1 (KHONG kiem - hanh vi truoc khi va)",
            info.count()
        );
        None
    } else {
        // SAFETY: đệm tĩnh 64 KiB; `info` khớp lần GetMemoryMap vừa ghi.
        let ptr = core::ptr::addr_of_mut!(MAP_BUF) as *const u8;
        match unsafe { memmap::validate(ptr, &info) } {
            Ok(s) => {
                print_memmap(out, &info, &s);
                Some(s)
            }
            Err(e) => fail(out, e.name(), e.exit_val(), None),
        }
    };

    // (4b) `EFI_CONFIGURATION_TABLE` → RSDP/DTB PA (0 nếu firmware không khai) — nguồn để kernel
    // parse ACPI/DTB **sau** khi đã có bảng nhớ, không phải đoán địa chỉ.
    let (rsdp_pa, dtb_pa) = config_tables(st);
    ulog!(
        out,
        "# uefi: cfgtable n={} rsdp_pa=0x{:x} dtb_pa=0x{:x}",
        n_tables,
        rsdp_pa,
        dtb_pa
    );

    // (4c) DTB (aarch64): kiểm header **khi firmware khai** — fail-closed `bad-dtb`.
    if let Err(e) = check_dtb(out, dtb_pa) {
        fail(out, e, EXIT_FAIL_MAP, Some("dtb-header"));
    }

    // (5) `BootInfo` v1 nguồn `Uefi` (D2-260) + bản ghi bàn giao — chỉ khi bảng nhớ đã kiểm sạch.
    if let Some(s) = stats {
        match vifast_boot_uefi_aarch64::bootinfo_from_memmap(&s) {
            Ok(bi) => print_bootinfo(out, &bi),
            // Fail-closed theo nguồn: bảng nhớ "hợp lệ theo UEFI" nhưng `BootInfo` từ chối
            // (ví dụ không còn vùng RAM usable nào) ⇒ **không** ExitBootServices.
            Err(e) => fail(
                out,
                e.name(),
                EXIT_FAIL_MAP,
                if e.detail().is_empty() {
                    None
                } else {
                    Some(e.detail())
                },
            ),
        }
        install_handoff(out, &info, &s, rsdp_pa, dtb_pa);
    }

    // (6) Làm mới MapKey rồi ExitBootServices; lặp khi key cũ.
    //
    // **Quan trọng (đo được trên OVMF/AAVMF)**: `ConOut->OutputString` có thể **cấp phát pool**
    // bên trong firmware ⇒ `MapKey` vừa lấy bị vô hiệu. Vì vậy sau `GetMemoryMap` cuối cùng
    // **không được in gì** trước `ExitBootServices`. Mọi dòng log nằm **trước** lần
    // `GetMemoryMap` của vòng.
    let mut tries = 0u8;
    loop {
        ulog!(
            out,
            "# uefi: exitbootservices=attempt try={} previous_key=0x{:x}",
            tries,
            info.map_key
        );
        // Từ đây tới `ExitBootServices`: KHÔNG in, KHÔNG cấp phát.
        let fresh = match get_memory_map(bs, MAP_CAP) {
            Ok(i) => i,
            Err(e) => fail(out, e.name(), e.exit_val(), None),
        };
        if fresh.map_size != info.map_size
            || fresh.desc_size != info.desc_size
            || fresh.desc_version != info.desc_version
        {
            let ptr = core::ptr::addr_of_mut!(MAP_BUF) as *const u8;
            if !NAIVE {
                if let Err(e) = unsafe { memmap::validate(ptr, &fresh) } {
                    fail(
                        out,
                        e.name(),
                        e.exit_val(),
                        Some("memmap-changed-before-exit"),
                    );
                }
            }
        }
        // SAFETY: `bs` hợp lệ tới khi ExitBootServices thành công; `image` do firmware truyền.
        let status = unsafe { ((*bs).exit_boot_services)(image, fresh.map_key) };
        if status == EFI_SUCCESS {
            break;
        }
        info = fresh;
        if status == EFI_INVALID_PARAMETER {
            tries += 1;
            if tries < EBS_TRIES {
                continue;
            }
        }
        fail(out, UefiError::ExitBootServices.name(), EXIT_FAIL_EBS, None);
    }

    // (7) Đã thoát dịch vụ boot: ConOut chết, chỉ còn semihosting.
    // SAFETY: semihosting; `exit_qemu` không trả về.
    unsafe {
        semihost::write0("# uefi: exitbootservices=ok");
        semihost::write0("# uefi: after-ebs=ok");
        semihost::exit_qemu(EXIT_OK)
    }
}

/// Entry của ảnh UEFI (`efi_main`): firmware gọi với `ImageHandle` + `SystemTable`.
#[no_mangle]
pub extern "efiapi" fn efi_main(image: usize, st: *mut SystemTable) -> usize {
    main_impl(image, st)
}
