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
   `RE_READY_TRANSITION`, `WAKE_DOORS`, `NON_WAKE_READY_SITES`, `in_test_only_wake_scope`,
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
  - **R2 chuyển `Ready`** (`E_DEPINFO_TRANSITION`): mọi `transition_tcb(…, TcbState::Ready)`
    trong tệp của crate **bảo vệ** (phạm vi sinh từ `cargo metadata` — **không** chỉ
    `v2/kernel`/`v2/sim`) phải nằm trong cửa `WAKE_DOORS` hoặc có tên trong
    `NON_WAKE_READY_SITES` (allowlist hiện có, khoá `(tệp, hàm, thứ tự)`). Crate "từ chối rõ"
    chỉ áp R1 — nó không thể gọi API `wake` (bao đóng dependency không có crate sản phẩm).

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

# **DỰ PHÒNG** (chỉ dùng khi không có `Scope`): mẫu "tệp nguồn hiển nhiên" theo thư mục.
# **KT-238** — định nghĩa phạm vi là `cargo metadata` (`Scope.classify`), **không** phải mẫu này;
# mẫu chỉ còn để báo cáo `hidden_files` khi selftest cũ gọi không kèm scope.
RE_CRATE_SRC = re.compile(r"^v2/(?:lib|kernel|sim|tools)/[^/]+/src/")

# Tệp `build.rs` cấp crate: cũng là tệp "hiển nhiên" (nằm cạnh `Cargo.toml`) — dự phòng như trên.
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


# ------------------------------------------------------------------ phạm vi từ cargo metadata
#
# **KT-238 / D2-188** — phạm vi bảo vệ **sinh từ `cargo metadata`**, không từ ba đường dẫn cố
# định (`v2/kernel`, `v2/sim`, `v2/lib`). Vì sao: WB-18 (KT-233) — một crate thành viên workspace
# **ngoài** ba gốc ấy sinh mã trong `$OUT_DIR`; lớp 2 thấy tệp nhưng chỉ cảnh báo rồi bỏ qua.
# Ba đường dẫn cố định **không phải** định nghĩa đầy đủ của phạm vi: crate thêm sau (hoặc crate
# chỉ tới được qua path-dependency) nằm ngoài mẫu ⇒ mù. Nay:
#
#   1. chạy `cargo metadata --no-deps --format-version 1` cho **mọi** workspace sản phẩm;
#   2. lấy **thành viên** + **bao đóng path-dependency nội bộ** (crate `path` trỏ vào repo);
#   3. phân loại: **bảo vệ** (sản phẩm) / **từ chối rõ** (công cụ — `v2/tools/`), kèm lý do;
#   4. crate "từ chối" mà bao đóng dependency chứa crate bảo vệ ⇒ **nâng** lên bảo vệ
#      (fail-closed: công cụ phụ thuộc sản phẩm thì **có thể** gọi API `wake`).
#
# Tệp rustc nạp mà **không** quy được về crate nào (và không phải tệp sinh trong target dir của
# cargo metadata) ⇒ `E_DEPINFO_SCOPE` — **từ chối rõ**, không quét-rồi-bỏ-qua.

# Hai workspace sản phẩm (D2-26): host (`v2/Cargo.toml`) và bare-metal (`v2/kernel/Cargo.toml`).
WORKSPACES: Tuple[Tuple[str, str], ...] = (
    ("host", "v2/Cargo.toml"),
    ("kernel", "v2/kernel/Cargo.toml"),
)

# Tiền tố crate **công cụ**: thành viên workspace nhưng **không** phải sản phẩm ⇒ "từ chối rõ".
REFUSED_PREFIXES: Tuple[str, ...] = ("v2/tools/",)

# Dấu "lần build hiện tại" cho `--no-build`: danh sách `.d` + sha256 của lần build xanh gần nhất.
# Không có dấu / dấu lệch ⇒ `E_DEPINFO_STALE` (fail-closed) — chứng minh không đọc `.d` cũ.
STAMP_NAME = ".kt238-depinfo-stamp.json"
STAMP_VERSION = 1


