#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245) — **`v2 nightly`** (KT-333b, AGENTS.md §6): lập kế hoạch, chạy, trạng thái,
# tự kiểm. Nguyên văn bản cũ, giữ nguyên chú thích thiết kế bên dưới; chỉ tách khỏi `v2/bin/v2`.
# ---------------------------------------------------------------- nightly (KT-333b)
#
# **KT-333b (D2-21 + D2-229 §7)** — `v2 nightly` thật:
#   --all        L1-full hợp đồng K6 (100.000 seed × smp 1,2,4,8 = 400k) + Miri + đột biến + soak
#   --l1 <seed>  L1 đúng số seed (mức smoke 1.000 ⇒ smp 1; ≥ 10.000 ⇒ 1,2,4,8)
#   --only-miri  CHỈ Miri trên tập chọn (KT-17, D2-47(a))
#   --soak [--n N] [--runs K] [--profile <tên>] [--smoke]
#                soak ảnh sản phẩm 3 arch (riscv64 1000×3, aarch64 1000×3, x86_64 600×3 — N đầy
#                theo D2-239); `--n`/`--runs` ghi rõ, hạ N ⇒ smoke + cảnh báo không phải bằng chứng
#                **KT-347b**: `--profile` chọn bộ bước soak — `product-shell` (3 arch ảnh sản phẩm
#                có shell), `soak-x86_64-max` (`-cpu max`), `virtio-blk` (soak **đĩa thật** riscv64),
#                `all` (cả 5). Không nêu = 4 bước cũ. Tên lạ ⇒ **từ chối** (exit 2, in tên).
#                `--smoke` = lối tắt của `--n 1 --runs 1` (không phải bằng chứng nghiệm thu).
#   --status [--soak]  trạng thái bằng chứng gần nhất (CHỈ đọc; không chạy lại); `--soak` ⇒
#                đọc **bằng chứng soak gần nhất**
#   --dry-run    in kế hoạch (bước, lệnh, seed, ước lượng) mà KHÔNG chạy
#   --selftest   tự kiểm việc lập kế hoạch + đọc trạng thái (không chạy sim)
#
# Không có mặc định chạy: `v2 nightly` trần trơn là **lỗi dùng** (exit 2) — tránh vô tình chạy
# 4 tiếng. `--dry-run` trần trơn in kế hoạch của `--all` (vẫn không chạy).
# Mỗi lần chạy **thật** ghi manifest + log thô dưới `target-task/nightly/` (gitignored), sinh
# `v2/agent/reports/STATUS.md` + log tóm tắt dưới `v2/agent/JOURNAL/nightly/` (QD Q12: số đo do
# máy sinh; STATUS.md trích nguyên văn trong khối `scope=` — luật máy 1 / KT-80). Không retry
# (D2-07); bước đầu tiên đỏ ⇒ dừng, vẫn ghi bằng chứng.


NIGHTLY_STATUS_MD="$V2_ROOT/agent/reports/STATUS.md"
NIGHTLY_JOURNAL="$V2_ROOT/agent/JOURNAL/nightly"
# **KT-333c (D2-243 mục 3c)** — `--soak` chạy **cả ba** arch với **N đầy** (D2-239: riscv64 1000,
# aarch64 1000, x86_64 600) và `--runs` luồng (1..3). `--n N` là **ghi rõ** khi cần chạy smoke để
# chứng minh đường chạy; hạ N **không** âm thầm (kế hoạch in N thật + cảnh báo "không phải bằng
# chứng nghiệm thu"). Mặc định rỗng = N đầy **theo từng arch**.
NIGHTLY_SOAK_N=""
NIGHTLY_SOAK_RUNS=3
NIGHTLY_STATUS_SOAK=0
# **KT-347b** — profile soak (rỗng = 4 bước cũ) và lối tắt `--smoke` = `--n 1 --runs 1`.
NIGHTLY_PROFILE=""
NIGHTLY_SMOKE=0
NIGHTLY_SOAK_RUNS_SET=0

nightly_evid_root() { printf '%s' "${V2_NIGHTLY_EVID:-$REPO_ROOT/target-task/nightly}"; }

nightly_usage() {
    cat <<'EOF'
  ./v2/bin/v2 nightly --all
                               L1-full hợp đồng K6 (100.000 seed × smp 1,2,4,8 = 400k) + Miri
                               + đột biến + soak 3 arch. CHẠY HÀNG GIỜ — không tự chạy khi
                               thiếu cờ; xem trước bằng --dry-run.
  ./v2/bin/v2 nightly --l1 <seed> [--smp a,b,c]
                               L1 đúng số seed (D2-53 Q13): < 10.000 ⇒ smp 1 (smoke);
                               >= 10.000 ⇒ smp 1,2,4,8; >= 100.000 dùng l1-tier --tier full.
                               Tương thích cũ: --l1 --seeds N --smp a,b,c.
  ./v2/bin/v2 nightly --only-miri
                               CHỈ Miri trên tập chọn (KT-17, D2-47(a)); không L1/mutation/soak.
  ./v2/bin/v2 nightly --soak [--n N] [--runs K] [--profile <tên>] [--smoke]
                               soak ảnh sản phẩm **3 arch** với N **đầy** (riscv64 1000×3,
                               aarch64 1000×3, x86_64 600×3; D2-24/D2-149/D2-239) — chạy một
                               mình, không QEMU khác. `--n`/`--runs` ghi rõ N/luồng; N dưới mức
                               nghiệm thu ⇒ chạy `--smoke` + cảnh báo **không phải bằng chứng**.
                               `--profile` (KT-347b): product-shell | soak-x86_64-max |
                               virtio-blk | all — in rõ profile đã chọn; tên lạ bị từ chối.
                               `--smoke` = `--n 1 --runs 1` (lối tắt kiểm đường chạy).
  ./v2/bin/v2 nightly --status [--soak]
                               in trạng thái bằng chứng gần nhất (`STATUS.md` + manifest cục bộ);
                               `--soak` ⇒ đọc **bằng chứng soak gần nhất**. CHỈ đọc, không chạy lại.
  ./v2/bin/v2 nightly --dry-run [cờ phạm vi]
                               in kế hoạch (bước, lệnh, seed, ước lượng [ƯL]) mà KHÔNG chạy.
  ./v2/bin/v2 nightly --selftest
                               tự kiểm lập kế hoạch + đọc trạng thái (>= 12 ca âm/dương;
                               KHÔNG chạy sim).
EOF
}

