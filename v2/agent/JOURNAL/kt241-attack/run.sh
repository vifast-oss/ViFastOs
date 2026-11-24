#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-241 — chạy ma trận tấn công (reviewer E). CHỈ chạy SAU khi A/B/C/D hợp nhất.
#
#   ./run.sh --dry-run            # in kế hoạch, KHÔNG chèn tệp (chạy được ngay cả khi API chưa có)
#   ./run.sh --preflight          # kiểm API/checker/toolchain đã sẵn sàng
#   ./run.sh --list               # bảng ô
#   ./run.sh --all                # chạy 27 ô + đối chứng dương (bỏ lớp 2)
#   ./run.sh --all --with-depinfo # chạy đầy đủ, gồm lớp 2 dep-info
#   ./run.sh --group A            # một nhóm
#   ./run.sh --cell E7            # một ô (E7 = đột biến mỏ neo)
#   ./run.sh --cell C1 --keep     # giữ tệp của ô cuối để debug (trap không khôi phục)
#
# Nguyên tắc: mỗi ô backup tệp sẽ sửa → chèn → đo → KHÔI PHỤC (trap EXIT/INT/TERM).
# Không dùng git checkout/stash/reset. Không sửa mã sản phẩm lâu dài.
set -uo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SELF_DIR/../../../.." && pwd)"
CELLS="$SELF_DIR/cells"
MANIFEST="$SELF_DIR/manifest.tsv"
LOGDIR="$SELF_DIR/logs"
RESULTS="$SELF_DIR/results"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/kt241-attack.XXXXXX")"

export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.99.1}"
PY="${PYTHON:-python3}"

MODE="run"
SELECT=""
WITH_DEPINFO=0
KEEP=0
VERBOSE=0

CORE_SRC="$ROOT/v2/kernel/vifast-core/src"
SIM_SRC="$ROOT/v2/sim/vifast-sim/src"
CORE_MANIFEST="$ROOT/v2/kernel/Cargo.toml"
HOST_MANIFEST="$ROOT/v2/Cargo.toml"
AGENTCTL="$ROOT/v2/tools/vifast-agentctl"

CREATED_FILES=()
CREATED_DIRS=()
BACKUP_ORIG=()
BACKUP_TMP=()
BACKUP_N=0

usage() {
    sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit 0
}

say()  { printf '%s\n' "$*"; }
note() { [[ "$VERBOSE" -eq 1 ]] && printf '  %s\n' "$*" || true; }
die()  { printf 'LỖI: %s\n' "$*" >&2; exit 3; }

# ---------------------------------------------------------------- backup/restore
backup_file() {
    local f="$1" i
    for i in "${!BACKUP_ORIG[@]}"; do
        [[ "${BACKUP_ORIG[$i]}" == "$f" ]] && return 0
    done
    [[ -f "$f" ]] || die "không có tệp để backup: $f"
    local t="$TMP/backup.$BACKUP_N"
    cp -p "$f" "$t" || die "backup thất bại: $f"
    BACKUP_ORIG+=("$f")
    BACKUP_TMP+=("$t")
    BACKUP_N=$((BACKUP_N + 1))
}

append_line() {
    local f="$1" line="$2"
    backup_file "$f"
    printf '%s\n' "$line" >> "$f"
}

insert_file() {
    local src="$1" dest="$2"
    [[ -f "$src" ]] || die "thiếu tệp tấn công: $src"
    create_dir "$(dirname "$dest")"
    cp "$src" "$dest" || die "chép thất bại: $src -> $dest"
    CREATED_FILES+=("$dest")
}

