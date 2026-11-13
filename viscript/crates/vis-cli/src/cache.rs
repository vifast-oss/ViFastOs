// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Cache bytecode** theo hash closure (ST-23, VD-52 #3): `.../vifast/vis/<hash>.vbc`.
//!
//! Hash (FNV-1a 64 bit × 4, VD-54) phủ: **văn bản nguồn** + **phiên bản compiler** + **phiên bản
//! định dạng VBC1** + **chế độ** (`.vis`/`.vish`) — tức "closure" của nguồn và phụ thuộc biên dịch.
//!
//! **Luôn** nạp qua [`vis_bytecode::load_verified`]: bytes hỏng/lệch ⇒ biên dịch lại và ghi lại;
//! **không bao giờ** chạy bytecode chưa xác minh (VD-52 #3).

use std::path::PathBuf;

/// Phiên bản compiler dùng trong hash (đổi ⇒ cache cũ tự vô hiệu).
pub const PHIEN_BAN: &str = env!("CARGO_PKG_VERSION");

/// Thư mục cache: `$XDG_CACHE_HOME/vifast/vis` hoặc `~/.cache/vifast/vis`.
pub fn thu_muc() -> Option<PathBuf> {
    let goc = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(goc.join("vifast").join("vis"))
}

/// Khoá cache 32 byte: FNV-1a×4 trên (nguồn, phiên bản compiler, phiên bản VBC1, chế độ).
pub fn khoa(nguon: &str, che_do: &str) -> [u8; 32] {
    let mut s = String::with_capacity(nguon.len() + 64);
    s.push_str("vifast-vis-cache-v1\n");
    s.push_str(PHIEN_BAN);
    s.push('\n');
    s.push_str(&vis_bytecode::VERSION.to_string());
    s.push('\n');
    s.push_str(che_do);
    s.push('\n');
    s.push_str(nguon);
    vis_bytecode::format::source_hash(&s)
}

/// Đường dẫn tệp cache cho một khoá.
pub fn duong_dan(k: &[u8; 32]) -> Option<PathBuf> {
    let mut ten = String::with_capacity(64 + 4);
    for b in k {
        ten.push_str(&format!("{b:02x}"));
    }
    ten.push_str(".vbc");
    Some(thu_muc()?.join(ten))
}

/// Nạp module từ cache — **chỉ** khi `load_verified` chấp nhận.
pub fn nap(k: &[u8; 32]) -> Option<vis_bytecode::Module> {
    let p = duong_dan(k)?;
    let bytes = std::fs::read(&p).ok()?;
    vis_bytecode::load_verified(&bytes).ok()
}

/// Ghi module đã xác minh xuống cache (lỗi I/O ⇒ bỏ qua, không làm hỏng lần chạy).
pub fn luu(k: &[u8; 32], m: &vis_bytecode::Module) -> bool {
    // Chỉ ghi thứ **qua được** verifier.
    if vis_bytecode::verify(m).is_err() {
        return false;
    }
    let Some(p) = duong_dan(k) else {
        return false;
    };
    if let Some(dir) = p.parent() {
        if std::fs::create_dir_all(dir).is_err() {
            return false;
        }
    }
    let bytes = vis_bytecode::encode(m);
    // Ghi tạm rồi đổi tên: tránh tệp cụt khi bị ngắt.
    let tmp = p.with_extension("vbc.tmp");
    if std::fs::write(&tmp, &bytes).is_err() {
        return false;
    }
    std::fs::rename(&tmp, &p).is_ok()
}

/// Xoá toàn bộ cache; trả số tệp đã xoá.
pub fn xoa() -> usize {
    let Some(dir) = thu_muc() else {
        return 0;
    };
    let Ok(ds) = std::fs::read_dir(&dir) else {
        return 0;
    };
    let mut n = 0;
    for e in ds.flatten() {
        let p = e.path();
        if p.extension()
            .map(|x| x == "vbc" || x == "tmp")
            .unwrap_or(false)
            && std::fs::remove_file(&p).is_ok()
        {
            n += 1;
        }
    }
    n
}
