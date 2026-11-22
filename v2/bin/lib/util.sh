#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245) — tiện ích dùng chung của cổng v2: tìm cargo/rustup/python, ghim toolchain,
# đo thời gian từng bước, `rules` (5 luật cấu trúc + E1 tên tệp NTFS).
#
# Tệp này được `source` từ `v2/bin/v2`; các hàm dưới đây giữ **nguyên văn** thân hàm cũ
# (KT-333a/KT-336b/KT-331) — tách tệp không đổi một phép kiểm nào.
# cargo có thể không nằm trên PATH (máy này: ~/.cargo/bin).
find_cargo() {
    if command -v cargo >/dev/null 2>&1; then
        command -v cargo
        return 0
    fi
    local c
    for c in "$HOME/.cargo/bin/cargo" "/usr/local/cargo/bin/cargo" "/opt/homebrew/bin/cargo"; do
        if [ -x "$c" ]; then
            echo "$c"
            return 0
        fi
    done
    return 1
}

CARGO="$(find_cargo)" || {
    echo "v2: không tìm thấy cargo (đã thử PATH, ~/.cargo/bin, /usr/local/cargo/bin, /opt/homebrew/bin)" >&2
    exit 127
}
export PATH="$(dirname "$CARGO"):$PATH"

# **KT-190** (T-INFRA 2026-11-19): ghim toolchain theo `v2/rust-toolchain.toml` cho **mọi** lệnh
# cargo của script. Trước đây script dựa vào toolchain **mặc định của máy**; khi rustup nâng
# stable 1.99.1 → 1.100.0, clippy bắt 13 lint `needless_range_loop` mới trong `vifast-fs`/
# `vifast-alloc` (mã **không đổi**) ⇒ cổng đỏ oan, và ảnh build có thể lệch toolchain so với
# `repro-build.sh` (vốn gọi `cargo +1.99.1`). Ghim để cổng + ảnh không phụ thuộc máy.
if [ -z "${RUSTUP_TOOLCHAIN:-}" ] && [ -f "$V2_ROOT/rust-toolchain.toml" ]; then
    _v2_tc="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$V2_ROOT/rust-toolchain.toml" | head -1)"
    if [ -n "$_v2_tc" ]; then
        export RUSTUP_TOOLCHAIN="$_v2_tc"
    fi
fi

# rustup có thể không nằm trên PATH (máy này: ~/.cargo/bin) — cùng cách tìm như cargo.
find_rustup() {
    if command -v rustup >/dev/null 2>&1; then
        command -v rustup
        return 0
    fi
    local c
    for c in "$HOME/.cargo/bin/rustup" "/usr/local/cargo/bin/rustup" "/opt/homebrew/bin/rustup"; do
        if [ -x "$c" ]; then
            echo "$c"
            return 0
        fi
    done
    return 1
}

find_python() {
    local c
    for c in python3.12 python3.11 python3; do
        if command -v "$c" >/dev/null 2>&1 && "$c" -c 'import tomllib' >/dev/null 2>&1; then
            command -v "$c"
            return 0
        fi
    done
    return 1
}

# ---------------------------------------------------------------- luật cấu trúc

# Tập thư mục được quét (thiết kế §8). Fixture của self-test nằm ở v2/test/rules nên
# không tự quét chính nó; `target/` luôn bị bỏ.
SCAN_DIRS="kernel lib sim user std boot test tools"
# **KT-336b (D2-232)** — R1 (≤ 1.500 dòng/tệp) mở rộng phủ `v2/sim/vifast-sim/src/` sau khi
# tách `kobj.rs` thành module theo miền. Phạm vi **có chủ ý** là `src/` của sim (không phải
# toàn bộ `v2/sim`): `v2/sim/vifast-sim/tests/` là harness kiểm, ngoài phạm vi luật cấu trúc
# sản phẩm (cùng lý do `kernel/*/tests/` nằm ngoài R1 — KT-242). `-name target -prune` vẫn
# bỏ cây build.
R1_DIRS="kernel lib sim/vifast-sim/src"