create_dir() {
    local d="$1"
    [[ -d "$d" ]] && return 0
    # Ghi lại **mọi** thành phần mới (từ nông tới sâu) để `restore` xoá đúng thứ tự
    # (restore duyệt ngược ⇒ xoá sâu trước, nông sau).
    local missing=()
    local p="$d"
    while [[ ! -d "$p" ]]; do
        missing+=("$p")
        p="$(dirname "$p")"
    done
    mkdir -p "$d"
    local i
    for ((i = ${#missing[@]} - 1; i >= 0; i--)); do
        CREATED_DIRS+=("${missing[$i]}")
    done
}

restore() {
    local i
    if [[ "$KEEP" -eq 1 ]]; then
        say "  [keep] giữ nguyên tệp của ô cuối (--keep); backup ở $TMP"
        return 0
    fi
    for i in "${!CREATED_FILES[@]}"; do rm -f "${CREATED_FILES[$i]}"; done
    for ((i = ${#CREATED_DIRS[@]} - 1; i >= 0; i--)); do
        rmdir "${CREATED_DIRS[$i]}" 2>/dev/null || true
    done
    for i in "${!BACKUP_ORIG[@]}"; do
        cp -p "${BACKUP_TMP[$i]}" "${BACKUP_ORIG[$i]}" || die "khôi phục thất bại: ${BACKUP_ORIG[$i]}"
    done
    CREATED_FILES=()
    CREATED_DIRS=()
    BACKUP_ORIG=()
    BACKUP_TMP=()
    BACKUP_N=0
}

cleanup() {
    local rc=$?
    restore
    if [[ "$KEEP" -eq 1 ]]; then
        say "  [keep] backup giữ ở $TMP (xoá thủ công sau khi debug)"
    else
        rm -rf "$TMP"
    fi
    exit "$rc"
}
trap cleanup EXIT INT TERM

# ---------------------------------------------------------------- preflight
api_present() {
    [[ -f "$CORE_SRC/state_write.rs" ]] \
        && grep -q 'NonReadyTarget' "$CORE_SRC/lib.rs" 2>/dev/null \
        && grep -q 'NonReadyTarget' "$ROOT/v2/sim/vifast-sim/src/kobj.rs" 2>/dev/null
}

preflight() {
    local fails=0
    say "== KT-241 PREFLIGHT =="
    say "root: $ROOT"
    say "toolchain: $RUSTUP_TOOLCHAIN (cargo: $(command -v cargo || echo 'KHÔNG THẤY'))"
    say "python: $PY ($(command -v "$PY" || echo 'KHÔNG THẤY'))"

    if [[ -f "$CORE_SRC/state_write.rs" ]]; then
        say "PASS  core state_write.rs tồn tại"
    else
        say "FAIL  core state_write.rs CHƯA có (A chưa hợp nhất)"; fails=$((fails + 1))
    fi
    if grep -q 'NonReadyTarget' "$CORE_SRC/lib.rs" 2>/dev/null; then
        say "PASS  core lib.rs re-export NonReadyTarget"
    else
        say "FAIL  core lib.rs CHƯA re-export NonReadyTarget"; fails=$((fails + 1))
    fi
    if grep -q 'NonReadyTarget' "$ROOT/v2/sim/vifast-sim/src/kobj.rs" 2>/dev/null; then
        say "PASS  sim kobj.rs có NonReadyTarget"
    else
        say "FAIL  sim kobj.rs CHƯA có NonReadyTarget"; fails=$((fails + 1))
    fi
    if grep -q 'fn write_state' "$CORE_SRC/state_write.rs" 2>/dev/null; then
        say "PASS  write_state private trong state_write.rs"
    else
        say "FAIL  write_state CHƯA có trong state_write.rs"; fails=$((fails + 1))
    fi
    if grep -q 'fn into_state' "$CORE_SRC/state_write.rs" 2>/dev/null; then
        say "PASS  into_state private trong state_write.rs"
    else
        say "FAIL  into_state CHƯA có trong state_write.rs"; fails=$((fails + 1))
    fi
    for f in v2/tools/check-wake-paths.py v2/tools/check-wake-depinfo.py v2/bin/v2; do
        if [[ -f "$ROOT/$f" ]]; then say "PASS  $f"; else say "FAIL  thiếu $f"; fails=$((fails + 1)); fi
    done
    if command -v cargo >/dev/null 2>&1; then
        say "PASS  cargo $(cargo --version 2>/dev/null | head -1)"
    else
        say "FAIL  không thấy cargo (thêm \$HOME/.cargo/bin vào PATH)"; fails=$((fails + 1))
    fi
    if [[ "$fails" -eq 0 ]]; then
        say "== PREFLIGHT PASS =="
        return 0
    fi
    say "== PREFLIGHT FAIL ($fails mục) =="
    return 2
}

# ---------------------------------------------------------------- setup từng kind
lower_of() { printf '%s' "$1" | tr 'A-Z' 'a-z'; }
# Tên module/tệp dùng chung tiền tố `kt241_` để khớp nội dung các tệp `.rs.txt`
# (B2 gọi `crate::kt241_b2_helper`, C2 `include!("kt241_c2_evil.rs")`, ...).
modname_of() { printf 'kt241_%s' "$(lower_of "$1")"; }

setup_cell() {
    local id="$1" kind="$2"
    local mod; mod="$(modname_of "$id")"
    case "$kind" in
        core)
            insert_file "$CELLS/$id.rs.txt" "$CORE_SRC/$mod.rs"
            [[ -f "$CELLS/$id.evil.rs.txt" ]] && insert_file "$CELLS/$id.evil.rs.txt" "$CORE_SRC/${mod}_evil.rs"
            append_line "$CORE_SRC/lib.rs" "mod $mod; // KT-241 $id"
            ;;
        core2)
            insert_file "$CELLS/$id.rs.txt" "$CORE_SRC/$mod.rs"
            insert_file "$CELLS/$id.helper.rs.txt" "$CORE_SRC/${mod}_helper.rs"
            append_line "$CORE_SRC/lib.rs" "mod $mod; // KT-241 $id"
            append_line "$CORE_SRC/lib.rs" "mod ${mod}_helper; // KT-241 $id"
            ;;
        sim)
            insert_file "$CELLS/$id.rs.txt" "$SIM_SRC/$mod.rs"
            append_line "$SIM_SRC/lib.rs" "mod $mod; // KT-241 $id"
            ;;
        path)
            insert_file "$CELLS/$id.rs.txt" "$CORE_SRC/$mod.rs"
            insert_file "$CELLS/$id.evil.rs.txt" "$ROOT/v2/kernel/vifast-core/evil.rs"
            create_dir "$CORE_SRC/$mod/inner"
            : > "$CORE_SRC/$mod/inner/notes.txt"
            CREATED_FILES+=("$CORE_SRC/$mod/inner/notes.txt")
            append_line "$CORE_SRC/lib.rs" "mod $mod; // KT-241 $id"
            ;;
        space)
            insert_file "$CELLS/$id.rs.txt" "$CORE_SRC/kt241 space.rs"
            append_line "$CORE_SRC/lib.rs" "#[path = \"kt241 space.rs\"] mod kt241_space; // KT-241 $id"
            ;;
        targetdir)
            insert_file "$CELLS/$id.rs.txt" "$CORE_SRC/target/mod.rs"
            insert_file "$CELLS/$id.evil.rs.txt" "$CORE_SRC/target/evil.rs"
            append_line "$CORE_SRC/lib.rs" "mod target; // KT-241 $id"
            ;;
        cfg)
            insert_file "$CELLS/$id.rs.txt" "$CORE_SRC/$mod.rs"
            append_line "$CORE_SRC/lib.rs" "#[cfg(any())] mod $mod; // KT-241 $id"
            ;;
        buildrs)
            insert_file "$CELLS/$id.build.rs.txt" "$ROOT/v2/kernel/vifast-core/build.rs"
            append_line "$CORE_SRC/lib.rs" "include!(concat!(env!(\"OUT_DIR\"), \"/evil.rs\")); // KT-241 $id"
            ;;
        crate)
            setup_crate "$id" no
            ;;
        dep)
            setup_dep "$id" no
            ;;
        mutate)
            setup_mutate "$id"
            ;;
        *)
            die "kind lạ: $kind"
            ;;
    esac
}

