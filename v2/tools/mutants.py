#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""mutants.py — **`v2 mutants`**: chạy đột biến theo manifest `v2/test/mutants.toml` (KT-403).

Vì sao có tệp này
-----------------
Trước KT-403, bộ kiểm có **43 khai báo feature `mut-*`** rải trong 7 `Cargo.toml` + `selfbreak`,
không có manifest, không có lệnh chạy: mỗi lần dùng là sửa tay `--features`, không ai đếm được
đột biến nào còn sống. Luật D2-113F đòi mỗi phép kiểm phải chứng minh **có răng**; thiết kế §9.9
đòi "đột biến sống = lỗ hổng bộ kiểm, mở task bổ sung kịch bản".

Cơ chế (không bao giờ sửa cây thật — D2-113F)
--------------------------------------------
- `kind = "feature"`: chép **bản sao** của cây (`v2/`, `spec/`, `docs/`, `Cargo.toml`) vào
  `mktemp -d`, rồi lật cfg của **đúng một** đột biến:
      `not(feature = "X")` → `any()`        (nhánh gốc tắt)
      `feature = "X"`     → `all()`          (nhánh đột biến bật)
  Phải có ≥ 1 điểm cfg, nếu không ⇒ `E_STALE` (đột biến đã chết vì mã đổi — sửa manifest).
- `kind = "build"`: như `feature` nhưng đột biến do **`build.rs` cấp `--cfg`** (hoặc do **đồ thị
  feature** của Cargo) nên không lật được bằng chuỗi `feature = "X"` trong `.rs`. Trên bản sao,
  công cụ áp danh sách `patches` của manifest (thay chuỗi trong `build.rs`/`Cargo.toml` của bản
  sao) để bật đúng `--cfg`/nhãn cũ; `patches` nào không khớp ⇒ `E_STALE`.
- `kind = "script"`: chạy thẳng lệnh (self-test tự bảo vệ cây thật, vd `selfbreak.sh`).

**KT-403 r2 (D2-259)**: 43 khai báo feature `mut-*` ad-hoc trong 7 `Cargo.toml` đã được **gỡ**;
manifest là nguồn duy nhất. Vì vậy:
  - `declarations = 43` là **số khai báo đã thay thế** (đóng băng tại `67b4ee2a`), không phải số
    khai báo còn trong cây;
  - `--lint` đỏ nếu còn **bất kỳ** khai báo `mut-*` nào trong `v2/kernel/*/Cargo.toml`
    (`E_LEFTOVER`) hoặc nếu `covered != declarations`;
  - mọi mục đều `wired = true`: `--all` không còn `E_UNWIRED`.

Phân loại kết quả
-----------------
| Kết quả | Nghĩa |
|---|---|
| `KILLED` | lệnh ĐỎ **và** marker khớp ⇒ bộ kiểm bắt được đột biến |
| `SURVIVED` | lệnh XANH ⇒ đột biến sống = lỗ hổng bộ kiểm (exit 1) |
| `E_WRONG-REASON` | lệnh ĐỎ nhưng marker không khớp ⇒ đỏ vì lý do khác (không tính là bắt) |
| `E_STALE` | 0 điểm cfg còn tồn tại (hoặc `patches` của `kind = "build"` không khớp) |
| `E_UNWIRED` | `wired = false` — không còn mục nào (giữ nhánh phân loại cho manifest sai) |

`--selftest` dựng một cây mini trong `/tmp` và chứng minh **cả 5** kết luận trên (đỏ-trước cho
chính công cụ): mỗi ca có kỳ vọng riêng, sai kỳ vọng ⇒ exit 1.

Chạy
----
    python3 v2/tools/mutants.py --root <REPO> --list
    python3 v2/tools/mutants.py --root <REPO> --lint
    python3 v2/tools/mutants.py --root <REPO> mut-no-irq
    python3 v2/tools/mutants.py --root <REPO> --all
    python3 v2/tools/mutants.py --selftest

Exit: 0 = KILLED (hoặc lint/selftest xanh); 1 = SURVIVED / sai lý do / lint đỏ;
      2 = dùng sai, manifest hỏng, `E_STALE`, hoặc mutant chưa nối; 3 = `--all` CHƯA NỐI ĐỦ.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

MANIFEST_REL = "v2/test/mutants.toml"
CFG_FEATURE = 'feature = "%s"'
CFG_NOT = 'not(feature = "%s")'
KINDS = ("feature", "script", "build")
EXPECTS = ("red", "pass")
COPY_ENTRIES = ("v2", "spec", "docs")
COPY_FILES = ("Cargo.toml", "rust-toolchain.toml", "AGENTS.md")


class ManifestError(Exception):
    """Manifest hỏng/dùng sai ⇒ exit 2."""


def _tomllib():
    try:
        import tomllib  # type: ignore[import-not-found]
    except Exception:  # pragma: no cover
        raise ManifestError("cần python ≥ 3.11 có `tomllib`")
    return tomllib


# --------------------------------------------------------------------------- manifest


