#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-263 + KT-271 — kiểm ảnh chụp màn hình PPM (`screendump`) của ảnh `ktest-gpu`.

Không dùng thư viện ngoài: chỉ đọc header `P6` và ba byte mỗi pixel. Đây là **bằng chứng thật**
rằng GPU đã vẽ — không phải "lệnh gửi xong": nếu driver chỉ gửi lệnh mà không vẽ/không transfer,
ảnh chụp vẫn là nền console cũ và mọi ca dưới đây đỏ.

# Bảng font: **một** nguồn, không hai bản

Các ca console so **bitmap 8×8** của từng ký tự với `glyph()`. Bảng font được **đọc từ chính mã
nguồn kernel** (`v2/lib/vifast-virtio/src/gpu.rs`, mảng `FONT_8X8`) chứ không chép lại vào Python:
hai bản font chép tay sẽ lệch nhau **im lặng** (sửa một bên, bên kia vẫn "xanh") — đúng lớp lỗi
mà KT-271 phải chặn. Bộ đọc kiểm luôn **thứ tự** và **độ phủ** (đúng 95 glyph 32..126); sai ⇒
`exit 2`, không bao giờ "bỏ qua ca console".

Điều kiện (fail-closed; mỗi ca in `ok`/`not ok`):

1. Header `P6`, `maxval = 255`, kích thước **640×480** — đúng cỡ resource driver tạo (sau
   `SET_SCANOUT`, QEMU thay surface của console bằng chính resource đó).
2. **Bốn** điểm trong hình chữ nhật đỏ `(40,40,200,120)` = `(255,0,0)`: góc trên-trái, giữa,
   góc dưới-phải, và một điểm sát biên trong.
3. **Sáu** điểm ngoài hình chữ nhật = `(0,0,0)`: bốn hướng quanh biên + góc xa + gốc `(0,0)`.
4. Vùng chữ `"ViFast"` tại `(40,200)`: đúng **122** pixel trắng (tổng `popcount` của sáu glyph
   8×8 trong bảng font public domain) và `(40,200)` trắng / `(42,200)` đen — bit 0 bật, bit 2 tắt
   của hàng đầu glyph `'V'` (`0x33`).
5. Biên hình chữ nhật **chính xác**: `(239,159)` đỏ, `(240,159)` đen, `(239,160)` đen.
6. **KT-271 — console**: ba dòng `ViFastOs` / `gpu console` / `0123456789` tại gốc `(8,8)`, mỗi
   ký tự so **đủ 64 pixel** của ô 8×8 với `glyph()`; in `matched/total`. Thêm ca "không lem": ô
   ngay sau ký tự cuối mỗi dòng và hàng ngay dưới dòng cuối phải là nền.

Dùng:
    python3 gpu_ppm.py <tệp.ppm>              # kiểm ảnh chụp
    python3 gpu_ppm.py --make-fixture <tệp>   # dựng PPM **kỳ vọng** (fixture cho `--red-selftest`)