setup_positive() {
    local id="$1" kind="$2"
    local mod; mod="$(modname_of "$id")"
    case "$kind" in
        core)
            insert_file "$CELLS/$id.positive.rs.txt" "$CORE_SRC/$mod.rs"
            [[ -f "$CELLS/$id.positive.evil.rs.txt" ]] && insert_file "$CELLS/$id.positive.evil.rs.txt" "$CORE_SRC/${mod}_evil.rs"
            [[ -f "$CELLS/$id.positive.ok.rs.txt" ]] && insert_file "$CELLS/$id.positive.ok.rs.txt" "$CORE_SRC/${mod}_ok.rs"
            append_line "$CORE_SRC/lib.rs" "mod $mod; // KT-241 $id positive"
            ;;
        core2)
            insert_file "$CELLS/$id.positive.rs.txt" "$CORE_SRC/$mod.rs"
            insert_file "$CELLS/$id.positive.helper.rs.txt" "$CORE_SRC/${mod}_helper.rs"
            append_line "$CORE_SRC/lib.rs" "mod $mod; // KT-241 $id positive"
            append_line "$CORE_SRC/lib.rs" "mod ${mod}_helper; // KT-241 $id positive"
            ;;
        path)
            insert_file "$CELLS/$id.positive.rs.txt" "$CORE_SRC/$mod.rs"
            create_dir "$CORE_SRC/$mod/inner"
            insert_file "$CELLS/$id.positive.ok.rs.txt" "$CORE_SRC/$mod/inner/${mod}_ok.rs"
            append_line "$CORE_SRC/lib.rs" "mod $mod; // KT-241 $id positive"
            ;;
        space)
            insert_file "$CELLS/$id.positive.rs.txt" "$CORE_SRC/kt241 space.rs"
            insert_file "$CELLS/$id.positive.ok.rs.txt" "$CORE_SRC/kt241 ok.rs"
            append_line "$CORE_SRC/lib.rs" "#[path = \"kt241 space.rs\"] mod kt241_space; // KT-241 $id positive"
            ;;
        targetdir)
            insert_file "$CELLS/$id.positive.rs.txt" "$CORE_SRC/target/mod.rs"
            insert_file "$CELLS/$id.positive.ok.rs.txt" "$CORE_SRC/target/ok.rs"
            append_line "$CORE_SRC/lib.rs" "mod target; // KT-241 $id positive"
            ;;
        cfg)
            insert_file "$CELLS/$id.positive.rs.txt" "$CORE_SRC/$mod.rs"
            append_line "$CORE_SRC/lib.rs" "#[cfg(any())] mod $mod; // KT-241 $id positive"
            ;;
        buildrs)
            insert_file "$CELLS/$id.positive.rs.txt" "$CORE_SRC/$mod.rs"
            insert_file "$CELLS/$id.positive.ok.rs.txt" "$CORE_SRC/${mod}_ok.rs"
            append_line "$CORE_SRC/lib.rs" "mod $mod; // KT-241 $id positive"
            ;;
        crate)
            setup_crate "$id" yes
            ;;
        dep)
            setup_dep "$id" yes
            ;;
        mutate)
            : # không đột biến: checker trên cây sạch là đối chứng dương
            ;;
        *)
            die "positive_kind lạ: $kind"
            ;;
    esac
}

