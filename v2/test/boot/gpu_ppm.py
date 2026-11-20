#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-263 — kiểm ảnh chụp màn hình PPM (`screendump`) của ảnh `ktest-gpu`.

Không dùng thư viện ngoài: chỉ đọc header `P6` và ba byte mỗi pixel. Đây là **bằng chứng thật**
rằng GPU đã vẽ — không phải "lệnh gửi xong": nếu driver chỉ gửi lệnh mà không vẽ/không transfer,
ảnh chụp vẫn là nền console cũ và mọi ca dưới đây đỏ.

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

Dùng: `python3 gpu_ppm.py <tệp.ppm>`
Exit: 0 = mọi ca đạt; 1 = có ca đỏ; 2 = không đọc được tệp.
"""

import sys

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

RED = (255, 0, 0)
BLACK = (0, 0, 0)
WHITE = (255, 255, 255)

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


def main(path):
    try:
        with open(path, "rb") as handle:
            data = handle.read()
    except OSError as exc:
        print(f"# gpu_ppm: không đọc được {path}: {exc}", file=sys.stderr)
        return 2
    parsed = parse_ppm(data)
    if parsed is None:
        bad(f"{path}: header P6/độ dài không hợp lệ")
        return 1
    width, height, maxval, pixels = parsed

    def get(x, y):
        offset = (y * width + x) * 3
        return tuple(pixels[offset : offset + 3])

    # 1) header
    if (width, height) == (WIDTH, HEIGHT):
        ok(f"kích thước {width}×{height} đúng cỡ resource")
    else:
        bad(f"kích thước {width}×{height}, đòi {WIDTH}×{HEIGHT}")
    if maxval == 255:
        ok("maxval=255")
    else:
        bad(f"maxval={maxval}, đòi 255")

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

    print(f"# gpu_ppm: {CHECKS} kiểm, {FAILS} lỗi ({path})")
    return 0 if FAILS == 0 else 1


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("dùng: gpu_ppm.py <tệp.ppm>", file=sys.stderr)
        sys.exit(2)
    sys.exit(main(sys.argv[1]))
