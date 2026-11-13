// SPDX-License-Identifier: MIT OR Apache-2.0
//! Định dạng **VBC1** — bytecode nội bộ của ViScript (ST-19, VD-52).
//!
//! # Bố cục tệp
//!
//! ```text
//! magic      : 4 byte  = "VBC1"
//! version    : u16     = 1
//! hash nguồn : 32 byte = FNV-1a 64 bit lặp 4 lần với seed khác nhau (**KHÔNG** phải SHA-256 —
//!                        xem cảnh báo dưới)
//! n_const    : u32, rồi từng hằng: tag u8 + độ dài u32 + bytes (xem [`Const`])
//! n_func     : u32, rồi từng hàm: tên (u32 + bytes), arity u16, nparams u16, nlocals u16,
//!                        max_stack u16, n_code u32 + lệnh, n_spans u32 + span
//! entry      : u32 (chỉ số hàm `main`)
//! ```
//!
//! Mọi số **little-endian**, mọi độ dài **tường minh** (u32) — `decode` từ chối dữ liệu hỏng bằng
//! [`crate::decode::DecodeError`], **không** panic.
//!
//! # ⚠️ Hash nguồn **không** phải SHA-256
//!
//! VD-52 đòi "hash nguồn 32 byte" cho khoá cache. Repo **không** có crate hash ngoài allowlist, nên
//! ST-19 dùng **FNV-1a 64 bit × 4 seed** ⇒ 32 byte. Đây **chỉ** là khoá cache, **không** phải bằng
//! chứng an toàn: khi làm **consent/quyền** (§11.3, vòng sau) **phải** thay bằng **SHA-256 thật**
//! (crate `sha2`, cần Agent 0 duyệt allowlist). Hàng rào bù: **mọi** bytecode nạp từ cache **luôn**
//! phải qua **verifier** (ST-21, VD-52 #3) — xem [`crate::decode::decode`].
//!
//! # Vì sao verifier kiểm lại được
//!
//! Mỗi lệnh có **span** (bảng song song, nén theo `(pc_bắt_đầu, line, col)`); đích nhảy là **tương
//! đối `i32`**; mỗi hàm ghi `arity`/`nparams`/`nlocals`/`max_stack`. ST-21 **kiểm lại độc lập từ
//! byte stream** (không tin `max_stack` ghi trong tệp).

use vis_types::{BinOp, UnOp};

/// Bốn byte nhận dạng: `VBC1`.
pub const MAGIC: [u8; 4] = *b"VBC1";
/// Phiên bản định dạng hiện tại.
pub const VERSION: u16 = 1;
/// Số byte hash nguồn.
pub const HASH_LEN: usize = 32;

/// Hằng trong **const pool** — giữ **nguyên văn** literal HIR (cùng `LitKind`) để không mất mát.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Const {
    /// `LitKind` của `vis-types` dưới dạng số: 0 Int, 1 Float, 2 Size, 3 Duration, 4 Percent,
    /// 5 Str, 6 Bytes, 7 Path, 8 Char, 9 Bool.
    pub kind: u8,
    /// Nguyên văn (đã NFC) — `1`, `5mb`, `"a"`, `true`, `50pct`, …
    pub text: String,
}

/// Một hàm (hoặc `main`, hoặc closure) trong module.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Func {
    pub name: String,
    /// Số tham số **vị trí** (bắt buộc).
    pub arity: u16,
    /// Tổng số tham số (kể cả `--tên` có default).
    pub nparams: u16,
    /// Số ô cục bộ (gồm tham số + biến khai trong thân).
    pub nlocals: u16,
    /// Độ sâu ngăn xếp tối đa — codegen tính; **ST-21 kiểm lại độc lập**.
    pub max_stack: u16,
    pub code: Vec<Instr>,
    /// Span của **từng** lệnh (cùng độ dài với `code`).
    pub spans: Vec<Span>,
}

/// Vị trí nguồn của một lệnh: **khoảng byte** `[start, end)` trong văn bản đã NFC.
///
/// Vì sao byte chứ không `(dòng, cột)`: VM/verifier nhận **cùng** văn bản nguồn (đã lưu ở cache),
/// nên tính `dòng:cột` lúc **báo lỗi** là đủ và không phải mã hoá lại bảng dòng.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

