// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-408c** — test host cho **đồng hồ tick ảo** [`crate::machine::Machine::set_now`].
//!
//! Vì sao có tệp này: `KT-408b` §6.4 đo được `t_ns = 0` trên **mọi** bản ghi trace của ảnh
//! riscv64 — ring 32 B có thật trong `.corespace` nhưng **mất trục thời gian** vì `Machine::now`
//! chỉ được khởi tạo `0` và không nơi nào tiến. Nền tảng cấp tick thật (`rdtime` quy đổi ns) qua
//! `core_bridge::with_machine`; ba ca dưới khoá đúng ba bất biến của đường đó, **chạy host**
//! (không QEMU, D2-72 Q12):
//!
//! 1. [`kt408c_set_now_don_dieu`] — `now` **không lùi**: nguồn hỏng (giá trị nhỏ hơn) không làm
//!    trace nhảy lùi; giá trị bằng thì không đổi (idempotent).
//! 2. [`kt408c_trace_t_ns_tang_that`] — `t_ns` trong bản ghi 32 B **tăng thật** theo `now`, đúng
//!    offset `t_ns@0` của `spec/abi.toml [trace]` (không còn `[0..0]`).
//! 3. [`kt408c_set_now_xu_ly_bien_nap_lai`] — khi đồng hồ tiến qua biên `period`, `SchedContext`
//!    sống được **nạp lại** (`ScBudget::refill_due`) nên `check_budget` vẫn xanh. Thiếu bước này,
//!    một đồng hồ thật làm `refill <= now` ⇒ `check_budget` ĐỎ dù không có lỗi nào (đỏ-trước của
//!    chính phép kiểm: bỏ vòng `refill_due` trong `set_now` là ca này đỏ).
#![cfg(test)]

extern crate std;

use crate::error::{ObjId, ObjType, SlotRef};
use crate::machine::Machine;
use crate::observer::TestObserver;
use crate::store::StaticStore;
use crate::trace::TraceKind;

type M = Machine<StaticStore, TestObserver>;

/// Cảnh tối thiểu: CNode 16 ô + Untyped gốc + `SchedControl` (ô 7) + `SchedContext` (ô 8) đã
/// `Configure(4, 8)`. Cùng khuôn `kt345b_tests::rig` (arena 4 KiB) nhưng **không** cần TCB:
/// bất biến cần đo là của `ScBudget` + `check_budget`.
fn rig() -> (M, ObjId) {
    let mut m = M::new();
    let u = m.add_untyped(0x8200_0000, 64 * 1024);
    let c = m.add_cnode(16);
    let self_cap = m.root_cap(c);
    m.put_cap(c, 0, self_cap).expect("self cap");
    let u_cap = m.root_cap(u);
    m.put_cap(c, 1, u_cap).expect("cap untyped");
    let ctl = m
        .retype(SlotRef::new(c, 1), ObjType::SchedControl, 0, 0)
        .expect("retype SchedControl");
    m.put_cap(c, 7, ctl).expect("put ctl");
    let sc_cap = m
        .retype(SlotRef::new(c, 1), ObjType::SchedContext, 0, 0)
        .expect("retype SchedContext");
    m.put_cap(c, 8, sc_cap).expect("put sc");
    m.sched_control_configure(SlotRef::new(c, 7), SlotRef::new(c, 8), 4, 8)
        .expect("SC budget 4/8");
    (m, c)
}

/// Ảnh chụp `ScBudget` của ô SC (đọc qua `ScData::budget().info()` — `pub(crate)`).
fn budget_of(m: &M, c: ObjId) -> crate::mcs::ScInfo {
    let obj = m.cap_obj_at(c, 8).expect("cap ô 8 → SC obj");
    m.sc_data(obj).expect("ScData").budget.info()
}

/// **KT-408c** — `set_now` **đơn điệu**: tăng thì nhận, bằng thì giữ, nhỏ hơn thì **không lùi**.
#[test]
fn kt408c_set_now_don_dieu() {
    let mut m = M::new();
    assert_eq!(m.now_ticks(), 0, "model mới: tick ảo = 0");
    assert_eq!(m.set_now(1_000), 1_000, "lần đầu: nhận giá trị nền tảng");
    assert_eq!(m.now_ticks(), 1_000);
    assert_eq!(m.set_now(1_000), 1_000, "bằng nhau: không đổi (idempotent)");
    // Nguồn hỏng/đọc cũ trả số nhỏ hơn ⇒ **không** lùi (trace không được nhảy lùi thời gian).
    assert_eq!(m.set_now(999), 1_000, "nhỏ hơn: giữ giá trị cũ");
    assert_eq!(m.now_ticks(), 1_000);
    assert_eq!(m.set_now(u64::MAX), u64::MAX, "biên trên vẫn nhận");
}

/// **KT-408c** — `t_ns` của bản ghi trace **tăng thật** theo `now` (hết `[0..0]`).
#[test]
fn kt408c_trace_t_ns_tang_that() {
    crate::trace::reset_all();
    let mut m = M::new();
    m.set_now(5_000);
    m.trace_push(TraceKind::Run, 1, 0, 0);
    m.set_now(15_000);
    m.trace_push(TraceKind::Sched, 2, 0, 0);
    assert_eq!(crate::trace::len_hart(0), 2, "hart 0 có 2 bản ghi");
    let mut buf = [0u8; 2 * crate::trace::TRACE_RECORD_BYTES];
    let n = crate::trace::dump_hart(0, &mut buf);
    assert_eq!(n, 2 * crate::trace::TRACE_RECORD_BYTES);
    let first = u64::from_le_bytes(buf[0..8].try_into().unwrap());
    let second = u64::from_le_bytes(buf[32..40].try_into().unwrap());
    assert_eq!(first, 5_000, "t_ns@0 = tick lúc ghi bản ghi 1");
    assert_eq!(second, 15_000, "t_ns@0 = tick lúc ghi bản ghi 2");
    assert!(
        second > first,
        "t_ns phải TĂNG thật giữa hai bản ghi (KT-408b §6.4: trước là [0..0])"
    );
    crate::trace::reset_all();
}

/// **KT-408c** — đồng hồ tiến qua biên `period` ⇒ `SchedContext` sống được nạp lại, `check_budget`
/// vẫn **xanh**. Đây là ca **có răng**: bỏ vòng `refill_due` trong `set_now` là ca này ĐỎ.
#[test]
fn kt408c_set_now_xu_ly_bien_nap_lai() {
    let (mut m, c) = rig();
    assert!(
        m.check_budget().is_ok(),
        "SC vừa Configure: bất biến ngân sách xanh"
    );
    // `Configure(4, 8)` lúc `now = 0` ⇒ `refill = 8`. Đồng hồ thật nhảy qua nhiều biên.
    m.set_now(1_000);
    let info = budget_of(&m, c);
    assert!(
        info.refill > 1_000,
        "refill ({}) phải ở TƯƠNG LAI sau khi đồng hồ tiến tới 1000",
        info.refill
    );
    assert!(
        m.check_budget().is_ok(),
        "check_budget phải xanh sau khi đồng hồ tiến (biên nạp lại đã xử lý)"
    );
    // Tiến thêm lần nữa (qua biên kế) — vẫn xanh, `refill` vẫn ở tương lai.
    m.set_now(2_500);
    let info = budget_of(&m, c);
    assert!(info.refill > 2_500, "refill phải tiếp tục ở tương lai");
    assert!(m.check_budget().is_ok());
}
