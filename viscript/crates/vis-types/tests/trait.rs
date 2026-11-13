// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-29 — **trait + `derives`**: dispatch **tĩnh** ở tầng hạ (không bảng `impls`, không bump VBC1).
//!
//! Nguyên tắc (Agent 0 chốt): `x.m()` với `x` kiểu tĩnh `Record`/`Enum` ⇒ **hạ thẳng** `Call(impl_fn)`
//! (chèn `self` đầu); kiểu bên nhận **không tĩnh** ⇒ `VS0403`; "nội tại thắng" (list/str/map giữ nguyên).
//! `derives Eq/Show/Json` = **kiểm + đăng ký**, ngữ nghĩa dùng cái có sẵn.

use vis_syntax::{lex, parse, Mode};
use vis_types::{lower, tyck};

fn kiem(src: &str) -> (Vec<String>, Vec<u32>) {
    let p = format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}");
    let lexed = lex(&p, Mode::Script);
    let out = parse(&p, Mode::Script);
    assert!(
        out.diagnostics.is_empty(),
        "cú pháp phải sạch: {:?}",
        out.diagnostics
    );
    let h = lower(&out.cst, &lexed);
    assert!(h.error.is_none(), "lỗi cấp API: {:?}", h.error);
    let kq = tyck::check(&h.hir, &h.symbols, &h.types);
    // Chẩn đoán **hạ** (derives/impl/phương thức) + chẩn đoán **kiểm kiểu**.
    let mut ma: Vec<String> = h
        .diagnostics
        .iter()
        .map(|d| d.code.short.to_string())
        .collect();
    let mut dong: Vec<u32> = h.diagnostics.iter().map(|d| d.line).collect();
    ma.extend(kq.codes().iter().map(|s| s.to_string()));
    dong.extend(kq.diags.iter().map(|d| d.line));
    (ma, dong)
}

fn sach(src: &str) {
    let (ma, _) = kiem(src);
    assert!(
        ma.is_empty(),
        "phải sạch chẩn đoán, được {ma:?} cho:\n{src}"
    );
}

fn co(src: &str, mong: &str) -> Vec<String> {
    let (ma, _) = kiem(src);
    assert!(
        ma.iter().any(|m| m == mong),
        "phải có `{mong}`, được {ma:?} cho:\n{src}"
    );
    ma
}

/// HIR phải chứa lời gọi **trực tiếp** tới hàm impl (tên `Hop.show`), không còn `Field` gọi mờ.
fn ha_thanh_call_voi(src: &str, mong: &str) {
    let p = format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}");
    let lexed = lex(&p, Mode::Script);
    let out = parse(&p, Mode::Script);
    let h = lower(&out.cst, &lexed);
    let dbg = format!("{:?}", h.hir);
    assert!(
        dbg.contains(mong),
        "phải hạ thành call trực tiếp tới `{mong}`: {dbg}"
    );
    assert!(
        !dbg.contains("Call { callee: Field"),
        "không được còn gọi phương thức mờ: {dbg}"
    );
}

const TYPE_HOP: &str = "type Hop:\n    gia_tri: int\n";
const TRAIT_SHOW: &str = "trait Show:\n    fn show(self) -> str\n";

// ---------------------------------------------------------------------------
// 1) derives: hợp lệ / không hợp lệ
// ---------------------------------------------------------------------------

#[test]
fn derives_eq_show_json_tren_record() {
    sach("type Hop derives Eq, Show, Json:\n    gia_tri: int\n\nlet h: Hop = {gia_tri: 1}\nprint(h)\n");
}

#[test]
fn derives_tren_enum() {
    sach("enum Or derives Eq, Show:\n    A\n    B\n\nprint(Or.A)\n");
}

#[test]
fn derives_la_bao_vs0403() {
    co("type Hop derives La:\n    gia_tri: int\n", "VS0403");
}

#[test]
fn derives_json_tren_truong_ham_bao_vs0403() {
    co("type F derives Json:\n    ham: fn(int) -> int\n", "VS0403");
}

#[test]
fn derives_json_hop_le_voi_list_int() {
    sach("type F derives Json:\n    xs: list[int]\n");
}