# Kế hoạch: in TSV `<pha>\t<bước>\t<kiểu chạy>\t<ước lượng giây>` cho phạm vi đang chọn.
# Một nguồn duy nhất: `--dry-run` in từ đây, `nightly_run` chạy từ đây ⇒ kế hoạch không lệch.
nightly_plan_rows() {
    local nseeds="$NIGHTLY_SEEDS" nsmp="$NIGHTLY_SMP" l1cmd
    case "$NIGHTLY_MODE" in
        all)
            printf 'l1\tL1-full (hợp đồng K6: 100.000 seed × smp 1,2,4,8 = 400.000)\tl1-tier-full\t900\tbash v2/tools/l1-tier.sh --tier full --logdir <bằng-chứng>/l1\n'
            printf 'miri\tMiri (tập chọn KT-17/KT-23..KT-43, D2-47(a))\tmiri\t3600\tcargo +nightly miri test --manifest-path v2/Cargo.toml <tập chọn>\n'
            printf 'mutation\tĐột biến (selfbreak 5 luật K6, KT-80; `v2 mutants` §9.13 chưa có)\tmutation\t95\tbash v2/test/rules/selfbreak.sh\n'
            nightly_plan_soak_rows
            ;;
        soak)
            nightly_plan_soak_rows
            ;;
        only-miri)
            printf 'miri\tMiri (tập chọn KT-17/KT-23..KT-43, D2-47(a))\tmiri\t3600\tcargo +nightly miri test --manifest-path v2/Cargo.toml <tập chọn>\n'
            ;;
        l1)
            l1cmd="VIFAST_SEEDS=$nseeds VIFAST_SMP=$nsmp cargo test --manifest-path v2/Cargo.toml -p vifast-sim driver_smp -- --nocapture"
            if [ "$nseeds" -ge 100000 ]; then
                printf 'l1\tL1-full (hợp đồng K6: %s seed × smp %s = %s)\tl1-tier-full\t900\tbash v2/tools/l1-tier.sh --tier full --logdir <bằng-chứng>/l1\n' \
                    "$nseeds" "$nsmp" "$((nseeds * $(printf '%s' "$nsmp" | tr ',' '\n' | grep -c .)))"
            else
                printf 'l1\tL1 (seed=%s, smp=%s)\tl1-driver\t%s\t%s\n' \
                    "$nseeds" "$nsmp" "$((nseeds / 1000 * 7 + 30))" "$l1cmd"
            fi
            ;;
    esac
}

nightly_plan_scope() {
    local prof
    prof="$(nightly_soak_profile_label)"
    case "$NIGHTLY_MODE" in
        all) printf '%s' "--all (L1-full + Miri + đột biến + soak; N=$(nightly_soak_n soak-riscv64)/$(nightly_soak_n soak-aarch64)/$(nightly_soak_n soak-x86_64)/$(nightly_soak_n soak-x86_64-max); runs=$NIGHTLY_SOAK_RUNS; profile=$prof)" ;;
        soak) printf '%s' "--soak (profile=$prof; N=$(nightly_soak_n soak-riscv64)/$(nightly_soak_n soak-aarch64)/$(nightly_soak_n soak-x86_64)/$(nightly_soak_n soak-x86_64-max); runs=$NIGHTLY_SOAK_RUNS)" ;;
        only-miri) printf '%s' '--only-miri' ;;
        l1) printf '%s' "--l1 $NIGHTLY_SEEDS (smp $NIGHTLY_SMP)" ;;
    esac
}

