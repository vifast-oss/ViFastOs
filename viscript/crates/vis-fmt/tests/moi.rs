// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-77 (VD-108)** — `vis fmt` cho cú pháp mới (vòng 11): `cap[...]`, `async`/`spawn`/`await`,
//! `chan`, `use pkg.*`, `derives`/`trait`/`impl`. Mỗi mẫu kiểm **snapshot** + **idempotent**, và
//! một test **lossless token** trên fixture mới. `.vish` (chế độ lệnh) do
//! `crates/vis-cli/tests/fmt_moi.rs` kiểm qua binary (CmdCST không phát INDENT/DEDENT).

use std::path::{Path, PathBuf};
use vis_fmt::format;
use vis_syntax::{lex, Kind, Mode};

fn fmt_ok(src: &str) -> String {
    format(src).unwrap_or_else(|e| panic!("phải format được:\n{src}\n— lỗi: {e:?}"))
}

fn kiem(src: &str, muon: &str) {
    let ra = fmt_ok(src);
    assert_eq!(ra, muon, "snapshot lệch");
    assert_eq!(fmt_ok(&ra), ra, "phải idempotent");
}

#[test]
fn cap_nhieu_tham_so() {
    kiem(
        "use std.fs\nfn main(d: cap[Dir] @ p\"./x\", n: cap[Net] @ \"localhost\", f: cap[File] @ p\"a.txt\"):\n  print(fs.exists(\"a\"))\n",
        "use std.fs\nfn main(d: cap[Dir] @ p\"./x\", n: cap[Net] @ \"localhost\", f: cap[File] @ p\"a.txt\"):\n    print(fs.exists(\"a\"))\n",
    );
}

#[test]
fn cap_co_write_va_comment() {
    kiem(
        "fn main(d: cap[Dir] @ p\"./x\" (write)):  # thư mục ghi\n  print(1)\n",
        "fn main(d: cap[Dir] @ p\"./x\" (write)):  # thư mục ghi\n    print(1)\n",
    );
}

#[test]
fn cap_net_host() {
    kiem(
        "fn main(n: cap[Net] @ \"example.test\"):\n  print(1)\n",
        "fn main(n: cap[Net] @ \"example.test\"):\n    print(1)\n",
    );
}

#[test]
fn async_spawn_await() {
    kiem(
        "async fn f(x: int) -> int:\n  return x\nfn main():\n  let t = spawn f(1)\n  print(await t)\n",
        "async fn f(x: int) -> int:\n    return x\nfn main():\n    let t = spawn f(1)\n    print(await t)\n",
    );
}

#[test]
fn async_va_impl_cung_tep() {
    // Cú pháp hiện tại: `async` chỉ trên `fn` cấp tệp (không lồng trong `impl` — nợ ST-60 §5);
    // mẫu này kiểm `async fn` + `impl` cùng tệp và thân `impl` được thụt đúng.
    kiem(
        "type P:\n  x: int\nasync fn f() -> int:\n  1\nimpl P:\n  fn mo(self) -> int:\n    self.x\n",
        "type P:\n    x: int\nasync fn f() -> int:\n    1\nimpl P:\n    fn mo(self) -> int:\n        self.x\n",
    );
}

#[test]
fn chan_co_comment() {
    kiem(
        "fn main():\n  let c = chan[int](1)  # kênh\n  c.send(1)\n  print(c.recv())\n  c.close()\n",
        "fn main():\n    let c = chan[int](1)  # kênh\n    c.send(1)\n    print(c.recv())\n    c.close()\n",
    );
}

#[test]
fn use_pkg_canh_std() {
    kiem(
        "use std.fs\nuse pkg.math.mod\nfn main():\n  print(math.add(1, 2))\n",
        "use std.fs\nuse pkg.math.mod\nfn main():\n    print(math.add(1, 2))\n",
    );
}

#[test]
fn derives_trait_impl() {
    kiem(
        "type P derives Show, Eq:\n  x: int\ntrait A:\n  fn mo(self) -> int\ntrait B: A:\n  fn hai(self) -> int\nimpl B for P:\n  fn hai(self) -> int:\n    self.x + 1\n",
        "type P derives Show, Eq:\n    x: int\ntrait A:\n    fn mo(self) -> int\ntrait B: A:\n    fn hai(self) -> int\nimpl B for P:\n    fn hai(self) -> int:\n        self.x + 1\n",
    );
}

/// **Lossless token**: trên mọi fixture mới hợp lệ, `fmt` không đổi token (kind + text).
#[test]
fn lossless_token_fixture_moi() {
    let goc = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/sem");
    let mut so = 0usize;
    for nhom in ["cap", "async", "kenh", "trait"] {
        let d = goc.join(nhom);
        let Ok(ds) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut files: Vec<PathBuf> = ds
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "vis").unwrap_or(false))
            .collect();
        files.sort();
        for p in files {
            // Bỏ ca âm cố ý (có sibling `.err`).
            if p.with_extension("err").exists() {
                continue;
            }
            let src = std::fs::read_to_string(&p).expect("đọc fixture");
            let ra = match format(&src) {
                Ok(v) => v,
                Err(e) => panic!("{}: không format được: {e:?}", p.display()),
            };
            let a = lex(&src, Mode::Script);
            let b = lex(&ra, Mode::Script);
            let ta: Vec<(Kind, &str)> =
                a.tokens.iter().map(|t| (t.kind, t.text.as_str())).collect();
            let tb: Vec<(Kind, &str)> =
                b.tokens.iter().map(|t| (t.kind, t.text.as_str())).collect();
            assert_eq!(ta, tb, "{}: token đổi sau fmt", p.display());
            assert_eq!(
                format(&ra).expect("format lại"),
                ra,
                "{}: không idempotent",
                p.display()
            );
            so += 1;
        }
    }
    assert!(
        so >= 30,
        "phải kiểm ≥30 tệp fixture mới (đo được 35), thật {so}"
    );
    let _ = Path::new("");
}