setup_crate() {
    local id="$1" positive="$2" dir name prefix
    if [[ "$positive" == "yes" ]]; then
        dir="$ROOT/v2/extra/ok"; name="vifast-ok"; prefix="$id.positive"
    else
        dir="$ROOT/v2/extra/evil"; name="vifast-evil"; prefix="$id"
    fi
    create_dir "$ROOT/v2/extra"
    create_dir "$dir"
    create_dir "$dir/src"
    insert_file "$CELLS/$prefix.crate.toml.txt" "$dir/Cargo.toml"
    insert_file "$CELLS/$prefix.crate.build.rs.txt" "$dir/build.rs"
    insert_file "$CELLS/$prefix.crate.lib.rs.txt" "$dir/src/lib.rs"
    local member="extra/${dir##*/}"
    backup_file "$HOST_MANIFEST"
    "$PY" - "$HOST_MANIFEST" "$member" <<'PY'
import sys
p, member = sys.argv[1], sys.argv[2]
text = open(p, encoding="utf-8").read()
needle = 'members = [\n'
if needle not in text:
    sys.exit(3)
if ('"%s"' % member) not in text:
    text = text.replace(needle, needle + '    "%s",\n' % member, 1)
    open(p, "w", encoding="utf-8").write(text)
PY
    [[ $? -eq 0 ]] || die "không thêm được workspace member $member"
}

setup_dep() {
    local id="$1" positive="$2" prefix
    if [[ "$positive" == "yes" ]]; then prefix="$id.positive"; else prefix="$id"; fi
    local f="$AGENTCTL/Cargo.toml"
    backup_file "$f"
    if ! grep -q 'vifast-sim' "$f"; then
        if grep -q '^\[dependencies\]' "$f"; then
            "$PY" - "$f" <<'PY'
import sys
p = sys.argv[1]
text = open(p, encoding="utf-8").read()
text = text.replace('[dependencies]\n', '[dependencies]\nvifast-sim = { path = "../../sim/vifast-sim" }\n', 1)
open(p, "w", encoding="utf-8").write(text)
PY
        else
            printf '\n[dependencies]\nvifast-sim = { path = "../../sim/vifast-sim" }\n' >> "$f"
        fi
    fi
    local mod; mod="$(modname_of "$id")"
    insert_file "$CELLS/$prefix.rs.txt" "$AGENTCTL/src/$mod.rs"
    append_line "$AGENTCTL/src/lib.rs" "mod $mod; // KT-241 $id"
}

