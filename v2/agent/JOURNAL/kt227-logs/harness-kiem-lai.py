#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-224 — harness tấn công lớp 2 (`check-wake-depinfo.py`) + đối chứng lớp 1.

Mỗi ca: dựng mã **biên dịch được**, chạy
  (1) lớp 1 `check-wake-paths.py` (thường, và `--strict` khi cần),
  (2) lớp 2 `check-wake-depinfo.py --unit <host|kernel>`,
ghi RC + lỗi, rồi **hoàn nguyên**.

Không sửa `check-wake-depinfo.py` / `check-wake-paths.py`. Chỉ tạo/xoá tệp tạm.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve()
ROOT = HERE.parents[4]  # v2/agent/JOURNAL/kt227-logs → gốc repo
LOGS = HERE.parent
PY = sys.executable

KERNEL_LIB = "v2/kernel/vifast-core/src/lib.rs"
MODEL_LIB = "v2/lib/vifast-model/src/lib.rs"

EVIL_KERNEL = """// SPDX-License-Identifier: MIT OR Apache-2.0
// KT-224 — ca tấn công: gọi `wake` (API chỉ-dùng-test) từ mã sản phẩm.
pub fn kt224_evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
"""

EVIL_LIB = """// SPDX-License-Identifier: MIT OR Apache-2.0
// KT-224 — ca tấn công: gọi `wake` trên holder cục bộ (không định nghĩa `fn wake` ở đây).
pub fn kt224_evil(h: &mut crate::kt224_holder::Holder, t: u32) {
    h.wake(t);
}
"""

HOLDER = """// SPDX-License-Identifier: MIT OR Apache-2.0
// KT-224 — holder có `fn wake` (tệp này được lớp 2 miễn vì định nghĩa cửa wake).
pub struct Holder;
impl Holder {
    pub fn wake(&mut self, _t: u32) {}
}
"""


class Patch:
    """Sửa cây tạm thời + hoàn nguyên chính xác."""

    def __init__(self) -> None:
        self.backups: dict[str, str] = {}
        self.created: list[Path] = []
        self.dirs: list[Path] = []

    def append(self, rel: str, text: str) -> None:
        p = ROOT / rel
        if rel not in self.backups:
            self.backups[rel] = p.read_text(encoding="utf-8")
        p.write_text(p.read_text(encoding="utf-8") + text, encoding="utf-8")

    def create(self, rel: str, text: str) -> Path:
        p = ROOT / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8")
        self.created.append(p)
        return p

    def symlink(self, rel: str, target: str) -> Path:
        p = ROOT / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        if p.is_symlink() or p.exists():
            p.unlink()
        p.symlink_to(target)
        self.created.append(p)
        return p

    def mkdir(self, rel: str) -> Path:
        p = ROOT / rel
        p.mkdir(parents=True, exist_ok=True)
        self.dirs.append(p)
        return p

    def cleanup(self) -> None:
        for rel, txt in self.backups.items():
            (ROOT / rel).write_text(txt, encoding="utf-8")
        for p in self.created:
            try:
                if p.is_symlink() or p.exists():
                    p.unlink()
            except OSError:
                pass
        for d in sorted(self.dirs, key=lambda x: len(x.parts), reverse=True):
            try:
                d.rmdir()
            except OSError:
                pass
        self.backups.clear()
        self.created.clear()
        self.dirs.clear()


def run(cmd: list[str], timeout: float = 900.0) -> tuple[int, str, float]:
    t0 = time.monotonic()
    try:
        p = subprocess.run(cmd, cwd=str(ROOT), capture_output=True, text=True, timeout=timeout)
        out = p.stdout + p.stderr
        rc = p.returncode
    except subprocess.TimeoutExpired:
        out = f"TIMEOUT {timeout}s"
        rc = 124
    return rc, out, time.monotonic() - t0


def layer1(strict: bool = False) -> tuple[int, str, float]:
    cmd = [PY, "v2/tools/check-wake-paths.py", "--root", str(ROOT)]
    if strict:
        cmd.append("--strict")
    return run(cmd)


def layer2(unit: str) -> tuple[int, str, float]:
    return run(
        [PY, "v2/tools/check-wake-depinfo.py", "--root", str(ROOT), "--unit", unit]
    )


def summarize(out: str) -> dict:
    """Rút gọn đầu ra: dòng tổng kết + tối đa 6 dòng E/W."""
    lines = out.splitlines()
    tail = [ln for ln in lines if ln.startswith("# check-wake") or ln.startswith("# đơn vị")]
    errs = [ln for ln in lines if ln.startswith("E ")]
    warns = [ln for ln in lines if ln.startswith("W ")]
    return {
        "tail": tail[-4:],
        "errors": errs[:8],
        "n_errors": len(errs),
        "warnings": warns[:4],
        "n_warnings": len(warns),
    }