def load_manifest(root):
    path = os.path.join(root, MANIFEST_REL)
    if not os.path.isfile(path):
        raise ManifestError("thiếu %s" % MANIFEST_REL)
    tomllib = _tomllib()
    try:
        with open(path, "rb") as fh:
            data = tomllib.load(fh)
    except Exception as exc:  # noqa: BLE001
        raise ManifestError("không parse được %s: %s" % (MANIFEST_REL, exc))
    if data.get("version") != 1:
        raise ManifestError("version=%r (cần 1)" % (data.get("version"),))
    if not isinstance(data.get("suite"), list) or not data["suite"]:
        raise ManifestError("thiếu mảng `[[suite]]`")
    if not isinstance(data.get("mutant"), list) or not data["mutant"]:
        raise ManifestError("thiếu mảng `[[mutant]]`")
    return data


def suites_by_id(data):
    suites = {}
    for s in data["suite"]:
        sid = s.get("id")
        if not sid or sid in suites:
            raise ManifestError("suite id thiếu/trùng: %r" % (sid,))
        for key in ("command", "marker"):
            if not isinstance(s.get(key), str) or not s[key]:
                raise ManifestError("suite %s thiếu `%s`" % (sid, key))
        suites[sid] = s
    return suites


def declarations(root):
    """Khai báo `mut-*` **còn sót** trong `[features]` của `v2/kernel/*/Cargo.toml`.

    Từ KT-403 r2 (D2-259) manifest là nguồn duy nhất: phép kiểm này phải trả **rỗng**; còn phần
    tử nào ⇒ `E_LEFTOVER` (ai đó đang dựng lại cơ chế feature ad-hoc).
    """
    out = []
    kernel = os.path.join(root, "v2", "kernel")
    for name in sorted(os.listdir(kernel)):
        manifest = os.path.join(kernel, name, "Cargo.toml")
        if not os.path.isfile(manifest):
            continue
        with open(manifest, "r", encoding="utf-8") as fh:
            text = fh.read()
        crate = re.search(r'^name\s*=\s*"([^"]+)"', text, re.MULTILINE)
        if not crate:
            continue
        section = ""
        for line in text.splitlines():
            stripped = line.strip()
            head = re.match(r"^\[([^\]]+)\]$", stripped)
            if head:
                section = head.group(1).strip()
                continue
            if section != "features":
                continue
            m = re.match(r"^(mut-[A-Za-z0-9_-]+)\s*=", stripped)
            if m:
                out.append((crate.group(1), m.group(1)))
    return out


def cfg_sites(root, feature):
    """Số điểm cfg `feature = "<id>"` trong `v2/kernel/**/*.rs` (không tính chú thích? — tính hết,
    vì `cfg!()` / `#[cfg]` đều là điểm thật; một dòng chú thích có chuỗi này vẫn là tín hiệu
    manifest còn sống, và phép thử "0 điểm" chỉ để bắt đột biến đã bị xoá hẳn)."""
    needle = CFG_FEATURE % feature
    count = 0
    kernel = os.path.join(root, "v2", "kernel")
    for dirpath, _dirnames, filenames in os.walk(kernel):
        for name in filenames:
            if not name.endswith(".rs"):
                continue
            with open(os.path.join(dirpath, name), "r", encoding="utf-8", errors="replace") as fh:
                count += fh.read().count(needle)
    return count


