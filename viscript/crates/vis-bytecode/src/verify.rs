// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Trình xác minh VBC1** (ST-21, VD-52 #3, VD-54 (2), thiết kế §14.5/§17).
//!
//! Xác minh **từ byte stream** (không tin metadata): chỉ số const/hàm/closure, đích nhảy, **độ sâu
//! ngăn xếp mô phỏng độc lập trên mọi nhánh** (worklist CFG), kích thước khung, `Return`/`Halt` không
//! rơi khỏi cuối hàm, bảng span khớp code.
//!
//! `max_stack` **được kiểm lại**: verifier tự tính `need` và **từ chối** nếu module khai **nhỏ hơn**
//! (`VerifyError::NeedStack`). Codegen dùng **chính** hàm [`need_stack`] để ghi giá trị ⇒ không có
//! khoảng chênh.
//!
//! Bytecode nạp từ cache **luôn** phải qua đây (ST-23): dùng [`load_verified`] (decode + verify).

use crate::decode::DecodeError;
use crate::format::{Instr, Module, VERSION};
use vis_types::{BinOp, UnOp};

/// Lỗi xác minh — **không** panic, luôn nêu hàm + `pc` khi có.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// Phiên bản khác [`VERSION`].
    BadVersion(u16),
    /// `entry` ngoài phạm vi hàm.
    BadEntry(u32),
    /// Chỉ số const ngoài phạm vi.
    BadConstIdx { func: u32, pc: usize, idx: u32 },
    /// Chỉ số hàm ngoài phạm vi.
    BadFuncIdx { func: u32, pc: usize, idx: u32 },
    /// Đích nhảy ngoài `[0, code.len()]`.
    BadJump { func: u32, pc: usize, off: i32 },
    /// Ngăn xếp âm tại một điểm (đường đi cụ thể).
    StackUnderflow { func: u32, pc: usize },
    /// Hai đường tới cùng `pc` với độ sâu khác nhau.
    StackMismatch {
        func: u32,
        pc: usize,
        a: i32,
        b: i32,
    },
    /// `max_stack` khai **nhỏ hơn** độ sâu thật.
    NeedStack { func: u32, declared: u16, need: u16 },
    /// Hàm không có lệnh nào.
    EmptyCode { func: u32 },
    /// Có đường rơi khỏi cuối hàm (không `Return`/`Halt`).
    FallsOffEnd { func: u32, pc: usize },
    /// Bảng span không khớp số lệnh.
    SpanMismatch {
        func: u32,
        code: usize,
        spans: usize,
    },
    /// `arity > nparams` hoặc `nlocals < nparams`.
    BadArity {
        func: u32,
        arity: u16,
        nparams: u16,
        nlocals: u16,
    },
    /// `MakeClosure` đòi ít biến bắt hơn số `LoadCaptured` của hàm đích dùng.
    ClosureFree {
        func: u32,
        pc: usize,
        nfree: u16,
        used: u16,
    },
    /// Ô cục bộ ngoài `nlocals`.
    LocalOverflow {
        func: u32,
        pc: usize,
        slot: u16,
        nlocals: u16,
    },
}

