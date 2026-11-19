#!/usr/bin/env python3
"""Fixture tối thiểu TỰ DỰNG (L30) — không dùng `_fixture_v2` của tác giả.

Đủ mỏ neo cho `scan_v2`: 3 hộp thư, `core_sched.rs`/`kobj.rs` có `fn wake(`/`fn wake_step(`,
`apply_mail`, `push_step`; crate root `src/lib.rs` cho kernel/sim; `v2/lib/vifast-sync`.
"""
import os
import shutil
import subprocess
import sys

BASE = "/tmp/kt192-l30/fx"
CHECKER = "/private/tmp/night2-L30-review184v4/v2/tools/check-wake-paths.py"

MAILBOX = """pub enum MailboxMsg {
    Wake(u32),
    Reschedule,
    Migrate { tcb: u32, to_hart: u16 },
    Stop,
    TlbFlush(u32),
}
"""

SYNC_MAILBOX = """pub enum Msg {
    Wake { hart: u16, tcb: u32 },
    Reschedule { hart: u16, tcb: u32 },
    Migrate { hart: u16, tcb: u32 },
    Stop { hart: u16 },
    VmKick { hart: u16 },
    PowerReq { hart: u16, state: u8 },
}
"""

SCHED = """impl M {
    fn wake(&mut self, tcb: u32) -> u8 {
        if self.home(tcb) != self.current_hart {
            return self.post_mail(self.home(tcb), MailboxMsg::Wake(tcb));
        }
        self.wake_step(tcb)
    }
    fn wake_step(&mut self, tcb: u32) -> u8 {
        self.transition_tcb(tcb, TcbState::Ready)
    }
    fn apply_mail(&mut self, hart: u16, msg: MailboxMsg) {
        match msg {
            MailboxMsg::Wake(tcb) => { let _ = self.wake_step(tcb); }
            MailboxMsg::Reschedule => {}
            MailboxMsg::Migrate { tcb, to_hart } => {}
            MailboxMsg::Stop => {}
            MailboxMsg::TlbFlush(_) => {}
        }
    }
    fn push_step(&mut self) -> usize {
        let msg = MailboxMsg::Migrate { tcb: 1, to_hart: 1 };
        self.mail[1].try_push(msg);
        1
    }
}
"""


def w(base, relp, text):
    p = os.path.join(base, relp)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w", encoding="utf-8") as f:
        f.write(text)


def build(base):
    if os.path.isdir(base):
        shutil.rmtree(base)
    w(base, "v2/kernel/vifast-core/src/mailbox.rs", MAILBOX)
    w(base, "v2/sim/vifast-sim/src/mailbox.rs", MAILBOX)
    w(base, "v2/lib/vifast-sync/src/mailbox.rs", SYNC_MAILBOX)
    w(
        base,
        "v2/lib/vifast-sync/tests/kt30_sync.rs",
        "fn t() { let _ = Msg::Wake { hart: 0, tcb: 1 }; let _ = Msg::Reschedule { hart: 0, tcb: 1 }; let _ = Msg::Migrate { hart: 0, tcb: 1 }; }\n",
    )
    w(base, "v2/kernel/vifast-core/src/core_sched.rs", SCHED)
    w(base, "v2/sim/vifast-sim/src/kobj.rs", SCHED)
    w(base, "v2/kernel/vifast-core/src/lib.rs", "pub mod mailbox;\nmod core_sched;\n")
    w(base, "v2/sim/vifast-sim/src/lib.rs", "pub mod mailbox;\nmod kobj;\n")
    w(base, "v2/kernel/vifast-core/Cargo.toml", '[package]\nname = "vifast-core"\nversion = "0.1.0"\n')
    w(base, "v2/sim/vifast-sim/Cargo.toml", '[package]\nname = "vifast-sim"\nversion = "0.1.0"\n')
    w(base, "v2/lib/vifast-sync/Cargo.toml", '[package]\nname = "vifast-sync"\nversion = "0.1.0"\n')
    w(base, "v2/Cargo.toml", "[workspace]\nmembers = []\n")


def run(base, strict):
    cmd = [sys.executable, CHECKER, "--root", base, "--json"]
    if strict:
        cmd.append("--strict")
    p = subprocess.run(cmd, capture_output=True, text=True)
    import json

    try:
        d = json.loads(p.stdout)
    except Exception:
        return p.returncode, ["<json fail %r>" % p.stdout[:200]]
    errs = [e for e in d["errors"] if "E_STRICT_STALE" not in e and "E_ANCHOR_MISSING" not in e]
    return (0 if not errs else 1), errs


def case(name, files, expect, note=""):
    build(BASE)
    for relp, content in files.items():
        w(BASE, relp, content)
    rc0, e0 = run(BASE, False)
    rc1, e1 = run(BASE, True)
    got = "RED" if rc0 else "GREEN"
    got_s = "RED" if rc1 else "GREEN"
    verdict = "OK" if (got == expect[0] and got_s == expect[1]) else "**MISMATCH**"
    print(f"### {name}  [{note}]")
    print(
        f"  normal={got} rc={rc0} n={len(e0)} | strict={got_s} rc={rc1} n={len(e1)}"
        f"  expect={expect} {verdict}"
    )
    for e in e0[:3]:
        print(f"    N {e.strip()[:170]}")
    for e in e1[:3]:
        print(f"    S {e.strip()[:170]}")
    print()
    return got, got_s, e0, e1