/// Tập lệnh **VBC1** — máy ngăn xếp; chú thích ghi `vào→ra` (số giá trị pop → push).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instr {
    /// Hằng `consts[i]` — 0→1.
    Const(u32),
    /// Đọc ô cục bộ `i` — 0→1.
    LoadLocal(u16),
    /// Ghi ô cục bộ `i` (khai báo `let`/`var`) — 1→0.
    StoreLocal(u16),
    /// Gán lại ô `i` — 1→0.
    Assign(u16),
    /// Đọc biến tự do `i` của closure (snapshot theo VD-51) — 0→1.
    LoadCaptured(u16),
    /// Bỏ giá trị đỉnh — 1→0.
    Pop,
    /// Toán tử nhị phân (`vis_types::BinOp`) — 2→1.
    Bin(BinOp),
    /// Toán tử một ngôi (`vis_types::UnOp`) — 1→1.
    Un(UnOp),
    /// Gọi hàm `f` với `n` đối số — n→1.
    Call(u32, u16),
    /// Gọi builtin theo **tên** (const idx) — n→1.
    CallBuiltin(u32, u16),
    /// Gọi phương thức theo **tên** (const idx), receiver là đối số 1 — n+1→1.
    CallMethod(u32, u16),
    /// Nhảy tương đối — 0→0.
    Jump(i32),
    /// Nhảy nếu đỉnh **sai** (pop) — 1→0.
    JumpIfFalse(i32),
    /// Nhảy nếu đỉnh **không truthy** nhưng **giữ** giá trị — 1→1 (cho `and`/`or`).
    JumpIfNotTrue(i32),
    /// Nhảy nếu đỉnh là `none` (pop) — 1→0 (cho `??`).
    JumpIfNone(i32),
    /// Nhảy nếu đỉnh là `some` (pop) — 1→0.
    JumpIfSome(i32),
    /// Trả giá trị đỉnh — 1→0.
    Return,
    /// Kết thúc chương trình — 0→0.
    Halt,
    /// Dựng `list` từ `n` phần tử — n→1.
    MakeList(u16),
    /// Dựng `set` — n→1.
    MakeSet(u16),
    /// Dựng tuple (biểu diễn nội bộ như list) — n→1.
    MakeTuple(u16),
    /// Dựng `map` từ `n` cặp khoá/giá trị — 2n→1.
    MakeMap(u16),
    /// Dựng bản ghi: tên kiểu (const idx) + `n` cặp tên/giá trị — 2n→1.
    MakeRecord(u32, u16),
    /// Dựng biến thể enum: tên biến thể (const idx) + `n` đối số — n→1.
    MakeEnum(u32, u16),
    /// `some(v)` — 1→1.
    MakeSome,
    /// `none` — 0→1.
    MakeNone,
    /// `ok(v)` — 1→1.
    MakeOk,
    /// `err(v)` — 1→1.
    MakeErr,
    /// Tạo closure: hàm `f` + `n` biến tự do (snapshot) — n→1.
    MakeClosure(u32, u16),
    /// Truy cập trường theo tên (const idx) — 1→1.
    Field(u32),
    /// Chỉ số `base[i]` — 2→1.
    Index,
    /// Cập nhật `base[i] = v` (list/map) — 3→1.
    Set,
    /// `base with ten: v …` — base + `n` cặp tên/giá trị — 2n+1→1.
    With(u16),
    /// Nội suy chuỗi từ `n` mảnh (mảnh chữ là `Const`) — n→1.
    Interp(u16),
    /// Khởi tạo vòng lặp trên đỉnh — 1→1.
    IterInit,
    /// Lấy phần tử kế: nhảy tới đích khi hết — 1→2 (giá trị + iterator mới).
    IterNext(i32),
    /// So khớp literal `consts[i]` với đỉnh — 1→1 (bool).
    TestLit(u32),
    /// So khớp biến thể `consts[i]` với `n` đối số — 1→1.
    TestCtor(u32, u16),
    /// So khớp tuple `n` thành phần — 1→1.
    TestTuple(u16),
    /// So khớp list `n` phần tử (không `..`) — 1→1.
    TestList(u16),
    /// So khớp bản ghi: kiểu (const) + `n` tên + `open` — 1→1.
    TestRecord(u32, u16, u8),
    /// So khớp khoảng `[lo, hi)` từ hai hằng — 1→1.
    TestRange(u32, u32, u8),
    /// Gán đỉnh vào ô `i` (**không** pop) — 1→1.
    BindLocal(u16),
    /// `expr?` — lan truyền `err`/`none` — 1→1.
    Try,
    /// Nhân đôi giá trị đỉnh — 1→2 (dùng cho match/destructuring).
    Dup,
    /// Lấy **đối số thứ `i`** của enum/option/result/tuple trên đỉnh — 1→1 (gán biến pattern).
    Unwrap(u16),
    /// Gọi **giá trị hàm** trên đỉnh (closure/biến) với `n` đối số — n+1→1.
    CallValue(u16),
    /// Bóc `some(x)` → `x`; giá trị khác giữ nguyên — 1→1 (cho `?.`/`??`).
    UnwrapOpt,
    /// `true` nếu đỉnh là `()` — 1→1 (prologue tham số có default).
    IsUnit,
    /// Hoặc logic **không đoản mạch** trên hai `bool` — 2→1 (gộp pattern `|`).
    BoolOr,
    /// Và logic **không đoản mạch** trên hai `bool` — 2→1 (gộp test pattern lồng).
    BoolAnd,
}

