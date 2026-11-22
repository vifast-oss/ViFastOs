#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245) — **làn song song** của cổng v2 (nguyên văn KT-242 P3 + KT-333a/KT-333c):
# `lane_step`/`lane_run`/`lane_fails`/`lane_failed`/`lane_count`, ghi `.rc`/`.ms`/`.skip`,
# `lane_report`, tổng kết `gate_summary`. Trùng id `lane_step` ⇒ cổng đỏ ngay (D2-235(5)).
# ---------------------------------------------------------------- làn song song (KT-242 P3, KT-333a)
#
# **Vì sao**: 69 bước chạy **tuần tự** trên máy 12 nhân. Đo KT-110 (`real 407 s, user 277 s`)
# cho thấy cổng chỉ dùng ~0,7 nhân; phần lớn bước là **checker tĩnh chỉ đọc** (không ghi
# `target/`, không sửa cây, không QEMU) nên chạy song song được mà **không đổi phép kiểm nào**.
#
# **Mô hình**: mỗi bước có **số thứ tự cố định** (đúng thứ tự cũ) và một **làn**:
#   main  — chạy tuần tự, song song với các làn nền; **chỉ** bước ghi `target/` hoặc bước **đo**.
#   A..F  — chạy **nền**, song song với `main` và với nhau; **chỉ** chứa bước chỉ đọc. Số làn
#           (6) chọn theo thời gian đo để cân tải, không phải theo số bước (KT-333a).
#   sau*  — pha **đuôi**: bắt đầu sau khi mọi làn nền xong. Nhiều làn đuôi (`sau`, `sau2`, …)
#           chạy **song song với nhau**, mỗi làn tuần tự, tài nguyên rời nhau (QEMU/persist/
#           git-guard/sửa cây/selfbreak — xem chú thích từng bước ở `do_gate_fast`).
# Log từng bước ghi ra tệp riêng rồi **phát lại theo số thứ tự** ⇒ log hai lần chạy so sánh
# được từng dòng dù thứ tự hoàn thành khác nhau.
#
# **Bất biến giữ nguyên**: cùng tập bước (**75** bước T2 sau khi KT-333e gộp 20+22,
# chốt bằng hằng số), cùng ngưỡng, cùng
# số ca; T1 vẫn bỏ đúng 14 bước harness (`V2_T2_ONLY_STEPS`, in rõ `BỎ QUA`); mã thoát của cổng
# = có bước nào đỏ hay không. **Trùng id `lane_step` ⇒ cổng đỏ ngay** (D2-235(5)).

LANE_DIR=""
LANE_T0=0
LANE_STEPS=""
LANE_BG=""
LANE_SERIAL=""
LANE_PAR=""
LANE_IDS=""
LANE_DUP=""
LANE_PHASE_END=""
LANE_PIDS=""

lane_reset() {
    LANE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/v2lanes.XXXXXX")" || return 1
    # **KT-242 (P3)** — thư mục tạm **riêng cho lần chạy cổng**: script con dùng nó thay cho
    # đường `/tmp/<cố định>` (xem `JOURNAL/kt242-lanes-inventory.md` §2), nên hai cổng ở hai
    # worktree — hoặc hai bước song song — không ghi đè lên nhau. Chạy script **rời** (không qua
    # cổng) vẫn dùng `${TMPDIR:-/tmp}` như cũ ⇒ không đổi hành vi cũ.
    if [ -z "${V2_RUN_TMP:-}" ]; then
        V2_RUN_TMP="$(mktemp -d "${TMPDIR:-/tmp}/v2run.XXXXXX")" || return 1
        export V2_RUN_TMP
    fi
    echo "# run-tmp: $V2_RUN_TMP"
    LANE_T0="$(now_ms)"
    LANE_STEPS=""
    LANE_BG=""
    LANE_SERIAL=""
    LANE_PAR=""
    LANE_IDS=""
    LANE_DUP=""
    LANE_PHASE_END=""
    LANE_PIDS=""
    return 0
}

