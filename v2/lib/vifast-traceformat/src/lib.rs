// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-traceformat` — bản ghi trace **32 B** dùng chung sim/QEMU (KT-12, D2-38).
//!
#![no_std]
//!
//! **no_std** (D2-42): `FixedRing` chạy trong kernel; `decode_all`/`Ring` cần feature `alloc`/`std`.
//!
//! Layout little-endian, không padding: `t_ns:u64, hart:u16, kind:u16, tcb:u32, a:u64, b:u64`.
//! Ring là bộ đệm vòng cố định: ghi đè bản ghi cũ nhất khi đầy, đọc ra theo thứ tự thời gian.

pub mod kernel_ring;

pub const RECORD_BYTES: usize = 32;

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
pub const KIND_SYSCALL: u16 = 1;
pub const KIND_IPC: u16 = 2;
pub const KIND_SCHED: u16 = 3;
pub const KIND_FAULT: u16 = 4;
pub const KIND_STORE: u16 = 5;
pub const KIND_RUN: u16 = 6;
pub const KIND_IDLE: u16 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    pub t_ns: u64,
    pub hart: u16,
    pub kind: u16,
    pub tcb: u32,
    pub a: u64,
    pub b: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceError {
    Truncated(usize),
    UnknownKind(u16),
}

impl Record {
    pub fn new(t_ns: u64, hart: u16, kind: u16, tcb: u32, a: u64, b: u64) -> Self {
        Self {
            t_ns,
            hart,
            kind,
            tcb,
            a,
            b,
        }
    }

    pub fn to_bytes(self) -> [u8; RECORD_BYTES] {
        let mut out = [0u8; RECORD_BYTES];
        out[0..8].copy_from_slice(&self.t_ns.to_le_bytes());
        out[8..10].copy_from_slice(&self.hart.to_le_bytes());
        out[10..12].copy_from_slice(&self.kind.to_le_bytes());
        out[12..16].copy_from_slice(&self.tcb.to_le_bytes());
        out[16..24].copy_from_slice(&self.a.to_le_bytes());
        out[24..32].copy_from_slice(&self.b.to_le_bytes());
        out
    }

    pub fn from_bytes(buf: &[u8]) -> Result<Self, TraceError> {
        if buf.len() < RECORD_BYTES {
            return Err(TraceError::Truncated(buf.len()));
        }
        let r = Self {
            t_ns: u64::from_le_bytes(buf[0..8].try_into().expect("8")),
            hart: u16::from_le_bytes(buf[8..10].try_into().expect("2")),
            kind: u16::from_le_bytes(buf[10..12].try_into().expect("2")),
            tcb: u32::from_le_bytes(buf[12..16].try_into().expect("4")),
            a: u64::from_le_bytes(buf[16..24].try_into().expect("8")),
            b: u64::from_le_bytes(buf[24..32].try_into().expect("8")),
        };
        if !is_known_kind(r.kind) {
            return Err(TraceError::UnknownKind(r.kind));
        }
        Ok(r)
    }
}

pub fn is_known_kind(k: u16) -> bool {
    matches!(k, KIND_SYSCALL..=KIND_IDLE)
}

/// Giải mã một dump liền mạch; lỗi nếu độ dài không chia hết 32 hoặc kind lạ.
#[cfg(feature = "alloc")]
pub fn decode_all(buf: &[u8]) -> Result<Vec<Record>, TraceError> {
    if !buf.len().is_multiple_of(RECORD_BYTES) {
        return Err(TraceError::Truncated(buf.len() % RECORD_BYTES));
    }
    let mut out = Vec::with_capacity(buf.len() / RECORD_BYTES);
    let mut i = 0;
    while i < buf.len() {
        out.push(Record::from_bytes(&buf[i..i + RECORD_BYTES])?);
        i += RECORD_BYTES;
    }
    Ok(out)
}

/// Ring cố định: `push` ghi đè cũ nhất khi đầy; `iter` trả theo thứ tự thời gian.
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct Ring {
    buf: Vec<Record>,
    cap: usize,
    next: usize,
    len: usize,
}

