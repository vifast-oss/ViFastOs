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
    pub get_memory_map:
        extern "efiapi" fn(*mut usize, *mut MemoryDescriptor, *mut usize, *mut usize, *mut u32) -> EfiStatus,
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
