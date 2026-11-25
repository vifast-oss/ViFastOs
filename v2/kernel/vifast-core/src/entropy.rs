// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-418b** — API entropy của `vifast-core`: [`EntropyPool`] **chỉ** cấp byte sau khi được
//! `seed` từ nguồn phần cứng (`virtio-rng`, xem `vifast_virtio::rngdev`).
//!
//! # Vì sao API này ở **core**, không ở driver
//!
//! Driver `virtio-rng` biết cách **lấy** byte từ thiết bị; nó **không** được quyết định "byte này
//! có phải entropy không" cho toàn kernel, và cũng không được là nơi duy nhất giữ trạng thái —
//! nếu vậy mọi consumer (khoá, ASLR, `getrandom` tương lai) sẽ phụ thuộc trực tiếp vào một driver
//! phần cứng. Tách ở đây cho **một** hợp đồng: `seed` một lần từ thiết bị, `fill` nhiều lần cho
//! consumer; core `no_std`, `forbid(unsafe_code)`, không cấp phát (D2-17).
//!
//! # Fail-closed
//!
//! - Chưa `seed` ⇒ [`EntropyError::NotSeeded`], `out` **không** bị ghi một byte nào.
//! - Seed quá ngắn (< [`ENTROPY_MIN_SEED_BYTES`]) ⇒ [`EntropyError::SeedTooShort`].
//! - Seed **suy biến** (mọi byte giống nhau — đúng dạng "thiết bị trả toàn 0") ⇒
//!   [`EntropyError::SeedDegenerate`]. Đây là lớp thứ hai: driver đã có cổng chất lượng
//!   (`vifast_virtio::rng::entropy_ok`), nhưng API core **không** tin người gọi.
//!
//! # Hàm trộn: **không** phải CSPRNG đã được đánh giá
//!
//! Trạng thái 256 bit ([`ENTROPY_STATE_WORDS`] × 64 bit) + bộ đếm khối, mỗi khối 32 byte chạy
//! bốn vòng `SipRound` (ARX: cộng–quay–xor, cùng họ với SipHash) sau khi trộn bộ đếm vào
//! `state[0]`. Mục tiêu **đo được** của KT-418b là ba bất biến: (a) hai lần `fill` liên tiếp khác
//! nhau, (b) 4 KiB ra **không** có khối 8 byte lặp, (c) 64 KiB ra phủ đủ 256 giá trị byte — cả ba
//! có test host (`entropy_tests.rs`). **Nợ có tên**: khi kernel cần entropy **mật mã** (khoá dài
//! hạn, chống kẻ tấn công biết seed), thay hàm trộn bằng ChaCha20 (hoặc tương đương đã được đánh
//! giá) — việc đó **không** đổi hợp đồng `seed`/`fill` ở đây.
//!
//! # Tất định theo seed — có chủ đích
//!
//! Cùng seed ⇒ cùng dòng ra. Đây là điều kiện để test host tái lập được **và** để một lần boot
//! có thể phát lại (replay) khi điều tra lỗi. Không có tính chất này thì "hai lần chạy khác nhau"
//! không phân biệt được với "hàm trộn hỏng".

/// Số word 64 bit của trạng thái (256 bit).
pub const ENTROPY_STATE_WORDS: usize = 4;
/// Số byte seed **tối thiểu** — dưới ngưỡng này không đủ trạng thái để khởi động pool.
pub const ENTROPY_MIN_SEED_BYTES: usize = 32;
/// Số byte một khối ra (4 word).
pub const ENTROPY_BLOCK_BYTES: usize = ENTROPY_STATE_WORDS * 8;

/// Hằng khởi động trạng thái — "nothing up my sleeve" (chữ số của π), **không** phải khoá.
const INIT: [u64; ENTROPY_STATE_WORDS] = [
    0x243f_6a88_85a3_08d3,
    0x1319_8a2e_0370_7344,
    0xa409_3822_299f_31d0,
    0x082e_fa98_ec4e_6c89,
];

/// Mã lỗi của API entropy — **tên** để in evidence (D2-102).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntropyError {
    /// `fill` trước `seed`.
    NotSeeded,
    /// Seed ngắn hơn [`ENTROPY_MIN_SEED_BYTES`].
    SeedTooShort,
    /// Seed suy biến: mọi byte giống nhau (thiết bị trả toàn 0/0xff).
    SeedDegenerate,
}

impl EntropyError {
    /// Tên để in evidence (D2-102).
    pub const fn name(self) -> &'static str {
        match self {
            EntropyError::NotSeeded => "not_seeded",
            EntropyError::SeedTooShort => "seed_too_short",
            EntropyError::SeedDegenerate => "seed_degenerate",
        }
    }
}

/// Ảnh chụp bộ đếm — đọc một lần, in từ dữ liệu (D2-102).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntropyStats {
    /// Số byte của seed **hiện hành** (0 khi chưa seed).
    pub seeded_bytes: u64,
    /// Tổng số byte đã cấp qua [`EntropyPool::fill`].
    pub bytes_out: u64,
    /// Số lần `seed` **thành công** sau lần đầu (0 ⇒ chưa seed lại).
    pub reseeds: u64,
    /// Số khối 32 byte đã sinh.
    pub blocks: u64,
}