nightly_print_plan() {
    local phase label kind est n=0 total=0 head dirty tmpplan prof
    tmpplan="$(mktemp "${TMPDIR:-/tmp}/v2nightlyplan.XXXXXX")" || return 1
    nightly_plan_rows >"$tmpplan"
    prof="$(nightly_soak_profile_label)"
    head="$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo '?')"
    dirty=0
    [ -n "$(git -C "$REPO_ROOT" status --porcelain 2>/dev/null)" ] && dirty=1
    echo "== v2 nightly --dry-run — KẾ HOẠCH (KHÔNG chạy) =="
    echo "   HEAD=$head dirty=$dirty"
    echo "   phạm vi: $(nightly_plan_scope)"
    case "$NIGHTLY_MODE" in
        all | soak)
            echo "   N: riscv64=$(nightly_soak_n soak-riscv64) aarch64=$(nightly_soak_n soak-aarch64) x86_64=$(nightly_soak_n soak-x86_64) x86_64-max=$(nightly_soak_n soak-x86_64-max); runs=$NIGHTLY_SOAK_RUNS (N đầy KT-348: 1000/1000/600/600 × 3)"
            echo "   profile soak: $prof — bước: $(nightly_soak_kinds | tr '\n' ' ')"
            if nightly_soak_is_smoke; then
                echo "   # CẢNH BÁO: N/luồng dưới mức nghiệm thu KT-348 — KHÔNG phải bằng chứng nghiệm thu"
            fi
            ;;
    esac
    echo "   #  pha       bước (và lệnh thật)                                           [ƯL]"
    while IFS="	" read -r phase label kind est cmd; do
        n=$((n + 1))
        total=$((total + est))
        printf '   %-2s %-9s %-58s %ss\n' "$n" "$phase" "$label" "$est"
        printf '        $ %s\n' "$cmd"
    done <"$tmpplan"
    echo "   tổng [ƯL]: $n bước ~ $((total / 60)) phút $((total % 60)) giây (máy rảnh; không phải cam kết)"
    echo "   không có mặc định chạy: phải nêu --all/--l1/--only-miri/--soak (AGENTS.md §6)."
    echo "   bước thật + log: $(nightly_evid_root)/<run-id>/; STATUS.md do máy sinh (QD Q12)."
    rm -f "$tmpplan"
    return 0
}

# Số seed của lô — không dùng ở đây; giữ chỗ cho l1-tier (đã tự chia lô).

nightly_manifest_key() { # <tệp> <khoá> <giá trị>
    printf '#key\t%s\t%s\n' "$2" "$3" >>"$1"
}

nightly_run() {
    local root run_id evid manifest tmpplan started ended result phase label kind est
    local n=0 rc t0 t1 wall log head dirty summary out
    local seeds="$NIGHTLY_SEEDS" smp="$NIGHTLY_SMP" soak_n="$NIGHTLY_SOAK_N"
    V2_PY="$(find_python || true)"
    if [ -z "$V2_PY" ]; then
        echo "v2 nightly: cần python ≥3.11 có tomllib (đã thử python3.12/3.11/3)" >&2
        return 2
    fi
    root="$(nightly_evid_root)"
    mkdir -p "$root" || return 1
    head="$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo '?')"
    dirty=0
    [ -n "$(git -C "$REPO_ROOT" status --porcelain 2>/dev/null)" ] && dirty=1
    run_id="nightly-$(date -u +%Y%m%d-%H%M%S)-${head}"
    evid="$root/$run_id"
    mkdir -p "$evid" || return 1
    manifest="$evid/manifest.tsv"
    tmpplan="$evid/plan.tsv"
    nightly_plan_rows >"$tmpplan" || return 1
    started="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    : >"$manifest"
    nightly_manifest_key "$manifest" run_id "$run_id"
    nightly_manifest_key "$manifest" head "$head"
    nightly_manifest_key "$manifest" dirty "$dirty"
    nightly_manifest_key "$manifest" flags "$(nightly_plan_scope)"
    case "$NIGHTLY_MODE" in
        soak | all)
            nightly_manifest_key "$manifest" soak_n_riscv64 "$(nightly_soak_n soak-riscv64)"
            nightly_manifest_key "$manifest" soak_n_aarch64 "$(nightly_soak_n soak-aarch64)"
            nightly_manifest_key "$manifest" soak_n_x86_64 "$(nightly_soak_n soak-x86_64)"
            nightly_manifest_key "$manifest" soak_n_x86_64_max "$(nightly_soak_n soak-x86_64-max)"
            nightly_manifest_key "$manifest" soak_n_riscv64_product_shell "$(nightly_soak_n soak-riscv64-product-shell)"
            nightly_manifest_key "$manifest" soak_n_riscv64_virtio_blk "$(nightly_soak_n soak-riscv64-virtio-blk)"
            nightly_manifest_key "$manifest" soak_runs "$NIGHTLY_SOAK_RUNS"
            nightly_manifest_key "$manifest" soak_profile "$(nightly_soak_profile_label)"
            nightly_manifest_key "$manifest" soak_kinds "$(nightly_soak_kinds | tr '\n' ',')"
            nightly_manifest_key "$manifest" soak_full "$(nightly_soak_is_full && echo 1 || echo 0)"
            ;;
    esac
    nightly_manifest_key "$manifest" started "$started"
    echo "== v2 nightly: phạm vi $(nightly_plan_scope) =="
    echo "   HEAD=$head dirty=$dirty; bằng chứng: $evid"
    case "$NIGHTLY_MODE" in
        soak | all)
            # **KT-347b** — in **profile đã chọn** + đúng bộ bước sẽ chạy (không để người đọc đoán).
            echo "   profile soak: $(nightly_soak_profile_label) — bước: $(nightly_soak_kinds | tr '\n' ' ')"
            ;;
    esac
    result=PASS
    while IFS="	" read -r phase label kind est cmd; do
        n=$((n + 1))
        log="$evid/step-$n-$phase.log"
        echo "--- nightly $n: $label"
        t0="$(date +%s)"
        nightly_step_exec "$kind" "$evid" "$seeds" "$smp" "$soak_n" "$NIGHTLY_SOAK_RUNS" >"$log" 2>&1
        rc=$?
        t1="$(date +%s)"
        wall=$((t1 - t0))
        printf '#step\t%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$n" "$phase" "$label" "$rc" "$((wall * 1000))" "$(basename "$log")" >>"$manifest"
        if [ "$rc" -eq 0 ]; then
            echo "--- nightly $n: PASS (${wall}s; log $log)"
        else
            echo "--- nightly $n: FAIL (rc=$rc, ${wall}s; log $log)" >&2
            echo "--- nightly: 40 dòng cuối của log:" >&2
            tail -40 "$log" >&2 2>/dev/null || true
            result=FAIL
            break
        fi
    done <"$tmpplan"
    ended="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    nightly_manifest_key "$manifest" ended "$ended"
    nightly_manifest_key "$manifest" result "$result"
    summary="$NIGHTLY_JOURNAL/$run_id/summary.log"
    if ! "$V2_PY" "$V2_ROOT/tools/nightly-report.py" render --manifest "$manifest" \
            --summary "$summary" --out "$NIGHTLY_STATUS_MD" --root "$REPO_ROOT"; then
        echo "v2 nightly: không sinh được STATUS.md (manifest/log hỏng)" >&2
        return 2
    fi
    mkdir -p "$NIGHTLY_JOURNAL/$run_id"
    cp "$manifest" "$NIGHTLY_JOURNAL/$run_id/manifest.tsv" 2>/dev/null || true
    rm -f "$tmpplan"
    if [ "$result" = "PASS" ]; then
        echo "v2 nightly: PASS ($n bước; bằng chứng $evid; STATUS.md đã sinh)"
        return 0
    fi
    echo "v2 nightly: FAIL (bước $n đỏ — không retry, D2-07; bằng chứng $evid)" >&2
    return 1
}

