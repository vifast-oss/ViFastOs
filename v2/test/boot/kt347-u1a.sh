#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-347 (U1a)** — T0: kiểm hợp đồng của `vifast-rt` + đường `exec` đọc ELF từ FS, **không QEMU**.
#
# Vì sao có tệp này: D2-05 đòi test **đỏ trước** cho từng tính năng mới. Phần chạy được trên
# host (không cần ảnh/QEMU) gồm:
#   1. lõi bump của `vifast-rt` — `rustc --test v2/user/rt/bump.rs` (8 ca: căn lề, hết chỗ
#      fail-closed, tràn số, `restore`, `reset`);
#   2. hợp đồng mã nguồn: `vifast-rt` có `entry`/`println!`/panic handler/`BumpAlloc`; kernel có
#      API `proc_fs::read_file` + `exec::exec_bytes`; **không** còn `include_bytes!` cho ELF U-mode
#      trong `proc_exec.rs`; runner seed đĩa bằng `vifast-fsutil` và kiểm `src=fs`.
#
# Exit: 0 = mọi phép kiểm xanh; 1 = có phép kiểm đỏ.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
USER="$V2_ROOT/user"
KBIN="$V2_ROOT/kernel/vifast-kbin-riscv64/src"
PRODUCT="$HERE/product-riscv64.sh"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
CHECKS=0
FAILS=0
good() { CHECKS=$((CHECKS + 1)); echo "ok $CHECKS - $*"; }
bad() { CHECKS=$((CHECKS + 1)); FAILS=$((FAILS + 1)); echo "not ok $CHECKS - $*"; }

need() { # <tệp> <mẫu regex> <nhãn>
    if grep -aqE "$2" "$1" 2>/dev/null; then good "$3"; else bad "$3 (thiếu /$2/ trong ${1#"$V2_ROOT"/})"; fi
}

# ---------------------------------------------------------------- 1) lõi bump chạy trên host
TMPD="$(mktemp -d "${TMPDIR:-/tmp}/kt347-u1a.XXXXXX")" || exit 2
trap 'rm -rf "$TMPD"' EXIT
[ -f "$USER/runtime/bump.rs" ] || {
    bad "vifast-rt: có $USER/runtime/bump.rs"
}
if rustc +1.100.0 --edition 2021 --test "$USER/runtime/bump.rs" -o "$TMPD/bump-tests" >"$TMPD/rustc.log" 2>&1 &&
    "$TMPD/bump-tests" >"$TMPD/bump.log" 2>&1; then
    passed="$(sed -n 's/^test result: ok\. \([0-9]*\) passed.*/\1/p' "$TMPD/bump.log" | head -1)"
    if [ "${passed:-0}" -ge 8 ]; then
        good "bump: ${passed} ca host PASS (căn lề, hết chỗ, tràn số, restore)"
    else
        bad "bump: chỉ ${passed:-0} ca PASS (mong >=8)"
    fi
else
    bad "bump: rustc --test FAIL (xem $TMPD/rustc.log)"
    sed -n '1,5p' "$TMPD/rustc.log" >&2
fi

# ---------------------------------------------------------------- 2) hợp đồng vifast-rt
need "$USER/rt/lib.rs" 'pub fn entry\(main: fn\(\)\) -> !' "rt: có entry(main)"
need "$USER/rt/lib.rs" 'macro_rules! println' "rt: có println!"
need "$USER/rt/lib.rs" '#\[panic_handler\]' "rt: có panic handler"
need "$USER/rt/lib.rs" 'pub use vifast_user_abi::heap' "rt: tái xuất heap (BumpAlloc/HEAP_BYTES)"
need "$USER/rt/lib.rs" 'fail-closed' "rt: ghi rõ hợp đồng fail-closed"
need "$USER/runtime/lib.rs" '#\[cfg\(rt_alloc\)\]' "runtime: mod heap chỉ biên dịch với --cfg rt_alloc"
need "$USER/runtime/lib.rs" 'alloc fail' "runtime: OOM in tên lỗi (fail-closed)"
need "$USER/programs/u1a.rs" '#\[global_allocator\]' "u1a: đặt global allocator"
need "$USER/programs/u1a.rs" '# u1a: rt done' "u1a: có dấu evidence rt done"

# ---------------------------------------------------------------- 3) exec đọc ELF từ FS
if grep -aqE '^static [^=]*= *include_bytes!\(' "$KBIN/proc_exec.rs"; then
    bad "exec: proc_exec.rs CÒN include_bytes! (chưa đọc từ FS)"
else
    good "exec: proc_exec.rs không còn include_bytes! (ELF từ FS)"
fi
need "$KBIN/proc_exec.rs" 'read_file' "exec: proc_exec dùng proc_fs::read_file"
need "$KBIN/proc_exec.rs" 'exec_bytes' "exec: proc_exec dùng exec::exec_bytes"
need "$KBIN/proc_exec.rs" 'src=fs bytes=' "exec: log evidence có src=fs bytes="
need "$KBIN/proc_fs.rs" 'pub fn read_file' "fs: có API đọc cả tệp read_file"
need "$V2_ROOT/kernel/vifast-plat-riscv64/src/exec.rs" 'pub fn exec_bytes' "loader: có exec_bytes"
need "$PRODUCT" 'vifast-fsutil' "runner: seed đĩa bằng vifast-fsutil"
need "$PRODUCT" 'seed_disk' "runner: hàm seed_disk"
need "$PRODUCT" 'run /u1a\.elf' "runner: boot kiểm u1a.elf"
need "$PRODUCT" 'src=fs bytes=\[1-9\]' "runner: đòi evidence src=fs bytes>0"
need "$PRODUCT" 'ELF U-mode KHÔNG nhúng' "runner: kiểm ELF không nhúng trong ảnh"

# ---------------------------------------------------------------- 4) tài liệu + test đỏ
DOC="$V2_ROOT/docs/u1-uefi-readiness.md"
[ -f "$DOC" ] && good "doc: có u1-uefi-readiness.md" || bad "doc: thiếu u1-uefi-readiness.md"
need "$DOC" '^## 2\.' "doc: mục hợp đồng .vidl"
need "$DOC" '^## 3\.' "doc: mục kiểm chéo cục bộ"
need "$DOC" '^## 4\.' "doc: mục lộ trình UEFI"
if grep -aq 'D2-241' "$DOC" && grep -aq 'launchd' "$DOC"; then
    good "doc: kiểm chéo cục bộ theo D2-241 (script + hẹn giờ launchd, không CI)"
else
    bad "doc: thiếu D2-241/launchd trong kịch bản kiểm chéo"
fi

echo
echo "# kt347-u1a: checks=$CHECKS fails=$FAILS"
[ "$FAILS" -eq 0 ] || exit 1