/// Pool entropy: trạng thái 256 bit + bộ đếm khối, **chỉ** cấp byte sau khi `seed`.
#[derive(Clone, Copy, Debug)]
pub struct EntropyPool {
    state: [u64; ENTROPY_STATE_WORDS],
    counter: u64,
    seeded: bool,
    seeded_bytes: u64,
    bytes_out: u64,
    reseeds: u64,
    blocks: u64,
}

impl Default for EntropyPool {
    fn default() -> Self {
        Self::new()
    }
}

impl EntropyPool {
    /// Pool **chưa** seed — `fill` sẽ trả [`EntropyError::NotSeeded`].
    pub const fn new() -> Self {
        Self {
            state: INIT,
            counter: 0,
            seeded: false,
            seeded_bytes: 0,
            bytes_out: 0,
            reseeds: 0,
            blocks: 0,
        }
    }

    /// `true` ⇔ đã `seed` thành công ít nhất một lần.
    pub const fn is_seeded(&self) -> bool {
        self.seeded
    }

    /// Nạp seed từ nguồn phần cứng. Seed **sau cùng** thay thế seed trước (không cộng dồn).
    ///
    /// Fail-closed: quá ngắn ⇒ [`EntropyError::SeedTooShort`]; mọi byte giống nhau ⇒
    /// [`EntropyError::SeedDegenerate`]. Lỗi **không** đổi trạng thái đã có (pool đang seed vẫn
    /// dùng được với seed cũ — không tự làm hỏng mình vì một lần nạp sai).
    pub fn seed(&mut self, bytes: &[u8]) -> Result<(), EntropyError> {
        if bytes.len() < ENTROPY_MIN_SEED_BYTES {
            return Err(EntropyError::SeedTooShort);
        }
        let first = bytes[0];
        if bytes.iter().all(|byte| *byte == first) {
            return Err(EntropyError::SeedDegenerate);
        }
        let mut state = INIT;
        // Trộn từng khối 8 byte vào word tương ứng, rồi chạy một vòng trộn cho mỗi khối — seed
        // dài hơn 32 byte vẫn ảnh hưởng toàn bộ trạng thái (không cắt im lặng).
        for (index, chunk) in bytes.chunks(8).enumerate() {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            state[index % ENTROPY_STATE_WORDS] ^= u64::from_le_bytes(word);
            mix(&mut state);
        }
        // Bộ đếm khởi động từ seed để hai seed khác nhau không cùng dòng ra ở khối 0.
        let mut counter = 0u64;
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            counter = counter.rotate_left(7) ^ u64::from_le_bytes(word);
        }
        self.state = state;
        self.counter = counter;
        self.seeded = true;
        self.seeded_bytes = bytes.len() as u64;
        if self.blocks > 0 || self.bytes_out > 0 {
            self.reseeds += 1;
        }
        Ok(())
    }

    /// Cấp `out.len()` byte. Chưa seed ⇒ [`EntropyError::NotSeeded`] và `out` **không** đổi.
    pub fn fill(&mut self, out: &mut [u8]) -> Result<(), EntropyError> {
        if !self.seeded {
            return Err(EntropyError::NotSeeded);
        }
        let mut written = 0usize;
        while written < out.len() {
            let block = self.next_block();
            let take = (out.len() - written).min(ENTROPY_BLOCK_BYTES);
            out[written..written + take].copy_from_slice(&block[..take]);
            written += take;
        }
        self.bytes_out += out.len() as u64;
        Ok(())
    }

    /// Ảnh chụp bộ đếm.
    pub const fn stats(&self) -> EntropyStats {
        EntropyStats {
            seeded_bytes: self.seeded_bytes,
            bytes_out: self.bytes_out,
            reseeds: self.reseeds,
            blocks: self.blocks,
        }
    }

    /// Sinh một khối 32 byte: trộn bộ đếm vào trạng thái rồi chạy bốn vòng `SipRound`.
    fn next_block(&mut self) -> [u8; ENTROPY_BLOCK_BYTES] {
        self.counter = self.counter.wrapping_add(1);
        self.state[0] ^= self.counter;
        self.state[1] ^= self.counter.rotate_left(32);
        for _ in 0..4 {
            mix(&mut self.state);
        }
        self.blocks += 1;
        let mut block = [0u8; ENTROPY_BLOCK_BYTES];
        for (index, word) in self.state.iter().enumerate() {
            block[index * 8..index * 8 + 8].copy_from_slice(&word.to_le_bytes());
        }
        block
    }
}

/// Một vòng trộn ARX 4 nhánh (cùng họ `SipRound` của SipHash) — xem đầu tệp về giới hạn.
fn mix(state: &mut [u64; ENTROPY_STATE_WORDS]) {
    state[0] = state[0].wrapping_add(state[1]);
    state[1] = state[1].rotate_left(13);
    state[1] ^= state[0];
    state[0] = state[0].rotate_left(32);
    state[2] = state[2].wrapping_add(state[3]);
    state[3] = state[3].rotate_left(16);
    state[3] ^= state[2];
    state[0] = state[0].wrapping_add(state[3]);
    state[3] = state[3].rotate_left(21);
    state[3] ^= state[0];
    state[2] = state[2].wrapping_add(state[1]);
    state[1] = state[1].rotate_left(17);
    state[1] ^= state[2];
    state[2] = state[2].rotate_left(32);
}
