#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-256 (L2, D2-188) — **MỘT** định nghĩa phạm vi bảo vệ của luật `wake`, sinh từ `cargo metadata`.

Vì sao có tệp này
-----------------
Trước KT-256 có **ba** định nghĩa phạm vi vênh nhau (nợ **L2**, ghi ở
`v2/agent/JOURNAL/kt252-wake-attack.md` §6):

| Nơi | Mẫu cũ | Dùng cho |
|---|---|---|
| `check-wake-paths.py` | `RE_TEST_ONLY_WAKE_SCOPE` (`v2/(kernel|sim)/<crate>/(src/**.rs|build.rs)`) ∪ crate root thật | luật token `wake` |
| `check-wake-depinfo.py` | `RE_CRATE_SRC` (`v2/(lib|kernel|sim|tools)/<crate>/src/`) | dự phòng khi không có `Scope` |
| `check-wake-paths.py` | `MACRO_SCAN_ROOTS` (`v2/lib`, `v2/kernel`, `v2/sim`) | thân `macro_rules!` |

Ba mẫu **không** phải một định nghĩa: crate thêm vào workspace ở thư mục gốc **mới** (ví dụ
`v2/drivers/<crate>`) nằm ngoài **cả ba**; `v2/tools/<crate>` phụ thuộc crate bảo vệ được lớp
dep-info **nâng** lên bảo vệ nhưng lớp tĩnh vẫn mù; tệp sản phẩm **ngoài `src/`** (chương trình
U-mode `init/`, shim `vifast-plat-user/`) chỉ được luật macro che, không được luật token che.

**D2-188** chốt: danh sách crate bảo vệ **phải sinh từ `cargo metadata`** của các workspace liên
quan, cộng **bao đóng dependency nội bộ**; **không** dùng đường dẫn cố định làm định nghĩa đầy đủ
của phạm vi. Tệp này là nguồn duy nhất đó — **cả hai** checker import nó, không chép luật.

Định nghĩa
----------
1. Chạy `cargo metadata --no-deps --format-version 1` cho **mọi** workspace sản phẩm
   (`WORKSPACES`: host `v2/Cargo.toml`, bare-metal `v2/kernel/Cargo.toml` — D2-26).
2. Lấy **thành viên** + **bao đóng path-dependency nội bộ** (crate `path` trỏ vào repo).
3. Phân loại: **bảo vệ** (sản phẩm) / **từ chối rõ** (công cụ — `v2/tools/`), kèm lý do; crate
   "từ chối" mà bao đóng dependency chứa crate bảo vệ ⇒ **nâng** lên bảo vệ (fail-closed).
4. `roots()` = thư mục crate **bảo vệ** ∪ `NON_CRATE_SOURCE_TREES` (cây nguồn **không phải crate**
   nhưng được biên dịch vào ảnh sản phẩm — lý do ghi kèm, không phải mẫu đường dẫn).
5. Ba **view** cho ba luật, tất cả suy từ cùng một `ProtectedScope`:
   - `contains(relp)` — tệp thuộc phạm vi bảo vệ (dùng cho `hidden_files` + luật macro);
   - `token_scope(relp)` — luật token `wake` (ảnh sản phẩm + sim — xem `TOKEN_RULE_*`);
   - `macro_roots()` — gốc quét thân `macro_rules!`.

**Không giảm phạm vi**: `check_no_reduction(root)` đối chiếu mọi tệp `.rs` dưới `LEGACY_ROOTS`
(ba gốc của ba mẫu cũ) với `contains()`; tệp nào ngoài ⇒ lỗi `E_SCOPE_REDUCTION`. Đây là **lưới
an toàn di trú**, không phải định nghĩa: nếu `cargo metadata` bỏ sót một cây nguồn từng được mẫu
cũ che, cổng **ĐỎ** thay vì âm thầm hẹp lại.