// ---------------------------------------------------------------------------
// 2) trait / impl: khớp chữ ký
// ---------------------------------------------------------------------------

#[test]
fn trait_khai_bao_sach() {
    sach(TRAIT_SHOW);
}

#[test]
fn impl_dung_chu_ky_sach() {
    sach(&format!(
        "{TRAIT_SHOW}\n{TYPE_HOP}\nimpl Show for Hop:\n    fn show(self) -> str:\n        \"hop\"\n"
    ));
}

#[test]
fn impl_thieu_phuong_thuc_bao_vs0403() {
    co(&format!("{TRAIT_SHOW}\n{TYPE_HOP}\nimpl Show for Hop:\n    fn khac(self) -> str:\n        \"x\"\n"), "VS0403");
}

#[test]
fn impl_thua_phuong_thuc_bao_vs0403() {
    co(
        &format!(
            "{TRAIT_SHOW}\n{TYPE_HOP}\nimpl Show for Hop:\n    fn show(self) -> str:\n        \"hop\"\n    fn them(self) -> int:\n        1\n"
        ),
        "VS0403",
    );
}

#[test]
fn impl_sai_kieu_tra_bao_vs0412() {
    co(
        &format!(
            "{TRAIT_SHOW}\n{TYPE_HOP}\nimpl Show for Hop:\n    fn show(self) -> int:\n        1\n"
        ),
        "VS0412",
    );
}

#[test]
fn impl_trait_khong_ton_tai_bao_vs0403() {
    co(
        &format!("{TYPE_HOP}\nimpl KhongCo for Hop:\n    fn show(self) -> str:\n        \"x\"\n"),
        "VS0403",
    );
}

#[test]
fn impl_inherent_khong_trait_sach() {
    sach(&format!(
        "{TYPE_HOP}\nimpl Hop:\n    fn gap(self) -> int:\n        self.gia_tri\n"
    ));
}

// ---------------------------------------------------------------------------
// 3) Dispatch tĩnh: `x.m()` với kiểu tĩnh Record/Enum
// ---------------------------------------------------------------------------

fn ha_thanh_call(src: &str) {
    ha_thanh_call_voi(src, "Hop.show");
}

#[test]
fn goi_phuong_thuc_impl_thanh_call_truc_tiep() {
    ha_thanh_call(&format!(
        "{TRAIT_SHOW}\n{TYPE_HOP}\nimpl Show for Hop:\n    fn show(self) -> str:\n        \"hop\"\n\nlet h: Hop = {{gia_tri: 1}}\nprint(h.show())\n"
    ));
}

#[test]
fn goi_phuong_thuc_inherent_thanh_call_truc_tiep() {
    ha_thanh_call_voi(
        &format!(
            "{TYPE_HOP}\nimpl Hop:\n    fn gap(self) -> int:\n        self.gia_tri\n\nlet h: Hop = {{gia_tri: 7}}\nprint(h.gap())\n"
        ),
        "Hop.gap",
    );
}

#[test]
fn tham_so_co_kieu_goi_duoc_phuong_thuc() {
    sach(&format!(
        "{TYPE_HOP}\nimpl Hop:\n    fn gap(self) -> int:\n        self.gia_tri\n\nfn f(h: Hop) -> int:\n    h.gap()\n\nprint(f({{gia_tri: 1}}))\n"
    ));
}

#[test]
fn nhieu_dong_truoc_voi_let() {
    sach(&format!(
        "{TYPE_HOP}\nimpl Hop:\n    fn gap(self) -> int:\n        self.gia_tri\n\nlet a: Hop = {{gia_tri: 1}}\nlet b: Hop = {{gia_tri: 2}}\nprint(a.gap())\nprint(b.gap())\n"
    ));
}

#[test]
fn enum_goi_phuong_thuc_duoc() {
    sach("enum Or:\n    A\n    B\n\nimpl Or:\n    fn la_a(self) -> bool:\n        true\n\nlet o: Or = Or.A\nprint(o.la_a())\n");
}

