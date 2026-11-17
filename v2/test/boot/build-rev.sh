#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-89 (D2-127) — **CỔNG ĐO THEO NỘI DUNG BUILD-INPUTS**: nguồn sự thật **duy nhất** cho định
# nghĩa `build-inputs`, `dirty` và cho phép kiểm "ảnh có đúng mã nguồn đang đứng ở HEAD không".
#
# Vì sao đổi (D2-127, thay KT-86/D2-125.3): cổng cũ so **lịch sử git** (`rev` 12 hex của HEAD,
# điều kiện "rev là tiền tố của HEAD"). Lịch sử đổi khi chỉ commit một tệp **không** nằm trong đầu
# vào build (`v2/docs`, `v2/agent`) ⇒ ảnh cũ bị TỪ CHỐI oan, trong khi một thay đổi **thật** trong
# crate ngoài `v2/kernel` (vd `v2/lib/vifast-alloc`) lại không được nhìn tới. Từ đây cổng so
# **nội dung đầu vào build**, không so lịch sử; **không** dùng điều kiện "`rev` là tổ tiên của
# HEAD". Định nghĩa (B6, Agent 1 quyết tạm `A1-188`, CHỜ Agent 0 xác nhận):
#
#   build-inputs = (bao đóng phụ thuộc theo `cargo metadata` của kernel/plat/lib/kbin)
#                ∪ (tệp trong v2/ mà ảnh phụ thuộc: Cargo.lock, rust-toolchain.toml,
#                   linker script, script build, v2/kernel/**, v2/lib/**, v2/bin/**)
#                TRỪ: v2/agent/**, v2/docs/**, v2/target*/**, v2/artifacts/**
#
# `v2/agent/**` là nơi harness **ghi log trong lúc đo**; nếu tính vào build-inputs thì mọi phép đo
# tự vô hiệu. `v2/agent/**` và `v2/docs/**` cũng **không** nằm trong `cargo metadata` ⇒ không phải
# đầu vào build. Phần TRỪ là quyết định tạm của Agent 1 (`A1-188`) chờ Agent 0 (`B6`).
#
# Dùng (source, không chạy trực tiếp):
#   . "$HERE/build-rev.sh"
#   vbrev_prepare_build <gốc-repo> [<nhãn>] [<im-lặng>]   # in danh sách + export env cho cargo
#   vbrev_gate <log-boot> <đường-dẫn-trong-repo> [<ảnh>]   # 0 = ĐO ĐƯỢC, 1 = TỪ CHỐI
#
# DẤU VÂN TAY (fingprint) — tính **theo nội dung ở HEAD**, không theo lịch sử:
#   * danh sách build-inputs do **script** sinh: `git ls-tree -r HEAD` cho `v2/kernel`,
#     `v2/lib`, `v2/bin` + các tệp ghim của `v2/` + **đường dẫn gói** trong bao đóng
#     `cargo metadata` của `v2/kernel/Cargo.toml` (không viết tay danh sách gói);
#   * với **từng** mục: `git rev-parse HEAD:<đường-dẫn>` (tệp → blob, thư mục → tree), ghép
#     theo thứ tự **xác định** (sắp xếp theo đường dẫn, mỗi mục một dòng `<đường-dẫn>\t<oid>`),
#     băm lại bằng sha256 ⇒ dấu vân tay; ảnh nhúng **mười sáu** hex đầu, log giữ đủ sha256.
#
# `dirty` = "0" ⇔ `git status --porcelain` không trả dòng nào trên **đúng tập** build-inputs
# (đường dẫn gốc của tập + mọi tệp tracked trong HEAD + tệp chưa track dưới các gốc đó). KHÔNG
# tính `v2/agent`, `v2/docs`, `v2/target*`, `v2/artifacts`. Mọi bất định (không git, git lỗi,
# không sinh được danh sách) ⇒ `dirty="1"` và dấu vân tay `unknown` ⇒ cổng **từ chối** (fail-closed).
#
# Định dạng dòng bằng chứng do **kernel** in lúc boot (một dòng, đúng thứ tự khoá):
#   # build: inputs=<16 hex|unknown> dirty=<0|1>
# `rev` HEAD **không** còn trong ảnh; nó chỉ xuất hiện trong log (`head_rev=`) **để tra**, không
# dùng để quyết (D2-127.5). Xem `v2/docs/k6-build-rev.md`.
set -u

