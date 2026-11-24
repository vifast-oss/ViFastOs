chen() {
  mkdir -p v2/sim/vifast-sim/bin
  cat > v2/sim/vifast-sim/bin/kt242_bin.rs <<'RS'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-242 — bin khai báo **ngoài `src/`** (`[[bin]] path = "bin/…"`).

use vifast_sim::kobj::Machine;

fn main() {
    let mut m = Machine::new();
    let _ = m.wake(0);
}
RS
  printf '\n[[bin]]\nname = "kt242-bin"\npath = "bin/kt242_bin.rs"\n' >> v2/sim/vifast-sim/Cargo.toml
}
go() { :; }