setup_mutate() {
    local id="$1"
    local f="$CORE_SRC/state_write.rs"
    backup_file "$f"
    "$PY" - "$f" <<'PY'
import re, sys
p = sys.argv[1]
text = open(p, encoding="utf-8").read()
new, n = re.subn(r'(pub fn transition_tcb\([^)]*?to:\s*)NonReadyTarget', r'\1TcbState', text, count=1)
if n == 0:
    sys.exit(3)
open(p, "w", encoding="utf-8").write(new)
PY
    [[ $? -eq 0 ]] || die "E7: không tìm thấy chữ ký transition_tcb(... to: NonReadyTarget) trong state_write.rs"
}

# ---------------------------------------------------------------- đo
run_cargo_for_kind() {
    local kind="$1" log="$2" positive="${3:-no}"
    local pkg manifest target
    case "$kind" in
        core|core2|path|space|targetdir|cfg|buildrs)
            pkg="vifast-core"; manifest="$CORE_MANIFEST"; target="riscv64gc-unknown-none-elf" ;;
        sim)
            pkg="vifast-sim"; manifest="$HOST_MANIFEST"; target="" ;;
        crate)
            if [[ "$positive" == "yes" ]]; then pkg="vifast-ok"; else pkg="vifast-evil"; fi
            manifest="$HOST_MANIFEST"; target="" ;;
        dep)
            pkg="vifast-agentctl"; manifest="$HOST_MANIFEST"; target="" ;;
        mutate)
            return 0 ;;
        *)
            die "run_cargo_for_kind: kind lạ $kind" ;;
    esac
    local args=(check --manifest-path "$manifest" -p "$pkg")
    [[ -n "$target" ]] && args+=(--target "$target")
    ( cd "$ROOT" && cargo "${args[@]}" ) >"$log" 2>&1
    return $?
}

run_checker() {
    local mode="$1" log="$2"
    case "$mode" in
        l1)  ( cd "$ROOT" && "$PY" v2/tools/check-wake-paths.py --root . ) >"$log" 2>&1 ;;
        l1s) ( cd "$ROOT" && "$PY" v2/tools/check-wake-paths.py --root . --strict ) >"$log" 2>&1 ;;
        l2)  ( cd "$ROOT" && "$PY" v2/tools/check-wake-depinfo.py --root . ) >"$log" 2>&1 ;;
        *) die "run_checker: mode lạ $mode" ;;
    esac
    return $?
}

extract_cargo_codes() {
    grep -oE 'error\[E[0-9]+\]|error: usage of an .unsafe. block' "$1" 2>/dev/null \
        | sed -E 's/error\[(E[0-9]+)\]/\1/; s/.*unsafe.*/unsafe/' \
        | sort -u | paste -sd, - 2>/dev/null || true
}

extract_checker_codes() {
    grep -oE 'E_[A-Z][A-Z0-9_]+' "$1" 2>/dev/null | sort -u | paste -sd, - 2>/dev/null || true
}

check_expect() {
    local rc="$1" codes="$2" expect="$3" want="$4"
    case "$expect" in
        any|skip) return 0 ;;
        ok|green) [[ "$rc" -eq 0 ]] && return 0 || return 1 ;;
        fail|red)
            [[ "$rc" -ne 0 ]] || return 1
            [[ -z "$want" || "$want" == "-" ]] && return 0
            printf '%s' "$codes" | grep -qE "$want" && return 0 || return 1
            ;;
        *) return 1 ;;
    esac
}