# Phạm vi tính build-inputs — KHÔNG thêm đường dẫn khác mà không sửa docs + ca kiểm.
VBREV_INPUT_ROOTS="v2/kernel v2/lib v2/bin"
VBREV_INPUT_FILES="v2/Cargo.toml v2/Cargo.lock v2/rust-toolchain.toml"
# Độ dài dấu vân tay nhúng trong ảnh (hex đầu của sha256 đầy đủ); log giữ đủ sha256.
VBREV_INPUTS_LEN=16
# Độ dài `rev` HEAD chỉ để tra trong log (12 hex đầu của SHA-1).
VBREV_REV_LEN=12

# ---------------------------------------------------------------- đọc git
# vbrev_toplevel <đường-dẫn-bất-kỳ-trong-repo> -> gốc worktree (rỗng nếu không có git)
vbrev_toplevel() {
    [ -n "${1:-}" ] || return 0
    git -C "$1" --no-optional-locks rev-parse --show-toplevel 2>/dev/null | tr -d '\r'
}

# vbrev_head <gốc> -> SHA đầy đủ của HEAD (rỗng nếu không đọc được)
vbrev_head() {
    [ -n "${1:-}" ] || return 0
    git -C "$1" --no-optional-locks rev-parse --verify HEAD 2>/dev/null | tr -d '\r'
}

# vbrev_rev12 <gốc> -> 12 hex đầu của HEAD (rỗng nếu không đọc được); **chỉ để tra trong log**
vbrev_rev12() {
    local head=""
    head="$(vbrev_head "${1:-}")"
    [ -n "$head" ] || return 0
    printf '%s' "$head" | cut -c1-"$VBREV_REV_LEN"
}

# ---------------------------------------------------------------- công cụ
# vbrev_cargo -> đường dẫn cargo (env VBREV_CARGO thắng; sau đó PATH; sau đó vài chỗ quen thuộc)
vbrev_cargo() {
    local c
    if [ -n "${VBREV_CARGO:-}" ] && [ -x "${VBREV_CARGO}" ]; then
        printf '%s' "$VBREV_CARGO"
        return 0
    fi
    if command -v cargo >/dev/null 2>&1; then
        command -v cargo
        return 0
    fi
    for c in "$HOME/.cargo/bin/cargo" /usr/local/cargo/bin/cargo /opt/homebrew/bin/cargo; do
        [ -x "$c" ] && {
            printf '%s' "$c"
            return 0
        }
    done
    return 1
}

# vbrev_python -> python3 dùng được (chỉ cần json, không cần tomllib)
vbrev_python() {
    local p
    for p in python3.12 python3.11 python3; do
        if command -v "$p" >/dev/null 2>&1; then
            command -v "$p"
            return 0
        fi
    done
    return 1
}

# vbrev_sha256_file <tệp> -> 64 hex (rỗng + exit != 0 nếu không có công cụ)
vbrev_sha256_file() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    elif command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        return 1
    fi
}

# vbrev_input_allowed <đường-dẫn-tương-đối> -> 0 nếu thuộc build-inputs (đã trừ phần loại trừ)
vbrev_input_allowed() {
    case "${1:-}" in
        v2/agent | v2/agent/*) return 1 ;;
        v2/docs | v2/docs/*) return 1 ;;
        v2/artifacts | v2/artifacts/*) return 1 ;;
        v2/target | v2/target/*) return 1 ;;
        v2/target-* | v2/target-*/*) return 1 ;;
        '' | */) return 1 ;;
        *) return 0 ;;
    esac
}

# vbrev_metadata_dirs <gốc> -> thư mục gói trong bao đóng `cargo metadata` của workspace kernel,
# đường dẫn **tương đối** so với gốc. Danh sách gói do cargo sinh — không viết tay. Gói nằm
# ngoài repo (registry) không thể `git rev-parse` ⇒ nội dung của nó đã được ghim bởi Cargo.lock
# (một build-input tường minh), không lặp lại ở đây.
vbrev_metadata_dirs() {
    local root="${1:-}" cargo="" py="" json=""
    [ -n "$root" ] || return 1
    cargo="$(vbrev_cargo)" || return 1
    py="$(vbrev_python)" || return 1
    json="$("$cargo" metadata --manifest-path "$root/v2/kernel/Cargo.toml" \
        --format-version 1 --locked --offline 2>/dev/null)" || return 1
    [ -n "$json" ] || return 1
    printf '%s' "$json" | "$py" -c '
import json
import os
import sys

root = os.path.realpath(sys.argv[1])
data = json.load(sys.stdin)
for pkg in data.get("packages", []):
    manifest = pkg.get("manifest_path") or ""
    if not manifest:
        continue
    directory = os.path.realpath(os.path.dirname(manifest))
    try:
        rel = os.path.relpath(directory, root)
    except ValueError:
        continue
    if rel == "." or rel == ".." or rel.startswith(".." + os.sep):
        continue
    parts = rel.split(os.sep)
    if parts[0] == "v2" and len(parts) > 1:
        if parts[1] in ("agent", "docs", "artifacts") or parts[1].startswith("target"):
            continue
    print(rel.replace(os.sep, "/"))
' "$root" | LC_ALL=C sort -u
}

