// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-56 (nợ lõi A.3)** — `&self` (receiver **không mượn thật**) + đóng nốt `Hop[Hop[int]]`.
//!
//! `&self`/`& self` trong `impl`/`trait` hạ **y như** `self` (VD-92): không mượn thật, **không** cho
//! gán trường (bản ghi bất biến ở VS0), `fn bump(self)` cũ **không đổi**. `Hop[Hop[int]]` **đã xanh
//! từ trước** — các biến thể (annotation, `list[…]`, tham số hàm, `impl Hop[Hop[int]]`, ba tầng) được
//! **khoá bằng ca**, không phải "giả vờ sửa". interp ↔ `--vm` **cùng byte** ở mọi ca chạy được.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-st56-self-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn ghi(d: &Path, ten: &str, src: &str) -> PathBuf {
    let p = d.join(format!("{ten}.vis"));
    std::fs::write(
        &p,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    p
}

fn chay(p: &Path, vm: bool) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.arg("run");
    if vm {
        c.arg("--vm").arg("--no-cache");
    }
    let o = c.arg(p).output().expect("chạy `vis run`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

fn hai_duong(d: &Path, ten: &str, src: &str) -> (i32, String, String) {
    let p = ghi(d, ten, src);
    let a = chay(&p, false);
    let b = chay(&p, true);
    assert_eq!(
        (a.0, &a.1, &a.2),
        (b.0, &b.1, &b.2),
        "interp ≠ `--vm` (exit/stdout/stderr)\n  interp: {a:?}\n  vm: {b:?}"
    );
    a
}

const P: &str = "type P:\n    n: int\n\nimpl P:\n    fn bump(&self) -> int:\n        self.n + 1\n\n    fn hai(& self) -> int:\n        self.n + 2\n";

#[test]
fn self_amp_va_self_khoang_trang_chay_hai_duong() {
    let d = tam("co-ban");
    let src = format!(
        "{P}\nfn main():\n    let p: P = {{n: 41}}\n    print(p.bump())\n    print(p.hai())\n"
    );
    let (c, o, e) = hai_duong(&d, "co-ban", &src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "42\n43\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn self_amp_trong_trait_va_impl() {
    // `&self` phải parse được **cả** chỗ khai `trait` lẫn `impl … for …`.
    let d = tam("trait");
    let src = "trait Hien:\n    fn show(&self) -> str\n\ntype Hop:\n    gia_tri: int\n\n\
               impl Hien for Hop:\n    fn show(&self) -> str:\n        \"hop\"\n\n\
               fn main():\n    let h: Hop = {gia_tri: 1}\n    print(h.show())\n";
    let (c, o, e) = hai_duong(&d, "trait", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "hop\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn self_thuong_van_nhu_cu() {
    let d = tam("self-cu");
    let src = "type P:\n    n: int\n\nimpl P:\n    fn bump(self) -> int:\n        self.n + 1\n\n\
               fn main():\n    let p: P = {n: 41}\n    print(p.bump())\n";
    let (c, o, e) = hai_duong(&d, "self-cu", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "42\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn self_amp_gan_ket_thuc_vao_receiver_that() {
    // Ca âm: `&self` phải **buộc** đúng receiver (không phải một `self` rỗng) ⇒ đọc trường lạ là
    // `VS0501` **giống hệt** đường `self`; hai đường **cùng byte** (mã + dòng:cột + thông điệp).
    let d = tam("thieu-truong");
    let src =
        "type P:\n    n: int\n\nimpl P:\n    fn doc(&self) -> int:\n        self.khong_co\n\n\
               fn main():\n    let p: P = {n: 1}\n    print(p.doc())\n";
    let (c, _, e) = hai_duong(&d, "thieu-truong", src);
    assert_eq!(c, 1, "phải là lỗi **lúc chạy**, stderr: {e}");
    assert!(e.contains("VS0501"), "stderr: {e}");
    assert!(e.contains("khong_co"), "nêu tên trường: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn self_amp_khong_nhan_chu_thich_kieu() {
    // `&self` là **receiver**, không phải tham số thường ⇒ `&self: int` vẫn là lỗi cú pháp.
    let d = tam("co-chu-thich");
    let src = "fn f(&self: int) -> int:\n    self\n\nfn main():\n    print(1)\n";
    let (c, _, e) = hai_duong(&d, "co-chu-thich", src);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn hop_long_bon_bien_the_chay_hai_duong() {
    // **Đã xanh từ trước ST-56** (probe 2026-11-14 tại HEAD `4f88ffa`) — ca này **khoá** hành vi,
    // không phải sửa: annotation, `list[…]`, tham số hàm, `impl Hop[Hop[int]]`, ba tầng.
    let d = tam("hop");
    let src = "type Hop[T]:\n    gia_tri: T\n\nimpl Hop[Hop[int]]:\n    fn sau(self) -> int:\n        self.gia_tri.gia_tri + 1\n\n\
               fn tong(xs: list[Hop[Hop[int]]]) -> int:\n    xs.len() + xs[0].gia_tri.gia_tri\n\n\
               fn main():\n    let h: Hop[Hop[int]] = {gia_tri: {gia_tri: 1}}\n    \
               let ds: list[Hop[Hop[int]]] = [{gia_tri: {gia_tri: 5}}]\n    \
               let ba: Hop[Hop[Hop[int]]] = {gia_tri: {gia_tri: {gia_tri: 9}}}\n    \
               print(h.gia_tri.gia_tri)\n    print(h.sau())\n    print(ds[0].gia_tri.gia_tri)\n    \
               print(tong(ds))\n    print(ba.gia_tri.gia_tri.gia_tri)\n";
    let (c, o, e) = hai_duong(&d, "hop", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "1\n2\n5\n6\n9\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn hop_long_sai_kieu_la_vs0412() {
    // Ca âm khoá hành vi: payload trong **sâu** sai kiểu ⇒ chẩn đoán **tĩnh** (không lọt xuống chạy).
    let d = tam("hop-sai");
    let src = "type Hop[T]:\n    gia_tri: T\n\nfn main():\n    let h: Hop[Hop[int]] = {gia_tri: {gia_tri: \"a\"}}\n    print(h.gia_tri.gia_tri)\n";
    let (c, _, e) = hai_duong(&d, "hop-sai", src);
    assert_eq!(c, 2, "lỗi biên dịch ⇒ exit 2, stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}
