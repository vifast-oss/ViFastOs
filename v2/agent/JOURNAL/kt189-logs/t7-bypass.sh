#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-189 / L28 — T7: đường vòng mới (build.rs, paste/concat_idents, tuyệt đối, chuỗi không-.rs).
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"
T=$BASE/t7
EVIL='use crate::{Machine, ObjId};

impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
'
: > "$BASE/logs/t7.log"
probe() {
    label=$1; D=$2
    {
      echo "### $label"
      run "$label CUR (chế độ thường)" "$CUR" "$D"
      run "$label CUR --strict" "$CUR" "$D" --strict
      echo "### $label cargo check -p vifast-core"
      ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -4 )
      echo
    } >> "$BASE/logs/t7.log" 2>&1
}

# (a) build.rs chứa include!(concat!(env!("OUT_DIR"), …)) — báo oan tiềm ẩn
D=$T/buildrs
build_full "$D"
cat > "$D/v2/kernel/vifast-core/build.rs" <<'EOF'
fn main() {
    include!(concat!(env!("OUT_DIR"), "/gen.rs"));
}
EOF
probe "T7a build.rs chứa include!(concat!(env!(\"OUT_DIR\"), …))" "$D"

# (b) mã sản phẩm include!(concat!(env!("OUT_DIR"), …)) — phải ĐỎ (fail-closed)
D=$T/outdir
build_full "$D"
printf '\ninclude!(concat!(env!("OUT_DIR"), "/gen.rs"));\n' >> "$D/v2/kernel/vifast-core/src/core_sched.rs"
probe "T7b sản phẩm include!(concat!(env!(\"OUT_DIR\"), …))" "$D"

# (c) include! đường dẫn TUYỆT ĐỐI ra ngoài cây
D=$T/abs
build_full "$D"
printf 'pub fn clean() {}\n' > /tmp/kt189-l28/evil_abs.rs
printf '\ninclude!("/tmp/kt189-l28/evil_abs.rs");\n' >> "$D/v2/kernel/vifast-core/src/core_sched.rs"
probe "T7c include! đường dẫn tuyệt đối ra ngoài cây" "$D"

# (d) chuỗi include! qua tệp KHÔNG phải .rs (evil.inc) → evil_t.rs test-only có wake
D=$T/inc-chain
build_full "$D"
printf '\ninclude!("evil.inc");\n' >> "$D/v2/kernel/vifast-core/src/core_sched.rs"
printf 'include!("evil_t.rs");\n' > "$D/v2/kernel/vifast-core/src/evil.inc"
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/evil_t.rs"
printf '\n#[cfg(test)]\nmod evil_t;\n' >> "$D/v2/kernel/vifast-core/src/lib.rs"
probe "T7d chuỗi include! qua tệp .inc → evil_t.rs (test-only, có wake)" "$D"

# (e) wake là đối số macro lạ (paste/concat_idents) — phải ĐỎ ở --strict
D=$T/macroarg
build_full "$D"
printf '\nfn dung_macro(m: &mut Machine, t: ObjId) { concat_idents!(wa, ke); let _ = m; let _ = t; }\n' >> "$D/v2/kernel/vifast-core/src/core_sched.rs"
probe "T7e concat_idents!(wa, ke) — không có token wake" "$D"

# (f) wake là đối số macro KHÔNG an toàn — phải ĐỎ ở --strict
D=$T/macroarg2
build_full "$D"
printf '\nfn dung_macro2(m: &mut Machine, t: ObjId) { some_macro!(wake); let _ = m; let _ = t; }\n' >> "$D/v2/kernel/vifast-core/src/core_sched.rs"
probe "T7f some_macro!(wake) — token wake là đối số macro lạ" "$D"

# (g) mod trong macro + #[cfg(not(test))] (đã có ở T3c) — kiểm lại trên cây đầy đủ
echo "xong t7"
