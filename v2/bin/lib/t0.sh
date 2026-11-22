#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-248 mục 2d) — **`v2 t0 <scope>`**: tầng **T0** chạy trong lúc sửa.
#
# T0 **KHÔNG** phải cổng: nó là bộ kiểm nhanh để agent tự bắt lỗi sớm (AGENTS.md §6.1). T1 là
# `gate --dev`, T2 là `gate --fast`. T0 **không** tạo bằng chứng nghiệm thu, **không** thay T1/T2,
# **không** chặn khi lớp 1 (T2/nightly/soak) đang chạy — chỉ **cảnh báo** (khoá 2 lớp, D2-248).
#
# Việc T0 làm (theo thứ tự):
#   1. `cargo fmt --all --check` (luôn, rẻ);
#   2. suy ra **crate bị chạm** từ `git diff --name-only HEAD` + tệp chưa theo dõi, hoặc từ
#      danh sách `--crate <tên>`; mỗi crate: `cargo clippy -p <crate> --all-targets -- -D warnings`;
#   3. `cargo test -p <crate>` (tắt bằng `--no-test`).
#
# Dùng:
#   ./v2/bin/v2 t0                     # tự suy crate từ thay đổi hiện tại
#   ./v2/bin/v2 t0 vifast-net          # crate cụ thể (một hoặc nhiều tên)
#   ./v2/bin/v2 t0 --crate vifast-net --no-test
#   ./v2/bin/v2 t0 --selftest          # tự kiểm phần suy luận + khoá + self-test công cụ T0
#   ./v2/bin/v2 t0 --help
#
# Exit: 0 = mọi bước xanh; 1 = có bước đỏ; 2 = dùng sai (cờ/crate không hợp lệ).

V2_T0_USAGE_SHOWN=0

t0_usage() {
    cat <<'EOF'
  ./v2/bin/v2 t0 [crate ...] [--crate <tên>] [--no-test] [--selftest]
                               **T0** — tầng kiểm nhanh trong lúc sửa, **KHÔNG phải cổng**.
                               fmt + clippy + `cargo test -p` trên crate bị chạm (suy ra từ
                               `git diff`/untracked, hoặc nêu tên). T1 = `gate --dev`,
                               T2 = `gate --fast`; T0 không tạo bằng chứng nghiệm thu.
                               `--selftest` kiểm phần suy luận + khoá lớp 2 + self-test công cụ T0.
EOF
}

t0_banner() {
    echo "== v2 t0 — TẦNG T0 (KHÔNG phải cổng; T1 = gate --dev, T2 = gate --fast) =="
    echo "# T0 chỉ để bắt lỗi sớm: fmt + clippy + cargo test trên crate bị chạm."
}

t0_die() {
    echo "v2 t0: $*" >&2
    return 2
}

# Tìm crate **gần nhất** của một đường dẫn (tệp hoặc thư mục) dưới `v2/`.
# In "<tên-gói>\t<Cargo.toml tính từ gốc repo>" hoặc không in gì nếu không suy ra được
# (vd. `v2/agent/QUEUE.md` — cố ý **không đoán bừa**, để T0 không kiểm nhầm crate).
t0_crate_of_path() {
    local p="$1" d name
    case "$p" in
        "$REPO_ROOT"/*) p="${p#"$REPO_ROOT"/}" ;;
        ./*) p="${p#./}" ;;
    esac
    case "$p" in
        v2/*) ;;
        *) return 0 ;;
    esac
    d="$p"
    # Đường dẫn không phải thư mục (tệp đã xoá, hoặc tệp) ⇒ bắt đầu từ thư mục cha.
    [ -d "$REPO_ROOT/$d" ] || d="$(dirname "$d")"
    while [ -n "$d" ] && [ "$d" != "." ] && [ "$d" != "/" ]; do
        if [ -f "$REPO_ROOT/$d/Cargo.toml" ]; then
            name="$(sed -n 's/^name *= *"\([^"]*\)".*/\1/p' "$REPO_ROOT/$d/Cargo.toml" 2>/dev/null | head -1)"
            if [ -n "$name" ]; then
                printf '%s\t%s\n' "$name" "$d/Cargo.toml"
                return 0
            fi
            # `v2/Cargo.toml` là workspace (không có `name`) ⇒ dừng, không trèo sang workspace v1.
            [ "$d" = "v2" ] && return 0
        fi
        # Chặn trèo lên gốc repo (workspace v1 đã đóng băng — không có crate v2 ở đó).
        [ "$d" = "v2" ] && return 0
        d="$(dirname "$d")"
    done
    return 0
}

