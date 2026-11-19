#!/usr/bin/env python3
"""L25 — selftest teeth: rebuild the 8 new KT-184 fixtures independently and check
that removing the mutation edge turns the checker green (case would fail), and
re-adding it turns red. Also run the old (7e7e1b53) checker on the same fixtures."""
import importlib.util
import os
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = Path(os.environ.get("REPO", str(HERE.parents[3])))
BASE = Path(os.environ.get("BASE", os.path.join(os.environ.get("TMPDIR", "/tmp"), "kt187-l25")))


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


cur = load("cur", str(REPO / "v2/tools/check-wake-paths.py"))
old = load("old", str(BASE / "check-kt174.py"))

W = """impl M {{
    fn duong_san_pham(&mut self, t: u32) -> u8 {{
        self.{call}(t);
        0
    }}
}}
"""


def fixture(base):
    cur._fixture_v2(base)


def run(mod, d, strict):
    rep = mod.scan_v2(d, strict=strict, allowlist={})
    return rep


def show(label, rep, want_red, want_code=None):
    codes = sorted({e.split()[0] for e in rep.errors})
    ok = (not rep.ok) == want_red
    if want_code:
        ok = ok and any(want_code in e for e in rep.errors)
    print(f"{'PASS' if ok else 'FAIL'}  {label}: errors={len(rep.errors)} codes={codes}")
    for e in rep.errors[:3]:
        print(f"        {e}")
    return ok


results = []
tmp = Path(tempfile.mkdtemp(prefix="l25-teeth-"))

# ---- 4at / 4au: H4 three layers ----
def h4_three(name, call):
    d = tmp / name
    fixture(d)
    cur._write(d / "v2/kernel/vifast-core/src/lib.rs",
               "#[cfg(test)]\nmod evil_a;\n#[cfg(test)]\nmod evil_b;\n"
               "#[cfg(test)]\nmod evil_c;\n")
    cur._write(d / "v2/kernel/vifast-core/src/evil_a.rs", 'include!("evil_b.rs");\n')
    cur._write(d / "v2/kernel/vifast-core/src/evil_b.rs", 'include!("evil_c.rs");\n')
    cur._write(d / "v2/kernel/vifast-core/src/evil_c.rs", W.format(call=call))
    cur._write(d / "v2/kernel/vifast-core/src/core_ipc.rs", 'include!("evil_a.rs");\n')
    return d

d = h4_three("h4-three-red", "wake")
results.append(show("4at H4 three-layer wake (current, strict)", run(cur, d, True), True, "E_STRICT_TEST_ONLY_WAKE"))
results.append(show("4at H4 three-layer wake (OLD 7e7e1b53, strict)", run(old, d, True), False))
d = h4_three("h4-three-green", "wake_from")
results.append(show("4au H4b three-layer wake_from (current, strict)", run(cur, d, True), False))
d = h4_three("h4-three-green2", "wake")
results.append(show("4au+ mutation re-added (current, strict)", run(cur, d, True), True, "E_STRICT_TEST_ONLY_WAKE"))

# ---- 4av / 4aw: #[path] + include! ----
def h4_path(name, call):
    d = tmp / name
    fixture(d)
    cur._write(d / "v2/kernel/vifast-core/src/lib.rs", "#[cfg(test)]\nmod evil_b;\n")
    cur._write(d / "v2/kernel/vifast-core/src/evil_a.rs", 'include!("evil_b.rs");\n')
    cur._write(d / "v2/kernel/vifast-core/src/evil_b.rs", W.format(call=call))
    cur._write(d / "v2/kernel/vifast-core/src/core_ipc.rs",
               '#[path = "evil_a.rs"]\nmod evil_a;\n')
    return d

d = h4_path("h4-path-red", "wake")
results.append(show("4av #[path]+include! wake (current, strict)", run(cur, d, True), True, "E_STRICT_TEST_ONLY_WAKE"))
results.append(show("4av #[path]+include! wake (OLD 7e7e1b53, strict)", run(old, d, True), False))
d = h4_path("h4-path-green", "wake_from")
results.append(show("4aw #[path]+include! wake_from (current, strict)", run(cur, d, True), False))