lane_count() { # số bước đã đăng ký
    printf '%s' "$LANE_STEPS" | awk 'END { print NR }'
}

lane_step() { # <làn> <số thứ tự> <tên> <hàm>
    local lane="$1" id="$2" name="$3" fn="$4"
    # **KT-333a (D2-235(5))** — id là **khoá duy nhất** của bước (tên tệp log/rc/ms = `<id>`).
    # Trùng id ⇒ `lane_done` thấy `.rc` của bước trước và **bỏ qua im lặng** bước sau. Ghi nhận
    # ngay tại đăng ký; `lane_run` sẽ **fail-closed** trước khi chạy bất kỳ bước nào.
    case " $LANE_IDS " in
        *" $id "*) LANE_DUP="${LANE_DUP:+$LANE_DUP }$id" ;;
        *) LANE_IDS="$LANE_IDS $id" ;;
    esac
    LANE_STEPS="${LANE_STEPS}${id}	${lane}	${name}	${fn}
"
    case "$lane" in
        main) ;;
        sau)
            # **KT-333a** — làn `sau` = pha đuôi **tuần tự, chạy một mình** (shell chính) trước
            # các làn đuôi song song. Chỉ dành cho bước **nhạy tải**: `persist --red-selftest` và
            # `persist --crash` (cắt điện giữa lúc ghi — D2-151: host chậm ⇒ kill rơi sau pha 7 ⇒
            # `records=13` đỏ oan).
            LANE_SERIAL="sau"
            ;;
        sau*)
            # **KT-333a** — làn đuôi **song song** (`sau2`, `sau3`, …): bắt đầu sau khi làn `sau`
            # xong; các làn này chạy đồng thời, mỗi làn tuần tự. Chỉ đặt bước khi tài nguyên
            # (cây nguồn, `target/`, `expected.txt`, git index, cổng mạng 45678) **rời** làn
            # đuôi khác — xem chú thích từng bước ở `do_gate_fast`.
            case " $LANE_PAR " in
                *" $lane "*) ;;
                *) LANE_PAR="$LANE_PAR $lane" ;;
            esac
            ;;
        *)
            case " $LANE_BG " in
                *" $lane "*) ;;
                *) LANE_BG="$LANE_BG $lane" ;;
            esac
            ;;
    esac
}

lane_ids() { # <làn> -> các số thứ tự, tăng dần
    printf '%s' "$LANE_STEPS" | awk -F'\t' -v l="$1" '$2 == l { print $1 }' | sort -n
}

lane_done() { # <số thứ tự> -> 0 nếu bước đã chạy (hoặc đã bỏ qua)
    [ -f "$LANE_DIR/$1.rc" ] || [ -f "$LANE_DIR/$1.skip" ]
}

lane_exec() { # <số thứ tự> <tên> <hàm> <làn> — chạy một bước, ghi log/rc/ms **riêng**
    local id="$1" name="$2" fn="$3" lane="$4" t0 t1 rc
    # T1 (`gate --dev`) bỏ đúng 14 bước chỉ kiểm harness — cùng danh sách, cùng cách khớp tên.
    if [ "$V2_TIER" = "t1" ]; then
        case "$V2_T2_ONLY_STEPS" in
            *"$name"*)
                : >"$LANE_DIR/$id.skip"
                return 0
                ;;
        esac
    fi
    # Tiến trình cho người xem: stdout chỉ in **sau** khi phát lại theo thứ tự cố định, nên
    # báo tiến độ ra stderr (không làm bẩn log cổng).
    printf '# làn %s: %s …\n' "$lane" "$name" >&2
    t0="$(now_ms)"
    # **`</dev/null` là bắt buộc**: `lane_run` đọc bảng bước từ heredoc, nên stdin của tiến trình
    # là **phần còn lại của bảng**. Bước nào đọc stdin — QEMU `-nographic` đọc UART từ stdin —
    # sẽ nuốt bảng, làm hỏng phép kiểm và **dừng vòng lặp** (các bước sau không chạy).
    "$fn" </dev/null >"$LANE_DIR/$id.log" 2>&1
    rc=$?
    t1="$(now_ms)"
    printf '%s\n' "$rc" >"$LANE_DIR/$id.rc"
    printf '%s\n' "$((t1 - t0))" >"$LANE_DIR/$id.ms"
    return 0
}