# Đường dẫn **đã thay đổi** (tracked: `git diff HEAD`, chưa theo dõi: `ls-files --others`).
t0_changed_paths() {
    {
        git -C "$REPO_ROOT" diff --name-only HEAD 2>/dev/null || true
        git -C "$REPO_ROOT" ls-files --others --exclude-standard 2>/dev/null || true
    } | sed '/^$/d'
}

# Đọc danh sách đường dẫn trên stdin ⇒ in cặp `pkg<TAB>manifest` **duy nhất**, đã sắp xếp.
t0_plan_crates() {
    local p
    while IFS= read -r p; do
        [ -n "$p" ] || continue
        t0_crate_of_path "$p"
    done | LC_ALL=C sort -u
}

# Tìm crate theo **tên gói** trong cây `v2/` (trừ `target*`). In `<pkg>\t<manifest>`.
t0_find_crate() {
    local want="$1" m name
    for m in $(find "$V2_ROOT" -name target -prune -o -name Cargo.toml -print 2>/dev/null); do
        case "$m" in
            "$V2_ROOT/target"/* | */target/*) continue ;;
        esac
        name="$(sed -n 's/^name *= *"\([^"]*\)".*/\1/p' "$m" 2>/dev/null | head -1)"
        if [ "$name" = "$want" ]; then
            printf '%s\t%s\n' "$name" "${m#"$REPO_ROOT"/}"
            return 0
        fi
    done
    return 1
}

# Một crate: clippy rồi (tuỳ chọn) test. Trả 0/1.
t0_one_crate() {
    local pkg="$1" manifest="$2" do_test="$3" rc=0
    echo "--- T0: clippy -p $pkg ($manifest)"
    if ! "$CARGO" clippy --manifest-path "$REPO_ROOT/$manifest" -p "$pkg" --all-targets -- -D warnings; then
        echo "--- T0: clippy -p $pkg: FAIL" >&2
        rc=1
    fi
    if [ "$do_test" -eq 1 ]; then
        echo "--- T0: test -p $pkg"
        if ! "$CARGO" test --manifest-path "$REPO_ROOT/$manifest" -p "$pkg"; then
            echo "--- T0: test -p $pkg: FAIL" >&2
            rc=1
        fi
    fi
    return "$rc"
}

