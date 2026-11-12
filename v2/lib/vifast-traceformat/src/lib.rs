// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-traceformat` — bản ghi trace **32 B** dùng chung sim/QEMU (KT-12, D2-38).
//!
//! Layout little-endian, không padding: `t_ns:u64, hart:u16, kind:u16, tcb:u32, a:u64, b:u64`.
//! Ring là bộ đệm vòng cố định: ghi đè bản ghi cũ nhất khi đầy, đọc ra theo thứ tự thời gian.

pub const RECORD_BYTES: usize = 32;
pub const KIND_SYSCALL: u16 = 1;
pub const KIND_IPC: u16 = 2;
pub const KIND_SCHED: u16 = 3;
pub const KIND_FAULT: u16 = 4;
pub const KIND_STORE: u16 = 5;

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
    (KIND_SYSCALL..=KIND_STORE).contains(&k)
}

/// Giải mã một dump liền mạch; lỗi nếu độ dài không chia hết 32 hoặc kind lạ.
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
#[derive(Debug, Clone)]
pub struct Ring {
    buf: Vec<Record>,
    cap: usize,
    next: usize,
    len: usize,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "ring phải có sức chứa > 0");
        Self {
            buf: vec![Record::new(0, 0, 0, 0, 0, 0); cap],
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

#[cfg(test)]
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
            vec![3, 4, 5, 6],
            "phải giữ 4 bản ghi mới nhất theo thứ tự"
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
        assert_eq!(back, vec![r(1), r(2)]);
    }
}
