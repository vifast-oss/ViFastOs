#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-256 (3) — kiểm **máy**: bản build **sản phẩm** không có `--cfg test`.

Vì sao có tệp này
-----------------
Luật `wake` (KT-184/KT-194) dựa trên một **giả định chưa kiểm máy**: ảnh sản phẩm được biên dịch
**không** bật `cfg(test)`. Nếu ai đó thêm `--cfg test` vào dòng lệnh rustc (qua `RUSTFLAGS`,
`build.rs` phát `cargo:rustc-cfg=test`, hay `[build] rustflags` trong `.cargo/config.toml`), thì
mọi khối `#[cfg(test)]` **vào ảnh sản phẩm** — và luật token `wake` (miễn vùng test) trở nên vô
nghĩa: mã test gọi `wake()` sẽ nằm trong ảnh thật.

Phép kiểm này đọc **dòng lệnh rustc thật** (`cargo build -v`) của ảnh sản phẩm và **ĐỎ** nếu có
`--cfg test` / `--cfg=test` / `--test`. Ca âm (`--selftest`) chứng minh phép kiểm **có răng**:
một log có `--cfg test` ⇒ ĐỎ.

Vì sao đọc `cargo build -v` chứ không `--build-plan`: `--build-plan` là tính năng **unstable**
(cần nightly) và **không** phản ánh `RUSTFLAGS`/`build.rs` như dòng lệnh thật. `-v` in **nguyên
văn** lệnh rustc mà cargo chạy — đúng thứ cần kiểm.

Dùng
----
    # build thật ảnh sản phẩm rồi kiểm (mặc định: --features product-fs, target dir tạm)
    python3 v2/tools/check-no-cfg-test.py

    # kiểm lại một log đã lưu (tái dùng bằng chứng, AGENTS §6.3)
    python3 v2/tools/check-no-cfg-test.py --log /tmp/build-v.log

    # ca tự kiểm (log tổng hợp; không cần cargo)
    python3 v2/tools/check-no-cfg-test.py --selftest

    # ca âm END-TO-END: build thật với RUSTFLAGS="--cfg test" ⇒ phải ĐỎ
    python3 v2/tools/check-no-cfg-test.py --selftest-live