nightly_status() {
    local V2_PY latest manifest d
    V2_PY="$(find_python || true)"
    if [ -z "$V2_PY" ]; then
        echo "v2 nightly --status: cần python ≥3.11 có tomllib" >&2
        return 2
    fi
    # **KT-333c (D2-243 mục 3c)** — `--soak --status`: đọc **bằng chứng soak gần nhất** (manifest
    # có bước phase `soak`, kể cả lượt `--all` có soak). Không chạy lại gì (chỉ đọc).
    if [ "$NIGHTLY_STATUS_SOAK" -eq 1 ]; then
        latest=""
        for d in $(ls -1d "$(nightly_evid_root)"/nightly-* 2>/dev/null | sort); do
            [ -f "$d/manifest.tsv" ] || continue
            if awk -F'\t' '$1 == "#step" && $3 == "soak" { f = 1 } END { exit(f ? 0 : 1) }' \
                "$d/manifest.tsv" 2>/dev/null; then
                latest="$d"
            fi
        done
        if [ -z "$latest" ]; then
            echo "v2 nightly --soak --status: chưa có bằng chứng soak dưới $(nightly_evid_root)"
            echo "# N đầy KT-348 (D2-239): riscv64=1000 aarch64=1000 x86_64=600; runs=3"
            echo "# chạy thật: ./v2/bin/v2 nightly --soak   (smoke: --soak --n N --runs K)"
            return 0
        fi
        echo "# bằng chứng soak gần nhất: $latest/manifest.tsv"
        "$V2_PY" "$V2_ROOT/tools/nightly-report.py" show --status "$NIGHTLY_STATUS_MD" \
            --manifest "$latest/manifest.tsv"
        awk -F'\t' '$1 == "#key" && ($2 ~ /^soak_/ || $2 == "result" || $2 == "ended") {
                printf "#   %s=%s\n", $2, $3
            }' "$latest/manifest.tsv" 2>/dev/null
        return 0
    fi
    latest="$(ls -1d "$(nightly_evid_root)"/nightly-* 2>/dev/null | sort | tail -1)"
    manifest=""
    [ -n "$latest" ] && [ -f "$latest/manifest.tsv" ] && manifest="$latest/manifest.tsv"
    if [ -n "$manifest" ]; then
        "$V2_PY" "$V2_ROOT/tools/nightly-report.py" show --status "$NIGHTLY_STATUS_MD" \
            --manifest "$manifest"
    else
        "$V2_PY" "$V2_ROOT/tools/nightly-report.py" show --status "$NIGHTLY_STATUS_MD"
    fi
}