# vbrev_inputs_list <gốc> -> danh sách build-inputs (một đường dẫn tương đối mỗi dòng, đã sắp)
#   Gồm: (1) mọi tệp tracked ở HEAD dưới các gốc của v2/; (2) các tệp ghim của v2/ có ở HEAD;
#   (3) thư mục gói từ `cargo metadata` (tree). Không có git / metadata lỗi ⇒ trả != 0.
vbrev_inputs_list() {
    local root="${1:-}" meta="" tracked="" pins=""
    [ -n "$root" ] || return 1
    [ -n "$(vbrev_toplevel "$root")" ] || return 1
    [ -n "$(vbrev_head "$root")" ] || return 1
    meta="$(vbrev_metadata_dirs "$root")" || return 1
    # shellcheck disable=SC2086 # VBREV_INPUT_ROOTS/VBREV_INPUT_FILES là danh sách pathspec, cố ý tách
    tracked="$(git -C "$root" --no-optional-locks ls-tree -r --name-only HEAD -- $VBREV_INPUT_ROOTS 2>/dev/null)" || return 1
    # shellcheck disable=SC2086
    pins="$(git -C "$root" --no-optional-locks ls-tree --name-only HEAD -- $VBREV_INPUT_FILES 2>/dev/null)" || return 1
    {
        [ -n "$tracked" ] && printf '%s\n' "$tracked"
        [ -n "$pins" ] && printf '%s\n' "$pins"
        [ -n "$meta" ] && printf '%s\n' "$meta"
    } | while IFS= read -r line; do
        vbrev_input_allowed "$line" && printf '%s\n' "$line"
    done | LC_ALL=C sort -u
}

# vbrev_oids <gốc> <đường-dẫn...> -> mỗi mục một dòng `<đường-dẫn>\t<oid>` (blob/tree ở HEAD)
vbrev_oids() {
    local root="${1:-}"
    shift || return 1
    [ "$#" -gt 0 ] || return 0
    local -a revs=()
    local -a args=("$@")
    local -a lines=()
    local out="" line="" i=0
    for line in "$@"; do
        revs+=("HEAD:$line")
    done
    out="$(git -C "$root" --no-optional-locks rev-parse "${revs[@]}" 2>/dev/null)" || return 1
    while IFS= read -r line; do
        [ -n "$line" ] && lines+=("$line")
    done <<EOF
$out
EOF
    [ "${#lines[@]}" -eq "${#args[@]}" ] || return 1
    while [ "$i" -lt "${#args[@]}" ]; do
        printf '%s\t%s\n' "${args[$i]}" "${lines[$i]}"
        i=$((i + 1))
    done
}

# --------------------------------------------------- tính dấu vân tay (một lần, có cache biến)
VBREV_CALC_COUNT=0
VBREV_CALC_FULL=""
VBREV_CALC_SHORT=""
VBREV_CALC_HEAD=""
VBREV_CALC_LIST=""
VBREV_CALC_META=""
VBREV_CALC_ERROR=""

