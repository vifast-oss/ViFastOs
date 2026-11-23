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
#
# **KT-134 (C1 của Agent 0, lượt 2) — `v2/bin` bị LOẠI khỏi phạm vi.** Đo được: commit
# `4ed158b4` chỉ sửa `v2/bin/v2` (script cổng) mà dấu vân tay đổi `0872fff0…` → `cc740cfa…`,
# tức danh sách ghim lại chứa tệp **không phải đầu vào của ảnh** — đúng lỗi C1 mà Agent 0 vừa
# bắt. Kiểm: `v2/kernel/*/build.rs` **không** gọi `v2/bin`; `vifast-gen` sinh ra
# `v2/lib/vifast-idl/src/generated*.rs` và các tệp đó **đã tracked trong `v2/lib`** (vẫn nằm
# trong phạm vi) ⇒ đổi generator mà sinh lại thì dấu vân tay vẫn đổi qua `v2/lib`.
# **KT-346** (D2-252): `v2/user` là **cây nguồn userland sản phẩm** (shim ABI `runtime/` +
# chương trình U-mode `programs/`) — được biên dịch vào ảnh sản phẩm `--features product-shell`
# nên **phải** nằm trong dấu vân tay. Tách khỏi `v2/kernel/vifast-plat-user` + `kbin-riscv64/init/`
# ở cùng commit; tăng `VBREV_ALGO` `kt134-v4` → `kt134-v5` vì **định nghĩa** đổi (thêm gốc).
VBREV_INPUT_ROOTS="v2/kernel v2/lib v2/user"
# KT-93 (review verify-kt89): **script build** cũng là đầu vào — `repro-build.sh` đặt RUSTFLAGS
# (remap/strip) ảnh hưởng **byte ảnh**; `rustc-canon.rs` là wrapper rustc canonical. Hai tệp này
# **phải** nằm trong dấu vân tay.
#
# **KT-134 (Q3 + C1 của Agent 0, 2026-11-18) — BA tệp bị LOẠI khỏi danh sách ghim:**
#   * `v2/test/boot/build-rev.sh` — chính script này, tức **định nghĩa** dấu vân tay, không phải
#     đầu vào của ảnh. Đo được: KT-128 sửa nó (thêm ca hồi quy âm) ⇒ sha ảnh đổi, dù mã sản phẩm
#     không đổi một dòng. Đó là "danh sách ghim chứa tệp không phải đầu vào", trái định nghĩa Q3.
#   * `v2/tools/pre-measure.sh` — kiểm **trước khi đo**, không tham gia build.
#   * `v2/tools/task-target.sh` — chỉ đặt `CARGO_TARGET_DIR`, không đổi byte ảnh.
# Ca kiểm (h) của `--selftest` chứng minh: sửa một tệp test/checker ⇒ dấu vân tay **KHÔNG** đổi.
# Rủi ro còn lại (ghi ở `JOURNAL/kt134-journal.md`): đổi **thuật toán** dấu vân tay thì dấu vân tay
# không đổi ⇒ dấu vân tay cũ và mới không so được. Chấp nhận: đổi thuật toán là đổi **định dạng**,
# phải bump `VBREV_ALGO` (hằng số dưới đây, **có** nằm trong chuỗi băm).
VBREV_ALGO="kt134-v5"
VBREV_INPUT_FILES="v2/Cargo.toml v2/Cargo.lock v2/rust-toolchain.toml v2/tools/repro-build.sh v2/tools/rustc-canon.rs"
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
        # **KT-134 (luật 2 của Agent 0: quét LẠI TOÀN BỘ phạm vi)**: ba thư mục dưới đây là test
        # tích hợp / bench / example — cargo **không bao giờ** biên dịch chúng vào binary phát
        # hành, nên chúng không phải đầu vào của ảnh. Đo được: `v2/kernel` + `v2/lib` có ba mươi
        # ba tệp như vậy; sửa một tệp trong đó trước đây làm dấu vân tay đổi (cùng lớp lỗi C1).
        # **Cố ý KHÔNG loại `src/`**: một tệp trong `src/` có thể được `mod` không-test biên dịch,
        # nên giữ nguyên là phía an toàn (thừa còn hơn thiếu: thiếu ⇒ hai ảnh khác nhau cùng dấu
        # vân tay).
        */tests | */tests/*) return 1 ;;
        */benches | */benches/*) return 1 ;;
        */examples | */examples/*) return 1 ;;
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
    # **KT-100 (review đêm 17/11)**: `ls-tree` coi đường dẫn **không khớp** là rỗng **hợp lệ** ⇒ một
    # tệp ghim bị `git rm --cached` + thêm vào `.gitignore` sẽ **âm thầm** rơi khỏi build-inputs:
    # dấu vân tay không đổi, `dirty=0`, và sửa nội dung tệp đó **không** bị bắt. Đó đúng là khe
    # KT-93 định đóng. Nay: **thiếu mặt** ⇒ lỗi (fail-closed), không im lặng.
    local _f="" _missing=""
    for _f in $VBREV_INPUT_FILES; do
        printf '%s\n' "$pins" | LC_ALL=C grep -qxF -- "$_f" || _missing="$_missing $_f"
    done
    if [ -n "$_missing" ]; then
        VBREV_CALC_ERROR="missing-input:$_missing"
        return 1
    fi
    # **KT-134 lượt 3 (luật 2 của Agent 0)**: `meta` là danh sách **thư mục gói**, và `git
    # rev-parse HEAD:<thư-mục>` trả **tree oid** — tree gộp **mọi** tệp bên trong, kể cả `tests/`.
    # Vì vậy chỉ cần `meta` có mặt là việc loại `*/tests/*` ở trên **vô hiệu**: sửa một tệp test
    # vẫn đổi dấu vân tay. Đo được: ca (j) đỏ đúng vì lý do này.
    # Nay chỉ giữ `meta` cho thư mục **nằm ngoài** các gốc — đó mới là mục đích ban đầu của nó
    # (bắt gói cargo ở nơi khác); thư mục nằm trong gốc đã được `tracked` phủ **theo từng tệp**.
    local _m="" _keep="" _under=""
    for _m in $meta; do
        _under=0
        for _r in $VBREV_INPUT_ROOTS; do
            case "$_m" in "$_r" | "$_r"/*) _under=1 ;; esac
        done
        [ "$_under" -eq 1 ] || _keep="$_keep $_m"
    done
    {
        [ -n "$tracked" ] && printf '%s\n' "$tracked"
        [ -n "$pins" ] && printf '%s\n' "$pins"
        [ -n "$_keep" ] && printf '%s\n' $_keep
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
        # `vbrev_inputs_list` chạy trong subshell ⇒ lỗi cụ thể của nó không truyền ra; kiểm lại
        # **đủ mặt** tệp ghim ở đây để báo đúng lý do (KT-100).
        local _f="" _missing=""
        for _f in $VBREV_INPUT_FILES; do
            git -C "$root" --no-optional-locks cat-file -e "HEAD:$_f" 2>/dev/null || _missing="$_missing $_f"
        done
        if [ -n "$_missing" ]; then
            VBREV_CALC_ERROR="missing-input:$_missing"
        else
            VBREV_CALC_ERROR="no-inputs-list"
        fi
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
    # KT-134: dòng **phiên bản thuật toán** đứng đầu chuỗi băm. Nhờ nó, đổi thuật toán mà quên
    # bump `VBREV_ALGO` thì dấu vân tay vẫn đổi (vì hằng số nằm trong chuỗi băm), còn đổi **tệp
    # công cụ** thì không đổi — đúng ranh giới "đầu vào ảnh" / "công cụ đo".
    printf 'algo\t%s\n' "$VBREV_ALGO" >"$oids" || {
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

# vbrev_parse <dòng> -> đặt VBREV_LINE_ALGO / VBREV_LINE_INPUTS / VBREV_LINE_DIRTY;
#   0 = đúng định dạng, 1 = sai.
# Định dạng **chặt**: `# build: algo=<tên> inputs=<16 hex|unknown> dirty=<0|1>` và **không có gì
# thêm** — dòng méo (thiếu khoá, hex sai độ dài, dirty=2, có rác) không được coi là bằng chứng.
#
# **KT-143**: `algo=` là **bắt buộc** và phải **khớp** `VBREV_ALGO` hiện hành. Vì sao chặt: đổi
# định nghĩa dấu vân tay (thêm/bớt thư mục khỏi phạm vi) làm `inputs` đổi mà **không** đổi một byte
# mã nguồn nào. Không có `algo=` thì hai log khác số mà người đọc không biết chúng có so được với
# nhau hay không — và tệ hơn, một log cũ có thể "tình cờ" khớp số với định nghĩa mới. Log cũ
# (trước KT-143) **không** được sửa và **không** được phân tích lại: truy nguyên bằng commit.
vbrev_parse() {
    local line="${1:-}" rest=""
    VBREV_LINE_ALGO=""
    VBREV_LINE_INPUTS=""
    VBREV_LINE_DIRTY=""
    case "$line" in
        "# build: algo="*) ;;
        *) return 1 ;;
    esac
    rest="${line#\# build: algo=}"
    VBREV_LINE_ALGO="${rest%% *}"
    rest="${rest#"$VBREV_LINE_ALGO"}"
    case "$rest" in
        " inputs="*) ;;
        *) return 1 ;;
    esac
    rest="${rest# inputs=}"
    VBREV_LINE_INPUTS="${rest%% *}"
    rest="${rest#"$VBREV_LINE_INPUTS"}"
    case "$rest" in
        " dirty="*) ;;
        *) return 1 ;;
    esac
    VBREV_LINE_DIRTY="${rest# dirty=}"
    # `algo` phải **đúng** định nghĩa hiện hành; `unknown` cũng bị từ chối (fail-closed).
    case "$VBREV_LINE_ALGO" in
        '' | *[!0-9A-Za-z._-]*) return 1 ;;
    esac
    [ "$VBREV_LINE_ALGO" = "$VBREV_ALGO" ] || return 1
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

# vbrev_image_has <ảnh> <algo> <inputs> <dirty> -> 0 nếu **chính tệp ảnh** chứa chuỗi danh tính đó.
# Kiểm này chặn ca "log của ảnh khác bị đem ra làm bằng chứng" (log không cần khớp ảnh đang đo).
# KT-143: chuỗi danh tính nay gồm **cả** `algo=` — ảnh build bằng định nghĩa khác thì không phải
# ảnh đang đo, dù `inputs` tình cờ trùng.
vbrev_image_has() {
    local image="${1:-}" algo="${2:-}" inputs="${3:-}" dirty="${4:-}"
    [ -f "$image" ] || return 1
    [ -n "$algo" ] && [ -n "$inputs" ] && [ -n "$dirty" ] || return 1
    LC_ALL=C grep -a -q -F "algo=$algo inputs=$inputs dirty=$dirty" "$image" 2>/dev/null
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
    local line="" root="" head="" head12="" inputs="" idirty="" ialgo="" tree_dirty=""
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
    ialgo="$VBREV_LINE_ALGO"

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
    # D2-127 + review KT-89: ảnh là **bắt buộc** — thiếu ảnh thì không chứng minh được log thuộc
    # ảnh nào ⇒ TỪ CHỐI (trước đây bỏ trống ảnh là bỏ qua bước này = fail-open).
    if [ -z "$image" ]; then
        vbrev_refuse no-image "image_inputs=$inputs dirty=$idirty head_rev=$head12 log=$log"
        return 1
    fi
    if [ ! -f "$image" ]; then
        vbrev_refuse image-missing "image=$image log=$log"
        return 1
    fi
    if true; then
        if ! vbrev_image_has "$image" "$ialgo" "$inputs" "$idirty"; then
            vbrev_refuse image-mismatch "image=$image image_algo=$ialgo image_inputs=$inputs dirty=$idirty head_rev=$head12 log=$log"
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
    local scratch="" root="" rc=0 fp0="" fp1="" fp2="" fp3="" fp4="" fp5="" fp6="" fp7="" fp8="" fp9="" fp10="" fp11="" dirty=""
    local gate_out=""
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
        mkdir -p v2/kernel/k/src v2/lib/alloc/src v2/agent v2/docs v2/bin
        # **KT-346** — cây userland sản phẩm (shim + chương trình U-mode) trong repo mini.
        mkdir -p v2/user/runtime v2/user/programs
        printf 'pub fn abi() -> u32 { 1 }\n' >v2/user/runtime/lib.rs
        printf 'pub fn shell() -> u32 { 1 }\n' >v2/user/programs/shell.rs
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
        printf 'pub fn f() -> u32 { 1 }\n' >v2/kernel/k/src/lib.rs
        printf '[package]\nname = "alloc"\nversion = "0.1.0"\nedition = "2021"\n' >v2/lib/alloc/Cargo.toml
        printf 'pub fn a() -> u32 { 1 }\n' >v2/lib/alloc/src/lib.rs
        printf '[toolchain]\nchannel = "1.99.1"\n' >v2/rust-toolchain.toml
        printf '# lock\n' >v2/Cargo.lock
        # `cargo metadata --locked` cần lockfile của **workspace kernel** (không tự tạo khi --locked).
        cat >v2/kernel/Cargo.lock <<'LOCK'
version = 3

[[package]]
name = "alloc"
version = "0.1.0"

[[package]]
name = "k"
version = "0.1.0"
dependencies = [
 "alloc",
]
LOCK
        printf '[workspace]\nresolver = "2"\nmembers = []\n' >v2/Cargo.toml
        printf '# tool\n' >v2/bin/v2
        printf 'log\n' >v2/agent/journal.log
        printf 'doc\n' >v2/docs/d.md
        # KT-128: tạo **mọi** tệp ghim trong `VBREV_INPUT_FILES` mà repo mini chưa có. Trước đây
        # danh sách này lớn dần (KT-93 thêm 3 tệp, KT-95/98 thêm 2) mà repo mini không cập nhật ⇒
        # `vbrev_inputs_calc` trả `missing-input` ⇒ (c)(d)(e) ĐỎ **âm thầm** (không cổng nào chạy
        # selftest). Sinh từ chính biến để không lệch lần nữa.
        for _f in $VBREV_INPUT_FILES; do
            [ -f "$_f" ] && continue
            mkdir -p "$(dirname "$_f")"
            printf '# pinned\n' >"$_f"
        done
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
    printf 'pub fn f() -> u32 { 2 }\n' >"$root/v2/kernel/k/src/lib.rs"
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
    # (f) KT-128 (Q3 bổ sung của Agent 0): ca hồi quy **ÂM** — sửa **một byte** trong mã kernel
    #     rồi commit ⇒ (i) dấu vân tay PHẢI đổi, VÀ (ii) **cổng PHẢI TỪ CHỐI ĐO** khi log/ảnh còn
    #     mang dấu vân tay CŨ. Vế (ii) là vế mà (c)/(d)/(e) **chưa** chứng minh: chúng chỉ nói
    #     "dấu vân tay đổi", không nói "harness từ chối". Không có vế này thì một cổng hỏng
    #     (luôn ACCEPT) vẫn qua được toàn bộ selftest.
    printf '# build: algo=%s inputs=%s dirty=0\n' "$VBREV_ALGO" "$fp4" >"$scratch/old.log"
    printf 'ELF-gia\n# build: algo=%s inputs=%s dirty=0\n' "$VBREV_ALGO" "$fp4" >"$scratch/old.bin"
    printf 'pub fn f() -> u32 { 3 }\n' >"$root/v2/kernel/k/src/lib.rs"
    git -C "$root" add -A >/dev/null 2>&1 && git -C "$root" commit -qm "kernel 1 byte" >/dev/null 2>&1
    vbrev_inputs_calc "$root" || rc=1
    fp5="$VBREV_CALC_SHORT"
    [ "$fp4" != "$fp5" ] || {
        echo "# vbrev-selftest (f) FAIL: sửa 1 byte mã kernel mà dấu vân tay không đổi"
        rc=1
    }
    gate_out="$(vbrev_gate "$scratch/old.log" "$root" "$scratch/old.bin" 2>&1)"
    case "$gate_out" in
        *"REFUSE reason=inputs-mismatch"*) ;;
        *)
            echo "# vbrev-selftest (f) FAIL: cổng KHÔNG từ chối ảnh cũ sau khi kernel đổi — [$gate_out]"
            rc=1
            ;;
    esac
    # (g) ĐỐI CHỨNG DƯƠNG của (f): cùng cây đó, log/ảnh mang dấu vân tay **MỚI** ⇒ cổng PHẢI
    #     ACCEPT. Thiếu vế này thì một cổng hỏng (luôn REFUSE) cũng qua được (f).
    printf '# build: algo=%s inputs=%s dirty=0\n' "$VBREV_ALGO" "$fp5" >"$scratch/new.log"
    printf 'ELF-gia\n# build: algo=%s inputs=%s dirty=0\n' "$VBREV_ALGO" "$fp5" >"$scratch/new.bin"
    gate_out="$(vbrev_gate "$scratch/new.log" "$root" "$scratch/new.bin" 2>&1)"
    case "$gate_out" in
        *"ACCEPT inputs=$fp5"*) ;;
        *)
            echo "# vbrev-selftest (g) FAIL: cổng KHÔNG chấp nhận ảnh khớp dấu vân tay hiện tại — [$gate_out]"
            rc=1
            ;;
    esac
    # (h) KT-134 (C1 của Agent 0): sửa một tệp **test/checker** (không phải đầu vào của ảnh) ⇒
    #     dấu vân tay **KHÔNG** đổi. Đây là vế chứng minh danh sách ghim không còn chứa tệp công cụ.
    #     Ca này ĐỎ nếu ai đó thêm lại `build-rev.sh`/`pre-measure.sh`/`task-target.sh` vào danh sách.
    fp6="$VBREV_CALC_SHORT"
    # Tệp test/checker **không** nằm trong danh sách ghim nên mini-repo không tự sinh ra nó;
    # ca này tự tạo rồi sửa — đúng thứ cần chứng minh.
    mkdir -p "$root/v2/test/boot"
    printf '# checker gia\n' >"$root/v2/test/boot/build-rev.sh"
    git -C "$root" add -A >/dev/null 2>&1
    git -C "$root" commit -q -m "checker" >/dev/null 2>&1
    # **Fail-closed**: nếu phép tính không chạy được (ví dụ ai đó thêm lại tệp công cụ vào danh
    # sách ghim ⇒ `missing-input`), ca này phải ĐỎ. Bản đầu của ca (h) dùng `|| true` nên khi
    # đột biến nó vẫn xanh — đúng loại "xanh giả" phải tránh.
    if ! vbrev_inputs_calc "$root" >/dev/null 2>&1; then
        echo "# vbrev-selftest (h) FAIL: không tính được dấu vân tay trên cây sạch (${VBREV_CALC_ERROR:-?}) — danh sách ghim có thể chứa tệp không tồn tại"
        rc=1
    else
        fp6="$VBREV_CALC_SHORT"
        # Dấu vân tay băm **blob ở HEAD**, nên phải commit thì mới thấy thay đổi (giống ca (f)).
        printf '# them mot dong vo hai (ca h)\n' >>"$root/v2/test/boot/build-rev.sh"
        git -C "$root" add -A >/dev/null 2>&1 && git -C "$root" commit -qm "checker 1 dong" >/dev/null 2>&1
        if ! vbrev_inputs_calc "$root" >/dev/null 2>&1; then
            echo "# vbrev-selftest (h) FAIL: không tính được dấu vân tay sau khi sửa tệp test (${VBREV_CALC_ERROR:-?})"
            rc=1
        else
            fp7="$VBREV_CALC_SHORT"
            if [ "$fp6" != "$fp7" ]; then
                echo "# vbrev-selftest (h) FAIL: sửa tệp test/checker mà dấu vân tay ĐỔI ($fp6 -> $fp7) — danh sách ghim còn chứa tệp không phải đầu vào"
                rc=1
            fi
        fi
    fi
    # (i) KT-134 lượt 2: sửa **script cổng** `v2/bin/v2` ⇒ dấu vân tay **KHÔNG** đổi. Ca này ĐỎ
    #     nếu ai đó thêm lại `v2/bin` vào `VBREV_INPUT_ROOTS` — đúng lỗi đã xảy ra ở commit
    #     `4ed158b4` (chỉ sửa script cổng mà dấu vân tay đổi).
    if [ "$rc" -eq 0 ]; then
        mkdir -p "$root/v2/bin"
        printf '#!/bin/sh\n# script cong gia\n' >"$root/v2/bin/v2"
        git -C "$root" add -A >/dev/null 2>&1 && git -C "$root" commit -qm "cong" >/dev/null 2>&1
        if ! vbrev_inputs_calc "$root" >/dev/null 2>&1; then
            echo "# vbrev-selftest (i) FAIL: không tính được dấu vân tay sau khi thêm script cổng (${VBREV_CALC_ERROR:-?})"
            rc=1
        else
            fp8="$VBREV_CALC_SHORT"
            printf '#!/bin/sh\n# script cong gia, sua mot dong\n' >"$root/v2/bin/v2"
            git -C "$root" add -A >/dev/null 2>&1 && git -C "$root" commit -qm "cong 1 dong" >/dev/null 2>&1
            if ! vbrev_inputs_calc "$root" >/dev/null 2>&1; then
                echo "# vbrev-selftest (i) FAIL: không tính được dấu vân tay sau khi sửa script cổng (${VBREV_CALC_ERROR:-?})"
                rc=1
            else
                fp9="$VBREV_CALC_SHORT"
                if [ "$fp8" != "$fp9" ]; then
                    echo "# vbrev-selftest (i) FAIL: sửa script cổng mà dấu vân tay ĐỔI ($fp8 -> $fp9) — 'v2/bin' còn trong phạm vi"
                    rc=1
                fi
            fi
        fi
    fi
    # (j) KT-134 lượt 3 (luật 2 của Agent 0): sửa một tệp trong `tests/` (test tích hợp — cargo
    #     không biên dịch vào binary phát hành) ⇒ dấu vân tay **KHÔNG** đổi. Ca này ĐỎ nếu ai đó
    #     bỏ ba dòng loại `*/tests/*` khỏi `vbrev_input_allowed`.
    if [ "$rc" -eq 0 ]; then
        mkdir -p "$root/v2/kernel/k/tests"
        printf '// test tich hop gia\n' >"$root/v2/kernel/k/tests/it.rs"
        git -C "$root" add -A >/dev/null 2>&1 && git -C "$root" commit -qm "test" >/dev/null 2>&1
        if ! vbrev_inputs_calc "$root" >/dev/null 2>&1; then
            echo "# vbrev-selftest (j) FAIL: không tính được dấu vân tay sau khi thêm tệp tests/ (${VBREV_CALC_ERROR:-?})"
            rc=1
        else
            fp10="$VBREV_CALC_SHORT"
            printf '// test tich hop gia, sua mot dong\n' >"$root/v2/kernel/k/tests/it.rs"
            git -C "$root" add -A >/dev/null 2>&1 && git -C "$root" commit -qm "test 1 dong" >/dev/null 2>&1
            if ! vbrev_inputs_calc "$root" >/dev/null 2>&1; then
                echo "# vbrev-selftest (j) FAIL: không tính được dấu vân tay sau khi sửa tệp tests/ (${VBREV_CALC_ERROR:-?})"
                rc=1
            else
                fp11="$VBREV_CALC_SHORT"
                if [ "$fp10" != "$fp11" ]; then
                    echo "# vbrev-selftest (j) FAIL: sửa tệp tests/ mà dấu vân tay ĐỔI ($fp10 -> $fp11) — test tích hợp còn trong phạm vi"
                    rc=1
                fi
            fi
        fi
    fi
    # (k) KT-143: dòng `# build:` **thiếu** `algo=` hoặc `algo=` **khác** định nghĩa hiện hành ⇒
    #     cổng PHẢI TỪ CHỐI (`malformed`), kể cả khi `inputs`/`dirty` khớp hoàn hảo. Vì sao cần:
    #     đổi định nghĩa dấu vân tay làm `inputs` đổi mà không đổi byte mã nào; một log cũ có thể
    #     "tình cờ" khớp số với định nghĩa mới, và khi ấy cổng sẽ nhận một ảnh không cùng định
    #     nghĩa. Ca này ĐỎ nếu ai đó bỏ phép so `algo` trong `vbrev_parse`.
    if [ "$rc" -eq 0 ]; then
        local kbad=0
        # (k1) thiếu hẳn `algo=`
        printf '# build: inputs=%s dirty=0\n' "$fp11" >"$scratch/noalgo.log"
        printf 'ELF-gia\n# build: inputs=%s dirty=0\n' "$fp11" >"$scratch/noalgo.bin"
        gate_out="$(vbrev_gate "$scratch/noalgo.log" "$root" "$scratch/noalgo.bin" 2>&1)"
        case "$gate_out" in
            *"REFUSE reason=malformed"*) ;;
            *)
                echo "# vbrev-selftest (k1) FAIL: log THIẾU algo= mà cổng không từ chối — [$gate_out]"
                kbad=1
                ;;
        esac
        # (k2) `algo=` khác định nghĩa hiện hành
        printf '# build: algo=kt000-khong-co inputs=%s dirty=0\n' "$fp11" >"$scratch/badalgo.log"
        printf 'ELF-gia\n# build: algo=kt000-khong-co inputs=%s dirty=0\n' "$fp11" >"$scratch/badalgo.bin"
        gate_out="$(vbrev_gate "$scratch/badalgo.log" "$root" "$scratch/badalgo.bin" 2>&1)"
        case "$gate_out" in
            *"REFUSE reason=malformed"*) ;;
            *)
                echo "# vbrev-selftest (k2) FAIL: log có algo= KHÁC định nghĩa mà cổng không từ chối — [$gate_out]"
                kbad=1
                ;;
        esac
        # (k3) đối chứng dương: cùng inputs/dirty nhưng `algo=` ĐÚNG ⇒ cổng ACCEPT. Thiếu vế này
        #      thì một cổng hỏng (luôn REFUSE) cũng qua được (k1)/(k2).
        printf '# build: algo=%s inputs=%s dirty=0\n' "$VBREV_ALGO" "$fp11" >"$scratch/okalgo.log"
        printf 'ELF-gia\n# build: algo=%s inputs=%s dirty=0\n' "$VBREV_ALGO" "$fp11" >"$scratch/okalgo.bin"
        gate_out="$(vbrev_gate "$scratch/okalgo.log" "$root" "$scratch/okalgo.bin" 2>&1)"
        case "$gate_out" in
            *"ACCEPT inputs=$fp11"*) ;;
            *)
                echo "# vbrev-selftest (k3) FAIL: log có algo= ĐÚNG mà cổng không chấp nhận — [$gate_out]"
                kbad=1
                ;;
        esac
        [ "$kbad" -eq 0 ] || rc=1
    fi
    # (l) **KT-346** — sửa **một** tệp userland sản phẩm (`v2/user/programs/shell.rs`) rồi commit
    #     ⇒ dấu vân tay **PHẢI** đổi. Ca này là **phép kiểm dương** của phạm vi `v2/user`: nếu ai
    #     bỏ `v2/user` khỏi `VBREV_INPUT_ROOTS` (đột biến ii của KT-346), ca này ĐỎ — đúng khe
    #     "sửa userland mà ảnh không đổi dấu vân tay". Đo được: (l) đỏ với `kt134-v4`.
    if [ "$rc" -eq 0 ]; then
        if ! vbrev_inputs_calc "$root" >/dev/null 2>&1; then
            echo "# vbrev-selftest (l) FAIL: không tính được dấu vân tay trước khi sửa userland (${VBREV_CALC_ERROR:-?})"
            rc=1
        else
            fp12="$VBREV_CALC_SHORT"
            printf 'pub fn shell() -> u32 { 2 }\n' >"$root/v2/user/programs/shell.rs"
            git -C "$root" add -A >/dev/null 2>&1 && git -C "$root" commit -qm "userland 1 byte" >/dev/null 2>&1
            if ! vbrev_inputs_calc "$root" >/dev/null 2>&1; then
                echo "# vbrev-selftest (l) FAIL: không tính được dấu vân tay sau khi sửa userland (${VBREV_CALC_ERROR:-?})"
                rc=1
            else
                fp13="$VBREV_CALC_SHORT"
                if [ "$fp12" = "$fp13" ]; then
                    echo "# vbrev-selftest (l) FAIL: sửa `v2/user/programs/shell.rs` mà dấu vân tay KHÔNG đổi ($fp12) — 'v2/user' thiếu trong VBREV_INPUT_ROOTS"
                    rc=1
                fi
            fi
        fi
    fi
    # (m) **KT-346 / P20** — **HAI THƯ MỤC KHÁC NHAU, cùng nội dung ⇒ dấu vân tay PHẢI GIỐNG**.
    #     Đây là bất biến "danh tính bền không phụ thuộc thư mục build" (AGENTS §6.3 nợ P20): dấu
    #     vân tay băm blob **nội dung** ở HEAD, không băm đường dẫn tuyệt đối. Đột biến (i) của
    #     KT-346 (chèn đường dẫn tuyệt đối vào chuỗi băm) làm ca này ĐỎ.
    if [ "$rc" -eq 0 ]; then
        clone="$scratch/clone-a-very-long-directory-name-for-absolute-path-0123456789"
        if ! git clone -q "$root" "$clone" >/dev/null 2>&1; then
            echo "# vbrev-selftest (m) FAIL: không clone được repo mini sang thư mục thứ hai"
            rc=1
        elif ! vbrev_inputs_calc "$clone" >/dev/null 2>&1; then
            echo "# vbrev-selftest (m) FAIL: không tính được dấu vân tay ở bản sao thứ hai (${VBREV_CALC_ERROR:-?})"
            rc=1
        else
            fp14="$VBREV_CALC_SHORT"
            if [ "$fp13" != "$fp14" ]; then
                echo "# vbrev-selftest (m) FAIL: cùng nội dung, hai thư mục KHÁC nhau ⇒ dấu vân tay khác ($fp13 vs $fp14) — dấu vân tay còn phụ thuộc đường dẫn build"
                rc=1
            fi
        fi
    fi
    rm -rf "$scratch"
    if [ "$rc" -eq 0 ]; then
        echo "# vbrev-selftest: PASS (a/b/c/d/e/f/g/h/i/j/k/l/m)"
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
