#!/usr/bin/env bash
# KT-252 B3 — **closure trong thân cửa** `wake_step` gọi `write_state` (nợ đã ghi: closure không
# phải `fn` nên `_innermost_fn_tree` quy cho cửa ⇒ xanh). Không phải cửa mới — là mã của cửa.
chen() {
    perl -0pi -e 's{(    pub\(crate\) fn wake_step\(&mut self, tcb: ObjId, unlink: bool\) -> Result<\(\), KError> \{\n)}{$1        // KT-252 B3 — closure trong thân cửa gọi `write_state` (nợ đã ghi).\n        let kt252_b3 = |s: &mut Self, t: ObjId| s.write_state(t, TcbState::Ready);\n        let _ = kt252_b3(self, tcb);\n}' v2/kernel/vifast-core/src/state_write.rs
}
go() { :; }