impl Instr {
    /// Tag ổn định trên đĩa (thêm lệnh ⇒ **thêm tag mới**, không đổi số cũ).
    pub fn tag(self) -> u8 {
        match self {
            Instr::Const(_) => 0,
            Instr::LoadLocal(_) => 1,
            Instr::StoreLocal(_) => 2,
            Instr::Assign(_) => 3,
            Instr::LoadCaptured(_) => 4,
            Instr::Pop => 5,
            Instr::Bin(_) => 6,
            Instr::Un(_) => 7,
            Instr::Call(..) => 8,
            Instr::CallBuiltin(..) => 9,
            Instr::CallMethod(..) => 10,
            Instr::Jump(_) => 11,
            Instr::JumpIfFalse(_) => 12,
            Instr::JumpIfNotTrue(_) => 13,
            Instr::JumpIfNone(_) => 14,
            Instr::JumpIfSome(_) => 15,
            Instr::Return => 16,
            Instr::Halt => 17,
            Instr::MakeList(_) => 18,
            Instr::MakeSet(_) => 19,
            Instr::MakeTuple(_) => 20,
            Instr::MakeMap(_) => 21,
            Instr::MakeRecord(..) => 22,
            Instr::MakeEnum(..) => 23,
            Instr::MakeSome => 24,
            Instr::MakeNone => 25,
            Instr::MakeOk => 26,
            Instr::MakeErr => 27,
            Instr::MakeClosure(..) => 28,
            Instr::Field(_) => 29,
            Instr::Index => 30,
            Instr::Set => 31,
            Instr::With(_) => 32,
            Instr::Interp(_) => 33,
            Instr::IterInit => 34,
            Instr::IterNext(_) => 35,
            Instr::TestLit(_) => 36,
            Instr::TestCtor(..) => 37,
            Instr::TestTuple(_) => 38,
            Instr::TestList(_) => 39,
            Instr::TestRecord(..) => 40,
            Instr::TestRange(..) => 41,
            Instr::BindLocal(_) => 42,
            Instr::Try => 43,
            Instr::Dup => 44,
            Instr::Unwrap(_) => 45,
            Instr::CallValue(_) => 46,
            Instr::UnwrapOpt => 47,
            Instr::IsUnit => 48,
            Instr::BoolOr => 49,
            Instr::BoolAnd => 50,
        }
    }

    /// Số byte toán hạng (không kể tag) — dùng để `decode` kiểm độ dài tường minh.
    pub fn operand_len(self) -> usize {
        match self {
            Instr::Const(_) | Instr::CallBuiltin(..) | Instr::Field(_) | Instr::TestLit(_) => 4,
            Instr::CallMethod(..) => 6,
            Instr::Call(..) => 6,
            Instr::Jump(_)
            | Instr::JumpIfFalse(_)
            | Instr::JumpIfNotTrue(_)
            | Instr::JumpIfNone(_)
            | Instr::JumpIfSome(_)
            | Instr::IterNext(_) => 4,
            Instr::LoadLocal(_)
            | Instr::StoreLocal(_)
            | Instr::Assign(_)
            | Instr::LoadCaptured(_)
            | Instr::MakeList(_)
            | Instr::MakeSet(_)
            | Instr::MakeTuple(_)
            | Instr::MakeMap(_)
            | Instr::With(_)
            | Instr::Interp(_)
            | Instr::TestTuple(_)
            | Instr::TestList(_)
            | Instr::BindLocal(_) => 2,
            Instr::MakeRecord(..) => 6,
            Instr::MakeEnum(..) => 6,
            Instr::MakeClosure(..) => 6,
            Instr::TestCtor(..) => 6,
            Instr::TestRecord(..) => 7,
            Instr::TestRange(..) => 9,
            Instr::Bin(_) => 1,
            Instr::Un(_) => 1,
            Instr::Pop
            | Instr::Return
            | Instr::Halt
            | Instr::MakeSome
            | Instr::MakeNone
            | Instr::MakeOk
            | Instr::MakeErr
            | Instr::Index
            | Instr::Set
            | Instr::IterInit
            | Instr::Try
            | Instr::Dup => 0,
            Instr::Unwrap(_) => 2,
            Instr::CallValue(_) => 2,
            Instr::UnwrapOpt => 0,
            Instr::IsUnit | Instr::BoolOr | Instr::BoolAnd => 0,
        }
    }
}

