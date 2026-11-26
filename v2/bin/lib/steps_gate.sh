#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245) — **hàm bước của `gate --fast`/`gate --dev`** (L0 song song, L0 kernel, L1,
# selftest runner/QEMU/persist/git-guard, QEMU smoke T1) + `l0_test_parallel` (KT-242 P1).
# Thân hàm **nguyên văn** bản cũ trong `v2/bin/v2`.
# --- `gate --fast` ---
step_l0() { l0_test_parallel; }
step_l0_kernel() {
    # D2-253/D2-262: `v2/Cargo.toml` đặt `exclude = ["kernel"]` nên `-p vifast-core` (137 test)
    # và `-p vifast-ktest` (16 test) từng bị L0 kernel bỏ sót — thêm vào đây để cổng phủ đủ 153 test
    # (0 bước cổng mới: cùng lệnh `cargo test`, chạy thực tế ~0,2 s).
    "$CARGO" test --manifest-path "$V2_ROOT/kernel/Cargo.toml" \
        -p vifast-plat-common -p vifast-plat-aarch64 -p vifast-plat-x86_64 \
        -p vifast-plat-riscv64-logic -p vifast-core -p vifast-ktest
}
step_l1() {
    local t0 t1 out
    t0="$(now_ms)"
    if out="$("$CARGO" test --manifest-path "$MANIFEST" -p vifast-sim l1_ -- --nocapture 2>&1)"; then
        printf '%s\n' "$out" | grep -E "^# L1 OK" || true
        t1="$(now_ms)"
        echo "--- L1: OK ($((t1 - t0)) ms, đo bởi cổng)"
        return 0
    fi
    printf '%s\n' "$out"
    echo "--- L1: FAIL"
    return 1
}
step_selftest_ktest() { bash "$V2_ROOT/test/boot/selftest.sh"; }
step_guard() { bash "$V2_ROOT/test/boot/guard_selftest.sh"; }
step_pre_measure() { bash "$V2_ROOT/tools/pre-measure.sh" --selftest; }
step_pin_artifact() { bash "$V2_ROOT/tools/pin-artifact.sh" --selftest; }
step_algo_strict() { bash "$V2_ROOT/test/boot/algo-strict.sh"; }
step_persist_red() { bash "$V2_ROOT/test/boot/persist.sh" --red-selftest; }
# **KT-245** (D2-196): tự kiểm validator của runner virtio-net — chứng minh bộ kiểm **không
# rỗng** (5 đột biến log tốt + ca âm "PASS giả" đều phải bị từ chối). Không QEMU, không sửa cây.
step_net_red() { bash "$V2_ROOT/test/boot/net.sh" --red-selftest; }
# **KT-250** (D2-195): runner QEMU aarch64 — build ảnh `vifast-kbin-aarch64`, boot, TAP 5 ca,
# 5 ca âm (marker sai/thiếu ok/not ok/plan sai/log rỗng). ~10 s; chạy ở **cả T1 và T2**.
step_aarch64() { bash "$V2_ROOT/test/boot/run-aarch64.sh"; }
# **KT-246** (D2-196): runner virtio-net **đầy đủ** — ca dương (DHCP+ARP+ping) + 2 ca âm, 3 boot
# QEMU (~40 s) ⇒ chỉ T2 (thêm vào `V2_T2_ONLY_STEPS`).
step_net_run() { bash "$V2_ROOT/test/boot/net.sh"; }
# **KT-257** (D2-201): runner QEMU x86_64 — build ảnh `vifast-kbin-x86_64`, boot long mode, marker,
# 5 ca âm. ~10 s; chạy ở **cả T1 và T2**.
step_x86_64() { bash "$V2_ROOT/test/boot/run-x86_64.sh"; }
# **KT-255** (D2-200): runner virtio-input — QEMU + monitor `sendkey`, 5 TAP + ca âm (~4 s) ⇒ T2.
step_input_run() { bash "$V2_ROOT/test/boot/input.sh"; }
# **KT-256** (D2-200): kiểm máy "ảnh sản phẩm **không** có `--cfg test`" — build thật (~40 s) ⇒ T2.
step_no_cfg_test() { python3 "$V2_ROOT/tools/check-no-cfg-test.py"; }
# Tự kiểm **validator** (không build, không QEMU) — KT-134 đòi mọi script có cờ tự kiểm phải có
# cổng gọi; hai bước dưới đây là bản rẻ của KT-255/KT-256, chạy ở **cả T1 và T2**.
step_input_red() { bash "$V2_ROOT/test/boot/input.sh" --red-selftest; }
step_no_cfg_test_red() { python3 "$V2_ROOT/tools/check-no-cfg-test.py" --selftest; }
# **KT-263**: runner virtio-gpu — QEMU + `screendump` PPM + kiểm điểm ảnh (~6 s) ⇒ T2.
step_gpu_run() { bash "$V2_ROOT/test/boot/gpu.sh"; }
# **KT-333c (D2-243 mục 3b)** — `gpu-x86_64.sh` vào T2 **kèm ca âm "không GPU"**: ảnh build
# không feature `gpu` nhưng QEMU vẫn có `-device virtio-gpu-pci` ⇒ bộ kiểm phải ĐỎ (không PASS giả).
# Đo được **21 s** cho ca dương (fresh worktree, 2026-11-22) + ~10 s ca âm ⇒ dưới trần 60 s.
step_gpu_x86_64() { KT337_NEGATIVE=1 bash "$V2_ROOT/test/boot/gpu-x86_64.sh"; }
step_gpu_red() { bash "$V2_ROOT/test/boot/gpu.sh" --red-selftest; }
# **KT-265**: tự kiểm bộ đo aarch64 (không build, không QEMU) — soak 1.000 và budget chỉ chạy
# `--selftest` ở cổng; soak thật là bước bàn giao riêng (`soak-aarch64.sh 1000 3`).
step_a64_soak_selfcheck() { bash "$V2_ROOT/test/boot/soak-aarch64.sh" --selftest; }
step_a64_budget() { bash "$V2_ROOT/test/boot/budget-aarch64.sh" --selftest; }
# **KT-340a** (D2-236): tự kiểm bộ đo ngân sách **x86_64** (e_machine=62, `.smp_stacks` riêng,
# ca âm 300 KiB `.bss` + ca âm arch) — thuần bash, không build, không QEMU.
step_x64_budget() { bash "$V2_ROOT/test/boot/budget-x86_64.sh" --selftest; }
# **KT-267 (c)**: lớp dep-info thấy mã chỉ tồn tại dưới feature của ảnh sản phẩm (`product-fs`).
# Tốn thêm ~20 s ấm ⇒ chỉ T2.
step_wake_depinfo_product_fs() { python3 "$V2_ROOT/tools/check-wake-depinfo.py" --product-fs; }
# **KT-293/296**: selftest của `soak-x86_64.sh` (thuần bash, ~0,3 s) — nối vào cổng để **xoá**
# miễn trừ tạm thời trong `check-selftest-wiring.py`.
step_x64_soak_selfcheck() { bash "$V2_ROOT/test/boot/soak-x86_64.sh" --selftest; }
step_persist_crash() { bash "$V2_ROOT/test/boot/persist.sh" --crash; }
step_selfbreak() { bash "$V2_ROOT/test/rules/selfbreak.sh"; }
step_neg1() { bash "$V2_ROOT/tools/git-guard/neg-test.sh"; }
step_neg2() { bash "$V2_ROOT/tools/git-guard/neg-test.sh" --selftest-parallel; }
step_neg3() { bash "$V2_ROOT/tools/git-guard/neg-test-kt85b.sh"; }
step_qemu_smoke() { bash "$V2_ROOT/test/boot/ktest.sh" 1 1; }

