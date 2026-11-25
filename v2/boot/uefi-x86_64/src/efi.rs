// SPDX-License-Identifier: MIT OR Apache-2.0
//! Định nghĩa `repr(C)` **tối thiểu** của UEFI 2.x cho boot stub: chỉ các bảng/trường mà tệp
//! `main.rs` thật sự chạm (`EFI_SYSTEM_TABLE`, `EFI_BOOT_SERVICES` tới `ExitBootServices`,
//! `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL`, `EFI_MEMORY_DESCRIPTOR`, `EFI_CONFIGURATION_TABLE`).
//!
//! Bố cục lấy **nguyên văn** từ UEFI Specification 2.10 §4.2/§7.2/§12.8. Thứ tự trường là ABI:
//! đổi thứ tự/thêm trường vào giữa là sai offset ⇒ gọi hàm rác. Vì vậy mỗi struct đều có test
//! host khoá `size_of`/`offset_of` (`tests/canonical.rs`).

use core::ffi::c_void;

/// `EFI_STATUS` (UINTN). Bit cao = lỗi.
pub type EfiStatus = usize;

/// `EFI_SUCCESS`.
pub const EFI_SUCCESS: EfiStatus = 0;
/// `EFI_BUFFER_TOO_SMALL` = `EFIERR(5)`.
pub const EFI_BUFFER_TOO_SMALL: EfiStatus = 0x8000_0000_0000_0005;
/// `EFI_INVALID_PARAMETER` = `EFIERR(2)` — `ExitBootServices` trả mã này khi `MapKey` cũ.
pub const EFI_INVALID_PARAMETER: EfiStatus = 0x8000_0000_0000_0002;

/// `EFI_TABLE_HEADER` (24 byte).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TableHeader {
    /// Chữ ký (`'T','S','T','B'` với system table).
    pub signature: u64,
    /// Phiên bản.
    pub revision: u32,
    /// `sizeof` của bảng.
    pub header_size: u32,
    /// CRC32.
    pub crc32: u32,
    /// Dành riêng.
    pub reserved: u32,
}

/// `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL` — chỉ cần `OutputString` (offset 8).
#[repr(C)]
pub struct SimpleTextOutput {
    /// Reset (không dùng).
    pub reset: usize,
    /// In chuỗi UTF-16 kết thúc NUL; **không** tự xuống dòng.
    pub output_string: extern "efiapi" fn(*mut SimpleTextOutput, *const u16) -> EfiStatus,
    /// TestString (không dùng).
    pub test_string: usize,
    /// QueryMode.
    pub query_mode: usize,
    /// SetMode.
    pub set_mode: usize,
    /// SetAttribute.
    pub set_attribute: usize,
    /// ClearScreen.
    pub clear_screen: usize,
    /// SetCursorPosition.
    pub set_cursor_position: usize,
    /// EnableCursor.
    pub enable_cursor: usize,
    /// `SIMPLE_TEXT_OUTPUT_MODE`.
    pub mode: *mut c_void,
}

/// `EFI_MEMORY_DESCRIPTOR` — 40 byte với `sizeof(UINTN) == 8`.
///
/// `GetMemoryMap` trả **mảng** descriptor với stride `DescriptorSize` (có thể **lớn hơn** 40 —
/// firmware được phép thêm trường); vì vậy bên đọc **luôn** dùng stride, không dùng
/// `size_of::<Self>()` để bước.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryDescriptor {
    /// `EFI_MEMORY_TYPE`.
    pub ty: u32,
    /// Padding ABI (luôn 0).
    pub pad: u32,
    /// Địa chỉ vật lý bắt đầu (phải căn trang).
    pub physical_start: u64,
    /// Địa chỉ ảo (trước `ExitBootServices` bằng vật lý).
    pub virtual_start: u64,
    /// Số trang 4 KiB.
    pub number_of_pages: u64,
    /// Thuộc tính `EFI_MEMORY_*`.
    pub attribute: u64,
}

/// `EFI_CONFIGURATION_TABLE` (24 byte).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ConfigurationTable {
    /// GUID của bảng.
    pub vendor_guid: [u8; 16],
    /// Con trỏ bảng.
    pub vendor_table: *mut c_void,
}

/// GUID `ACPI 2.0` (`8868e871-e4f1-11d3-bc22-0080c73c8881`) — `vendor_table` là `RSDP` (ACPI 6.x).
///
/// Byte theo **layout EFI GUID** (Data1/Data2/Data3 little-endian, Data4 nguyên thứ tự), không
/// phải chuỗi hex của spec — sai layout là không tìm thấy bảng trên firmware thật.
///
/// **KT-414b sửa lỗi byte-order**: bản KT-414 khai `e8 71 68 88 …` (Data1 `0x8868_71e8`) trong
/// khi `Data1 = 0x8868_e871` ⇒ LE là `71 e8 68 88`. Trên OVMF lỗi này **bị che** vì firmware
/// cũng cấp bảng ACPI 1.0 (fallback đúng) nên `rsdp_pa` vẫn khác 0; trên **AAVMF** chỉ có bảng
/// ACPI 2.0 ⇒ `rsdp_pa=0` (đo được 2026-11-25). Test host `config_table_guids_match_spec_bytes` khoá
/// từng byte để không tái diễn.
pub const ACPI_20_TABLE_GUID: [u8; 16] = [
    0x71, 0xe8, 0x68, 0x88, 0xf1, 0xe4, 0xd3, 0x11, 0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81,
];