#[test]
fn no_i_tai_thang_impl_cung_ten() {
    // `list.len()` vẫn là **nội tại** dù có `impl` khác kiểu — không mơ hồ, không đổi hành vi.
    // (Không đặt tên phương thức impl trùng `len` của bảng dựng sẵn: đó là `VS0302` khác phạm vi.)
    sach("type L:\n    x: int\n\nimpl L:\n    fn gap(self) -> int:\n        1\n\nprint([1, 2].len())\n");
}

#[test]
fn phuong_thuc_khong_ton_tai_bao_vs0403() {
    co(
        &format!("{TYPE_HOP}\nlet h: Hop = {{gia_tri: 1}}\nprint(h.khong_co())\n"),
        "VS0403",
    );
}

#[test]
fn mo_ho_giua_hai_impl_bao_vs0403() {
    co(
        &format!(
            "{TRAIT_SHOW}\ntrait Eq2:\n    fn show(self) -> str\n\n{TYPE_HOP}\nimpl Show for Hop:\n    fn show(self) -> str:\n        \"a\"\nimpl Eq2 for Hop:\n    fn show(self) -> str:\n        \"b\"\n\nlet h: Hop = {{gia_tri: 1}}\nprint(h.show())\n"
        ),
        "VS0403",
    );
}

#[test]
fn kieu_khong_tinh_bao_vs0403() {
    // `xs.get(0)` trả `{error}`/generic chưa biết ⇒ không phân giải được phương thức impl.
    co(
        &format!(
            "{TYPE_HOP}\nimpl Hop:\n    fn gap(self) -> int:\n        self.gia_tri\n\nfn f[T](xs: list[T]) -> int:\n    xs.get(0).gap()\n"
        ),
        "VS0403",
    );
}

#[test]
fn khong_co_phuong_thuc_giu_nguyen_loi_cu() {
    // Kiểu không phải record/enum (`int`) vẫn báo lỗi như trước (không đổi ngữ nghĩa cũ).
    co("let x: int = 1\nprint(x.khong_co())\n", "VS0403");
}

// ---------------------------------------------------------------------------
// 4) ST-57 — siêu trait (một tầng) + `derives` **sinh phương thức thật**
// ---------------------------------------------------------------------------

const TRAIT_A: &str = "trait A:\n    fn mo(self) -> int\n";
const TRAIT_B: &str = "trait B: A:\n    fn hai(self) -> int\n";
const IMPL_B: &str = "impl B for P:\n    fn hai(self) -> int:\n        self.x\n";

#[test]
fn sieu_trait_du_sach() {
    // **ST-57 (VD-98 (2))**: `impl B` khai **trước** `impl A` vẫn xanh (kiểm **sau** vòng 1b).
    sach(&format!(
        "{TRAIT_A}\n{TRAIT_B}\ntype P:\n    x: int\n\n{IMPL_B}impl A for P:\n    fn mo(self) -> int:\n        self.x\n"
    ));
}

#[test]
fn sieu_trait_thieu_bao_vs0403() {
    co(
        &format!("{TRAIT_A}\n{TRAIT_B}\ntype P:\n    x: int\n\n{IMPL_B}"),
        "VS0403",
    );
}

#[test]
fn derives_show_sinh_phuong_thuc_that() {
    ha_thanh_call_voi(
        "type Hop derives Show:\n    gia_tri: int\n\nlet h: Hop = {gia_tri: 1}\nprint(h.show())\n",
        "Hop.show",
    );
}

#[test]
fn derives_eq_sinh_phuong_thuc_that() {
    ha_thanh_call_voi(
        "type Hop derives Eq:\n    gia_tri: int\n\nlet a: Hop = {gia_tri: 1}\nlet b: Hop = {gia_tri: 1}\nprint(a.eq(b))\n",
        "Hop.eq",
    );
}

#[test]
fn derives_eq_truong_float_bao_vs0403() {
    co("type F derives Eq:\n    x: float\n", "VS0403");
}

#[test]
fn derives_show_tren_kieu_generic_bao_vs0403() {
    co("type Hop[T] derives Show:\n    gia_tri: T\n", "VS0403");
}

#[test]
fn derives_enum_eq_sinh_phuong_thuc_that() {
    ha_thanh_call_voi(
        "enum Or derives Eq:\n    A\n    B\n\nlet a: Or = Or.A\nprint(a.eq(Or.A))\n",
        "Or.eq",
    );
}
