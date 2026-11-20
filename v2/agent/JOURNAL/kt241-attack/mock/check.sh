#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-241 — tự kiểm MÃ LỖI CARGO của các tệp tấn công trên một MOCK API (ngoài cây ViFastOs).
# Mục đích: chứng minh tệp tấn công không hỏng cú pháp và sinh ĐÚNG mã lỗi kỳ vọng
# (E0308/E0624/E0631/unsafe) TRƯỚC khi API thật hợp nhất.
# KHÔNG phải bằng chứng nghiệm thu; nghiệm thu là `run.sh --all --with-depinfo` trên cây thật.
set -uo pipefail
SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ATTACK="$(cd "$SELF_DIR/.." && pwd)"
CELLS="$ATTACK/cells"
MANIFEST="$ATTACK/manifest.tsv"
MOCK="${MOCK_DIR:-/tmp/kt241-mock}"
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.99.1}"

rm -rf "$MOCK"; mkdir -p "$MOCK/src"
cat > "$MOCK/Cargo.toml" <<'EOF'
[package]
name = "kt241-mock"
version = "0.1.0"
edition = "2021"
EOF
cat > "$MOCK/src/lib.rs" <<'EOF'
#![no_std]
#![forbid(unsafe_code)]
pub type ObjId = u32;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KError { IllegalOperation, InvalidArgument }
pub mod tcb {
    use super::ObjId;
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FaultKind { Exit { code: u32 } }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SleepReason { Timeout, Budget }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TcbState {
        Inactive, Ready, Running { hart: u16 },
        BlockedOnSend { ep: ObjId, call: bool, fault: Option<FaultKind> },
        BlockedOnRecv { ep: ObjId, reply: Option<ObjId> },
        BlockedOnReply { reply: ObjId },
        BlockedOnNotification { ntfn: ObjId },
        BlockedOnAddress { frame: ObjId, offset: u32 },
        Sleeping { reason: SleepReason }, Stopping, Dead,
    }
}
pub use tcb::TcbState;
pub struct Machine { pub n: u32 }
impl Machine {
    pub fn transition_tcb(&mut self, _t: ObjId, _to: NonReadyTarget) -> Result<(), KError> { Ok(()) }
    pub fn block_current(&mut self, _h: u16, _to: NonReadyTarget) -> Result<ObjId, KError> { Ok(0) }
    pub fn wake(&mut self, _t: ObjId) -> Result<(), KError> { Ok(()) }
}
mod state_write {
    use super::tcb::{FaultKind, SleepReason, TcbState};
    use super::{KError, Machine, ObjId};
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum NonReadyTarget {
        Inactive, Running { hart: u16 },
        BlockedOnSend { ep: ObjId, call: bool, fault: Option<FaultKind> },
        BlockedOnRecv { ep: ObjId, reply: Option<ObjId> },
        BlockedOnReply { reply: ObjId },
        BlockedOnNotification { ntfn: ObjId },
        BlockedOnAddress { frame: ObjId, offset: u32 },
        Sleeping { reason: SleepReason }, Stopping, Dead,
    }
    impl NonReadyTarget { fn into_state(self) -> TcbState { TcbState::Inactive } }
    impl Machine { fn write_state(&mut self, _t: ObjId, _to: TcbState) -> Result<(), KError> { Ok(()) } }
}
pub use state_write::NonReadyTarget;
EOF
cp "$MOCK/src/lib.rs" "$MOCK/lib.base"

codes_of() { grep -oE 'error\[E[0-9]+\]|error: usage of an .unsafe. block' "$1" 2>/dev/null | sed -E 's/error\[(E[0-9]+)\]/\1/; s/.*unsafe.*/unsafe/' | sort -u | paste -sd, -; }
reset_mock() { cp "$MOCK/lib.base" "$MOCK/src/lib.rs"; rm -rf "$MOCK/src/kt241_"* "$MOCK/src/target" "$MOCK/src/kt241 space.rs" "$MOCK/src/kt241 ok.rs" "$MOCK/build.rs" "$MOCK/evil.rs"; }

pass=0; fail=0
while IFS=$'\t' read -r id group kind ecargo ecodes el1 el1s el2 ecodes_checker positive pkind desc; do
    [[ -z "$id" || "$id" == \#* ]] && continue
    case "$kind" in core|core2) ;; *) continue ;; esac
    reset_mock
    mod="kt241_$(printf '%s' "$id" | tr 'A-Z' 'a-z')"
    cp "$CELLS/$id.rs.txt" "$MOCK/src/$mod.rs"
    if [[ "$kind" == "core2" ]]; then cp "$CELLS/$id.helper.rs.txt" "$MOCK/src/${mod}_helper.rs"; fi
    if [[ -f "$CELLS/$id.evil.rs.txt" ]]; then cp "$CELLS/$id.evil.rs.txt" "$MOCK/src/${mod}_evil.rs"; fi
    printf 'mod %s;\n' "$mod" >> "$MOCK/src/lib.rs"
    [[ "$kind" == "core2" ]] && printf 'mod %s_helper;\n' "$mod" >> "$MOCK/src/lib.rs"
    ( cd "$MOCK" && cargo check --manifest-path "$MOCK/Cargo.toml" ) > "$MOCK/out.log" 2>&1; rc=$?
    got="$(codes_of "$MOCK/out.log")"; [[ -z "$got" ]] && got="-"
    ok=0
    case "$ecargo" in
        ok) [[ "$rc" -eq 0 ]] && ok=1 ;;
        fail) [[ "$rc" -ne 0 ]] && { [[ "$ecodes" == "-" ]] || printf '%s' "$got" | grep -qE "$ecodes"; } && ok=1 ;;
    esac
    if [[ "$ok" -eq 1 ]]; then pass=$((pass+1)); printf 'PASS %-3s cargo rc=%-3s codes=%s\n' "$id" "$rc" "$got"
    else fail=$((fail+1)); printf 'FAIL %-3s cargo rc=%-3s codes=%s (muốn %s/%s)\n' "$id" "$rc" "$got" "$ecargo" "$ecodes"; fi
done < <(awk -F'\t' '!/^#/ && NF>=12 {print}' "$MANIFEST")
reset_mock
printf '\nMOCK: PASS=%d FAIL=%d\n' "$pass" "$fail"
[[ "$fail" -eq 0 ]]