impl core::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            VerifyError::BadVersion(v) => write!(f, "version {v} ≠ {VERSION}"),
            VerifyError::BadEntry(i) => write!(f, "entry {i} ngoài phạm vi"),
            VerifyError::BadConstIdx { func, pc, idx } => {
                write!(f, "hàm {func} pc {pc}: chỉ số const {idx} ngoài phạm vi")
            }
            VerifyError::BadFuncIdx { func, pc, idx } => {
                write!(f, "hàm {func} pc {pc}: chỉ số hàm {idx} ngoài phạm vi")
            }
            VerifyError::BadJump { func, pc, off } => {
                write!(f, "hàm {func} pc {pc}: đích nhảy ngoài code (offset {off})")
            }
            VerifyError::StackUnderflow { func, pc } => {
                write!(f, "hàm {func} pc {pc}: ngăn xếp âm")
            }
            VerifyError::StackMismatch { func, pc, a, b } => {
                write!(f, "hàm {func} pc {pc}: hai đường tới với độ sâu {a} và {b}")
            }
            VerifyError::NeedStack {
                func,
                declared,
                need,
            } => write!(f, "hàm {func}: max_stack khai {declared} < cần {need}"),
            VerifyError::EmptyCode { func } => write!(f, "hàm {func}: code rỗng"),
            VerifyError::FallsOffEnd { func, pc } => {
                write!(f, "hàm {func} pc {pc}: rơi khỏi cuối hàm")
            }
            VerifyError::SpanMismatch { func, code, spans } => {
                write!(f, "hàm {func}: {code} lệnh nhưng {spans} span")
            }
            VerifyError::BadArity {
                func,
                arity,
                nparams,
                nlocals,
            } => write!(
                f,
                "hàm {func}: arity {arity} / nparams {nparams} / nlocals {nlocals} không hợp lệ"
            ),
            VerifyError::ClosureFree {
                func,
                pc,
                nfree,
                used,
            } => write!(
                f,
                "hàm {func} pc {pc}: MakeClosure nfree {nfree} < {used} biến bắt được dùng"
            ),
            VerifyError::LocalOverflow {
                func,
                pc,
                slot,
                nlocals,
            } => write!(f, "hàm {func} pc {pc}: ô {slot} ngoài nlocals {nlocals}"),
        }
    }
}

/// Lỗi khi nạp từ byte: giải mã hoặc xác minh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// Bytes hỏng (magic/version/độ dài/tag/utf8…).
    Decode(DecodeError),
    /// Giải mã được nhưng không hợp lệ.
    Verify(VerifyError),
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::Decode(e) => write!(f, "decode: {e}"),
            LoadError::Verify(e) => write!(f, "verify: {e}"),
        }
    }
}

/// Số giá trị lệnh **pop** và **push** (theo đúng VM ST-20).
fn hieu_ung(i: Instr) -> (i32, i32) {
    match i {
        Instr::Const(_)
        | Instr::LoadLocal(_)
        | Instr::LoadCaptured(_)
        | Instr::MakeNone
        | Instr::PushUnit => (0, 1),
        Instr::StoreLocal(_) | Instr::Assign(_) | Instr::BindLocal(_) | Instr::Pop => (1, 0),
        Instr::Dup => (1, 2),
        Instr::Swap => (2, 2),
        Instr::Bin(_) => (2, 1),
        Instr::Un(_) => (1, 1),
        Instr::Call(_, n) => (n as i32, 1),
        Instr::CallBuiltin(_, n) => (n as i32, 1),
        Instr::CallMethod(_, n) => (n as i32 + 1, 1),
        Instr::CallValue(n) => (n as i32 + 1, 1),
        Instr::Jump(_) | Instr::Halt => (0, 0),
        Instr::JumpIfFalse(_) | Instr::JumpIfNone(_) | Instr::JumpIfSome(_) => (1, 0),
        Instr::JumpIfNotTrue(_) => (1, 1),
        Instr::Return => (1, 0),
        Instr::MakeList(n)
        | Instr::MakeSet(n)
        | Instr::MakeTuple(n)
        | Instr::MakeEnum(_, n)
        | Instr::MakeClosure(_, n)
        | Instr::Interp(n) => (n as i32, 1),
        Instr::MakeMap(n) => (2 * n as i32, 1),
        Instr::MakeRecord(_, n) => (2 * n as i32, 1),
        Instr::MakeSome | Instr::MakeOk | Instr::MakeErr => (1, 1),
        Instr::Field(_) => (1, 1),
        Instr::Index => (2, 1),
        Instr::Set => (3, 1),
        Instr::With(n) => (2 * n as i32 + 1, 1),
        Instr::IterInit => (1, 0),
        Instr::IterNext(_) => (0, 1),
        Instr::TestLit(_) | Instr::TestCtor(_, _) | Instr::TestTuple(_) | Instr::TestList(_) => {
            (1, 1)
        }
        Instr::TestRecord(_, _, _) | Instr::TestRange(_, _, _) => (1, 1),
        Instr::Unwrap(_) | Instr::UnwrapOpt | Instr::IsUnit | Instr::Try => (1, 1),
        Instr::BoolOr | Instr::BoolAnd => (2, 1),
    }
}

