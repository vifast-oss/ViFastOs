#!/usr/bin/env bash
# KT-71 — kiểm chéo: mọi số trong docs/STATE/journal phải xuất hiện trong log ĐÃ CHÉP VÀO REPO.
set -u
J=v2/agent/JOURNAL
DOCS="v2/docs/k6-vfs-journal.md v2/docs/k3b-core.md v2/agent/STATE.md v2/agent/JOURNAL/kt71-journal.md"
ok=0 bad=0
check() { # <token> <log...>
    local tok="$1"; shift
    local d l found_d=0 found_l=0
    for d in $DOCS; do grep -qF -- "$tok" "$d" && found_d=1; done
    for l in "$@"; do grep -qF -- "$tok" "$l" && found_l=1; done
    if [ "$found_d" = 1 ] && [ "$found_l" = 1 ]; then
        echo "ok   \"$tok\""
        ok=$((ok + 1))
    else
        echo "FAIL \"$tok\" (docs=$found_d logs=$found_l)"
        bad=$((bad + 1))
    fi
}
B=$J/kt71-ktest-boot.log
check "index=143 used=6->9->6 delta=3 (=3: root+bang L2+data) root=37 l2=38 data_ptr=36" "$B"
check "reuse=4 (=used_again-used_t0) used_again=6 used_t0=2 reuse_ok=true" "$B"
check "dirty_marks=0->1 dirty_now=0 writes=8 flushes=1" "$B"
check "replay=1 new=1 size=7 keep=true fsck=true crc_reject=2 crc_applied=0 crc_disk=old" "$B"
check "pre=old replay_r=1 new_r=1 post=new keep_r=true" "$B"
check "phases=7 writes=7 old=3 new=4 torn=0 consistent=7 threshold=4 last_old=3 atomic=1 replays=3" "$B"
check "found=4 fixed=4 unfixed=0 orphan_blocks=1 orphan_inodes=1 bad_nlink=1 bad_blocks=1 used=2->4->2" "$B"
check "unfixed_pos=true unfixed2=1 double_blocks2=1 second_bad=true" "$B"
check "stack entry used=39761" "$B"
check "stack end used=40577" "$B"
check "ktest 70 70" v2/test/boot/expected.txt
check "KTEST: 12/12 boot sạch" "$J/kt71-ktest-matrix.log"
check "trace-compare: 61/61 dòng khớp, 5/5 ca" "$J/kt71-conformance.log"
check "soak: 0/1000 lỗi (PASS)" "$J/kt71-soak.log"
check "BUILD_ID=02ed5a1810ba59ce" "$J/kt71-soak.log"
check "a0e187e8ae1dc5cd448283bd3cdccb32a80ceda8c9c7d225d6da049e89713b34" "$J/kt71-product-sha.log" "$J/kt71-soak.log"
check "build=02ed5a1810ba59ce text=49854" "$J/kt71-product-sha.log"
check "data+bss=242840" "$J/kt71-budget-ktest.log"
check "EXIT=0" "$J/kt71-soak.log"
check "not ok 67 - fs journal replay sau crash" "$J/kt71-red-reorder-qemu-boot.log"
check "replay_r=2 new_r=0 post=old" "$J/kt71-red-reorder-qemu-boot.log"
check "not ok - ktest smp=1 boot 1" "$J/kt71-red-reorder-qemu.log"
check "left: 2" "$J/kt71-red-reorder.log"
check "L1 OK: 1000 seed, 999 trace khác nhau" "$J/kt71-gate.log" "$J/kt71-l1-thuan.log"
echo "---"
echo "# verify: ok=$ok fail=$bad"
[ "$bad" = 0 ]
