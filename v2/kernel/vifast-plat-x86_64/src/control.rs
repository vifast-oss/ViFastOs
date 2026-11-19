// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hằng **thanh ghi điều khiển + MSR** của x86_64 — KT-186 / D2-163 (K5' bước M2).
//!
//! Tệp này **thuần dữ liệu**: vị trí bit của `CR0`, `CR4`, `IA32_EFER` và địa chỉ các MSR mà
//! bước M4/M7 sẽ nạp. **Không** asm, **không** `unsafe` — bước M2 chỉ chốt **giá trị tham chiếu**
//! bằng test host; việc `mov cr0/cr4`, `wrmsr`, `swapgs` là bước M4/M7 (crate arch, D2-17).
//!
//! # Nguồn
//!
//! - **Intel SDM Vol 3A**, mục "Control Registers" — `CR0` (bit `PE/MP/EM/TS/ET/NE/WP/AM/NW/CD/PG`)
//!   và `CR4` (bit `VME/PVI/TSD/DE/PSE/PAE/MCE/PGE/PCE/OSFXSR/OSXMMEXCPT/UMIP/LA57/VMXE/SMXE/
//!   FSGSBASE/PCIDE/OSXSAVE/SMEP/SMAP/PKE/CET/PKS`).
//! - **Intel SDM Vol 3A**, mục "Extended Feature Enable Register (IA32_EFER)" — `SCE` bit 0,
//!   `LME` bit 8, `LMA` bit 10, `NXE` bit 11, `SVME` bit 12, `LMSLE` bit 13, `FFXSR` bit 14,
//!   `TCE` bit 15.
//! - **Intel SDM Vol 4**, ch. 2 "Model-Specific Registers (MSRs)" — địa chỉ `IA32_EFER`,
//!   `IA32_STAR`, `IA32_LSTAR`, `IA32_FMASK`, `IA32_FS_BASE`, `IA32_GS_BASE`,
//!   `IA32_KERNEL_GS_BASE`, `IA32_PAT`, `IA32_APIC_BASE`.
//! - **Intel SDM Vol 3A**, mục "Initializing IA-32e Mode" (§9.8.5) — thứ tự bật long mode:
//!   `CR4.PAE = 1`, nạp `CR3`, `IA32_EFER.LME = 1`, `CR0.PG = 1` (khi đó `LMA` tự bật).
//!
//! # Vì sao chỉ là hằng số
//!
//! Đây là **hợp đồng dữ liệu** giữa bước M2 (host) và bước M4/M7 (asm): nếu ai đặt sai một bit,
//! test `control_bits_literal_intel_sdm` đỏ **trước** khi có ảnh boot — đúng bài học "xanh giả"
//! của K5' (audit §5 R6).

/// `CR0.PE` — protection enable (bit 0).
pub const CR0_PE: u64 = 1 << 0;
/// `CR0.MP` — monitor coprocessor (bit 1).
pub const CR0_MP: u64 = 1 << 1;
/// `CR0.EM` — emulation (bit 2).
pub const CR0_EM: u64 = 1 << 2;
/// `CR0.TS` — task switched (bit 3).
pub const CR0_TS: u64 = 1 << 3;
/// `CR0.ET` — extension type (bit 4).
pub const CR0_ET: u64 = 1 << 4;
/// `CR0.NE` — numeric error (bit 5).
pub const CR0_NE: u64 = 1 << 5;
/// `CR0.WP` — write protect (bit 16).
pub const CR0_WP: u64 = 1 << 16;
/// `CR0.AM` — alignment mask (bit 18).
pub const CR0_AM: u64 = 1 << 18;
/// `CR0.NW` — not write-through (bit 29).
pub const CR0_NW: u64 = 1 << 29;
/// `CR0.CD` — cache disable (bit 30).
pub const CR0_CD: u64 = 1 << 30;
/// `CR0.PG` — paging enable (bit 31).
pub const CR0_PG: u64 = 1 << 31;

