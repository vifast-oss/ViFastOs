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
pub mod conchan;
pub mod console;
pub mod features;
/// **KT-245** `virtio_net` — device id/feature/`virtio_net_config`/`virtio_net_hdr` (§5.1).
pub mod net;
pub mod pci;
pub mod queue;
pub mod strict;
