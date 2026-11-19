#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# KT-187 / L25 — kiểm ngữ nghĩa rustc độc lập cho ba câu hỏi của §5/§6:
#   (a) `include!` trong thân `macro_rules!` giải theo **tệp định nghĩa** hay **tệp gọi**?
#   (b) `#[path]` trong thân `macro_rules!` giải theo tệp nào?
#   (c) `#[cfg_attr(not(test), path = "…")]` có nạp tệp đích ở bản không-test không?
#   (d) ba dạng viết cách khác (`include ! (…)`, `#[ path = … ]`, `#[\npath = …]`) có hợp lệ?
#
# Chạy: bash v2/agent/JOURNAL/kt187-logs/t8-rustc.sh
# Ghi:  rustc-semantics.log
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
BASE="${TMPDIR:-/tmp}/kt187-l25"
export PATH="$HOME/.cargo/bin:$PATH"
R=$BASE/rustc
rm -rf "$R"
mkdir -p "$R"
{
echo "# KT-187 / L25 — ngữ nghĩa rustc (rustc $(rustc --version))"
echo

# (a) include! trong macro: định nghĩa a/m.rs, gọi b/main.rs
mkdir -p "$R/a" "$R/b"
cat > "$R/a/m.rs" <<'EOF'
#[macro_export]
macro_rules! inc_evil {
    () => { include!("evil.rs"); };
}
EOF
printf 'pub fn from_a() -> u32 { 1 }\n' > "$R/a/evil.rs"
printf 'pub fn from_b() -> u32 { 2 }\n' > "$R/b/evil.rs"
cat > "$R/b/main.rs" <<'EOF'
#[macro_use]
#[path = "../a/m.rs"]
mod m;
inc_evil!();
fn main() { println!("{}", from_a()); }
EOF
cat > "$R/b/main_b.rs" <<'EOF'
#[macro_use]
#[path = "../a/m.rs"]
mod m;
inc_evil!();
fn main() { println!("{}", from_b()); }
EOF
echo "## (a) include! trong macro — macro định nghĩa ở a/m.rs, gọi ở b/main.rs"
echo "##     a/evil.rs = from_a()=1 ; b/evil.rs = from_b()=2"
echo "##     gọi from_a():"
rustc --edition 2021 "$R/b/main.rs" -o "$R/a.out" 2>&1 | grep -E '^error' | head -2
echo "##     gọi from_b():"
rustc --edition 2021 "$R/b/main_b.rs" -o "$R/b2.out" 2>&1 | grep -E '^error' | head -2
echo "##     in ra (1 = tệp ĐỊNH NGHĨA, 2 = tệp GỌI):"
"$R/b2.out" 2>/dev/null || echo "(không chạy được)"
echo

# (b) #[path] trong macro
mkdir -p "$R/c" "$R/d"
cat > "$R/c/m.rs" <<'EOF'
#[macro_export]
macro_rules! evil_mod {
    () => { #[path = "evil.rs"] mod x; };
}
EOF
printf 'pub fn danger() -> u32 { 0 }\n' > "$R/c/evil.rs"
printf 'pub fn danger() -> u32 { 7 }\n' > "$R/d/evil.rs"
cat > "$R/d/main.rs" <<'EOF'
#[macro_use]
#[path = "../c/m.rs"]
mod m;
evil_mod!();
fn main() { println!("{}", x::danger()); }
EOF
echo "## (b) #[path] trong macro — định nghĩa ở c/m.rs, gọi ở d/main.rs"
echo "##     c/evil.rs danger()=0 ; d/evil.rs danger()=7"
rustc --edition 2021 "$R/d/main.rs" -o "$R/b.out" 2>&1 | head -6
echo "## in ra (0 = tệp ĐỊNH NGHĨA, 7 = tệp GỌI):"
"$R/b.out" 2>/dev/null || echo "(không chạy được)"
echo

# (c) cfg_attr path
mkdir -p "$R/e"
printf 'pub fn danger() -> u32 { 42 }\n' > "$R/e/evil.rs"
printf 'pub fn clean() -> u32 { 0 }\n' > "$R/e/evil2.rs"
cat > "$R/e/main.rs" <<'EOF'
#[cfg(test)]
mod evil;
#[cfg_attr(not(test), path = "evil.rs")]
mod evil2;
fn main() { println!("{}", evil2::danger()); }
EOF
echo "## (c) #[cfg_attr(not(test), path = \"evil.rs\")] — bản không-test"
rustc --edition 2021 "$R/e/main.rs" -o "$R/c.out" 2>&1 | head -6
echo "## in ra (42 = nạp evil.rs ở bản sản phẩm):"
"$R/c.out" 2>/dev/null || echo "(không chạy được)"
echo

# (d) ba dạng viết cách khác
mkdir -p "$R/f"
printf 'pub fn danger() -> u32 { 7 }\n' > "$R/f/evil.rs"
cat > "$R/f/main.rs" <<'EOF'
#[ path = "evil.rs" ]
mod evil2;
fn main() { println!("{}", evil2::danger()); }
EOF
cat > "$R/f/main2.rs" <<'EOF'
include ! ("evil.rs");
fn main() { println!("{}", danger()); }
EOF
cat > "$R/f/main3.rs" <<'EOF'
#[
path = "evil.rs"
]
mod evil3;
fn main() { println!("{}", evil3::danger()); }
EOF
echo "## (d) ba dạng viết cách khác — rustc có chấp nhận không?"
for f in main main2 main3; do
  rustc --edition 2021 "$R/f/$f.rs" -o "$R/f/$f.out" 2>&1 | head -3
  echo "  $f: rc=$? out=$("$R/f/$f.out" 2>/dev/null || echo '(không chạy)')"
done
echo

# (e) #[path = concat!(...)] có hợp lệ không?
mkdir -p "$R/g"
printf 'pub fn danger() -> u32 { 7 }\n' > "$R/g/evil_x.rs"
cat > "$R/g/main.rs" <<'EOF'
#[path = concat!("evil", "_x.rs")]
mod evil_x;
fn main() { println!("{}", evil_x::danger()); }
EOF
echo "## (e) #[path = concat!(…)] — rustc có chấp nhận không?"
rustc --edition 2021 "$R/g/main.rs" -o "$R/g/out" > "$R/g/err.txt" 2>&1
rc=$?
head -4 "$R/g/err.txt"
echo "  rc=$rc"
echo

echo "## (d2) rustfmt --check trên ba dạng — fmt có chuẩn hoá không?"
for f in main main2 main3; do
  echo "  --- $f:"
  rustfmt --edition 2021 --check "$R/f/$f.rs" 2>&1 | grep -E '^[-+]#|^[-+]include|^[-+]#\[|^[-+]path|^[-+]\]' | head -6
done
} > "$HERE/rustc-semantics.log" 2>&1
echo "xong: $HERE/rustc-semantics.log"