t0_main() {
    local selftest=0 do_test=1 explicit="" arg pkg manifest plan rc=0 n=0
    while [ $# -gt 0 ]; do
        case "$1" in
            --selftest)
                selftest=1
                shift
                ;;
            --no-test)
                do_test=0
                shift
                ;;
            --crate)
                [ $# -ge 2 ] || { t0_die "--crate cần tên gói"; return 2; }
                explicit="$explicit $2"
                shift 2
                ;;
            -h | --help)
                t0_usage
                return 0
                ;;
            -*)
                t0_die "cờ lạ '$1'"
                t0_usage >&2
                return 2
                ;;
            *)
                explicit="$explicit $1"
                shift
                ;;
        esac
    done
    if [ "$selftest" -eq 1 ]; then
        t0_selftest
        return $?
    fi
    t0_banner
    # Khoá 2 lớp: T0 **chỉ cảnh báo** khi lớp 1 đang chạy (không chặn — T0 không phải phép đo).
    v2_lock_t0_warn
    echo "--- T0: fmt (cargo fmt --all --check)"
    if ! "$CARGO" fmt --all --check --manifest-path "$MANIFEST"; then
        echo "--- T0: fmt: FAIL" >&2
        rc=1
    fi
    if [ -n "$explicit" ]; then
        for pkg in $explicit; do
            if ! plan="$(t0_find_crate "$pkg")" || [ -z "$plan" ]; then
                t0_die "không tìm thấy crate '$pkg' trong v2/ (dùng tên gói trong Cargo.toml)"
                rc=2
                continue
            fi
            manifest="$(printf '%s' "$plan" | cut -f2)"
            n=$((n + 1))
            t0_one_crate "$pkg" "$manifest" "$do_test" || rc=1
        done
    else
        plan="$(t0_changed_paths | t0_plan_crates)"
        if [ -z "$plan" ]; then
            echo "# T0: không suy ra được crate nào từ thay đổi hiện tại — chỉ chạy fmt."
            echo "#     (nêu tên crate để kiểm cụ thể: ./v2/bin/v2 t0 vifast-net)"
        else
            while IFS="	" read -r pkg manifest; do
                [ -n "$pkg" ] || continue
                n=$((n + 1))
                t0_one_crate "$pkg" "$manifest" "$do_test" || rc=1
            done <<EOF
$plan
EOF
        fi
    fi
    echo
    if [ "$rc" -eq 0 ]; then
        echo "v2 t0: PASS ($n crate; **T0** — KHÔNG phải cổng; cổng = gate --dev (T1) / gate --fast (T2))"
    else
        echo "v2 t0: FAIL ($n crate; **T0** — KHÔNG phải cổng)" >&2
    fi
    return "$rc"
}