/// **Mô phỏng độ sâu ngăn xếp độc lập** trên mọi nhánh; trả `need` (độ sâu tối đa).
///
/// Dùng **chung** cho codegen (ghi `max_stack`) và verifier (kiểm lại) — một nguồn sự thật.
pub fn need_stack(code: &[Instr], func: u32) -> Result<u16, VerifyError> {
    let n = code.len();
    if n == 0 {
        return Err(VerifyError::EmptyCode { func });
    }
    let mut trong = vec![i32::MIN; n + 1];
    let mut hang = vec![(0usize, 0i32)];
    let mut need = 0i32;
    while let Some((pc, d)) = hang.pop() {
        if pc > n {
            return Err(VerifyError::BadJump {
                func,
                pc: pc.min(n),
                off: 0,
            });
        }
        if pc == n {
            // Rơi khỏi cuối hàm.
            return Err(VerifyError::FallsOffEnd {
                func,
                pc: pc.saturating_sub(1),
            });
        }
        if trong[pc] == i32::MIN {
            trong[pc] = d;
        } else if trong[pc] != d {
            return Err(VerifyError::StackMismatch {
                func,
                pc,
                a: trong[pc],
                b: d,
            });
        } else {
            continue;
        }
        let (pop, push) = hieu_ung(code[pc]);
        if d < pop {
            return Err(VerifyError::StackUnderflow { func, pc });
        }
        let sau = d - pop + push;
        if sau > need {
            need = sau;
        }
        let ke = sau as u16;
        let _ = ke;
        match code[pc] {
            Instr::Return | Instr::Halt => {}
            Instr::Jump(off) => hang.push((dich(pc, off, n, func)?, sau)),
            Instr::JumpIfFalse(off)
            | Instr::JumpIfNone(off)
            | Instr::JumpIfSome(off)
            | Instr::JumpIfNotTrue(off) => {
                hang.push((dich(pc, off, n, func)?, sau));
                hang.push((pc + 1, sau));
            }
            Instr::IterNext(off) => {
                // Nhảy = hết phần tử (không đẩy gì); rơi tiếp = có phần tử (đẩy 1).
                hang.push((dich(pc, off, n, func)?, d));
                hang.push((pc + 1, sau));
            }
            _ => hang.push((pc + 1, sau)),
        }
    }
    Ok((need.max(0) as u16).max(1))
}

fn dich(pc: usize, off: i32, n: usize, func: u32) -> Result<usize, VerifyError> {
    let t = pc as i64 + off as i64;
    if t < 0 || t > n as i64 {
        return Err(VerifyError::BadJump { func, pc, off });
    }
    Ok(t as usize)
}

/// Số biến bắt **được dùng** (max `LoadCaptured` + 1) trong code của một hàm.
pub fn captured_used(code: &[Instr]) -> u16 {
    let mut m = 0u16;
    for i in code {
        if let Instr::LoadCaptured(s) = i {
            m = m.max(s.saturating_add(1));
        }
    }
    m
}

