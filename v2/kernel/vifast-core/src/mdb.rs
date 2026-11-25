// SPDX-License-Identifier: MIT OR Apache-2.0
//! Helper **MDB/cây object** của [`Machine`] (KT-63 tách khỏi `machine.rs` để giữ R1 ≤ 1.500
//! dòng): gom cây dẫn xuất theo cạnh cha, gom cây Untyped (đệ quy), mảng có biên + sắp xếp/khử
//! trùng tại chỗ (`no_std`, không cấp phát) — port nguyên `collect_subtree`/
//! `collect_untyped_tree` của sim (`v2/sim/vifast-sim/src/kobj.rs`, oracle).

use crate::error::{CapId, ObjId, ObjType};
use crate::machine::{Machine, MAX_CAPS, MAX_OBJECTS};
use crate::observer::CoreObserver;
use crate::store::KernelStore;

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
    /// Gom `root` và mọi con/cháu còn sống của nó (theo cạnh cha) vào `out`.
    pub(crate) fn collect_subtree(&self, root: CapId, out: &mut [CapId; MAX_CAPS], n: &mut usize) {
        for i in 1..=self.n_caps as CapId {
            if !self.caps[i as usize - 1].alive {
                continue;
            }
            let mut cur = Some(i);
            let mut steps = 0usize;
            while let Some(p) = cur {
                if p == root {
                    push_id(out, n, i);
                    break;
                }
                if p == 0 || p as usize > self.n_caps {
                    break;
                }
                cur = self.caps[p as usize - 1].parent;
                steps += 1;
                if steps > self.n_caps {
                    break;
                }
            }
        }
    }

    /// Gom mọi object sống nằm trong `u_idx`, đệ quy qua Untyped con.
    pub(crate) fn collect_untyped_tree(
        &self,
        u_idx: usize,
        out: &mut [ObjId; MAX_OBJECTS],
        n: &mut usize,
    ) {
        for i in 0..self.n_objects {
            let (alive, unty, ty) = {
                let m = &self.objects[i];
                (m.alive, m.untyped, m.ty)
            };
            if !alive || unty.map(usize::from) != Some(u_idx) {
                continue;
            }
            push_id(out, n, (i as u32) + 1);
            if ty == ObjType::Untyped {
                self.collect_untyped_tree(i, out, n);
            }
        }
    }
}

/// Thêm id vào mảng có biên (bỏ nếu đầy — mọi nơi gọi đều ≤ `MAX_*` theo bất biến bảng).
pub(crate) fn push_id<const N: usize>(out: &mut [u32; N], n: &mut usize, id: u32) {
    if *n < N {
        out[*n] = id;
        *n += 1;
    }
}

/// Sắp xếp + khử trùng tại chỗ (mảng nhỏ, không cấp phát).
pub(crate) fn sort_dedup(out: &mut [CapId; MAX_CAPS], n: &mut usize) {
    for i in 1..*n {
        let x = out[i];
        let mut j = i;
        while j > 0 && out[j - 1] > x {
            out[j] = out[j - 1];
            j -= 1;
        }
        out[j] = x;
    }
    let mut w = 0usize;
    for r in 0..*n {
        if w == 0 || out[w - 1] != out[r] {
            out[w] = out[r];
            w += 1;
        }
    }
    *n = w;
}