# ---------------------------------------------------------------- --selftest (không chạy cargo)
#
# 7 ca phần **suy luận** + khoá + nhãn, cộng self-test công cụ T0 (`lib/selftests.sh`): measure-lock
# (`is_held`), `wt.sh` (4 slot), `vifast-agentctl lint`/`sync-queue`. Không build, không QEMU.
t0_selftest() {
    local ca=0 fails=0 out want
    _t0_ok() { ca=$((ca + 1)); echo "ok $ca - $1"; }
    _t0_bad() { ca=$((ca + 1)); fails=$((fails + 1)); echo "not ok $ca - $1"; }
    _t0_eq() { # <nhãn> <nhận> <mong>
        if [ "$2" = "$3" ]; then _t0_ok "$1"; else _t0_bad "$1 (nhận '$2', mong '$3')"; fi
    }

    # (1) crate của tệp trong v2/lib/<crate>/src/...
    _t0_eq "crate của v2/lib/vifast-net/src/tcp.rs" \
        "$(t0_crate_of_path v2/lib/vifast-net/src/tcp.rs)" \
        "vifast-net	v2/lib/vifast-net/Cargo.toml"
    # (2) crate kernel (manifest riêng, không phải workspace v2/Cargo.toml)
    _t0_eq "crate của v2/kernel/vifast-kbin-x86_64/src/main.rs" \
        "$(t0_crate_of_path v2/kernel/vifast-kbin-x86_64/src/main.rs)" \
        "vifast-kbin-x86_64	v2/kernel/vifast-kbin-x86_64/Cargo.toml"
    # (3) tệp tài liệu ⇒ **không** suy ra crate (không đoán bừa)
    _t0_eq "v2/agent/QUEUE.md KHÔNG suy ra crate" "$(t0_crate_of_path v2/agent/QUEUE.md)" ""
    # (4) hai tệp cùng crate ⇒ một dòng duy nhất
    _t0_eq "khử trùng: hai tệp cùng crate ⇒ 1 dòng" \
        "$(printf '%s\n' v2/lib/vifast-net/src/a.rs v2/lib/vifast-net/src/b.rs | t0_plan_crates | wc -l | tr -d ' ')" "1"
    # (5) tìm crate theo tên (có thật) + tên lạ ⇒ không thấy
    out="$(t0_find_crate vifast-agentctl 2>/dev/null)"
    case "$out" in
        *v2/tools/vifast-agentctl/Cargo.toml) _t0_ok "t0_find_crate vifast-agentctl ⇒ đúng manifest" ;;
        *) _t0_bad "t0_find_crate vifast-agentctl ⇒ '$out'" ;;
    esac
    if t0_find_crate khong-co-crate-nay >/dev/null 2>&1; then
        _t0_bad "t0_find_crate tên lạ phải KHÔNG tìm thấy"
    else
        _t0_ok "t0_find_crate tên lạ ⇒ không tìm thấy (rc khác 0)"
    fi
    # (6) khoá lớp 2: đang giữ ⇒ CẢNH BÁO, không chặn; trống ⇒ không cảnh báo.
    out="$(
        d="$(mktemp -d "${TMPDIR:-/tmp}/t0-lock.XXXXXX")"
        V2_MEASURE_LOCK_DIR="$d/lock"
        export V2_MEASURE_LOCK_DIR
        . "$REPO_ROOT/v2/tools/measure-lock.sh"
        . "$REPO_ROOT/v2/bin/lib/lock.sh"
        measure_lock_is_held && echo held || echo free
        rm -rf "$d"
    )"
    _t0_eq "khoá trống ⇒ is_held=free" "$out" "free"
    out="$(
        d="$(mktemp -d "${TMPDIR:-/tmp}/t0-lock.XXXXXX")"
        V2_MEASURE_LOCK_DIR="$d/lock"
        export V2_MEASURE_LOCK_DIR
        mkdir -p "$V2_MEASURE_LOCK_DIR"
        printf '%s:%s:%s:%s\n' "$$" "$(uname -n)" "$(date +%s)" "t0-selftest" >"$V2_MEASURE_LOCK_DIR/owner"
        . "$REPO_ROOT/v2/tools/measure-lock.sh"
        . "$REPO_ROOT/v2/bin/lib/lock.sh"
        v2_lock_t0_warn 2>&1
        rm -rf "$d"
    )"
    case "$out" in
        *"CẢNH BÁO T0"*"KHÔNG chặn"*) _t0_ok "khoá đang giữ ⇒ T0 chỉ CẢNH BÁO (không chặn)" ;;
        *) _t0_bad "khoá đang giữ ⇒ không cảnh báo/warning sai: '$out'" ;;
    esac
    # (7) nhãn T0: banner nói rõ KHÔNG phải cổng
    out="$(t0_banner)"
    case "$out" in
        *"KHÔNG phải cổng"*) _t0_ok "banner T0 nói rõ KHÔNG phải cổng" ;;
        *) _t0_bad "banner T0 thiếu nhãn 'KHÔNG phải cổng'" ;;
    esac
    # (8) `--crate` lạ ⇒ rc=2 (không lặng lẽ bỏ qua)
    out="$(t0_main --crate khong-co-crate-nay --no-test 2>&1)"
    if [ $? -eq 2 ]; then
        printf '%s' "$out" | grep -q "không tìm thấy crate" && _t0_ok "--crate lạ ⇒ rc=2 + nói rõ" ||
            _t0_bad "--crate lạ ⇒ rc=2 nhưng thiếu thông điệp"
    else
        _t0_bad "--crate lạ KHÔNG trả rc=2"
    fi

    # (9) self-test **công cụ T0** (measure-lock is_held, wt.sh 4 slot, agentctl lint/sync-queue).
    run_t0_tooling
    if [ $? -eq 0 ]; then _t0_ok "self-test công cụ T0 (measure-lock + wt.sh + agentctl lint/sync-queue)"; else _t0_bad "self-test công cụ T0"; fi

    echo
    if [ "$fails" -gt 0 ]; then
        echo "v2 t0 --selftest: FAIL ($fails/$ca)" >&2
        return 1
    fi
    echo "v2 t0 --selftest: PASS ($ca/$ca)"
    return 0
}