class CrateInfo:
    """Một crate trong phạm vi, sinh từ `cargo metadata`."""

    __slots__ = (
        "name",
        "dir_rel",
        "manifest_rel",
        "deps",
        "target_names",
        "src_paths",
        "status",
        "reason",
    )

    def __init__(
        self,
        name: str,
        dir_rel: str,
        manifest_rel: str,
        deps: Sequence[str],
        target_names: Sequence[str],
        src_paths: Sequence[str],
    ) -> None:
        self.name = name
        self.dir_rel = dir_rel
        self.manifest_rel = manifest_rel
        self.deps = list(deps)
        self.target_names = list(target_names)
        self.src_paths = list(src_paths)
        self.status = "protected"
        self.reason = ""

    def __repr__(self) -> str:  # pragma: no cover - chẩn đoán
        return f"CrateInfo({self.name!r}, {self.dir_rel!r}, {self.status!r})"


class Scope:
    """Phạm vi bảo vệ sinh từ `cargo metadata` + bao đóng dependency nội bộ."""

    def __init__(self) -> None:
        self.crates: Dict[str, CrateInfo] = {}  # dir_rel → crate
        self.by_name: Dict[str, CrateInfo] = {}
        self.by_target: Dict[str, CrateInfo] = {}
        self.target_dirs: Dict[str, Path] = {}  # đơn vị build → target dir (tuyệt đối)
        self.notes: List[str] = []

    def classify(self, relp: str) -> Optional[CrateInfo]:
        """Crate chứa `relp` (khớp tiền tố **dài nhất**), hoặc `None` nếu ngoài mọi crate."""
        best: Optional[CrateInfo] = None
        for d, c in self.crates.items():
            if relp == d or relp.startswith(d + "/"):
                if best is None or len(d) > len(best.dir_rel):
                    best = c
        return best

    def protected(self) -> List[CrateInfo]:
        return sorted((c for c in self.crates.values() if c.status == "protected"), key=lambda c: c.dir_rel)

    def refused(self) -> List[CrateInfo]:
        return sorted((c for c in self.crates.values() if c.status == "refused"), key=lambda c: c.dir_rel)

    def summary(self) -> List[str]:
        out = [
            f"# phạm vi (cargo metadata): {len(self.protected())} crate bảo vệ, "
            f"{len(self.refused())} crate từ chối rõ"
        ]
        for c in self.protected():
            out.append(f"#   bảo vệ: {c.name} ({c.dir_rel})")
        for c in self.refused():
            out.append(f"#   từ chối rõ: {c.name} ({c.dir_rel}) — {c.reason}")
        for n in self.notes:
            out.append(f"#   ghi chú phạm vi: {n}")
        return out


def _rel_or_none(root: Path, p: Path) -> Optional[str]:
    try:
        return str(p.resolve().relative_to(root))
    except (ValueError, OSError):
        return None


def _crate_from_package(root: Path, pkg: dict) -> Optional[CrateInfo]:
    """Một mục `packages[]` của `cargo metadata` ⇒ `CrateInfo` (bỏ crate ngoài gốc repo)."""
    mp = pkg.get("manifest_path")
    if not mp:
        return None
    manifest = Path(mp)
    dir_rel = _rel_or_none(root, manifest.parent)
    if dir_rel is None:
        return None
    manifest_rel = _rel_or_none(root, manifest) or dir_rel
    deps: List[str] = []
    for d in pkg.get("dependencies") or []:
        p = d.get("path")
        if not p:
            continue
        r = _rel_or_none(root, Path(p))
        if r is not None:
            deps.append(r)
    target_names: List[str] = []
    src_paths: List[str] = []
    for t in pkg.get("targets") or []:
        sp = t.get("src_path")
        if sp:
            r = _rel_or_none(root, Path(sp))
            if r is not None:
                src_paths.append(r)
        nm = (t.get("name") or "").replace("-", "_")
        if nm:
            target_names.append(nm)
    return CrateInfo(
        pkg.get("name") or dir_rel,
        dir_rel,
        manifest_rel,
        deps,
        target_names,
        src_paths,
    )


def _dep_closure(sc: Scope, c: CrateInfo) -> Set[str]:
    """Bao đóng dependency **nội bộ** (bắc cầu) của `c` — tập `dir_rel`."""
    seen: Set[str] = set()
    stack = list(c.deps)
    while stack:
        d = stack.pop()
        if d in seen:
            continue
        seen.add(d)
        dc = sc.crates.get(d)
        if dc is not None:
            stack.extend(dc.deps)
    return seen


