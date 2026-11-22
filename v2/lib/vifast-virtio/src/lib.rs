// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-virtio` — **logic thuần** của thiết bị virtio: `virtio-blk` (KT-72) và `virtio-console`
//! (KT-73), không MMIO, không `unsafe`.
//!
//! # Vì sao tách crate này
//!
//! `kernel/vifast-plat-riscv64` chứa `unsafe` (MMIO/asm) nên **không** chạy host test. Phần
//! *quyết định* của driver — đọc BAR từ mask, đi capability list, dựng descriptor/avail/used
//! ring theo đúng byte layout của spec, giải mã `virtio_blk_config`, ánh xạ status — là logic
//! **thuần** trên byte, tách ở đây để test host chạy được (D2-05: đỏ trước rồi mới xanh) và để
//! kernel chỉ còn phần chạm thanh ghi.
//!
//! # Nguyên tắc
//!
//! - `#![no_std]`, `#![forbid(unsafe_code)]`, không dependency ngoài (D2-21).
//! - Mọi truy cập PCI đi qua trait [`pci::PciCfg`] (kernel cài bằng ECAM, host test cài bằng
//!   mảng 256 byte mô phỏng đúng quy tắc BAR của QEMU).
//! - Ring thao tác trên một **lát byte có sẵn** (driver cấp trang 4 KiB căn lề); crate này không
//!   biết địa chỉ vật lý, không tự cấp phát.
//! - Mã lỗi in qua [`blk::BlkError::name`]/[`console::ConsoleError::name`] (D2-102: không in số
//!   trần trong evidence).
//! - **KT-73**: `console` (device id/feature/chỉ số queue — D2-110B) và `conchan` (codec byte trên
//!   slot 4 word của `vifast-ring`, **không** layout mới). Hai thiết bị dùng **chung** `pci::`,
//!   `queue::`, `features::` — thêm console không nhân đôi đường PCI/BAR.
#![no_std]
#![forbid(unsafe_code)]

pub mod blk;
/// **KT-334** driver `virtio-blk` dùng chung 3 arch — trên [`mmio::MmioBus`] + [`dma::DmaPool`]
/// + [`blkdev::Clock`]; platform chỉ còn adapter MMIO/DMA/cache (D2-231, D2-239(1)).
pub mod blkdev;
pub mod conchan;
pub mod console;
/// **KT-334** vùng DMA vật lý liên tục + hai hook cache `sync_for_device`/`sync_for_cpu`
/// (giữ nguyên `dc civac`/`dsb` của aarch64, KT-277).
pub mod dma;
pub mod features;
/// **KT-263** `virtio_gpu` — device id/feature/`virtio_gpu_ctrl_hdr` + sáu lệnh 2D
/// (§5.7), **font bitmap 8×8** ASCII 32..126 và hàm vẽ `fill_rect`/`draw_text`.
pub mod gpu;
/// **KT-271** `gpu_console` — console chữ trên framebuffer: con trỏ `(col, row)`, `putc`/`puts`/
/// `newline`, **cuộn** (memmove vùng framebuffer) và **vùng bẩn** để flush theo lô.
pub mod gpu_console;
/// **KT-337** driver `virtio-gpu` 2D dùng chung 3 arch — trên [`mmio::MmioBus`] + [`dma::DmaPool`]
/// + [`blkdev::Clock`] (D2-231/D2-239); platform chỉ còn adapter MMIO/DMA/cache.
pub mod gpudev;
/// **KT-255** `virtio_input` — device id/feature/`virtio_input_config`/`virtio_input_event` +
/// **bảng chuyển mã phím → ASCII** (§5.8).
pub mod input;
/// **KT-337** driver `virtio-input` dùng chung 3 arch — trên cùng transport; platform aarch64 bật
/// thêm queue `status` bằng cờ `with_status_queue` của `Input::init`.
pub mod inputdev;
/// **KT-334** cổng MMIO 1/2/4/8 byte + barrier + cửa thông báo cho transport VirtIO-PCI.
pub mod mmio;
/// **KT-245** `virtio_net` — device id/feature/`virtio_net_config`/`virtio_net_hdr` (§5.1).
pub mod net;
pub mod pci;
/// **KT-334** transport PCI dùng chung (quét, đo/gán BAR, capability) trên trait `PciBus`.
pub mod pcibus;
pub mod queue;
pub mod strict;