# vbrev_inputs_calc <gốc> — đặt VBREV_CALC_*; trả 0 nếu tính được dấu vân tay.
vbrev_inputs_calc() {
    local root="${1:-}" list="" head="" tmp="" oids="" full="" count=0
    VBREV_CALC_COUNT=0
    VBREV_CALC_FULL=""
    VBREV_CALC_SHORT=""
    VBREV_CALC_HEAD=""
    VBREV_CALC_LIST=""
    VBREV_CALC_META=""
    VBREV_CALC_ERROR=""
    if [ -z "$root" ] || [ ! -d "$root" ]; then
        VBREV_CALC_ERROR="no-root"
        return 1
    fi
    head="$(vbrev_head "$root")"
    if [ -z "$head" ]; then
        VBREV_CALC_ERROR="no-git-head"
        return 1
    fi
    VBREV_CALC_META="$(vbrev_metadata_dirs "$root")" || {
        VBREV_CALC_ERROR="no-cargo-metadata"
        return 1
    }
    list="$(vbrev_inputs_list "$root")" || {
        VBREV_CALC_ERROR="no-inputs-list"
        return 1
    }
    if [ -z "$list" ]; then
        VBREV_CALC_ERROR="empty-inputs"
        return 1
    fi
    tmp="$(mktemp "${TMPDIR:-/tmp}/vbrev-inputs.XXXXXX" 2>/dev/null)" || {
        VBREV_CALC_ERROR="no-temp"
        return 1
    }
    oids="$tmp.oids"
    if ! printf '%s\n' "$list" >"$tmp"; then
        rm -f "$tmp"
        VBREV_CALC_ERROR="no-temp"
        return 1
    fi
    count="$(wc -l <"$tmp" | tr -d ' ')"
    : >"$oids" || {
        rm -f "$tmp" "$oids"
        VBREV_CALC_ERROR="no-temp"
        return 1
    }
    # `git rev-parse` nhận nhiều revision trong **một** lời gọi: rẻ và giữ nguyên thứ tự.
    local -a chunk=()
    local line=""
    local chunk_ok=1
    while IFS= read -r line || [ -n "$line" ]; do
        [ -n "$line" ] || continue
        chunk+=("$line")
        if [ "${#chunk[@]}" -ge 100 ]; then
            vbrev_oids "$root" "${chunk[@]}" >>"$oids" || chunk_ok=0
            chunk=()
        fi
    done <"$tmp"
    if [ "${#chunk[@]}" -gt 0 ]; then
        vbrev_oids "$root" "${chunk[@]}" >>"$oids" || chunk_ok=0
    fi
    if [ "$chunk_ok" != "1" ]; then
        rm -f "$tmp" "$oids"
        VBREV_CALC_ERROR="git-rev-parse"
        return 1
    fi
    full="$(vbrev_sha256_file "$oids")" || {
        rm -f "$tmp" "$oids"
        VBREV_CALC_ERROR="no-sha256"
        return 1
    }
    rm -f "$tmp" "$oids"
    [ -n "$full" ] || {
        VBREV_CALC_ERROR="no-sha256"
        return 1
    }
    VBREV_CALC_COUNT="$count"
    VBREV_CALC_FULL="$full"
    VBREV_CALC_SHORT="$(printf '%s' "$full" | cut -c1-"$VBREV_INPUTS_LEN")"
    VBREV_CALC_HEAD="$(printf '%s' "$head" | cut -c1-"$VBREV_REV_LEN")"
    VBREV_CALC_LIST="$list"
    return 0
}

# ---------------------------------------------------------------- dirty
# Tập đường dẫn dùng cho `git status`: gốc + tệp ghim + thư mục gói từ metadata.
# Cố ý dùng **gốc** (không chỉ tệp tracked): tệp nguồn MỚI chưa track cũng vào ảnh ⇒ phải bẩn.
vbrev_dirty_scope() {
    local root="${1:-}" p=""
    # shellcheck disable=SC2086 # danh sách pathspec, cố ý tách theo khoảng trắng
    printf '%s\n' $VBREV_INPUT_ROOTS $VBREV_INPUT_FILES
    if [ -n "${VBREV_CALC_META:-}" ]; then
        printf '%s\n' "$VBREV_CALC_META"
    elif [ -n "$root" ]; then
        vbrev_metadata_dirs "$root" 2>/dev/null || true
    fi
}

# vbrev_dirty_lines <gốc> -> các dòng `git status --porcelain` trong phạm vi (rỗng = sạch);
# exit != 0 khi **không đọc được** trạng thái (caller phải coi là bẩn).
vbrev_dirty_lines() {
    local root="${1:-}" scope=""
    [ -n "$root" ] || return 1
    scope="$(vbrev_dirty_scope "$root")" || return 1
    [ -n "$scope" ] || return 1
    local -a pathspec=()
    local line=""
    while IFS= read -r line; do
        [ -n "$line" ] && pathspec+=("$line")
    done <<EOF
$scope
EOF
    [ "${#pathspec[@]}" -gt 0 ] || return 1
    git -C "$root" --no-optional-locks status --porcelain --untracked-files=normal \
        --ignore-submodules=all -- "${pathspec[@]}" 2>/dev/null
}