def _classify_scope(sc: Scope) -> None:
    """Gán `status`/`reason` cho mọi crate: bảo vệ / từ chối rõ (+ nâng khi cần)."""
    for c in sc.crates.values():
        c.status = "protected"
        c.reason = ""
        if any(c.dir_rel == p or c.dir_rel.startswith(p) for p in REFUSED_PREFIXES):
            c.status = "refused"
            c.reason = "công cụ (v2/tools/), không phải sản phẩm"
    # Nâng: crate "từ chối" mà bao đóng dependency chứa crate bảo vệ ⇒ bảo vệ (fail-closed).
    changed = True
    while changed:
        changed = False
        for c in sc.crates.values():
            if c.status != "refused":
                continue
            for d in _dep_closure(sc, c):
                dc = sc.crates.get(d)
                if dc is not None and dc.status == "protected":
                    c.status = "protected"
                    c.reason = (
                        f"nâng từ 'từ chối rõ': bao đóng dependency chứa crate bảo vệ "
                        f"`{dc.name}` ({dc.dir_rel})"
                    )
                    changed = True
                    break


def scope_from_metadata(root: Path, metas: Sequence[Tuple[str, dict]]) -> Scope:
    """Dựng `Scope` từ các blob `cargo metadata --no-deps` — hàm **thuần** (selftest được).

    `metas` = `[(tên đơn vị, blob), …]`. Không chạy cargo, không đọc đĩa (trừ `root` để tính
    đường dẫn tương đối).
    """
    root = root.resolve()
    sc = Scope()
    for unit, meta in metas:
        td = meta.get("target_directory")
        if td:
            sc.target_dirs[unit] = Path(td).resolve()
        for pkg in meta.get("packages") or []:
            c = _crate_from_package(root, pkg)
            if c is None:
                continue
            sc.crates[c.dir_rel] = c
            sc.by_name[c.name] = c
            for nm in c.target_names:
                sc.by_target[nm] = c
    _classify_scope(sc)
    return sc


