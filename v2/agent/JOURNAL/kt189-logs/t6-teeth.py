#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-189 / L28 — "răng" của ba ca selftest mới 4ba/4bb/4bc, dựng lại ĐỘC LẬP.
# Không chạy --selftest của tác giả; dùng chính _fixture_v2 của checker để dựng cây,
# rồi tự dựng lại đột biến và bỏ đúng cạnh đột biến.
import importlib.util, sys, tempfile, shutil
from pathlib import Path

spec = importlib.util.spec_from_file_location("cwp", "/tmp/night2-L28-review187/v2/tools/check-wake-paths.py")
cwp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cwp)

W = cwp._write
F = cwp._fixture_v2
EVIL = "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake(t);\n        0\n    }\n}\n"

def run_case(name, build, expect_err):
    d = Path(tempfile.mkdtemp(prefix="kt189-teeth-"))
    try:
        F(d)
        build(d)
        rep = cwp.scan_v2(d, strict=True, allowlist={})
        hit = any(expect_err in e for e in rep.errors)
        print(f"{'ok' if hit else 'NOT-OK'} - {name}: errors={len(rep.errors)} hit={hit}")
        for e in rep.errors[:3]:
            print(f"#    {e.strip()}")
        return hit
    finally:
        shutil.rmtree(d, ignore_errors=True)

def b4ba(d, with_mut=True):
    W(d / "v2/kernel/vifast-core/src/lib.rs", "#[cfg(test)]\nmod evil_b;\n")
    W(d / "v2/kernel/vifast-core/src/evil_b.rs", EVIL)
    line = '#[cfg_attr(not(test), path = "evil_b.rs")]\n' if with_mut else ''
    W(d / "v2/kernel/vifast-core/src/core_ipc.rs", line + "mod evil_b_product;\n")

def b4bb(d, with_mut=True):
    body = '#[path = "evil_b.rs"] mod evil;' if with_mut else 'mod evil;'
    W(d / "v2/kernel/vifast-core/src/core_ipc.rs",
      'macro_rules! evil_mod { () => { %s } }\n' % body)

def b4bc(d, with_mut=True):
    body = 'include!("evil_b.rs");' if with_mut else 'let _ = 1;'
    W(d / "v2/kernel/vifast-core/src/core_ipc.rs",
      'macro_rules! evil_inc { () => { %s } }\n' % body)

print("== 4ba: cfg_attr(not(test), path) tới tệp test-only ==")
a1 = run_case("4ba có đột biến", lambda d: b4ba(d, True), "E_STRICT_TEST_ONLY_WAKE")
a0 = run_case("4ba BỎ cạnh đột biến (phải XANH)", lambda d: b4ba(d, False), "E_STRICT_TEST_ONLY_WAKE")
print("== 4bb: #[path] trong thân macro ==")
b1 = run_case("4bb có đột biến", lambda d: b4bb(d, True), "E_STRICT_MACRO_INCLUDE")
b0 = run_case("4bb BỎ cạnh đột biến (phải XANH)", lambda d: b4bb(d, False), "E_STRICT_MACRO_INCLUDE")
print("== 4bc: include! trong thân macro ==")
c1 = run_case("4bc có đột biến", lambda d: b4bc(d, True), "E_STRICT_MACRO_INCLUDE")
c0 = run_case("4bc BỎ cạnh đột biến (phải XANH)", lambda d: b4bc(d, False), "E_STRICT_MACRO_INCLUDE")
print()
print("TEETH:", "PASS" if (a1 and not a0 and b1 and not b0 and c1 and not c0) else "FAIL")