# vbrev_dirty <gốc> -> "0" | "1" (in ra stdout; không bao giờ lỗi — fail-closed về "1")
vbrev_dirty() {
    local root="${1:-}" lines=""
    if [ -z "$root" ] || [ ! -d "$root" ]; then
        printf '1'
        return 0
    fi
    if ! lines="$(vbrev_dirty_lines "$root")"; then
        printf '1'
        return 0
    fi
    case "$lines" in
        *[![:space:]]*) printf '1' ;;
        *) printf '0' ;;
    esac
}

# vbrev_dirty_files <gốc> -> danh sách tệp bẩn (một dòng một tệp, đã bỏ cột trạng thái) để log
vbrev_dirty_files() {
    local lines=""
    lines="$(vbrev_dirty_lines "${1:-}" 2>/dev/null)" || lines=""
    [ -n "$lines" ] || return 0
    printf '%s\n' "$lines" | sed -e 's/^...//' -e 's/^ *//'
}

# --------------------------------------------------- đọc dòng bằng chứng từ log/ảnh
# vbrev_line <log> -> dòng `# build: …` ĐẦU TIÊN (rỗng nếu không có)
vbrev_line() {
    [ -f "${1:-/nonexistent}" ] || return 0
    # `\r` vì serial QEMU có thể chèn CR (log lưu qua `tr`/`tee` khác nhau giữa các runner).
    grep -a -m1 -E '^# build: ' "$1" 2>/dev/null | tr -d '\r'
}

# vbrev_parse <dòng> -> đặt VBREV_LINE_INPUTS / VBREV_LINE_DIRTY; 0 = đúng định dạng, 1 = sai
# Định dạng **chặt**: `# build: inputs=<16 hex|unknown> dirty=<0|1>` và **không có gì thêm** —
# dòng méo (thiếu khoá, hex sai độ dài, dirty=2, có rác) không được coi là bằng chứng.
vbrev_parse() {
    local line="${1:-}" rest=""
    VBREV_LINE_INPUTS=""
    VBREV_LINE_DIRTY=""
    case "$line" in
        "# build: inputs="*) ;;
        *) return 1 ;;
    esac
    rest="${line#\# build: inputs=}"
    VBREV_LINE_INPUTS="${rest%% *}"
    rest="${rest#"$VBREV_LINE_INPUTS"}"
    case "$rest" in
        " dirty="*) ;;
        *) return 1 ;;
    esac
    VBREV_LINE_DIRTY="${rest# dirty=}"
    case "$VBREV_LINE_INPUTS" in
        unknown) ;;
        '' | *[!0-9a-f]*) return 1 ;;
        *)
            [ "${#VBREV_LINE_INPUTS}" -eq "$VBREV_INPUTS_LEN" ] || return 1
            ;;
    esac
    case "$VBREV_LINE_DIRTY" in
        0 | 1) ;;
        *) return 1 ;;
    esac
    return 0
}

# vbrev_image_has <ảnh> <inputs> <dirty> -> 0 nếu **chính tệp ảnh** chứa chuỗi danh tính đó.
# Kiểm này chặn ca "log của ảnh khác bị đem ra làm bằng chứng" (log không cần khớp ảnh đang đo).
vbrev_image_has() {
    local image="${1:-}" inputs="${2:-}" dirty="${3:-}"
    [ -f "$image" ] || return 1
    [ -n "$inputs" ] && [ -n "$dirty" ] || return 1
    LC_ALL=C grep -a -q -F "inputs=$inputs dirty=$dirty" "$image" 2>/dev/null
}

# ------------------------------------------------------------------- cổng
# vbrev_refuse <lý-do> [<chi-tiết>] — in dòng TỪ CHỐI (máy grep được) ra stdout
vbrev_refuse() {
    echo "REV-GATE: REFUSE reason=$1${2:+ $2}"
}

# vbrev_accept <inputs> <current_inputs> <head_rev> <count> <log> — in dòng CHẤP NHẬN
vbrev_accept() {
    echo "REV-GATE: ACCEPT inputs=$1 current_inputs=$2 head_rev=$3 count=$4 log=$5"
}