def _run_metadata(
    root: Path,
    manifest_rel: str,
    cargo: str,
    toolchain: Optional[str],
    target_dir: Optional[Path],
) -> Optional[dict]:
    """Chạy `cargo metadata --no-deps --format-version 1`; `None` nếu lỗi (tầng gọi fail-closed)."""
    cmd = [
        cargo,
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--manifest-path",
        str(root / manifest_rel),
    ]
    env = dict(os.environ)
    if toolchain:
        env["RUSTUP_TOOLCHAIN"] = toolchain
    if target_dir is not None:
        env["CARGO_TARGET_DIR"] = str(target_dir)
    env.setdefault("CARGO_TERM_COLOR", "never")
    try:
        proc = subprocess.run(
            cmd, cwd=str(root), env=env, capture_output=True, text=True, timeout=180
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError:
        return None


def load_scope(
    root: Path,
    cargo: Optional[str],
    toolchain: Optional[str],
    target_overrides: Optional[Dict[str, Path]] = None,
) -> Scope:
    """Sinh phạm vi từ `cargo metadata` của **các workspace liên quan** + bao đóng path-dep.

    `target_overrides` = `{đơn vị: target dir}` (dùng cho `--fresh`: `CARGO_TARGET_DIR` tạm) —
    truyền **cùng** biến môi trường cho `cargo metadata` để `target_directory` khớp lần build.
    """
    overrides = target_overrides or {}
    if cargo is None:
        sc = Scope()
        sc.notes.append("không tìm thấy cargo ⇒ phạm vi rỗng (fail-closed)")
        return sc
    metas: List[Tuple[str, dict]] = []
    notes: List[str] = []
    for unit, manifest in WORKSPACES:
        meta = _run_metadata(root, manifest, cargo, toolchain, overrides.get(unit))
        if meta is None:
            notes.append(f"không đọc được cargo metadata cho `{manifest}` (fail-closed)")
            continue
        metas.append((unit, meta))
    sc = scope_from_metadata(root, metas)
    sc.notes.extend(notes)
    # Bao đóng: path-dependency chưa có trong `crates` ⇒ chạy metadata trên manifest của nó.
    for _ in range(8):
        missing = sorted(
            {d for c in list(sc.crates.values()) for d in c.deps if d not in sc.crates}
        )
        if not missing:
            break
        added = False
        for d in missing:
            mf = root / d / "Cargo.toml"
            if not mf.is_file():
                sc.notes.append(f"path-dependency `{d}` không có Cargo.toml — bỏ khỏi bao đóng")
                continue
            meta = _run_metadata(root, str(mf.relative_to(root)), cargo, toolchain, None)
            if meta is None:
                sc.notes.append(f"không đọc được metadata cho path-dependency `{d}`")
                continue
            for pkg in meta.get("packages") or []:
                c = _crate_from_package(root, pkg)
                if c is None or c.dir_rel in sc.crates:
                    continue
                sc.crates[c.dir_rel] = c
                sc.by_name[c.name] = c
                for nm in c.target_names:
                    sc.by_target[nm] = c
                added = True
        if not added:
            break
    _classify_scope(sc)
    return sc


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


def write_stamp(target_dir: Path, unit: str, deps_dir: Path, depinfo_files: Sequence[Path]) -> None:
    """Ghi dấu "lần build hiện tại" — danh sách `.d` **đã dùng** + sha256."""
    target_dir = target_dir.resolve()
    deps_dir = deps_dir.resolve()
    data = {
        "version": STAMP_VERSION,
        "unit": unit,
        "deps_dir": str(deps_dir),
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
    target_dir: Path, unit: str, deps_dir: Path, present: Set[str]
) -> Tuple[Optional[Set[str]], List[str]]:
    """Kiểm dấu lần build cho `--no-build` ⇒ `(tập .d hiện hành, lỗi)`.

    Không có dấu / dấu lệch / `.d` đã đổi nội dung ⇒ lỗi (fail-closed). Chỉ `.d` **có trong
    dấu** mới được đọc — chứng minh `.d` cũ (và nguồn `OUT_DIR` cũ mà nó trỏ tới) không bị đọc.
    """
    target_dir = target_dir.resolve()
    deps_dir = deps_dir.resolve()
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
    if data.get("deps_dir") != str(deps_dir):
        errs.append(
            f"  đơn vị `{unit}`: dấu lần build thuộc deps dir khác "
            f"(`{data.get('deps_dir')}` ≠ `{deps_dir}`) — fail-closed (KT-238)"
        )
    files: Dict[str, str] = data.get("files") or {}
    missing = sorted(set(files) - present)
    if missing:
        errs.append(
            f"  đơn vị `{unit}`: `.d` trong dấu không còn trên đĩa: {', '.join(missing[:5])} "
            f"— fail-closed (KT-238)"
        )
    for name, want in sorted(files.items()):
        p = deps_dir / name
        if not p.is_file():
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


def collect_depinfo(
    deps_dir: Path,
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
    if not deps_dir.is_dir():
        res.note = f"không có thư mục dep-info {deps_dir}"
        return
    seen_skip: Set[Tuple[str, str]] = set()

    def skip(path: str, why: str) -> None:
        key = (path, why)
        if key not in seen_skip:
            seen_skip.add(key)
            res.skipped.append(key)

    for d in sorted(deps_dir.glob("*.d")):
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
    deps_dir = target_dir
    if triple:
        deps_dir = deps_dir / triple
    deps_dir = deps_dir / "debug/deps"
    ws_root = (root / manifest).parent

    # Khi build **thành công**: chỉ lấy `.d` của unit có mặt trong lần chạy này (chính xác).
    # Khi build **thất bại**: cargo **không** phát `compiler-artifact` cho unit hỏng, nhưng rustc
    # **vẫn ghi** `.d` (đã đo với lỗi kiểu E0308 và E0583) ⇒ lấy **mọi** `.d` trong thư mục
    # (superset, fail-closed) và báo `E_DEPINFO_BUILD` ở tầng gọi.
    use_all = (build_ok is False) or (not artifacts)
    if build_ok is False:
        res.note = "build ĐỎ — dùng mọi .d trong thư mục (superset, fail-closed)"

    collect_depinfo(
        deps_dir,
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
        write_stamp(target_dir, name, deps_dir, res.depinfo_files)
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
    không phải mẫu đường dẫn cố định; `RE_CRATE_SRC`/`RE_BUILD_RS` chỉ còn là dự phòng khi
    không có scope (selftest cũ).
    """
    out: List[str] = []
    for p in loaded:
        relp = CWP.rel(root, p)
        if scope is not None:
            c = scope.classify(relp)
            if c is not None and (
                relp == c.dir_rel + "/build.rs" or relp.startswith(c.dir_rel + "/src/")
            ):
                continue
            out.append(relp)
            continue
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
        CWP._check_wake_text(relp, text, CWP.test_line_ranges(text), safe_macros, sub)
        for e in sub.errors:
            # `Report.err` lưu `"<mã> <thông điệp>"`; đổi mã sang mã của lớp dep-info.
            msg = e.split(" ", 1)[1] if " " in e else e
            r.err("E_DEPINFO_WAKE", msg)

    if do_transitions:
        # R2 — **KT-231**: dùng **cùng** bộ quét token-tree của lớp một (`CWP._transition_calls`),
        # không regex văn bản. Vì sao: đường lọt #15 (reviewer L54, KT-229) — `RE_READY_TRANSITION`
        # khớp `TcbState::Ready` trên **một dòng**; `use crate::TcbState as S;` +
        # `m.transition_tcb(t, S::Ready)` XANH cả hai lớp. Nay:
        #   - `ready` (literal `TcbState::Ready`) ⇒ cửa wake / allowlist như cũ;
        #   - `other` (literal `TcbState::<BiếnThểKhác>`) ⇒ chứng minh không Ready ⇒ XANH;
        #   - `alias`/`unknown` ⇒ **ĐỎ** (fail-closed);
        #   - `var` ⇒ ĐỎ nếu hàm gán biến từ `TcbState::Ready` (dataflow-lite, tránh báo oan
        #     `block_current(cur, to)`).
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
            tr = CWP.test_line_ranges(text)
            fns = CWP.find_functions(p, text)
            ordinal: Dict[Tuple[str, str], int] = {}
            for lineno, kind, desc in CWP._transition_calls(text, relp):
                if CWP.in_ranges(lineno, tr):
                    continue
                fn = CWP.enclosing(fns, lineno)
                if fn is None:
                    r.err(
                        "E_DEPINFO_TRANSITION",
                        f"  {relp}:{lineno}: `transition_tcb` ngoài hàm — không xác định "
                        f"được cửa (fail-closed, KT-231)",
                    )
                    continue
                if (relp, fn.name) in CWP.WAKE_DOORS:
                    continue
                if kind == "other":
                    continue
                if kind == "alias":
                    r.err(
                        "E_DEPINFO_TRANSITION",
                        f"  {relp}:{lineno}: `transition_tcb(…, {desc})` — đối số thứ hai "
                        f"KHÔNG phải literal `TcbState::Ready` (alias/đường dẫn lạ) ⇒ không "
                        f"chứng minh được (fail-closed, KT-231)",
                    )
                    continue
                if kind == "var":
                    if CWP._var_may_be_ready(text, fn, desc):
                        r.err(
                            "E_DEPINFO_TRANSITION",
                            f"  {relp}:{lineno}: `transition_tcb(…, {desc})` — biến `{desc}` "
                            f"được gán từ `TcbState::Ready` trong `{fn.name}` ⇒ không chứng "
                            f"minh được (fail-closed, KT-231)",
                        )
                    continue
                if kind == "unknown":
                    r.err(
                        "E_DEPINFO_TRANSITION",
                        f"  {relp}:{lineno}: `transition_tcb` — {desc} ⇒ không chứng minh "
                        f"được (fail-closed, KT-231)",
                    )
                    continue
                key = (relp, fn.name)
                ordinal[key] = ordinal.get(key, 0) + 1
                idx = ordinal[key]
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
            rep.ok and counts["scanned"] == 3 and counts["producer_files"] == 1,
            f"errors={rep.errors} counts={counts}",
        )
        ghi(
            "(6) tệp định nghĩa `fn wake` VẪN được quét (chỉ miễn định nghĩa, KT-227 P0-3)",
            counts["producer_files"] == 1 and counts["scanned"] == 3,
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

        # --- (36) KT-231 đối chứng dương: `TcbState::Blocked` ⇒ XANH ------------------------
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
            "(36) KT-231 đối chứng dương: `TcbState::Blocked` ngoài cửa ⇒ XANH",
            rep.ok,
            f"errors={rep.errors}",
        )

        # --- (37) KT-231 đối chứng dương: biến tham số `to` ⇒ XANH (không báo oan) ----------
        param = src / "param_ok.rs"
        param.write_text(
            "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
            "pub fn block_current(m: &mut crate::Machine, t: u32, to: TcbState) {\n"
            "    let _ = m.transition_tcb(t, to);\n"
            "}\n",
            encoding="utf-8",
        )
        rep, _ = scan(root, loaded | {param}, test_only=set())
        ghi(
            "(37) KT-231 đối chứng dương: biến tham số `to` ⇒ XANH (không báo oan)",
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
            deps_dir = target_dir
            if triple:
                deps_dir = deps_dir / triple
            deps_dir = deps_dir / "debug/deps"
            ws_root = (root / manifest).parent
            present = {d.name for d in deps_dir.glob("*.d")} if deps_dir.is_dir() else set()
            only, errs = verify_stamp(target_dir, name, deps_dir, present)
            stamp_errors.extend(errs)
            collect_depinfo(
                deps_dir,
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
