#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-84 (D2-122.6) — `CARGO_TARGET_DIR` **riêng cho từng task**: `<repo>/target-task/<mã>`.
#
# Vì sao: hai task build song song vào cùng `target/` sẽ giẫm lên nhau (ảnh của task này bị task kia
# ghi đè giữa lúc đo). Mỗi task một thư mục ⇒ không giẫm; xoá thư mục là dọn sạch task đó.
#
# **Không** dùng cho build canonical: `v2/tools/repro-build.sh` cấm `CARGO_TARGET_DIR` nằm trong cây
# nguồn (A1-175) và tự tạo thư mục tạm. Hai đường khác nhau, có chủ ý.
#
# Dùng:
#   export CARGO_TARGET_DIR="$(bash v2/tools/task-target.sh KT-84)"
#   bash v2/tools/task-target.sh --list        # liệt kê thư mục task đang có
#   bash v2/tools/task-target.sh --clean KT-84 # xoá thư mục của một task
#
# Exit: 0 = in đường dẫn (đã tạo); 2 = dùng sai / mã không hợp lệ.
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$V2_ROOT/.." && pwd)"
BASE="$ROOT/target-task"

die() {
    echo "task-target: $*" >&2
    exit 2
}

valid_id() {
    case "${1:-}" in
        '' | *[!A-Za-z0-9._-]*) return 1 ;;
        *) return 0 ;;
    esac
}

case "${1:-}" in
    --list)
        [ -d "$BASE" ] || exit 0
        find "$BASE" -mindepth 1 -maxdepth 1 -type d -print | LC_ALL=C sort
        exit 0
        ;;
    --clean)
        [ "$#" -eq 2 ] || die "--clean cần đúng một mã task"
        valid_id "$2" || die "mã task không hợp lệ: $2"
        DIR="$BASE/$2"
        [ -d "$DIR" ] || die "không có thư mục: $DIR"
        # Chỉ xoá đúng thư mục con của target-task, không xoá gì khác.
        case "$DIR" in
            "$BASE"/*) rm -rf -- "$DIR" ;;
            *) die "đường dẫn lạ: $DIR" ;;
        esac
        echo "task-target: đã xoá $DIR"
        exit 0
        ;;
esac

[ "$#" -eq 1 ] || die "dùng: task-target.sh <mã-task> | --list | --clean <mã>"
valid_id "$1" || die "mã task không hợp lệ: $1"
DIR="$BASE/$1"
mkdir -p "$DIR" || die "không tạo được $DIR"
echo "$DIR"
