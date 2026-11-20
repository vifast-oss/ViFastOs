#!/usr/bin/env python3
"""KT-242 — bộ repro trên **cây fixture tối thiểu** (nhanh, không cần cargo).

Mỗi ca: dựng cây fixture của chính checker (`_fixture_v2`) rồi chèn repro, chạy
`scan_v2(root, strict=False/True)`. In `errors` + mã lỗi.

KHÔNG sửa checker, KHÔNG nới ngưỡng. Dùng để tái lập nhanh các phát hiện trong
`../kt242-wake-review.md`; ca cần `cargo check` nằm ở các `*.sh` cạnh tệp này.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from probe import run, w, CWP  # noqa: E402

EVIL = "pub fn evil(m: &mut M, t: u32) { let _ = m.wake(t); }\n"
SW = CWP._state_write_rs()


def mk(relp, text, extra_files=None):
    def extra(base):
        for r, t in (extra_files or {}).items():
            w(base, r, t)
        w(base, relp, text)
    return extra


def sw(text):
    def extra(base):
        w(base, CWP.KERNEL_STATE_WRITE, text)
    return extra


def main():
    # ---- N1/N2: cửa thứ bảy lồng trong thân một cửa (R3) -------------------
    sw_a = SW.replace(
        """    pub(crate) fn wake_step(&mut self, tcb: u32) -> u8 {
        self.write_state(tcb, TcbState::Ready)
    }""",
        """    pub(crate) fn wake_step(&mut self, tcb: u32) -> u8 {
        impl M {
            pub fn kt242_door(&mut self, tcb: u32) -> u8 {
                self.write_state(tcb, TcbState::Ready)
            }
        }
        self.write_state(tcb, TcbState::Ready)
    }""")
    assert sw_a != SW
    run("N1 impl long trong wake_step -> cua thu bay (R3)", sw(sw_a), show_ok=True)

    sw_b = SW.replace(
        """    pub(crate) fn wake_step(&mut self, tcb: u32) -> u8 {
        self.write_state(tcb, TcbState::Ready)
    }""",
        """    pub(crate) fn wake_step(&mut self, tcb: u32) -> u8 {
        fn kt242_door(m: &mut M, tcb: u32) -> u8 {
            m.write_state(tcb, TcbState::Ready)
        }
        let _ = kt242_door;
        self.write_state(tcb, TcbState::Ready)
    }""")
    assert sw_b != SW
    run("N2 fn long trong wake_step (R3)", sw(sw_b), show_ok=True)

    # ---- N5: luật 1–3 với mã sản phẩm CÙNG DÒNG một mục #[cfg(test)] -------
    run("N5a luật 3 (arch doorbell) cùng dòng #[cfg(test)]",
        mk("v2/kernel/vifast-plat-riscv64/src/ipi.rs",
           '#[cfg(test)]\nfn helper() {} fn post_wake(hart: usize) '
           '{ IPI_MAILBOXES.get(hart).post(IpiKind::Wake); }\n'), show_ok=True)
    run("N5a' ĐỐI CHỨNG ÂM (bỏ #[cfg(test)])",
        mk("v2/kernel/vifast-plat-riscv64/src/ipi.rs",
           'fn post_wake(hart: usize) { IPI_MAILBOXES.get(hart).post(IpiKind::Wake); }\n'),
        show_ok=True)
    run("N5b luật 1 (post_mail MailboxMsg::Wake) cùng dòng #[cfg(test)]",
        mk("v2/kernel/vifast-core/src/sub/mailbox.rs",
           '#[cfg(test)]\nfn helper() {} fn evil(&mut self) '
           '{ self.post_mail(0, MailboxMsg::Wake(1)); }\n'), show_ok=True)
    run("N5b' ĐỐI CHỨNG ÂM (bỏ #[cfg(test)])",
        mk("v2/kernel/vifast-core/src/sub/mailbox.rs",
           'fn evil(&mut self) { self.post_mail(0, MailboxMsg::Wake(1)); }\n'), show_ok=True)
    run("N5c luật 2 (consumer MailboxMsg::Wake) cùng dòng #[cfg(test)]",
        mk("v2/kernel/vifast-core/src/evil2.rs",
           '#[cfg(test)]\nfn helper() {} fn evil(msg: MailboxMsg) '
           '{ match msg { MailboxMsg::Wake(t) => { let _ = t; } _ => {} } }\n'), show_ok=True)

    # ---- ĐỐI CHỨNG DƯƠNG: R1 (luật token wake) KHÔNG bị cùng-dòng lừa ------
    run("ĐC+ R1 cùng dòng #[cfg(test)] -> phải ĐỎ",
        mk("v2/kernel/vifast-core/src/e.rs",
           '#[cfg(test)]\nfn helper() {} pub fn evil(m: &mut M, t: u32) '
           '{ let _ = m.wake(t); }\n'), show_ok=True)
    run("ĐC+ R1 trong #[cfg(test)] mod -> phải XANH",
        mk("v2/kernel/vifast-core/src/e.rs",
           '#[cfg(test)]\nmod t { pub fn evil(m: &mut M, t: u32) { let _ = m.wake(t); } }\n'),
        show_ok=True)

    # ---- N6: include!("\0") -> crash --------------------------------------
    try:
        run("N6 include!(\"\\0\") -> crash?", mk("v2/kernel/vifast-core/src/e.rs",
                                                'include!("\\0");\n'), show_ok=True)
    except Exception as e:  # noqa: BLE001
        print(f"=== N6 include!(\"\\0\")\n  CRASH: {type(e).__name__}: {e}")

    # ---- N3 / DEBT-KT227-1: v2/lib ----------------------------------------
    run("N3 cfg_attr lồng 2 tầng trong v2/lib (đích trong v2/lib)",
        mk("v2/lib/vifast-abi/src/lib.rs",
           '#[cfg_attr(any(), cfg_attr(any(), path = "evil.rs"))]\nmod m;\n',
           {"v2/lib/vifast-abi/src/evil.rs": "pub fn evil() {}\n"}), show_ok=True)
    run("N3b cfg_attr lồng trong v2/lib trỏ RA NGOÀI v2/lib -> phải ĐỎ",
        mk("v2/lib/vifast-abi/src/lib.rs",
           '#[cfg_attr(any(), cfg_attr(any(), '
           'path = "../../../kernel/vifast-core/src/core_sched.rs"))]\nmod m;\n'), show_ok=True)
    run("DEBT-KT227-1 token wake trong v2/lib (ngoài thân macro)",
        mk("v2/lib/vifast-abi/src/lib.rs",
           "pub fn evil(m: &mut Machine, t: u32) { let _ = m.wake(t); }\n"), show_ok=True)

    # ---- L2: crate mới dưới v2/kernel -------------------------------------
    run("L2 crate mới dưới v2/kernel (mẫu in_test_only_wake_scope)",
        mk("v2/kernel/vifast-newcrate/src/lib.rs",
           "pub fn evil(m: &mut Machine, t: u32) { let _ = m.wake(t); }\n"), show_ok=True)

    # ---- include!/#[path]: các dạng đã thử, đều ĐỎ (fail-closed) -----------
    for name, text, files in [
        ("include! byte string", 'include!(b"x.rs");\n', None),
        ("include! C string", 'include!(c"x.rs");\n', None),
        ("include! raw byte string", 'include!(br"x.rs");\n', None),
        ("#[path] byte string", '#[path = b"x.rs"]\nmod m;\n', None),
        ("#[path] trong khối (...)", 'const _: () = { #[path = "../../../../evil.rs"] mod m; };\n', None),
        ("#[path] trong inline mod của tệp non-mod-rs",
         'mod inner { #[path = "../../../../evil.rs"] mod baz; }\n', None),
        ("#[path] .txt có wake", '#[path = "evil.txt"]\nmod m;\n',
         {"v2/kernel/vifast-core/src/evil.txt": EVIL}),
        ("include! .txt có wake", 'include!("evil.txt");\n',
         {"v2/kernel/vifast-core/src/evil.txt": EVIL}),
        ("include! .RS hoa có wake", 'include!("evil.RS");\n',
         {"v2/kernel/vifast-core/src/evil.RS": EVIL}),
        ("mod .RS hoa (FS không phân biệt hoa/thường)", 'mod evil;\n',
         {"v2/kernel/vifast-core/src/evil.RS": EVIL}),
    ]:
        run(name, mk("v2/kernel/vifast-core/src/e.rs", text, files), show_ok=True)


if __name__ == "__main__":
    main()
