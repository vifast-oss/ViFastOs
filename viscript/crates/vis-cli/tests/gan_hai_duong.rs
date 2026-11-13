// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-56b (VD-97)** — **một** ngữ nghĩa cho **phép gán**, hai đường **cùng byte**.
//!
//! - `self` (không `var`) là **tham số bất biến** ⇒ `self.x = v` là **`VS0412`** ở **cả hai**
//!   đường (tyck chặn trước codegen, span tại **phép gán**).
//! - `var self` / `var x`: `x.f = v` = **rebind cục bộ** `x = x with f: v` (giá trị; người gọi
//!   **không** thấy đổi), `x[i] = v` = sửa **list/map của `x`** rồi rebind (giá trị — không aliasing).
//! - Target **không** hỗ trợ (`f().x = 1`, gán ghép `x += v` trên trường/phần tử) ⇒ **cùng** chẩn
//!   đoán hai đường, **tyck chặn trước**.
//! - `vis run --vm` gặp lỗi **codegen** ⇒ **exit ≠ 0** + chẩn đoán stderr (trước: in rồi exit 0).

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-st56b-{tag}-{}", std::process::id()));
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

fn check(p: &Path) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("check")
        .arg(p)
        .output()
        .expect("chạy `vis check`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// Chạy **cả hai đường** trên **cùng một tệp**; đòi exit + stdout + stderr **byte-khớp**.
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

const P: &str = "type P:\n    x: int\n\n";

#[test]
fn self_bat_bien_la_vs0412_hai_duong() {
    // VD-97 (1): `self` (không `var`) là **tham số bất biến** ⇒ chặn ở **tyck**, span tại phép gán.
    let d = tam("self-bat-bien");
    let src = format!(
        "{P}impl P:\n    fn set(self) -> int:\n        self.x = 2\n        return self.x\n\n\
         fn main():\n    let p: P = {{x: 1}}\n    print(p.set())\n"
    );
    let p = ghi(&d, "self-bat-bien", &src);
    // `vis check` in TAP ra **stdout** (`not ok` + dòng `# VSxxxx d:c msg`).
    let (cc, co, ce) = check(&p);
    assert_eq!(cc, 1, "`check` phải đỏ: {co}{ce}");
    assert!(co.contains("VS0412"), "check stdout: {co}");
    // Tệp thử có thêm **1 dòng SPDX** ở đầu ⇒ phép gán `self.x = 2` ở **dòng 7, cột 9**.
    assert!(
        co.contains("7:9"),
        "span phải nằm ở **phép gán** (dòng 7, cột 9): {co}"
    );
    let a = chay(&p, false);
    let b = chay(&p, true);
    // Lỗi **biên dịch** ⇒ exit **2** cả hai đường, **cùng** chẩn đoán (không lệch chữ).
    assert_eq!((a.0, b.0), (2, 2), "interp: {a:?}\nvm: {b:?}");
    assert_eq!(a.2, b.2, "chẩn đoán hai đường phải **y hệt**");
    assert!(a.2.contains("VS0412"), "stderr: {}", a.2);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn var_self_gan_truong_hai_duong() {
    let d = tam("var-self");
    let src = format!(
        "{P}impl P:\n    fn bump(var self) -> int:\n        self.x = self.x + 1\n        return self.x\n\n\
         fn main():\n    let p: P = {{x: 41}}\n    print(p.bump())\n    print(p.x)\n"
    );
    let (c, o, e) = hai_duong(&d, "var-self", &src);
    assert_eq!(c, 0, "stderr: {e}");
    // `var self` rebind **cục bộ** ⇒ người gọi **không** thấy đổi (`p.x` vẫn 41).
    assert_eq!(o, "42\n41\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn var_truong_gan_hai_duong() {
    let d = tam("var-truong");
    let src = "type P:\n    x: int\n    ten: str\n\n\
               fn sua(p: P) -> int:\n    var q = p\n    q.x = 9\n    q.ten = \"sau\"\n    return q.x\n\n\
               fn main():\n    var a: P = {x: 1, ten: \"truoc\"}\n    print(sua(a))\n    print(a.x)\n    a.x = 7\n    print(a.x)\n";
    let (c, o, e) = hai_duong(&d, "var-truong", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "9\n1\n7\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn var_list_chi_so_va_cow_hai_duong() {
    let d = tam("var-list");
    let src = "fn main():\n    var xs = [1, 2, 3]\n    xs[1] = 9\n    print(xs)\n    \
               var ys = xs\n    ys[0] = 5\n    print(xs)\n    print(ys)\n";
    let (c, o, e) = hai_duong(&d, "var-list", src);
    assert_eq!(c, 0, "stderr: {e}");
    // Giá trị (không aliasing): `ys[0] = 5` **không** đổi `xs`.
    assert_eq!(o, "[1, 9, 3]\n[1, 9, 3]\n[5, 9, 3]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn var_map_khoa_hai_duong() {
    let d = tam("var-map");
    let src =
        "fn main():\n    var m = {\"a\": 1}\n    m[\"a\"] = 2\n    m[\"b\"] = 3\n    print(m)\n";
    let (c, o, e) = hai_duong(&d, "var-map", src);
    assert_eq!(c, 0, "stderr: {e}");
    // Khoá **chưa có** ⇒ thêm vào (giữ thứ tự), đúng ngữ nghĩa `Set` của VM.
    assert_eq!(o, "{\"a\": 2, \"b\": 3}\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn gan_ghep_tren_truong_bi_chan_truoc_hai_duong() {
    // VD-97 (2)/(3): gán ghép `+=` trên **trường** chưa hỗ trợ ⇒ **front end** chặn **trước**
    // (hiện là lỗi cú pháp `VS0201` — còn sớm hơn tyck); hai đường phải **cùng** chẩn đoán.
    let d = tam("gan-ghep");
    let src = format!("{P}fn main():\n    var p: P = {{x: 1}}\n    p.x += 1\n    print(p.x)\n");
    let p = ghi(&d, "gan-ghep", &src);
    let (cc, co, ce) = check(&p);
    assert_eq!(cc, 1, "`check` phải đỏ: {co}{ce}");
    assert!(
        co.contains("VS0201") || co.contains("VS0412"),
        "check stdout: {co}"
    );
    let a = chay(&p, false);
    let b = chay(&p, true);
    assert_eq!((a.0, b.0), (2, 2), "interp: {a:?}\nvm: {b:?}");
    assert_eq!(a.2, b.2, "chẩn đoán hai đường phải **y hệt**");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn dich_gan_khong_ho_tro_la_vs0412() {
    // `f().x = 1` — không phải biến `var` ⇒ tyck chặn **trước** (không để lệch hai đường).
    let d = tam("dich-la");
    let src =
        format!("{P}fn lay() -> P:\n    {{x: 1}}\n\nfn main():\n    lay().x = 2\n    print(1)\n");
    let p = ghi(&d, "dich-la", &src);
    let (cc, co, ce) = check(&p);
    assert_eq!(cc, 1, "`check` phải đỏ: {co}{ce}");
    assert!(co.contains("VS0412"), "check stdout: {co}");
    let a = chay(&p, false);
    let b = chay(&p, true);
    assert_eq!((a.0, b.0), (2, 2), "interp: {a:?}\nvm: {b:?}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn gan_truong_tren_kieu_khong_ro_cung_chan_doan_giong_nhau() {
    // Base có kiểu tĩnh **không rõ** (`Ty::Any` của `list.concat`) ⇒ tyck không chặn được;
    // **lúc chạy** hai đường phải cho **cùng** `(mã, dòng, cột, thông điệp)`.
    let d = tam("khong-ro");
    let src = "use std.list\n\nfn main():\n    var x = list.concat([1], [2])\n    x.f = 3\n    print(x)\n";
    let (c, _, e) = hai_duong(&d, "khong-ro", src);
    assert_eq!(c, 1, "phải là lỗi **lúc chạy**: {e}");
    assert!(e.contains("VS0501"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn codegen_loi_vm_phai_exit_khac_0() {
    // VD-97 (5): codegen không biểu diễn được ⇒ **exit ≠ 0** + chẩn đoán stderr (không im lặng).
    let d = tam("codegen");
    let src = "async fn f() -> int:\n    return 1\n\nfn main():\n    print(await f())\n";
    let p = ghi(&d, "codegen", src);
    // `check` **sạch** (async/await là lỗi **động** ở VS0) ⇒ không đường nào bị tyck chặn trước.
    let (cc, _, _) = check(&p);
    assert_eq!(cc, 0, "`check` phải sạch (lỗi động)");
    let (c, o, e) = chay(&p, true);
    assert_ne!(c, 0, "codegen lỗi **không** được exit 0; stdout: {o:?}");
    assert!(
        e.contains("codegen lỗi"),
        "stderr phải nêu lỗi codegen: {e}"
    );
    assert!(e.contains("await"), "stderr phải nêu biến thể: {e}");
    // Interp: cùng chương trình ⇒ **lỗi động** `VS0501`, cũng exit ≠ 0.
    let (ci, _, ei) = chay(&p, false);
    assert_eq!(ci, 1, "interp: {ei}");
    assert!(ei.contains("VS0501"), "interp stderr: {ei}");
    let _ = std::fs::remove_dir_all(&d);
}
