# SPDX-License-Identifier: MIT OR Apache-2.0
# P7c — `[[bin]] path = "../evil/main.rs"` trỏ tới tệp **không thuộc crate nào**
# (`v2/kernel/evil/main.rs`): `cargo check --workspace` biên dịch nó, nhưng `classify()` không
# quy được về crate ⇒ lớp tĩnh mù (token_scope=False), lớp dep-info phải bắt.
chen() {
    mkdir -p v2/kernel/evil
    cat > v2/kernel/evil/main.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
#![no_main]
use core::panic::PanicInfo;

#[panic_handler]
fn kt258_panic(_: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut m = vifast_core::Machine::new();
    let _ = m.wake(0);
    loop {}
}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-plat-common/Cargo.toml"); t = p.read_text()
t += '\n[[bin]]\nname = "kt258-evil"\npath = "../evil/main.rs"\n'
p.write_text(t)
PY
}