# rules <root>: in vi phạm "R<k> <đường dẫn>[:dòng]: <lý do>" (5 luật cấu trúc) và
# "E1_NTFS_* <đường dẫn>: <lý do>" (tên tệp tương thích Windows/NTFS — KT-331/D2-230);
# trả 0 nếu sạch.
relpath() {
    case "$1" in
        "$REPO_ROOT"/*) printf '%s' "${1#"$REPO_ROOT"/}" ;;
        *) printf '%s' "${1#"$2"/}" ;;
    esac
}

rules() {
    local root found=0
    root="$(cd "$1" 2>/dev/null && pwd)" || {
        echo "rules: không có thư mục $1" >&2
        return 1
    }
    # **KT-221 (T-INFRA, 2026-11-20)** — bash 3.2 (macOS) thỉnh thoảng **SIGSEGV/SIGTRAP**
    # khi `rules` chạy **sau `clippy`** trong cùng một lượt `v2 check` (đo được 1 lần
    # `Segmentation fault: 11` ở `gate --fast`; chạy rời 10/10 lượt xanh). Nghi phạm:
    # **process substitution** `< <(find …)` + vòng `while read` **lồng nhau** — mẫu đã làm
    # hỏng heap bash 3.2 ở KT-155/KT-207. Nay ghi kết quả `find`/`grep` ra **tệp tạm** rồi
    # đọc từ tệp: **không** còn process substitution. Cùng output, cùng mã thoát.
    local TMPD
    TMPD="$(mktemp -d "${TMPDIR:-/tmp}/v2rules.XXXXXX")" || {
        echo "rules: không tạo được thư mục tạm" >&2
        return 1
    }
    _rules_impl "$root" "$TMPD"
    found=$?
    rm -rf "$TMPD"
    return "$found"
}

_rules_impl() {
    local root="$1" TMPD="$2" found=0 dirs=() d f n line path E1_PY e1_rc
    # Fixture cố ý vi phạm nằm ở v2/test/fixtures — chỉ loại khi quét chính cây v2.
    local EXCL=(-not -path '/__khong_ton_tai__/*')
    if [ "$root" = "$V2_ROOT" ]; then
        EXCL=(-not -path "$V2_ROOT/test/fixtures/*")
    fi
    for d in $SCAN_DIRS; do
        [ -d "$root/$d" ] && dirs+=("$root/$d")
    done
    if [ "$root" = "$V2_ROOT" ] && [ -d "$REPO_ROOT/spec" ]; then
        dirs+=("$REPO_ROOT/spec")
    fi
    if [ "${#dirs[@]}" -eq 0 ]; then
        echo "rules: không có thư mục nào trong ($SCAN_DIRS) dưới $root" >&2
        return 1
    fi

    # R1: tệp trong kernel/, lib/ và sim/vifast-sim/src/ không quá 1.500 dòng [TK]
    # (**KT-336b**: thêm `sim/vifast-sim/src` — D2-232/D2-238; giữ nguyên ngưỡng 1.500)
    for d in $R1_DIRS; do
        [ -d "$root/$d" ] || continue
        # **KT-242 (P7)** — `-name target -prune` thay `-not -path '*/target/*'`: cùng tập tệp
        # (mọi thứ dưới thư mục tên `target` bị bỏ), nhưng **không đi vào** cây target. Đo trên
        # cây hiện tại: `find v2 -type f -not -path '*/target/*'` = **4,82 s** (442.781 tệp trong
        # `v2/target`), bản `-prune` = **0,03 s**. Không đổi phép kiểm nào.
        find "$root/$d" -name target -prune -o -type f \
            \( -name '*.rs' -o -name '*.s' -o -name '*.asm' \) \
            "${EXCL[@]}" -print0 2>/dev/null >"$TMPD/r1"
        while IFS= read -r -d '' f; do
            n=$(wc -l <"$f" | tr -d ' ')
            if [ "$n" -gt 1500 ]; then
                echo "R1 $(relpath "$f" "$root"): $n dòng > 1500"
                found=1
            fi
        done <"$TMPD/r1"
    done

    # R2: cấm cfg(target_arch) trong kernel/vifast-core và lib/ (thiết kế §8)
    for d in "$root/kernel/vifast-core" "$root/lib"; do
        [ -d "$d" ] || continue
        grep -rn --include='*.rs' 'cfg(target_arch' "$d" 2>/dev/null >"$TMPD/r2"
        while IFS= read -r line; do
            echo "R2 ${line#"$root"/}"
            found=1
        done <"$TMPD/r2"
    done

    # R3: SPDX theo loại tệp (QD §9.3)
    for d in "${dirs[@]}"; do
        find "$d" -name target -prune -o -type f "${EXCL[@]}" -print0 2>/dev/null >"$TMPD/r3"
        while IFS= read -r -d '' f; do
            case "$f" in
                *.rs | *.s | *.asm | *.ld | *.toml | *.vis | *.vish | *.vidl | *.sh | *.py)
                    if ! head -5 "$f" | grep -q 'SPDX-License-Identifier'; then
                        echo "R3 $(relpath "$f" "$root"): thiếu SPDX-License-Identifier trong 5 dòng đầu"
                        found=1
                    fi
                    ;;
                *.json | *.bin | *.txt)
                    if [ ! -f "$f.license" ]; then
                        echo "R3 $(relpath "$f" "$root"): tệp không có cú pháp chú thích cần sidecar .license"
                        found=1
                    fi
                    ;;
            esac
        done <"$TMPD/r3"
    done

    # R4: cấm path-dependency sang crates/ — v1 đã đóng băng (D2-26)
    for d in "${dirs[@]}"; do
        find "$d" -name target -prune -o -name Cargo.toml "${EXCL[@]}" -print0 2>/dev/null >"$TMPD/r4"
        while IFS= read -r -d '' f; do
            n=0
            grep -nE 'path[[:space:]]*=[[:space:]]*"[^"]*crates/' "$f" 2>/dev/null >"$TMPD/r4in"
            while IFS= read -r line; do
                n=$((n + 1))
                echo "R4 $(relpath "$f" "$root"):$n: path-dependency sang crates/ ($line)"
                found=1
            done <"$TMPD/r4in"
        done <"$TMPD/r4"
    done

    # R5: unsafe chỉ trong danh sách đóng (D2-17): vifast-sync, vifast-objmem,
    #     kernel/vifast-plat-*, boot/
    for d in "${dirs[@]}"; do
        find "$d" -name target -prune -o -name '*.rs' "${EXCL[@]}" -print0 2>/dev/null >"$TMPD/r5"
        while IFS= read -r -d '' f; do
            path="$(relpath "$f" "$root")"
            case "$f" in
                "$root"/boot/* | "$root"/kernel/vifast-plat-* | *vifast-sync* | *vifast-objmem*) continue ;;
            esac
            grep -nE '\bunsafe[[:space:]]*(fn|impl|trait|extern|\{)' "$f" 2>/dev/null |
                sed "s|^|$path:|" >"$TMPD/r5in"
            while IFS= read -r line; do
                echo "R5 $line"
                found=1
            done <"$TMPD/r5in"
        done <"$TMPD/r5"
    done

    # E1 (KT-331 / D2-230): **bước riêng, có nhãn** — tên tệp/thư mục tương thích Windows/NTFS.
    # Không gộp vào R1..R5 để mã lỗi (`E1_NTFS_*`) không lẫn với luật cấu trúc. Nguồn tên duy
    # nhất là `git ls-files` (**tệp đã theo dõi Git**) — không quét toàn đĩa (JOURNAL ~9.000
    # tệp, `target/`). Tên xấu (`[<>:"|?*]`, tên thiết bị Windows, dấu chấm/cách cuối tên, hay
    # > 255 byte) làm `git clone` trên Windows gãy (D2-229/D2-230) ⇒ phải đỏ **trước khi** vào
    # lịch sử. Ca âm/đỏ của chính máy kiểm: `v2/tools/check-filenames.py --selftest` (step 72).
    # Chỉ chạy khi quét **chính cây v2 hoặc gốc repo**: fixture của `v2/test/rules/run.sh` là
    # thư mục con để kiểm R1..R5, E1 ở đó chỉ lặp lại cùng một phép kiểm repo-wide (đo được
    # ~0,6 s/lượt ⇒ 6 lượt fixture là 3,6 s vô ích). `v2 rules --root <cây khác>` vẫn bỏ E1.
    if { [ "$root" = "$V2_ROOT" ] || [ "$root" = "$REPO_ROOT" ]; } &&
        git -C "$REPO_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        if [ -n "${V2_PY:-}" ]; then
            E1_PY="$V2_PY"
        else
            E1_PY="$(find_python 2>/dev/null || command -v python3 2>/dev/null || true)"
        fi
        if [ ! -f "$V2_ROOT/tools/check-filenames.py" ]; then
            echo "E1 tên tệp NTFS: FAIL (thiếu $V2_ROOT/tools/check-filenames.py)"
            found=1
        elif [ -z "$E1_PY" ]; then
            echo "E1 tên tệp NTFS: FAIL (cần python3 — không tìm thấy)"
            found=1
        else
            "$E1_PY" "$V2_ROOT/tools/check-filenames.py" --root "$REPO_ROOT" >"$TMPD/e1" 2>&1
            e1_rc=$?
            cat "$TMPD/e1"
            [ "$e1_rc" -eq 0 ] || found=1
        fi
    fi

    return "$found"
}

# **KT-242 (P9)** — mốc thời gian từng bước. Mỗi bước in thêm một dòng `# step-time <tên> <ms>`
# (tiền tố `#` nên mọi bộ đọc log cũ bỏ qua) và cuối `check`/`gate` in bảng tổng. Mục đích:
# cổng đã phình 18 → 69 bước trong vài ngày mà **không ai thấy** bước nào ăn bao nhiêu giây.
# Không đổi thứ tự, không đổi phép kiểm, không đổi mã thoát.
now_ms() {
    if command -v perl >/dev/null 2>&1; then
        perl -MTime::HiRes=time -e 'printf "%d", time*1000'
    else
        date +%s | awk '{printf "%d", $1*1000}'
    fi
}

# Bảng thời gian tích luỹ của lượt chạy hiện tại (mỗi dòng "ms<TAB>tên").
STEP_TIMES=""
step_time_reset() { STEP_TIMES=""; }
step_time_add() { STEP_TIMES="${STEP_TIMES}$1	$2
"; }
step_time_report() {
    [ -n "$STEP_TIMES" ] || return 0
    echo "--- bảng thời gian từng bước (ms, giảm dần)"
    printf '%s' "$STEP_TIMES" | sort -rn | awk -F'\t' '{printf "# step-time-total %8d ms  %s\n", $1, $2}'
}

run_step() { # <tên> <lệnh...>
    local name="$1"
    shift
    local t0 t1
    # **KT-242** — T1 (`gate --dev`) bỏ qua các bước chỉ kiểm harness; in rõ để không ai
    # nhầm là đã chạy. Không bỏ âm thầm: mỗi bước bỏ đều có một dòng.
    if [ "$V2_TIER" = "t1" ]; then
        case "$V2_T2_ONLY_STEPS" in
            *"$name"*)
                echo "--- $name: BỎ QUA (T1 — chỉ chạy ở T2)"
                return 0
                ;;
        esac
    fi
    t0="$(now_ms)"
    echo "--- $name"
    if "$@"; then
        t1="$(now_ms)"
        echo "--- $name: OK"
        printf '# step-time %s %s ms\n' "$name" "$((t1 - t0))"
        step_time_add "$((t1 - t0))" "$name"
        return 0
    fi
    t1="$(now_ms)"
    echo "--- $name: FAIL" >&2
    printf '# step-time %s %s ms (FAIL)\n' "$name" "$((t1 - t0))"
    step_time_add "$((t1 - t0))" "$name (FAIL)"
    return 1
}