Exit: 0 = mọi ca đạt; 1 = có ca đỏ; 2 = không đọc được tệp / không đọc được bảng font.
"""

import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
# Bảng font **thật** của kernel — đường dẫn tính từ tệp này (`v2/test/boot/` → `v2/lib/...`).
FONT_RS = os.path.normpath(
    os.path.join(HERE, "..", "..", "lib", "vifast-virtio", "src", "gpu.rs")
)

# Hằng **thiết kế** của ảnh `ktest-gpu` (`ktest_gpu.rs`): đổi một bên phải đổi cả hai.
WIDTH = 640
HEIGHT = 480
RECT_X = 40
RECT_Y = 40
RECT_W = 200
RECT_H = 120
TEXT_X = 40
TEXT_Y = 200
TEXT = "ViFast"
# Tổng `popcount` của sáu glyph 8×8 của "ViFast" trong `vifast_virtio::gpu::FONT_8X8`.
WHITE_PIXELS = 122
# **KT-271**: gốc + ba dòng console (khớp `ktest_gpu.rs::CONSOLE_X/CONSOLE_Y/CONSOLE_LINES`).
CONSOLE_X = 8
CONSOLE_Y = 8
CONSOLE_LINES = ("ViFastOs", "gpu console", "0123456789")
GLYPH_W = 8
GLYPH_H = 8

RED = (255, 0, 0)
BLACK = (0, 0, 0)
WHITE = (255, 255, 255)

# Một dòng của `FONT_8X8`: tám byte hex + chú thích mã ASCII (`// 0x41 'A'`).
FONT_LINE = re.compile(
    r"^\s*\[((?:0x[0-9a-fA-F]{2},\s*){7}0x[0-9a-fA-F]{2})\],\s*//\s*0x([0-9a-fA-F]{2})\b"
)

FAILS = 0
CHECKS = 0


def ok(what):
    global CHECKS
    CHECKS += 1
    print(f"ok {CHECKS} - ppm: {what}")


def bad(what):
    global FAILS
    FAILS += 1
    print(f"not ok - ppm: {what}")


def load_font(path=FONT_RS):
    """Đọc `FONT_8X8` từ mã nguồn Rust. Trả `{mã: [8 byte]}`; `ValueError` nếu bảng hỏng."""
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    start = text.index("pub const FONT_8X8")
    # `];` đầu tiên sau `start` nằm **trong kiểu** (`[[u8; GLYPH_BYTES]; FONT_COUNT]`) — phải tìm
    # dấu đóng mảng ở **đầu dòng** (`\n];`) sau `= [`.
    open_at = text.index("= [", start)
    end = text.index("\n];", open_at)
    body = text[open_at:end]
    font = {}
    order = []
    for line in body.splitlines():
        match = FONT_LINE.match(line)
        if match is None:
            continue
        values = [int(value, 16) for value in re.findall(r"0x([0-9a-fA-F]{2})", match.group(1))]
        code = int(match.group(2), 16)
        if len(values) != GLYPH_H:
            raise ValueError(f"glyph 0x{code:02x} có {len(values)} byte, đòi {GLYPH_H}")
        if code in font:
            raise ValueError(f"glyph 0x{code:02x} bị lặp trong bảng")
        font[code] = values
        order.append(code)
    expected = list(range(32, 127))
    if order != expected:
        raise ValueError(
            f"bảng font sai thứ tự/độ phủ: {len(order)} glyph, đòi đúng 95 (0x20..0x7e) theo thứ tự"
        )
    return font


def glyph_pixel(bits, col, row):
    """`True` ⇔ pixel `(col, row)` của glyph bật (bit 0 = trái nhất — cùng quy ước `glyph_pixel`)."""
    return bool(bits[row] >> col & 1)


def parse_ppm(data):
    """Trả `(width, height, maxval, pixels)` hoặc `None` nếu header sai."""
    if not data.startswith(b"P6"):
        return None
    index = 2
    values = []
    while len(values) < 3:
        while index < len(data) and data[index : index + 1].isspace():
            index += 1
        if index < len(data) and data[index : index + 1] == b"#":
            while index < len(data) and data[index : index + 1] != b"\n":
                index += 1
            continue
        start = index
        while index < len(data) and not data[index : index + 1].isspace():
            index += 1
        if start == index:
            return None
        values.append(int(data[start:index]))
    index += 1  # đúng **một** byte trắng sau maxval
    width, height, maxval = values
    need = width * height * 3
    if len(data) - index < need:
        return None
    return width, height, maxval, data[index : index + need]


def check_console(get, font):
    """So **từng ký tự** của ba dòng console với bitmap 8×8; trả `(matched, total)`."""
    matched = 0
    total = 0
    for line_index, line in enumerate(CONSOLE_LINES):
        y = CONSOLE_Y + line_index * GLYPH_H
        for char_index, char in enumerate(line):
            total += 1
            bits = font[ord(char)]
            x = CONSOLE_X + char_index * GLYPH_W
            same = True
            for row in range(GLYPH_H):
                for col in range(GLYPH_W):
                    want = WHITE if glyph_pixel(bits, col, row) else BLACK
                    if get(x + col, y + row) != want:
                        same = False
                        break
                if not same:
                    break
            if same:
                matched += 1
            else:
                bad(
                    f"console: ký tự {char_index} dòng {line_index} '{char}' tại ({x},{y}) "
                    f"không khớp bitmap 8×8"
                )
    if matched == total:
        ok(f"console: {matched}/{total} ký tự khớp bitmap 8×8 (font đọc từ gpu.rs)")
    else:
        bad(f"console: chỉ {matched}/{total} ký tự khớp bitmap 8×8")
    # Không lem: ô ngay sau ký tự cuối mỗi dòng, và hàng ngay dưới dòng cuối, phải là nền.
    for line_index, line in enumerate(CONSOLE_LINES):
        y = CONSOLE_Y + line_index * GLYPH_H
        x = CONSOLE_X + len(line) * GLYPH_W
        if get(x, y) == BLACK:
            ok(f"console: ô sau ký tự cuối dòng {line_index} là nền (không lem)")
        else:
            bad(f"console: ô ({x},{y}) sau dòng {line_index} = {get(x, y)}, đòi nền")
    below = CONSOLE_Y + len(CONSOLE_LINES) * GLYPH_H
    if get(CONSOLE_X, below) == BLACK:
        ok("console: hàng ngay dưới dòng cuối là nền")
    else:
        bad(f"console: ({CONSOLE_X},{below}) = {get(CONSOLE_X, below)}, đòi nền")
    return matched, total


def main(path):
    try:
        with open(path, "rb") as handle:
            data = handle.read()
    except OSError as exc:
        print(f"# gpu_ppm: không đọc được {path}: {exc}", file=sys.stderr)
        return 2
    try:
        font = load_font()
    except (OSError, ValueError) as exc:
        print(f"# gpu_ppm: không đọc được bảng font {FONT_RS}: {exc}", file=sys.stderr)
        return 2
    parsed = parse_ppm(data)
    if parsed is None:
        bad(f"{path}: header P6/độ dài không hợp lệ")
        return 1
    width, height, maxval, pixels = parsed

    def get(x, y):
        offset = (y * width + x) * 3
        return tuple(pixels[offset : offset + 3])

    # 1) header — **dừng ngay** nếu sai cỡ/maxval: mọi phép `get(x, y)` dưới đây giả định đúng
    #    640×480×255, nên đọc tiếp sẽ `IndexError` (traceback) thay vì một ca đỏ có tên.
    if (width, height) != (WIDTH, HEIGHT):
        bad(f"kích thước {width}×{height}, đòi {WIDTH}×{HEIGHT}")
        print(f"# gpu_ppm: {CHECKS} kiểm, {FAILS} lỗi ({path})")
        return 1
    ok(f"kích thước {width}×{height} đúng cỡ resource")
    if maxval != 255:
        bad(f"maxval={maxval}, đòi 255")
        print(f"# gpu_ppm: {CHECKS} kiểm, {FAILS} lỗi ({path})")
        return 1
    ok("maxval=255")

    # 2) trong hình chữ nhật đỏ
    inside = [
        (RECT_X, RECT_Y, "góc trên-trái"),
        (RECT_X + RECT_W // 2, RECT_Y + RECT_H // 2, "giữa"),
        (RECT_X + RECT_W - 1, RECT_Y + RECT_H - 1, "góc dưới-phải"),
        (RECT_X + 1, RECT_Y + 1, "sát biên trong"),
    ]
    for x, y, label in inside:
        got = get(x, y)
        if got == RED:
            ok(f"đỏ tại ({x},{y}) {label}")
        else:
            bad(f"({x},{y}) {label} = {got}, đòi {RED}")

    # 3) ngoài hình chữ nhật
    outside = [
        (RECT_X + RECT_W, RECT_Y, "phải biên"),
        (RECT_X - 1, RECT_Y, "trái biên"),
        (RECT_X, RECT_Y + RECT_H, "dưới biên"),
        (RECT_X, RECT_Y - 1, "trên biên"),
        (WIDTH - 1, HEIGHT - 1, "góc xa"),
        (0, 0, "gốc"),
    ]
    for x, y, label in outside:
        got = get(x, y)
        if got == BLACK:
            ok(f"đen tại ({x},{y}) {label}")
        else:
            bad(f"({x},{y}) {label} = {got}, đòi {BLACK}")

    # 4) vùng chữ
    if get(TEXT_X, TEXT_Y) == WHITE:
        ok(f"trắng tại ({TEXT_X},{TEXT_Y}) — bit 0 hàng 0 của 'V'")
    else:
        bad(f"({TEXT_X},{TEXT_Y}) = {get(TEXT_X, TEXT_Y)}, đòi {WHITE}")
    if get(TEXT_X + 2, TEXT_Y) == BLACK:
        ok(f"đen tại ({TEXT_X + 2},{TEXT_Y}) — bit 2 hàng 0 của 'V'")
    else:
        bad(f"({TEXT_X + 2},{TEXT_Y}) = {get(TEXT_X + 2, TEXT_Y)}, đòi {BLACK}")
    whites = 0
    for row in range(8):
        for col in range(8 * len(TEXT)):
            if get(TEXT_X + col, TEXT_Y + row) == WHITE:
                whites += 1
    if whites == WHITE_PIXELS:
        ok(f"đúng {WHITE_PIXELS} pixel trắng trong khối chữ '{TEXT}'")
    else:
        bad(f"{whites} pixel trắng trong khối chữ '{TEXT}', đòi {WHITE_PIXELS}")

    # 5) biên chính xác
    if get(RECT_X + RECT_W - 1, RECT_Y + RECT_H - 1) == RED:
        ok("pixel cuối của hình chữ nhật đỏ")
    else:
        bad("pixel cuối của hình chữ nhật không đỏ")
    if get(RECT_X + RECT_W, RECT_Y + RECT_H - 1) == BLACK:
        ok("ngay ngoài biên phải là đen")
    else:
        bad("ngay ngoài biên phải không đen")
    if get(RECT_X + RECT_W - 1, RECT_Y + RECT_H) == BLACK:
        ok("ngay ngoài biên dưới là đen")
    else:
        bad("ngay ngoài biên dưới không đen")

    # 6) console — từng ký tự, bitmap 8×8 đọc từ bảng font của kernel.
    check_console(get, font)

    print(f"# gpu_ppm: {CHECKS} kiểm, {FAILS} lỗi ({path})")
    return 0 if FAILS == 0 else 1


def draw_text(buf, x0, y0, text, font):
    """Vẽ `text` trắng trên nền đen có sẵn (dùng cho fixture, không dùng cho phép kiểm)."""
    for index, char in enumerate(text):
        bits = font[ord(char)]
        for row in range(GLYPH_H):
            for col in range(GLYPH_W):
                if glyph_pixel(bits, col, row):
                    offset = ((y0 + row) * WIDTH + x0 + index * GLYPH_W + col) * 3
                    buf[offset : offset + 3] = bytes(WHITE)


def make_fixture(path):
    """Dựng PPM **kỳ vọng** từ cùng hằng + cùng bảng font — fixture cho `--red-selftest`.

    Đây **không** phải bằng chứng KT-263/KT-271 (không có QEMU nào chạy); nó chỉ để chứng minh
    validator **có răng**: đột biến một ký tự trong fixture phải làm nó đỏ.
    """
    try:
        font = load_font()
    except (OSError, ValueError) as exc:
        print(f"# gpu_ppm: không đọc được bảng font {FONT_RS}: {exc}", file=sys.stderr)
        return 2
    buf = bytearray(WIDTH * HEIGHT * 3)
    for y in range(RECT_Y, RECT_Y + RECT_H):
        for x in range(RECT_X, RECT_X + RECT_W):
            offset = (y * WIDTH + x) * 3
            buf[offset : offset + 3] = bytes(RED)
    draw_text(buf, TEXT_X, TEXT_Y, TEXT, font)
    for line_index, line in enumerate(CONSOLE_LINES):
        draw_text(buf, CONSOLE_X, CONSOLE_Y + line_index * GLYPH_H, line, font)
    with open(path, "wb") as handle:
        handle.write(b"P6\n%d %d\n255\n" % (WIDTH, HEIGHT))
        handle.write(bytes(buf))
    return 0


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--make-fixture":
        sys.exit(make_fixture(sys.argv[2]))
    if len(sys.argv) != 2:
        print("dùng: gpu_ppm.py <tệp.ppm> | --make-fixture <tệp.ppm>", file=sys.stderr)
        sys.exit(2)
    sys.exit(main(sys.argv[1]))
