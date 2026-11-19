#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""check-wake-depinfo.py — **KT-214**: lớp kiểm `wake` theo **danh sách tệp THẬT của rustc**.

Vì sao có tệp này
-----------------
Luật `wake`/`include!`/`#[path]` đã bị phá **mười một lần** trong ba ngày. Mọi bản vá đều
**tự suy luận cú pháp** (lexer + token-tree + walker) và mỗi lần lại bỏ sót **một** vị trí
hoặc **một** gốc phân giải:

  - L40 (KT-201): `macro_rules!` passthrough `:item` + `include!` trong **đối số lời gọi macro**;
  - L44 (KT-208): `include!` ở **vị trí biểu thức** trong `( … )`/`[ … ]`;
  - L47 (KT-212): `#[path]` trong **inline module** của tệp **non-mod-rs** — gốc phân giải lệch
    đúng một tầng `<tên-tệp>/`.

Giả thuyết của lead (D2-171 §5): thay vì tự suy luận cú pháp, **hỏi chính rustc** danh sách tệp
nó thật sự biên dịch, rồi quét `wake` trên **đúng** danh sách đó. Khi ấy `include!`/`#[path]`/
macro/`cfg_if!` đều do rustc phân giải — lớp lỗi "bỏ sót vị trí" **biến mất**, không phải được
vá thêm lần nữa.

Cách làm
--------
1. `cargo check --workspace --message-format=json` cho **hai** workspace sản phẩm
   (`v2/Cargo.toml` cho host, `v2/kernel/Cargo.toml` cho bare-metal riscv64). Cargo **luôn**
   truyền `--emit=dep-info` cho mỗi unit ⇒ mỗi crate có một tệp `.d` trong `<target>/…/deps/`.
2. Đọc `.d` (định dạng make: `<target>: <phụ thuộc…>`, có nối dòng `\\`) ⇒ **tập tệp nguồn thật**
   rustc đã nạp, gồm cả đích của `include!` và `#[path]` (đã đo: đích `include!` xuất hiện với
   đúng đường dẫn tương đối).
3. Áp **luật `wake`** của checker tĩnh trên từng tệp trong danh sách — **không chép** lexer/luật:
   `import` `check-wake-paths.py` và dùng lại `_check_wake_text`, `find_functions`,
   `RE_READY_TRANSITION`, `WAKE_DOORS`, `NON_WAKE_READY_SITES`, `in_test_only_wake_scope`,
   `WAKE_PRODUCER`, `SAFE_VALUE_MACROS`.
4. In **giá trị cốt lõi**: những tệp rustc nạp mà **không** nằm dưới `<crate>/src/` — tức tệp
   chỉ tới được qua `#[path]`/`include!`/macro, đúng lớp mà walker thủ công bỏ sót.

Hai luật được áp (cùng ngữ nghĩa với checker tĩnh, khác **nguồn danh sách tệp**):

  - **R1 `wake`** (`E_DEPINFO_WAKE`): mọi tham chiếu `wake` không được miễn (định nghĩa `fn wake`,
    biến cục bộ, chú thích/chuỗi, ngữ cảnh test) trong **bất kỳ** tệp rustc nạp ⇒ ĐỎ. Miễn:
    tệp **định nghĩa** một cửa trong `WAKE_PRODUCER` (`fn wake`/`wake_from`/`wake_from_unlink`)
    và tệp **test-only** (đồ thị module của checker tĩnh).
  - **R2 chuyển `Ready`** (`E_DEPINFO_TRANSITION`): mọi `transition_tcb(…, TcbState::Ready)`
    trong tệp dưới `v2/kernel`/`v2/sim` phải nằm trong cửa `WAKE_DOORS` hoặc có tên trong
    `NON_WAKE_READY_SITES` (allowlist hiện có, khoá `(tệp, hàm, thứ tự)`).

Khác biệt **có chủ đích** so với checker tĩnh: R1 áp cho **mọi** tệp rustc nạp, không chỉ tệp
khớp `in_test_only_wake_scope` — vì chính cái "khớp mẫu đường dẫn" là thứ đã sai ở L47
(`v2/kernel/vifast-core/evil.rs` nằm ngoài `src/` nên không được quét).

Chạy
----
    python3 v2/tools/check-wake-depinfo.py [--root R] [--json] [--selftest]
    python3 v2/tools/check-wake-depinfo.py --no-build      # dùng .d đã có, không build lại
    python3 v2/tools/check-wake-depinfo.py --fresh         # target dir tạm ⇒ build lạnh
    python3 v2/tools/check-wake-depinfo.py --unit host     # chỉ một đơn vị build
    python3 v2/tools/check-wake-depinfo.py --verbose       # in danh sách tệp ẩn

Exit: 0 = xanh; 1 = đỏ; 2 = dùng sai.

**Trạng thái: NGUYÊN MẪU (KT-214).** Chưa nối vào cổng nào — xem
`v2/agent/JOURNAL/kt214-depinfo-prototype.md` §khuyến nghị.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Set, Tuple

HERE = Path(__file__).resolve()
DEFAULT_ROOT = HERE.parent.parent.parent  # v2/tools/… → gốc repo


# ------------------------------------------------------------------ import checker tĩnh