# vbrev_gate <log-boot> <đường-dẫn-trong-repo> [<ảnh>]
#   0 = cho phép đo; 1 = TỪ CHỐI ĐO (caller phải exit != 0 trước khi chạy phép đo).
# Quyết định **chỉ** dựa trên nội dung build-inputs + cờ dirty; `head_rev` chỉ để tra.
# Thứ tự: (1) có log; (2) có dòng `# build:`; (3) định dạng; (4) tính được dấu vân tay hiện tại;
# (5) dấu vân tay ảnh = hiện tại; (6) ảnh tự khai sạch; (7) cây nguồn sạch (harness tự tính);
# (8) byte ảnh chứa đúng danh tính (nếu truyền ảnh).
vbrev_gate() {
    local log="${1:-}" where="${2:-}" image="${3:-}"
    local line="" root="" head="" head12="" inputs="" idirty="" tree_dirty=""
    local cur="" count="" first=""

    if [ ! -f "$log" ]; then
        vbrev_refuse no-log "log=$log"
        return 1
    fi
    line="$(vbrev_line "$log")"
    if [ -z "$line" ]; then
        # Fail-closed: ảnh không chứng minh được nó build từ đâu ⇒ không đo (D2-127.5).
        vbrev_refuse no-build-line "log=$log"
        return 1
    fi
    if ! vbrev_parse "$line"; then
        vbrev_refuse malformed "line=[$line] log=$log"
        return 1
    fi
    inputs="$VBREV_LINE_INPUTS"
    idirty="$VBREV_LINE_DIRTY"

    root="$(vbrev_toplevel "$where")"
    if [ -z "$root" ]; then
        vbrev_refuse no-git "where=$where image_inputs=$inputs log=$log"
        return 1
    fi
    head="$(vbrev_head "$root")"
    if [ -z "$head" ]; then
        vbrev_refuse no-git "root=$root image_inputs=$inputs log=$log"
        return 1
    fi
    head12="$(printf '%s' "$head" | cut -c1-"$VBREV_REV_LEN")"
    if ! vbrev_inputs_calc "$root"; then
        vbrev_refuse no-inputs-fp "image_inputs=$inputs head_rev=$head12 why=${VBREV_CALC_ERROR:-unknown} where=$where log=$log"
        return 1
    fi
    cur="$VBREV_CALC_SHORT"
    count="$VBREV_CALC_COUNT"
    if [ "$inputs" = "unknown" ]; then
        vbrev_refuse no-inputs "image_inputs=unknown current_inputs=$cur head_rev=$head12 count=$count log=$log"
        return 1
    fi
    # Quyết định: **nội dung** build-inputs, không dùng lịch sử git (D2-127.2/.5).
    if [ "$inputs" != "$cur" ]; then
        vbrev_refuse inputs-mismatch "image_inputs=$inputs current_inputs=$cur head_rev=$head12 count=$count log=$log"
        return 1
    fi
    if [ "$idirty" != "0" ]; then
        vbrev_refuse dirty "image_dirty=$idirty image_inputs=$inputs current_inputs=$cur head_rev=$head12 log=$log"
        return 1
    fi
    # Harness **tự** tính lại dirty trên đúng tập build-inputs: ảnh nói sạch mà cây đang bẩn ⇒
    # ảnh không đại diện cho nội dung hiện tại (đúng lớp lỗi KT-76/KT-77b).
    tree_dirty="$(vbrev_dirty "$root")"
    if [ "$tree_dirty" != "0" ]; then
        first="$(vbrev_dirty_files "$root" | head -1)"
        vbrev_refuse tree-dirty "root=$root image_inputs=$inputs current_inputs=$cur head_rev=$head12 count=$count first=[$first] log=$log"
        return 1
    fi
    # Ảnh phải chứa đúng chuỗi danh tính của dòng log (log của ảnh khác ⇒ từ chối).
    if [ -n "$image" ]; then
        if ! vbrev_image_has "$image" "$inputs" "$idirty"; then
            vbrev_refuse image-mismatch "image=$image image_inputs=$inputs dirty=$idirty head_rev=$head12 log=$log"
            return 1
        fi
    fi
    vbrev_accept "$inputs" "$cur" "$head12" "$count" "$log"
    return 0
}

# vbrev_summary <đường-dẫn-trong-repo> — in một dòng trạng thái để log (không phải cổng)
vbrev_summary() {
    local root="" head12="" dirty="" fp="unknown" count=0
    root="$(vbrev_toplevel "${1:-}")"
    if [ -z "$root" ]; then
        echo "REV-STATE: root=<none> head_rev=<none> inputs=unknown count=0 dirty=1"
        return 0
    fi
    head12="$(vbrev_rev12 "$root")"
    if vbrev_inputs_calc "$root"; then
        fp="$VBREV_CALC_SHORT"
        count="$VBREV_CALC_COUNT"
    fi
    dirty="$(vbrev_dirty "$root")"
    echo "REV-STATE: root=$root head_rev=${head12:-<none>} inputs=$fp count=$count dirty=$dirty scope=[$VBREV_INPUT_ROOTS $VBREV_INPUT_FILES]"
}