ATTACKS: list[dict] = []


def attack(name: str, unit: str, strict: bool = False):
    def deco(fn):
        ATTACKS.append({"name": name, "unit": unit, "strict": strict, "fn": fn})
        return fn

    return deco


# ---------------------------------------------------------------- B. tấn công lớp 2

@attack("B1 macro_rules! thân chứa m.wake(t) (kernel)", "kernel")
def b1(p: Patch) -> None:
    p.append(
        KERNEL_LIB,
        """
macro_rules! kt224_macro_wake {
    ($m:expr, $t:expr) => {
        $m.wake($t)
    };
}
pub fn kt224_macro_call(m: &mut Machine, t: ObjId) {
    let _ = kt224_macro_wake!(m, t);
}
""",
    )


@attack("B2 macro_rules! nhận tên hàm `wake` làm đối số (kernel)", "kernel")
def b2(p: Patch) -> None:
    p.append(
        KERNEL_LIB,
        """
macro_rules! kt224_call {
    ($m:expr, $f:ident, $t:expr) => {
        $m.$f($t)
    };
}
pub fn kt224_macro_call2(m: &mut Machine, t: ObjId) {
    let _ = kt224_call!(m, wake, t);
}
""",
    )


@attack("B3a #[cfg(feature = \"ktest\")] khối trong lib.rs (kernel)", "kernel")
def b3a(p: Patch) -> None:
    p.append(
        KERNEL_LIB,
        """
#[cfg(feature = "ktest")]
pub fn kt224_cfg_feature(m: &mut Machine, t: ObjId) {
    let _ = m.wake(t);
}
""",
    )


@attack("B3b #[cfg(feature = \"ktest\")] mod tệp RIÊNG (kernel)", "kernel")
def b3b(p: Patch) -> None:
    p.create("v2/kernel/vifast-core/src/kt224_cfg_feature.rs", EVIL_KERNEL)
    p.append(KERNEL_LIB, '\n#[cfg(feature = "ktest")]\nmod kt224_cfg_feature;\n')


@attack("B4a #[cfg(any())] khối trong lib.rs (kernel)", "kernel")
def b4a(p: Patch) -> None:
    p.append(
        KERNEL_LIB,
        """
#[cfg(any())]
pub fn kt224_cfg_any(m: &mut Machine, t: ObjId) {
    let _ = m.wake(t);
}
""",
    )


@attack("B4b #[cfg(any())] mod tệp RIÊNG (kernel)", "kernel")
def b4b(p: Patch) -> None:
    p.create("v2/kernel/vifast-core/src/kt224_cfg_any.rs", EVIL_KERNEL)
    p.append(KERNEL_LIB, "\n#[cfg(any())]\nmod kt224_cfg_any;\n")


@attack("B5 #[cfg(target_os = \"none\")] mod tệp RIÊNG (kernel)", "kernel")
def b5(p: Patch) -> None:
    p.create("v2/kernel/vifast-core/src/kt224_cfg_none.rs", EVIL_KERNEL)
    p.append(KERNEL_LIB, '\n#[cfg(target_os = "none")]\nmod kt224_cfg_none;\n')


@attack("B6 include! trỏ tệp KHÔNG tồn tại (kernel)", "kernel")
def b6(p: Patch) -> None:
    p.append(KERNEL_LIB, '\ninclude!("kt224_khong_ton_tai.rs");\n')


@attack("B7 #[path] trỏ /tmp/kt224_evil.rs ngoài workspace (kernel)", "kernel")
def b7(p: Patch) -> None:
    Path("/tmp/kt224_evil.rs").write_text(EVIL_KERNEL, encoding="utf-8")
    p.append(KERNEL_LIB, '\n#[path = "/tmp/kt224_evil.rs"]\nmod kt224_evil;\n')


@attack("B8 symlink src/kt224_link.rs → /tmp/kt224_evil_link.rs (kernel)", "kernel")
def b8(p: Patch) -> None:
    Path("/tmp/kt224_evil_link.rs").write_text(EVIL_KERNEL, encoding="utf-8")
    p.symlink("v2/kernel/vifast-core/src/kt224_link.rs", "/tmp/kt224_evil_link.rs")
    p.append(KERNEL_LIB, "\nmod kt224_link;\n")


@attack("B9 #[path = \"kt224_evil.txt\"] tệp không đuôi .rs (kernel)", "kernel")
def b9(p: Patch) -> None:
    p.create("v2/kernel/vifast-core/src/kt224_evil.txt", EVIL_KERNEL)
    p.append(KERNEL_LIB, '\n#[path = "kt224_evil.txt"]\nmod kt224_evil_txt;\n')