**DEBT-KT227-1** — `v2/lib` **ngoài** luật token `wake` (luật chỉ áp ảnh sản phẩm + sim). Điều
này chỉ an toàn khi **không** crate `v2/lib` nào phụ thuộc `vifast-core` (normal dependency):
không có crate đó thì `wake()` **không gọi được** từ mã sản phẩm `v2/lib`. `check_lib_core_dependency()`
canh đúng điều kiện đó ⇒ có crate `v2/lib` phụ thuộc `vifast-core` thì **ĐỎ** (`E_SCOPE_LIB_CORE`),
buộc người thêm phải mở rộng luật (hoặc gỡ phụ thuộc).

Dùng
----
    from wake_scope import protected_scope, load_scope, scope_from_metadata
    sc = protected_scope(root)          # cache theo gốc cây
    sc.contains("v2/kernel/vifast-core/src/lib.rs")   # True
    sc.token_scope("v2/lib/vifast-abi/src/lib.rs")    # False (DEBT-KT227-1)
"""

from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Set, Tuple

# ------------------------------------------------------------------ hằng số

# Hai workspace sản phẩm (D2-26): host (`v2/Cargo.toml`) và bare-metal (`v2/kernel/Cargo.toml`).
WORKSPACES: Tuple[Tuple[str, str], ...] = (
    ("host", "v2/Cargo.toml"),
    ("kernel", "v2/kernel/Cargo.toml"),
)

# Tiền tố crate **công cụ**: thành viên workspace nhưng **không** phải sản phẩm ⇒ "từ chối rõ".
REFUSED_PREFIXES: Tuple[str, ...] = ("v2/tools/",)

# Cây nguồn **không phải crate** nhưng **được biên dịch vào ảnh sản phẩm** ⇒ phải nằm trong phạm
# vi. Đây là **ngoại lệ có lý do**, không phải mẫu đường dẫn: mỗi mục là một cây cụ thể, và
# `check_no_reduction` vẫn đối chiếu với ba gốc cũ.
NON_CRATE_SOURCE_TREES: Tuple[Tuple[str, str], ...] = (
    (
        "v2/kernel/vifast-plat-user",
        "shim ABI U-mode (không phải crate): biên dịch bởi vifast-kbin-riscv64/build.rs, "
        "đóng vào ảnh sản phẩm",
    ),
)

# Ba gốc của **ba mẫu cũ** (KT-209/L2, KT-181, KT-238). Chỉ dùng cho `check_no_reduction` —
# **không** dùng làm định nghĩa phạm vi (D2-188).
LEGACY_ROOTS: Tuple[str, ...] = ("v2/lib", "v2/kernel", "v2/sim")

# Luật token `wake` áp cho **ảnh sản phẩm** (workspace `kernel`) + **sim** (oracle chạy cùng
# ngữ nghĩa, tự định nghĩa `fn wake` — `v2/sim/vifast-sim/src/kobj.rs`). Đây là **chính sách của
# luật**, không phải định nghĩa phạm vi: `v2/lib` nằm ngoài luật token, xem DEBT-KT227-1 và
# `check_lib_core_dependency()`.
TOKEN_RULE_WORKSPACES: Tuple[str, ...] = ("kernel",)
TOKEN_RULE_EXTRA_CRATES: Tuple[str, ...] = ("vifast-sim",)

# Crate lõi: `wake()` là API của nó. `v2/lib` phụ thuộc nó (normal) ⇒ luật token phải mở rộng.
CORE_CRATE = "vifast-core"

# Dự phòng **khi cây không có workspace manifest** (cây fixture của selftest): mẫu cũ, giữ nguyên
# hành vi cho các ca đã có. Trên cây thật (có `v2/Cargo.toml`/`v2/kernel/Cargo.toml`) **không**
# dùng mẫu này — metadata là định nghĩa.
_FALLBACK_CRATE = r"^v2/(?:lib|kernel|sim|tools)/[^/]+/"
_FALLBACK_TOKEN = r"^v2/(?:kernel|sim)/[^/]+/"


# ------------------------------------------------------------------ kiểu dữ liệu


class CrateScope:
    """Một crate trong phạm vi, sinh từ `cargo metadata`."""

    __slots__ = (
        "name",
        "dir_rel",
        "manifest_rel",
        "deps",
        "normal_deps",
        "target_names",
        "src_paths",
        "workspace",
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
        normal_deps: Optional[Sequence[str]] = None,
        workspace: str = "",
    ) -> None:
        self.name = name
        self.dir_rel = dir_rel
        self.manifest_rel = manifest_rel
        self.deps = list(deps)
        # `normal_deps` = path-dependency **kind = null** (không dev/build). Dùng cho
        # `check_lib_core_dependency` (DEBT-KT227-1): dev-dependency không làm `wake()` gọi được
        # từ **mã sản phẩm**.
        self.normal_deps = list(normal_deps if normal_deps is not None else deps)
        self.target_names = list(target_names)
        self.src_paths = list(src_paths)
        self.workspace = workspace
        self.status = "protected"
        self.reason = ""

    def __repr__(self) -> str:  # pragma: no cover - chẩn đoán
        return f"CrateScope({self.name!r}, {self.dir_rel!r}, {self.status!r})"


class ProtectedScope:
    """Phạm vi bảo vệ sinh từ `cargo metadata` + bao đóng dependency nội bộ."""

    def __init__(self) -> None:
        self.crates: Dict[str, CrateScope] = {}  # dir_rel → crate
        self.by_name: Dict[str, CrateScope] = {}
        self.by_target: Dict[str, CrateScope] = {}
        self.target_dirs: Dict[str, Path] = {}  # đơn vị build → target dir (tuyệt đối)
        self.notes: List[str] = []
        # `True` = sinh từ `cargo metadata`; `False` = dự phòng mẫu (cây không có manifest).
        self.from_metadata = False
        # Lỗi dựng phạm vi (fail-closed): tầng gọi **phải** biến nó thành lỗi cổng.
        self.error: Optional[str] = None

    # -- phân loại ---------------------------------------------------------

    def classify(self, relp: str) -> Optional[CrateScope]:
        """Crate chứa `relp` (khớp tiền tố **dài nhất**), hoặc `None` nếu ngoài mọi crate."""
        best: Optional[CrateScope] = None
        for d, c in self.crates.items():
            if relp == d or relp.startswith(d + "/"):
                if best is None or len(d) > len(best.dir_rel):
                    best = c
        return best

    def protected(self) -> List[CrateScope]:
        return sorted(
            (c for c in self.crates.values() if c.status == "protected"),
            key=lambda c: c.dir_rel,
        )

    def refused(self) -> List[CrateScope]:
        return sorted(
            (c for c in self.crates.values() if c.status == "refused"),
            key=lambda c: c.dir_rel,
        )

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

    # -- view cho từng luật ------------------------------------------------

    def roots(self) -> List[str]:
        """Gốc nguồn trong phạm vi: thư mục crate bảo vệ ∪ cây nguồn không-phải-crate."""
        out = [c.dir_rel for c in self.protected()]
        out.extend(d for d, _why in NON_CRATE_SOURCE_TREES)
        return sorted(set(out))

    def macro_roots(self) -> List[str]:
        """Gốc quét thân `macro_rules!` — **mọi** tệp nguồn trong phạm vi."""
        return self.roots()

    def contains(self, relp: str) -> bool:
        """`relp` thuộc phạm vi bảo vệ (crate bảo vệ hoặc cây nguồn không-phải-crate)?"""
        if not self.from_metadata:
            return _fallback_match(_FALLBACK_CRATE, relp)
        for d, _why in NON_CRATE_SOURCE_TREES:
            if relp == d or relp.startswith(d + "/"):
                return True
        c = self.classify(relp)
        return c is not None and c.status == "protected"

    def token_scope(self, relp: str) -> bool:
        """Luật token `wake` có áp cho `relp` không (ảnh sản phẩm + sim + cây sản phẩm).

        **DEBT-KT227-1** — `v2/lib` **ngoài** luật (xem `check_lib_core_dependency`).
        """
        if not self.from_metadata:
            return _fallback_match(_FALLBACK_TOKEN, relp)
        for d, _why in NON_CRATE_SOURCE_TREES:
            if relp == d or relp.startswith(d + "/"):
                return True
        c = self.classify(relp)
        if c is None or c.status != "protected":
            return False
        return c.workspace in TOKEN_RULE_WORKSPACES or c.name in TOKEN_RULE_EXTRA_CRATES

    # -- kiểm bất biến (fail-closed) ---------------------------------------

    def check_no_reduction(self, root: Path) -> List[str]:
        """Mọi tệp `.rs` dưới `LEGACY_ROOTS` phải còn trong phạm vi — nếu không ⇒ lỗi.

        Lưới an toàn **di trú** (KT-256): ba mẫu cũ che `v2/lib`+`v2/kernel`+`v2/sim`; định nghĩa
        mới **không** được hẹp lại ở bất kỳ đâu. Chỉ chạy khi phạm vi sinh từ metadata (cây thật).
        """
        if not self.from_metadata:
            return []
        out: List[str] = []
        for base_rel in LEGACY_ROOTS:
            base = root / base_rel
            if not base.is_dir():
                continue
            for p in sorted(base.rglob("*")):
                if not p.is_file() or p.suffix.lower() != ".rs":
                    continue
                try:
                    relp = str(p.relative_to(root))
                except ValueError:  # pragma: no cover - phòng hờ
                    continue
                # Thư mục build của cargo (`target/` **trước** `src/`) — cùng quy tắc với
                # `check-wake-paths._is_build_dir` (KT-215): nguồn sinh trong `OUT_DIR` do lớp
                # dep-info kiểm, không phải lưới "không giảm phạm vi" của lớp tĩnh.
                if _is_build_dir_parts(Path(relp).parts):
                    continue
                if not self.contains(relp):
                    out.append(
                        f"  {relp}: nằm dưới `{base_rel}` (mẫu cũ che) nhưng **ngoài** phạm vi "
                        f"sinh từ cargo metadata — phạm vi bị **giảm** (fail-closed, KT-256/L2)"
                    )
        return out

    def check_lib_core_dependency(self) -> List[str]:
        """**DEBT-KT227-1** — canh điều kiện làm `v2/lib` ngoài luật token `wake` là **an toàn**.

        Luật token chỉ áp ảnh sản phẩm + sim. `v2/lib` chỉ an toàn khi **không** crate nào phụ
        thuộc `vifast-core` bằng **normal dependency** (dev/build không tính: chúng không làm
        `wake()` gọi được từ mã sản phẩm). Có crate như vậy ⇒ **ĐỎ**, buộc mở rộng luật.
        """
        if not self.from_metadata:
            return []
        out: List[str] = []
        for c in self.protected():
            if not c.dir_rel.startswith("v2/lib/"):
                continue
            closure = self._normal_closure(c)
            hit = sorted(n for n in closure if n == CORE_CRATE)
            if hit:
                out.append(
                    f"  {c.name} ({c.dir_rel}): phụ thuộc `{CORE_CRATE}` bằng **normal "
                    f"dependency** ⇒ `wake()` gọi được từ mã sản phẩm `v2/lib`, nhưng luật token "
                    f"`wake` chỉ áp ảnh sản phẩm + sim (DEBT-KT227-1) — **mở rộng luật** "
                    f"(`TOKEN_RULE_WORKSPACES`/`token_scope`) hoặc gỡ phụ thuộc (fail-closed)"
                )
        return out

    def _normal_closure(self, c: CrateScope) -> Set[str]:
        """Tên crate trong bao đóng **normal dependency** (bắc cầu) của `c`."""
        seen: Set[str] = set()
        stack = list(c.normal_deps)
        while stack:
            d = stack.pop()
            if d in seen:
                continue
            seen.add(d)
            dc = self.crates.get(d)
            if dc is not None:
                stack.extend(dc.normal_deps)
        return {self.crates[d].name for d in seen if d in self.crates}


def _is_build_dir_parts(parts: Sequence[str]) -> bool:
    """`target/` **trước** `src/` ⇒ thư mục build của cargo (cùng quy tắc `_is_build_dir`, KT-215)."""
    if "target" not in parts:
        return False
    ti = parts.index("target")
    if "src" not in parts:
        return True
    return ti < parts.index("src")


def _fallback_match(pattern: str, relp: str) -> bool:
    return re.match(pattern, relp) is not None


def fallback_token_scope(relp: str) -> bool:
    """Mẫu dự phòng của luật token (chỉ dùng khi **không** có gốc cây — caller cũ/selftest)."""
    return _fallback_match(_FALLBACK_TOKEN, relp)


# ------------------------------------------------------------------ dựng phạm vi


def _rel_or_none(root: Path, p: Path) -> Optional[str]:
    try:
        return str(p.resolve().relative_to(root))
    except (ValueError, OSError):
        return None


def _crate_from_package(
    root: Path, pkg: dict, workspace: str = ""
) -> Optional[CrateScope]:
    """Một mục `packages[]` của `cargo metadata` ⇒ `CrateScope` (bỏ crate ngoài gốc repo)."""
    mp = pkg.get("manifest_path")
    if not mp:
        return None
    manifest = Path(mp)
    dir_rel = _rel_or_none(root, manifest.parent)
    if dir_rel is None:
        return None
    manifest_rel = _rel_or_none(root, manifest) or dir_rel
    deps: List[str] = []
    normal_deps: List[str] = []
    for d in pkg.get("dependencies") or []:
        p = d.get("path")
        if not p:
            continue
        r = _rel_or_none(root, Path(p))
        if r is None:
            continue
        deps.append(r)
        # `kind` = None ⇒ normal dependency; "dev"/"build" ⇒ không tính cho DEBT-KT227-1.
        if d.get("kind") is None:
            normal_deps.append(r)
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
    return CrateScope(
        pkg.get("name") or dir_rel,
        dir_rel,
        manifest_rel,
        deps,
        target_names,
        src_paths,
        normal_deps=normal_deps,
        workspace=workspace,
    )


def _dep_closure(sc: ProtectedScope, c: CrateScope) -> Set[str]:
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


def _classify_scope(sc: ProtectedScope) -> None:
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


def scope_from_metadata(
    root: Path, metas: Sequence[Tuple[str, dict]]
) -> ProtectedScope:
    """Dựng `ProtectedScope` từ các blob `cargo metadata --no-deps` — hàm **thuần**.

    `metas` = `[(tên đơn vị, blob), …]`. Không chạy cargo, không đọc đĩa (trừ `root` để tính
    đường dẫn tương đối). Selftest dùng hàm này để chứng minh định nghĩa **có răng** mà không
    cần cargo.
    """
    root = root.resolve()
    sc = ProtectedScope()
    sc.from_metadata = True
    for unit, meta in metas:
        td = meta.get("target_directory")
        if td:
            sc.target_dirs[unit] = Path(td).resolve()
        for pkg in meta.get("packages") or []:
            c = _crate_from_package(root, pkg, unit)
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
) -> ProtectedScope:
    """Sinh phạm vi từ `cargo metadata` của **các workspace liên quan** + bao đóng path-dep.

    `target_overrides` = `{đơn vị: target dir}` (dùng cho `--fresh`: `CARGO_TARGET_DIR` tạm) —
    truyền **cùng** biến môi trường cho `cargo metadata` để `target_directory` khớp lần build.
    """
    overrides = target_overrides or {}
    if cargo is None:
        sc = ProtectedScope()
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
                c = _crate_from_package(root, pkg, "closure")
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


def fallback_scope(root: Path) -> ProtectedScope:
    """Phạm vi **dự phòng** cho cây **không có** workspace manifest (cây fixture của selftest).

    Giữ nguyên hành vi của ba mẫu cũ cho các ca đã có; **không** dùng trên cây thật. Mọi thư mục
    cấp 1 dưới `v2/{lib,kernel,sim,tools}` được coi là một crate (không cần `Cargo.toml` — cây
    fixture không có), `v2/tools/*` là "từ chối rõ" như metadata.
    """
    sc = ProtectedScope()
    sc.from_metadata = False
    sc.notes.append("cây không có workspace manifest ⇒ phạm vi dự phòng theo mẫu (KT-256)")
    for base_rel in ("v2/lib", "v2/kernel", "v2/sim", "v2/tools"):
        base = root / base_rel
        if not base.is_dir():
            continue
        for d in sorted(p for p in base.iterdir() if p.is_dir()):
            if d.name in ("target", "node_modules") or d.name.startswith("."):
                continue
            relp = f"{base_rel}/{d.name}"
            ws = "kernel" if base_rel == "v2/kernel" else "host"
            c = CrateScope(d.name, relp, f"{relp}/Cargo.toml", [], [], [], workspace=ws)
            if base_rel == "v2/tools":
                c.status = "refused"
                c.reason = "công cụ (v2/tools/), không phải sản phẩm"
            sc.crates[relp] = c
            sc.by_name[c.name] = c
    return sc


# Cache theo gốc cây (selftest dựng nhiều cây tạm; cây thật chỉ dựng một lần).
_SCOPE_CACHE: Dict[str, ProtectedScope] = {}


def protected_scope(root: Path) -> ProtectedScope:
    """**Điểm vào duy nhất**: phạm vi bảo vệ của cây `root` (cache theo gốc).

    Có `v2/Cargo.toml` hoặc `v2/kernel/Cargo.toml` ⇒ **bắt buộc** sinh từ `cargo metadata`
    (D2-188). Không dựng được ⇒ `scope.error` được đặt (tầng gọi **phải** biến thành lỗi cổng)
    và phạm vi **vẫn** dự phòng theo mẫu để không quét-rồi-PASS âm thầm.
    """
    key = str(root.resolve())
    cached = _SCOPE_CACHE.get(key)
    if cached is not None:
        return cached
    root = root.resolve()
    has_ws = (root / "v2/Cargo.toml").is_file() or (root / "v2/kernel/Cargo.toml").is_file()
    if not has_ws:
        sc = fallback_scope(root)
        _SCOPE_CACHE[key] = sc
        return sc
    cargo = find_cargo()
    if cargo is None:
        sc = fallback_scope(root)
        sc.error = (
            "có workspace manifest nhưng không tìm thấy `cargo` — không dựng được phạm vi từ "
            "cargo metadata (fail-closed, D2-188)"
        )
        _SCOPE_CACHE[key] = sc
        return sc
    sc = load_scope(root, cargo, pinned_toolchain(root))
    if not sc.crates:
        sc.error = (
            "cargo metadata trả 0 crate — không dựng được phạm vi bảo vệ (fail-closed, D2-188)"
        )
    _SCOPE_CACHE[key] = sc
    return sc


def clear_cache() -> None:
    """Xoá cache phạm vi (selftest dựng nhiều cây; tầng sản xuất không cần)."""
    _SCOPE_CACHE.clear()


# ------------------------------------------------------------------ môi trường


def find_cargo() -> Optional[str]:
    """Tìm `cargo` — ưu tiên rustup shim trong `$HOME/.cargo/bin` (toolchain ghim)."""
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
    """Toolchain ghim của dòng OS (`v2/rust-toolchain.toml` hoặc `v2/kernel/rust-toolchain.toml`)."""
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