/// Tag **ổn định** cho `BinOp` (một nguồn ngữ nghĩa: enum của `vis-types`) — **hàm tự do** vì
/// `impl` ngoài crate là lỗi.
pub fn binop_tag(op: BinOp) -> u8 {
    match op {
        BinOp::Add => 0,
        BinOp::Sub => 1,
        BinOp::Mul => 2,
        BinOp::Div => 3,
        BinOp::Rem => 4,
        BinOp::Pow => 5,
        BinOp::Range => 6,
        BinOp::RangeIncl => 7,
        BinOp::Coalesce => 8,
        BinOp::Eq => 9,
        BinOp::Ne => 10,
        BinOp::Lt => 11,
        BinOp::Le => 12,
        BinOp::Gt => 13,
        BinOp::Ge => 14,
        BinOp::And => 15,
        BinOp::Or => 16,
        BinOp::Pipe => 17,
    }
}

/// Nghịch đảo của [`binop_tag`] — `None` nếu tag lạ (dữ liệu hỏng).
pub fn binop_from_tag(t: u8) -> Option<BinOp> {
    Some(match t {
        0 => BinOp::Add,
        1 => BinOp::Sub,
        2 => BinOp::Mul,
        3 => BinOp::Div,
        4 => BinOp::Rem,
        5 => BinOp::Pow,
        6 => BinOp::Range,
        7 => BinOp::RangeIncl,
        8 => BinOp::Coalesce,
        9 => BinOp::Eq,
        10 => BinOp::Ne,
        11 => BinOp::Lt,
        12 => BinOp::Le,
        13 => BinOp::Gt,
        14 => BinOp::Ge,
        15 => BinOp::And,
        16 => BinOp::Or,
        17 => BinOp::Pipe,
        _ => return None,
    })
}

/// Tag ổn định cho `UnOp`.
pub fn unop_tag(op: UnOp) -> u8 {
    match op {
        UnOp::Neg => 0,
        UnOp::Not => 1,
    }
}

/// Nghịch đảo của [`unop_tag`].
pub fn unop_from_tag(t: u8) -> Option<UnOp> {
    Some(match t {
        0 => UnOp::Neg,
        1 => UnOp::Not,
        _ => return None,
    })
}

/// Module bytecode đã hạ từ HIR.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Module {
    pub version: u16,
    pub source_hash: [u8; HASH_LEN],
    pub consts: Vec<Const>,
    pub funcs: Vec<Func>,
    /// Chỉ số hàm vào (`main`).
    pub entry: u32,
}

impl Module {
    /// Tổng số lệnh của mọi hàm — dùng cho test "chống xanh-giả".
    pub fn total_code(&self) -> usize {
        self.funcs.iter().map(|f| f.code.len()).sum()
    }

    /// Kiểm **cấu trúc tối thiểu** (ST-19; verifier đầy đủ là ST-21): chỉ số hợp lệ + độ dài bảng.
    pub fn is_well_formed(&self) -> bool {
        if self.version != VERSION || self.entry as usize >= self.funcs.len() {
            return false;
        }
        self.funcs.iter().all(|f| {
            f.code.len() == f.spans.len()
                && !f.code.is_empty()
                && f.code.iter().all(|ins| match ins {
                    Instr::Const(i) | Instr::TestLit(i) => (*i as usize) < self.consts.len(),
                    Instr::Call(fi, _) | Instr::MakeClosure(fi, _) => {
                        (*fi as usize) < self.funcs.len()
                    }
                    _ => true,
                })
        })
    }
}

/// Hash nguồn 32 byte: **FNV-1a 64 bit × 4 seed** (xem cảnh báo ở đầu tệp — **không** phải SHA-256).
pub fn source_hash(src: &str) -> [u8; HASH_LEN] {
    const SEEDS: [u64; 4] = [
        0xcbf2_9ce4_8422_2325,
        0x9e37_79b9_7f4a_7c15,
        0xff51_afd7_ed55_8ccd,
        0xc4ce_b9fe_1a85_ec53,
    ];
    let mut out = [0u8; HASH_LEN];
    for (i, seed) in SEEDS.iter().enumerate() {
        let mut h = *seed;
        for b in src.as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        out[i * 8..i * 8 + 8].copy_from_slice(&h.to_le_bytes());
    }
    out
}