lane_run() {
    local lane id l name fn p
    # 0) **KT-333a (D2-235(5))** — fail-closed khi trùng id `lane_step`. Không chạy bước nào:
    #    nếu chạy, `lane_done` sẽ thấy `<id>.rc` của bước trước và **bỏ qua im lặng** bước sau.
    if [ -n "$LANE_DUP" ]; then
        echo "--- LỖI: trùng id lane_step:${LANE_DUP} — bước cùng id thứ hai sẽ bị bỏ qua im lặng (D2-235(5)); sửa id rồi chạy lại" >&2
        return 1
    fi
    # 1) làn nền: một tiến trình cho **mỗi** làn, các bước trong làn chạy tuần tự.
    for lane in $LANE_BG; do
        (
            while IFS="	" read -r id l name fn; do
                [ "$l" = "$lane" ] || continue
                lane_done "$id" || lane_exec "$id" "$name" "$fn" "$lane"
            done <<EOF
$LANE_STEPS
EOF
        ) &
        LANE_PIDS="$LANE_PIDS $!"
    done
    # 2) làn main: tuần tự, chạy **song song** với các làn nền ở trên.
    while IFS="	" read -r id l name fn; do
        [ "$l" = "main" ] || continue
        lane_done "$id" || lane_exec "$id" "$name" "$fn" main
    done <<EOF
$LANE_STEPS
EOF
    # 3) chờ mọi làn nền — không để tiến trình mồ côi.
    for p in $LANE_PIDS; do wait "$p" 2>/dev/null || true; done
    LANE_PIDS=""
    # 4) **KT-333a** — làn `sau`: pha đuôi **tuần tự chạy một mình** sau khi mọi làn nền xong.
    #    Chỉ chứa bước **nhạy tải** (persist cắt điện — D2-151): chạy cạnh QEMU khác thì kill có
    #    thể rơi **sau** pha 7 ⇒ đỏ oan.
    while IFS="	" read -r id l name fn; do
        [ "$l" = "sau" ] || continue
        lane_done "$id" || lane_exec "$id" "$name" "$fn" sau
    done <<EOF
$LANE_STEPS
EOF
    # 5) **KT-333a** — các làn đuôi **song song** (`sau2`, `sau3`, …): bắt đầu sau làn `sau`;
    #    mỗi làn tuần tự, tài nguyên rời nhau (xem `do_gate_fast`). QEMU tối đa **3** tiến trình
    #    đồng thời (AGENTS §9) ⇒ tối đa 3 làn đuôi có QEMU tại một thời điểm.
    for lane in $LANE_PAR; do
        (
            while IFS="	" read -r id l name fn; do
                [ "$l" = "$lane" ] || continue
                lane_done "$id" || lane_exec "$id" "$name" "$fn" "$lane"
            done <<EOF
$LANE_STEPS
EOF
        ) &
        LANE_PIDS="$LANE_PIDS $!"
    done
    # 6) chờ mọi làn đuôi song song — không để tiến trình mồ côi.
    for p in $LANE_PIDS; do wait "$p" 2>/dev/null || true; done
    LANE_PIDS=""
    # **KT-333a** — mốc kết thúc **pha** (mỗi lần gọi `lane_run` là một pha). Cổng có 2 pha
    # (`check` rồi `gate --fast`), T1 có 3 (thêm smoke QEMU). Dùng để tính **critical path đo
    # được** thay vì cộng tổng các làn (dễ sai khi pha 1 và pha 2 có làn trội khác nhau).
    LANE_PHASE_END="$LANE_PHASE_END $(( $(now_ms) - LANE_T0 ))"
}

