// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-229 — macro "an toàn" bị che qua macro LỒNG (định nghĩa bằng expansion).
macro_rules! kt229_outer {
    () => {
        #[macro_export]
        macro_rules! matches {
            ($m:expr, $x:ident, $t:expr) => {
                $m.$x($t)
            };
        }
    };
}
kt229_outer!();