# **KT-242 (P1)** — chạy **song song** các binary test của L0 thay vì để cargo chạy lần lượt.
#
# Vì sao: `cargo test --workspace` chạy **từng binary một**; đo trên cây hiện tại (20/11):
# 111 binary, tổng thời gian test **256,5 s**, cộng ~33 s khởi chạy ⇒ ~290 s trên **1 nhân**
# trong khi máy có 12 nhân. Chạy 92 binary với `-P 8` (đo dưới tải) còn **103 s**.
#
# Ngữ nghĩa **không đổi**: cùng binary, cùng test, cùng assert, cùng seed, cùng mã thoát.
# Chỉ đổi *thứ tự/đồng thời*. Ba điều kiện an toàn:
#   1. **Doc-test** không nằm trong binary ⇒ chạy riêng `cargo test --workspace --doc`
#      (nếu bỏ bước này là **mất phủ** 19 suite doc-test).
#   2. **Binary nhạy thời gian** (`kt30_stress` biên 3 s, `spsc` hạn 10 s) chạy **một mình**,
#      không song song — dưới tải chúng đỏ oan (D2-116/D2-141). Danh sách: `V2_TEST_SERIAL`.
#   3. Mỗi binary ghi log + mã thoát **riêng**; gộp sau khi chạy xong ⇒ không đua tệp.
#
# Tinh chỉnh: `V2_TEST_JOBS` (mặc định 6), `V2_TEST_THREADS` (mặc định rỗng = để libtest tự
# chọn `num_cpus`), `V2_TEST_SERIAL` (mặc định "kt30_stress spsc").
l0_test_parallel() {
    local jobs="${V2_TEST_JOBS:-6}" threads="${V2_TEST_THREADS:-}"
    local serial_pat="${V2_TEST_SERIAL:-kt30_stress spsc}"
    local serial_all="${V2_TEST_SERIAL_ALL:-0}"
    local ncpu tmpd rc=0 t0 t1 nrun=0 nfail=0 ntest=0 b pkg n r
    local PY
    PY="$(find_python)" || {
        echo "L0: cần python ≥ 3.11 có tomllib để đọc --message-format=json" >&2
        return 1
    }
    ncpu="$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 4)"
    # **A0 duyệt (2026-11-20)**: giới hạn **tổng** mức song song, không chỉ số process.
    # `jobs` binary × `threads` test-thread mỗi binary ≤ ncpu. Mặc định chia đều; đặt
    # `V2_TEST_THREADS` để ép một giá trị (dùng cho phép đo D).
    if [ -z "$threads" ]; then
        threads=$((ncpu / jobs))
        [ "$threads" -lt 1 ] && threads=1
    fi
    tmpd="$(mktemp -d "${TMPDIR:-/tmp}/v2l0.XXXXXX")" || return 1
    t0="$(now_ms)"

    # 1) Biên dịch toàn bộ test target (cargo tự song song theo crate).
    if ! "$CARGO" test --manifest-path "$MANIFEST" --workspace --no-run \
            --message-format=json >"$tmpd/build.json" 2>"$tmpd/build.err"; then
        cat "$tmpd/build.err" >&2
        rm -rf "$tmpd"
        return 1
    fi
    t1="$(now_ms)"
    echo "# L0: biên dịch xong sau $((t1 - t0)) ms"

    # 2) Danh sách binary test + **thư mục gói** (cwd mà cargo dùng khi chạy test).
    #    - Chỉ lấy artifact của **lần build hiện tại** (`--message-format=json` của chính
    #      lệnh vừa chạy), không quét binary cũ trong target dir.
    #    - Ghi **NUL-separated** để đường dẫn có dấu cách vẫn đúng.
    #    - Vì sao cần cwd: một số test đọc đường dẫn **tương đối** (`../../agent/QUEUE.md`);
    #      cargo chạy binary với cwd = thư mục gói, nên runner phải làm y hệt — nếu không,
    #      test đỏ oan (đo được: `so_that_v2_doc_duoc_va_viscript_da_tach`).
    "$PY" - "$tmpd/build.json" >"$tmpd/bins" <<'PYEOF'
import json, os, sys
seen = set()
out = sys.stdout
for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    try:
        m = json.loads(line)
    except Exception:
        continue
    if m.get("reason") != "compiler-artifact":
        continue
    if not m.get("profile", {}).get("test"):
        continue
    e = m.get("executable")
    if not e or e in seen:
        continue
    seen.add(e)
    pkg = os.path.dirname(m.get("manifest_path") or "")
    out.write("%s\0%s\0" % (e, pkg))
PYEOF
    if [ ! -s "$tmpd/bins" ]; then
        echo "L0: không lấy được binary test nào từ cargo (build.json rỗng?)" >&2
        rm -rf "$tmpd"
        return 1
    fi

    # 3) Tách binary nhạy thời gian ra khỏi lô song song (hoặc tất cả nếu ép tuần tự).
    : >"$tmpd/serial"
    : >"$tmpd/par"
    while IFS= read -r -d '' b && IFS= read -r -d '' pkg; do
        n="$(basename "$b")"
        if [ "$serial_all" = "1" ]; then
            printf '%s\0%s\0' "$b" "$pkg" >>"$tmpd/serial"
            continue
        fi
        case " $serial_pat " in
            *" ${n%%-*} "*) printf '%s\0%s\0' "$b" "$pkg" >>"$tmpd/serial" ;;
            *) printf '%s\0%s\0' "$b" "$pkg" >>"$tmpd/par" ;;
        esac
    done <"$tmpd/bins"

    # 4) Runner một binary: cwd = thư mục gói (như cargo), log riêng, mã thoát riêng.
    #    Mã thoát được ghi ra tệp `.rc` để **không** bị pipeline/xargs che mất.
    cat >"$tmpd/run-one.sh" <<'SHEOF'
