#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""check-wake-depinfo.py — **KT-214/KT-238**: lớp kiểm `wake` theo **danh sách tệp THẬT của rustc**.

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
1. **KT-238 / D2-188** — sinh **phạm vi bảo vệ** từ `cargo metadata --no-deps --format-version 1`
   của **các workspace liên quan** (`v2/Cargo.toml`, `v2/kernel/Cargo.toml`) **cộng bao đóng
   path-dependency nội bộ**. Không dùng ba đường dẫn cố định (`v2/kernel`, `v2/sim`, `v2/lib`)
   làm định nghĩa đầy đủ: crate thêm sau / crate chỉ tới được qua path-dep vẫn vào phạm vi.
   Crate `v2/tools/` = **từ chối rõ** (công cụ, không phải sản phẩm) — chỉ khi bao đóng
   dependency của nó **không** chứa crate bảo vệ nào; nếu chứa ⇒ **nâng** lên bảo vệ.
2. `cargo check --workspace --message-format=json` cho **hai** workspace sản phẩm
   (`v2/Cargo.toml` cho host, `v2/kernel/Cargo.toml` cho bare-metal riscv64). Cargo **luôn**
   truyền `--emit=dep-info` cho mỗi unit ⇒ mỗi crate có một tệp `.d` trong `<target>/…/deps/`.
   Target dir lấy từ **`cargo metadata`** (không từ hằng), nên `.d`/nguồn sinh đọc đúng thư mục
   của lần build đang kiểm (kể cả `CARGO_TARGET_DIR` / `--fresh`).
3. Đọc `.d` (định dạng make: `<target>: <phụ thuộc…>`, có nối dòng `\\`) ⇒ **tập tệp nguồn thật**
   rustc đã nạp, gồm cả đích của `include!` và `#[path]` (đã đo: đích `include!` xuất hiện với
   đúng đường dẫn tương đối). Chỉ `.d` có artifact trong **lần chạy này** mới được dùng
   (`compiler-artifact` của cargo = fingerprint); `--no-build` chỉ đọc `.d` có trong **dấu lần
   build** (sha256) — `.d` cũ / nguồn `OUT_DIR` cũ **không** bị đọc nhầm.
4. **KT-238** — tệp trong **target dir của cargo metadata** (nguồn sinh `$OUT_DIR`) **được nạp +
   quét**, không còn `W_DEPINFO_SKIPPED` (WB-18). Tệp sinh được quy về crate qua `package_id`
   của artifact; không quy được ⇒ `E_DEPINFO_SCOPE`. Tệp rustc nạp **không thuộc crate nào**
   ⇒ `E_DEPINFO_SCOPE` (từ chối rõ). Mọi mục "bỏ qua" cũ đều thành **ĐỎ**.
5. Áp **luật `wake`** của checker tĩnh trên từng tệp trong danh sách — **không chép** lexer/luật:
   `import` `check-wake-paths.py` và dùng lại `_check_wake_text`, `find_functions`,
   `_transition_calls`, `_check_state_write`, `STATE_WRITE_FILES`, `in_test_only_wake_scope`,
   `WAKE_PRODUCER`, `SAFE_VALUE_MACROS`.
6. In **giá trị cốt lõi**: những tệp rustc nạp mà **không** nằm dưới `<crate>/src/` — tức tệp
   chỉ tới được qua `#[path]`/`include!`/macro, đúng lớp mà walker thủ công bỏ sót.

Hai luật được áp (cùng ngữ nghĩa với checker tĩnh, khác **nguồn danh sách tệp**):

  - **R1 `wake`** (`E_DEPINFO_WAKE`): mọi tham chiếu `wake` không được miễn (định nghĩa `fn wake`,
    biến cục bộ, chú thích/chuỗi, ngữ cảnh test) trong **bất kỳ** tệp rustc nạp ⇒ ĐỎ. Miễn:
    **định nghĩa** `fn wake` (do `_check_wake_text` xử lý) và tệp **test-only** (đồ thị module
    của checker tĩnh). **KT-226**: **không** miễn **cả tệp** định nghĩa cửa `wake` nữa — lớp 1
    chỉ miễn định nghĩa, lớp 2 phải đồng nhất (bất đối xứng cũ: tệp producer chứa lời gọi
    `wake(` XANH ở lớp 2 nhưng ĐỎ ở lớp 1).
  - **R2 chuyển trạng thái** (`E_DEPINFO_TRANSITION`, **KT-241**): mọi lời gọi
    `transition_tcb(…)` trong tệp của crate **bảo vệ** (phạm vi sinh từ `cargo metadata` —
    **không** chỉ `v2/kernel`/`v2/sim`) bị ĐỎ nếu đối số thứ hai là/chứa `TcbState` hoặc
    `…::Ready` — dấu hiệu **đảo kiểu** ngược về `TcbState` (API KT-241: `to: NonReadyTarget`,
    enum **không có** `Ready`). Dùng **cùng** bộ quét `CWP._transition_calls` của lớp 1.
    Allowlist `NON_WAKE_READY_SITES` **đã xoá**: dưới API mới không còn chỗ `transition_tcb(…,
    Ready)` hợp lệ nào để miễn.
  - **R3 primitive ghi thô** (`E_DEPINFO_STATE_WRITE`, **KT-241**): `write_state`/`into_state`
    **chỉ** được xuất hiện trong `state_write.rs`; trong đó `write_state` **chỉ** được gọi bởi
    `CWP.STATE_WRITE_CALLERS` (cửa thông thường + 5 cửa `Ready`). Dùng **cùng**
    `CWP._check_state_write` của lớp 1 (không chép luật).
    Crate "từ chối rõ" chỉ áp R1 — nó không thể gọi API `wake` (bao đóng dependency không có
    crate sản phẩm).

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

**Trạng thái: đã nối vào `./v2/bin/v2 check` (KT-217).** KT-238 đóng WB-18: phạm vi sinh từ
`cargo metadata`, nguồn sinh `$OUT_DIR` được quét, `.d` cũ bị loại bằng artifact/fingerprint +
dấu lần build. Nhật ký: `v2/agent/JOURNAL/kt238-metadata-scope.md`.
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.parse
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Set, Tuple, Union

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

# **KT-256 (L2, D2-188)** — phạm vi bảo vệ là **MỘT** định nghĩa (`v2/tools/wake_scope.py`), dùng
# chung với lớp tĩnh. Trước KT-256 lớp này có `Scope`/`CrateInfo`/`scope_from_metadata`/`load_scope`
# **riêng** và lớp tĩnh có **hai** mẫu riêng (`RE_TEST_ONLY_WAKE_SCOPE`, `MACRO_SCAN_ROOTS`) —
# ba định nghĩa vênh nhau (nợ L2). Nay cả hai import cùng một nguồn; các tên cũ giữ làm **alias**
# để selftest/ca đã có không phải sửa hàng loạt.
sys.path.insert(0, str(HERE.parent))
import wake_scope  # noqa: E402  (đường dẫn phải sửa trước khi import)

CrateInfo = wake_scope.CrateScope
Scope = wake_scope.ProtectedScope
scope_from_metadata = wake_scope.scope_from_metadata
load_scope = wake_scope.load_scope
protected_scope = wake_scope.protected_scope


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

# **KT-256 (L2)** — `RE_CRATE_SRC`/`RE_BUILD_RS` **xoá**: "tệp nguồn hiển nhiên" nay là
# `wake_scope.protected_scope(root).contains(relp)` (định nghĩa duy nhất sinh từ `cargo metadata`).
# Mẫu cũ bỏ sót crate ở thư mục gốc **mới** (`v2/drivers/…`) và tệp nguồn **ngoài `src/`**.

# Tên tệp dep-info do cargo sinh: `<crate>-<hash>.d` (lib) hoặc `build_script_build-<hash>.d`.
RE_DEPINFO = re.compile(r"\.d$")

# "Trông giống tệp test" — **chỉ dùng để quyết định có dựng đồ thị module hay không**, KHÔNG
# dùng để miễn. Đồ thị module (`test_only_modules`) là định nghĩa duy nhất (KT-172) nhưng tốn
# ~9 s; một bản build **sản phẩm** không nạp tệp test, nên khi danh sách nạp không có tệp nào
# trông giống test thì không cần dựng đồ thị. Nếu có, dựng đồ thị và dùng **kết quả của nó**
# (không dùng mẫu tên). Mẫu tên chỉ có thể làm ta **quét nhiều hơn** (fail-closed), không thể
# miễn thêm.
RE_TESTISH = re.compile(r"(?:^|/)(?:tests?)/|ktest|_tests?\.rs$")


# ------------------------------------------------------------------ phạm vi từ cargo metadata
#
# **KT-238 / D2-188** — phạm vi bảo vệ **sinh từ `cargo metadata`**, không từ ba đường dẫn cố
# định (`v2/kernel`, `v2/sim`, `v2/lib`). Vì sao: WB-18 (KT-233) — một crate thành viên workspace
# **ngoài** ba gốc ấy sinh mã trong `$OUT_DIR`; lớp 2 thấy tệp nhưng chỉ cảnh báo rồi bỏ qua.
#
# **KT-256 (L2)** — định nghĩa đó nay nằm ở **một** chỗ: `v2/tools/wake_scope.py` (dùng chung với
# lớp tĩnh). Tệp này **không** còn `Scope`/`CrateInfo`/`scope_from_metadata`/`load_scope` riêng —
# chỉ còn alias (dưới) để ca/selftest cũ không phải sửa hàng loạt. Ba mẫu phạm vi cũ
# (`RE_TEST_ONLY_WAKE_SCOPE`, `MACRO_SCAN_ROOTS`, `RE_CRATE_SRC`) đã **xoá**.
#
# Tệp rustc nạp mà **không** quy được về crate nào (và không phải tệp sinh trong target dir của
# cargo metadata) ⇒ `E_DEPINFO_SCOPE` — **từ chối rõ**, không quét-rồi-bỏ-qua.

# Hai workspace sản phẩm (D2-26) + tiền tố crate công cụ — **một** nguồn ở `wake_scope`.
WORKSPACES: Tuple[Tuple[str, str], ...] = wake_scope.WORKSPACES
REFUSED_PREFIXES: Tuple[str, ...] = wake_scope.REFUSED_PREFIXES

# Dấu "lần build hiện tại" cho `--no-build`: danh sách `.d` + sha256 của lần build xanh gần nhất.
# Không có dấu / dấu lệch ⇒ `E_DEPINFO_STALE` (fail-closed) — chứng minh không đọc `.d` cũ.
STAMP_NAME = ".kt238-depinfo-stamp.json"
STAMP_VERSION = 2


# **KT-256** — `_rel_or_none` là tiện ích đường dẫn dùng chung (một nguồn ở `wake_scope`).
_rel_or_none = wake_scope._rel_or_none


def _under_any(p: Path, dirs: Sequence[Path]) -> bool:
    for d in dirs:
        try:
            p.relative_to(d)
            return True
        except ValueError:
            continue
    return False


def _pkg_dir_from_id(root: Path, pid: str) -> Optional[str]:
    """`package_id` của cargo (`path+file:///…/v2/lib/x#0.1.0`) ⇒ `dir_rel` trong repo."""
    if not pid.startswith("path+file://"):
        return None
    frag = pid[len("path+file://") :].split("#", 1)[0]
    try:
        p = Path(urllib.parse.unquote(frag)).resolve()
    except OSError:  # pragma: no cover - phòng hờ
        return None
    return _rel_or_none(root, p)


# ------------------------------------------------------------------ dấu lần build (--no-build)


def _sha256_file(p: Path) -> str:
    h = hashlib.sha256()
    try:
        with p.open("rb") as f:
            for chunk in iter(lambda: f.read(1 << 20), b""):
                h.update(chunk)
    except OSError:
        return ""
    return h.hexdigest()


def stamp_path(target_dir: Path) -> Path:
    return target_dir / STAMP_NAME