# vbrev_prepare_build <gốc-repo> [<nhãn>] [<im-lặng 1|0>]
#   Tính build-inputs + dirty, **in danh sách + số mục** (mỗi lần build), rồi export env để
#   `build.rs` nhúng dấu vân tay vào ảnh: VIFAST_BUILD_INPUTS / _INPUTS_COUNT / _DIRTY / _REV.
#   Không tính được (không git/metadata) ⇒ export `unknown`+`dirty=1` (ảnh nói thật; cổng từ chối).
vbrev_prepare_build() {
    local root="${1:-}" label="${2:-build}" quiet="${3:-0}" dirty="1" head12="" i=0 p=""
    head12="$(vbrev_rev12 "$root")"
    if vbrev_inputs_calc "$root"; then
        dirty="$(vbrev_dirty "$root")"
        export VIFAST_BUILD_INPUTS="$VBREV_CALC_SHORT"
        export VIFAST_BUILD_INPUTS_COUNT="$VBREV_CALC_COUNT"
        export VIFAST_BUILD_DIRTY="$dirty"
        export VIFAST_BUILD_REV="${head12:-unknown}"
        echo "# build-inputs ($label): count=$VBREV_CALC_COUNT sha256=$VBREV_CALC_FULL short=$VBREV_CALC_SHORT dirty=$dirty head_rev=${head12:-unknown} (head_rev chỉ để tra, không dùng để quyết)"
        if [ "$quiet" != "1" ]; then
            while IFS= read -r p; do
                [ -n "$p" ] || continue
                i=$((i + 1))
                echo "# build-input[$i] $p"
            done <<EOF
$VBREV_CALC_LIST
EOF
        fi
        return 0
    fi
    export VIFAST_BUILD_INPUTS="unknown"
    export VIFAST_BUILD_INPUTS_COUNT="0"
    export VIFAST_BUILD_DIRTY="1"
    export VIFAST_BUILD_REV="${head12:-unknown}"
    echo "# build-inputs ($label): KHÔNG sinh được danh sách (${VBREV_CALC_ERROR:-unknown}) — ảnh mang inputs=unknown, cổng đo sẽ TỪ CHỐI"
    return 0
}