#[cfg(feature = "std")]
impl Ring {
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "ring phải có sức chứa > 0");
        Self {
            buf: alloc::vec![Record::new(0, 0, 0, 0, 0, 0); cap],
            cap,
            next: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, r: Record) {
        self.buf[self.next] = r;
        self.next = (self.next + 1) % self.cap;
        if self.len < self.cap {
            self.len += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &Record> {
        let start = if self.len == self.cap { self.next } else { 0 };
        (0..self.len).map(move |i| &self.buf[(start + i) % self.cap])
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.iter().flat_map(|r| r.to_bytes()).collect()
    }
}

// ---------------------------------------------------------------------------------------------
// KT-55 — dòng trace **văn bản chuẩn hoá** dùng chung sim/QEMU
// ---------------------------------------------------------------------------------------------

/// Dòng `# trace: <scn> <step> <op> <k=v>… => <kết quả>` — **một** định dạng cho cả sim (host)
/// và kernel (ktest in qua UART), để `v2/test/conformance/trace_compare.py` so được từng trường.
///
/// Không cấp phát (D2-42): bộ đệm `[u8; N]` do người gọi chọn; kernel dùng 192 B trên stack.
/// Chỉ chứa **trường được phép** (do backend chọn khi gọi): không thời gian, không địa chỉ PA
/// tuyệt đối, không hart (tập trường so sánh nằm ở `v2/docs/k3-riscv64.md` §18 + script so).
pub struct TraceLine<const N: usize> {
    buf: [u8; N],
    len: usize,
    /// Set khi bộ đệm đầy — `as_str` trả phần đã ghi, `truncated` cho backend biết để đỏ.
    truncated: bool,
}

impl<const N: usize> Default for TraceLine<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> TraceLine<N> {
    /// Bộ đệm rỗng.
    pub const fn new() -> Self {
        Self {
            buf: [0u8; N],
            len: 0,
            truncated: false,
        }
    }

    /// `true` ⇔ có lần ghi bị cắt (dòng không hợp lệ — backend phải coi là lỗi).
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    /// Nội dung đã ghi (không gồm byte 0 cuối).
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    fn push_str(&mut self, s: &str) {
        for &byte in s.as_bytes() {
            if self.len < N {
                self.buf[self.len] = byte;
                self.len += 1;
            } else {
                self.truncated = true;
            }
        }
    }

    fn push_u64_dec(&mut self, mut v: u64) {
        let mut tmp = [0u8; 20];
        let mut n = 0usize;
        if v == 0 {
            self.push_str("0");
            return;
        }
        while v > 0 {
            tmp[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
        }
        while n > 0 {
            n -= 1;
            let byte = tmp[n];
            if self.len < N {
                self.buf[self.len] = byte;
                self.len += 1;
            } else {
                self.truncated = true;
            }
        }
    }

    fn push_u64_hex(&mut self, v: u64) {
        self.push_str("0x");
        let mut started = false;
        for shift in (0..16).rev() {
            let nib = ((v >> (shift * 4)) & 0xf) as u8;
            if nib != 0 || started || shift == 0 {
                started = true;
                let byte = if nib < 10 {
                    b'0' + nib
                } else {
                    b'a' + nib - 10
                };
                if self.len < N {
                    self.buf[self.len] = byte;
                    self.len += 1;
                } else {
                    self.truncated = true;
                }
            }
        }
    }

    /// Mở dòng: `# trace: <scn> <step> <op>`.
    pub fn start(&mut self, scn: u8, step: u16, op: &str) -> &mut Self {
        self.len = 0;
        self.truncated = false;
        self.push_str("# trace: ");
        self.push_u64_dec(scn as u64);
        self.push_str(" ");
        self.push_u64_dec(step as u64);
        self.push_str(" ");
        self.push_str(op);
        self
    }

    /// Thêm trường chuỗi: ` k=v` (tên/giá trị do backend chọn trong allowlist §18).
    pub fn field_str(&mut self, key: &str, value: &str) -> &mut Self {
        self.push_str(" ");
        self.push_str(key);
        self.push_str("=");
        self.push_str(value);
        self
    }

    /// Thêm trường số thập phân.
    pub fn field_u64(&mut self, key: &str, value: u64) -> &mut Self {
        self.push_str(" ");
        self.push_str(key);
        self.push_str("=");
        self.push_u64_dec(value);
        self
    }

    /// Thêm trường số hex `0x…` (dùng cho VA/bitmask — không phải địa chỉ PA tuyệt đối).
    pub fn field_hex(&mut self, key: &str, value: u64) -> &mut Self {
        self.push_str(" ");
        self.push_str(key);
        self.push_str("=");
        self.push_u64_hex(value);
        self
    }

    /// Kết thúc dòng: ` => <kết quả>\n`.
    pub fn result(&mut self, result: &str) -> &mut Self {
        self.push_str(" => ");
        self.push_str(result);
        self.push_str("\n");
        self
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    fn r(n: u64) -> Record {
        Record::new(n, 1, KIND_IPC, 7, n * 2, n * 3)
    }

    #[test]
    fn kich_thuoc_dung_32_va_roundtrip() {
        let rec = r(42);
        let b = rec.to_bytes();
        assert_eq!(b.len(), RECORD_BYTES);
        assert_eq!(Record::from_bytes(&b).unwrap(), rec);
    }

    #[test]
    fn thieu_byte_bi_bat() {
        let b = r(1).to_bytes();
        assert!(matches!(
            Record::from_bytes(&b[..31]),
            Err(TraceError::Truncated(31))
        ));
        let mut long = b.to_vec();
        long.push(0);
        assert!(matches!(decode_all(&long), Err(TraceError::Truncated(1))));
    }

    #[test]
    fn kind_la_bi_bat() {
        let mut b = r(1).to_bytes();
        b[10..12].copy_from_slice(&99u16.to_le_bytes());
        assert!(matches!(
            Record::from_bytes(&b),
            Err(TraceError::UnknownKind(99))
        ));
    }

    #[test]
    fn ring_day_ghi_de_cu_nhat_va_giu_thu_tu() {
        let mut ring = Ring::new(4);
        for n in 1..=6 {
            ring.push(r(n));
        }
        assert_eq!(ring.len(), 4);
        let ts: Vec<u64> = ring.iter().map(|x| x.t_ns).collect();
        assert_eq!(
            ts,
            alloc::vec![3, 4, 5, 6],
            "phải giữ 4 bản ghi mới nhất theo thứ tự"
        );
    }

    #[test]
    fn fixed_ring_khong_cap_phat_giu_thu_tu() {
        let mut ring: FixedRing<4> = FixedRing::new();
        assert!(ring.is_empty());
        for n in 1..=6 {
            ring.push(r(n));
        }
        assert_eq!(ring.len(), 4);
        assert_eq!(
            ring.iter().map(|x| x.t_ns).collect::<Vec<_>>(),
            alloc::vec![3, 4, 5, 6]
        );
        let mut one: FixedRing<1> = FixedRing::new();
        one.push(r(9));
        one.push(r(10));
        assert_eq!(
            one.iter().map(|x| x.t_ns).collect::<Vec<_>>(),
            alloc::vec![10]
        );
    }

    #[test]
    fn decode_all_tron_du_va_noi_lien_ring() {
        let mut ring = Ring::new(3);
        ring.push(r(1));
        ring.push(r(2));
        let bytes = ring.to_bytes();
        assert_eq!(bytes.len(), 2 * RECORD_BYTES);
        let back = decode_all(&bytes).unwrap();
        assert_eq!(back, alloc::vec![r(1), r(2)]);
    }
}

/// Ring **không cấp phát** (dùng được trong kernel `no_std`): `[Record; N]` trên stack.
/// Cùng ngữ nghĩa với [`Ring`]: đầy thì ghi đè bản ghi cũ nhất, `iter` theo thời gian.
#[derive(Debug, Clone)]
pub struct FixedRing<const N: usize> {
    buf: [Record; N],
    next: usize,
    len: usize,
}

impl<const N: usize> Default for FixedRing<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> FixedRing<N> {
    pub const fn new() -> Self {
        assert!(N > 0, "FixedRing cần N > 0");
        Self {
            buf: [Record {
                t_ns: 0,
                hart: 0,
                kind: 0,
                tcb: 0,
                a: 0,
                b: 0,
            }; N],
            next: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, r: Record) {
        self.buf[self.next] = r;
        self.next = (self.next + 1) % N;
        if self.len < N {
            self.len += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &Record> {
        let start = if self.len == N { self.next } else { 0 };
        (0..self.len).map(move |i| &self.buf[(start + i) % N])
    }
}