# >= 26 ca: 22 ca lập kế hoạch (gồm 5 ca soak của KT-333c + 9 ca `--profile`/`--smoke` của
# KT-347b) + 4 ca đọc trạng thái/sinh STATUS (2 dương + 2 âm).
nightly_selftest() {
    local V2_PY tmp ca=0 fails=0 out rc run manifest summary a b
    V2_PY="$(find_python || true)"
    if [ -z "$V2_PY" ]; then
        echo "v2 nightly --selftest: cần python ≥3.11 có tomllib (đã thử python3.12/3.11/3)" >&2
        return 2
    fi
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/v2nightly-st.XXXXXX")" || return 2
    _kt_ok() { ca=$((ca + 1)); echo "ok $ca - $1"; }
    _kt_bad() { ca=$((ca + 1)); fails=$((fails + 1)); echo "not ok $ca - $1"; }
    _kt_plan() { # <tên> <mong|-> <có|không> <mẫu> <tham số...>
        local name="$1" want_rc="$2" want_grep="$3" pat="$4"
        shift 4
        out="$(V2_NIGHTLY_EVID="$tmp/evid-$ca" bash "$V2_ROOT/bin/v2" nightly "$@" 2>&1)"
        rc=$?
        [ "$rc" -eq "$want_rc" ] || { _kt_bad "$name (rc=$rc, muốn $want_rc)"; return; }
        if [ "$want_grep" = "có" ]; then
            printf '%s' "$out" | grep -Fq -- "$pat" || { _kt_bad "$name (thiếu '$pat')"; return; }
        fi
        _kt_ok "$name"
    }
    _kt_plan "kế hoạch --all: 7 bước, đủ L1-full + Miri + đột biến + soak 3 arch + x64-max" 0 "có" \
        "soak.sh 1000 3" --all --dry-run
    _kt_plan "kế hoạch --l1 1000: driver smp 1" 0 "có" "VIFAST_SEEDS=1000" --l1 1000 --dry-run
    _kt_plan "kế hoạch --l1 100000: l1-tier --tier full (hợp đồng K6)" 0 "có" \
        "l1-tier.sh --tier full" --l1 100000 --dry-run
    _kt_plan "âm: --l1 0 bị từ chối (exit 2)" 2 "-" "" --l1 0 --dry-run
    _kt_plan "âm: --l1 abc bị từ chối (exit 2)" 2 "-" "" --l1 abc --dry-run
    _kt_plan "âm: cờ lạ bị từ chối (exit 2)" 2 "-" "" --bogus
    _kt_plan "dương: --dry-run trần = kế hoạch --all (vẫn không chạy)" 0 "có" \
        "mutation" --dry-run
    # **KT-333c (D2-243 mục 3c)** — >= 3 ca cho phần soak: N đầy đúng arch, N ghi rõ + cảnh báo
    # khi hạ, từ chối N/runs lạ, và đọc bằng chứng soak gần nhất.
    _kt_plan "kế hoạch --soak: N đầy 3 arch (riscv64=1000, aarch64=1000, x86_64=600 × 3 luồng)" 0 "có" \
        "soak-aarch64.sh 1000 3" --soak --dry-run
    # **KT-333d (D2-248 mục 2f)** — `-cpu max` là kịch bản riêng của nightly (KHÔNG vào T2),
    # in rõ env `V2_X64_CPU=max` + N đầy 600 × 3 (KT-323 đã có biến này trong soak-x86_64.sh).
    _kt_plan "kế hoạch --soak: thêm soak-x86_64-max (V2_X64_CPU=max, 600 × 3)" 0 "có" \
        "V2_X64_CPU=max bash v2/test/boot/soak-x86_64.sh 600 3" --soak --dry-run
    _kt_plan "kế hoạch --soak --n 5 --runs 1: N ghi rõ + cảnh báo KHÔNG phải bằng chứng" 0 "có" \
        "KHÔNG phải bằng chứng nghiệm thu" --soak --n 5 --runs 1 --dry-run
    _kt_plan "âm: --soak --n abc bị từ chối (exit 2)" 2 "-" "" --soak --n abc --dry-run
    _kt_plan "âm: --soak --runs 0 bị từ chối (exit 2)" 2 "-" "" --soak --runs 0 --dry-run
    _kt_plan "dương: --soak --status khi chưa có bằng chứng ⇒ exit 0 + nói rõ" 0 "có" \
        "chưa có bằng chứng soak" --soak --status
    # **KT-347b** — `--profile` (product-shell | soak-x86_64-max | virtio-blk | all) + `--smoke`.
    _kt_plan "KT-347b: --profile product-shell = 3 arch ảnh sản phẩm có shell (riscv64 --features)" 0 "có" \
        "bash v2/test/boot/soak.sh --features product-shell 1000 3" --soak --profile product-shell --dry-run
    _kt_plan "KT-347b: --profile virtio-blk = soak đĩa thật (--features product-shell,product-fs --disk)" 0 "có" \
        "--features product-shell,product-fs --disk 1000 3" --soak --profile virtio-blk --dry-run
    _kt_plan "KT-347b: --profile soak-x86_64-max = đúng nhánh -cpu max" 0 "có" \
        "V2_X64_CPU=max bash v2/test/boot/soak-x86_64.sh 600 3" --soak --profile soak-x86_64-max --dry-run
    _kt_plan "KT-347b: --profile all = gộp 5 bước (có đĩa thật + product-shell)" 0 "có" \
        "soak-riscv64-virtio-blk" --soak --profile all --dry-run
    _kt_plan "KT-347b: --smoke = --n 1 --runs 1 (in cú pháp smoke thật)" 0 "có" \
        "--features product-shell --smoke 1 1" --soak --profile product-shell --smoke --dry-run
    _kt_plan "âm: --profile tên lạ bị từ chối (exit 2, in tên)" 2 "có" \
        "tên lạ 'khong-co'" --soak --profile khong-co --dry-run
    _kt_plan "âm: --profile chỉ dùng với --soak/--all" 2 "có" \
        "chỉ dùng với --soak" --l1 1000 --profile product-shell --dry-run
    _kt_plan "âm: --smoke chỉ dùng với --soak/--all" 2 "có" \
        "--smoke chỉ dùng với --soak" --l1 1000 --smoke --dry-run
    _kt_plan "âm: --smoke không đi với --n (hai nguồn N)" 2 "có" \
        "không đi với --n/--runs" --soak --smoke --n 5 --dry-run
    # profile `soak-x86_64-max` **không** được lặng lẽ kéo 3 arch khác (fail-closed của bộ chọn).
    out="$("$V2_ROOT/bin/v2" nightly --soak --profile soak-x86_64-max --dry-run 2>&1)"
    if printf '%s' "$out" | grep -Fq "soak-x86_64.sh 600 3" &&
        ! printf '%s' "$out" | grep -Fq "soak-aarch64.sh 1000 3"; then
        _kt_ok "KT-347b: profile soak-x86_64-max chỉ chạy đúng 1 bước (không kéo arch khác)"
    else
        _kt_bad "KT-347b: profile soak-x86_64-max chạy nhầm bước khác"
    fi
    # không được tạo bằng chứng khi chỉ --dry-run
    if ls -1d "$tmp"/evid-* >/dev/null 2>&1; then
        _kt_bad "dương: --dry-run không tạo thư mục bằng chứng"
    else
        _kt_ok "dương: --dry-run không tạo thư mục bằng chứng"
    fi

    # --- đọc/sinh trạng thái trên repo giả (không chạm repo thật) -------------------------
    run="run-selftest"
    mkdir -p "$tmp/repo/v2/agent/JOURNAL/nightly/$run"
    manifest="$tmp/manifest.tsv"
    printf '#key\trun_id\t%s\n#key\thead\tabc1234\n#key\tdirty\t0\n#key\tflags\t--l1 1000\n#key\tstarted\t2026-11-22T05:00:00Z\n#key\tended\t2026-11-22T05:01:00Z\n#key\tresult\tPASS\n#step\t1\tl1\tL1 (1000 seed × smp 1)\t0\t61000\tstep-1-l1.log\n' "$run" >"$manifest"
    summary="$tmp/repo/v2/agent/JOURNAL/nightly/$run/summary.log"
    "$V2_PY" "$V2_ROOT/tools/nightly-report.py" render --manifest "$manifest" --summary "$summary" \
        --out "$tmp/repo/STATUS.md" --root "$tmp/repo" >/dev/null 2>&1
    a="$tmp/a.md"; b="$tmp/b.md"
    cp "$tmp/repo/STATUS.md" "$a"
    "$V2_PY" "$V2_ROOT/tools/nightly-report.py" render --manifest "$manifest" --summary "$summary" \
        --out "$b" --root "$tmp/repo" >/dev/null 2>&1
    if cmp -s "$a" "$b"; then
        _kt_ok "dương: STATUS.md tất định (2 lần render cùng byte)"
    else
        _kt_bad "dương: STATUS.md tất định (2 lần render cùng byte)"
    fi
    if "$V2_PY" "$V2_ROOT/tools/nightly-report.py" verify --status "$a" --root "$tmp/repo" >/dev/null 2>&1; then
        _kt_ok "dương: khối scope= trích nguyên văn + prose không có số đo"
    else
        _kt_bad "dương: khối scope= trích nguyên văn + prose không có số đo"
    fi
    out="$(V2_PY="$V2_PY" "$V2_PY" "$V2_ROOT/tools/nightly-report.py" show --status "$a" 2>&1)"; rc=$?
    if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q "kết quả: PASS"; then
        _kt_ok "dương: --status đọc PASS, không chạy lại"
    else
        _kt_bad "dương: --status đọc PASS, không chạy lại (rc=$rc)"
    fi
    # ca âm 1: sửa 1 dòng trong log tóm tắt ⇒ khối scope= hết nguyên văn
    sed 's/^# result=PASS$/# result=RED/' "$summary" >"$summary.sua" && mv "$summary.sua" "$summary"
    "$V2_PY" "$V2_ROOT/tools/nightly-report.py" verify --status "$a" --root "$tmp/repo" \
        >"$tmp/verify.log" 2>&1
    rc=$?
    if [ "$rc" -eq 1 ] && grep -q "STATUS_NOT_VERBATIM" "$tmp/verify.log"; then
        _kt_ok "âm: log tóm tắt lệch ⇒ STATUS_NOT_VERBATIM"
    else
        _kt_bad "âm: log tóm tắt lệch ⇒ STATUS_NOT_VERBATIM (rc=$rc)"
    fi
    # ca âm 2: STATUS.md mất khối scope= ⇒ STATUS_MALFORMED
    printf '# Trạng thái nightly\n- kết quả: PASS\n' >"$tmp/broken.md"
    "$V2_PY" "$V2_ROOT/tools/nightly-report.py" show --status "$tmp/broken.md" >"$tmp/broken.log" 2>&1
    rc=$?
    if [ "$rc" -eq 1 ] && grep -q "STATUS_MALFORMED" "$tmp/broken.log"; then
        _kt_ok "âm: STATUS.md hỏng ⇒ STATUS_MALFORMED"
    else
        _kt_bad "âm: STATUS.md hỏng ⇒ STATUS_MALFORMED (rc=$rc)"
    fi
    # dương: chưa có bằng chứng ⇒ đọc được, exit 0, nói rõ "chưa có"
    out="$("$V2_PY" "$V2_ROOT/tools/nightly-report.py" show --status "$tmp/khong-co.md" 2>&1)"; rc=$?
    if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q "chưa có bằng chứng"; then
        _kt_ok "dương: chưa có bằng chứng ⇒ exit 0 + nói rõ"
    else
        _kt_bad "dương: chưa có bằng chứng ⇒ exit 0 + nói rõ (rc=$rc)"
    fi

    rm -rf "$tmp"
    echo
    if [ "$fails" -gt 0 ]; then
        echo "v2 nightly --selftest: FAIL ($fails/$ca)" >&2
        return 1
    fi
    echo "v2 nightly --selftest: PASS ($ca/$ca)"
    return 0
}