#!/bin/sh
d="$1"; b="$2"; pkg="$3"; n="$(basename "$b")"
if [ ! -x "$b" ]; then
    echo "L0: binary không chạy được: $b" >"$d/$n.log" 2>&1
    echo 126 >"$d/$n.rc"
    exit 0
fi
if [ -n "$pkg" ]; then
    cd "$pkg" || { echo "L0: không vào được thư mục gói: $pkg" >"$d/$n.log" 2>&1; echo 97 >"$d/$n.rc"; exit 0; }
fi
if [ -n "${V2_TEST_THREADS:-}" ]; then
    "$b" --test-threads="$V2_TEST_THREADS" >"$d/$n.log" 2>&1
else
    "$b" >"$d/$n.log" 2>&1
fi
echo $? >"$d/$n.rc"
SHEOF
    chmod +x "$tmpd/run-one.sh"

    # 5) Chạy: lô song song trước, rồi các binary nhạy thời gian chạy một mình.
    if [ -s "$tmpd/par" ]; then
        V2_TEST_THREADS="$threads" xargs -0 -P "$jobs" -n 2 sh "$tmpd/run-one.sh" "$tmpd" \
            <"$tmpd/par" || echo "# L0: cảnh báo — xargs trả mã khác 0 (kiểm từng .rc bên dưới)" >&2
    fi
    while IFS= read -r -d '' b && IFS= read -r -d '' pkg; do
        V2_TEST_THREADS="$threads" sh "$tmpd/run-one.sh" "$tmpd" "$b" "$pkg"
    done <"$tmpd/serial"

    # 6) Gộp kết quả: đỏ / không chạy được / bị signal ở **bất kỳ** binary nào ⇒ L0 đỏ.
    while IFS= read -r -d '' b && IFS= read -r -d '' pkg; do
        n="$(basename "$b")"
        nrun=$((nrun + 1))
        if [ ! -f "$tmpd/$n.rc" ]; then
            nfail=$((nfail + 1))
            echo "--- L0: ĐỎ $n (không có mã thoát — binary chưa chạy?)" >&2
            continue
        fi
        r="$(cat "$tmpd/$n.rc")"
        if [ "$r" -ne 0 ]; then
            nfail=$((nfail + 1))
            if [ "$r" -gt 128 ]; then
                echo "--- L0: ĐỎ $n (bị signal $((r - 128)))" >&2
            else
                echo "--- L0: ĐỎ $n (rc=$r)" >&2
            fi
            sed -n '1,200p' "$tmpd/$n.log" >&2
        fi
        # Đếm test **thực thi** (không chỉ đếm binary) để đối chiếu trước/sau.
        if [ -f "$tmpd/$n.log" ]; then
            ntest=$((ntest + $(grep -oE '[0-9]+ passed' "$tmpd/$n.log" 2>/dev/null | awk '{s+=$1} END {print s+0}')))
        fi
    done <"$tmpd/bins"
    t1="$(now_ms)"
    echo "# L0: $nrun binary, $ntest test đã chạy, $nfail đỏ, $((t1 - t0)) ms (jobs=$jobs, threads=$threads, serial_all=$serial_all)"
    [ "$nfail" -eq 0 ] || rc=1

    # 7) Doc-test: KHÔNG nằm trong binary test ⇒ phải chạy riêng, nếu không là mất phủ.
    if ! "$CARGO" test --manifest-path "$MANIFEST" --workspace --doc; then
        rc=1
    fi

    rm -rf "$tmpd"
    return "$rc"
}
