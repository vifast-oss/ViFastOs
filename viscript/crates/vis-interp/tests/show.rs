// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Snapshot `Show`** (VD-47 #2/#3) — khoá **đúng chuỗi** của `str(x)` cho mọi dạng giá trị.
//!
//! Đây là test **khoá** (không phải đỏ-trước): Agent 0 chốt các dạng ở VD-47; đổi bất kỳ dòng nào
//! dưới đây là **thay đổi ngữ nghĩa** ⇒ phải có VD mới.

use vis_interp::Value;

fn f(v: Value) -> String {
    v.show()
}

#[test]
fn snapshot_show_scalar() {
    assert_eq!(f(Value::Int(-7)), "-7");
    assert_eq!(f(Value::Int(i64::MIN)), "-9223372036854775808");
    assert_eq!(f(Value::Float(1.5)), "1.5");
    assert_eq!(f(Value::Float(1.0)), "1");
    assert_eq!(f(Value::Float(f64::NAN)), "NaN");
    assert_eq!(f(Value::Bool(true)), "true");
    assert_eq!(f(Value::Char('ố')), "'ố'");
    assert_eq!(f(Value::Str("héllo".into())), "héllo");
    assert_eq!(f(Value::Path("./a.txt".into())), "./a.txt");
    assert_eq!(f(Value::Bytes(vec![b'a', 0x00, 0xff])), "b\"a\\x00\\xff\"");
    assert_eq!(f(Value::Unit), "()");
}

/// `size`: **đơn vị lớn nhất chia hết**, không chia hết ⇒ `b` (VD-47 #1).
#[test]
fn snapshot_show_size() {
    assert_eq!(f(Value::Size(0)), "0b");
    assert_eq!(f(Value::Size(5000)), "5kb");
    assert_eq!(f(Value::Size(5_000_000)), "5mb");
    assert_eq!(f(Value::Size(1024)), "1kib");
    assert_eq!(f(Value::Size(1500)), "1500b");
}

/// `duration`: **đơn vị lớn nhất chia hết**, không chia hết ⇒ `ns` (VD-47 #1).
#[test]
fn snapshot_show_duration() {
    assert_eq!(f(Value::Duration(0)), "0ns");
    assert_eq!(f(Value::Duration(1_000_000_000)), "1s");
    assert_eq!(f(Value::Duration(250_000_000)), "250ms");
    assert_eq!(f(Value::Duration(1_500)), "1500ns");
}

/// `percent`: `ratio*100` + `pct`, bỏ `.0` (VD-47 #2).
#[test]
fn snapshot_show_percent() {
    assert_eq!(f(Value::Percent(0.5)), "50pct");
    assert_eq!(f(Value::Percent(0.125)), "12.5pct");
    assert_eq!(f(Value::Percent(1.0)), "100pct");
}

/// Tập hợp + kiểu người dùng (VD-47 #3).
#[test]
fn snapshot_show_hop_va_kieu_nguoi_dung() {
    assert_eq!(f(Value::List(vec![Value::Int(1), Value::Int(2)])), "[1, 2]");
    assert_eq!(
        f(Value::Map(vec![
            (Value::Str("a".into()), Value::Int(1)),
            (Value::Str("b".into()), Value::Int(2)),
        ])),
        "{\"a\": 1, \"b\": 2}"
    );
    assert_eq!(
        f(Value::Record(
            "User".into(),
            vec![
                ("name".into(), Value::Str("An".into())),
                ("age".into(), Value::Int(20)),
            ]
        )),
        "User(name: An, age: 20)"
    );
    assert_eq!(
        f(Value::Enum(
            "Shape".into(),
            "Circle".into(),
            vec![Value::Float(2.0)]
        )),
        "Circle(2)"
    );
    assert_eq!(
        f(Value::Enum("Shape".into(), "Empty".into(), vec![])),
        "Empty"
    );
    assert_eq!(f(Value::Opt(None)), "none");
    assert_eq!(f(Value::Opt(Some(Box::new(Value::Int(5))))), "some(5)");
    assert_eq!(f(Value::Res(Ok(Box::new(Value::Int(1))))), "Ok(1)");
    assert_eq!(
        f(Value::Res(Err(Box::new(Value::Str("âm".into()))))),
        "Err(âm)"
    );
}