@attack("B10 #[path] tên tệp có DẤU CÁCH (kernel)", "kernel")
def b10(p: Patch) -> None:
    p.create("v2/kernel/vifast-core/src/kt224 evil.rs", EVIL_KERNEL)
    p.append(KERNEL_LIB, '\n#[path = "kt224 evil.rs"]\nmod kt224_evil_space;\n')


@attack("B11 #[path] vào thư mục tên `target` (kernel)", "kernel")
def b11(p: Patch) -> None:
    p.mkdir("v2/kernel/vifast-core/src/target")
    p.create("v2/kernel/vifast-core/src/target/kt224_evil.rs", EVIL_KERNEL)
    p.append(KERNEL_LIB, '\n#[path = "target/kt224_evil.rs"]\nmod kt224_evil_target;\n')


@attack("B12 #[path = \"kt224_evil.txt\"] trong v2/lib (vifast-model)", "host")
def b12(p: Patch) -> None:
    p.create("v2/lib/vifast-model/src/kt224_holder.rs", HOLDER)
    p.create("v2/lib/vifast-model/src/kt224_evil.txt", EVIL_LIB)
    p.append(
        MODEL_LIB,
        '\nmod kt224_holder;\n#[path = "kt224_evil.txt"]\nmod kt224_evil;\n',
    )


@attack("B13 symlink trong v2/lib → /tmp/kt224_evil_lib.rs", "host")
def b13(p: Patch) -> None:
    Path("/tmp/kt224_evil_lib.rs").write_text(EVIL_LIB, encoding="utf-8")
    p.create("v2/lib/vifast-model/src/kt224_holder.rs", HOLDER)
    p.symlink("v2/lib/vifast-model/src/kt224_link.rs", "/tmp/kt224_evil_lib.rs")
    p.append(MODEL_LIB, "\nmod kt224_holder;\nmod kt224_link;\n")


@attack("B14 #[path] vào src/target/ trong v2/lib", "host")
def b14(p: Patch) -> None:
    p.create("v2/lib/vifast-model/src/kt224_holder.rs", HOLDER)
    p.mkdir("v2/lib/vifast-model/src/target")
    p.create("v2/lib/vifast-model/src/target/kt224_evil.rs", EVIL_LIB)
    p.append(
        MODEL_LIB,
        '\nmod kt224_holder;\n#[path = "target/kt224_evil.rs"]\nmod kt224_evil_target;\n',
    )


@attack("B15 #[path] tên tệp có DẤU CÁCH trong v2/lib", "host")
def b15(p: Patch) -> None:
    p.create("v2/lib/vifast-model/src/kt224_holder.rs", HOLDER)
    p.create("v2/lib/vifast-model/src/kt224 evil.rs", EVIL_LIB)
    p.append(
        MODEL_LIB,
        '\nmod kt224_holder;\n#[path = "kt224 evil.rs"]\nmod kt224_evil_space;\n',
    )


@attack("B16 #[path] tên tệp UNICODE trong v2/lib (đối chứng parse UTF-8)", "host")
def b16(p: Patch) -> None:
    p.create("v2/lib/vifast-model/src/kt224_holder.rs", HOLDER)
    p.create("v2/lib/vifast-model/src/kt224_évil.rs", EVIL_LIB)
    p.append(
        MODEL_LIB,
        '\nmod kt224_holder;\n#[path = "kt224_évil.rs"]\nmod kt224_evil_uni;\n',
    )


@attack("B17 tệp sinh bởi build.rs vào OUT_DIR (v2/lib)", "host")
def b17(p: Patch) -> None:
    p.create("v2/lib/vifast-model/src/kt224_holder.rs", HOLDER)
    p.create(
        "v2/lib/vifast-model/build.rs",
        """// SPDX-License-Identifier: MIT OR Apache-2.0
fn main() {
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    let p = std::path::Path::new(&out).join("kt224_gen.rs");
    std::fs::write(
        &p,
        "pub fn kt224_evil(h: &mut crate::kt224_holder::Holder, t: u32) { h.wake(t); }\\n",
    )
    .expect("ghi gen");
    println!("cargo:rerun-if-changed=build.rs");
}
""",
    )
    p.append(
        MODEL_LIB,
        '\nmod kt224_holder;\ninclude!(concat!(env!("OUT_DIR"), "/kt224_gen.rs"));\n',
    )


# ---------------------------------------------------------------- C. đối chứng dương

@attack("B18 m.wake(t) trong tệp CỬA WAKE core_sched.rs (miễn cả tệp ở lớp 2)", "kernel")
def b18(p: Patch) -> None:
    p.append(
        "v2/kernel/vifast-core/src/core_sched.rs",
        """
pub fn kt224_producer_evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
""",
    )