# `lane_failed <id>` — 1 nếu **bước đó** (đã chạy) đỏ, 0 nếu xanh hoặc chưa chạy. Dùng cho
# bước mới của `check` có id > 40 (id 72): `lane_fails <max>` là đếm **dải** nên không dùng
# được cho một bước nằm sau dải của `gate --fast`, mà nới `max` lên 72 sẽ kéo cả bước 51
# (`selfbreak`, do `gate --fast` đăng ký **trước** `check`) vào lượt đếm — đổi hành vi cổng.
lane_failed() {
    if [ -f "$LANE_DIR/$1.rc" ] && [ "$(cat "$LANE_DIR/$1.rc")" -ne 0 ]; then
        printf 1
    else
        printf 0
    fi
}

lane_fails() { # [id tối đa] — đếm bước đỏ; `check` chỉ đếm bước của nó (id ≤ 40)
    local id n=0 max="${1:-999}"
    for id in $(printf '%s' "$LANE_STEPS" | awk -F'\t' '{ print $1 }' | sort -n); do
        [ "$id" -le "$max" ] || continue
        [ -f "$LANE_DIR/$id.rc" ] || continue
        [ "$(cat "$LANE_DIR/$id.rc")" -eq 0 ] || n=$((n + 1))
    done
    printf '%s' "$n"
}

lane_report() { # phát lại log từng bước theo **số thứ tự** (không theo thứ tự hoàn thành)
    local id l name fn rc ms nreg=0 nran=0 nskip=0 nfail=0
    [ -n "$LANE_DIR" ] || return 0
    printf '%s' "$LANE_STEPS" | sort -n -t"	" -k1,1 >"$LANE_DIR/order"
    while IFS="	" read -r id l name fn; do
        nreg=$((nreg + 1))
        if [ -f "$LANE_DIR/$id.skip" ]; then
            nskip=$((nskip + 1))
            echo "--- $name: BỎ QUA (T1 — chỉ chạy ở T2)"
            continue
        fi
        nran=$((nran + 1))
        echo "--- $name"
        [ -f "$LANE_DIR/$id.log" ] && cat "$LANE_DIR/$id.log"
        rc="$(cat "$LANE_DIR/$id.rc" 2>/dev/null || echo 1)"
        ms="$(cat "$LANE_DIR/$id.ms" 2>/dev/null || echo 0)"
        if [ "$rc" -eq 0 ]; then
            echo "--- $name: OK"
            printf '# step-time %s %s ms\n' "$name" "$ms"
            step_time_add "$ms" "$name"
        else
            nfail=$((nfail + 1))
            echo "--- $name: FAIL" >&2
            printf '# step-time %s %s ms (FAIL)\n' "$name" "$ms"
            step_time_add "$ms" "$name (FAIL)"
        fi
    done <"$LANE_DIR/order"
    # **KT-333a** — dòng tự mô tả cơ chế đếm bước: `registered` phải bằng hằng số kỳ vọng (đã
    # kiểm fail-closed trước khi chạy), `ran + skipped = registered`, `failed` là số bước đỏ.
    printf '# gate-steps registered=%d ran=%d skipped=%d failed=%d\n' "$nreg" "$nran" "$nskip" "$nfail"
}

lane_sum() { # <làn> -> tổng ms của các bước trong làn
    local id s=0
    for id in $(lane_ids "$1"); do
        [ -f "$LANE_DIR/$id.ms" ] && s=$((s + $(cat "$LANE_DIR/$id.ms")))
    done
    printf '%s' "$s"
}

cpu_user_sys() { # in "user sys" (giây) của shell + mọi con đã wait — không cần tiến trình ngoài
    # `times` là builtin báo CPU của **shell hiện tại** + các con đã wait. Nó phải chạy ở shell
    # hiện tại: đặt trong `$( … )` là subshell ⇒ luôn ra 0. Vì vậy ghi ra tệp rồi đọc.
    times >"$LANE_DIR/times.txt"
    awk '{
        v = $1; sub(/s$/, "", v); split(v, a, "m"); u += a[1] * 60 + a[2]
        v = $2; sub(/s$/, "", v); split(v, a, "m"); s += a[1] * 60 + a[2]
    } END { printf "%.3f %.3f", u, s }' "$LANE_DIR/times.txt"
}