# ---------------------------------------------------------------- một ô
run_one_cell() {
    local id="$1" group="$2" kind="$3" ecargo="$4" ecodes="$5" el1="$6" el1s="$7" el2="$8" ecodes_checker="$9" positive="${10}" pkind="${11}" desc="${12}"
    local cell_log="$LOGDIR/$id"
    mkdir -p "$cell_log"
    restore

    local status="PASS" detail=""
    local cargo_rc="-" cargo_codes="-" l1_rc="-" l1_codes="-" l1s_rc="-" l1s_codes="-" l2_rc="-" l2_codes="-"

    if ! setup_cell "$id" "$kind" 2>"$cell_log/setup.err"; then
        status="ERROR"; detail="setup thất bại: $(tr '\n' ' ' < "$cell_log/setup.err")"
    else
        if [[ "$ecargo" != "skip" ]]; then
            run_cargo_for_kind "$kind" "$cell_log/cargo.log"; cargo_rc=$?
            cargo_codes="$(extract_cargo_codes "$cell_log/cargo.log")"; [[ -z "$cargo_codes" ]] && cargo_codes="-"
            check_expect "$cargo_rc" "$cargo_codes" "$ecargo" "$ecodes" \
                || { status="FAIL"; detail="$detail cargo rc=$cargo_rc codes=$cargo_codes (muốn $ecargo/$ecodes);"; }
        fi
        if [[ "$el1" != "skip" ]]; then
            run_checker l1 "$cell_log/l1.log"; l1_rc=$?
            l1_codes="$(extract_checker_codes "$cell_log/l1.log")"; [[ -z "$l1_codes" ]] && l1_codes="-"
            check_expect "$l1_rc" "$l1_codes" "$el1" "$ecodes_checker" \
                || { status="FAIL"; detail="$detail l1 rc=$l1_rc codes=$l1_codes (muốn $el1/$ecodes_checker);"; }
        fi
        if [[ "$el1s" != "skip" ]]; then
            run_checker l1s "$cell_log/l1s.log"; l1s_rc=$?
            l1s_codes="$(extract_checker_codes "$cell_log/l1s.log")"; [[ -z "$l1s_codes" ]] && l1s_codes="-"
            check_expect "$l1s_rc" "$l1s_codes" "$el1s" "$ecodes_checker" \
                || { status="FAIL"; detail="$detail l1s rc=$l1s_rc codes=$l1s_codes (muốn $el1s/$ecodes_checker);"; }
        fi
        if [[ "$el2" != "skip" && "$WITH_DEPINFO" -eq 1 ]]; then
            run_checker l2 "$cell_log/l2.log"; l2_rc=$?
            l2_codes="$(extract_checker_codes "$cell_log/l2.log")"; [[ -z "$l2_codes" ]] && l2_codes="-"
            check_expect "$l2_rc" "$l2_codes" "$el2" "$ecodes_checker" \
                || { status="FAIL"; detail="$detail l2 rc=$l2_rc codes=$l2_codes (muốn $el2/$ecodes_checker);"; }
        elif [[ "$el2" != "skip" ]]; then
            l2_rc="skip"; l2_codes="skip"
        fi
    fi
    restore

    # --- đối chứng dương ---
    local pos="skip" pos_detail=""
    if [[ "$positive" == "yes" ]]; then
        if ! setup_positive "$id" "$pkind" 2>"$cell_log/positive-setup.err"; then
            pos="ERROR"; pos_detail="setup positive thất bại"
        else
            local pc_rc=0 pl1_rc=0 pl1s_rc=0
            run_cargo_for_kind "$pkind" "$cell_log/positive-cargo.log" yes; pc_rc=$?
            run_checker l1 "$cell_log/positive-l1.log"; pl1_rc=$?
            run_checker l1s "$cell_log/positive-l1s.log"; pl1s_rc=$?
            if [[ "$pc_rc" -eq 0 && "$pl1_rc" -eq 0 && "$pl1s_rc" -eq 0 ]]; then
                pos="PASS"
            else
                pos="FAIL"; pos_detail="cargo=$pc_rc l1=$pl1_rc l1s=$pl1s_rc"
            fi
        fi
        restore
    fi

    local verdict="$status"
    [[ "$pos" == "FAIL" || "$pos" == "ERROR" ]] && verdict="FAIL"
    [[ "$status" == "ERROR" ]] && verdict="ERROR"

    printf '%-3s %-2s %-9s %-6s %-14s %-6s %-14s %-6s %-14s %-6s %-14s %-6s %s\n' \
        "$id" "$group" "$kind" "$cargo_rc" "$cargo_codes" "$l1_rc" "$l1_codes" "$l1s_rc" "$l1s_codes" "$l2_rc" "$l2_codes" "$pos" "$verdict" \
        | tee -a "$RESULTS/current.tsv"
    [[ -n "$detail" ]] && say "      ↳ $detail"
    [[ -n "$pos_detail" ]] && say "      ↳ positive: $pos_detail"
    [[ "$VERBOSE" -eq 1 ]] && say "      ↳ $desc"
    case "$verdict" in
        PASS) return 0 ;;
        ERROR) return 2 ;;
        *) return 1 ;;
    esac
}