/// `CR4.VME` — virtual-8086 mode extensions (bit 0).
pub const CR4_VME: u64 = 1 << 0;
/// `CR4.PVI` — protected-mode virtual interrupts (bit 1).
pub const CR4_PVI: u64 = 1 << 1;
/// `CR4.TSD` — time stamp disable (bit 2).
pub const CR4_TSD: u64 = 1 << 2;
/// `CR4.DE` — debugging extensions (bit 3).
pub const CR4_DE: u64 = 1 << 3;
/// `CR4.PSE` — page size extensions (bit 4).
pub const CR4_PSE: u64 = 1 << 4;
/// `CR4.PAE` — physical address extension (bit 5) — **bắt buộc** cho long mode.
pub const CR4_PAE: u64 = 1 << 5;
/// `CR4.MCE` — machine-check enable (bit 6).
pub const CR4_MCE: u64 = 1 << 6;
/// `CR4.PGE` — page global enable (bit 7).
pub const CR4_PGE: u64 = 1 << 7;
/// `CR4.PCE` — performance-monitoring counter enable (bit 8).
pub const CR4_PCE: u64 = 1 << 8;
/// `CR4.OSFXSR` — OS supports FXSAVE/FXRSTOR (bit 9).
pub const CR4_OSFXSR: u64 = 1 << 9;
/// `CR4.OSXMMEXCPT` — OS supports unmasked SIMD FP exceptions (bit 10).
pub const CR4_OSXMMEXCPT: u64 = 1 << 10;
/// `CR4.UMIP` — user-mode instruction prevention (bit 11).
pub const CR4_UMIP: u64 = 1 << 11;
/// `CR4.LA57` — 57-bit linear addresses (bit 12).
pub const CR4_LA57: u64 = 1 << 12;
/// `CR4.VMXE` — VMX enable (bit 13).
pub const CR4_VMXE: u64 = 1 << 13;
/// `CR4.SMXE` — SMX enable (bit 14).
pub const CR4_SMXE: u64 = 1 << 14;
/// `CR4.FSGSBASE` — FSGSBASE instructions enable (bit 16).
pub const CR4_FSGSBASE: u64 = 1 << 16;
/// `CR4.PCIDE` — process-context identifier enable (bit 17).
pub const CR4_PCIDE: u64 = 1 << 17;
/// `CR4.OSXSAVE` — OS supports XSAVE (bit 18).
pub const CR4_OSXSAVE: u64 = 1 << 18;
/// `CR4.SMEP` — supervisor-mode execution prevention (bit 20).
pub const CR4_SMEP: u64 = 1 << 20;
/// `CR4.SMAP` — supervisor-mode access prevention (bit 21).
pub const CR4_SMAP: u64 = 1 << 21;
/// `CR4.PKE` — protection keys enable (bit 22).
pub const CR4_PKE: u64 = 1 << 22;
/// `CR4.CET` — control-flow enforcement technology (bit 23).
pub const CR4_CET: u64 = 1 << 23;
/// `CR4.PKS` — protection keys for supervisor (bit 24).
pub const CR4_PKS: u64 = 1 << 24;

/// `IA32_EFER.SCE` — SYSCALL/SYSRET enable (bit 0).
pub const EFER_SCE: u64 = 1 << 0;
/// `IA32_EFER.LME` — IA-32e mode enable (bit 8).
pub const EFER_LME: u64 = 1 << 8;
/// `IA32_EFER.LMA` — IA-32e mode active (bit 10, chỉ đọc).
pub const EFER_LMA: u64 = 1 << 10;
/// `IA32_EFER.NXE` — execute-disable bit enable (bit 11).
pub const EFER_NXE: u64 = 1 << 11;
/// `IA32_EFER.SVME` — SVM enable (bit 12).
pub const EFER_SVME: u64 = 1 << 12;
/// `IA32_EFER.LMSLE` — long-mode segment limit enable (bit 13).
pub const EFER_LMSLE: u64 = 1 << 13;
/// `IA32_EFER.FFXSR` — fast FXSAVE/FXRSTOR (bit 14).
pub const EFER_FFXSR: u64 = 1 << 14;
/// `IA32_EFER.TCE` — translation cache extension (bit 15).
pub const EFER_TCE: u64 = 1 << 15;

/// MSR `IA32_EFER` (0xC000_0080).
pub const MSR_IA32_EFER: u32 = 0xC000_0080;
/// MSR `IA32_STAR` (0xC000_0081) — phân đoạn CS/SS cho `syscall`/`sysret`.
pub const MSR_IA32_STAR: u32 = 0xC000_0081;
/// MSR `IA32_LSTAR` (0xC000_0082) — RIP vào kernel của `syscall`.
pub const MSR_IA32_LSTAR: u32 = 0xC000_0082;
/// MSR `IA32_FMASK` (0xC000_0084) — RFLAGS bị xoá khi `syscall`.
pub const MSR_IA32_FMASK: u32 = 0xC000_0084;
/// MSR `IA32_FS_BASE` (0xC000_0100).
pub const MSR_IA32_FS_BASE: u32 = 0xC000_0100;
/// MSR `IA32_GS_BASE` (0xC000_0101).
pub const MSR_IA32_GS_BASE: u32 = 0xC000_0101;
/// MSR `IA32_KERNEL_GS_BASE` (0xC000_0102) — đích của `swapgs`.
pub const MSR_IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;
/// MSR `IA32_PAT` (0x0000_0277) — bảng kiểu bộ nhớ.
pub const MSR_IA32_PAT: u32 = 0x0000_0277;
/// MSR `IA32_APIC_BASE` (0x0000_001B).
pub const MSR_IA32_APIC_BASE: u32 = 0x0000_001B;

/// `CR3` bit `PCD` (bit 3).
pub const CR3_PCD: u64 = 1 << 3;
/// `CR3` bit `PWT` (bit 4).
pub const CR3_PWT: u64 = 1 << 4;
/// `CR3` — PCID 12 bit ở bit 11:0 (khi `CR4.PCIDE = 1`).
pub const CR3_PCID_MASK: u64 = 0xFFF;
/// `CR3` — địa chỉ bảng gốc PML4 ở bit 51:12.
pub const CR3_ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

/// Tổ hợp bit tối thiểu để vào long mode (Intel SDM Vol 3A §9.8.5): `CR0.PE | CR0.PG`,
/// `CR4.PAE`, `IA32_EFER.LME`. `CR0.PG` bật **sau** khi `LME = 1`; khi đó `LMA` tự bật.
pub const LONG_MODE_CR0: u64 = CR0_PE | CR0_PG;
/// `CR4` tối thiểu cho long mode.
pub const LONG_MODE_CR4: u64 = CR4_PAE;
/// `IA32_EFER` tối thiểu cho long mode (chưa gồm `NXE`/`SCE`).
pub const LONG_MODE_EFER: u64 = EFER_LME;
