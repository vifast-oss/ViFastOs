// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-414** — ảnh UEFI x86_64 (`PE/COFF`, entry `efi_main`): chụp `GetMemoryMap` **ngay trước**
//! `ExitBootServices`, kiểm **fail-closed có tên**, dựng **`BootInfo` v1 nguồn `Uefi`** (D2-260)
//! + bản ghi bàn giao `UefiHandoffV1`, in bằng chứng `# uefi:`/`# bootinfo:` ra `ConOut`.
//!
//! Sau đó thoát dịch vụ boot và báo QEMU thoát bằng `isa-debug-exit`.
//!
//! Thứ tự cố định (không đổi để log so sánh được giữa các lần chạy):
//!
//! 1. `# uefi: app=…` + firmware + chế độ build (naive/fault);
//! 2. `GetMemoryMap` (hỏi cỡ → đọc vào đệm 64 KiB tĩnh);
//! 3. tiêm lỗi nếu `VIFAST_UEFI_FAULT` (chỉ harness) → **kiểm** (`memmap::validate`);
//! 4. `# uefi: memmap=ok …` + `# uefi: cfgtable …` (RSDP/DTB từ `EFI_CONFIGURATION_TABLE`);
//! 5. dựng `BootInfo` v1 `source=uefi` + `validate()` **fail-closed theo nguồn** rồi in
//!    `# bootinfo:` **nguyên văn** bằng `BootInfo::write_log` của kernel;
//! 6. dựng + `verify` bản ghi bàn giao (`rsdp_pa`/`dtb_pa`), in địa chỉ/kích thước/checksum +
//!    **bất biến trang bàn giao** (reserved/copy khi nhảy `e_entry`);
//! 7. `GetMemoryMap` làm mới `MapKey` → `ExitBootServices` (thử lại khi `EFI_INVALID_PARAMETER`);
//! 8. sau khi thoát: `# uefi: after-ebs=ok` qua **cổng debug QEMU** (ConOut đã chết) → thoát
//!    QEMU mã `0x10` (rc 33).
//!
//! Đường lỗi: in `# uefi: fail-closed err=<mã> code=<mã thoát>` rồi thoát QEMU mã `0x20/0x21`
//! (rc 65/67) — **không** gọi `ExitBootServices` khi bảng nhớ hỏng **hoặc** `BootInfo` không
//! dựng được (ví dụ bảng nhớ hợp lệ nhưng không còn vùng RAM usable nào).

#![no_std]
#![no_main]

use vifast_boot_uefi::bootinfo::BootInfo;
use vifast_boot_uefi::console::{
    debugcon_str, line, qemu_exit, ConOutWriter, EXIT_FAIL_MAP, EXIT_OK,
};
use vifast_boot_uefi::efi::{
    find_config_table, BootServices, MemoryDescriptor, SimpleTextOutput, SystemTable,
    ACPI_10_TABLE_GUID, ACPI_20_TABLE_GUID, DTB_TABLE_GUID, EFI_BUFFER_TOO_SMALL,
    EFI_INVALID_PARAMETER, EFI_SUCCESS,
};
use vifast_boot_uefi::handoff::{UefiHandoffV1, HANDOFF_EMPTY, HANDOFF_RESERVED_INVARIANT};
use vifast_boot_uefi::memmap::{self, MapInfo, MemMapStats, UefiError};

/// Tên lỗi tiêm lúc build (`""` = ảnh sản phẩm). Xem `build.rs`.
const FAULT: &str = env!("VIFAST_UEFI_FAULT");
/// Tái hiện hành vi **trước khi vá** (bỏ kiểm bảng nhớ) — chỉ dùng để chứng minh ca âm đỏ.
const NAIVE: bool = cfg!(uefi_naive);

/// Đệm bảng nhớ (tĩnh, không cấp phát): OVMF ~100 mô tả × 48 B ≈ 5 KiB; 64 KiB là dư.
const MAP_CAP: usize = 64 * 1024;
/// Số lần thử tối đa khi `ExitBootServices` trả `EFI_INVALID_PARAMETER`.
const EBS_TRIES: u8 = 8;

static mut MAP_BUF: [u8; MAP_CAP] = [0; MAP_CAP];
static mut HANDOFF: UefiHandoffV1 = HANDOFF_EMPTY;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // SAFETY: cổng debug QEMU; panic handler không được trả về.
    unsafe {
        debugcon_str("# uefi: panic\n");
    }
    // SAFETY: thoát QEMU; không trả về.
    unsafe { qemu_exit(EXIT_FAIL_MAP) }
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
/// ACPI 2.0, fallback ACPI 1.0 (firmware cũ); DTB chỉ có trên `aarch64` (KT-414b) nên `x86_64`
/// thường 0 — **không** phải lỗi.
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