# ---------------------------------------------------------------- chế độ
print_plan() {
    local id group kind ecargo ecodes el1 el1s el2 ecodes_checker positive pkind desc
    local n=0
    say "== KT-241 ATTACK PLAN (dry-run — KHÔNG chèn tệp) =="
    say "root: $ROOT"
    say "toolchain: $RUSTUP_TOOLCHAIN"
    if api_present; then
        say "API: ĐÃ có (NonReadyTarget + state_write) — có thể chạy thật sau khi hợp nhất"
    else
        say "API: CHƯA có — chỉ dry-run; chạy thật sau khi A/B/C/D hợp nhất"
    fi
    say ""
    while IFS=$'\t' read -r id group kind ecargo ecodes el1 el1s el2 ecodes_checker positive pkind desc; do
        [[ -z "$id" || "$id" == \#* ]] && continue
        selected "$id" "$group" || continue
        n=$((n + 1))
        say "[$id] group=$group kind=$kind"
        say "     mô tả: $desc"
        say "     kỳ vọng: cargo=$ecargo($ecodes) l1=$el1 l1s=$el1s l2=$el2 checker_codes=$ecodes_checker"
        case "$kind" in
            core)      say "     chèn: cells/$id.rs.txt -> v2/kernel/vifast-core/src/$(modname_of "$id").rs + mod" ;;
            core2)     say "     chèn: cells/$id.rs.txt + cells/$id.helper.rs.txt -> core/src/$(modname_of "$id"){,_helper}.rs + 2 mod" ;;
            sim)       say "     chèn: cells/$id.rs.txt -> v2/sim/vifast-sim/src/$(modname_of "$id").rs + mod" ;;
            path)      say "     chèn: cells/$id.rs.txt -> core/src/$(modname_of "$id").rs; cells/$id.evil.rs.txt -> core/evil.rs; + mod" ;;
            space)     say "     chèn: cells/$id.rs.txt -> core/src/kt241 space.rs; lib.rs += #[path] mod" ;;
            targetdir) say "     chèn: cells/$id.rs.txt -> core/src/target/mod.rs; cells/$id.evil.rs.txt -> core/src/target/evil.rs; + mod target" ;;
            cfg)       say "     chèn: cells/$id.rs.txt -> core/src/$(modname_of "$id").rs; lib.rs += #[cfg(any())] mod" ;;
            buildrs)   say "     chèn: cells/$id.build.rs.txt -> core/build.rs; lib.rs += include!(concat!(env!(\"OUT_DIR\"),...))" ;;
            crate)     say "     tạo: v2/extra/evil/{Cargo.toml,build.rs,src/lib.rs}; v2/Cargo.toml += member; cargo check -p vifast-evil" ;;
            dep)       say "     sửa: v2/tools/vifast-agentctl/Cargo.toml += vifast-sim; src/$(modname_of "$id").rs + mod; cargo check -p vifast-agentctl" ;;
            mutate)    say "     đột biến: state_write.rs transition_tcb(... to: NonReadyTarget) -> TcbState; chạy checker" ;;
        esac
        if [[ "$positive" == "yes" ]]; then
            case "$pkind" in
                crate)  say "     đối chứng dương: v2/extra/ok (mã sinh lành) — kỳ vọng cargo OK + l2 XANH" ;;
                dep)    say "     đối chứng dương: cells/$id.positive.rs.txt (cùng dep, lành) — kỳ vọng cargo OK + l2 XANH" ;;
                mutate) say "     đối chứng dương: checker trên cây sạch — kỳ vọng l1/l1s XANH" ;;
                *)      say "     đối chứng dương: cells/$id.positive.rs.txt (kind=$pkind) — kỳ vọng cargo OK + l1/l1s XANH" ;;
            esac
        fi
        say ""
    done < <(awk -F'\t' '!/^#/ && NF>=12 {print}' "$MANIFEST")
    say "Tổng ô được chọn: $n"
    say "Lệnh chạy thật: ./run.sh --all --with-depinfo   (sau khi API hợp nhất)"
}