/// GUID `ACPI 1.0` (`eb9d2d30-2d88-11d3-9a16-0090273fc14d`) — fallback khi firmware chỉ khai bản 1.0.
pub const ACPI_10_TABLE_GUID: [u8; 16] = [
    0x30, 0x2d, 0x9d, 0xeb, 0x88, 0x2d, 0xd3, 0x11, 0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d,
];

/// GUID DTB (`b1b621d5-f19c-41a5-830b-d9152c69aae0`) — `vendor_table` là FDT (`aarch64`, KT-414b).
pub const DTB_TABLE_GUID: [u8; 16] = [
    0xd5, 0x21, 0xb6, 0xb1, 0x9c, 0xf1, 0xa5, 0x41, 0x83, 0x0b, 0xd9, 0x15, 0x2c, 0x69, 0xaa, 0xe0,
];

/// Tìm bảng cấu hình theo GUID trong mảng `tables`; trả **địa chỉ vật lý** bảng (0 nếu không có).
///
/// Trước `ExitBootServices` mọi vùng của firmware được identity-map ⇒ VA = PA; sau đó kernel
/// dùng PA này để đọc `RSDP`/DTB. Hàm **thuần** (không `unsafe`) nên test host khoá được.
pub fn find_config_table(tables: &[ConfigurationTable], guid: &[u8; 16]) -> u64 {
    for t in tables {
        if &t.vendor_guid == guid {
            return t.vendor_table as usize as u64;
        }
    }
    0
}

/// `EFI_BOOT_SERVICES` — cắt ngay sau `ExitBootServices`.
///
/// Các trường **giữa** `GetMemoryMap` và `ExitBootServices` phải đủ chỗ: thiếu một trường là
/// `ExitBootServices` trỏ sai hàm. Đếm theo spec: 27 con trỏ sau header ⇒
/// `size_of == 24 + 27*8 = 240`, `ExitBootServices` ở offset **232**.
#[repr(C)]
pub struct BootServices {
    /// Header bảng.
    pub hdr: TableHeader,
    /// RaiseTPL.
    pub raise_tpl: usize,
    /// RestoreTPL.
    pub restore_tpl: usize,
    /// AllocatePages.
    pub allocate_pages: usize,
    /// FreePages.
    pub free_pages: usize,
    /// `GetMemoryMap(MapSize, Map, MapKey, DescriptorSize, DescriptorVersion)`.
    pub get_memory_map: extern "efiapi" fn(
        *mut usize,
        *mut MemoryDescriptor,
        *mut usize,
        *mut usize,
        *mut u32,
    ) -> EfiStatus,
    /// AllocatePool.
    pub allocate_pool: usize,
    /// FreePool.
    pub free_pool: usize,
    /// CreateEvent.
    pub create_event: usize,
    /// SetTimer.
    pub set_timer: usize,
    /// WaitForEvent.
    pub wait_for_event: usize,
    /// SignalEvent.
    pub signal_event: usize,
    /// CloseEvent.
    pub close_event: usize,
    /// CheckEvent.
    pub check_event: usize,
    /// InstallProtocolInterface.
    pub install_protocol_interface: usize,
    /// ReinstallProtocolInterface.
    pub reinstall_protocol_interface: usize,
    /// UninstallProtocolInterface.
    pub uninstall_protocol_interface: usize,
    /// HandleProtocol.
    pub handle_protocol: usize,
    /// Reserved (ABI).
    pub reserved: usize,
    /// RegisterProtocolNotify.
    pub register_protocol_notify: usize,
    /// LocateHandle.
    pub locate_handle: usize,
    /// LocateDevicePath.
    pub locate_device_path: usize,
    /// InstallConfigurationTable.
    pub install_configuration_table: usize,
    /// LoadImage.
    pub load_image: usize,
    /// StartImage.
    pub start_image: usize,
    /// Exit.
    pub exit: usize,
    /// UnloadImage.
    pub unload_image: usize,
    /// `ExitBootServices(ImageHandle, MapKey)`.
    pub exit_boot_services: extern "efiapi" fn(usize, usize) -> EfiStatus,
}

/// `EFI_SYSTEM_TABLE` (112 byte + bảng cấu hình).
#[repr(C)]
pub struct SystemTable {
    /// Header.
    pub hdr: TableHeader,
    /// Nhà sản xuất firmware (UTF-16).
    pub firmware_vendor: *const u16,
    /// Phiên bản firmware.
    pub firmware_revision: u32,
    /// `ConsoleInHandle`.
    pub console_in_handle: usize,
    /// `ConIn`.
    pub con_in: *mut c_void,
    /// `ConsoleOutHandle`.
    pub console_out_handle: usize,
    /// `ConOut` — nơi in bằng chứng `# uefi:`.
    pub con_out: *mut SimpleTextOutput,
    /// `StandardErrorHandle`.
    pub std_error_handle: usize,
    /// `StdErr`.
    pub std_err: *mut SimpleTextOutput,
    /// `RuntimeServices` (KHÔNG dùng sau `ExitBootServices`).
    pub runtime_services: *mut c_void,
    /// `BootServices`.
    pub boot_services: *mut BootServices,
    /// Số bảng cấu hình.
    pub number_of_table_entries: usize,
    /// Mảng bảng cấu hình (RSDP/ACPI/DTB).
    pub configuration_table: *mut ConfigurationTable,
}