def lint(root, data):
    problems = []
    suites = suites_by_id(data)
    seen = set()
    covered = set()
    n_feature = 0
    for m in data["mutant"]:
        mid = m.get("id")
        if not mid:
            problems.append("E_MANIFEST mutant thiếu `id`")
            continue
        if mid in seen:
            problems.append("E_MANIFEST id trùng: %s" % mid)
        seen.add(mid)
        kind = m.get("kind")
        if kind not in KINDS:
            problems.append("E_MANIFEST %s: kind=%r" % (mid, kind))
            continue
        if m.get("expect") not in EXPECTS:
            problems.append("E_MANIFEST %s: expect=%r" % (mid, m.get("expect")))
        suite = m.get("suite")
        if suite not in suites:
            problems.append("E_MANIFEST %s: suite lạ %r" % (mid, suite))
        if not isinstance(m.get("wired"), bool):
            problems.append("E_MANIFEST %s: thiếu `wired` (bool)" % mid)
        if "marker" in m and (not isinstance(m["marker"], str) or not m["marker"]):
            problems.append("E_MANIFEST %s: marker phải là chuỗi khác rỗng" % mid)
        if kind == "build":
            n_feature += 1
            feature = m.get("feature")
            if not isinstance(feature, str) or not feature.startswith("mut-"):
                problems.append("E_MANIFEST %s: feature=%r" % (mid, feature))
                continue
            crates = m.get("crates") or []
            covered.add((crates[0] if crates else "?", feature))
            # **KT-403 r2** — đột biến `build` không còn feature Cargo để bật: bằng chứng là
            # `patches` áp lên **bản sao**. Mỗi `find` phải còn khớp trong tệp thật, nếu không
            # ⇒ đột biến đã chết (mã đổi mà manifest không đổi) ⇒ `E_STALE`.
            patches = m.get("patches")
            if not isinstance(patches, list) or not patches:
                problems.append("E_MANIFEST %s: kind=build cần mảng `patches`" % mid)
                continue
            for p in patches:
                if not isinstance(p, dict) or not all(
                    isinstance(p.get(k), str) and p.get(k) for k in ("file", "find", "replace")
                ):
                    problems.append("E_MANIFEST %s: patch cần `file`/`find`/`replace` (chuỗi)" % mid)
                    continue
                path = os.path.join(root, p["file"])
                if not os.path.isfile(path):
                    problems.append("E_STALE %s: không thấy tệp patch %s" % (mid, p["file"]))
                    continue
                with open(path, "r", encoding="utf-8", errors="replace") as fh:
                    if p["find"] not in fh.read():
                        problems.append(
                            "E_STALE %s: `find` không khớp trong %s" % (mid, p["file"])
                        )
            continue
        if kind == "feature":
            n_feature += 1
            feature = m.get("feature")
            if not isinstance(feature, str) or not feature.startswith("mut-"):
                problems.append("E_MANIFEST %s: feature=%r" % (mid, feature))
                continue
            crates = m.get("crates")
            if not isinstance(crates, list) or not all(isinstance(c, str) for c in crates):
                problems.append("E_MANIFEST %s: crates phải là mảng chuỗi" % mid)
                continue
            for c in crates:
                covered.add((c, feature))
            sites = cfg_sites(root, feature)
            if sites == 0:
                problems.append("E_STALE %s: 0 điểm cfg `%s` trong v2/kernel" % (mid, feature))
    decls = declarations(root)
    want = data.get("declarations")
    for crate, feature in sorted(decls):
        problems.append(
            "E_LEFTOVER khai báo (%s, %s): manifest là nguồn duy nhất — gỡ khỏi `Cargo.toml`"
            " (KT-403 r2, D2-259)"
            % (crate, feature)
        )
    if isinstance(want, int) and want != len(covered):
        problems.append(
            "E_MANIFEST covered=%d, manifest ghi declarations=%d" % (len(covered), want)
        )
    print(
        "# mutants lint: mutants=%d feature=%d declarations=%s covered=%d leftover=%d"
        " suites=%d problems=%d"
        % (
            len(data["mutant"]),
            n_feature,
            want if isinstance(want, int) else "?",
            len(covered),
            len(decls),
            len(suites),
            len(problems),
        )
    )
    for p in problems:
        print(p)
    return problems


# --------------------------------------------------------------------------- chạy một đột biến


KT407_TARGET_LINKS = (
    os.path.join("v2", "kernel", "target"),
    os.path.join("v2", "target"),
)