# ---- 4ax: include!(concat!) normal mode ----
def d2161_inc(name, body):
    d = tmp / name
    fixture(d)
    cur._write(d / "v2/kernel/vifast-core/src/evil_b.rs", W.format(call="wake"))
    cur._write(d / "v2/kernel/vifast-core/src/core_ipc.rs", body)
    return d

d = d2161_inc("d2161-inc-red", 'include!(concat!("evil", "_b.rs"));\n')
results.append(show("4ax include!(concat!) normal (current)", run(cur, d, False), True, "E_STRICT_WAKE_INCLUDE"))
results.append(show("4ax include!(concat!) normal (OLD 7e7e1b53)", run(old, d, False), False))
d = d2161_inc("d2161-inc-green", 'include!("evil_b.rs");\n')
results.append(show("4ax- mutation removed (literal include) (current, normal)", run(cur, d, False), False))

# ---- 4ay: #[path] outside tree normal mode ----
def d2161_out(name, body):
    d = tmp / name
    fixture(d)
    cur._write(d / "v2/lib/vifast-abi/src/evil_out.rs", W.format(call="wake"))
    cur._write(d / "v2/kernel/vifast-core/src/core_ipc.rs", body)
    return d

d = d2161_out("d2161-out-red", '#[path = r#"../../../lib/vifast-abi/src/evil_out.rs"#]\nmod evil_out;\n')
results.append(show("4ay #[path] outside normal (current)", run(cur, d, False), True, "E_STRICT_WAKE_INCLUDE"))
results.append(show("4ay #[path] outside normal (OLD 7e7e1b53)", run(old, d, False), False))
d = d2161_out("d2161-out-green", '#[path = "evil_ok.rs"]\nmod evil_ok;\n')
cur._write(d / "v2/kernel/vifast-core/src/evil_ok.rs", W.format(call="wake_from"))
results.append(show("4ay- mutation removed (in-tree literal) (current)", run(cur, d, False), False))

# ---- 4az: #[path = concat!] normal mode ----
def d2161_nonlit(name, body):
    d = tmp / name
    fixture(d)
    cur._write(d / "v2/kernel/vifast-core/src/evil_x.rs", W.format(call="wake"))
    cur._write(d / "v2/kernel/vifast-core/src/core_ipc.rs", body)
    return d

d = d2161_nonlit("d2161-nonlit-red", '#[path = concat!("evil", "_x.rs")]\nmod evil_x;\n')
results.append(show("4az #[path=concat!] normal (current)", run(cur, d, False), True, "E_STRICT_WAKE_INCLUDE"))
results.append(show("4az #[path=concat!] normal (OLD 7e7e1b53)", run(old, d, False), False))
d = d2161_nonlit("d2161-nonlit-green", '#[path = "evil_x.rs"]\nmod evil_x;\n')
results.append(show("4az- mutation removed (literal path) (current, normal)", run(cur, d, False), False))

# ---- 4ba: positive control ----
d = tmp / "d2161-ok"
fixture(d)
cur._write(d / "v2/kernel/vifast-core/src/evil_ok.rs", W.format(call="wake_from"))
cur._write(d / "v2/kernel/vifast-core/src/core_ipc.rs",
           '#[path = "evil_ok.rs"]\nmod evil_ok_product;\n')
results.append(show("4ba #[path] literal in-tree wake_from normal (current)", run(cur, d, False), False))
cur._write(d / "v2/kernel/vifast-core/src/core_ipc.rs",
           '#[path = "../../../lib/vifast-abi/src/evil_out.rs"]\nmod evil_ok_product;\n')
cur._write(d / "v2/lib/vifast-abi/src/evil_out.rs", W.format(call="wake"))
results.append(show("4ba+ mutation added (path outside) (current, normal)", run(cur, d, False), True, "E_STRICT_WAKE_INCLUDE"))

print()
print(f"teeth: {sum(results)}/{len(results)} pass")
