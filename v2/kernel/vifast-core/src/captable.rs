// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Iterator bảng cap/con của cap — thay `Vec` của sim (KT-58), tách khỏi `machine.rs` để giữ
//! giới hạn dòng mỗi tệp (QD §3.15).

use crate::error::{CapId, CapInfo};
use crate::machine::Cap;

/// Iterator bảng cap (thay `Vec<CapInfo>` của sim).
#[derive(Debug, Clone)]
pub struct CapTable<'a> {
    pub(crate) caps: &'a [Cap],
    pub(crate) idx: usize,
}

impl Iterator for CapTable<'_> {
    type Item = CapInfo;

    fn next(&mut self) -> Option<CapInfo> {
        let i = self.idx;
        if i >= self.caps.len() {
            return None;
        }
        self.idx += 1;
        let c = &self.caps[i];
        Some(CapInfo {
            cap: (i as u32) + 1,
            obj: c.obj,
            ty: c.ty,
            parent: c.parent(),
            slot: c.slot(),
            rights: c.rights,
            badge: c.badge(),
            alive: c.alive(),
        })
    }
}

/// Iterator con trực tiếp của một cap (thay `Vec<CapId>` của sim).
#[derive(Debug, Clone)]
pub struct CapChildren<'a> {
    pub(crate) caps: &'a [Cap],
    pub(crate) parent: CapId,
    pub(crate) idx: usize,
}

impl Iterator for CapChildren<'_> {
    type Item = CapId;

    fn next(&mut self) -> Option<CapId> {
        while self.idx < self.caps.len() {
            let i = self.idx;
            self.idx += 1;
            let c = &self.caps[i];
            if c.alive() && c.parent() == Some(self.parent) {
                return Some((i as u32) + 1);
            }
        }
        None
    }
}