# vbrev_selftest — ca hồi quy của **shell** (không có git thật của repo): dựng repo mini trong
# thư mục tạm, kiểm (a) tệp ngoài build-inputs không đổi dấu vân tay, (b) sửa kernel **chưa
# commit** ⇒ dirty=1, (c) commit kernel ⇒ dấu vân tay đổi, (d) crate ngoài `v2/kernel` (metadata
# báo) ⇒ dấu vân tay đổi, (e) `v2/rust-toolchain.toml` ⇒ dấu vân tay đổi. Trả 0 = đạt.
vbrev_selftest() {
    local scratch="" root="" rc=0 fp0="" fp1="" fp2="" fp3="" fp4="" dirty=""
    local cargo=""
    cargo="$(vbrev_cargo)" || {
        echo "# vbrev-selftest: không có cargo — bỏ qua"
        return 0
    }
    scratch="$(mktemp -d "${TMPDIR:-/tmp}/vbrev-selftest.XXXXXX" 2>/dev/null)" || return 1
    root="$scratch/repo"
    mkdir -p "$root" || {
        rm -rf "$scratch"
        return 1
    }
    (
        set -ex
        cd "$root" || exit 1
        git init -q .
        git config user.email "kt89@example.invalid"
        git config user.name "kt89"
        mkdir -p v2/kernel/k v2/lib/alloc/src v2/agent v2/docs v2/bin
        cat >v2/kernel/Cargo.toml <<'EOF'
[workspace]
resolver = "2"
members = ["k"]
EOF
        cat >v2/kernel/k/Cargo.toml <<'EOF'
[package]
name = "k"
version = "0.1.0"
edition = "2021"
[dependencies]
alloc = { path = "../../lib/alloc" }
EOF
        printf 'pub fn f() -> u32 { 1 }\n' >v2/kernel/k/src.rs
        printf '[package]\nname = "alloc"\nversion = "0.1.0"\nedition = "2021"\n' >v2/lib/alloc/Cargo.toml
        printf 'pub fn a() -> u32 { 1 }\n' >v2/lib/alloc/src/lib.rs
        printf '[toolchain]\nchannel = "1.99.1"\n' >v2/rust-toolchain.toml
        printf '# lock\n' >v2/Cargo.lock
        printf '# host\n' >v2/Cargo.toml
        printf '# tool\n' >v2/bin/v2
        printf 'log\n' >v2/agent/journal.log
        printf 'doc\n' >v2/docs/d.md
        git add -A
        git commit -qm baseline
    ) >/dev/null 2>&1 || {
        echo "# vbrev-selftest: dựng repo mini FAIL"
        rm -rf "$scratch"
        return 1
    }
    vbrev_inputs_calc "$root" || rc=1
    fp0="$VBREV_CALC_SHORT"
    # (a) tệp ngoài build-inputs, commit ⇒ dấu vân tay KHÔNG đổi
    printf 'them\n' >>"$root/v2/docs/d.md"
    printf 'them\n' >>"$root/v2/agent/journal.log"
    git -C "$root" add -A >/dev/null 2>&1 && git -C "$root" commit -qm outside >/dev/null 2>&1
    vbrev_inputs_calc "$root" || rc=1
    fp1="$VBREV_CALC_SHORT"
    [ "$fp0" = "$fp1" ] || {
        echo "# vbrev-selftest (a) FAIL: commit ngoài build-inputs làm đổi dấu vân tay"
        rc=1
    }
    # (b) sửa kernel **chưa** commit ⇒ dirty=1
    printf 'pub fn f() -> u32 { 2 }\n' >"$root/v2/kernel/k/src.rs"
    dirty="$(vbrev_dirty "$root")"
    [ "$dirty" = "1" ] || {
        echo "# vbrev-selftest (b) FAIL: sửa kernel chưa commit mà dirty=$dirty"
        rc=1
    }
    # (c) commit kernel ⇒ dấu vân tay đổi
    git -C "$root" add -A >/dev/null 2>&1 && git -C "$root" commit -qm kernel >/dev/null 2>&1
    vbrev_inputs_calc "$root" || rc=1
    fp2="$VBREV_CALC_SHORT"
    [ "$fp1" != "$fp2" ] || {
        echo "# vbrev-selftest (c) FAIL: commit kernel mà dấu vân tay không đổi"
        rc=1
    }
    [ "$(vbrev_dirty "$root")" = "0" ] || {
        echo "# vbrev-selftest (c) FAIL: sau commit kernel mà dirty != 0"
        rc=1
    }
    # (d) crate ngoài v2/kernel do metadata báo ⇒ dấu vân tay đổi
    printf 'pub fn a() -> u32 { 2 }\n' >"$root/v2/lib/alloc/src/lib.rs"
    git -C "$root" add -A >/dev/null 2>&1 && git -C "$root" commit -qm alloc >/dev/null 2>&1
    vbrev_inputs_calc "$root" || rc=1
    fp3="$VBREV_CALC_SHORT"
    [ "$fp2" != "$fp3" ] || {
        echo "# vbrev-selftest (d) FAIL: crate ngoài v2/kernel đổi mà dấu vân tay không đổi"
        rc=1
    }
    # (e) rust-toolchain.toml ở v2/ ⇒ dấu vân tay đổi
    printf '[toolchain]\nchannel = "1.99.2"\n' >"$root/v2/rust-toolchain.toml"
    git -C "$root" add -A >/dev/null 2>&1 && git -C "$root" commit -qm toolchain >/dev/null 2>&1
    vbrev_inputs_calc "$root" || rc=1
    fp4="$VBREV_CALC_SHORT"
    [ "$fp3" != "$fp4" ] || {
        echo "# vbrev-selftest (e) FAIL: rust-toolchain.toml đổi mà dấu vân tay không đổi"
        rc=1
    }
    rm -rf "$scratch"
    if [ "$rc" -eq 0 ]; then
        echo "# vbrev-selftest: PASS (a/b/c/d/e)"
    else
        echo "# vbrev-selftest: FAIL"
    fi
    return "$rc"
}

# Chạy trực tiếp: `bash build-rev.sh --selftest` (dùng cho ca hồi quy của shell, luật 8).
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    case "${1:-}" in
        --selftest) vbrev_selftest ;;
        *)
            echo "dùng: bash build-rev.sh --selftest   (hoặc source từ runner)" >&2
            exit 2
            ;;
    esac
fi