print_list() {
    local id group kind ecargo ecodes el1 el1s el2 ecodes_checker positive pkind desc
    printf '%-3s %-2s %-9s %-5s %-5s %-5s %-5s %-5s %-4s %s\n' ID GRP KIND CARGO L1 L1S L2 POS DESC
    while IFS=$'\t' read -r id group kind ecargo ecodes el1 el1s el2 ecodes_checker positive pkind desc; do
        [[ -z "$id" || "$id" == \#* ]] && continue
        printf '%-3s %-2s %-9s %-5s %-5s %-5s %-5s %-5s %-4s %s\n' \
            "$id" "$group" "$kind" "$ecargo" "$el1" "$el1s" "$el2" "$positive" "$desc"
    done < <(awk -F'\t' '!/^#/ && NF>=12 {print}' "$MANIFEST")
}

selected() {
    local id="$1" group="$2"
    [[ -z "$SELECT" || "$SELECT" == "all" ]] && return 0
    local s
    for s in ${SELECT//,/ }; do
        [[ "$s" == "group:$group" ]] && return 0
        [[ "$s" == "cell:$id" || "$s" == "$id" ]] && return 0
    done
    return 1
}

baseline() {
    say "== BASELINE (cây sạch) =="
    local r1=0 r1s=0 r2=0
    run_checker l1 "$LOGDIR/baseline-l1.log"; r1=$?
    run_checker l1s "$LOGDIR/baseline-l1s.log"; r1s=$?
    if [[ "$WITH_DEPINFO" -eq 1 ]]; then
        run_checker l2 "$LOGDIR/baseline-l2.log"; r2=$?
    fi
    if [[ "$r1" -ne 0 || "$r1s" -ne 0 || "$r2" -ne 0 ]]; then
        say "CÂY NỀN ĐỎ — dừng (l1=$r1 l1s=$r1s l2=$r2). Xem $LOGDIR/baseline-*.log"
        return 4
    fi
    say "baseline XANH (l1/l1s${WITH_DEPINFO:+/l2})"
    return 0
}

run_matrix() {
    mkdir -p "$LOGDIR" "$RESULTS"
    : > "$RESULTS/current.tsv"
    printf '%-3s %-2s %-9s %-6s %-14s %-6s %-14s %-6s %-14s %-6s %-14s %-6s %s\n' \
        ID GRP KIND CARGO CODES L1 L1CODES L1S L1SCODES L2 L2CODES POS VERDICT | tee -a "$RESULTS/current.tsv"

    local id group kind ecargo ecodes el1 el1s el2 ecodes_checker positive pkind desc
    local total=0 pass=0 fail=0 err=0
    while IFS=$'\t' read -r id group kind ecargo ecodes el1 el1s el2 ecodes_checker positive pkind desc; do
        [[ -z "$id" || "$id" == \#* ]] && continue
        selected "$id" "$group" || continue
        total=$((total + 1))
        run_one_cell "$id" "$group" "$kind" "$ecargo" "$ecodes" "$el1" "$el1s" "$el2" "$ecodes_checker" "$positive" "$pkind" "$desc"
        local rc=$?
        if [[ "$rc" -eq 0 ]]; then
            pass=$((pass + 1))
        elif [[ "$rc" -eq 1 ]]; then
            fail=$((fail + 1))
        else
            err=$((err + 1))
        fi
    done < <(awk -F'\t' '!/^#/ && NF>=12 {print}' "$MANIFEST")

    say ""
    say "== TỔNG: $total ô | PASS=$pass FAIL=$fail ERROR=$err =="
    say "kết quả: $RESULTS/current.tsv"
    say "log: $LOGDIR/<ID>/"
    [[ "$fail" -eq 0 && "$err" -eq 0 ]] && return 0 || return 1
}

# ---------------------------------------------------------------- main
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) MODE="dry-run" ;;
        --preflight) MODE="preflight" ;;
        --list) MODE="list" ;;
        --all) SELECT="all" ;;
        --group) shift; SELECT="${SELECT:+$SELECT,}group:$1" ;;
        --cell) shift; SELECT="${SELECT:+$SELECT,}cell:$1" ;;
        --with-depinfo) WITH_DEPINFO=1 ;;
        --keep) KEEP=1 ;;
        --verbose|-v) VERBOSE=1 ;;
        --help|-h) usage ;;
        *) die "tham số lạ: $1 (xem --help)" ;;
    esac
    shift
done

case "$MODE" in
    dry-run)
        print_plan
        ;;
    preflight)
        preflight
        ;;
    list)
        print_list
        ;;
    run)
        if ! api_present; then
            say "API CHƯA HỢP NHẤT — không chạy tấn công thật (sẽ chỉ ra lỗi biên dịch vô nghĩa)."
            say "Dùng: ./run.sh --dry-run   (in kế hoạch)"
            say "Sau khi A/B/C/D hợp nhất: ./run.sh --all --with-depinfo"
            exit 2
        fi
        if [[ -z "$SELECT" ]]; then
            say "chưa chọn ô: dùng --all | --group A | --cell A1 (hoặc --dry-run/--list)"
            exit 2
        fi
        if [[ "$SELECT" == "all" ]]; then
            baseline || exit $?
        fi
        run_matrix
        ;;
esac