def write_stamp(target_dir: Path, unit: str, deps_dir, depinfo_files: Sequence[Path]) -> None:
    """Ghi dấu "lần build hiện tại" — danh sách `.d` **đã dùng** + sha256 + thư mục."""
    target_dir = target_dir.resolve()
    dirs = [p.resolve() for p in _as_dirs(deps_dir)]
    data = {
        "version": STAMP_VERSION,
        "unit": unit,
        "deps_dirs": [str(p) for p in dirs],
        "files": {p.name: _sha256_file(p) for p in sorted(depinfo_files)},
    }
    try:
        target_dir.mkdir(parents=True, exist_ok=True)
        stamp_path(target_dir).write_text(
            json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
    except OSError:  # pragma: no cover - target dir lạ
        pass


def verify_stamp(
    target_dir: Path, unit: str, deps_dir, present: Set[str]
) -> Tuple[Optional[Set[str]], List[str]]:
    """Kiểm dấu lần build cho `--no-build` ⇒ `(tập .d hiện hành, lỗi)`.

    Không có dấu / dấu lệch / `.d` đã đổi nội dung ⇒ lỗi (fail-closed). Chỉ `.d` **có trong
    dấu** mới được đọc — chứng minh `.d` cũ (và nguồn `OUT_DIR` cũ mà nó trỏ tới) không bị đọc.
    """
    target_dir = target_dir.resolve()
    dir_paths = [p.resolve() for p in _as_dirs(deps_dir)]
    dirs = {str(p) for p in dir_paths}
    sp = stamp_path(target_dir)
    if not sp.is_file():
        return None, [
            f"  đơn vị `{unit}`: không có dấu lần build `{sp}` — `--no-build` không chứng minh "
            f"được `.d` là của lần build hiện tại (fail-closed, KT-238)"
        ]
    try:
        data = json.loads(sp.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None, [f"  đơn vị `{unit}`: dấu lần build hỏng `{sp}` (fail-closed, KT-238)"]
    errs: List[str] = []
    if data.get("version") != STAMP_VERSION:
        errs.append(f"  đơn vị `{unit}`: dấu lần build sai phiên bản (fail-closed, KT-238)")
    if set(data.get("deps_dirs") or []) != dirs:
        errs.append(
            f"  đơn vị `{unit}`: dấu lần build thuộc deps dir khác "
            f"(`{sorted(data.get('deps_dirs') or [])}` ≠ `{sorted(dirs)}`) — fail-closed (KT-238)"
        )
    files: Dict[str, str] = data.get("files") or {}
    missing = sorted(set(files) - present)
    if missing:
        errs.append(
            f"  đơn vị `{unit}`: `.d` trong dấu không còn trên đĩa: {', '.join(missing[:5])} "
            f"— fail-closed (KT-238)"
        )
    for name, want in sorted(files.items()):
        p = next((d / name for d in dir_paths if (d / name).is_file()), None)
        if p is None:
            continue
        got = _sha256_file(p)
        if got != want:
            errs.append(
                f"  đơn vị `{unit}`: `.d` đã đổi từ lần build có dấu (`{name}`) — fail-closed "
                f"(KT-238)"
            )
    return set(files), errs


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


def _join_continuations(text: str) -> str:
    """**KT-227 P0-2** — nối dòng make dep-info: dòng kết thúc bằng **số lẻ** dấu `\\`.

    Vì sao đếm số lẻ: `\\\\` (hai dấu) là **một** backslash literal trong tên tệp, không phải
    dòng nối. Chỉ dấu `\\` **lẻ** cuối dòng mới escape newline. Bản cũ dùng
    `text.replace("\\\\\\n", " ")` nên một tên tệp kết thúc bằng `\\` bị nối nhầm.
    """
    out: List[str] = []
    buf = ""
    for raw in text.split("\n"):
        line = raw[:-1] if raw.endswith("\r") else raw
        k = len(line) - len(line.rstrip("\\"))
        if k % 2 == 1:
            buf += line[:-1] + " "
            continue
        out.append(buf + line)
        buf = ""
    if buf:
        out.append(buf)
    return "\n".join(out)


def _split_dep_tokens(s: str) -> List[str]:
    """**KT-227 P0-2** — tách danh sách phụ thuộc theo khoảng trắng, **tôn trọng** escape.

    Vì sao không `s.split()`: tên tệp có dấu cách được make escape thành `a\\ b.rs`; `split()`
    cắt thành `a\\` + `b.rs` ⇒ không bao giờ resolve được (đường lọt B10/B15 của KT-224).
    """
    out: List[str] = []
    cur: List[str] = []
    i = 0
    n = len(s)
    while i < n:
        ch = s[i]
        if ch == "\\" and i + 1 < n and s[i + 1] in (" ", "\\", "#"):
            cur.append(s[i + 1])
            i += 2
            continue
        if ch.isspace():
            if cur:
                out.append("".join(cur))
                cur = []
            i += 1
            continue
        cur.append(ch)
        i += 1
    if cur:
        out.append("".join(cur))
    return out


def parse_depinfo(path: Path) -> Tuple[Set[str], List[str]]:
    """Đọc một tệp `.d` ⇒ (tập **đích** của tệp, danh sách **phụ thuộc**).

    Định dạng make: `<đích>: <phụ thuộc> <phụ thuộc> …`, nối dòng bằng `\\` + newline. Tệp `.d`
    của rustc có **nhiều** dòng đích (chính `.d`, `.rmeta`/binary, rồi từng tệp nguồn làm đích
    rỗng) — gom hết phụ thuộc của mọi dòng, gom hết đích để đối chiếu với artifact của cargo.

    **KT-227 P0-2** — dòng bắt đầu bằng `#` là **chú thích** của make (`# env-dep:OUT_DIR=…`);
    bản cũ coi `OUT_DIR=…` là phụ thuộc. Escape `\\ ` được xử lý **trong lúc tách token**, không
    tách trước rồi mới giải.
    """
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return set(), []
    text = _join_continuations(text)
    targets: Set[str] = set()
    deps: List[str] = []
    for line in text.splitlines():
        if line.startswith("#"):
            continue
        if ":" not in line:
            continue
        lhs, _, rhs = line.partition(":")
        lhs = lhs.strip()
        if lhs:
            targets.add(_unescape_dep(lhs))
        deps.extend(_split_dep_tokens(rhs))
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

    __slots__ = (
        "name",
        "cmd",
        "rc",
        "seconds",
        "artifacts",
        "depinfo_used",
        "loaded",
        "skipped",
        "note",
        "pkg_artifacts",
        "generated",
        "depinfo_files",
        "target_dir",
    )

    def __init__(self, name: str, cmd: Sequence[str]) -> None:
        self.name = name
        self.cmd = list(cmd)
        self.rc = 0
        self.seconds = 0.0
        self.artifacts = 0
        self.depinfo_used = 0
        self.loaded: Set[Path] = set()
        # **KT-227 P0-3** — tệp rustc nạp mà công cụ **không quét** (ngoài gốc repo / thư mục
        # build / không phải tệp). Trước bản này chúng bị bỏ **im lặng** — đúng lớp "xanh-giả".
        # **KT-238**: mọi mục ở đây đều là **ĐỎ** (không còn `W_DEPINFO_SKIPPED`).
        self.skipped: List[Tuple[str, str]] = []
        self.note = ""
        # **KT-238** — artifact của lần chạy ⇒ `dir_rel` crate (từ `package_id`), để quy tệp sinh
        # trong target dir về đúng crate.
        self.pkg_artifacts: Dict[str, str] = {}
        # Tệp sinh trong target dir ⇒ `dir_rel` crate (None = không quy được ⇒ từ chối rõ).
        self.generated: Dict[Path, Optional[str]] = {}
        # `.d` **đã dùng** của lần chạy này (ghi vào dấu cho `--no-build`).
        self.depinfo_files: List[Path] = []
        self.target_dir: Optional[Path] = None


def _unit_pkg(
    res: UnitResult, targets: Set[str], d: Path, scope: Optional[Scope]
) -> Optional[str]:
    """Crate của một tệp `.d`: ưu tiên artifact của lần chạy, dự phòng theo tên crate trong `.d`."""
    pkgs = {res.pkg_artifacts[t] for t in targets if t in res.pkg_artifacts}
    if len(pkgs) == 1:
        return next(iter(pkgs))
    stem = d.name[:-2] if d.name.endswith(".d") else d.name
    base = stem.rsplit("-", 1)[0]
    if scope is not None:
        c = scope.by_target.get(base)
        if c is not None:
            return c.dir_rel
    return None



def depinfo_dirs(target_dir: Path, triple: Optional[str], artifacts: Iterable[str]) -> List[Path]:
    """Thư mục chứa tệp `.d` — suy từ **artifact của lần build hiện tại**.

    **KT-242 (A0 duyệt 2026-11-20)** — cargo ≥ 1.100 đổi **bố cục artifact**: `.d` nằm ở
    `<target>/debug/build/<pkg>/<hash>/out/` thay vì `<target>/debug/deps/`. Không được chọn
    thư mục **theo việc nó tồn tại**: sau nhiều lần build, cả hai bố cục có thể cùng có mặt và
    cách đó sẽ đọc `.d` cũ hoặc bỏ sót nguồn mới.

    Cách đúng: mỗi artifact cargo báo (đường dẫn tuyệt đối) nằm **cùng thư mục** với `.d` của
    unit đó ở **cả hai** bố cục ⇒ lấy `dirname(artifact)`. Nhờ vậy:
      - gắn với **lần build và cấu hình đang kiểm** (danh sách artifact của chính lần chạy);
      - hai bố cục cùng tồn tại vẫn lấy đúng tệp của lần này;
      - build đỏ (không có artifact) ⇒ quét **cả hai** bố cục (superset, fail-closed) — tầng
        gọi đã báo `E_DEPINFO_BUILD`.
    """
    base = target_dir / triple if triple else target_dir
    dirs: List[Path] = []
    for a in artifacts:
        p = Path(a)
        if not p.is_absolute():
            continue
        d = p.parent
        if d.is_dir() and d not in dirs:
            dirs.append(d)
    if not dirs:
        for cand in (base / "debug/deps", base / "debug/build"):
            if cand.is_dir():
                dirs.append(cand)
    return dirs


def _as_dirs(d) -> List[Path]:
    """Chuẩn hoá tham số thư mục: một `Path` hoặc danh sách `Path`."""
    if isinstance(d, (list, tuple)):
        return [Path(x) for x in d]
    return [Path(d)]


def stamp_deps_dirs(target_dir: Path) -> List[Path]:
    """Thư mục `.d` đã ghi trong **dấu lần build** (dùng cho `--no-build`)."""
    sp = stamp_path(target_dir.resolve())
    try:
        data = json.loads(sp.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    if data.get("version") != STAMP_VERSION:
        return []
    return [Path(p) for p in data.get("deps_dirs") or []]


def depinfo_glob(deps_dir: Path) -> List[Path]:
    """Danh sách `.d` trong `deps_dir` — đệ quy khi là `build/` (bố cục cargo ≥ 1.100)."""
    pattern = "**/*.d" if deps_dir.name == "build" else "*.d"
    return sorted(deps_dir.glob(pattern))


def collect_depinfo(
    deps_dir: Union[Path, Sequence[Path]],
    ws_root: Path,
    root: Path,
    res: UnitResult,
    use_all: bool,
    artifacts: Set[str],
    scope: Optional[Scope] = None,
    target_dirs: Sequence[Path] = (),
    only_files: Optional[Set[str]] = None,
) -> None:
    """**KT-227 P0-3 / KT-238** — đọc `.d` trong `deps_dir` ⇒ `res.loaded` + `res.skipped`.

    Dùng chung cho đường build (`run_unit`) và `--no-build` để hai đường không lệch nhau.

    **KT-238** — ba thay đổi so với KT-227:
      - tệp trong **target dir của cargo metadata** (nguồn sinh `$OUT_DIR`) **được nạp + quét**,
        không còn bị bỏ qua với `W_DEPINFO_SKIPPED` (WB-18). Tệp sinh được quy về crate qua
        artifact của lần chạy (`package_id`); không quy được ⇒ `E_DEPINFO_SCOPE`;
      - tệp dưới một thư mục tên `target` **không phải** target dir của metadata ⇒ **từ chối rõ**
        (`E_DEPINFO_SCOPE`), không im lặng;
      - mọi mục `skipped` (không resolve / không phải tệp / ngoài gốc repo) ⇒ tầng gọi báo **ĐỎ**;
        không còn cảnh báo-rồi-PASS.
      - `only_files` (dùng cho `--no-build`): chỉ đọc `.d` có trong **dấu lần build**.
    """
    dirs = [d for d in _as_dirs(deps_dir) if d.is_dir()]
    if not dirs:
        res.note = f"không có thư mục dep-info {deps_dir}"
        return
    seen_skip: Set[Tuple[str, str]] = set()

    def skip(path: str, why: str) -> None:
        key = (path, why)
        if key not in seen_skip:
            seen_skip.add(key)
            res.skipped.append(key)

    dep_files: List[Path] = []
    for dd in dirs:
        dep_files.extend(depinfo_glob(dd))
    for d in dep_files:
        if not RE_DEPINFO.search(d.name):
            continue
        if only_files is not None and d.name not in only_files:
            continue
        targets, deps = parse_depinfo(d)
        if not use_all and not (targets & artifacts):
            continue
        res.depinfo_used += 1
        res.depinfo_files.append(d)
        unit_pkg = _unit_pkg(res, targets, d, scope)
        for tok in deps:
            p = resolve_dep(tok, ws_root)
            if p is None:
                skip(tok, "không resolve được")
                continue
            if not p.is_file():
                skip(tok, "không phải tệp")
                continue
            try:
                relp = p.relative_to(root)
            except ValueError:
                skip(str(p), "ngoài gốc repo")
                continue
            if _under_any(p, target_dirs):
                # **KT-238** — nguồn sinh trong target dir của **lần build này**: nạp + quét.
                res.loaded.add(p)
                res.generated[p] = unit_pkg
                continue
            if CWP._is_build_dir(root, p):
                # Thư mục tên `target` nhưng **không** phải target dir của cargo metadata ⇒ từ
                # chối rõ (không im lặng, không quét-rồi-bỏ-qua).
                skip(str(relp), "thư mục build lạ (không phải target dir của cargo metadata)")
                continue
            res.loaded.add(p)


def depinfo_empty_errors(results: Sequence[UnitResult], loaded: Set[Path]) -> List[str]:
    """**KT-227 P0-1** — fail-closed khi `.d` rỗng/thiếu/không cho tệp nào.

    Trước bản này: `.d` rỗng hoặc không tồn tại ⇒ `errors=0`, RC=0, **0 tệp quét** — xanh-giả
    đúng lớp lỗi repo đang chống. Nay mọi trường hợp "không đọc được tệp nào" là ĐỎ.
    """
    errs: List[str] = []
    if not loaded:
        errs.append(
            "  không đọc được tệp nguồn nào từ `.d` — fail-closed (KT-227 P0-1); "
            "kiểm `target/`, `--no-build`, bộ lọc artifact"
        )
    for res in results:
        if res.depinfo_used == 0:
            errs.append(
                f"  đơn vị `{res.name}`: không có tệp `.d` nào được dùng — fail-closed "
                f"(KT-227 P0-1)"
            )
        elif not res.loaded:
            errs.append(
                f"  đơn vị `{res.name}`: `.d` không cho tệp nguồn nào — fail-closed "
                f"(KT-227 P0-1)"
            )
    return errs


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
    scope: Optional[Scope] = None,
) -> UnitResult:
    """Chạy `cargo check` cho một đơn vị và trả kết quả (không tự đỏ khi RC≠0 — tầng gọi quyết).

    **KT-238** — target dir lấy từ `cargo metadata` (`scope.target_dirs[name]`), không từ hằng
    `target_rel`: đó là **cùng** thư mục cargo dùng cho lần build này (kể cả `CARGO_TARGET_DIR`
    của người dùng / `--fresh`). `target_rel` chỉ còn là dự phòng khi metadata hỏng (tầng gọi
    báo `E_DEPINFO_SCOPE`).
    """
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
            # **KT-238** — `package_id` ⇒ crate của artifact, để quy tệp sinh về đúng crate.
            dir_rel = _pkg_dir_from_id(root, msg.get("package_id") or "")
            for fn in msg.get("filenames") or []:
                key = str(Path(fn))
                artifacts.add(key)
                if dir_rel is not None:
                    res.pkg_artifacts[key] = dir_rel
        elif reason == "build-finished":
            build_ok = bool(msg.get("success"))
    res.artifacts = len(artifacts)

    # --- đọc `.d` --------------------------------------------------------------------------
    target_dir: Optional[Path] = None
    if scope is not None:
        target_dir = scope.target_dirs.get(name)
    if target_dir is None:
        target_dir = (root / target_rel).resolve()
        res.note = "không có target dir từ cargo metadata — dùng dự phòng (fail-closed)"
    res.target_dir = target_dir
    deps_dirs = depinfo_dirs(target_dir, triple, artifacts)
    ws_root = (root / manifest).parent

    # Khi build **thành công**: chỉ lấy `.d` của unit có mặt trong lần chạy này (chính xác).
    # Khi build **thất bại**: cargo **không** phát `compiler-artifact` cho unit hỏng, nhưng rustc
    # **vẫn ghi** `.d` (đã đo với lỗi kiểu E0308 và E0583) ⇒ lấy **mọi** `.d` trong thư mục
    # (superset, fail-closed) và báo `E_DEPINFO_BUILD` ở tầng gọi.
    use_all = (build_ok is False) or (not artifacts)
    if build_ok is False:
        res.note = "build ĐỎ — dùng mọi .d trong thư mục (superset, fail-closed)"

    collect_depinfo(
        deps_dirs,
        ws_root,
        root,
        res,
        use_all,
        artifacts,
        scope=scope,
        target_dirs=(target_dir,),
    )
    # **KT-238** — dấu "lần build hiện tại" cho `--no-build`: chỉ ghi khi build xanh và dùng
    # target dir thật (không ghi vào target dir tạm của `--fresh`).
    if res.rc == 0 and fresh_dir is None and build_ok is not False:
        write_stamp(target_dir, name, deps_dirs, res.depinfo_files)
    return res


# ------------------------------------------------------------------ luật

def is_producer_file(text: str) -> bool:
    """Tệp **định nghĩa** một cửa phát `Wake` (`WAKE_PRODUCER`).

    **KT-227 P0-3 / KT-226** — chỉ dùng để **đếm** (`producer_files`), **KHÔNG** miễn cả tệp.
    Việc miễn **định nghĩa** `fn wake` do `CWP._check_wake_text` làm; miễn cả tệp là bất đối
    xứng với lớp 1 (đường lọt B18 của KT-224).
    """
    for name in CWP.WAKE_PRODUCER:
        if re.search(r"\bfn\s+" + re.escape(name) + r"\s*\(", text):
            return True
    return False


def hidden_files(
    root: Path, loaded: Iterable[Path], scope: Optional[Scope] = None
) -> List[str]:
    """Tệp rustc nạp mà **không** nằm dưới `<crate>/src/` — chỉ tới được qua `#[path]`/`include!`.

    **KT-238** — khi có `scope` (sinh từ `cargo metadata`), "crate" là **crate thật** của tệp,
    không phải mẫu đường dẫn cố định.

    **KT-256 (L2)** — không có `scope` ⇒ dựng từ `wake_scope.protected_scope(root)` (định nghĩa
    duy nhất), **không** còn mẫu `RE_CRATE_SRC`/`RE_BUILD_RS` (đã xoá).
    """
    if scope is None:
        scope = protected_scope(root)
    out: List[str] = []
    for p in loaded:
        relp = CWP.rel(root, p)
        c = scope.classify(relp)
        if c is not None and (
            relp == c.dir_rel + "/build.rs" or relp.startswith(c.dir_rel + "/src/")
        ):
            continue
        out.append(relp)
    return sorted(out)


def outside_static_scope(root: Path, loaded: Iterable[Path]) -> List[str]:
    """Tệp rustc nạp mà luật `wake` **tĩnh** không mở (không khớp `in_test_only_wake_scope`)."""
    return sorted(
        CWP.rel(root, p)
        for p in loaded
        if not CWP.in_test_only_wake_scope(CWP.rel(root, p), root)
    )


def scan(
    root: Path,
    loaded: Iterable[Path],
    test_only: Optional[Set[str]] = None,
    do_transitions: bool = True,
    scope: Optional[Scope] = None,
    generated: Optional[Dict[Path, Optional[str]]] = None,
) -> Tuple["CWP.Report", Dict[str, int]]:
    """Áp R1 (`wake`) + R2 (chuyển `Ready`) trên **danh sách tệp rustc nạp**.

    Trả `(Report, đếm)`. `test_only` = tập relpath test-only (đồ thị module); `None` ⇒ không miễn
    theo test (dùng cho `--selftest` khi cây fixture không có `Cargo.toml`).

    **KT-238** — `scope` (sinh từ `cargo metadata`) quyết định:
      - tệp **không thuộc crate nào** (và không phải tệp sinh trong target dir) ⇒
        `E_DEPINFO_SCOPE` — **từ chối rõ**, không quét-rồi-PASS;
      - tệp sinh trong target dir mà **không quy được** về crate ⇒ `E_DEPINFO_SCOPE`;
      - R2 áp cho **mọi** tệp của crate bảo vệ + tệp sinh + tệp không rõ phạm vi (fail-closed);
        crate "từ chối rõ" chỉ áp R1 (nó không thể gọi API `wake` — bao đóng dependency đã kiểm).
    `scope=None` ⇒ giữ hành vi cũ (R2 chỉ trên `v2/kernel`/`v2/sim`) — dùng cho selftest cũ.
    """
    r = CWP.Report()
    files = sorted(loaded)
    gen = generated or {}
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

    counts = {"loaded": len(files), "scanned": 0, "producer_files": 0, "exempt_test": 0}
    for p in files:
        text = texts.get(p)
        if text is None:
            continue
        relp = CWP.rel(root, p)
        if test_only is not None and relp in test_only:
            counts["exempt_test"] += 1
            continue
        # **KT-238** — phạm vi: tệp rustc nạp phải quy được về crate (hoặc là tệp sinh trong
        # target dir của cargo metadata). Không quy được ⇒ **từ chối rõ** (không im lặng).
        if scope is not None:
            if p in gen:
                if gen.get(p) is None:
                    r.err(
                        "E_DEPINFO_SCOPE",
                        f"  {relp}: tệp sinh trong target dir nhưng KHÔNG quy được về crate nào "
                        f"trong cargo metadata — từ chối rõ (fail-closed, KT-238)",
                    )
            elif scope.classify(relp) is None:
                r.err(
                    "E_DEPINFO_SCOPE",
                    f"  {relp}: tệp rustc nạp KHÔNG thuộc crate nào trong cargo metadata "
                    f"(workspace + bao đóng path-dependency) — từ chối rõ (fail-closed, KT-238)",
                )
        # **KT-227 P0-3 / KT-226** — KHÔNG miễn **cả tệp** producer nữa: lớp 2 chỉ miễn
        # **định nghĩa** `fn wake` (do `_check_wake_text` xử lý), đồng nhất với lớp 1.
        # Trước KT-226/KT-227 tệp định nghĩa `fn wake` được miễn cả tệp ⇒ lời gọi `wake(`
        # trong tệp đó XANH ở lớp 2 nhưng ĐỎ ở lớp 1 (bất đối xứng allowlist — reviewer L51
        # KT-222 §4; đường lọt B18 của KT-224).
        if is_producer_file(text):
            counts["producer_files"] += 1
        counts["scanned"] += 1
        # R1 — luật token `wake` của checker tĩnh, **nguyên văn** (không chép).
        sub = CWP.Report()
        # **KT-241** — không truyền vùng test: `_check_wake_text` **tự** tính
        # `_test_char_ranges` (một nơi sở hữu phân vùng; tham số `tr` cũ bị bỏ qua).
        CWP._check_wake_text(relp, text, safe_macros, sub)
        for e in sub.errors:
            # `Report.err` lưu `"<mã> <thông điệp>"`; đổi mã sang mã của lớp dep-info.
            msg = e.split(" ", 1)[1] if " " in e else e
            r.err("E_DEPINFO_WAKE", msg)

    if do_transitions:
        # R2 — **KT-241 (D2-194 §3)**: dùng **cùng** bộ quét token-tree của lớp một
        # (`CWP._transition_calls`), không regex văn bản. Dưới API mới (`to: NonReadyTarget`)
        # đối số thứ hai **không thể** là `Ready`; luật còn lại là **chốt chống đảo kiểu**:
        #   - `tcbstate` (đối số thứ hai là/chứa `TcbState`) ⇒ **ĐỎ**;
        #   - `ready_alias` (đối số thứ hai kết thúc bằng/chứa `…::Ready`) ⇒ **ĐỎ**;
        #   - `ok` (mọi dạng khác: `NonReadyTarget::*`, biến, lời gọi hàm) ⇒ XANH.
        # **Bỏ** `_var_may_be_ready` và `NON_WAKE_READY_SITES` (allowlist cũ): câu hỏi "biến
        # này có thể là Ready không" **không còn nghĩa** khi kiểu đã loại trừ `Ready`, và
        # allowlist chỉ tồn tại để miễn những chỗ `transition_tcb(…, Ready)` hợp lệ — dưới API
        # mới **không còn chỗ nào như vậy**.
        # **KT-238** — phạm vi R2 sinh từ `cargo metadata`: mọi tệp của crate **bảo vệ** + tệp
        # sinh + tệp không rõ phạm vi. Crate "từ chối rõ" (công cụ, không phụ thuộc sản phẩm)
        # chỉ áp R1 — nó không thể gọi `transition_tcb` vì bao đóng dependency không có crate
        # sản phẩm nào (đã kiểm ở `_classify_scope`).
        for p in files:
            text = texts.get(p)
            if text is None:
                continue
            relp = CWP.rel(root, p)
            if scope is not None:
                crate = scope.classify(relp)
                if crate is not None and crate.status == "refused":
                    continue
            elif not (relp.startswith("v2/kernel/") or relp.startswith("v2/sim/")):
                continue
            if test_only is not None and relp in test_only:
                continue
            # **KT-242 (N5)** — vùng test là **khoảng KÝ TỰ** (`_test_char_ranges`,
            # token-tree), không phải khoảng dòng: `test_line_ranges` lan cả dòng nên miễn
            # mã sản phẩm **cùng dòng** một mục `#[cfg(test)]`. Dùng **cùng** hàm của lớp
            # một (import, không chép luật).
            tr = CWP._test_char_ranges(CWP.strip_rust_noise(text), relp)
            fns = CWP.find_functions(p, text)
            for lineno, kind, desc, _start in CWP._transition_calls(text, relp):
                if CWP.in_ranges(_start, tr):
                    continue
                if kind == "ok":
                    continue
                fn = CWP.enclosing(fns, lineno)
                where = f" trong `{fn.name}`" if fn is not None else ""
                if kind == "tcbstate":
                    r.err(
                        "E_DEPINFO_TRANSITION",
                        f"  {relp}:{lineno}: `transition_tcb(…, {desc})`{where} — đối số thứ "
                        f"hai kiểu `TcbState`; API KT-241 đã thu hẹp sang `NonReadyTarget` ⇒ "
                        f"kiểu bị đảo ngược (fail-closed, D2-194 §3)",
                    )
                else:  # ready_alias
                    r.err(
                        "E_DEPINFO_TRANSITION",
                        f"  {relp}:{lineno}: `transition_tcb(…, {desc})`{where} — đối số thứ "
                        f"hai là `…::Ready` (alias/đường dẫn lạ); `NonReadyTarget` **không "
                        f"có** biến thể `Ready` ⇒ không chứng minh được (fail-closed, KT-241)",
                    )

        # R3 — **KT-241**: `write_state`/`into_state` **chỉ** sống trong `state_write.rs`;
        # trong đó `write_state` **chỉ** được gọi bởi `CWP.STATE_WRITE_CALLERS`. Dùng **cùng**
        # `CWP._check_state_write` của lớp một (import, không chép luật) ⇒ hai lớp không lệch.
        for p in files:
            text = texts.get(p)
            if text is None:
                continue
            relp = CWP.rel(root, p)
            if scope is not None:
                crate = scope.classify(relp)
                if crate is not None and crate.status == "refused":
                    continue
            elif not (relp.startswith("v2/kernel/") or relp.startswith("v2/sim/")):
                continue
            if test_only is not None and relp in test_only:
                continue
            sub = CWP.Report()
            # **KT-242 (N1/N2 + N5)** — `_check_state_write` nay nhận **khoảng ký tự** và tự
            # quy `write_state` cho **hàm trong cùng theo token-tree** (bỏ tham số `fns`).
            CWP._check_state_write(
                relp,
                text,
                CWP._test_char_ranges(CWP.strip_rust_noise(text), relp),
                None,
                sub,
            )
            for e in sub.errors:
                msg = e.split(" ", 1)[1] if " " in e else e
                r.err("E_DEPINFO_STATE_WRITE", msg)

        # R4 — **KT-241 (E1/E2)**: cấm mọi phép chuyển `TcbState` ⇄ `NonReadyTarget` ngoài
        # `into_state` private. Dùng **cùng** `CWP._check_nonready_conv_file` của lớp một
        # (import, không chép luật). Áp cho **mọi** tệp rustc nạp (kể cả `v2/lib` — rộng hơn
        # phạm vi luật `wake`), vì orphan rule cho phép impl **bên trong** crate.
        for p in files:
            text = texts.get(p)
            if text is None:
                continue
            relp = CWP.rel(root, p)
            if test_only is not None and relp in test_only:
                continue
            sub = CWP.Report()
            CWP._check_nonready_conv_file(relp, text, sub)
            for e in sub.errors:
                msg = e.split(" ", 1)[1] if " " in e else e
                r.err("E_DEPINFO_NONREADY_CONV", msg)
    return r, counts


# ------------------------------------------------------------------ selftest

_GIA_LIB = """// SPDX-License-Identifier: MIT OR Apache-2.0
pub struct Machine;
pub type ObjId = u32;
"""

# **KT-241** — `wake_step` **rời** `core_sched.rs` sang `state_write.rs` (API-SPEC §3).
# `core_sched.rs` chỉ còn các **cửa sản phẩm phát `Wake`** (`WAKE_PRODUCER`) — đúng hình
# dạng sau khi A/B hợp nhất.
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
}
pub enum TcbState {
    Ready,
}
"""

# **KT-241** — `state_write.rs` giả theo API-SPEC: enum `NonReadyTarget` (không `Ready`),
# `write_state`/`into_state` private, cửa thông thường + 5 cửa `Ready` + `tcb_yield`.
_GIA_STATE_WRITE = """// SPDX-License-Identifier: MIT OR Apache-2.0
pub enum NonReadyTarget {
    Inactive,
    Running { hart: u16 },
    BlockedOnSend { ep: u32, call: bool, fault: Option<u8> },
    BlockedOnRecv { ep: u32, reply: Option<u32> },
    BlockedOnReply { reply: u32 },
    BlockedOnNotification { ntfn: u32 },
    BlockedOnAddress { frame: u32, offset: u32 },
    Sleeping { reason: u8 },
    Stopping,
    Dead,
}
impl NonReadyTarget {
    fn into_state(self) -> TcbState {
        match self {
            NonReadyTarget::Inactive => TcbState::Inactive,
            NonReadyTarget::Running { hart } => TcbState::Running { hart },
            NonReadyTarget::BlockedOnSend { ep, call, fault } => TcbState::BlockedOnSend { ep, call, fault },
            NonReadyTarget::BlockedOnRecv { ep, reply } => TcbState::BlockedOnRecv { ep, reply },
            NonReadyTarget::BlockedOnReply { reply } => TcbState::BlockedOnReply { reply },
            NonReadyTarget::BlockedOnNotification { ntfn } => TcbState::BlockedOnNotification { ntfn },
            NonReadyTarget::BlockedOnAddress { frame, offset } => TcbState::BlockedOnAddress { frame, offset },
            NonReadyTarget::Sleeping { reason } => TcbState::Sleeping { reason },
            NonReadyTarget::Stopping => TcbState::Stopping,
            NonReadyTarget::Dead => TcbState::Dead,
        }
    }
}
impl Machine {
    fn write_state(&mut self, _t: u32, _to: TcbState) {
        let _ = (_t, _to);
    }
    pub fn transition_tcb(&mut self, _t: u32, to: NonReadyTarget) {
        self.write_state(_t, to.into_state());
    }
    pub fn wake_step(&mut self, _t: u32) {
        self.write_state(_t, TcbState::Ready);
    }
    pub fn preempt_current(&mut self, _t: u32) {
        self.write_state(_t, TcbState::Ready);
    }
    pub fn yield_to_ready(&mut self, _t: u32) {
        self.write_state(_t, TcbState::Ready);
    }
    pub fn tcb_yield(&mut self, _t: u32) {
        self.yield_to_ready(_t);
    }
    pub fn start_tcb(&mut self, _t: u32) {
        self.write_state(_t, TcbState::Ready);
    }
    pub fn suspend_to_ready(&mut self, _t: u32) {
        self.write_state(_t, TcbState::Ready);
    }
    pub fn block_current(&mut self, _h: u16, to: NonReadyTarget) {
        self.transition_tcb(1, to);
    }
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
    """Dựng cây v2 tối thiểu: crate kernel có `core_sched.rs` (cửa phát `Wake`),
    `state_write.rs` (API KT-241) + tệp sạch."""
    root = base / "repo"
    src = root / "v2/kernel/vifast-core/src"
    src.mkdir(parents=True, exist_ok=True)
    (src / "lib.rs").write_text(_GIA_LIB, encoding="utf-8")
    (src / "core_sched.rs").write_text(_GIA_SCHED, encoding="utf-8")
    (src / "state_write.rs").write_text(_GIA_STATE_WRITE, encoding="utf-8")
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
        loaded = {src / "lib.rs", src / "core_sched.rs", src / "state_write.rs", src / "clean.rs"}
        rep, counts = scan(root, loaded, test_only=set())
        ghi(
            "(5) cây sạch (chỉ tệp trong src/) ⇒ XANH",
            rep.ok and counts["scanned"] == 4 and counts["producer_files"] == 1,
            f"errors={rep.errors} counts={counts}",
        )
        ghi(
            "(6) tệp định nghĩa `fn wake` VẪN được quét (chỉ miễn định nghĩa, KT-227 P0-3)",
            counts["producer_files"] == 1 and counts["scanned"] == 4,
            f"producer_files={counts['producer_files']} scanned={counts['scanned']}",
        )
        # **KT-226** — bất đối xứng allowlist producer: lớp 2 KHÔNG miễn cả tệp nữa. Tệp định
        # nghĩa `fn wake` mà **gọi** `wake(` phải ĐỎ (trước KT-226: XANH ở lớp 2, ĐỎ ở lớp 1).
        prod_call = src / "prod_call.rs"
        prod_call.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn wake(_t: u32) {}\n"
            "pub fn call_it() {\n"
            "    wake(1);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {prod_call}, test_only=set())
        ghi(
            "(6b) tệp producer **gọi** `wake(` ⇒ ĐỎ (đồng nhất lớp 1; trước KT-226: XANH)",
            (not rep.ok) and any("E_DEPINFO_WAKE" in e for e in rep.errors),
            f"errors={rep.errors}",
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
        # **KT-256 (L2)** — lỗ cũ **đã đóng**: phạm vi luật token sinh từ `cargo metadata` nên tệp
        # dưới crate bảo vệ nhưng **ngoài `src/`** nay **thuộc** phạm vi tĩnh (trước KT-256 mẫu
        # `src/.*\.rs|build\.rs` bỏ sót ⇒ lớp tĩnh XANH, lớp dep-info ĐỎ — bất đối xứng).
        ghi(
            "(10) KT-256: tệp NGOÀI `src/` của crate bảo vệ nay THUỘC phạm vi tĩnh (lỗ cũ đã đóng)",
            CWP.in_test_only_wake_scope("v2/kernel/vifast-core/evil.rs"),
            "in_test_only_wake_scope trả False (phạm vi chưa hợp nhất?)",
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

        # --- (14) R2: đảo kiểu (`TcbState::*` làm đối số) ⇒ ĐỎ -------------------------------
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
            "(14) R2: `transition_tcb(…, TcbState::Ready)` (đảo kiểu) ⇒ ĐỎ E_DEPINFO_TRANSITION",
            (not rep.ok) and any("E_DEPINFO_TRANSITION" in e for e in rep.errors),
            f"errors={rep.errors}",
        )
        ghi(
            "(15) R2: `state_write.rs` hợp lệ (5 cửa gọi `write_state`) ⇒ XANH",
            not any("E_DEPINFO_TRANSITION" in e for e in scan(root, loaded, test_only=set())[0].errors),
            "cửa trong state_write.rs bị báo oan",
        )

        # --- (16) R2: `NonReadyTarget::*` (đường hợp lệ duy nhất) ⇒ XANH ---------------------
        # **KT-241** — thay ca cũ "mục trong `NON_WAKE_READY_SITES` ⇒ XANH": allowlist đó
        # **đã xoá** (dưới API mới không còn chỗ `transition_tcb(…, Ready)` hợp lệ để miễn).
        # Đối chứng dương nay là **đối số kiểu `NonReadyTarget`** — dạng duy nhất biên dịch được.
        allow = src / "allow_ready.rs"
        allow.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn block_current(_h: u16, to: NonReadyTarget) {\n"
            "    transition_tcb(1, to);\n"
            "    transition_tcb(1, NonReadyTarget::Dead);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {allow}, test_only=set())
        ghi(
            "(16) R2: `NonReadyTarget::*`/biến `to` ⇒ XANH (đường hợp lệ duy nhất)",
            not any("E_DEPINFO_TRANSITION" in e for e in rep.errors),
            f"errors={rep.errors}",
        )
        # **KT-241** — R2 vẫn ĐỎ với `TcbState::<BiếnThểKhác>` (mọi `TcbState::*` là đảo kiểu).
        bad2 = src / "bad_other.rs"
        bad2.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn hop_le() {\n"
            "    transition_tcb(1, TcbState::Blocked);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {bad2}, test_only=set())
        ghi(
            "(16b) R2: `TcbState::Blocked` cũng ĐỎ (mọi `TcbState::*` = đảo kiểu)",
            (not rep.ok) and any("E_DEPINFO_TRANSITION" in e for e in rep.errors),
            f"errors={rep.errors}",
        )

        # --- (16c) R3: `write_state` ngoài `state_write.rs` ⇒ ĐỎ -----------------------------
        bad3 = src / "bad_ws.rs"
        bad3.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn goi_thang(m: &mut crate::Machine) {\n"
            "    m.write_state(1, TcbState::Ready);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {bad3}, test_only=set())
        ghi(
            "(16c) R3: `write_state` ngoài `state_write.rs` ⇒ ĐỎ E_DEPINFO_STATE_WRITE",
            (not rep.ok) and any("E_DEPINFO_STATE_WRITE" in e for e in rep.errors),
            f"errors={rep.errors}",
        )
        # --- (16d) R3: `into_state` ngoài `state_write.rs` ⇒ ĐỎ ------------------------------
        bad4 = src / "bad_is.rs"
        bad4.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn doi(t: NonReadyTarget) -> TcbState {\n"
            "    t.into_state()\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {bad4}, test_only=set())
        ghi(
            "(16d) R3: `into_state` ngoài `state_write.rs` ⇒ ĐỎ E_DEPINFO_STATE_WRITE",
            (not rep.ok) and any("E_DEPINFO_STATE_WRITE" in e for e in rep.errors),
            f"errors={rep.errors}",
        )
        # --- (16e) R3: `write_state` trong `tcb_yield` (phải qua `yield_to_ready`) ⇒ ĐỎ ------
        sw_bad = src / "state_write_bad.rs"
        sw_bad.write_text(
            _GIA_STATE_WRITE.replace(
                "    pub fn tcb_yield(&mut self, _t: u32) {\n"
                "        self.yield_to_ready(_t);\n"
                "    }\n",
                "    pub fn tcb_yield(&mut self, _t: u32) {\n"
                "        self.write_state(_t, TcbState::Ready);\n"
                "    }\n",
            ),
            encoding="utf-8",
        )
        # Tệp này **không** phải `state_write.rs` ⇒ mọi `write_state` đều ĐỎ; ca riêng dưới
        # kiểm đúng ngữ nghĩa "trong `state_write.rs` nhưng sai cửa" bằng cách thay tệp thật.
        rep, _ = scan(root, loaded | {sw_bad}, test_only=set())
        ghi(
            "(16e) R3: bản sao `state_write` ngoài đường dẫn chuẩn ⇒ ĐỎ (không miễn theo tên)",
            (not rep.ok) and any("E_DEPINFO_STATE_WRITE" in e for e in rep.errors),
            f"errors={rep.errors}",
        )
        _sw_path = src / "state_write.rs"
        _sw_orig = _sw_path.read_text(encoding="utf-8")
        try:
            _sw_path.write_text(
                _sw_orig.replace(
                    "    pub fn tcb_yield(&mut self, _t: u32) {\n"
                    "        self.yield_to_ready(_t);\n"
                    "    }\n",
                    "    pub fn tcb_yield(&mut self, _t: u32) {\n"
                    "        self.write_state(_t, TcbState::Ready);\n"
                    "    }\n",
                ),
                encoding="utf-8",
            )
            rep, _ = scan(root, loaded, test_only=set())
        finally:
            _sw_path.write_text(_sw_orig, encoding="utf-8")
        ghi(
            "(16f) R3: `write_state` trong `tcb_yield` (trong `state_write.rs`) ⇒ ĐỎ",
            (not rep.ok) and any("E_DEPINFO_STATE_WRITE" in e for e in rep.errors),
            f"errors={rep.errors}",
        )

        # --- (16k) **KT-242 N1/N2**: `write_state` trong `impl` **lồng trong thân** một cửa --
        # Đường lọt "cửa thứ bảy": `impl Machine { pub fn kt242_door(…) { self.write_state(…) } }`
        # lồng trong `wake_step`; mã sản phẩm gọi `m.kt242_door(t)` ⇒ TCB về `Ready` **không
        # qua điều kiện nguồn nào**. Trước KT-242 lớp 2 XANH (cùng gốc `enclosing`/`find_functions`
        # với lớp 1). Nay `_check_state_write` quy cho **hàm trong cùng theo token-tree**.
        _sw_path2 = src / "state_write.rs"
        _sw_orig2 = _sw_path2.read_text(encoding="utf-8")
        try:
            _sw_path2.write_text(
                _sw_orig2.replace(
                    "    pub fn wake_step(&mut self, _t: u32) {\n"
                    "        self.write_state(_t, TcbState::Ready);\n"
                    "    }\n",
                    "    pub fn wake_step(&mut self, _t: u32) {\n"
                    "        impl Machine {\n"
                    "            pub fn kt242_door(&mut self, _t: u32) {\n"
                    "                self.write_state(_t, TcbState::Ready);\n"
                    "            }\n"
                    "        }\n"
                    "        self.write_state(_t, TcbState::Ready);\n"
                    "    }\n",
                ),
                encoding="utf-8",
            )
            rep, _ = scan(root, loaded, test_only=set())
        finally:
            _sw_path2.write_text(_sw_orig2, encoding="utf-8")
        ghi(
            "(16k) KT-242 N1/N2: `impl` lồng trong cửa gọi `write_state` ⇒ ĐỎ "
            "E_DEPINFO_STATE_WRITE",
            (not rep.ok)
            and any("E_DEPINFO_STATE_WRITE" in e for e in rep.errors)
            and any("kt242_door" in e for e in rep.errors),
            f"errors={rep.errors}",
        )

        # --- (16l) **KT-242 N5**: R2 miễn theo **ký tự**, không theo dòng --------------------
        # `test_line_ranges` cũ phủ **cả** dòng `#[cfg(test)] fn helper() {} fn evil(…) { … }`
        # ⇒ `transition_tcb(…, TcbState::Ready)` cùng dòng được miễn. Nay chỉ mục test được miễn.
        _n5 = src / "kt242_n5.rs"
        _n5.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "#[cfg(test)]\n"
            "fn helper() {} fn evil() { transition_tcb(1, TcbState::Ready); }\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {_n5}, test_only=set())
        ghi(
            "(16l) KT-242 N5: R2 cùng dòng `#[cfg(test)]` ⇒ ĐỎ E_DEPINFO_TRANSITION",
            (not rep.ok) and any("E_DEPINFO_TRANSITION" in e for e in rep.errors),
            f"errors={rep.errors}",
        )
        # Đối chứng dương: `#[cfg(test)] mod t { … }` **vẫn** được miễn.
        _n5b = src / "kt242_n5b.rs"
        _n5b.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "#[cfg(test)]\n"
            "mod t { fn evil() { transition_tcb(1, TcbState::Ready); } }\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {_n5b}, test_only=set())
        ghi(
            "(16m) KT-242 N5 đối chứng dương: `#[cfg(test)] mod t { … }` ⇒ XANH",
            not any("E_DEPINFO_TRANSITION" in e for e in rep.errors),
            f"errors={rep.errors}",
        )

        # --- (16g) R4: `impl From<TcbState> for NonReadyTarget` ⇒ ĐỎ ------------------------
        # **KT-241 (E1/E2)** — orphan rule cho phép impl **bên trong** crate ⇒ `cargo check`
        # RC=0 ⇒ đường vòng tái tạo qua ranh giới kiểu. Lớp 2 dùng **cùng** hàm của lớp 1.
        bad5 = src / "bad_conv.rs"
        bad5.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "impl From<TcbState> for NonReadyTarget {\n"
            "    fn from(_s: TcbState) -> NonReadyTarget {\n"
            "        NonReadyTarget::Dead\n"
            "    }\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {bad5}, test_only=set())
        ghi(
            "(16g) R4: `impl From<TcbState> for NonReadyTarget` ⇒ ĐỎ E_DEPINFO_NONREADY_CONV",
            (not rep.ok) and any("E_DEPINFO_NONREADY_CONV" in e for e in rep.errors),
            f"errors={rep.errors}",
        )
        # --- (16h) R4: `impl TryFrom<TcbState> for NonReadyTarget` ⇒ ĐỎ ---------------------
        bad6 = src / "bad_conv2.rs"
        bad6.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "impl TryFrom<TcbState> for NonReadyTarget {\n"
            "    type Error = ();\n"
            "    fn try_from(_s: TcbState) -> Result<Self, ()> {\n"
            "        Ok(NonReadyTarget::Dead)\n"
            "    }\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {bad6}, test_only=set())
        ghi(
            "(16h) R4: `impl TryFrom<TcbState> for NonReadyTarget` ⇒ ĐỎ E_DEPINFO_NONREADY_CONV",
            (not rep.ok) and any("E_DEPINFO_NONREADY_CONV" in e for e in rep.errors),
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

        # --- (21) KT-227 P0-2: escape `\ ` đi **qua `parse_depinfo`** ------------------------
        d = base / "space.d"
        d.write_text("libx.rmeta: src/a\\ b.rs src/c.rs\n", encoding="utf-8")
        _, deps = parse_depinfo(d)
        ghi(
            "(21) P0-2: `parse_depinfo` giữ tên tệp có dấu cách (`a\\ b.rs` ⇒ `a b.rs`)",
            deps == ["src/a b.rs", "src/c.rs"],
            f"deps={deps}",
        )

        # --- (22) KT-227 P0-2: nối dòng + `\\` + dòng chú thích `#` --------------------------
        d = base / "cont.d"
        d.write_text(
            "# env-dep:OUT_DIR=/tmp/x\n"
            "libx.rmeta: a.rs \\\n  b.rs c\\\\d.rs\n",
            encoding="utf-8",
        )
        targets, deps = parse_depinfo(d)
        ghi(
            "(22) P0-2: nối dòng `\\`, `\\\\` literal, bỏ dòng `#` (không coi OUT_DIR là phụ thuộc)",
            targets == {"libx.rmeta"} and deps == ["a.rs", "b.rs", "c\\d.rs"],
            f"targets={sorted(targets)} deps={deps}",
        )

        # --- (23) KT-227 P0-1: `.d` rỗng/thiếu ⇒ fail-closed --------------------------------
        empty_res = UnitResult("host", ["(no-build)"])
        ghi(
            "(23) P0-1: `depinfo_empty_errors` ĐỎ khi 0 tệp nạp / 0 `.d`",
            len(depinfo_empty_errors([empty_res], set())) >= 2,
            str(depinfo_empty_errors([empty_res], set())),
        )
        ok_res = UnitResult("host", ["(no-build)"])
        ok_res.depinfo_used = 1
        ok_res.loaded = {src / "clean.rs"}
        ghi(
            "(24) P0-1: `depinfo_empty_errors` rỗng khi có tệp nạp",
            depinfo_empty_errors([ok_res], ok_res.loaded) == [],
            str(depinfo_empty_errors([ok_res], ok_res.loaded)),
        )

        # --- (25) KT-227 P0-1 end-to-end: `--no-build` trên cây không `.d` ⇒ RC=1 -----------
        fake = base / "fake-root"
        (fake / "v2/kernel").mkdir(parents=True, exist_ok=True)
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = main(["--root", str(fake), "--no-build"])
        out = buf.getvalue()
        ghi(
            "(25) P0-1 end-to-end: `--no-build` không `.d` ⇒ RC=1 + `E_DEPINFO_EMPTY`",
            rc == 1 and "E_DEPINFO_EMPTY" in out,
            f"rc={rc} out={out[-160:]}",
        )

        # --- (26) KT-227 P0-3: `.txt` + `src/target/` được quét, `/tmp` bị báo --------------
        ws3 = base / "ws3"
        (ws3 / "src/target").mkdir(parents=True, exist_ok=True)
        (ws3 / "src/a.rs").write_text("// a\n", encoding="utf-8")
        (ws3 / "src/evil.txt").write_text("// evil\n", encoding="utf-8")
        (ws3 / "src/target/evil.rs").write_text("// evil\n", encoding="utf-8")
        outside = base / "ngoai.rs"
        outside.write_text("// ngoai\n", encoding="utf-8")
        dd = base / "deps3"
        dd.mkdir()
        _ghi_d(
            dd / "x-1.d",
            ws3,
            [ws3 / "src/a.rs", ws3 / "src/evil.txt", ws3 / "src/target/evil.rs", outside],
            target=dd / "libx.rmeta",
        )
        res3 = UnitResult("host", ["(no-build)"])
        collect_depinfo(dd, ws3, ws3.resolve(), res3, True, set())
        rel3 = {str(p.relative_to(ws3.resolve())) for p in res3.loaded}
        ghi(
            "(26) P0-3: `.txt` + `src/target/` được nạp; tệp ngoài gốc bị ghi `skipped`",
            rel3 == {"src/a.rs", "src/evil.txt", "src/target/evil.rs"}
            and any(why == "ngoài gốc repo" for _, why in res3.skipped),
            f"loaded={sorted(rel3)} skipped={res3.skipped}",
        )

        # --- (27) KT-227 P0-3: tệp cửa wake KHÔNG còn được miễn cả tệp ----------------------
        prod = src / "core_sched_evil.rs"
        prod.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn wake(&mut self, _t: u32) {}\n"
            "pub fn go(m: &mut crate::Machine, t: u32) { let _ = m.wake(t); }\n",
            encoding="utf-8",
        )
        rep, counts = scan(root, loaded | {prod}, test_only=set())
        ghi(
            "(27) P0-3: tệp có `fn wake` VẪN bị bắt nếu có `m.wake(t)` (không miễn cả tệp)",
            (not rep.ok) and any("E_DEPINFO_WAKE" in e for e in rep.errors),
            f"errors={rep.errors}",
        )

        # --- (28) KT-226: `_is_build_dir` — chỉ bỏ `target` **trước** `src` ------------------
        src_target = root / "v2/kernel/vifast-core/src/target/evil.rs"
        build_target = root / "v2/kernel/target/evil.rs"
        src_target.parent.mkdir(parents=True, exist_ok=True)
        build_target.parent.mkdir(parents=True, exist_ok=True)
        src_target.write_text(_GIA_EVIL, encoding="utf-8")
        build_target.write_text(_GIA_EVIL, encoding="utf-8")
        ghi(
            "(28) `_is_build_dir`: `src/target/` KHÔNG bỏ qua, `v2/kernel/target/` bỏ qua",
            CWP._is_build_dir(root, src_target) is False
            and CWP._is_build_dir(root, build_target) is True
            and CWP._is_build_dir(root, src / "clean.rs") is False,
            f"src/target={CWP._is_build_dir(root, src_target)} "
            f"kernel/target={CWP._is_build_dir(root, build_target)}",
        )

        # --- (29) KT-226: `.d` nạp `src/target/evil.rs` ⇒ vào `loaded` ⇒ ĐỎ -----------------
        dd_a = base / "deps-kt226-src-target"
        _ghi_d(dd_a / "liba.d", root, [src_target])
        res_a = UnitResult("kernel", ["(selftest)"])
        collect_depinfo(dd_a, root, root.resolve(), res_a, True, set())
        rep_a, _ = scan(root, res_a.loaded, test_only=set())
        ghi(
            "(29) `.d` nạp `src/target/evil.rs` (wake) ⇒ ĐỎ `E_DEPINFO_WAKE` (trước KT-226: XANH)",
            src_target.resolve() in res_a.loaded
            and any("E_DEPINFO_WAKE" in e for e in rep_a.errors),
            f"loaded={sorted(CWP.rel(root, p) for p in res_a.loaded)} errors={rep_a.errors}",
        )

        # --- (30) KT-238: `.d` nạp tệp trong target dir CỦA METADATA ⇒ nạp + quét ⇒ ĐỎ ------
        dd_b = base / "deps-kt238-build-target"
        _ghi_d(dd_b / "libb.d", root, [build_target])
        res_b = UnitResult("kernel", ["(selftest)"])
        collect_depinfo(
            dd_b,
            root,
            root.resolve(),
            res_b,
            True,
            set(),
            target_dirs=((root / "v2/kernel/target").resolve(),),
        )
        rep_b, _ = scan(root, res_b.loaded, test_only=set())
        ghi(
            "(30) KT-238: `.d` nạp `v2/kernel/target/evil.rs` (target dir của metadata) ⇒ "
            "NẠP + quét ⇒ ĐỎ (trước KT-238: bị lọc, XANH)",
            build_target.resolve() in res_b.loaded
            and any("E_DEPINFO_WAKE" in e for e in rep_b.errors),
            f"loaded={sorted(CWP.rel(root, p) for p in res_b.loaded)} errors={rep_b.errors}",
        )
        # --- (30b) thư mục tên `target` KHÔNG phải target dir của metadata ⇒ từ chối rõ ----
        other_target = root / "v2/other/target/evil.rs"
        other_target.parent.mkdir(parents=True, exist_ok=True)
        other_target.write_text(_GIA_EVIL, encoding="utf-8")
        dd_b2 = base / "deps-kt238-other-target"
        _ghi_d(dd_b2 / "libb2.d", root, [other_target])
        res_b2 = UnitResult("kernel", ["(selftest)"])
        collect_depinfo(
            dd_b2,
            root,
            root.resolve(),
            res_b2,
            True,
            set(),
            target_dirs=((root / "v2/kernel/target").resolve(),),
        )
        ghi(
            "(30b) KT-238: thư mục tên `target` KHÔNG phải target dir của metadata ⇒ "
            "`skipped` (ĐỎ `E_DEPINFO_SCOPE`, không im lặng)",
            other_target.resolve() not in res_b2.loaded
            and any(why.startswith("thư mục build lạ") for _, why in res_b2.skipped),
            f"loaded={sorted(CWP.rel(root, p) for p in res_b2.loaded)} skipped={res_b2.skipped}",
        )

        # --- (31) KT-230: macro_rules! LỒNG che tên an toàn ⇒ ĐỎ (đường lọt #14) ----------
        # Lớp 2 dùng **danh sách tệp rustc nạp** cho `_shadowed_safe_macros`; bản KT-177 chỉ
        # thấy `macro_rules!` mức mục nên `matches` lồng vẫn "an toàn" ⇒ `let wake = …;
        # matches!(m, wake, t)` XANH. Nay `_collect_safe_macro_defs` đệ quy mọi tầng.
        nested = src / "nested_macro.rs"
        nested.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "macro_rules! kt230_outer {\n"
            "    () => {\n"
            "        #[macro_export]\n"
            "        macro_rules! matches {\n"
            "            ($m:expr, $x:ident, $t:expr) => { $m.$x($t) };\n"
            "        }\n"
            "    };\n"
            "}\n"
            "kt230_outer!();\n"
            "pub fn evil(m: &mut crate::Machine, t: u32) {\n"
            "    let wake = 0u32;\n"
            "    let _ = wake;\n"
            "    let _ = matches!(m, wake, t);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {nested}, test_only=set())
        ghi(
            "(31) KT-230: `macro_rules! matches` LỒNG + `let wake` + `matches!(m, wake, t)` "
            "⇒ ĐỎ `E_DEPINFO_WAKE` (trước: XANH)",
            (not rep.ok) and any("E_DEPINFO_WAKE" in e for e in rep.errors),
            f"errors={rep.errors}",
        )

        # --- (32) KT-230 đối chứng dương: macro THẬT + `let wake` ⇒ XANH (KT-171 B10) ------
        ok_macro = src / "ok_macro.rs"
        ok_macro.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn hop_le(t: u32) {\n"
            "    let wake = 0u32;\n"
            "    let _ = wake;\n"
            "    let _ = matches!(t, wake);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {ok_macro}, test_only=set())
        ghi(
            "(32) KT-230 đối chứng dương: `matches!(t, wake)` macro THẬT + `let wake` ⇒ XANH",
            rep.ok,
            f"errors={rep.errors}",
        )

        # --- (33) KT-230: macro lồng trong tệp KHÔNG `.rs` được nạp ⇒ ĐỎ -------------------
        nested_txt = root / "v2/lib/vifast-abi/src/nested_macros.txt"
        nested_txt.parent.mkdir(parents=True, exist_ok=True)
        nested_txt.write_text(
            "macro_rules! kt230_outer {\n"
            "    () => {\n"
            "        #[macro_export]\n"
            "        macro_rules! matches {\n"
            "            ($m:expr, $x:ident, $t:expr) => { $m.$x($t) };\n"
            "        }\n"
            "    };\n"
            "}\n"
            "kt230_outer!();\n",
            encoding="utf-8",
        )
        call_txt = src / "call_txt.rs"
        call_txt.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn evil(m: &mut crate::Machine, t: u32) {\n"
            "    let wake = 0u32;\n"
            "    let _ = wake;\n"
            "    let _ = matches!(m, wake, t);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {nested_txt, call_txt}, test_only=set())
        ghi(
            "(33) KT-230: macro lồng trong tệp `.txt` được nạp + lời gọi ⇒ ĐỎ `E_DEPINFO_WAKE`",
            (not rep.ok) and any("E_DEPINFO_WAKE" in e for e in rep.errors),
            f"errors={rep.errors}",
        )

        # --- (35) KT-231: alias `TcbState as S` + `S::Ready` ⇒ ĐỎ (đường lọt #15) -----------
        alias = src / "alias_ready.rs"
        alias.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "use crate::TcbState as S;\n"
            "pub fn evil(m: &mut crate::Machine, t: u32) {\n"
            "    let _ = m.transition_tcb(t, S::Ready);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {alias}, test_only=set())
        ghi(
            "(35) KT-231: alias `TcbState as S` + `S::Ready` ⇒ ĐỎ `E_DEPINFO_TRANSITION` "
            "(trước: XANH)",
            (not rep.ok) and any("E_DEPINFO_TRANSITION" in e for e in rep.errors),
            f"errors={rep.errors}",
        )

        # --- (36) **KT-241** — `TcbState::Blocked` cũng ĐỎ (mọi `TcbState::*` = đảo kiểu) ----
        # Ca cũ (KT-231) là **đối chứng dương** "`TcbState::Blocked` ⇒ XANH". Dưới API KT-241
        # (`to: NonReadyTarget`) dạng này **không biên dịch được** ⇒ nó là dấu hiệu **đảo
        # kiểu**, không còn là "chứng minh không Ready". Kỳ vọng **đảo** thành ĐỎ.
        blocked = src / "blocked_ok.rs"
        blocked.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn hop_le(m: &mut crate::Machine, t: u32) {\n"
            "    let _ = m.transition_tcb(t, TcbState::Blocked);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {blocked}, test_only=set())
        ghi(
            "(36) KT-241: `TcbState::Blocked` làm đối số ⇒ ĐỎ (đảo kiểu; ca cũ kỳ vọng XANH)",
            (not rep.ok) and any("E_DEPINFO_TRANSITION" in e for e in rep.errors),
            f"errors={rep.errors}",
        )

        # --- (37) KT-241 đối chứng dương: biến tham số `to: NonReadyTarget` ⇒ XANH ----------
        param = src / "param_ok.rs"
        param.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn block_current(m: &mut crate::Machine, t: u32, to: NonReadyTarget) {\n"
            "    let _ = m.transition_tcb(t, to);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {param}, test_only=set())
        ghi(
            "(37) KT-241 đối chứng dương: biến tham số `to: NonReadyTarget` ⇒ XANH (không báo oan)",
            rep.ok,
            f"errors={rep.errors}",
        )

        # ================================================================ KT-238
        # Phạm vi sinh từ `cargo metadata` + bao đóng path-dependency nội bộ; nguồn sinh trong
        # target dir được nạp + quét (WB-18); `.d` cũ / dấu lần build; R2 theo phạm vi.

        # --- (38) scope_from_metadata: bảo vệ / từ chối rõ / nâng ---------------------------
        sroot = base / "scope-root"
        for d in ("v2/lib/a", "v2/sim/s", "v2/tools/t", "v2/tools/t2", "v2/extra/e", "v2/kernel/k"):
            (sroot / d / "src").mkdir(parents=True, exist_ok=True)

        def _meta(unit: str, pkgs: Sequence[Tuple[str, str, Sequence[str]]], tdir: Path):
            return (
                unit,
                {
                    "target_directory": str(tdir),
                    "workspace_root": str(sroot / "v2"),
                    "packages": [
                        {
                            "name": name,
                            "manifest_path": str(sroot / d / "Cargo.toml"),
                            "targets": [
                                {
                                    "name": name.replace("-", "_"),
                                    "kind": ["lib"],
                                    "src_path": str(sroot / d / "src/lib.rs"),
                                }
                            ],
                            "dependencies": [
                                {"name": dep.split("/")[-1], "path": str(sroot / dep)}
                                for dep in deps
                            ],
                        }
                        for (name, d, deps) in pkgs
                    ],
                },
            )

        metas = [
            _meta(
                "host",
                [
                    ("a", "v2/lib/a", ()),
                    ("s", "v2/sim/s", ()),
                    ("t", "v2/tools/t", ()),
                    ("t2", "v2/tools/t2", ("v2/sim/s",)),
                    ("e", "v2/extra/e", ("v2/sim/s",)),
                ],
                sroot / "v2/target",
            ),
            _meta("kernel", [("k", "v2/kernel/k", ("v2/lib/a",))], sroot / "v2/kernel/target"),
        ]
        sc = scope_from_metadata(sroot, metas)
        ghi(
            "(38) KT-238: phạm vi sinh từ cargo metadata — 5 bảo vệ, 1 từ chối rõ (công cụ)",
            len(sc.protected()) == 5
            and len(sc.refused()) == 1
            and sc.refused()[0].name == "t"
            and sc.target_dirs.get("host") == (sroot / "v2/target").resolve(),
            f"protected={[c.name for c in sc.protected()]} "
            f"refused={[c.name for c in sc.refused()]} target_dirs={sc.target_dirs}",
        )
        ghi(
            "(39) KT-238: crate 'từ chối rõ' phụ thuộc crate bảo vệ ⇒ NÂNG lên bảo vệ",
            sc.by_name["t2"].status == "protected" and "nâng" in sc.by_name["t2"].reason,
            f"t2={sc.by_name['t2'].status} reason={sc.by_name['t2'].reason!r}",
        )
        ghi(
            "(40) KT-238: classify khớp tiền tố dài nhất; tệp ngoài mọi crate ⇒ None",
            sc.classify("v2/lib/a/src/lib.rs") is not None
            and sc.classify("v2/lib/a/src/lib.rs").name == "a"
            and sc.classify("v2/tools/t/src/x.rs").status == "refused"
            and sc.classify("v2/lib/loose.rs") is None,
            f"a={sc.classify('v2/lib/a/src/lib.rs')} loose={sc.classify('v2/lib/loose.rs')}",
        )

        # **KT-256 (L2, D2-188)** — **một** định nghĩa phạm vi dùng cho **cả hai** checker.
        # `contains()` phủ crate ở thư mục gốc **mới** (`v2/extra/…`) — mẫu cũ `RE_CRATE_SRC`
        # (`v2/(lib|kernel|sim|tools)/…`) **mù**; `token_scope()` giữ chính sách DEBT-KT227-1
        # (`v2/lib` ngoài luật token, ảnh sản phẩm + sim trong).
        ghi(
            "(40b) KT-256: `contains` phủ crate ở gốc MỚI `v2/extra/…` (mẫu cũ `RE_CRATE_SRC` mù)",
            sc.contains("v2/extra/e/src/x.rs")
            and re.match(r"^v2/(?:lib|kernel|sim|tools)/[^/]+/src/", "v2/extra/e/src/x.rs") is None,
            f"contains={sc.contains('v2/extra/e/src/x.rs')}",
        )
        # `token_scope` = crate workspace **kernel** (ảnh sản phẩm) + crate **sim** (tên thật
        # `vifast-sim` — cùng kiểu mỏ neo tên như `SIM_KOBJ` của lớp tĩnh) — **không** `v2/lib`.
        sc_tok = scope_from_metadata(
            sroot,
            [
                _meta(
                    "host",
                    [("a", "v2/lib/a", ()), ("vifast-sim", "v2/sim/s", ())],
                    sroot / "v2/target",
                ),
                _meta("kernel", [("k", "v2/kernel/k", ())], sroot / "v2/kernel/target"),
            ],
        )
        ghi(
            "(40c) KT-256/DEBT-KT227-1: `token_scope` = ảnh sản phẩm + sim, KHÔNG `v2/lib`",
            sc_tok.token_scope("v2/kernel/k/src/lib.rs")
            and sc_tok.token_scope("v2/sim/s/src/lib.rs")
            and not sc_tok.token_scope("v2/lib/a/src/lib.rs"),
            f"kernel={sc_tok.token_scope('v2/kernel/k/src/lib.rs')} "
            f"sim={sc_tok.token_scope('v2/sim/s/src/lib.rs')} "
            f"lib={sc_tok.token_scope('v2/lib/a/src/lib.rs')}",
        )

        # --- (41) WB-18: tệp sinh trong target dir của crate bảo vệ ⇒ nạp + quét ⇒ ĐỎ --------
        troot = base / "gen-root"
        tgt = troot / "v2/target"
        out_evil = tgt / "debug/build/evil-123/out"
        out_evil.mkdir(parents=True, exist_ok=True)
        gen_evil = out_evil / "evil.rs"
        gen_evil.write_text(_GIA_EVIL, encoding="utf-8")
        dd_gen = base / "deps-gen"
        _ghi_d(dd_gen / "libevil-123.d", troot, [gen_evil], target=dd_gen / "libevil.rmeta")
        res_gen = UnitResult("host", ["(selftest)"])
        res_gen.pkg_artifacts[str(dd_gen / "libevil.rmeta")] = "v2/extra/e"
        collect_depinfo(
            dd_gen,
            troot,
            troot.resolve(),
            res_gen,
            True,
            set(),
            scope=sc,
            target_dirs=(tgt.resolve(),),
        )
        rep_gen, _ = scan(
            troot, res_gen.loaded, test_only=set(), scope=sc, generated=res_gen.generated
        )
        ghi(
            "(41) KT-238/WB-18: tệp sinh `$OUT_DIR` trong target dir ⇒ NẠP + quét ⇒ ĐỎ "
            "`E_DEPINFO_WAKE` (trước: `W_DEPINFO_SKIPPED` rồi XANH)",
            gen_evil.resolve() in res_gen.loaded
            and res_gen.generated.get(gen_evil.resolve()) == "v2/extra/e"
            and any("E_DEPINFO_WAKE" in e for e in rep_gen.errors),
            f"loaded={sorted(CWP.rel(troot, p) for p in res_gen.loaded)} "
            f"generated={res_gen.generated} errors={rep_gen.errors}",
        )

        # --- (42) đối chứng dương: tệp sinh LÀNH trong target dir ⇒ XANH ---------------------
        gen_ok = out_evil / "ok.rs"
        gen_ok.write_text(_GIA_CLEAN, encoding="utf-8")
        dd_ok = base / "deps-gen-ok"
        _ghi_d(dd_ok / "libok-456.d", troot, [gen_ok], target=dd_ok / "libok.rmeta")
        res_ok = UnitResult("host", ["(selftest)"])
        res_ok.pkg_artifacts[str(dd_ok / "libok.rmeta")] = "v2/extra/e"
        collect_depinfo(
            dd_ok,
            troot,
            troot.resolve(),
            res_ok,
            True,
            set(),
            scope=sc,
            target_dirs=(tgt.resolve(),),
        )
        rep_ok, _ = scan(
            troot, res_ok.loaded, test_only=set(), scope=sc, generated=res_ok.generated
        )
        ghi(
            "(42) KT-238 đối chứng dương: tệp sinh LÀNH trong target dir ⇒ XANH",
            gen_ok.resolve() in res_ok.loaded and rep_ok.ok,
            f"loaded={sorted(CWP.rel(troot, p) for p in res_ok.loaded)} errors={rep_ok.errors}",
        )

        # --- (43) tệp sinh KHÔNG quy được về crate ⇒ từ chối rõ -----------------------------
        dd_unk = base / "deps-gen-unknown"
        _ghi_d(dd_unk / "libunknown-789.d", troot, [gen_evil], target=dd_unk / "libunknown.rmeta")
        res_unk = UnitResult("host", ["(selftest)"])
        collect_depinfo(
            dd_unk,
            troot,
            troot.resolve(),
            res_unk,
            True,
            set(),
            scope=sc,
            target_dirs=(tgt.resolve(),),
        )
        rep_unk, _ = scan(
            troot, res_unk.loaded, test_only=set(), scope=sc, generated=res_unk.generated
        )
        ghi(
            "(43) KT-238: tệp sinh không quy được về crate ⇒ `E_DEPINFO_SCOPE` (từ chối rõ)",
            res_unk.generated.get(gen_evil.resolve()) is None
            and any("E_DEPINFO_SCOPE" in e for e in rep_unk.errors),
            f"generated={res_unk.generated} errors={rep_unk.errors}",
        )

        # --- (44) tệp trong repo nhưng NGOÀI mọi crate ⇒ từ chối rõ -------------------------
        loose = sroot / "v2/lib/loose.rs"
        loose.write_text(_GIA_EVIL, encoding="utf-8")
        rep_loose, _ = scan(sroot, {loose}, test_only=set(), scope=sc)
        ghi(
            "(44) KT-238: tệp rustc nạp ngoài mọi crate ⇒ `E_DEPINFO_SCOPE` (vẫn quét R1)",
            any("E_DEPINFO_SCOPE" in e for e in rep_loose.errors)
            and any("E_DEPINFO_WAKE" in e for e in rep_loose.errors),
            f"errors={rep_loose.errors}",
        )

        # --- (45) `.d` cũ (artifact không thuộc lần chạy) ⇒ KHÔNG đọc nguồn OUT_DIR cũ ------
        old_out = tgt / "debug/build/old-999/out"
        old_out.mkdir(parents=True, exist_ok=True)
        old_evil = old_out / "evil.rs"
        old_evil.write_text(_GIA_EVIL, encoding="utf-8")
        dd_old = base / "deps-old"
        _ghi_d(dd_old / "libold-999.d", troot, [old_evil], target=dd_old / "libold.rmeta")
        res_old = UnitResult("host", ["(selftest)"])
        collect_depinfo(
            dd_old,
            troot,
            troot.resolve(),
            res_old,
            False,
            {str(dd_old / "libnew-111.rmeta")},
            scope=sc,
            target_dirs=(tgt.resolve(),),
        )
        ghi(
            "(45) KT-238: `.d` cũ (artifact không có trong lần chạy) ⇒ KHÔNG đọc nguồn OUT_DIR cũ",
            res_old.depinfo_used == 0 and not res_old.loaded,
            f"depinfo_used={res_old.depinfo_used} loaded={sorted(res_old.loaded)}",
        )

        # --- (46) `.d` trỏ tệp không còn ⇒ ghi `skipped` (tầng gọi báo ĐỎ) ------------------
        dd_miss = base / "deps-missing"
        _ghi_d(dd_miss / "libmiss-222.d", troot, [troot / "v2/lib/a/src/gone.rs"],
               target=dd_miss / "libmiss.rmeta")
        res_miss = UnitResult("host", ["(selftest)"])
        collect_depinfo(
            dd_miss,
            troot,
            troot.resolve(),
            res_miss,
            True,
            set(),
            scope=sc,
            target_dirs=(tgt.resolve(),),
        )
        ghi(
            "(46) KT-238: `.d` trỏ tệp không còn ⇒ `skipped` (ĐỎ `E_DEPINFO_STALE`, không cảnh báo)",
            any(why == "không phải tệp" for _, why in res_miss.skipped),
            f"skipped={res_miss.skipped}",
        )

        # --- (47) dấu lần build: ghi/kiểm/giả mạo -------------------------------------------
        tdir = base / "stamp-target"
        tdir.mkdir(parents=True, exist_ok=True)
        sdeps = base / "stamp-deps"
        sdeps.mkdir(parents=True, exist_ok=True)
        d1 = sdeps / "a-1.d"
        d1.write_text("liba.rmeta: src/a.rs\n", encoding="utf-8")
        write_stamp(tdir, "host", sdeps, [d1])
        only1, errs1 = verify_stamp(tdir, "host", sdeps, {"a-1.d"})
        d1.write_text("liba.rmeta: src/b.rs\n", encoding="utf-8")
        only2, errs2 = verify_stamp(tdir, "host", sdeps, {"a-1.d"})
        only3, errs3 = verify_stamp(base / "no-target", "host", sdeps, {"a-1.d"})
        ghi(
            "(47) KT-238: dấu lần build — khớp ⇒ OK; `.d` đổi ⇒ STALE; thiếu dấu ⇒ STALE",
            only1 == {"a-1.d"}
            and errs1 == []
            and only2 is not None
            and any("đã đổi" in e for e in errs2)
            and only3 is None
            and bool(errs3),
            f"only1={only1} errs1={errs1} errs2={errs2} errs3={errs3}",
        )

        # --- (48) R2 áp cho crate bảo vệ NGOÀI kernel/sim (phạm vi từ metadata) --------------
        lib_bad = sroot / "v2/lib/a/src/bad_ready.rs"
        lib_bad.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn khong_qua_cua() {\n"
            "    transition_tcb(1, TcbState::Ready);\n"
            "}\n",
            encoding="utf-8",
        )
        rep_lib, _ = scan(sroot, {lib_bad}, test_only=set(), scope=sc)
        ghi(
            "(48) KT-238: R2 áp cho crate bảo vệ ngoài kernel/sim ⇒ ĐỎ `E_DEPINFO_TRANSITION`",
            any("E_DEPINFO_TRANSITION" in e for e in rep_lib.errors),
            f"errors={rep_lib.errors}",
        )

        # --- (49) crate 'từ chối rõ': R1 vẫn áp, R2 không áp --------------------------------
        tool_evil = sroot / "v2/tools/t/src/evil.rs"
        tool_evil.write_text(_GIA_EVIL, encoding="utf-8")
        tool_ready = sroot / "v2/tools/t/src/ready.rs"
        tool_ready.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn f() { transition_tcb(1, TcbState::Ready); }\n",
            encoding="utf-8",
        )
        rep_tool, _ = scan(sroot, {tool_evil, tool_ready}, test_only=set(), scope=sc)
        ghi(
            "(49) KT-238: crate 'từ chối rõ' — R1 vẫn bắt `wake`, R2 không áp (không thể tới API)",
            any("E_DEPINFO_WAKE" in e for e in rep_tool.errors)
            and not any("E_DEPINFO_TRANSITION" in e for e in rep_tool.errors),
            f"errors={rep_tool.errors}",
        )

        # --- (50) `--no-build` không có dấu ⇒ ĐỎ `E_DEPINFO_STALE` --------------------------
        fake2 = base / "fake-no-stamp"
        (fake2 / "v2/kernel").mkdir(parents=True, exist_ok=True)
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = main(["--root", str(fake2), "--no-build"])
        out = buf.getvalue()
        ghi(
            "(50) KT-238: `--no-build` không có dấu lần build ⇒ RC=1 + `E_DEPINFO_STALE`",
            rc == 1 and "E_DEPINFO_STALE" in out,
            f"rc={rc} out={out[-200:]}",
        )

        # --- (51) `--no-build` có dấu ⇒ chỉ đọc `.d` trong dấu (end-to-end) ----------------
        fake3 = base / "fake-stamp"
        (fake3 / "v2/kernel").mkdir(parents=True, exist_ok=True)
        (fake3 / "v2/Cargo.toml").write_text(
            "[workspace]\nresolver = \"2\"\nmembers = [\"lib/a\"]\n", encoding="utf-8"
        )
        (fake3 / "v2/lib/a/src").mkdir(parents=True, exist_ok=True)
        (fake3 / "v2/lib/a/Cargo.toml").write_text(
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            encoding="utf-8",
        )
        (fake3 / "v2/lib/a/src/lib.rs").write_text(_GIA_CLEAN, encoding="utf-8")
        (fake3 / "v2/kernel/Cargo.toml").write_text(
            "[workspace]\nresolver = \"2\"\nmembers = [\"k\"]\n", encoding="utf-8"
        )
        (fake3 / "v2/kernel/k/src").mkdir(parents=True, exist_ok=True)
        (fake3 / "v2/kernel/k/Cargo.toml").write_text(
            "[package]\nname = \"k\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            encoding="utf-8",
        )
        (fake3 / "v2/kernel/k/src/lib.rs").write_text(_GIA_CLEAN, encoding="utf-8")
        for unit, tdir3, ws3, src3, triple3 in (
            ("host", fake3 / "v2/target", fake3 / "v2", fake3 / "v2/lib/a/src/lib.rs", None),
            (
                "kernel",
                fake3 / "v2/kernel/target",
                fake3 / "v2/kernel",
                fake3 / "v2/kernel/k/src/lib.rs",
                "riscv64gc-unknown-none-elf",
            ),
        ):
            deps3 = tdir3 / triple3 / "debug/deps" if triple3 else tdir3 / "debug/deps"
            deps3.mkdir(parents=True, exist_ok=True)
            _ghi_d(deps3 / "libx-1.d", ws3, [src3], target=deps3 / "libx.rmeta")
            write_stamp(tdir3, unit, deps3, [deps3 / "libx-1.d"])
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = main(["--root", str(fake3), "--no-build"])
        out = buf.getvalue()
        ghi(
            "(51) KT-238: `--no-build` có dấu ⇒ XANH, chỉ đọc `.d` trong dấu (end-to-end)",
            rc == 0 and "errors=0" in out,
            f"rc={rc} out={out[-300:]}",
        )

        # --- (52)(53) KT-242: hai bố cục artifact cùng tồn tại ------------------------------
        # **A0 duyệt 2026-11-20**: không được chọn thư mục `.d` chỉ vì nó tồn tại. Khi cả
        # `debug/deps/` (bố cục cũ) và `debug/build/<pkg>/<hash>/out/` (cargo ≥ 1.100) cùng có
        # mặt, phải chọn theo **artifact của lần build này**.
        base52 = base / "layout"
        legacy52 = base52 / "debug/deps"
        new52 = base52 / "debug/build/pkg/abc123/out"
        legacy52.mkdir(parents=True, exist_ok=True)
        new52.mkdir(parents=True, exist_ok=True)
        (legacy52 / "cu-1.d").write_text("", encoding="utf-8")
        (new52 / "moi-1.d").write_text("", encoding="utf-8")
        got52 = depinfo_dirs(base52, None, [str(new52 / "libx-abc123.rlib")])
        ghi(
            "(52) KT-242: hai bố cục cùng tồn tại — chọn theo artifact của lần build",
            got52 == [new52],
            f"got={got52}",
        )
        got53 = depinfo_dirs(base52, None, [])
        # Dự phòng quét **cả** `debug/deps` và **cả cây** `debug/build` (đệ quy) ⇒ phủ mọi bố cục.
        ghi(
            "(53) KT-242: build đỏ (không artifact) — quét CẢ HAI bố cục (superset, fail-closed)",
            set(got53) == {legacy52, base52 / "debug/build"},
            f"got={got53}",
        )
        # Layout trộn: artifact của hai unit nằm ở hai bố cục khác nhau ⇒ lấy cả hai thư mục.
        got54 = depinfo_dirs(
            base52, None, [str(new52 / "libx-abc123.rlib"), str(legacy52 / "liby-def456.rlib")]
        )
        ghi(
            "(54) KT-242: layout trộn — artifact ở hai bố cục ⇒ lấy cả hai thư mục",
            set(got54) == {new52, legacy52},
            f"got={got54}",
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

    # **KT-238** — phạm vi sinh từ `cargo metadata` của **các workspace liên quan** + bao đóng
    # path-dependency nội bộ. `--fresh` ⇒ truyền **cùng** `CARGO_TARGET_DIR` cho metadata để
    # `target_directory` khớp lần build (nguồn sinh `OUT_DIR` gắn đúng lần build đang kiểm).
    overrides: Dict[str, Path] = {}
    if fresh_dir is not None:
        for name, _manifest, _triple, _target_rel in units:
            overrides[name] = fresh_dir / name
    scope = load_scope(root, cargo, toolchain, overrides)

    results: List[UnitResult] = []
    build_errors: List[str] = []
    scope_errors: List[str] = []
    stamp_errors: List[str] = []

    if not scope.crates:
        scope_errors.append(
            "  không dựng được phạm vi từ cargo metadata (0 crate) — fail-closed (KT-238)"
        )
    # **KT-258 (WB-23)** — path-dependency ra ngoài gốc repo ⇒ từ chối rõ (cùng ca canh với
    # lớp tĩnh; lớp này chỉ thấy nếu cấu hình build đang kiểm có build nó).
    for e in scope.check_outside_dependencies():
        scope_errors.append(e)
    for name, _manifest, _triple, _target_rel in units:
        if scope.target_dirs.get(name) is None:
            scope_errors.append(
                f"  đơn vị `{name}`: không lấy được target dir từ cargo metadata — fail-closed "
                f"(KT-238)"
            )

    if args.no_build:
        # **KT-238** — `--no-build` chỉ đọc `.d` có trong **dấu lần build** (sha256). Không có
        # dấu / dấu lệch ⇒ `E_DEPINFO_STALE`: chứng minh `.d` cũ (và nguồn `OUT_DIR` cũ mà nó
        # trỏ tới) **không** được đọc nhầm.
        for name, manifest, triple, target_rel in units:
            res = UnitResult(name, ["(no-build)"])
            target_dir = scope.target_dirs.get(name)
            if target_dir is None:
                target_dir = (root / target_rel).resolve()
                res.note = "không có target dir từ cargo metadata — dùng dự phòng (fail-closed)"
            res.target_dir = target_dir
            # **KT-242 (A0 duyệt)**: thư mục `.d` lấy từ **dấu lần build** (đã ghi lúc build),
            # không đoán theo thư mục đang tồn tại.
            deps_dirs = stamp_deps_dirs(target_dir) or depinfo_dirs(target_dir, triple, [])
            ws_root = (root / manifest).parent
            present = {d.name for dd in deps_dirs for d in depinfo_glob(dd)}
            only, errs = verify_stamp(target_dir, name, deps_dirs, present)
            stamp_errors.extend(errs)
            collect_depinfo(
                deps_dirs,
                ws_root,
                root,
                res,
                True,
                set(),
                scope=scope,
                target_dirs=(target_dir,),
                only_files=only if only is not None else set(),
            )
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
                scope=scope,
            )
            results.append(res)
            if res.rc != 0:
                build_errors.append(
                    f"  đơn vị `{name}`: cargo check RC={res.rc}"
                    + (f" ({res.note})" if res.note else "")
                )

    loaded: Set[Path] = set()
    generated: Dict[Path, Optional[str]] = {}
    for res in results:
        loaded |= res.loaded
        generated.update(res.generated)

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

    rep, counts = scan(
        root,
        loaded,
        test_only=test_only,
        do_transitions=not args.no_transition,
        scope=scope,
        generated=generated,
    )
    for e in build_errors:
        rep.err("E_DEPINFO_BUILD", e)
    for e in scope_errors:
        rep.err("E_DEPINFO_SCOPE", e)
    for e in stamp_errors:
        rep.err("E_DEPINFO_STALE", e)
    # **KT-227 P0-1** — `.d` rỗng/thiếu/không cho tệp nào ⇒ ĐỎ (trước bản này là xanh-giả).
    for e in depinfo_empty_errors(results, loaded):
        rep.err("E_DEPINFO_EMPTY", e)
    # **KT-238** — tệp rustc nạp mà công cụ **không quét**: mọi trường hợp đều **ĐỎ** (không còn
    # `W_DEPINFO_SKIPPED`): ngoài gốc repo ⇒ `E_DEPINFO_OUTSIDE`; thư mục build lạ ⇒
    # `E_DEPINFO_SCOPE`; không resolve / không phải tệp ⇒ `E_DEPINFO_STALE`.
    for res in results:
        for path, why in res.skipped:
            if why == "ngoài gốc repo":
                rep.err(
                    "E_DEPINFO_OUTSIDE",
                    f"  {path}: tệp rustc nạp nằm ngoài gốc repo — không quét được "
                    f"(fail-closed, KT-227 P0-3)",
                )
            elif why.startswith("thư mục build lạ"):
                rep.err(
                    "E_DEPINFO_SCOPE",
                    f"  {path}: {why} — từ chối rõ (fail-closed, KT-238)",
                )
            else:
                rep.err(
                    "E_DEPINFO_STALE",
                    f"  {path}: tệp rustc nạp không đọc được ({why}) — `.d` không khớp lần "
                    f"build hiện tại (fail-closed, KT-238)",
                )

    hidden = hidden_files(root, loaded, scope)
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
                        "scope": {
                            "protected": [
                                {"name": c.name, "dir": c.dir_rel} for c in scope.protected()
                            ],
                            "refused": [
                                {"name": c.name, "dir": c.dir_rel, "reason": c.reason}
                                for c in scope.refused()
                            ],
                            "target_dirs": {k: str(v) for k, v in scope.target_dirs.items()},
                            "notes": scope.notes,
                        },
                        "units": [
                            {
                                "name": u.name,
                                "cmd": u.cmd,
                                "rc": u.rc,
                                "seconds": round(u.seconds, 3),
                                "artifacts": u.artifacts,
                                "depinfo_used": u.depinfo_used,
                                "loaded": len(u.loaded),
                                "generated": len(u.generated),
                                "target_dir": str(u.target_dir) if u.target_dir else None,
                                "note": u.note,
                            }
                            for u in results
                        ],
                        "loaded_files": counts["loaded"],
                        "scanned_files": counts["scanned"],
                        "generated_files": len(generated),
                        "producer_files": counts["producer_files"],
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
    for line in scope.summary():
        print(line)
    for u in results:
        cmd = " ".join(u.cmd)
        print(
            f"# đơn vị {u.name}: {cmd} → RC={u.rc} ({u.seconds:.2f} s), "
            f"{u.artifacts} artifact, {u.depinfo_used} tệp .d, {len(u.loaded)} tệp .rs"
            + (f", {len(u.generated)} tệp sinh" if u.generated else "")
            + (f" [target dir: {u.target_dir}]" if u.target_dir else "")
            + (f" [{u.note}]" if u.note else "")
        )
    print(f"# rustc nạp: {counts['loaded']} tệp .rs trong cây (gộp {len(results)} đơn vị)")
    print(
        f"# quét: {counts['scanned']} tệp "
        f"(tệp cửa wake: {counts['producer_files']} — KHÔNG miễn cả tệp; miễn {counts['exempt_test']} tệp test-only)"
    )
    print(f"# tệp sinh trong target dir (đã quét): {len(generated)}")
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