def _copy_tree(root, dest):
    """Bản sao **đủ để harness chạy**: `v2/`, `spec/`, `docs/` + tệp gốc. Bỏ `target/`."""
    os.makedirs(dest, exist_ok=True)
    ignore = shutil.ignore_patterns("target", ".git")
    for name in COPY_ENTRIES:
        src = os.path.join(root, name)
        if os.path.isdir(src):
            shutil.copytree(src, os.path.join(dest, name), ignore=ignore, symlinks=False)
    for name in COPY_FILES:
        src = os.path.join(root, name)
        if os.path.isfile(src):
            shutil.copy2(src, os.path.join(dest, name))
    # **Nợ KT-407** — runner boot tìm ảnh ở `<repo>/v2/kernel/target/...` và ảnh sản phẩm tìm
    # `vifast-fsutil` ở `<repo>/v2/target/release/` (đường dẫn **cứng**). Khi các slot dùng
    # `CARGO_TARGET_DIR` dùng chung, bản sao không có hai đường dẫn đó ⇒ ĐỎ **giả** vì hạ tầng.
    # Cách tránh đã ghi trong KT-403: trỏ **bản sao** về target dir thật (không sửa cây thật).
    tgt = os.environ.get("CARGO_TARGET_DIR")
    if tgt and os.path.isdir(tgt):
        for rel in KT407_TARGET_LINKS:
            path = os.path.join(dest, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            if not os.path.lexists(path):
                os.symlink(tgt, path)
    return dest


def apply_feature(copy_root, feature):
    """Lật cfg của **một** đột biến trên bản sao; trả số lần thay thế."""
    needle_not = CFG_NOT % feature
    needle = CFG_FEATURE % feature
    n = 0
    kernel = os.path.join(copy_root, "v2", "kernel")
    for dirpath, _dirnames, filenames in os.walk(kernel):
        for name in filenames:
            if not name.endswith(".rs"):
                continue
            path = os.path.join(dirpath, name)
            with open(path, "r", encoding="utf-8", errors="replace") as fh:
                text = fh.read()
            if needle not in text:
                continue
            new = text.replace(needle_not, "any()").replace(needle, "all()")
            if new != text:
                # `not()` đã bị thay trước nên `feature = "X"` chỉ còn ở nhánh đột biến.
                n += text.count(needle)
                with open(path, "w", encoding="utf-8") as fh:
                    fh.write(new)
    return n


def apply_patches(copy_root, patches):
    """Áp `patches` của `kind = "build"` lên **bản sao**; trả `(đã-áp, thiếu)`.

    Vì sao cần: đột biến `build` nằm sau `--cfg` do `build.rs` cấp (hoặc sau **đồ thị feature**
    của Cargo) — không có chuỗi `feature = "X"` nào trong `.rs` để lật. Bản sao nhận đúng bản vá
    mà cây thật **không** được phép có (D2-113F).
    """
    applied = 0
    missing = []
    for p in patches:
        path = os.path.join(copy_root, p["file"])
        if not os.path.isfile(path):
            missing.append("%s (thiếu tệp)" % p["file"])
            continue
        with open(path, "r", encoding="utf-8", errors="replace") as fh:
            text = fh.read()
        if p["find"] not in text:
            missing.append("%s (find không khớp)" % p["file"])
            continue
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(text.replace(p["find"], p["replace"], 1))
        applied += 1
    return applied, missing


# **KT-408c** — danh tính git **cố định** cho cây tạm của bản sao: commit của mỗi lượt chạy không
# phụ thuộc `~/.gitconfig` của máy đo (và không phụ thuộc giờ chạy ⇒ cùng đầu vào, cùng commit).
GIT_NAME = "vifast-mutants"
GIT_EMAIL = "mutants@vifast.invalid"
GIT_DATE = "2026-01-01T00:00:00+0000"

# Hai **liên kết** target của KT-407 (`_copy_tree`) nằm trong gốc build-inputs (`v2/kernel`,
# `v2/target`). `.gitignore` gốc chỉ có `target/` (mẫu **thư mục**), mà git coi symlink là tệp ⇒
# phải ghim **đường dẫn** thì `git status` của bản sao mới sạch. Đây là tệp của **bản sao**, không
# phải cây thật, và `.gitignore` **không** nằm trong build-inputs (`VBREV_INPUT_FILES`) nên không
# đổi dấu vân tay.
GIT_IGNORE_EXTRA = (
    "# KT-408c: liên kết target của bản sao đột biến (KT-407) — không phải đầu vào build",
    "v2/kernel/target",
    "v2/target",
    "",
)


def _git_snapshot(root, copy_root, label):
    """Tạo **git tree tạm trong bản sao** để cổng `rev-gate` (KT-86/KT-89) thấy `dirty=0` thật.

    Vì sao cần (KT-403 §Phát hiện 1): `_copy_tree` cố ý bỏ `.git`; suite `ktest-riscv64` gọi
    `v2/test/boot/ktest.sh`, mà bước `rev-gate` đòi ảnh chứng minh được nguồn gốc
    (`vbrev_toplevel` + `git status` trên tập build-inputs). Bản sao không có git ⇒
    `REV-GATE: REFUSE reason=no-git` ⇒ runner `exit 3` **trước** khi chạy QEMU ⇒ mọi đột biến
    riscv64 bị chấm `E_WRONG-REASON` dù bộ kiểm có răng.

    Cách làm **không nới cổng**: commit **đúng nội dung đã đột biến** vào một kho git mới trong
    bản sao. `build.rs` tính `# build: algo=… inputs=… dirty=0` **trong bản sao** (fingprint =
    `git ls-tree -r HEAD` của `v2/kernel`/`v2/lib`/`v2/user` + tệp ghim, tức **nội dung đã đột
    biến**), rồi `rev-gate` tính lại **cùng** công thức trên **cùng** cây ⇒ khớp thật. Không
    miễn trừ, không sửa `build-rev.sh`, không đụng cây thật (D2-113F).

    Trả `(ok, detail)`; lỗi git ⇒ `(False, lý-do)` và lượt chạy vẫn tiếp (verdict sẽ tự khai
    `E_WRONG-REASON` — không bịa KILLED).
    """
    git = shutil.which("git")
    if not git:
        return False, "no-git-binary"
    # Chốt an toàn: bản sao **không được** có sẵn `.git` (kể cả tệp `gitdir:` của worktree liên
    # kết). `_copy_tree` đã bỏ `.git`, nhưng nếu ai đó đổi `COPY_ENTRIES`/copy tay mà để lọt thì
    # `git init`+`commit` dưới đây sẽ ghi vào **kho của cây thật** — đúng tai nạn phải chặn bằng
    # cấu trúc, không bằng lời nhắc. Thà mất verdict (E_WRONG-REASON) còn hơn bẩn kho thật.
    if os.path.lexists(os.path.join(copy_root, ".git")):
        return False, "copy-has-git"
    # Copy `.gitignore` gốc rồi ghim thêm hai liên kết target của bản sao.
    ignore = os.path.join(copy_root, ".gitignore")
    try:
        src_ignore = os.path.join(root, ".gitignore")
        lines = []
        if os.path.isfile(src_ignore):
            with open(src_ignore, "r", encoding="utf-8", errors="replace") as fh:
                lines.append(fh.read().rstrip("\n"))
        lines.extend(GIT_IGNORE_EXTRA)
        with open(ignore, "w", encoding="utf-8") as fh:
            fh.write("\n".join(line for line in lines if line is not None) + "\n")
    except OSError as exc:  # noqa: BLE001
        return False, "gitignore: %s" % exc

    env = dict(os.environ)
    # Không bao giờ đóng giả lead (hook `vifast-guard`); cây tạm không có hook, nhưng vẫn chặn.
    env.pop("VIFAST_ROLE", None)
    env["GIT_AUTHOR_DATE"] = GIT_DATE
    env["GIT_COMMITTER_DATE"] = GIT_DATE
    env["GIT_AUTHOR_NAME"] = GIT_NAME
    env["GIT_AUTHOR_EMAIL"] = GIT_EMAIL
    env["GIT_COMMITTER_NAME"] = GIT_NAME
    env["GIT_COMMITTER_EMAIL"] = GIT_EMAIL
    env["GIT_CONFIG_NOSYSTEM"] = "1"
    env["GIT_OPTIONAL_LOCKS"] = "0"
    cfg = (
        "-c", "user.name=" + GIT_NAME,
        "-c", "user.email=" + GIT_EMAIL,
        "-c", "commit.gpgsign=false",
        "-c", "tag.gpgsign=false",
        "-c", "core.hooksPath=/dev/null",
        "-c", "init.defaultBranch=main",
    )
    steps = (
        ("init", (git,) + cfg + ("init", "-q")),
        ("add", (git,) + cfg + ("add", "-A",)),
        ("commit", (git,) + cfg + ("commit", "-q", "--no-gpg-sign",
                                   "-m", "mutant " + label)),
    )
    for name, cmd in steps:
        try:
            proc = subprocess.run(
                list(cmd), cwd=copy_root, env=env, capture_output=True, text=True, timeout=600
            )
        except (OSError, subprocess.TimeoutExpired) as exc:  # noqa: BLE001
            return False, "git-%s: %s" % (name, exc)
        if proc.returncode != 0:
            tail = ((proc.stdout or "") + (proc.stderr or "")).strip().splitlines()[-1:]
            return False, "git-%s rc=%d %s" % (name, proc.returncode, tail[0] if tail else "")
    return True, "HEAD"


def _run_command(cmd, cwd, timeout):
    env = dict(os.environ)
    env["GIT_OPTIONAL_LOCKS"] = "0"  # git chỉ đọc (build-rev.sh) — không ghi .git
    env.pop("VIFAST_ROLE", None)  # không bao giờ đóng giả lead
    # **KT-403 r2** — chạy trong `v2 nightly`: cha **đã giữ** khoá máy đo và uỷ quyền
    # (`V2_MEASURE_LOCK_DELEGATED_BY`). Runner chỉ-đọc (`product-riscv64.sh`) không thấy uỷ quyền
    # nên chỉ hiểu cờ `off`; nightly chạy tuần tự từng bước nên không có phép đo nào chồng nhau.
    if env.get("V2_MEASURE_LOCK_DELEGATED_BY"):
        env["V2_MEASURE_LOCK"] = "off"
    t0 = time.time()
    try:
        proc = subprocess.run(
            ["bash", "-c", cmd],
            cwd=cwd,
            env=env,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return proc.returncode, (proc.stdout or "") + (proc.stderr or ""), time.time() - t0
    except subprocess.TimeoutExpired as exc:
        out = (exc.stdout or "") + (exc.stderr or "")
        if isinstance(out, bytes):
            out = out.decode("utf-8", "replace")
        return 124, out, time.time() - t0


def run_mutant(root, mutant, suites, args):
    """Trả `(verdict, rc, marker_ok, detail, elapsed)`."""
    mid = mutant["id"]
    if not mutant.get("wired"):
        return "E_UNWIRED", None, False, mutant.get("why", ""), 0.0
    suite = suites[mutant["suite"]]
    cmd = suite["command"]
    # **KT-403 r2** — `marker` riêng của mục (nếu có) thay marker chung của suite: một runner
    # nhiều ca (vd `ktest-aarch64`) thì "ĐỎ đúng lý do" phải chỉ đúng dấu hiệu của đột biến, không
    # phải bất kỳ `not ok` nào (nếu không, đỏ do hạ tầng cũng bị tính là KILLED).
    marker = mutant.get("marker") or suite["marker"]
    kind = mutant["kind"]
    if kind in ("feature", "build"):
        copy_root = os.path.join(args.scratch, "mut-" + re.sub(r"[^A-Za-z0-9_-]", "_", mid))
        _copy_tree(root, copy_root)
        if kind == "feature":
            sites = apply_feature(copy_root, mutant["feature"])
            if sites == 0:
                return "E_STALE", None, False, "0 điểm cfg `%s`" % mutant["feature"], 0.0
            detail = "copy=%s sites=%d" % (copy_root, sites)
        else:
            applied, missing = apply_patches(copy_root, mutant.get("patches") or [])
            if missing or applied == 0:
                return (
                    "E_STALE",
                    None,
                    False,
                    "patch không khớp: %s" % ("; ".join(missing) or "rỗng"),
                    0.0,
                )
            detail = "copy=%s patches=%d" % (copy_root, applied)
        # **KT-408c** — commit **bản sao ĐÃ đột biến** vào git tree tạm (sau `apply_*`, trước khi
        # chạy suite) để `rev-gate` (KT-86/KT-89) thấy `rev=HEAD` + `dirty=0` **thật** — không
        # miễn trừ, không nới cổng. `script` không cần: nó tự bảo vệ cây thật.
        snap_ok, snap_detail = _git_snapshot(root, copy_root, mid)
        detail += " git=%s" % ("HEAD" if snap_ok else snap_detail)
        cwd = copy_root
    else:
        cwd = root
        detail = "cwd=repo (script tự bảo vệ cây thật)"
    rc, out, elapsed = _run_command(cmd, cwd, args.timeout)
    marker_ok = re.search(marker, out, re.MULTILINE) is not None
    if mutant["expect"] == "pass":
        verdict = "KILLED" if (rc == 0 and marker_ok) else ("E_WRONG-REASON" if rc == 0 else "SURVIVED")
    elif rc != 0 and marker_ok:
        verdict = "KILLED"
    elif rc == 0:
        verdict = "SURVIVED"
    else:
        verdict = "E_WRONG-REASON"
    log = os.path.join(args.scratch, mid + ".log")
    with open(log, "w", encoding="utf-8") as fh:
        fh.write(out)
    detail += " rc=%d marker=%s log=%s" % (rc, "ok" if marker_ok else "KHONG", log)
    return verdict, rc, marker_ok, detail, elapsed


# --------------------------------------------------------------------------- selftest

MINI_LIB = """\
// SPDX-License-Identifier: MIT OR Apache-2.0
#[cfg(not(feature = "mut-demo"))]
pub fn f() -> u32 {
    0
}
#[cfg(feature = "mut-demo")]
pub fn f() -> u32 {
    1
}
"""

ORACLE_RED = """\
#!/usr/bin/env bash
if grep -q 'cfg(all())' v2/kernel/vifast-plat-demo/src/lib.rs; then
    echo "not ok 1 - demo: đột biến bị bắt"
    exit 1
fi
echo "ok 1 - demo"
exit 0
"""

ORACLE_GREEN = "#!/usr/bin/env bash\necho 'ok 1 - demo'\nexit 0\n"

ORACLE_WRONG = "#!/usr/bin/env bash\necho 'lỗi hạ tầng khác'\nexit 1\n"

# `kind = "build"`: đột biến nằm sau `--cfg` do `build.rs` cấp ⇒ oracle đọc **bản sao** của
# `build.rs` và chỉ ĐỎ khi bản vá đã bật nhánh đột biến (`if true {`).
MINI_BUILD_RS = """\
// SPDX-License-Identifier: MIT OR Apache-2.0
fn feature(name: &str) -> bool {
    std::env::var_os(format!("CARGO_FEATURE_{name}")).is_some()
}
fn main() {
    if feature("MUT_DEMO_BUILD") {
        println!("cargo:rustc-cfg=mut_demo_build");
    }
}
"""

ORACLE_BUILD = """\
#!/usr/bin/env bash
if grep -q 'if true {' v2/kernel/vifast-plat-demo/build.rs; then
    echo "not ok 1 - build: đột biến `--cfg` bị bắt"
    exit 1
fi
echo "ok 1 - build"
exit 0
"""


def _mini_manifest():
    return {
        "version": 1,
        "suite": [
            {"id": "red", "command": "bash v2/test/oracle/oracle-red.sh", "marker": "^not ok "},
            {"id": "green", "command": "bash v2/test/oracle/oracle-green.sh", "marker": "^not ok "},
            {"id": "wrong", "command": "bash v2/test/oracle/oracle-wrong.sh", "marker": "^not ok "},
            {"id": "pass", "command": "bash v2/test/oracle/oracle-green.sh", "marker": "^ok 1"},
            {"id": "build-red", "command": "bash v2/test/oracle/oracle-build.sh", "marker": "^not ok "},
        ],
        "mutant": [
            {
                "id": "mut-demo",
                "kind": "feature",
                "feature": "mut-demo",
                "archs": ["host"],
                "crates": ["vifast-plat-demo"],
                "suite": "red",
                "expect": "red",
                "wired": True,
            },
            {
                "id": "mut-ghost",
                "kind": "feature",
                "feature": "mut-ghost",
                "archs": ["host"],
                "crates": ["vifast-plat-demo"],
                "suite": "red",
                "expect": "red",
                "wired": True,
            },
            {
                "id": "mut-survive",
                "kind": "feature",
                "feature": "mut-demo",
                "archs": ["host"],
                "crates": ["vifast-plat-demo"],
                "suite": "green",
                "expect": "red",
                "wired": True,
            },
            {
                "id": "mut-wrong",
                "kind": "feature",
                "feature": "mut-demo",
                "archs": ["host"],
                "crates": ["vifast-plat-demo"],
                "suite": "wrong",
                "expect": "red",
                "wired": True,
            },
            {
                "id": "green-pass",
                "kind": "script",
                "suite": "pass",
                "expect": "pass",
                "wired": True,
            },
            {
                "id": "mut-demo-build",
                "kind": "build",
                "feature": "mut-demo-build",
                "archs": ["host"],
                "crates": ["vifast-plat-demo"],
                "patches": [
                    {
                        "file": "v2/kernel/vifast-plat-demo/build.rs",
                        "find": 'if feature("MUT_DEMO_BUILD") {',
                        "replace": "if true {",
                    }
                ],
                "suite": "build-red",
                "expect": "red",
                "wired": True,
            },
            {
                "id": "mut-build-ghost",
                "kind": "build",
                "feature": "mut-build-ghost",
                "archs": ["host"],
                "crates": ["vifast-plat-demo"],
                "patches": [
                    {
                        "file": "v2/kernel/vifast-plat-demo/build.rs",
                        "find": "if KHONG_CO_DONG_NAY {",
                        "replace": "if true {",
                    }
                ],
                "suite": "build-red",
                "expect": "red",
                "wired": True,
            },
            {
                "id": "unwired",
                "kind": "script",
                "suite": "pass",
                "expect": "pass",
                "wired": False,
                "why": "ca minh hoạ `E_UNWIRED`",
            },
        ],
    }


def _mini_tree(base):
    lib = os.path.join(base, "v2", "kernel", "vifast-plat-demo", "src", "lib.rs")
    os.makedirs(os.path.dirname(lib), exist_ok=True)
    with open(lib, "w", encoding="utf-8") as fh:
        fh.write(MINI_LIB)
    with open(os.path.join(base, "v2", "kernel", "vifast-plat-demo", "build.rs"), "w") as fh:
        fh.write(MINI_BUILD_RS)
    with open(os.path.join(base, "v2", "kernel", "vifast-plat-demo", "Cargo.toml"), "w") as fh:
        fh.write('[package]\nname = "vifast-plat-demo"\nversion = "0.1.0"\n')
    for name, body in (("oracle-red.sh", ORACLE_RED), ("oracle-green.sh", ORACLE_GREEN),
                       ("oracle-wrong.sh", ORACLE_WRONG),
                       ("oracle-build.sh", ORACLE_BUILD)):
        path = os.path.join(base, "v2", "test", "oracle", name)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(body)
        os.chmod(path, 0o755)


def _hash_tree(root):
    import hashlib

    h = hashlib.sha256()
    for dirpath, _dirnames, filenames in os.walk(root):
        for name in sorted(filenames):
            path = os.path.join(dirpath, name)
            h.update(os.path.relpath(path, root).encode())
            with open(path, "rb") as fh:
                h.update(fh.read())
    return h.hexdigest()


class _Args:
    def __init__(self, scratch, timeout):
        self.scratch = scratch
        self.timeout = timeout


def run_selftest(root, args):
    base = tempfile.mkdtemp(prefix="kt403-mutants-", dir="/tmp")
    data = _mini_manifest()
    suites = suites_by_id(data)
    case_args = _Args(os.path.join(base, "scratch"), 120)
    os.makedirs(case_args.scratch)
    _mini_tree(os.path.join(base, "mini"))
    mini = os.path.join(base, "mini")
    before = _hash_tree(mini)
    cases = [
        ("mut-demo", "KILLED", "đột biến bị bắt (đỏ + marker)"),
        ("mut-ghost", "E_STALE", "0 điểm cfg ⇒ E_STALE"),
        ("mut-survive", "SURVIVED", "lệnh XANH ⇒ SURVIVED (lỗ hổng bộ kiểm)"),
        ("mut-wrong", "E_WRONG-REASON", "đỏ vì lý do khác ⇒ E_WRONG-REASON"),
        ("mut-demo-build", "KILLED", "kind=build: patch `--cfg` trên bản sao bị bắt"),
        ("mut-build-ghost", "E_STALE", "kind=build: `find` không khớp ⇒ E_STALE"),
        ("green-pass", "KILLED", "script expect=pass, marker khớp ⇒ KILLED"),
        ("unwired", "E_UNWIRED", "wired=false ⇒ E_UNWIRED"),
    ]
    by_id = {m["id"]: m for m in data["mutant"]}
    total = len(cases) + 1
    fail = 0
    print("== mutants — SELFTEST (cây mini %s) ==" % mini)
    for i, (mid, want, label) in enumerate(cases, 1):
        verdict, _rc, _mk, detail, _el = run_mutant(mini, by_id[mid], suites, case_args)
        ok = verdict == want
        print(
            "%s %d - %s: verdict=%s (muốn %s) => %s"
            % ("ok" if ok else "not ok", i, label, verdict, want, "ĐÚNG" if ok else "SAI")
        )
        if not ok:
            print("#    chi tiết: %s" % detail)
            fail += 1
    after = _hash_tree(mini)
    ok = before == after
    print(
        "%s %d - cây thật (mini) không bị sửa bởi phép chạy => %s"
        % ("ok" if ok else "not ok", total, "ĐÚNG" if ok else "SAI")
    )
    if not ok:
        fail += 1
    shutil.rmtree(base, ignore_errors=True)
    if fail:
        print("== SELFTEST mutants: SAI — %d/%d ca sai kỳ vọng ==" % (fail, total))
        return 1
    print("== SELFTEST mutants: OK — %d/%d ca (đỏ-trước cho chính công cụ) ==" % (total, total))
    return 0


# --------------------------------------------------------------------------- CLI


def main(argv):
    ap = argparse.ArgumentParser(description="v2 mutants — chạy đột biến theo v2/test/mutants.toml")
    ap.add_argument("--root", default=None, help="gốc repo (mặc định: suy từ vị trí script)")
    ap.add_argument("--list", action="store_true", help="liệt kê đột biến + trạng thái `wired`")
    ap.add_argument("--lint", action="store_true", help="kiểm manifest (không chạy đột biến)")
    ap.add_argument("--all", action="store_true", help="chạy mọi đột biến ĐÃ NỐI")
    ap.add_argument("--dry-run", action="store_true", help="in việc sẽ làm, không thực thi")
    ap.add_argument("--selftest", action="store_true", help="đỏ-trước cho chính công cụ")
    ap.add_argument("--timeout", type=float, default=1800.0, help="timeout mỗi đột biến (giây)")
    ap.add_argument("--keep", action="store_true", help="giữ bản sao/log sau khi chạy")
    ap.add_argument("--verbose", action="store_true", help="in cả log đường dẫn")
    ap.add_argument("name", nargs="?", default=None, help="id đột biến cần chạy")
    args = ap.parse_args(argv)

    if args.root is None:
        args.root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    args.root = os.path.abspath(args.root)

    if args.selftest:
        return run_selftest(args.root, args)

    try:
        data = load_manifest(args.root)
        suites = suites_by_id(data)
    except ManifestError as exc:
        print("mutants: %s" % exc, file=sys.stderr)
        return 2

    if args.list:
        print("# mutants: id\tkind\tarchs\tsuite\texpect\twired")
        for m in data["mutant"]:
            print(
                "%s\t%s\t%s\t%s\t%s\t%s"
                % (
                    m["id"],
                    m.get("kind"),
                    ",".join(m.get("archs", ["-"])) if m.get("kind") == "feature" else "-",
                    m.get("suite"),
                    m.get("expect"),
                    "yes" if m.get("wired") else "NO",
                )
            )
        wired = sum(1 for m in data["mutant"] if m.get("wired"))
        print("# wired=%d/%d declarations=%s" % (wired, len(data["mutant"]), data.get("declarations")))
        return 0

    if args.lint or not (args.all or args.name):
        problems = lint(args.root, data)
        if args.lint:
            return 1 if problems else 0
        print("mutants: cần `--list`, `--lint`, `--all`, `--selftest` hoặc <id>", file=sys.stderr)
        return 2

    selected = data["mutant"] if args.all else [m for m in data["mutant"] if m["id"] == args.name]
    if not selected:
        print("mutants: id lạ %r" % args.name, file=sys.stderr)
        return 2
    unwired = [m["id"] for m in selected if not m.get("wired")]
    if args.dry_run:
        for m in selected:
            print(
                "# dry-run %s: kind=%s suite=%s expect=%s wired=%s"
                % (m["id"], m.get("kind"), m.get("suite"), m.get("expect"), m.get("wired"))
            )
        print("# dry-run: %d đột biến, %d chưa nối" % (len(selected), len(unwired)))
        return 3 if unwired else 0

    scratch = tempfile.mkdtemp(prefix="kt403-mutants-", dir="/tmp")
    args.scratch = scratch
    rc_total = 0
    killed = survived = wrong = stale = 0
    try:
        for m in selected:
            verdict, _rc, _mk, detail, elapsed = run_mutant(args.root, m, suites, args)
            if verdict == "KILLED":
                killed += 1
            elif verdict == "SURVIVED":
                survived += 1
                rc_total = 1
            elif verdict == "E_WRONG-REASON":
                wrong += 1
                rc_total = 1
            elif verdict == "E_STALE":
                stale += 1
                rc_total = 2
            elif verdict == "E_UNWIRED":
                rc_total = 3
            print(
                "%s - %s: %s (%.1f s) %s"
                % ("ok" if verdict == "KILLED" else "not ok", m["id"], verdict, elapsed, detail)
            )
        print(
            "# mutants: chạy=%d killed=%d survived=%d wrong-reason=%d stale=%d chưa-nối=%d"
            % (len(selected), killed, survived, wrong, stale, len(unwired))
        )
    finally:
        if args.keep:
            print("# giữ bản sao: %s" % scratch)
        else:
            shutil.rmtree(scratch, ignore_errors=True)
    if unwired and not args.all:
        return 2
    return rc_total


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