nightly_parse() {
    NIGHTLY_MODE=""
    NIGHTLY_SEEDS=""
    NIGHTLY_SMP=""
    NIGHTLY_DRY=0
    NIGHTLY_SOAK_N=""
    NIGHTLY_SOAK_RUNS=3
    NIGHTLY_STATUS_SOAK=0
    NIGHTLY_PROFILE=""
    NIGHTLY_SMOKE=0
    NIGHTLY_SOAK_RUNS_SET=0
    local legacy_seeds="" legacy_smp="" next
    while [ $# -gt 0 ]; do
        case "$1" in
            --all)
                [ -z "$NIGHTLY_MODE" ] || { echo "v2 nightly: chỉ một phạm vi (đã có --$NIGHTLY_MODE)" >&2; return 2; }
                NIGHTLY_MODE=all
                shift
                ;;
            --l1)
                [ -z "$NIGHTLY_MODE" ] || { echo "v2 nightly: chỉ một phạm vi" >&2; return 2; }
                NIGHTLY_MODE=l1
                shift
                if [ $# -gt 0 ]; then
                    case "$1" in
                        ''|*[!0-9]*) ;; # không phải số ⇒ để --seeds/--smp hoặc mặc định
                        *) NIGHTLY_SEEDS="$1"; shift ;;
                    esac
                fi
                ;;
            --seeds)
                [ $# -ge 2 ] || { echo "v2 nightly: --seeds cần giá trị" >&2; return 2; }
                legacy_seeds="$2"
                shift 2
                ;;
            --smp)
                [ $# -ge 2 ] || { echo "v2 nightly: --smp cần giá trị" >&2; return 2; }
                legacy_smp="$2"
                shift 2
                ;;
            --only-miri)
                [ -z "$NIGHTLY_MODE" ] || { echo "v2 nightly: chỉ một phạm vi" >&2; return 2; }
                NIGHTLY_MODE=only-miri
                shift
                ;;
            --soak)
                # **KT-333c** — `--status --soak` = đọc bằng chứng soak gần nhất (D2-243 mục 3c).
                if [ -z "$NIGHTLY_MODE" ]; then
                    NIGHTLY_MODE=soak
                elif [ "$NIGHTLY_MODE" = "status" ] && [ "$NIGHTLY_STATUS_SOAK" -eq 0 ]; then
                    NIGHTLY_STATUS_SOAK=1
                else
                    echo "v2 nightly: chỉ một phạm vi" >&2
                    return 2
                fi
                shift
                ;;
            --status)
                if [ -z "$NIGHTLY_MODE" ]; then
                    NIGHTLY_MODE=status
                elif [ "$NIGHTLY_MODE" = "soak" ] && [ "$NIGHTLY_STATUS_SOAK" -eq 0 ]; then
                    NIGHTLY_MODE=status
                    NIGHTLY_STATUS_SOAK=1
                elif [ "$NIGHTLY_MODE" = "status" ] && [ "$NIGHTLY_STATUS_SOAK" -eq 0 ]; then
                    NIGHTLY_STATUS_SOAK=1
                else
                    echo "v2 nightly: chỉ một phạm vi" >&2
                    return 2
                fi
                shift
                ;;
            --n)
                [ $# -ge 2 ] || { echo "v2 nightly: --n cần N (số boot mỗi arch)" >&2; return 2; }
                NIGHTLY_SOAK_N="$2"
                shift 2
                ;;
            --runs)
                [ $# -ge 2 ] || { echo "v2 nightly: --runs cần số luồng (1..3)" >&2; return 2; }
                NIGHTLY_SOAK_RUNS="$2"
                NIGHTLY_SOAK_RUNS_SET=1
                shift 2
                ;;
            --profile)
                # **KT-347b** — profile soak (product-shell | soak-x86_64-max | virtio-blk | all).
                [ $# -ge 2 ] || { echo "v2 nightly --profile: cần tên (product-shell|soak-x86_64-max|virtio-blk|all)" >&2; return 2; }
                NIGHTLY_PROFILE="$2"
                shift 2
                ;;
            --smoke)
                # **KT-347b** — lối tắt `--soak --n 1 --runs 1` để kiểm đường chạy nhanh.
                NIGHTLY_SMOKE=1
                shift
                ;;
            --selftest)
                [ -z "$NIGHTLY_MODE" ] || { echo "v2 nightly: chỉ một phạm vi" >&2; return 2; }
                NIGHTLY_MODE=selftest
                shift
                ;;
            --dry-run)
                NIGHTLY_DRY=1
                shift
                ;;
            -h|--help)
                nightly_usage
                return 3
                ;;
            *)
                echo "v2 nightly: tham số lạ '$1'" >&2
                nightly_usage >&2
                return 2
                ;;
        esac
    done
    if [ "$NIGHTLY_MODE" = "l1" ]; then
        [ -n "$NIGHTLY_SEEDS" ] || NIGHTLY_SEEDS="$legacy_seeds"
        [ -n "$NIGHTLY_SEEDS" ] || NIGHTLY_SEEDS=10000
        case "$NIGHTLY_SEEDS" in
            ''|*[!0-9]*) echo "v2 nightly --l1: seed phải là số dương (nhận '$NIGHTLY_SEEDS')" >&2; return 2 ;;
        esac
        [ "$NIGHTLY_SEEDS" -gt 0 ] || { echo "v2 nightly --l1: seed phải dương" >&2; return 2; }
        if [ -n "$legacy_smp" ]; then
            NIGHTLY_SMP="$legacy_smp"
        elif [ "$NIGHTLY_SEEDS" -ge 10000 ]; then
            NIGHTLY_SMP="1,2,4,8"
        else
            NIGHTLY_SMP="1"
        fi
        case "$NIGHTLY_SMP" in
            ''|*[!0-9,]*) echo "v2 nightly --l1: --smp phải là danh sách số cách nhau dấu phẩy" >&2; return 2 ;;
        esac
    else
        [ -z "$legacy_seeds" ] || { echo "v2 nightly: --seeds chỉ dùng với --l1" >&2; return 2; }
        [ -z "$legacy_smp" ] || { echo "v2 nightly: --smp chỉ dùng với --l1" >&2; return 2; }
    fi
    # **KT-333c** — `--n`/`--runs` chỉ có nghĩa với `--soak` (hoặc `--all`); không âm thầm bỏ qua.
    if [ -n "$NIGHTLY_SOAK_N" ]; then
        case "$NIGHTLY_MODE" in
            soak | all) ;;
            *) echo "v2 nightly: --n chỉ dùng với --soak (hoặc --all)" >&2; return 2 ;;
        esac
        case "$NIGHTLY_SOAK_N" in
            '' | *[!0-9]*) echo "v2 nightly --soak: N phải là số dương (nhận '$NIGHTLY_SOAK_N')" >&2; return 2 ;;
        esac
        [ "$NIGHTLY_SOAK_N" -gt 0 ] || { echo "v2 nightly --soak: N phải > 0" >&2; return 2; }
    fi
    case "$NIGHTLY_SOAK_RUNS" in
        '' | *[!0-9]*) echo "v2 nightly --soak: --runs phải là số (nhận '$NIGHTLY_SOAK_RUNS')" >&2; return 2 ;;
    esac
    [ "$NIGHTLY_SOAK_RUNS" -ge 1 ] && [ "$NIGHTLY_SOAK_RUNS" -le 3 ] ||
        { echo "v2 nightly --soak: --runs=$NIGHTLY_SOAK_RUNS ngoài [1,3] (AGENTS §9: tối đa 3 QEMU)" >&2; return 2; }
    if [ -z "$NIGHTLY_MODE" ]; then
        if [ "$NIGHTLY_DRY" -eq 1 ]; then
            NIGHTLY_MODE=all
            NIGHTLY_SEEDS=""
            NIGHTLY_SMP=""
        else
            echo "v2 nightly: phải nêu rõ phạm vi — --all | --l1 <seed> | --only-miri | --soak | --status | --dry-run | --selftest" >&2
            echo "  (không có mặc định chạy — tránh vô tình chạy nightly 4 tiếng; AGENTS.md §6)" >&2
            nightly_usage >&2
            return 2
        fi
    fi
    # **KT-347b** — `--profile` fail-closed: chỉ 4 tên; chỉ dùng với `--soak`/`--all`; in tên lạ.
    if [ -n "$NIGHTLY_PROFILE" ]; then
        case "$NIGHTLY_PROFILE" in
            product-shell | soak-x86_64-max | virtio-blk | all) ;;
            *)
                echo "v2 nightly --profile: tên lạ '$NIGHTLY_PROFILE' (hợp lệ: product-shell|soak-x86_64-max|virtio-blk|all)" >&2
                return 2
                ;;
        esac
        case "$NIGHTLY_MODE" in
            soak | all) ;;
            *) echo "v2 nightly: --profile chỉ dùng với --soak (hoặc --all) — nhận '$NIGHTLY_PROFILE' với --$NIGHTLY_MODE" >&2; return 2 ;;
        esac
    fi
    # **KT-347b** — `--smoke` = `--n 1 --runs 1`; không đi với `--n`/`--runs` (tránh hai nguồn N).
    if [ "$NIGHTLY_SMOKE" -eq 1 ]; then
        case "$NIGHTLY_MODE" in
            soak | all) ;;
            *) echo "v2 nightly: --smoke chỉ dùng với --soak (hoặc --all)" >&2; return 2 ;;
        esac
        if [ -n "$NIGHTLY_SOAK_N" ] || [ "$NIGHTLY_SOAK_RUNS_SET" -eq 1 ]; then
            echo "v2 nightly: --smoke không đi với --n/--runs (đã có N='$NIGHTLY_SOAK_N' runs=$NIGHTLY_SOAK_RUNS)" >&2
            return 2
        fi
        NIGHTLY_SOAK_N=1
        NIGHTLY_SOAK_RUNS=1
    fi
    if [ "$NIGHTLY_MODE" = "status" ] && [ "$NIGHTLY_DRY" -eq 1 ]; then
        echo "v2 nightly: --status chỉ đọc, không đi với --dry-run" >&2
        return 2
    fi
    if [ "$NIGHTLY_MODE" = "selftest" ] && [ "$NIGHTLY_DRY" -eq 1 ]; then
        echo "v2 nightly: --selftest không đi với --dry-run" >&2
        return 2
    fi
    return 0
}

do_nightly() {
    nightly_parse "$@" || {
        case $? in
            3) return 0 ;;
            *) return 2 ;;
        esac
    }
    case "$NIGHTLY_MODE" in
        status) nightly_status ;;
        selftest) nightly_selftest ;;
        *)
            if [ "$NIGHTLY_DRY" -eq 1 ]; then
                nightly_print_plan
            else
                # **KT-333c (D2-243 mục 3a)** — nightly là phép đo hàng giờ (L1-full/Miri/soak):
                # **fail-closed** nếu máy đo đang bận. Khoá giữ tới khi tiến trình thoát (trap EXIT);
                # tiến trình con (`soak*.sh`) được uỷ quyền qua env, không xin khoá lần hai.
                measure_lock_enter "nightly $(nightly_plan_scope)" || return 2
                export V2_MEASURE_LOCK_DELEGATED_BY="$$"
                nightly_run
            fi
            ;;
    esac
}
