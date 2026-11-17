#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-84 (D2-122.6) — **ghim** một ảnh đã build vào `v2/artifacts/<sha256>.bin`.
#
# Vì sao: mọi phép đo phải trỏ vào **một tệp bất biến**, không trỏ vào `target/…` (nơi lần build
# sau ghi đè). Tên tệp **là** sha256 ⇒ không thể "ghim nhầm ảnh": đọc tên là biết nội dung.
#
# Dùng:
#   bash v2/tools/pin-artifact.sh <ảnh>            # ghim vào v2/artifacts/
#   bash v2/tools/pin-artifact.sh <ảnh> --dir DIR  # ghim vào DIR
#
# Exit: 0 = đã ghim (mới hoặc đã có, byte giống); 2 = dùng sai; 3 = ảnh không có dòng `# build:`;
#       4 = tệp đích đã tồn tại nhưng **khác** byte (không ghi đè — fail-closed).
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/.." && pwd)"
DEST_DIR="$V2_ROOT/artifacts"

die() {
    echo "pin-artifact: $*" >&2
    exit 2
}

[ "$#" -ge 1 ] || die "dùng: pin-artifact.sh <ảnh> [--dir DIR]"
IMAGE="$1"
shift
while [ "$#" -gt 0 ]; do
    case "$1" in
        --dir)
            [ "$#" -ge 2 ] || die "--dir cần tham số"
            DEST_DIR="$2"
            shift 2
            ;;
        *) die "tham số lạ: $1" ;;
    esac
done

[ -f "$IMAGE" ] || die "không thấy ảnh: $IMAGE"
mkdir -p "$DEST_DIR" || die "không tạo được $DEST_DIR"

SHA="$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
SIZE="$(wc -c <"$IMAGE" | tr -d ' ')"
DEST="$DEST_DIR/$SHA.bin"

# Ảnh phải tự khai nguồn (KT-86/KT-89): không có dòng `# build:` ⇒ không ghim.
LINE="$(LC_ALL=C grep -a -o -m1 '# build: inputs=[0-9a-f]* dirty=[01]' "$IMAGE" 2>/dev/null || true)"
[ -n "$LINE" ] || {
    echo "pin-artifact: ảnh KHÔNG có dòng '# build: inputs=… dirty=…' — không ghim (fail-closed)" >&2
    exit 3
}

if [ -f "$DEST" ]; then
    OLD="$(shasum -a 256 "$DEST" | awk '{print $1}')"
    [ "$OLD" = "$SHA" ] || {
        echo "pin-artifact: $DEST đã tồn tại nhưng KHÁC byte (sha=$OLD) — không ghi đè" >&2
        exit 4
    }
    echo "PIN sha=$SHA size=$SIZE path=$DEST $LINE state=da-co"
    exit 0
fi

cp "$IMAGE" "$DEST" || die "không chép được vào $DEST"
NEW="$(shasum -a 256 "$DEST" | awk '{print $1}')"
[ "$NEW" = "$SHA" ] || die "sau khi chép, sha đổi ($NEW != $SHA) — không dùng"
echo "PIN sha=$SHA size=$SIZE path=$DEST $LINE state=moi"