/// Xác minh **toàn bộ** module.
pub fn verify(m: &Module) -> Result<(), VerifyError> {
    if m.version != VERSION {
        return Err(VerifyError::BadVersion(m.version));
    }
    if m.funcs.is_empty() {
        return Err(VerifyError::BadEntry(m.entry));
    }
    if m.entry as usize >= m.funcs.len() {
        return Err(VerifyError::BadEntry(m.entry));
    }
    let nf = m.funcs.len() as u32;
    let nc = m.consts.len() as u32;
    // Bảng "biến bắt được dùng" của từng hàm (để kiểm `MakeClosure`).
    let used: Vec<u16> = m.funcs.iter().map(|f| captured_used(&f.code)).collect();
    for (fi, f) in m.funcs.iter().enumerate() {
        let func = fi as u32;
        if f.code.is_empty() {
            return Err(VerifyError::EmptyCode { func });
        }
        if f.code.len() != f.spans.len() {
            return Err(VerifyError::SpanMismatch {
                func,
                code: f.code.len(),
                spans: f.spans.len(),
            });
        }
        if f.arity > f.nparams || f.nlocals < f.nparams {
            return Err(VerifyError::BadArity {
                func,
                arity: f.arity,
                nparams: f.nparams,
                nlocals: f.nlocals,
            });
        }
        // Kết thúc bằng Return/Halt (không rơi khỏi cuối).
        if !matches!(f.code.last(), Some(Instr::Return) | Some(Instr::Halt)) {
            return Err(VerifyError::FallsOffEnd {
                func,
                pc: f.code.len() - 1,
            });
        }
        for (pc, ins) in f.code.iter().enumerate() {
            match *ins {
                Instr::Const(i)
                | Instr::TestLit(i)
                | Instr::Field(i)
                | Instr::CallBuiltin(i, _) => {
                    if i >= nc {
                        return Err(VerifyError::BadConstIdx { func, pc, idx: i });
                    }
                }
                Instr::CallMethod(i, _)
                | Instr::MakeRecord(i, _)
                | Instr::MakeEnum(i, _)
                | Instr::TestRecord(i, _, _)
                | Instr::TestCtor(i, _) => {
                    if i >= nc {
                        return Err(VerifyError::BadConstIdx { func, pc, idx: i });
                    }
                }
                Instr::TestRange(a, b, _) => {
                    if a >= nc {
                        return Err(VerifyError::BadConstIdx { func, pc, idx: a });
                    }
                    if b >= nc {
                        return Err(VerifyError::BadConstIdx { func, pc, idx: b });
                    }
                }
                Instr::Call(t, _) => {
                    if t >= nf {
                        return Err(VerifyError::BadFuncIdx { func, pc, idx: t });
                    }
                }
                Instr::MakeClosure(t, nfree) => {
                    if t >= nf {
                        return Err(VerifyError::BadFuncIdx { func, pc, idx: t });
                    }
                    if nfree < used[t as usize] {
                        return Err(VerifyError::ClosureFree {
                            func,
                            pc,
                            nfree,
                            used: used[t as usize],
                        });
                    }
                }
                Instr::LoadLocal(s)
                | Instr::StoreLocal(s)
                | Instr::Assign(s)
                | Instr::BindLocal(s) => {
                    if s >= f.nlocals {
                        return Err(VerifyError::LocalOverflow {
                            func,
                            pc,
                            slot: s,
                            nlocals: f.nlocals,
                        });
                    }
                }
                Instr::LoadCaptured(s) => {
                    if s >= f.nparams.max(f.nlocals) {
                        // Biến bắt nằm ngoài khung: kiểm qua `MakeClosure` của nơi tạo.
                    }
                }
                Instr::Jump(off)
                | Instr::JumpIfFalse(off)
                | Instr::JumpIfNotTrue(off)
                | Instr::JumpIfNone(off)
                | Instr::JumpIfSome(off)
                | Instr::IterNext(off) => {
                    dich(pc, off, f.code.len(), func)?;
                }
                _ => {}
            }
        }
        // Độ sâu ngăn xếp: mô phỏng **độc lập** rồi so với khai báo.
        let need = need_stack(&f.code, func)?;
        if f.max_stack < need {
            return Err(VerifyError::NeedStack {
                func,
                declared: f.max_stack,
                need,
            });
        }
    }
    Ok(())
}

/// Giải mã **rồi** xác minh — đường nạp duy nhất cho cache (ST-23).
pub fn load_verified(bytes: &[u8]) -> Result<Module, LoadError> {
    let m = crate::decode::decode(bytes).map_err(LoadError::Decode)?;
    verify(&m).map_err(LoadError::Verify)?;
    Ok(m)
}

/// `true` nếu `op` là toán tử **đoản mạch** (không đi qua bảng ngữ nghĩa).
pub fn la_doan_mach(op: BinOp) -> bool {
    matches!(op, BinOp::And | BinOp::Or | BinOp::Coalesce)
}

/// `true` nếu `op` là toán tử một ngôi hợp lệ.
pub fn hop_le_un(op: UnOp) -> bool {
    matches!(op, UnOp::Neg | UnOp::Not)
}