Exit: 0 = xanh (không có `--cfg test`); 1 = đỏ; 2 = dùng sai / không chứng minh được.
"""

from __future__ import annotations

import argparse
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import List, Optional, Sequence, Tuple

HERE = Path(__file__).resolve()
DEFAULT_ROOT = HERE.parent.parent.parent  # v2/tools/… → gốc repo

# Ảnh **sản phẩm** (KT-77c, D2-123.2): `product-fs` kéo `product-shell` → `user`; **không** kéo
# `ktest` (D2-08: ảnh test tách bằng feature, không bằng `cfg`).
DEFAULT_PACKAGE = "vifast-kbin-riscv64"
DEFAULT_BIN = "vifast-kbin-riscv64"
DEFAULT_TARGET = "riscv64gc-unknown-none-elf"
DEFAULT_FEATURES = "product-fs"

# Dòng lệnh rustc do cargo in ra ở `-v`: `     Running \`/path/rustc --crate-name …\``.
RE_RUNNING = re.compile(r"^\s*Running\s+`(.+)`\s*$")
# Dự phòng: dòng bắt đầu bằng đường dẫn rustc (một số bản cargo in trần).
RE_RUSTC_LINE = re.compile(r"^\s*(?:\S*/)?rustc(?:\.exe)?\s")

# `--cfg test` (hai token) / `--cfg=test` (một token) / `--test` (harness test).
# **Không** khớp `--check-cfg 'cfg(docsrs,test)'` (khác tên cờ) và **không** khớp
# `--cfg 'feature="ktest"'` (predicate là `feature`, không phải `test`).
RE_CFG_TEST_RAW = re.compile(r"(?<![-\w])--cfg(?:=|\s+)(?:'|\")?test(?:'|\")?(?![-\w])")
RE_TEST_FLAG_RAW = re.compile(r"(?<![-\w])--test(?![-\w])")


def _split_cmd(line: str) -> Optional[List[str]]:
    """Tách dòng `Running \\`…\\`` thành argv; `None` nếu không phải lệnh rustc."""
    m = RE_RUNNING.match(line)
    if m:
        raw = m.group(1)
    elif RE_RUSTC_LINE.match(line):
        raw = line.strip()
    else:
        return None
    try:
        argv = shlex.split(raw)
    except ValueError:
        return None
    if not argv:
        return None
    base = os.path.basename(argv[0])
    if base not in ("rustc", "rustc.exe"):
        return None
    return argv


def cfg_predicates(argv: Sequence[str]) -> List[str]:
    """Các predicate của `--cfg` trong một argv rustc (đã bỏ nháy)."""
    out: List[str] = []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--cfg" and i + 1 < len(argv):
            out.append(argv[i + 1])
            i += 2
            continue
        if a.startswith("--cfg="):
            out.append(a[len("--cfg=") :])
        i += 1
    return out


def check_log(text: str) -> Tuple[List[str], int, int]:
    """Kiểm một log `cargo build -v`.

    Trả `(errors, số dòng rustc, số predicate --cfg)`. **Fail-closed**: log **không** có dòng
    rustc nào ⇒ lỗi (không chứng minh được gì), trừ khi `allow_empty` (dùng cho ca selftest).
    """
    errors: List[str] = []
    rustc_lines = 0
    cfg_count = 0
    for lineno, line in enumerate(text.splitlines(), 1):
        argv = _split_cmd(line)
        if argv is None:
            # Ngoài dòng rustc: vẫn quét thô (fail-closed) — `RUSTFLAGS` bị echo, wrapper tự in…
            if RE_CFG_TEST_RAW.search(line) or RE_TEST_FLAG_RAW.search(line):
                errors.append(
                    f"  dòng {lineno}: có `--cfg test`/`--test` **ngoài** dòng lệnh rustc — "
                    f"không chứng minh được nó không vào rustc (fail-closed): {line.strip()[:200]}"
                )
            continue
        rustc_lines += 1
        preds = cfg_predicates(argv)
        cfg_count += len(preds)
        for p in preds:
            if p.strip() == "test":
                errors.append(
                    f"  dòng {lineno}: rustc có `--cfg test` — ảnh **sản phẩm** sẽ biên dịch "
                    f"mọi khối `#[cfg(test)]` (luật token `wake` mất hiệu lực)"
                )
        if "--test" in argv:
            errors.append(
                f"  dòng {lineno}: rustc có `--test` — build harness test, không phải ảnh sản phẩm"
            )
    if rustc_lines == 0:
        errors.append(
            "  log KHÔNG có dòng lệnh rustc nào — không chứng minh được gì (fail-closed; "
            "build lại với `-v` và target dir **mới**, hoặc truyền `--log` của lần build thật)"
        )
    return errors, rustc_lines, cfg_count


def find_cargo() -> Optional[str]:
    home = os.environ.get("HOME", "")
    for c in (
        os.path.join(home, ".cargo/bin/cargo") if home else "",
        "/usr/local/cargo/bin/cargo",
        "/opt/homebrew/bin/cargo",
    ):
        if c and os.access(c, os.X_OK):
            return c
    for d in os.environ.get("PATH", "").split(os.pathsep):
        p = os.path.join(d, "cargo")
        if os.access(p, os.X_OK):
            return p
    return None


def pinned_toolchain(root: Path) -> Optional[str]:
    for relp in ("v2/rust-toolchain.toml", "v2/kernel/rust-toolchain.toml"):
        p = root / relp
        if not p.is_file():
            continue
        try:
            text = p.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        m = re.search(r'^\s*channel\s*=\s*"([^"]+)"', text, re.MULTILINE)
        if m:
            return m.group(1)
    return None


def build_log(
    root: Path,
    package: str,
    bin_name: str,
    target: str,
    features: str,
    extra_rustflags: str = "",
    timeout: float = 1800.0,
) -> Tuple[Optional[str], str]:
    """Chạy `cargo build -v` cho ảnh sản phẩm; trả `(log, ghi chú)`.

    `CARGO_TARGET_DIR` = thư mục tạm **mới** để cargo **buộc** chạy rustc (target dir cũ ⇒ cargo
    in `Fresh`, không có dòng lệnh rustc ⇒ không chứng minh được gì).
    """
    cargo = find_cargo()
    if cargo is None:
        return None, "không tìm thấy cargo"
    tmp = tempfile.mkdtemp(prefix="kt256-cfgtest-")
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = tmp
    env.setdefault("CARGO_TERM_COLOR", "never")
    tc = pinned_toolchain(root)
    if tc:
        env["RUSTUP_TOOLCHAIN"] = tc
    if extra_rustflags:
        env["RUSTFLAGS"] = (env.get("RUSTFLAGS", "") + " " + extra_rustflags).strip()
    cmd = [
        cargo,
        "build",
        "--manifest-path",
        str(root / "v2/kernel/Cargo.toml"),
        "-p",
        package,
        "--bin",
        bin_name,
        "--target",
        target,
        "--release",
        "--features",
        features,
        "-v",
    ]
    try:
        proc = subprocess.run(
            cmd, cwd=str(root), env=env, capture_output=True, text=True, timeout=timeout
        )
    except (OSError, subprocess.TimeoutExpired) as e:
        shutil.rmtree(tmp, ignore_errors=True)
        return None, f"cargo build lỗi: {e}"
    log = proc.stdout + proc.stderr
    shutil.rmtree(tmp, ignore_errors=True)
    if proc.returncode != 0:
        return log, f"cargo build RC={proc.returncode} (log vẫn được kiểm)"
    return log, ""


# ------------------------------------------------------------------ selftest

_SYN_HEADER = (
    "   Compiling vifast-core v0.1.0 (/repo/v2/kernel/vifast-core)\n"
    "     Running `/home/u/.rustup/toolchains/1.100.0/bin/rustc --crate-name vifast_core "
    "--edition=2021 vifast-core/src/lib.rs --crate-type lib "
    "--check-cfg 'cfg(docsrs,test)' --check-cfg 'cfg(feature, values())' "
    "--cfg 'feature=\"default\"' --cfg 'feature=\"product-fs\"' "
    "-C opt-level=s --out-dir /tmp/t/out --target riscv64gc-unknown-none-elf`\n"
)


def selftest() -> int:
    cases: List[Tuple[str, bool, str]] = []

    def run(name: str, text: str, want_ok: bool) -> None:
        errs, n, c = check_log(text)
        ok = (not errs) if want_ok else bool(errs)
        cases.append((name, ok, f"errors={errs} rustc_lines={n} cfg={c}"))

    # (1) Log sản phẩm thật (rút gọn): chỉ `--cfg 'feature=…'` + `--check-cfg` ⇒ XANH.
    run("log sản phẩm (`--cfg feature=…`, `--check-cfg 'cfg(docsrs,test)'`) ⇒ XANH", _SYN_HEADER, True)

    # (2) `--cfg test` (hai token) ⇒ ĐỎ.
    run(
        "`--cfg test` trong dòng rustc ⇒ ĐỎ",
        _SYN_HEADER.replace("--cfg 'feature=\"default\"'", "--cfg test --cfg 'feature=\"default\"'"),
        False,
    )

    # (3) `--cfg=test` (một token) ⇒ ĐỎ.
    run(
        "`--cfg=test` trong dòng rustc ⇒ ĐỎ",
        _SYN_HEADER.replace("--cfg 'feature=\"default\"'", "--cfg=test --cfg 'feature=\"default\"'"),
        False,
    )

    # (4) `--cfg 'test'` (có nháy) ⇒ ĐỎ.
    run(
        "`--cfg 'test'` (có nháy) ⇒ ĐỎ",
        _SYN_HEADER.replace("--cfg 'feature=\"default\"'", "--cfg 'test'"),
        False,
    )

    # (5) `--test` (harness) ⇒ ĐỎ.
    run("`--test` trong dòng rustc ⇒ ĐỎ", _SYN_HEADER.replace("--crate-type lib", "--crate-type lib --test"), False)

    # (6) `--cfg 'feature="ktest"'` ⇒ XANH (không phải predicate `test`).
    run(
        "`--cfg 'feature=\"ktest\"'` ⇒ XANH (feature, không phải `test`)",
        _SYN_HEADER.replace("--cfg 'feature=\"default\"'", "--cfg 'feature=\"ktest\"'"),
        True,
    )

    # (7) `--cfg test` **ngoài** dòng rustc (RUSTFLAGS bị echo) ⇒ ĐỎ (fail-closed).
    run(
        "`--cfg test` ngoài dòng rustc (echo RUSTFLAGS) ⇒ ĐỎ (fail-closed)",
        _SYN_HEADER + "RUSTFLAGS=--cfg test\n",
        False,
    )

    # (8) Log rỗng / không có dòng rustc ⇒ ĐỎ (không chứng minh được).
    run("log không có dòng rustc ⇒ ĐỎ (fail-closed)", "   Compiling x v0.1.0\n", False)

    # (9) `--test-threads=1` không bị nhầm là `--test`.
    run(
        "`--test-threads=1` KHÔNG bị nhầm là `--test`",
        _SYN_HEADER.replace("--crate-type lib", "--crate-type lib -Z test-threads=1"),
        True,
    )

    # (10) `--check-cfg 'cfg(test)'` (khai báo cfg hợp lệ) KHÔNG bị nhầm là `--cfg test`.
    run(
        "`--check-cfg 'cfg(test)'` KHÔNG bị nhầm là `--cfg test`",
        _SYN_HEADER.replace("--check-cfg 'cfg(docsrs,test)'", "--check-cfg 'cfg(test)'"),
        True,
    )

    fails = 0
    for name, ok, detail in cases:
        if ok:
            print(f"ok - {name}")
        else:
            fails += 1
            print(f"not ok - {name}")
            if detail:
                print(f"#   {detail}")
    print(f"# selftest: {len(cases) - fails}/{len(cases)} ok")
    return 0 if fails == 0 else 1


def selftest_live(root: Path) -> int:
    """Ca âm **end-to-end**: build thật với `RUSTFLAGS=--cfg test` ⇒ phép kiểm phải ĐỎ.

    Dùng crate **nhỏ** (`vifast-abi`, host) để nhanh; điều cần chứng minh là **đường ống**:
    `RUSTFLAGS` → dòng lệnh rustc → phép kiểm. Ảnh sản phẩm dùng cùng cơ chế.
    """
    cargo = find_cargo()
    if cargo is None:
        print("not ok - ca âm end-to-end: không tìm thấy cargo")
        print("# selftest-live: 0/1 ok")
        return 1
    tmp = tempfile.mkdtemp(prefix="kt256-live-")
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = tmp
    env["RUSTFLAGS"] = "--cfg test"
    env.setdefault("CARGO_TERM_COLOR", "never")
    tc = pinned_toolchain(root)
    if tc:
        env["RUSTUP_TOOLCHAIN"] = tc
    cmd = [
        cargo,
        "build",
        "--manifest-path",
        str(root / "v2/Cargo.toml"),
        "-p",
        "vifast-abi",
        "--target",
        "riscv64gc-unknown-none-elf",
        "-v",
    ]
    try:
        proc = subprocess.run(
            cmd, cwd=str(root), env=env, capture_output=True, text=True, timeout=600
        )
        log = proc.stdout + proc.stderr
    except (OSError, subprocess.TimeoutExpired) as e:
        shutil.rmtree(tmp, ignore_errors=True)
        print(f"not ok - ca âm end-to-end: cargo lỗi {e}")
        print("# selftest-live: 0/1 ok")
        return 1
    shutil.rmtree(tmp, ignore_errors=True)
    errs, n, _c = check_log(log)
    ok = bool(errs) and n > 0
    print(("ok - " if ok else "not ok - ") + "ca âm end-to-end: `RUSTFLAGS=--cfg test` ⇒ ĐỎ")
    if not ok:
        print(f"#   errors={errs} rustc_lines={n} (build RC={proc.returncode})")
    print(f"# selftest-live: {1 if ok else 0}/1 ok")
    return 0 if ok else 1


# ------------------------------------------------------------------ main

def main(argv: Sequence[str]) -> int:
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--root", default=None, help="gốc cây nguồn (mặc định: repo chứa tệp này)")
    ap.add_argument("--log", default=None, help="kiểm log `cargo build -v` đã lưu (không build)")
    ap.add_argument("--package", default=DEFAULT_PACKAGE)
    ap.add_argument("--bin", default=DEFAULT_BIN)
    ap.add_argument("--target", default=DEFAULT_TARGET)
    ap.add_argument("--features", default=DEFAULT_FEATURES)
    ap.add_argument("--timeout", type=float, default=1800.0)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--selftest", action="store_true", help="ca tự kiểm trên log tổng hợp")
    ap.add_argument(
        "--selftest-live",
        action="store_true",
        help="ca âm end-to-end: build thật với RUSTFLAGS=--cfg test ⇒ phải ĐỎ",
    )
    args = ap.parse_args(argv)

    root = Path(args.root).resolve() if args.root else DEFAULT_ROOT

    if args.selftest:
        return selftest()
    if args.selftest_live:
        return selftest_live(root)

    if args.log:
        try:
            text = Path(args.log).read_text(encoding="utf-8", errors="replace")
        except OSError as e:
            print(f"check-no-cfg-test: không đọc được log {args.log}: {e}", file=sys.stderr)
            return 2
        mode = f"log={args.log}"
        note = ""
    else:
        if not (root / "v2/kernel/Cargo.toml").is_file():
            print(f"check-no-cfg-test: {root} không có `v2/kernel/Cargo.toml`", file=sys.stderr)
            return 2
        text, note = build_log(
            root, args.package, args.bin, args.target, args.features, timeout=args.timeout
        )
        if text is None:
            print(f"check-no-cfg-test: {note} — fail-closed", file=sys.stderr)
            return 2
        mode = f"build={args.package}/{args.bin} target={args.target} features={args.features}"

    errors, rustc_lines, cfg_count = check_log(text)
    if args.json:
        import json

        print(
            json.dumps(
                {
                    "ok": not errors,
                    "mode": mode,
                    "rustc_lines": rustc_lines,
                    "cfg_predicates": cfg_count,
                    "errors": errors,
                    "note": note,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    else:
        if note:
            print(f"# {note}")
        for e in errors:
            print(f"E {e}")
        print(
            f"# check-no-cfg-test: mode={mode} rustc_lines={rustc_lines} "
            f"cfg_predicates={cfg_count} errors={len(errors)}"
        )
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