/// Dựng, `verify` và đặt bản ghi bàn giao vào `static`; in địa chỉ/kích thước/checksum +
/// **bất biến trang bàn giao** (reserved/copy khi nhảy `e_entry`).
fn install_handoff(
    out: *mut SimpleTextOutput,
    info: &MapInfo,
    st: &MemMapStats,
    rsdp_pa: u64,
    dtb_pa: u64,
) {
    let mut rec = UefiHandoffV1::build(st, info, rsdp_pa, dtb_pa);
    rec.seal();
    match rec.verify() {
        Ok(()) => {}
        Err(e) => fail(out, UefiError::MapBadRegion, Some(e)),
    }
    // SAFETY: ghi vào `static` của ảnh; không ai đọc đồng thời (single-thread boot).
    let ptr = core::ptr::addr_of_mut!(HANDOFF);
    // SAFETY: `ptr` hợp lệ, `rec` là bản ghi đã seal.
    unsafe { ptr.write(rec) };
    let pa = ptr as usize as u64;
    ulog!(
        out,
        "# uefi: handoff magic=0x{:016x} version={} va=0x{:x} size={} n_ram={} rsdp_pa=0x{:x} dtb_pa=0x{:x} checksum=0x{:08x}",
        rec.magic,
        rec.version,
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
fn fail(out: *mut SimpleTextOutput, e: UefiError, detail: Option<&'static str>) -> ! {
    let detail = detail.unwrap_or("-");
    ulog!(
        out,
        "# uefi: fail-closed err={} detail={} code=0x{:x}",
        e.name(),
        detail,
        e.exit_val()
    );
    ulog!(
        out,
        "# uefi: memmap=FAIL err={} exitbootservices=not-called",
        e.name()
    );
    ulog!(
        out,
        "# uefi: exit=0x{:x} (QEMU isa-debug-exit ⇒ rc {})",
        e.exit_val(),
        e.exit_val() * 2 + 1
    );
    // SAFETY: cổng debug QEMU vẫn sống; sau đó thoát QEMU (không trả về).
    unsafe {
        debugcon_str("# uefi: fail-closed\n");
        qemu_exit(e.exit_val())
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
        "# uefi: app=vifast-boot-x86_64 v=1 target=x86_64-unknown-uefi"
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
        Err(e) => fail(out, e, None),
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
        // minh ca âm là **đỏ** trên bản chưa vá (xem run.sh --selftest).
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
            Err(e) => fail(out, e, None),
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

    // (5) `BootInfo` v1 nguồn `Uefi` (D2-260) + bản ghi bàn giao — chỉ khi bảng nhớ đã kiểm sạch.
    if let Some(s) = stats {
        match vifast_boot_uefi::bootinfo_from_memmap(&s) {
            Ok(bi) => print_bootinfo(out, &bi),
            // Fail-closed theo nguồn: bảng nhớ "hợp lệ theo UEFI" nhưng `BootInfo` từ chối
            // (ví dụ không còn vùng RAM usable nào) ⇒ **không** ExitBootServices.
            Err(e) => fail(
                out,
                UefiError::BootInfo(e.name()),
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
    // **Quan trọng (đo được trên OVMF 2026)**: `ConOut->OutputString` có thể **cấp phát pool**
    // bên trong firmware ⇒ `MapKey` vừa lấy bị vô hiệu. Vì vậy sau `GetMemoryMap` cuối cùng
    // **không được in gì** trước `ExitBootServices` (lần đầu làm task này đã hỏng đúng như vậy:
    // key tăng 0x10 mỗi vòng và EBS trả `EFI_INVALID_PARAMETER` 8/8). Mọi dòng log nằm **trước**
    // lần `GetMemoryMap` của vòng.
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
            Err(e) => fail(out, e, None),
        };
        if fresh.map_size != info.map_size
            || fresh.desc_size != info.desc_size
            || fresh.desc_version != info.desc_version
        {
            let ptr = core::ptr::addr_of_mut!(MAP_BUF) as *const u8;
            if !NAIVE {
                if let Err(e) = unsafe { memmap::validate(ptr, &fresh) } {
                    fail(out, e, Some("memmap-changed-before-exit"));
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
        fail(out, UefiError::ExitBootServices, None);
    }

    // (7) Đã thoát dịch vụ boot: ConOut chết, chỉ còn cổng debug QEMU.
    // SAFETY: cổng debug QEMU; qemu_exit không trả về.
    unsafe {
        debugcon_str("# uefi: exitbootservices=ok\n");
        debugcon_str("# uefi: after-ebs=ok\n");
        qemu_exit(EXIT_OK)
    }
}

/// Entry của ảnh UEFI (`efi_main`): firmware gọi với `ImageHandle` + `SystemTable`.
#[no_mangle]
pub extern "efiapi" fn efi_main(image: usize, st: *mut SystemTable) -> usize {
    main_impl(image, st)
}