# **KT-242 (P3)** — tổng kết cổng: wall, CPU (user+sys), critical path, số làn.
# **KT-333a** — pha đuôi = làn `sau` (tuần tự một mình) + **max** các làn đuôi song song; critical
# path **đo được** = tổng độ dài các pha (mỗi pha ghi ở `# gate-phase`). In cả ba để thấy chỗ còn
# cắt được.
gate_summary() { # <nhãn> <t_start_ms>
    local label="$1" t_start="$2" wall us sy main_ms tail_ms
    local lane s tail_lane
    [ -n "$LANE_DIR" ] || return 0
    wall=$(( $(now_ms) - t_start ))
    # `times` phải chạy ở **shell hiện tại** ⇒ gọi hàm với chuyển hướng (không `$( … )`).
    cpu_user_sys >"$LANE_DIR/cpu.txt"
    read -r us sy <"$LANE_DIR/cpu.txt"
    main_ms="$(lane_sum main)"
    # pha đuôi = làn `sau` (tuần tự một mình) + max các làn đuôi **song song** (`sau2`, …).
    local sau_ms par_ms par_lane
    sau_ms="$(lane_sum sau)"
    par_ms=0
    par_lane="—"
    for lane in $LANE_PAR; do
        s="$(lane_sum "$lane")"
        if [ "$s" -gt "$par_ms" ]; then
            par_ms="$s"
            par_lane="$lane"
        fi
    done
    tail_ms=$((sau_ms + par_ms))
    tail_lane="sau${par_ms:++$par_lane}"
    echo "--- bảng thời gian cổng ($label)"
    printf '# gate-wall %d ms (%.1f s)\n' "$wall" "$(awk -v w="$wall" 'BEGIN { printf "%.1f", w / 1000 }')"
    printf '# gate-cpu user=%s s sys=%s s total=%s s\n' "$us" "$sy" \
        "$(awk -v a="$us" -v b="$sy" 'BEGIN { printf "%.3f", a + b }')"
    printf '# gate-lanes %d làn nền (%s) + main + làn đuôi tuần tự (sau) + %d làn đuôi song song (%s)\n' \
        "$(printf '%s' "$LANE_BG" | wc -w | tr -d ' ')" "${LANE_BG# }" \
        "$(printf '%s' "$LANE_PAR" | wc -w | tr -d ' ')" "${LANE_PAR# }"
    printf '# gate-lane-time main=%d ms%s sau=%d ms%s\n' "$main_ms" \
        "$(for lane in $LANE_BG; do printf ' %s=%d ms' "$lane" "$(lane_sum "$lane")"; done)" \
        "$sau_ms" \
        "$(for lane in $LANE_PAR; do printf ' %s=%d ms' "$lane" "$(lane_sum "$lane")"; done)"
    printf '# gate-tail %d ms (sau=%d ms + làn song song dài nhất %s=%d ms)\n' \
        "$tail_ms" "$sau_ms" "$par_lane" "$par_ms"
    # **KT-333a** — critical path **đo được** = tổng độ dài các pha (mỗi pha = max(main, làn nền)
    # + pha đuôi của nó). Xem thêm dòng `# gate-phase` để biết từng pha.
    local prev=0 t phsum=0 phi=0
    local phline=""
    for t in $LANE_PHASE_END; do
        phi=$((phi + 1))
        phline="$phline pha$phi=$((t - prev)) ms"
        phsum=$((phsum + (t - prev)))
        prev="$t"
    done
    printf '# gate-phase%s (tổng %d ms)\n' "$phline" "$phsum"
    printf '# gate-critical-path %d ms (tổng các pha đo được; wall=%d ms)\n' "$phsum" "$wall"
    step_time_report
    printf '# step-time-total %8d ms  TỔNG %s\n' "$wall" "$label"
}