def load_static_checker():
    """Nạp `check-wake-paths.py` như **module** — dùng lại lexer/luật, không chép.

    Vì sao import chứ không chép: hai bản sao của cùng một luật sẽ lệch nhau (đúng lớp lỗi đã
    xảy ra với `is_test_file_heuristic` — KT-172). Nguyên mẫu này chỉ **đổi nguồn danh sách
    tệp**, không đổi định nghĩa vi phạm.
    """
    path = HERE.parent / "check-wake-paths.py"
    if not path.is_file():
        raise SystemExit(f"check-wake-depinfo: thiếu {path} — không import được luật")
    spec = importlib.util.spec_from_file_location("check_wake_paths", path)
    if spec is None or spec.loader is None:  # pragma: no cover - phòng hờ
        raise SystemExit(f"check-wake-depinfo: không nạp được {path}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


CWP = load_static_checker()


# ------------------------------------------------------------------ hằng số

# Đơn vị build sản phẩm. `target` = triple; `None` = host.
#
# Vì sao **hai** đơn vị: `v2/Cargo.toml` (host: lib + sim + tools) và `v2/kernel/Cargo.toml`
# (bare-metal) là **hai workspace độc lập** (D2-26). Kernel phải build cho
# `riscv64gc-unknown-none-elf` — `vifast-plat-riscv64` có asm chỉ hợp lệ trên triple đó (đã đo:
# `cargo check` host-target cho kernel workspace **ĐỎ** 30 lỗi "invalid register").
BUILD_UNITS: Tuple[Tuple[str, str, Optional[str], str], ...] = (
    ("host", "v2/Cargo.toml", None, "v2/target"),
    ("kernel", "v2/kernel/Cargo.toml", "riscv64gc-unknown-none-elf", "v2/kernel/target"),
)

# Tệp nguồn "hiển nhiên" theo thư mục: `<cây>/<crate>/src/**`. Tệp rustc nạp mà **không** khớp
# mẫu này là tệp chỉ tới được qua `#[path]`/`include!`/macro — **giá trị cốt lõi** của nguyên mẫu.
RE_CRATE_SRC = re.compile(r"^v2/(?:lib|kernel|sim|tools)/[^/]+/src/")

# Tệp `build.rs` cấp crate: cũng là tệp "hiển nhiên" (nằm cạnh `Cargo.toml`).
RE_BUILD_RS = re.compile(r"^v2/(?:lib|kernel|sim|tools)/[^/]+/build\.rs$")

# Tên tệp dep-info do cargo sinh: `<crate>-<hash>.d` (lib) hoặc `build_script_build-<hash>.d`.
RE_DEPINFO = re.compile(r"\.d$")

# "Trông giống tệp test" — **chỉ dùng để quyết định có dựng đồ thị module hay không**, KHÔNG
# dùng để miễn. Đồ thị module (`test_only_modules`) là định nghĩa duy nhất (KT-172) nhưng tốn
# ~9 s; một bản build **sản phẩm** không nạp tệp test, nên khi danh sách nạp không có tệp nào
# trông giống test thì không cần dựng đồ thị. Nếu có, dựng đồ thị và dùng **kết quả của nó**
# (không dùng mẫu tên). Mẫu tên chỉ có thể làm ta **quét nhiều hơn** (fail-closed), không thể
# miễn thêm.
RE_TESTISH = re.compile(r"(?:^|/)(?:tests?)/|ktest|_tests?\.rs$")


# ------------------------------------------------------------------ tiện ích

def find_cargo() -> Optional[str]:
    """Tìm `cargo` — cùng thứ tự với `v2/bin/v2` (PATH, ~/.cargo/bin, /usr/local/cargo/bin)."""
    c = shutil.which("cargo")
    if c:
        return c
    for cand in (
        Path.home() / ".cargo/bin/cargo",
        Path("/usr/local/cargo/bin/cargo"),
        Path("/opt/homebrew/bin/cargo"),
    ):
        if cand.is_file() and os.access(cand, os.X_OK):
            return str(cand)
    return None


def pinned_toolchain(root: Path) -> Optional[str]:
    """Đọc `channel` từ `v2/rust-toolchain.toml` — **ghim** như `v2/bin/v2` (KT-190).

    Vì sao phải ghim: cổng dùng 1.99.1 stable; máy có thể có toolchain mặc định khác. Nguyên mẫu
    chạy `cargo` trực tiếp nên phải tự ghim, nếu không kết quả phụ thuộc máy.
    """
    p = root / "v2/rust-toolchain.toml"
    if not p.is_file():
        return None
    m = re.search(r'^\s*channel\s*=\s*"([^"]+)"', p.read_text(encoding="utf-8", errors="replace"), re.M)
    return m.group(1) if m else None


def _unescape_dep(tok: str) -> str:
    """Giải escape của make dep-info: `\\ ` (dấu cách), `\\\\` (backslash), `\\#`."""
    out: List[str] = []
    i = 0
    while i < len(tok):
        ch = tok[i]
        if ch == "\\" and i + 1 < len(tok):
            nxt = tok[i + 1]
            if nxt in (" ", "\\", "#"):
                out.append(nxt)
                i += 2
                continue
        out.append(ch)
        i += 1
    return "".join(out)


def parse_depinfo(path: Path) -> Tuple[Set[str], List[str]]:
    """Đọc một tệp `.d` ⇒ (tập **đích** của tệp, danh sách **phụ thuộc**).

    Định dạng make: `<đích>: <phụ thuộc> <phụ thuộc> …`, nối dòng bằng `\\` + newline. Tệp `.d`
    của rustc có **nhiều** dòng đích (chính `.d`, `.rmeta`/binary, rồi từng tệp nguồn làm đích
    rỗng) — gom hết phụ thuộc của mọi dòng, gom hết đích để đối chiếu với artifact của cargo.
    """
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return set(), []
    text = text.replace("\\\r\n", " ").replace("\\\n", " ")
    targets: Set[str] = set()
    deps: List[str] = []
    for line in text.splitlines():
        if ":" not in line:
            continue
        lhs, _, rhs = line.partition(":")
        lhs = lhs.strip()
        if lhs:
            targets.add(_unescape_dep(lhs))
        for tok in rhs.split():
            deps.append(_unescape_dep(tok))
    return targets, deps


def resolve_dep(tok: str, ws_root: Path) -> Optional[Path]:
    """Đổi một mục trong `.d` thành đường dẫn tuyệt đối.

    Vì sao cần `ws_root`: cargo chạy rustc với cwd = **gốc workspace**, nên tệp của thành viên
    workspace là đường dẫn **tương đối** (`vifast-core/src/lib.rs`), còn path-dependency **ngoài**
    workspace là **tuyệt đối** (`/…/v2/lib/vifast-abi/src/lib.rs`). Đã đo cả hai dạng.
    """
    p = Path(tok)
    if not p.is_absolute():
        p = ws_root / tok
    try:
        return p.resolve()
    except OSError:  # pragma: no cover - phòng hờ
        return None


# ------------------------------------------------------------------ build + thu thập

class UnitResult:
    """Kết quả một đơn vị build: RC, thời gian, artifact, tệp nguồn rustc nạp."""

    __slots__ = ("name", "cmd", "rc", "seconds", "artifacts", "depinfo_used", "loaded", "note")

    def __init__(self, name: str, cmd: Sequence[str]) -> None:
        self.name = name
        self.cmd = list(cmd)
        self.rc = 0
        self.seconds = 0.0
        self.artifacts = 0
        self.depinfo_used = 0
        self.loaded: Set[Path] = set()
        self.note = ""


def run_unit(
    root: Path,
    name: str,
    manifest: str,
    triple: Optional[str],
    target_rel: str,
    cargo: str,
    toolchain: Optional[str],
    timeout: float,
    fresh_dir: Optional[Path],
) -> UnitResult:
    """Chạy `cargo check` cho một đơn vị và trả kết quả (không tự đỏ khi RC≠0 — tầng gọi quyết)."""
    cmd = [
        cargo,
        "check",
        "--manifest-path",
        str(root / manifest),
        "--workspace",
        "--message-format=json",
    ]
    if triple:
        cmd += ["--target", triple]
    res = UnitResult(name, cmd)

    env = dict(os.environ)
    if toolchain:
        env["RUSTUP_TOOLCHAIN"] = toolchain
    if fresh_dir is not None:
        env["CARGO_TARGET_DIR"] = str(fresh_dir / name)
    # `CARGO_TERM_COLOR=never` để log dễ đọc; `RUSTFLAGS` giữ nguyên của người dùng.
    env.setdefault("CARGO_TERM_COLOR", "never")

    t0 = time.monotonic()
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(root),
            env=env,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        stdout, stderr, rc = proc.stdout, proc.stderr, proc.returncode
    except subprocess.TimeoutExpired:
        res.seconds = time.monotonic() - t0
        res.rc = 124
        res.note = f"quá thời gian {timeout:.0f}s"
        return res
    except OSError as e:  # pragma: no cover - phòng hờ
        res.seconds = time.monotonic() - t0
        res.rc = 127
        res.note = f"không chạy được cargo: {e}"
        return res
    res.seconds = time.monotonic() - t0
    res.rc = rc

    # --- artifact của **lần chạy này** (kể cả unit `fresh`) --------------------------------
    artifacts: Set[str] = set()
    build_ok: Optional[bool] = None
    for line in stdout.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        reason = msg.get("reason")
        if reason == "compiler-artifact":
            for fn in msg.get("filenames") or []:
                artifacts.add(str(Path(fn)))
        elif reason == "build-finished":
            build_ok = bool(msg.get("success"))
    res.artifacts = len(artifacts)

    # --- đọc `.d` --------------------------------------------------------------------------
    deps_dir = Path(fresh_dir / name) if fresh_dir is not None else (root / target_rel)
    if triple:
        deps_dir = deps_dir / triple
    deps_dir = deps_dir / "debug/deps"
    ws_root = (root / manifest).parent

    if not deps_dir.is_dir():
        res.note = f"không có thư mục dep-info {deps_dir}"
        return res

    # Khi build **thành công**: chỉ lấy `.d` của unit có mặt trong lần chạy này (chính xác).
    # Khi build **thất bại**: cargo **không** phát `compiler-artifact` cho unit hỏng, nhưng rustc
    # **vẫn ghi** `.d` (đã đo với lỗi kiểu E0308 và E0583) ⇒ lấy **mọi** `.d` trong thư mục
    # (superset, fail-closed) và báo `E_DEPINFO_BUILD` ở tầng gọi.
    use_all = (build_ok is False) or (not artifacts)
    if build_ok is False:
        res.note = "build ĐỎ — dùng mọi .d trong thư mục (superset, fail-closed)"

    for d in sorted(deps_dir.glob("*.d")):
        if not RE_DEPINFO.search(d.name):
            continue
        targets, deps = parse_depinfo(d)
        if not use_all and not (targets & artifacts):
            continue
        res.depinfo_used += 1
        for tok in deps:
            p = resolve_dep(tok, ws_root)
            if p is None or p.suffix != ".rs" or not p.is_file():
                continue
            try:
                relp = p.relative_to(root)
            except ValueError:
                continue
            if "target" in relp.parts:
                continue
            res.loaded.add(p)
    return res


# ------------------------------------------------------------------ luật

def is_producer_file(text: str) -> bool:
    """Tệp **định nghĩa** một cửa phát `Wake` (`WAKE_PRODUCER`) ⇒ được phép chứa `wake`.

    Vì sao cần: `core_sched.rs`/`kobj.rs` khai `pub fn wake(…)` — chính chỗ khai báo API. Luật
    token đã miễn **định nghĩa** `fn wake`, nhưng tệp còn có thể chứa tham chiếu khác (doc, alias)
    trong tương lai; allowlist theo **tệp** giữ nguyên tinh thần "chỉ cửa wake được phát Wake".
    """
    for name in CWP.WAKE_PRODUCER:
        if re.search(r"\bfn\s+" + re.escape(name) + r"\s*\(", text):
            return True
    return False


def hidden_files(root: Path, loaded: Iterable[Path]) -> List[str]:
    """Tệp rustc nạp mà **không** nằm dưới `<crate>/src/` — chỉ tới được qua `#[path]`/`include!`."""
    out: List[str] = []
    for p in loaded:
        relp = CWP.rel(root, p)
        if RE_CRATE_SRC.match(relp) or RE_BUILD_RS.match(relp):
            continue
        out.append(relp)
    return sorted(out)


def outside_static_scope(root: Path, loaded: Iterable[Path]) -> List[str]:
    """Tệp rustc nạp mà luật `wake` **tĩnh** không mở (không khớp `in_test_only_wake_scope`)."""
    return sorted(CWP.rel(root, p) for p in loaded if not CWP.in_test_only_wake_scope(CWP.rel(root, p)))


def scan(
    root: Path,
    loaded: Iterable[Path],
    test_only: Optional[Set[str]] = None,
    do_transitions: bool = True,
) -> Tuple["CWP.Report", Dict[str, int]]:
    """Áp R1 (`wake`) + R2 (chuyển `Ready`) trên **danh sách tệp rustc nạp**.

    Trả `(Report, đếm)`. `test_only` = tập relpath test-only (đồ thị module); `None` ⇒ không miễn
    theo test (dùng cho `--selftest` khi cây fixture không có `Cargo.toml`).
    """
    r = CWP.Report()
    files = sorted(loaded)
    texts: Dict[Path, str] = {}
    triples: List[Tuple[Path, str, List["CWP.Fn"]]] = []
    for p in files:
        try:
            text = p.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        texts[p] = text
        triples.append((p, text, CWP.find_functions(p, text)))

    # Tên macro "an toàn" bị `macro_rules!` trong cây che ⇒ không còn an toàn (KT-177/G6).
    safe_macros = CWP.SAFE_VALUE_MACROS - CWP._shadowed_safe_macros(root, triples)

    counts = {"loaded": len(files), "scanned": 0, "exempt_producer": 0, "exempt_test": 0}
    for p in files:
        text = texts.get(p)
        if text is None:
            continue
        relp = CWP.rel(root, p)
        if test_only is not None and relp in test_only:
            counts["exempt_test"] += 1
            continue
        if is_producer_file(text):
            counts["exempt_producer"] += 1
            continue
        counts["scanned"] += 1
        # R1 — luật token `wake` của checker tĩnh, **nguyên văn** (không chép).
        sub = CWP.Report()
        CWP._check_wake_text(relp, text, CWP.test_line_ranges(text), safe_macros, sub)
        for e in sub.errors:
            # `Report.err` lưu `"<mã> <thông điệp>"`; đổi mã sang mã của lớp dep-info.
            msg = e.split(" ", 1)[1] if " " in e else e
            r.err("E_DEPINFO_WAKE", msg)

    if do_transitions:
        # R2 — **nguyên văn** cách checker tĩnh làm (KT-155): quét **theo dòng**, bỏ vùng
        # `#[cfg(test)]`/`mod tests` (`test_line_ranges`), xác định hàm bao quanh bằng
        # `enclosing`, đếm **thứ tự** trong hàm. Không dùng khoảng dòng của `find_functions`
        # vì khoảng đó gộp cả hàm test nằm sau ⇒ báo oan (đã đo: 7 dương tính giả).
        for p in files:
            text = texts.get(p)
            if text is None:
                continue
            relp = CWP.rel(root, p)
            if not (relp.startswith("v2/kernel/") or relp.startswith("v2/sim/")):
                continue
            if test_only is not None and relp in test_only:
                continue
            tr = CWP.test_line_ranges(text)
            fns = CWP.find_functions(p, text)
            lines = text.splitlines()
            ordinal: Dict[Tuple[str, str], int] = {}
            for lineno, line in enumerate(lines, 1):
                if CWP.in_ranges(lineno, tr):
                    continue
                if not CWP.RE_READY_TRANSITION.search(line):
                    continue
                fn = CWP.enclosing(fns, lineno)
                if fn is None:
                    r.err(
                        "E_DEPINFO_TRANSITION",
                        f"  {relp}:{lineno}: `TcbState::Ready` ngoài hàm — không xác định "
                        f"được cửa (fail-closed)",
                    )
                    continue
                key = (relp, fn.name)
                ordinal[key] = ordinal.get(key, 0) + 1
                idx = ordinal[key]
                if (relp, fn.name) in CWP.WAKE_DOORS:
                    continue
                if (relp, fn.name, idx) in CWP.NON_WAKE_READY_SITES:
                    continue
                r.err(
                    "E_DEPINFO_TRANSITION",
                    f"  {relp}:{lineno}: `transition_tcb(…, Ready)` #{idx} trong "
                    f"`{fn.name}` không qua cửa wake và không có trong "
                    f"NON_WAKE_READY_SITES",
                )
    return r, counts


# ------------------------------------------------------------------ selftest

_GIA_LIB = """// SPDX-License-Identifier: MIT OR Apache-2.0
pub struct Machine;
pub type ObjId = u32;
"""

_GIA_SCHED = """// SPDX-License-Identifier: MIT OR Apache-2.0
pub struct Machine;
impl Machine {
    pub fn wake(&mut self, _t: u32) {
        let _ = _t;
    }
    pub fn wake_from(&mut self, _t: u32) {
        let _ = _t;
    }
    pub fn wake_from_unlink(&mut self, _t: u32) {
        let _ = _t;
    }
    pub fn wake_step(&mut self, _t: u32) {
        transition_tcb(1, TcbState::Ready);
    }
}
pub fn transition_tcb(_a: u32, _b: u32) {
    let _ = (_a, _b);
}
pub enum TcbState {
    Ready,
}
"""

_GIA_EVIL = """// SPDX-License-Identifier: MIT OR Apache-2.0
pub fn evil(m: &mut crate::Machine, t: u32) {
    let _ = m.wake(t);
}
"""

_GIA_CLEAN = """// SPDX-License-Identifier: MIT OR Apache-2.0
pub fn clean() -> u32 { 1 }
"""


def _dung_cay(base: Path) -> Path:
    """Dựng cây v2 tối thiểu: một crate kernel có `core_sched.rs` (cửa wake) + tệp sạch."""
    root = base / "repo"
    src = root / "v2/kernel/vifast-core/src"
    src.mkdir(parents=True, exist_ok=True)
    (src / "lib.rs").write_text(_GIA_LIB, encoding="utf-8")
    (src / "core_sched.rs").write_text(_GIA_SCHED, encoding="utf-8")
    (src / "clean.rs").write_text(_GIA_CLEAN, encoding="utf-8")
    return root


def _ghi_d(duong_dan: Path, ws_root: Path, deps: Sequence[Path], target: Optional[Path] = None) -> None:
    """Ghi một tệp `.d` giả theo đúng định dạng make của rustc (có nối dòng).

    `target` phải là **đường dẫn tuyệt đối** như rustc ghi (đã đo trên cây thật) — bộ lọc
    artifact của `run_unit` so khớp tuyệt đối.
    """
    duong_dan.parent.mkdir(parents=True, exist_ok=True)
    tgt = str(target) if target is not None else str(duong_dan.with_suffix(".rmeta"))
    parts: List[str] = []
    for d in deps:
        try:
            parts.append(str(d.relative_to(ws_root)))
        except ValueError:
            parts.append(str(d))
    body = f"{tgt}: " + " ".join(parts) + "\n\n"
    body += f"{tgt}: " + " ".join(parts) + "\n"
    duong_dan.write_text(body, encoding="utf-8")


def selftest() -> int:
    """Ca tự kiểm — **không** cần cargo: kiểm tầng đọc `.d` + hai luật trên cây dựng tay."""
    cases: List[Tuple[str, bool, str]] = []

    def ghi(ten: str, ok: bool, chi_tiet: str = "") -> None:
        cases.append((ten, ok, chi_tiet))
        print(f"{'ok  - ' if ok else 'FAIL - '}{ten}" + (f"  [{chi_tiet}]" if chi_tiet and not ok else ""))

    with tempfile.TemporaryDirectory(prefix="kt214-depinfo-") as td:
        base = Path(td)

        # --- (1) parse `.d`: nối dòng, đường dẫn tuyệt đối + tương đối, escape dấu cách ------
        ws = base / "ws"
        f1 = ws / "src/a.rs"
        f2 = ws / "src/b.rs"
        f1.parent.mkdir(parents=True, exist_ok=True)
        f1.write_text("// x\n", encoding="utf-8")
        f2.write_text("// y\n", encoding="utf-8")
        d = base / "x.d"
        d.write_text(
            "libx.rmeta: src/a.rs \\\n  src/b.rs\n\nlibx.rmeta: src/a.rs src/b.rs\n",
            encoding="utf-8",
        )
        targets, deps = parse_depinfo(d)
        ghi(
            "(1) parse .d: nối dòng `\\` + gom nhiều dòng đích",
            targets == {"libx.rmeta"} and deps == ["src/a.rs", "src/b.rs", "src/a.rs", "src/b.rs"],
            f"targets={sorted(targets)} deps={deps}",
        )
        ghi(
            "(2) resolve tương đối theo gốc workspace",
            resolve_dep("src/a.rs", ws) == f1.resolve(),
            str(resolve_dep("src/a.rs", ws)),
        )
        ghi(
            "(3) resolve tuyệt đối giữ nguyên",
            resolve_dep(str(f2), ws) == f2.resolve(),
            str(resolve_dep(str(f2), ws)),
        )
        ghi(
            "(4) escape `\\ ` trong dep-info (tên tệp có dấu cách)",
            _unescape_dep("a\\ b.rs") == "a b.rs",
            _unescape_dep("a\\ b.rs"),
        )

        # --- (5) cây sạch ⇒ XANH ------------------------------------------------------------
        root = _dung_cay(base / "sach")
        src = root / "v2/kernel/vifast-core/src"
        loaded = {src / "lib.rs", src / "core_sched.rs", src / "clean.rs"}
        rep, counts = scan(root, loaded, test_only=set())
        ghi(
            "(5) cây sạch (chỉ tệp trong src/) ⇒ XANH",
            rep.ok and counts["scanned"] == 2 and counts["exempt_producer"] == 1,
            f"errors={rep.errors} counts={counts}",
        )
        ghi(
            "(6) tệp định nghĩa `fn wake` được miễn (WAKE_PRODUCER)",
            counts["exempt_producer"] == 1,
            f"exempt_producer={counts['exempt_producer']}",
        )

        # --- (7) ĐÍCH `include!` NGOÀI `<crate>/src` chứa `wake` ⇒ ĐỎ -----------------------
        evil = root / "v2/lib/evil.rs"
        evil.parent.mkdir(parents=True, exist_ok=True)
        evil.write_text(_GIA_EVIL, encoding="utf-8")
        rep, _ = scan(root, loaded | {evil}, test_only=set())
        ghi(
            "(7) tệp ngoài `<crate>/src` (đích include!) chứa `wake` ⇒ ĐỎ E_DEPINFO_WAKE",
            (not rep.ok) and any("E_DEPINFO_WAKE" in e for e in rep.errors),
            f"errors={rep.errors}",
        )
        ghi(
            "(8) tệp đó nằm trong danh sách `hidden_files` (giá trị cốt lõi)",
            hidden_files(root, loaded | {evil}) == ["v2/lib/evil.rs"],
            str(hidden_files(root, loaded | {evil})),
        )

        # --- (9) `#[path]` trỏ ra ngoài `src/` (lớp L47) ⇒ ĐỎ --------------------------------
        evil2 = root / "v2/kernel/vifast-core/evil.rs"
        evil2.write_text(_GIA_EVIL, encoding="utf-8")
        rep, _ = scan(root, loaded | {evil2}, test_only=set())
        ghi(
            "(9) tệp dưới v2/kernel nhưng NGOÀI src/ (lớp L47) chứa `wake` ⇒ ĐỎ",
            (not rep.ok) and any("E_DEPINFO_WAKE" in e for e in rep.errors),
            f"errors={rep.errors}",
        )
        ghi(
            "(10) tệp đó KHÔNG khớp `in_test_only_wake_scope` (đúng lỗ của checker tĩnh)",
            not CWP.in_test_only_wake_scope("v2/kernel/vifast-core/evil.rs"),
            "in_test_only_wake_scope trả True (mẫu đã đổi?)",
        )

        # --- (11) test-only ⇒ miễn ----------------------------------------------------------
        ktest = src / "ktest_evil.rs"
        ktest.write_text(_GIA_EVIL, encoding="utf-8")
        rep, counts = scan(root, loaded | {ktest}, test_only={"v2/kernel/vifast-core/src/ktest_evil.rs"})
        ghi(
            "(11) tệp test-only (đồ thị module) chứa `wake` ⇒ XANH",
            rep.ok and counts["exempt_test"] == 1,
            f"errors={rep.errors} counts={counts}",
        )

        # --- (12) `wake` trong chú thích/chuỗi ⇒ miễn ---------------------------------------
        cmt = src / "cmt.rs"
        cmt.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "// gọi m.wake(t) ở đây là sai\n"
            'pub const S: &str = "m.wake(t)";\n',
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {cmt}, test_only=set())
        ghi("(12) `wake` trong chú thích + literal chuỗi ⇒ XANH", rep.ok, f"errors={rep.errors}")

        # --- (13) biến cục bộ tên `wake` ⇒ miễn ---------------------------------------------
        loc = src / "loc.rs"
        loc.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn f() -> u32 { let mut wake = [0u32; 4]; wake[0] = 1; wake[0] }\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {loc}, test_only=set())
        ghi("(13) biến cục bộ tên `wake` ⇒ XANH", rep.ok, f"errors={rep.errors}")

        # --- (14) R2: chuyển Ready ngoài cửa/allowlist ⇒ ĐỎ ---------------------------------
        bad = src / "bad_ready.rs"
        bad.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn khong_qua_cua() {\n"
            "    transition_tcb(1, TcbState::Ready);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {bad}, test_only=set())
        ghi(
            "(14) R2: `transition_tcb(…, Ready)` ngoài cửa wake ⇒ ĐỎ E_DEPINFO_TRANSITION",
            (not rep.ok) and any("E_DEPINFO_TRANSITION" in e for e in rep.errors),
            f"errors={rep.errors}",
        )
        ghi(
            "(15) R2: `wake_step` (cửa trong WAKE_DOORS) ⇒ XANH",
            not any("E_DEPINFO_TRANSITION" in e for e in scan(root, loaded, test_only=set())[0].errors),
            "cửa wake_step bị báo oan",
        )

        # --- (16) R2: mục trong NON_WAKE_READY_SITES ⇒ XANH ---------------------------------
        allow = src / "allow_ready.rs"
        allow.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn dispatch_step() {\n"
            "    transition_tcb(1, TcbState::Ready);\n"
            "}\n",
            encoding="utf-8",
        )
        key = ("v2/kernel/vifast-core/src/allow_ready.rs", "dispatch_step", 1)
        cu = dict(CWP.NON_WAKE_READY_SITES)
        CWP.NON_WAKE_READY_SITES[key] = "ca kiểm KT-214"
        try:
            rep, _ = scan(root, loaded | {allow}, test_only=set())
        finally:
            del CWP.NON_WAKE_READY_SITES[key]
        ghi(
            "(16) R2: mục có trong NON_WAKE_READY_SITES ⇒ XANH",
            not any("E_DEPINFO_TRANSITION" in e for e in rep.errors),
            f"errors={rep.errors}",
        )

        # --- (17) `.d` của unit không thuộc lần chạy này ⇒ không lấy -------------------------
        ws2 = base / "ws2"
        (ws2 / "src").mkdir(parents=True, exist_ok=True)
        (ws2 / "src/a.rs").write_text("// a\n", encoding="utf-8")
        dd = base / "deps"
        _ghi_d(dd / "cu-1111.d", ws2, [ws2 / "src/a.rs"], target=dd / "libcu.rmeta")
        _ghi_d(dd / "moi-2222.d", ws2, [ws2 / "src/a.rs"], target=dd / "libmoi.rmeta")
        # Mô phỏng bộ lọc của `run_unit`: chỉ `.d` có đích nằm trong artifact của lần chạy.
        artifacts = {str(dd / "libmoi.rmeta")}
        used = []
        for f in sorted(dd.glob("*.d")):
            tg, _ = parse_depinfo(f)
            if tg & artifacts:
                used.append(f.name)
        ghi(
            "(17) chỉ lấy `.d` của unit có artifact trong lần chạy (bỏ `.d` cũ)",
            used == ["moi-2222.d"],
            str(used),
        )

        # --- (18) `hidden_files` không báo oan tệp chuẩn ------------------------------------
        ghi(
            "(18) `hidden_files` rỗng với tệp trong `<crate>/src` + `build.rs`",
            hidden_files(root, loaded | {root / "v2/kernel/vifast-core/build.rs"}) == [],
            str(hidden_files(root, loaded | {root / "v2/kernel/vifast-core/build.rs"})),
        )

        # --- (19) `outside_static_scope` đúng với mẫu của checker tĩnh ----------------------
        ghi(
            "(19) `outside_static_scope` gồm v2/lib nhưng không gồm src/ của kernel",
            outside_static_scope(root, {evil, src / "clean.rs"}) == ["v2/lib/evil.rs"],
            str(outside_static_scope(root, {evil, src / "clean.rs"})),
        )

        # --- (20) JSON của Report là JSON hợp lệ -------------------------------------------
        rep, counts = scan(root, loaded | {evil}, test_only=set())
        blob = json.dumps({"ok": rep.ok, "errors": rep.errors, "counts": counts}, ensure_ascii=False)
        ghi(
            "(20) kết quả tuần tự hoá được sang JSON",
            json.loads(blob)["ok"] is False and len(json.loads(blob)["errors"]) >= 1,
            blob[:120],
        )

    ok = sum(1 for _, o, _ in cases if o)
    print(f"# selftest: {ok}/{len(cases)} ok")
    return 0 if ok == len(cases) else 1


# ------------------------------------------------------------------ main

def main(argv: Sequence[str]) -> int:
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--root", default=None, help="gốc repo (mặc định: repo chứa tệp này)")
    ap.add_argument("--json", action="store_true", help="in kết quả dạng JSON")
    ap.add_argument("--selftest", action="store_true", help="chạy ca tự kiểm (không cần cargo)")
    ap.add_argument("--no-build", action="store_true", help="không build; đọc .d đã có")
    ap.add_argument("--fresh", action="store_true", help="target dir tạm ⇒ build lạnh (đo chi phí)")
    ap.add_argument(
        "--unit",
        default="all",
        choices=["all", "host", "kernel"],
        help="đơn vị build (mặc định: cả hai)",
    )
    ap.add_argument("--timeout", type=float, default=900.0, help="thời gian tối đa mỗi đơn vị (s)")
    ap.add_argument("--verbose", action="store_true", help="in danh sách tệp ẩn/ngoài phạm vi")
    ap.add_argument(
        "--no-transition",
        action="store_true",
        help="bỏ luật R2 (chuyển Ready) — chỉ áp luật token `wake`",
    )
    args = ap.parse_args(argv)

    if args.selftest:
        return selftest()

    root = Path(args.root).resolve() if args.root else DEFAULT_ROOT
    if not (root / "v2/kernel").is_dir():
        print(f"check-wake-depinfo: {root} không có `v2/kernel` — fail-closed", file=sys.stderr)
        return 2

    t0 = time.monotonic()
    cargo = find_cargo()
    toolchain = pinned_toolchain(root)
    fresh_dir: Optional[Path] = None
    tmp_holder: Optional[tempfile.TemporaryDirectory] = None
    if args.fresh:
        tmp_holder = tempfile.TemporaryDirectory(prefix="kt214-target-")
        fresh_dir = Path(tmp_holder.name)

    units = [u for u in BUILD_UNITS if args.unit in ("all", u[0])]
    results: List[UnitResult] = []
    build_errors: List[str] = []

    if args.no_build:
        # Không build: đọc `.d` đã có trong target dir mặc định (dùng cho đo chi phí quét).
        for name, manifest, triple, target_rel in units:
            res = UnitResult(name, ["(no-build)"])
            deps_dir = root / target_rel
            if triple:
                deps_dir = deps_dir / triple
            deps_dir = deps_dir / "debug/deps"
            ws_root = (root / manifest).parent
            if deps_dir.is_dir():
                for d in sorted(deps_dir.glob("*.d")):
                    targets, deps = parse_depinfo(d)
                    res.depinfo_used += 1
                    for tok in deps:
                        p = resolve_dep(tok, ws_root)
                        if p is None or p.suffix != ".rs" or not p.is_file():
                            continue
                        try:
                            relp = p.relative_to(root)
                        except ValueError:
                            continue
                        if "target" in relp.parts:
                            continue
                        res.loaded.add(p)
            else:
                res.note = f"không có {deps_dir}"
            results.append(res)
    else:
        if cargo is None:
            print("check-wake-depinfo: không tìm thấy cargo", file=sys.stderr)
            return 2
        for name, manifest, triple, target_rel in units:
            res = run_unit(
                root,
                name,
                manifest,
                triple,
                target_rel,
                cargo,
                toolchain,
                args.timeout,
                fresh_dir,
            )
            results.append(res)
            if res.rc != 0:
                build_errors.append(
                    f"  đơn vị `{name}`: cargo check RC={res.rc}"
                    + (f" ({res.note})" if res.note else "")
                )

    loaded: Set[Path] = set()
    for res in results:
        loaded |= res.loaded

    # Test-only theo **đồ thị module** của checker tĩnh (một định nghĩa duy nhất — KT-172).
    # Chỉ dựng khi cần: bản build sản phẩm không nạp tệp test, nên nếu danh sách nạp không có
    # tệp nào "trông giống test" thì miễn theo test là vô nghĩa (và đồ thị tốn ~9 s).
    testish = [CWP.rel(root, p) for p in loaded if RE_TESTISH.search(CWP.rel(root, p))]
    graph_mode = "không cần (không có tệp tên test trong danh sách nạp)"
    test_only: Set[str] = set()
    if testish:
        try:
            test_only = CWP.test_only_modules(root)
            graph_mode = f"đồ thị module ({len(test_only)} tệp test-only)"
        except Exception:  # pragma: no cover - cây lạ ⇒ không miễn theo test (fail-closed)
            test_only = set()
            graph_mode = "LỖI dựng đồ thị ⇒ không miễn theo test (fail-closed)"

    rep, counts = scan(root, loaded, test_only=test_only, do_transitions=not args.no_transition)
    for e in build_errors:
        rep.err("E_DEPINFO_BUILD", e)

    hidden = hidden_files(root, loaded)
    outside = outside_static_scope(root, loaded)
    total = time.monotonic() - t0

    if args.json:
        try:
            print(
                json.dumps(
                    {
                        "ok": rep.ok,
                        "root": str(root),
                        "seconds": round(total, 3),
                        "units": [
                            {
                                "name": u.name,
                                "cmd": u.cmd,
                                "rc": u.rc,
                                "seconds": round(u.seconds, 3),
                                "artifacts": u.artifacts,
                                "depinfo_used": u.depinfo_used,
                                "loaded": len(u.loaded),
                                "note": u.note,
                            }
                            for u in results
                        ],
                        "loaded_files": counts["loaded"],
                        "scanned_files": counts["scanned"],
                        "exempt_producer": counts["exempt_producer"],
                        "exempt_test": counts["exempt_test"],
                        "hidden_files": hidden,
                        "outside_static_scope": outside,
                        "errors": rep.errors,
                        "warnings": rep.warnings,
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
        finally:
            if tmp_holder is not None:
                tmp_holder.cleanup()
        return 0 if rep.ok else 1

    print(f"# check-wake-depinfo: root={root}")
    for u in results:
        cmd = " ".join(u.cmd)
        print(
            f"# đơn vị {u.name}: {cmd} → RC={u.rc} ({u.seconds:.2f} s), "
            f"{u.artifacts} artifact, {u.depinfo_used} tệp .d, {len(u.loaded)} tệp .rs"
            + (f" [{u.note}]" if u.note else "")
        )
    print(f"# rustc nạp: {counts['loaded']} tệp .rs trong cây (gộp {len(results)} đơn vị)")
    print(
        f"# quét: {counts['scanned']} tệp "
        f"(miễn {counts['exempt_producer']} tệp cửa wake + {counts['exempt_test']} tệp test-only)"
    )
    print(f"# test-only: {graph_mode}")
    print(
        f"# tệp CHỈ tới được qua #[path]/include!/macro (ngoài mọi <crate>/src): {len(hidden)}"
    )
    if hidden:
        for h in hidden:
            print(f"#   {h}")
    print(f"# tệp ngoài phạm vi luật wake tĩnh (in_test_only_wake_scope): {len(outside)}")
    if args.verbose:
        for o in outside:
            print(f"#   {o}")
    for line in rep.warnings:
        print(f"W {line}")
    for line in rep.errors:
        print(f"E {line}")
    print(
        f"# check-wake-depinfo: errors={len(rep.errors)} warnings={len(rep.warnings)} "
        f"({total:.2f} s)"
    )
    if tmp_holder is not None:
        tmp_holder.cleanup()
    return 0 if rep.ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
