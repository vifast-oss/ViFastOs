// KT-241 D6 — đích NGOÀI cây quét (`v2/kernel`+`v2/sim`), chứa `wake`.
// Được kéo vào bằng `#[path]` từ `v2/kernel/vifast-core/src/kt241_d6.rs`.
use crate::{Machine, ObjId};

pub fn evil(m: &mut Machine, t: ObjId) {
    let _ = m.wake(t);
}
