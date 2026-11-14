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
    // **SỬA KÈM LÝ DO (ST-58)**: `list.concat` nay **có kiểu** (`list[T]`) ⇒ không còn là nguồn
    // "kiểu tĩnh không rõ". Dùng **tham số generic** `T` (bị **xoá kiểu** ở VS0 ⇒ `{error}`) trong
    // một biến **`var`**: tyck **không** chặn (Error), **lúc chạy** hai đường phải cho **cùng**
    // `(mã, dòng, cột, thông điệp)`.
    let d = tam("khong-ro");
    let src = "fn gan[T](x: T) -> int:\n    var y = x\n    y.f = 3\n    return 0\n\nfn main():\n    print(gan(1))\n";
    let (c, _, e) = hai_duong(&d, "khong-ro", src);
    assert_eq!(c, 1, "phải là lỗi **lúc chạy**: {e}");
    assert!(e.contains("VS0501"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn codegen_loi_vm_phai_exit_khac_0() {
    // VD-97 (5): codegen không biểu diễn được ⇒ **exit ≠ 0** + chẩn đoán stderr (không im lặng).
    // **SỬA KÈM LÝ DO (ST-60)**: `await` **đã** hạ thật (`$await`, VD-95) ⇒ ca này dùng khối
    // `cmd { … }` (vẫn `Unsupported("cmd-block")` ở codegen, `check` vẫn sạch).
    let d = tam("codegen");
    let src = "fn main():\n    let x = cmd { ls . }\n    print(x)\n";
    let p = ghi(&d, "codegen", src);
    // `check` **sạch** (`cmd { … }` là lỗi **động** ở VS0) ⇒ không đường nào bị tyck chặn trước.
    let (cc, _, _) = check(&p);
    assert_eq!(cc, 0, "`check` phải sạch (lỗi động)");
    let (c, o, e) = chay(&p, true);
    assert_ne!(c, 0, "codegen lỗi **không** được exit 0; stdout: {o:?}");
    assert!(
        e.contains("codegen lỗi"),
        "stderr phải nêu lỗi codegen: {e}"
    );
    assert!(e.contains("cmd-block"), "stderr phải nêu biến thể: {e}");
    // Interp: cùng chương trình ⇒ **lỗi động** `VS0501`, cũng exit ≠ 0.
    let (ci, _, ei) = chay(&p, false);
    assert_eq!(ci, 1, "interp: {ei}");
    assert!(ei.contains("VS0501"), "interp stderr: {ei}");
    let _ = std::fs::remove_dir_all(&d);
}

// ---------------------------------------------------------------------------
// **ST-60b** — hồi quy **closure bắt biến theo giá trị** (từ `dd04afb`) + đích lồng + `std --tên`.
// ---------------------------------------------------------------------------

/// **C1 (BLOCKER)**: closure **bắt** `var xs` rồi `xs[0] = 9` — interp đổi **bản sao** (không đụng
/// biến ngoài); trước ST-60b `--vm` hỏng (`gán chỉ số không hợp lệ`) vì codegen dùng `LoadLocal`
/// với **chỉ số captured**.
#[test]
fn closure_gan_phan_tu_bien_bat_hai_duong() {
    let d = tam("cl-c1");
    let src =
        "fn main():\n    var xs = [1, 2]\n    let f = fn(): xs[0] = 9\n    f()\n    print(xs)\n";
    let (c, o, e) = hai_duong(&d, "c1", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[1, 2]\n", "bắt **theo giá trị** ⇒ biến ngoài không đổi");
    let _ = std::fs::remove_dir_all(&d);
}

/// **C2**: closure **có tham số** gán biến bắt — `()` (không phải giá trị vừa gán), ngoài vẫn `0`;
/// giữ **byte-khớp** với `--vm` (trước: `()` ↔ chuỗi rỗng).
#[test]
fn closure_gan_bien_bat_co_tham_so_hai_duong() {
    let d = tam("cl-c2");
    let src = "fn main():\n    var x = 0\n    let f = fn(a: int): x = 5\n    let r = f(7)\n    print(r)\n    print(x)\n";
    let (c, o, e) = hai_duong(&d, "c2", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o, "()\n0\n",
        "phép gán cho `()`; tham số không bị ghi đè; ngoài không đổi"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Gán **trường** của biến bắt (`p.x = 5`) + `push` — bản sao trong closure, ngoài không đổi.
#[test]
fn closure_gan_truong_bien_bat_hai_duong() {
    let d = tam("cl-truong");
    let src = "type P:\n    x: int\n\nfn main():\n    var p: P = {x: 1}\n    let f = fn():\n        p.x = 5\n        print(p.x)\n    f()\n    print(p.x)\n";
    let (c, o, e) = hai_duong(&d, "truong", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "5\n1\n", "trong closure thấy 5, ngoài vẫn 1");
    let _ = std::fs::remove_dir_all(&d);
}

/// Closure **lồng 2 tầng**: tầng trong đọc biến bắt của tầng ngoài; gán ở tầng trong **không** đổi
/// biến ngoài cùng.
#[test]
fn closure_long_hai_tang_hai_duong() {
    let d = tam("cl-long");
    let src = "fn main():\n    var x = 1\n    let ngoai = fn():\n        let trong = fn():\n            x = 7\n            print(x)\n        trong()\n        print(x)\n    ngoai()\n    print(x)\n";
    let (c, o, e) = hai_duong(&d, "long", src);
    assert_eq!(c, 0, "stderr: {e}");
    // Bắt **theo giá trị** từng tầng: tầng trong ghi **bản sao của nó** ⇒ tầng ngoài vẫn `1`, ngoài
    // cùng vẫn `1` (không có chia sẻ ô giữa các tầng — đúng `VD-97`/thiết kế §5).
    assert_eq!(
        o, "7\n1\n1\n",
        "chỉ tầng trong thấy 7; hai tầng ngoài giữ 1"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-60b** — đích **lồng** khi gốc **đã** là `var`: chẩn đoán phải nói **lồng** (không báo oan
/// "let/tham số bất biến"); hai đường **cùng** chẩn đoán.
#[test]
fn gan_long_goc_var_bao_long_hai_duong() {
    let d = tam("long-var");
    let src = "fn main():\n    var xs = [1, 2]\n    xs[0][1] = 9\n    print(xs)\n";
    let (c, _, e) = hai_duong(&d, "longvar", src);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    assert!(
        e.contains("lồng"),
        "gốc `var` ⇒ phải nói **gán lồng**, không phải bất biến: {e}"
    );
    assert!(
        !e.contains("chỉ gán được phần tử của `var` — `let`/tham số"),
        "không được báo oan let/tham số: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-60b** — đích lồng với gốc **`let`** ⇒ nói đúng **gốc** (bất biến), không phải "lồng".
#[test]
fn gan_long_goc_let_bao_bat_bien_hai_duong() {
    let d = tam("long-let");
    let src = "type P:\n    xs: list[int]\n\ntype Q:\n    p: P\n\nfn main():\n    let q: Q = {p: {xs: [1, 2]}}\n    q.p.xs[0] = 9\n    print(q)\n";
    let (c, _, e) = hai_duong(&d, "longlet", src);
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    assert!(
        e.contains("`let`/tham số"),
        "gốc `let` ⇒ phải nói bất biến do `let`/tham số: {e}"
    );
    assert!(
        !e.contains("lồng"),
        "không được nói lồng khi gốc `let`: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-60b** — `std.<module>.<fn>(…, --tên)` **bị từ chối** `VS0401` (trước: bỏ cờ **im lặng**).
#[test]
fn std_khong_nhan_tham_so_ten() {
    let d = tam("std-ten");
    let src = "use std.list\nprint(list.sort_by([3, 1, 2], x => 0 - x, --desc))\n";
    let (c, o, e) = hai_duong(&d, "stdten", src);
    assert_eq!(c, 2, "stdout: {o:?} stderr: {e}");
    assert!(e.contains("VS0401"), "stderr: {e}");
    assert!(
        e.contains("--desc"),
        "thông điệp phải nêu **tên** không nhận: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-60b** — `.vish`: `let out = ls -l` là **shell-let** (định danh `l` **chưa** bind), **không**
/// phải khai báo `ls - l` (trước ST-60b: `check` xanh nhưng `run` báo `tên l chưa hỗ trợ`).
#[test]
fn vish_shell_let_khong_thanh_khai_bao() {
    let d = tam("shell-let");
    let p = d.join("sh.vish");
    std::fs::write(
        &p,
        "# SPDX-License-Identifier: MIT OR Apache-2.0\nlet out = ls -l\nprint(out)\n",
    )
    .expect("ghi");
    let (cc, co, _ce) = check(&p);
    assert_eq!(cc, 0, "`check` phải xanh (shell-let là stage): {co}");
    let (c, _, e) = chay(&p, false);
    assert_eq!(c, 1, "stderr: {e}");
    assert!(
        e.contains("`out`") && !e.contains("`l`"),
        "phải báo `out` chưa bind (shell-let chưa hạ — nợ cũ), không phải `l`: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}