@attack("B19 #[cfg(test)] trên nhánh match + wake ở nhánh sản phẩm (KT-219 #13)", "kernel")
def b19(p: Patch) -> None:
    p.append(
        KERNEL_LIB,
        """
pub fn kt224_match(x: u32, m: &mut Machine, t: ObjId) {
    match x {
        #[cfg(test)]
        0 => (),
        1 => {
            let _ = m.wake(t);
        }
        _ => (),
    }
}
""",
    )


@attack("C1a đối chứng dương: wake_from/wake_step/biến `wake` (kernel)", "kernel")
def c1a(p: Patch) -> None:
    p.append(
        KERNEL_LIB,
        """
pub fn kt224_positive(m: &mut Machine, t: ObjId) {
    let mut wake = [0u32; 4];
    wake[0] = 1;
    let _ = wake;
    let _ = m.wake_from(t);
    let _ = m.wake_step(t, false);
}
""",
    )


@attack("C1b đối chứng dương: MailboxMsg::Wake trong PATTERN (kernel)", "kernel")
def c1b(p: Patch) -> None:
    p.append(
        KERNEL_LIB,
        """
pub fn kt224_positive_pattern(msg: &MailboxMsg) -> bool {
    matches!(msg, MailboxMsg::Wake(_))
}
""",
    )


@attack("C1c đối chứng dương: định nghĩa `fn wake` trong tệp mới (kernel)", "kernel")
def c1c(p: Patch) -> None:
    p.create(
        "v2/kernel/vifast-core/src/kt224_positive_def.rs",
        """// SPDX-License-Identifier: MIT OR Apache-2.0
// KT-224 — đối chứng dương: định nghĩa `fn wake` (không phải tham chiếu).
pub fn wake(_t: u32) {}
""",
    )
    p.append(KERNEL_LIB, "\nmod kt224_positive_def;\n")


@attack("C1d đối chứng dương: `wake` trong chú thích + chuỗi (kernel)", "kernel")
def c1d(p: Patch) -> None:
    p.create(
        "v2/kernel/vifast-core/src/kt224_positive_cmt.rs",
        """// SPDX-License-Identifier: MIT OR Apache-2.0
// KT-224 — đối chứng dương: `m.wake(t)` trong chú thích và literal chuỗi.
pub const KT224_S: &str = "m.wake(t)";
""",
    )
    p.append(KERNEL_LIB, "\nmod kt224_positive_cmt;\n")


def main(argv: list[str]) -> int:
    only = argv[0] if argv else None
    out_json = LOGS / "bang-tan-cong.json"
    results = []
    if out_json.is_file():
        try:
            results = json.loads(out_json.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            results = []
    done = {r["name"] for r in results}
    for a in ATTACKS:
        if only and only not in a["name"]:
            continue
        if a["name"] in done:
            print(f"\n===== {a['name']} (đã có, bỏ qua) =====", flush=True)
            continue
        print(f"\n===== {a['name']} =====", flush=True)
        p = Patch()
        rec = {"name": a["name"], "unit": a["unit"], "strict": a["strict"]}
        try:
            a["fn"](p)
            rc2, out2, t2 = layer2(a["unit"])
            rc1, out1, t1 = layer1(strict=False)
            rec["layer2"] = {"rc": rc2, "seconds": round(t2, 2), **summarize(out2)}
            rec["layer1"] = {"rc": rc1, "seconds": round(t1, 2), **summarize(out1)}
            if a["strict"] or rc1 == 0:
                rc1s, out1s, t1s = layer1(strict=True)
                rec["layer1_strict"] = {
                    "rc": rc1s,
                    "seconds": round(t1s, 2),
                    **summarize(out1s),
                }
            slug = a["name"].split()[0].lower()
            (LOGS / f"b-{slug}-layer2.log").write_text(out2, encoding="utf-8")
            (LOGS / f"b-{slug}-layer1.log").write_text(out1, encoding="utf-8")
            if "layer1_strict" in rec:
                (LOGS / f"b-{slug}-layer1-strict.log").write_text(out1s, encoding="utf-8")
            print(
                f"  lớp 2: RC={rc2} ({t2:.1f}s) errors={rec['layer2']['n_errors']} | "
                f"lớp 1: RC={rc1} ({t1:.1f}s) errors={rec['layer1']['n_errors']}"
                + (
                    f" | lớp 1 --strict: RC={rec['layer1_strict']['rc']}"
                    if "layer1_strict" in rec
                    else ""
                ),
                flush=True,
            )
        finally:
            p.cleanup()
        results.append(rec)
        (LOGS / "bang-tan-cong.json").write_text(
            json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8"
        )
    print("\n# xong", len(results), "ca", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
