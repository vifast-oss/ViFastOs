#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-163 — máy kiểm "test có răng": chống lớp lỗi **xanh-giả**.

Vì sao có tệp này
-----------------
Repo đã có **hơn mười** sự cố "test xanh nhưng giả" (test không khẳng định gì, hoặc
khẳng định điều luôn đúng). Gần nhất là N7 suýt đóng sai (D2-137): chỉ reviewer độc lập
mới phát hiện, vì **không có máy kiểm** nào bắt lớp lỗi đó. Một bảng viết tay không chống
được tái diễn — tệp này là máy kiểm.

Nguyên tắc: **chỉ bắt lớp chắc chắn**, không bắt heuristic mờ. Năm lớp:

(a) **Hằng-đúng** — `assert!(true)`, `assert_eq!(x, x)` với hai vế **giống hệt** sau khi
    chuẩn hoá khoảng trắng, `assert!(1 == 1)`. Cú pháp là bằng chứng: không cần suy luận.
    `assert_ne!(x, x)` (luôn sai) bắt riêng bằng `E_CONST_FALSE` — cùng lớp suy biến.
    **Không** bắt `assert_eq!(f(seed), f(seed))`: hai vế có **lời gọi** là **ca tất định**
    (gọi hai lần phải ra cùng kết quả) — test thật, đã có trong cây (`kt20_budget.rs`,
    `kt21_donation.rs`, `kt22_mailbox.rs`). Phân biệt bằng cú pháp, không bằng tên hàm.
(b) **`#[ignore]`** — repo cấm tuyệt đối (AGENTS.md §4.3). Không có allowlist cho lớp này.
(c) **`#[test]` không có răng** — thân hàm không có token khẳng định nào
    (`assert!`, `assert_eq!`, `assert_ne!`, `assert_matches!`, `debug_assert*!`,
    `expect_err`, `unwrap_err`, `panic!` **có điều kiện**) **và** không gọi hàm trợ giúp
    đã biết. "Hàm trợ giúp đã biết" = hàm cùng tệp mà **transitive** dẫn tới một token
    khẳng định (tự dò, không cần khai báo), hoặc hàm **ở tệp khác** có trong **bản đồ
    toàn cây** (KT-170: dựng từ chính cây nguồn, phân giải theo tên và `use … as …`),
    hoặc tên có trong `HELPER_ALLOW` (đường **dự phòng** tường minh, dùng khi tên mơ hồ).
    `#[should_panic]` cũng là răng: nó khẳng định một panic. `unwrap()` **không** phải
    răng: nó chỉ chứng minh "không lỗi", không chứng minh giá trị đúng. Ca hợp lệ không
    có token ⇒ `ALLOW_NO_ASSERT` kèm lý do.
(d) **`let _ =` nuốt `Result`** — trong thân test, `let _ = <gọi hàm>;` mà hàm được gọi
    **khai báo trả `Result<…>`** (tra bảng chữ ký dựng từ chính cây nguồn, có phân giải
    `type X = Result<…>`). Không đoán theo tên: chỉ ĐỎ khi **đọc được** kiểu trả về.
    `let _ = foo().unwrap();` **không** bị bắt: `unwrap` panic khi lỗi, không nuốt.
(e) **`#[should_panic]` trần** — `#[should_panic]` **không** có `expected = "…"` ⇒ ĐỎ
    (`E_SHOULD_PANIC_BARE`): bắt **panic bất kỳ** là răng yếu — một panic không liên quan
    (tràn số, `unwrap` hỏng ở chỗ khác) vẫn làm test xanh, che mất lỗi thật. Cú pháp là
    bằng chứng: không cần suy luận. `#[should_panic(expected = "…")]` ⇒ **XANH**: khẳng
    định **đúng** panic được chờ, không phải "panic gì cũng được". `expected = ""` (khớp
    mọi thông điệp) tính là **trần**. Ca hợp lệ ⇒ `ALLOW_SHOULD_PANIC_BARE` kèm lý do.

Fail-closed
-----------
- Mục **mới** không có trong allowlist ⇒ ĐỎ. Allowlist chỉ chứa mục **đã rà** kèm lý do.
- Mục allowlist **không còn khớp** chỗ nào ⇒ ĐỎ (`E_ALLOW_STALE`) — nếu không, allowlist
  phình dần và che mất vi phạm mới.
- Bản đồ trợ giúp toàn cây chỉ tính một tên khi **mọi** định nghĩa cùng tên trong cây đều
  có răng (bắc cầu). Tên **mơ hồ** (có định nghĩa không răng) ⇒ không tính — fail-closed;
  ca thật thì khai `HELPER_ALLOW` kèm lý do, không nới luật.
- Cây không có tệp test nào ⇒ ĐỎ (`E_NO_TESTS`) — "không có gì để kiểm" không phải xanh.
- Cây không có `v2/` ⇒ ĐỎ (exit 1), không im lặng bỏ qua.

Khoá allowlist: `(tệp, tên hàm test, thứ tự trong hàm)` — **không** phải số dòng. Cùng
lựa chọn với `check-wake-paths.py` (`NON_WAKE_READY_SITES`): số dòng trôi khi sửa tệp, và
khoá theo dòng sẽ **im lặng** chuyển sang một vi phạm khác khi dòng dịch. Thông báo vẫn in
`tệp:dòng` để người đọc mở đúng chỗ; trường `line` trong allowlist chỉ để đối chiếu.
Thứ tự chỉ đếm trên **ứng viên** (đã phân giải `Result`), nên thêm một `let _ =` không
liên quan không làm trôi khoá.

Phạm vi
-------
- `v2/**/tests/*.rs` (trừ `compile-fail/`) — cả tệp là phạm vi test.
- `v2/**/src/**/*.rs` — chỉ trong khối `#[cfg(test)]` (kể cả `#[cfg(all(test, …))]`),
  dạng `mod X { … }` **hoặc** `mod X;` trỏ tới tệp khác.
- Bỏ `target/` và `v2/test/fixtures/` (fixture cố tình sai cho luật khác).

Dùng
----
    python3 v2/tools/check-test-teeth.py [--root R] [--json]
    python3 v2/tools/check-test-teeth.py --selftest

Exit: 0 = xanh; 1 = đỏ; 2 = dùng sai.
"""

from __future__ import annotations

import argparse
import bisect
import json
import re
import shutil
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Set, Tuple

# ------------------------------------------------------------------ hằng số

# Token khẳng định **mạnh**: có mặt là test có răng. `unwrap()` **không** nằm trong danh
# sách này — nó chỉ chứng minh "không lỗi", không chứng minh **giá trị đúng**; đó đúng là
# lớp xanh-giả mà tệp này bắt.
ASSERT_TOKENS: Tuple[str, ...] = (
    "assert!",
    "assert_eq!",
    "assert_ne!",
    "assert_matches!",
    "debug_assert!",
    "debug_assert_eq!",
    "debug_assert_ne!",
    "expect_err",
    "unwrap_err",
)

# `panic!` chỉ là răng khi **có điều kiện** (nằm trong nhánh `if`/`match`/vòng lặp). Một
# `panic!` vô điều kiện làm test ĐỎ, không phải xanh-giả — nhưng nó cũng không khẳng định
# gì, nên vẫn phải có control flow mới tính.
CONTROL_FLOW = re.compile(r"\b(?:if|match|else|while|for|loop)\b")

# `#[should_panic(expected = "…")]` — răng **thật**: khẳng định đúng panic được chờ.
# Nội dung chuỗi đã bị `mask_source` che thành dấu cách nhưng **độ dài giữ nguyên**, nên
# `expected = ""` (khớp **mọi** thông điệp ⇒ y như trần) vẫn phân biệt được với `"x"`:
# `""` không có ký tự nào giữa hai dấu nháy.
SHOULD_PANIC_EXPECTED_RE = re.compile(r'^should_panic\s*\(\s*expected\s*=\s*"[^"]+"')

# Lời gọi hàm, **kể cả turbofish** `tro_giup::<T>(…)`. Bản đầu chỉ khớp `ten(` nên test gọi
# trợ giúp generic bị báo oan `E_NO_ASSERT` (review L6 dựng lại trên `mmu_shared.rs`).
CALL_NAME_RE = r"\b([A-Za-z_][A-Za-z0-9_]*)\s*(?:::\s*<[^;{}()]*?>)?\s*[(!]"

# Hàm trợ giúp **ở tệp khác** mà vẫn tính là răng — **đường dự phòng tường minh**.
# Từ KT-170, lớp (c) tự dựng **bản đồ toàn cây** (xem `build_global_helpers`), nên phần lớn
# trợ giúp ở tệp khác được nhận **không cần** khai báo. Bảng này chỉ còn dùng khi tên **mơ
# hồ** (nhiều định nghĩa cùng tên, có cái không răng) hoặc trợ giúp nằm ngoài cây nguồn.
# Khoá = tên hàm; giá trị = lý do. Hiện **rỗng**.
HELPER_ALLOW: Dict[str, str] = {}

# `#[test]` **không** có token khẳng định nào — allowlist tường minh (hiện **rỗng**).
# Khoá = (tệp tương đối, tên hàm test); giá trị = (số dòng lúc khai, lý do).
# Chỉ dùng cho ca **thật sự** không khẳng định được gì bằng token: vd test chỉ đòi "không
# panic" trên đầu vào hỏng. Không dùng để né việc viết khẳng định.
ALLOW_NO_ASSERT: Dict[Tuple[str, str], Tuple[int, str]] = {}

# `#[should_panic]` **trần** (không `expected = "…"`) — allowlist tường minh (hiện **rỗng**).
# Khoá = (tệp tương đối, tên hàm test); giá trị = (số dòng lúc khai, lý do).
# Cây hiện **không** có `#[should_panic]` nào (KT-170 rà 0/977 test), nên bảng rỗng. Giữ cơ
# chế để ca hợp lệ về sau (vd panic của `core` không có thông điệp ổn định) khai tường minh
# thay vì nới luật.
ALLOW_SHOULD_PANIC_BARE: Dict[Tuple[str, str], Tuple[int, str]] = {}

# `let _ = <gọi hàm>;` trong thân test, hàm trả `Result<…>` — allowlist tường minh.
# Khoá = (tệp tương đối, tên hàm test, thứ tự 1-based trong thân hàm).
# Giá trị = (số dòng lúc khai, lý do). Số dòng chỉ để đối chiếu; khoá thật là thứ tự.
ALLOW_LET_UNDERSCORE: Dict[Tuple[str, str, int], Tuple[int, str]] = {
    # --- vifast-fs: gieo lỗi ghi / thăm dò "không panic" — lỗi **là** đầu vào của phép đo.
    ("v2/lib/vifast-fs/src/tests/part2.rs", "crash_every_write_for_mutations", 1): (
        266,
        "đếm số lần ghi của một lần chạy thăm dò; kết quả op không phải đối tượng đo",
    ),
    ("v2/lib/vifast-fs/src/tests/part2.rs", "crash_every_write_for_mutations", 2): (
        276,
        "vòng cắt điện: op được phép lỗi ở mọi điểm ghi; răng nằm ở assert kích thước sau",
    ),
    ("v2/lib/vifast-fs/src/tests/part2.rs", "crash_near_full_small_image_keeps_all_or_nothing", 1): (
        525,
        "cắt điện giữa lúc ghi: lỗi ghi là kịch bản; răng nằm ở assert size ∈ {0, 3}",
    ),
    ("v2/lib/vifast-fs/src/tests/part2.rs", "corrupt_dir_entry_length_does_not_panic", 1): (
        552,
        "ca 'không được panic' trên entry hỏng: giá trị trả không phải đối tượng đo",
    ),
    ("v2/lib/vifast-fs/src/tests/part2.rs", "corrupt_dir_entry_length_does_not_panic", 2): (
        553,
        "ca 'không được panic' trên entry hỏng: giá trị trả không phải đối tượng đo",
    ),
    ("v2/lib/vifast-fs/src/tests/part2.rs", "corrupt_dir_size_beyond_block_slots_does_not_panic", 1): (
        602,
        "ca 'không được panic' khi dir size vượt số slot: răng nằm ở assert blocks_used()",
    ),
    ("v2/lib/vifast-fs/src/tests/part2.rs", "corrupt_dir_size_beyond_block_slots_does_not_panic", 2): (
        603,
        "ca 'không được panic' khi dir size vượt số slot: răng nằm ở assert blocks_used()",
    ),
    # --- sim: vòng fuzz/property, thao tác ngẫu nhiên **được phép** lỗi.
    ("v2/sim/vifast-sim/tests/kt19_sched.rs", "mot_nghin_seed_64_op_cung_seed_cung_trace_bat_bien_giu", 1): (
        519,
        "fuzz 1.000 seed: op ngẫu nhiên được phép lỗi; răng nằm ở bất biến kiểm sau mỗi bước",
    ),
    ("v2/sim/vifast-sim/tests/kt19_sched.rs", "mot_nghin_seed_64_op_cung_seed_cung_trace_bat_bien_giu", 2): (
        522,
        "fuzz 1.000 seed: op ngẫu nhiên được phép lỗi; răng nằm ở bất biến kiểm sau mỗi bước",
    ),
    ("v2/sim/vifast-sim/tests/kt20_budget.rs", "invariant_budget_moi_chu_ky_thang_hang_khong_vuot", 1): (
        822,
        "fuzz budget: op ngẫu nhiên được phép lỗi; răng nằm ở bất biến budget sau mỗi bước",
    ),
    ("v2/sim/vifast-sim/tests/kt20_budget.rs", "invariant_budget_moi_chu_ky_thang_hang_khong_vuot", 2): (
        829,
        "fuzz budget: op ngẫu nhiên được phép lỗi; răng nằm ở bất biến budget sau mỗi bước",
    ),
    ("v2/sim/vifast-sim/tests/kt20_budget.rs", "invariant_budget_moi_chu_ky_thang_hang_khong_vuot", 3): (
        833,
        "fuzz budget: op ngẫu nhiên được phép lỗi; răng nằm ở bất biến budget sau mỗi bước",
    ),
    ("v2/sim/vifast-sim/tests/kt20_budget.rs", "invariant_budget_moi_chu_ky_thang_hang_khong_vuot", 4): (
        837,
        "fuzz budget: op ngẫu nhiên được phép lỗi; răng nằm ở bất biến budget sau mỗi bước",
    ),
    ("v2/sim/vifast-sim/tests/kt20_budget.rs", "invariant_budget_moi_chu_ky_thang_hang_khong_vuot", 5): (
        842,
        "fuzz budget: op ngẫu nhiên được phép lỗi; răng nằm ở bất biến budget sau mỗi bước",
    ),
    ("v2/sim/vifast-sim/tests/kt20_budget.rs", "invariant_budget_moi_chu_ky_thang_hang_khong_vuot", 6): (
        846,
        "fuzz budget: op ngẫu nhiên được phép lỗi; răng nằm ở bất biến budget sau mỗi bước",
    ),
    ("v2/sim/vifast-sim/tests/kt20_budget.rs", "invariant_budget_moi_chu_ky_thang_hang_khong_vuot", 7): (
        849,
        "fuzz budget: op ngẫu nhiên được phép lỗi; răng nằm ở bất biến budget sau mỗi bước",
    ),
    ("v2/sim/vifast-sim/tests/kt23_k1b.rs", "ma_tran_46_cap_va_73_cap_cam", 1): (
        196,
        "dọn slot tạm của ma trận (mỗi cặp một TCB); lỗi xoá không đổi kết luận ma trận",
    ),
    ("v2/sim/vifast-sim/tests/kt37_pagetable.rs", "map_qua_cap_loi_co_ban", 1): (
        1106,
        "thăm dò 'không panic' trên object đã chết; răng nằm ở assert InvalidCapability ở trên",
    ),
    ("v2/sim/vifast-sim/tests/kt40_tlb.rs", "property_10k_tlb_shootdown", 1): (
        804,
        "fuzz 10k vòng: translate ngẫu nhiên được phép fault; răng nằm ở check_tlb + 0 vi phạm",
    ),
    ("v2/sim/vifast-sim/tests/kt60_core_parity.rs", "kt63_parity_chuyen_cap_qua_ipc", 1): (
        3955,
        "cố ý bỏ kết quả từng op để pin **bộ đếm** sau cả kịch bản; răng nằm ở "
        "assert_eq!(cm.cap_transfer_stats(), (9, 2, 8))",
    ),
}

# Thư mục / tiền tố **không** quét.
SKIP_DIR_NAMES = {"target", "compile-fail"}
SKIP_PREFIXES = ("v2/test/fixtures/",)

# ------------------------------------------------------------------ che bình luận + chuỗi


_INTERESTING = re.compile(r"//|/\*|\"|'")


def mask_source(src: str) -> str:
    """Thay **nội dung** bình luận và chuỗi/ký tự bằng dấu cách, giữ nguyên số dòng.

    Nhờ vậy mọi phép dò token chạy trên văn bản đã che: `assert!(true)` trong doc-comment
    không bị bắt oan, mà số dòng vẫn khớp tệp gốc.

    Vòng lặp chỉ ghé các **vị trí đáng ngờ** (`//`, `/*`, `"`, `'`) do `re.finditer` tìm ở
    tầng C, không duyệt từng ký tự — trước đây duyệt từng ký tự làm riêng bước này mất ~2 s
    trên cả cây.
    """
    out = list(src)
    n = len(src)
    i = 0
    for mt in _INTERESTING.finditer(src):
        pos = mt.start()
        if pos < i:
            continue
        tok = mt.group(0)
        if tok == "//":
            j = src.find("\n", pos)
            if j < 0:
                j = n
            for k in range(pos, j):
                out[k] = " "
            i = j
        elif tok == "/*":
            depth = 1
            j = pos + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            for k in range(pos, j):
                if src[k] != "\n":
                    out[k] = " "
            i = j
        elif tok == '"':
            j = pos + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            for k in range(pos + 1, max(pos + 1, j - 1)):
                if src[k] != "\n":
                    out[k] = " "
            i = j
        else:  # "'" — ký tự hoặc lifetime
            mm = re.match(r"'(\\.|[^\\'])'", src[pos:])
            if mm:
                for k in range(pos + 1, pos + mm.end() - 1):
                    if src[k] != "\n":
                        out[k] = " "
                i = pos + mm.end()
            else:
                i = pos + 1
    return "".join(out)


# ------------------------------------------------------------------ phân tích cú pháp

FN_RE = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)")
CFG_RE = re.compile(r"#\s*\[\s*cfg\s*\(")
MOD_RE = re.compile(r"mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*([{;])")


@dataclass
class Fn:
    """Một `fn` trong tệp, toạ độ trên văn bản **đã che**."""

    name: str
    start: int
    body_start: int
    body_end: int
    attrs: List[str] = field(default_factory=list)

    @property
    def is_test(self) -> bool:
        return any(is_test_attr(a) for a in self.attrs)

    @property
    def is_should_panic(self) -> bool:
        return any(a.startswith("should_panic") for a in self.attrs)

    @property
    def should_panic_has_expected(self) -> bool:
        """`#[should_panic(expected = "…")]` với chuỗi **không rỗng** ⇒ răng thật."""
        return any(SHOULD_PANIC_EXPECTED_RE.match(a) for a in self.attrs)


def is_test_attr(attr: str) -> bool:
    """`test`, `tokio::test`, `async_std::test` — nhưng **không** `cfg(test)`."""
    return bool(re.fullmatch(r"(?:[A-Za-z_][A-Za-z0-9_]*::)*test", attr.strip()))


def collect_attrs(m: str, pos: int) -> List[str]:
    """Thu các `#[…]` liền trước `pos` (đã che bình luận nên chỉ cần bỏ khoảng trắng)."""
    attrs: List[str] = []
    i = pos
    while True:
        j = i
        while j > 0 and m[j - 1] in " \t\n":
            j -= 1
        if j == 0 or m[j - 1] != "]":
            break
        depth = 0
        k = j - 1
        while k >= 0:
            if m[k] == "]":
                depth += 1
            elif m[k] == "[":
                depth -= 1
                if depth == 0:
                    break
            k -= 1
        if k < 0:
            break
        h = k - 1
        while h >= 0 and m[h] in " \t\n":
            h -= 1
        if h < 0 or m[h] != "#":
            break
        attrs.append(m[k + 1 : j - 1].strip())
        i = h
    attrs.reverse()
    return attrs


def _match_brace(m: str, open_pos: int) -> int:
    """Vị trí `}` khớp với `{` ở `open_pos`; -1 nếu không khớp.

    Nhảy bằng `str.find` (tầng C) thay vì duyệt từng ký tự — cùng kết quả, nhanh hơn nhiều
    trên tệp lớn.
    """
    depth = 0
    k = open_pos
    n = len(m)
    while k < n:
        nxt_o = m.find("{", k)
        nxt_c = m.find("}", k)
        if nxt_c < 0:
            return -1
        if 0 <= nxt_o < nxt_c:
            depth += 1
            k = nxt_o + 1
        else:
            depth -= 1
            if depth == 0:
                return nxt_c
            k = nxt_c + 1
    return -1


def find_functions(m: str) -> List[Fn]:
    """Mọi `fn` (kể cả lồng nhau, kể cả method) với toạ độ thân hàm."""
    out: List[Fn] = []
    for mt in FN_RE.finditer(m):
        name = mt.group(1)
        j = mt.end()
        depth = 0
        found = False
        while j < len(m):
            ch = m[j]
            if ch == "-" and j + 1 < len(m) and m[j + 1] == ">":
                j += 2
                continue
            if ch in "([<":
                depth += 1
            elif ch in ")]>":
                depth -= 1
            elif ch == "{" and depth <= 0:
                found = True
                break
            elif ch == ";" and depth <= 0:
                break
            j += 1
        if not found:
            continue
        k = _match_brace(m, j)
        if k < 0:
            continue
        out.append(
            Fn(
                name=name,
                start=mt.start(),
                body_start=j,
                body_end=k,
                attrs=collect_attrs(m, mt.start()),
            )
        )
    return out


def top_level_fns(fns: Sequence[Fn]) -> List[Fn]:
    """Bỏ hàm **lồng trong** hàm khác (vd `fn shape()` khai trong thân một `#[test]`)."""
    out = []
    for f in fns:
        if any(g.body_start < f.start < g.body_end for g in fns if g is not f):
            continue
        out.append(f)
    return out


def parse_cfg_mods(m: str) -> List[Tuple[str, str, Optional[Tuple[int, int]]]]:
    """Các `#[cfg(…test…)] mod X { … }` / `mod X;` — trả (kind, tên, khoảng).

    `kind` = `"inline"` (kèm khoảng thân) hoặc `"file"` (module ở tệp khác).
    """
    out: List[Tuple[str, str, Optional[Tuple[int, int]]]] = []
    for mt in CFG_RE.finditer(m):
        i = mt.end() - 1  # tại '('
        depth = 0
        j = i
        while j < len(m):
            if m[j] == "(":
                depth += 1
            elif m[j] == ")":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        if j >= len(m):
            continue
        cfg_text = m[i + 1 : j]
        if not re.search(r"\btest\b", cfg_text):
            continue
        if re.search(r"\bnot\s*\(\s*test\s*\)", cfg_text):
            continue
        k = j + 1
        while k < len(m) and m[k] in " \t\n":
            k += 1
        if k >= len(m) or m[k] != "]":
            continue
        k += 1
        # bỏ qua các thuộc tính khác giữa `#[cfg(…)]` và `mod`
        while True:
            while k < len(m) and m[k] in " \t\n":
                k += 1
            if k < len(m) and m[k] == "#":
                k2 = k + 1
                while k2 < len(m) and m[k2] in " \t\n":
                    k2 += 1
                if k2 < len(m) and m[k2] == "[":
                    d2 = 0
                    k3 = k2
                    while k3 < len(m):
                        if m[k3] == "[":
                            d2 += 1
                        elif m[k3] == "]":
                            d2 -= 1
                            if d2 == 0:
                                break
                        k3 += 1
                    k = k3 + 1
                    continue
            break
        mm = MOD_RE.match(m, k)
        if not mm:
            continue
        name = mm.group(1)
        if mm.group(2) == ";":
            out.append(("file", name, None))
        else:
            b = mm.end() - 1
            d = 0
            e = b
            while e < len(m):
                if m[e] == "{":
                    d += 1
                elif m[e] == "}":
                    d -= 1
                    if d == 0:
                        break
                e += 1
            out.append(("inline", name, (b, e)))
    return out


# ------------------------------------------------------------------ phạm vi test


def iter_rs_files(root: Path) -> List[Path]:
    out: List[Path] = []
    base = root / "v2"
    if not base.is_dir():
        return out
    for p in sorted(base.rglob("*.rs")):
        rel = str(p.relative_to(root))
        if any(part in SKIP_DIR_NAMES for part in p.relative_to(root).parts):
            continue
        if any(rel.startswith(pre) for pre in SKIP_PREFIXES):
            continue
        out.append(p)
    return out


@dataclass
class ParsedFile:
    """Một tệp đã che bình luận/chuỗi + danh sách `fn` — **phân tích một lần**, dùng lại.

    Trước khi gom vào đây, mỗi tệp bị che và quét ba lần (phạm vi, kiểu trả về, kiểm) và
    `str.count("\\n", 0, pos)` là O(n) cho **mỗi** assert ⇒ quét cây mất ~12 s. Đo lại sau
    khi gom: xem `v2/agent/JOURNAL/kt163-teeth.md`.
    """

    path: Path
    rel: str
    masked: str
    fns: List[Fn]
    line_starts: List[int]

    def line_of(self, pos: int) -> int:
        return bisect.bisect_right(self.line_starts, pos)


def parse_file(p: Path, root: Path) -> Optional[ParsedFile]:
    try:
        src = p.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    m = mask_source(src)
    starts = [0]
    idx = m.find("\n")
    while idx >= 0:
        starts.append(idx + 1)
        idx = m.find("\n", idx + 1)
    return ParsedFile(
        path=p,
        rel=str(p.relative_to(root)),
        masked=m,
        fns=find_functions(m),
        line_starts=starts,
    )


def build_scope(
    root: Path, parsed: Sequence["ParsedFile"]
) -> Tuple[Set[Path], Dict[Path, List[Tuple[int, int]]]]:
    """Trả (tệp thuộc phạm vi test toàn phần, bản đồ khoảng `#[cfg(test)]` nội tuyến)."""
    whole: Set[Path] = set()
    inline: Dict[Path, List[Tuple[int, int]]] = {}
    for pf in parsed:
        for kind, name, rng in parse_cfg_mods(pf.masked):
            if kind == "inline" and rng is not None:
                inline.setdefault(pf.path, []).append(rng)
            elif kind == "file":
                for cand in (pf.path.parent / f"{name}.rs", pf.path.parent / name / "mod.rs"):
                    if cand.exists():
                        whole.add(cand)
    for pf in parsed:
        parts = pf.path.relative_to(root).parts
        if "tests" in parts[:-1]:
            whole.add(pf.path)
    return whole, inline


def in_ranges(pos: int, ranges: Sequence[Tuple[int, int]]) -> bool:
    return any(a <= pos <= b for a, b in ranges)


# ------------------------------------------------------------------ bảng kiểu trả về

RET_ALIAS_RE = re.compile(r"\btype\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^;=]*>)?\s*=\s*([^;]+);")


def build_return_types(
    parsed: Sequence["ParsedFile"],
) -> Tuple[Dict[str, Set[str]], Dict[str, str]]:
    """Bảng `tên hàm -> các kiểu trả về đã khai` và `tên alias -> kiểu`."""
    rets: Dict[str, Set[str]] = {}
    aliases: Dict[str, str] = {}
    for pf in parsed:
        m = pf.masked
        for mt in RET_ALIAS_RE.finditer(m):
            aliases[mt.group(1)] = " ".join(mt.group(2).split())
        for f in pf.fns:
            sig = m[f.start : f.body_start]
            mm = re.search(r"->\s*(.+)$", sig, re.S)
            if not mm:
                continue
            rt = " ".join(mm.group(1).split())
            rt = re.sub(r"\bwhere\b.*$", "", rt).strip()
            if rt:
                rets.setdefault(f.name, set()).add(rt)
    return rets, aliases


def is_result_type(rt: str, aliases: Dict[str, str], depth: int = 0) -> bool:
    """Kiểu trả về có phải `Result<…>` (trực tiếp hoặc qua alias)?"""
    rt = rt.strip()
    if not rt or depth > 4:
        return False
    if re.match(r"^(?:[A-Za-z_][A-Za-z0-9_]*::)*Result\b", rt):
        return True
    mm = re.fullmatch(r"(?:[A-Za-z_][A-Za-z0-9_]*::)*([A-Za-z_][A-Za-z0-9_]*)", rt)
    if mm and mm.group(1) in aliases:
        return is_result_type(aliases[mm.group(1)], aliases, depth + 1)
    return False


# ------------------------------------------------------------------ bản đồ trợ giúp toàn cây

# `use …::ten as biet_danh;` — chỉ cần cặp (bí danh, tên gốc). `use x::*` (glob) không phân
# giải được nên bỏ qua: bỏ qua chỉ làm **đỏ oan** (an toàn), không làm xanh giả.
USE_ALIAS_RE = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s+as\s+([A-Za-z_][A-Za-z0-9_]*)")


def build_use_aliases(
    parsed: Sequence["ParsedFile"], scope: Set[Path]
) -> Dict[Path, Dict[str, str]]:
    """Bảng `bí danh -> tên gốc` theo từng tệp **trong phạm vi test**, từ các câu `use`.

    Chỉ tệp trong phạm vi test mới cần: bảng này dùng để phân giải tên gọi **trong thân
    test**. Quét cả cây là phí (295 tệp so với 127).
    """
    out: Dict[Path, Dict[str, str]] = {}
    for pf in parsed:
        if pf.path not in scope:
            continue
        aliases: Dict[str, str] = {}
        for mt in re.finditer(r"\buse\s+([^;]+);", pf.masked):
            for am in USE_ALIAS_RE.finditer(mt.group(1)):
                aliases[am.group(2)] = am.group(1)
        if aliases:
            out[pf.path] = aliases
    return out


def build_global_helpers(parsed: Sequence["ParsedFile"]) -> Set[str]:
    """Tên hàm **không phải** `#[test]` mà **mọi** định nghĩa cùng tên trong cây đều có răng.

    "Có răng" = thân có token khẳng định (hoặc `panic!` **có điều kiện**), **hoặc** gọi một
    tên đã có răng — bao đóng bắc cầu tới điểm bất động. Đây là bản đồ **toàn cây** (KT-170),
    nên test gọi trợ giúp ở **tệp khác** được tính là có răng mà **không cần** `HELPER_ALLOW`.

    Vì sao đòi **mọi** định nghĩa cùng tên: tên trùng giữa các tệp là chuyện thường (`kiem`,
    `setup`…). Nếu chỉ cần **một** định nghĩa có răng thì một test gọi trợ giúp **không** răng
    trùng tên sẽ được xanh oan. Đòi tất cả là fail-closed: tên mơ hồ ⇒ không tính, người viết
    khai `HELPER_ALLOW` nếu ca thật.

    Hàm `#[test]` bị loại khỏi bản đồ: test không phải trợ giúp, và gọi chéo test là mùi lạ.
    """
    direct: Dict[str, List[bool]] = {}
    calls: Dict[str, List[Set[str]]] = {}
    for pf in parsed:
        m = pf.masked
        for f in pf.fns:
            if f.is_test:
                continue
            body = m[f.body_start : f.body_end]
            has = any(t in body for t in ASSERT_TOKENS) or (
                "panic!" in body and bool(CONTROL_FLOW.search(body))
            )
            direct.setdefault(f.name, []).append(has)
            calls.setdefault(f.name, []).append(set(re.findall(CALL_NAME_RE, body)))
    teeth: Set[str] = {n for n, ds in direct.items() if all(ds)}
    changed = True
    while changed:
        changed = False
        for n, ds in direct.items():
            if n in teeth:
                continue
            ok = True
            for i, has in enumerate(ds):
                if has:
                    continue
                if not any(c in teeth for c in calls[n][i]):
                    ok = False
                    break
            if ok:
                teeth.add(n)
                changed = True
    return teeth


# ------------------------------------------------------------------ phân tích biểu thức

# Phương thức **tiêu thụ** `Result` (panic/đổi sang giá trị khác) — không phải "nuốt".
CONSUMING_METHODS = {
    "unwrap",
    "expect",
    "unwrap_err",
    "expect_err",
    "is_ok",
    "is_err",
    "ok",
    "err",
    "unwrap_or",
    "unwrap_or_default",
    "unwrap_or_else",
    "unwrap_or_insert",
    "ok_or",
    "ok_or_else",
    "into_ok",
    "into_err",
    "expect_none",
    "unwrap_unchecked",
}


def _match_paren_back(text: str, close: int) -> int:
    """Vị trí `(` khớp với `)` ở `close`; -1 nếu không khớp."""
    depth = 0
    i = close
    while i >= 0:
        if text[i] == ")":
            depth += 1
        elif text[i] == "(":
            depth -= 1
            if depth == 0:
                return i
        i -= 1
    return -1


def callee_of(rhs: str) -> Optional[str]:
    """Tên hàm được gọi ở **ngoài cùng** của biểu thức `rhs`, nếu có.

    Trả `None` cho: không phải lời gọi, macro (`foo!`), tuple/ngoặc, `?`-lan truyền.
    """
    r = rhs.strip()
    if not r or r.endswith("?"):
        return None
    if not r.endswith(")"):
        return None
    i = _match_paren_back(r, len(r) - 1)
    if i < 0:
        return None
    before = r[:i].rstrip()
    if not before:
        # `(foo())` — bóc ngoặc rồi thử lại
        inner = r[1:-1].strip()
        return callee_of(inner) if inner != r else None
    if before.endswith("!"):
        return None  # macro
    # turbofish `foo::<T>(…)`
    if before.endswith(">"):
        depth = 0
        k = len(before) - 1
        while k >= 0:
            if before[k] == ">":
                depth += 1
            elif before[k] == "<":
                depth -= 1
                if depth == 0:
                    break
            k -= 1
        if k < 0:
            return None
        before = before[:k].rstrip()
        if before.endswith("::"):
            before = before[:-2].rstrip()
    mm = re.search(r"([A-Za-z_][A-Za-z0-9_]*)$", before)
    if not mm:
        return None
    name = mm.group(1)
    if name in CONSUMING_METHODS:
        return None
    return name


def split_macro_args(text: str) -> List[str]:
    """Tách tham số macro ở dấu phẩy **mức ngoài cùng** (chuỗi đã bị che).

    Chỉ đếm `()[]{}` — **không** đếm `<`/`>`: trong biểu thức Rust chúng là toán tử so
    sánh (`assert_eq!(a > b, c)`), không phải ngoặc. Turbofish nhiều tham số
    (`::<A, B>`) vì thế có thể bị tách sai — nhưng tách sai chỉ làm hai vế **khác nhau**
    ⇒ không báo, tức là bỏ sót (an toàn), không báo oan.
    """
    args: List[str] = []
    depth = 0
    cur: List[str] = []
    for ch in text:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        if ch == "," and depth == 0:
            args.append("".join(cur))
            cur = []
            continue
        cur.append(ch)
    tail = "".join(cur)
    if tail.strip():
        args.append(tail)
    return args


def norm(expr: str) -> str:
    return " ".join(expr.split())


# Một **lời gọi** (hàm/macro) trong biểu thức: định danh (hoặc `)`/`]`) rồi `(`/`!`.
# Dùng để phân biệt `assert_eq!(x, x)` (hằng-đúng: hai vế là **cùng một biểu thức không
# gọi gì**) với `assert_eq!(f(seed), f(seed))` (**ca tất định**: gọi hai lần phải ra cùng
# kết quả — đây là test thật, không phải xanh-giả).
CALL_RE = re.compile(r"[A-Za-z0-9_\]\)]\s*[(!]")


def has_call(expr: str) -> bool:
    return bool(CALL_RE.search(expr))


MACRO_CALL_RE = re.compile(r"\b(assert|assert_eq|assert_ne|debug_assert|debug_assert_eq|debug_assert_ne)\s*!")


def find_macro_calls(m: str, start: int, end: int) -> Iterable[Tuple[str, int, List[str]]]:
    """Các lời gọi macro khẳng định trong `[start, end)`: (tên, vị trí, tham số)."""
    for mt in MACRO_CALL_RE.finditer(m, start, end):
        j = mt.end()
        while j < end and m[j] in " \t\n":
            j += 1
        if j >= end or m[j] not in "([{":
            continue
        opener = m[j]
        closer = {"(": ")", "[": "]", "{": "}"}[opener]
        depth = 0
        k = j
        while k < end:
            if m[k] == opener:
                depth += 1
            elif m[k] == closer:
                depth -= 1
                if depth == 0:
                    break
            k += 1
        if k >= end:
            continue
        yield mt.group(1), mt.start(), split_macro_args(m[j + 1 : k])


# ------------------------------------------------------------------ kết quả


@dataclass
class Report:
    errors: List[str] = field(default_factory=list)
    warnings: List[str] = field(default_factory=list)
    info: List[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.errors


# ------------------------------------------------------------------ quét


def scan(
    root: Path,
    allow_let: Optional[Dict[Tuple[str, str, int], Tuple[int, str]]] = None,
    helper_allow: Optional[Dict[str, str]] = None,
    allow_no_assert: Optional[Dict[Tuple[str, str], Tuple[int, str]]] = None,
    allow_should_panic: Optional[Dict[Tuple[str, str], Tuple[int, str]]] = None,
) -> Report:
    allow_let = ALLOW_LET_UNDERSCORE if allow_let is None else allow_let
    helper_allow = HELPER_ALLOW if helper_allow is None else helper_allow
    allow_no_assert = ALLOW_NO_ASSERT if allow_no_assert is None else allow_no_assert
    allow_should_panic = (
        ALLOW_SHOULD_PANIC_BARE if allow_should_panic is None else allow_should_panic
    )
    rep = Report()

    files = iter_rs_files(root)
    if not files:
        rep.errors.append("E_NO_TREE v2/: không có tệp .rs nào — fail-closed")
        return rep

    parsed: List[ParsedFile] = []
    for p in files:
        pf = parse_file(p, root)
        if pf is None:  # pragma: no cover - chỉ khi quyền hỏng
            rep.errors.append(f"E_IO {p}: không đọc được")
            continue
        parsed.append(pf)

    whole, inline = build_scope(root, parsed)
    rets, aliases = build_return_types(parsed)
    # KT-170: bản đồ trợ giúp **toàn cây** (lớp (c) không còn phụ thuộc `HELPER_ALLOW`) và
    # bảng bí danh `use … as …` để phân giải tên gọi.
    g_teeth = build_global_helpers(parsed)
    use_aliases = build_use_aliases(parsed, whole | set(inline))

    used_allow: Set[Tuple[str, str, int]] = set()
    used_no_assert: Set[Tuple[str, str]] = set()
    used_should_panic: Set[Tuple[str, str]] = set()
    n_tests = 0
    n_files = 0
    n_global_used = 0

    for pf in parsed:
        p = pf.path
        rel = pf.rel
        m = pf.masked

        # (b) `#[ignore]` — cấm toàn cây, không allowlist.
        for mt in re.finditer(r"#\s*\[\s*ignore\b", m):
            line = pf.line_of(mt.start())
            rep.errors.append(
                f"E_IGNORE {rel}:{line}: `#[ignore]` bị cấm (AGENTS.md §4.3) — "
                "sửa test hoặc bỏ, không né"
            )

        is_whole = p in whole
        ranges = inline.get(p, [])
        if not is_whole and not ranges:
            continue
        n_files += 1

        fns = pf.fns
        top = top_level_fns(fns)

        # Bản đồ trợ giúp: hàm **không phải** `#[test]` mà thân có token khẳng định.
        helper_teeth: Dict[str, bool] = {}
        for f in fns:
            if f.is_test:
                continue
            body = m[f.body_start : f.body_end]
            helper_teeth[f.name] = any(t in body for t in ASSERT_TOKENS) or (
                "panic!" in body and bool(CONTROL_FLOW.search(body))
            )
        # Bao đóng bắc cầu: trợ giúp gọi trợ giúp.
        calls_of: Dict[str, Set[str]] = {}
        for f in fns:
            body = m[f.body_start : f.body_end]
            # **KT-163b** (review L6): phải nhận cả turbofish `tro_giup::<T>(…)` — nếu không,
            # test gọi trợ giúp generic bị báo oan `E_NO_ASSERT` (đã dựng lại trên
            # `vifast-plat-common/tests/mmu_shared.rs`).
            calls_of[f.name] = set(re.findall(CALL_NAME_RE, body))

        def teeth_of(name: str, seen: Optional[Set[str]] = None) -> bool:
            seen = set() if seen is None else seen
            if name in seen:
                return False
            seen.add(name)
            if helper_teeth.get(name):
                return True
            for c in calls_of.get(name, ()):
                if c in helper_teeth and teeth_of(c, seen):
                    return True
            return False

        # (a) hằng-đúng / hằng-sai — quét **toàn phạm vi test** (kể cả hàm trợ giúp), không
        # chỉ thân `#[test]`: một `assert!(true)` nằm trong trợ giúp cũng là xanh-giả.
        scope_ranges = [(0, len(m))] if is_whole else ranges
        for lo, hi in scope_ranges:
            for macro, pos, args in find_macro_calls(m, lo, hi):
                line = pf.line_of(pos)
                if macro in ("assert", "debug_assert"):
                    if len(args) >= 1 and norm(args[0]) == "true":
                        rep.errors.append(
                            f"E_CONST_TRUE {rel}:{line}: `{macro}!(true)` — khẳng định hằng-đúng, "
                            "test không có răng"
                        )
                        continue
                    if len(args) >= 1:
                        mm = re.fullmatch(r"(\d+)\s*==\s*(\d+)", norm(args[0]))
                        if mm and mm.group(1) == mm.group(2):
                            rep.errors.append(
                                f"E_CONST_TRUE {rel}:{line}: `{macro}!({norm(args[0])})` — "
                                "so sánh hai hằng giống hệt, test không có răng"
                            )
                elif macro in ("assert_eq", "debug_assert_eq", "assert_ne", "debug_assert_ne"):
                    if len(args) < 2:
                        continue
                    a, b = norm(args[0]), norm(args[1])
                    if a != b:
                        continue
                    # Hai vế giống hệt **và** không vế nào gọi gì ⇒ hằng-đúng/hằng-sai.
                    # Có lời gọi ⇒ ca tất định (`assert_eq!(f(seed), f(seed))`) — test thật.
                    if has_call(a):
                        continue
                    code = "E_CONST_FALSE" if "ne" in macro else "E_CONST_TRUE"
                    rep.errors.append(
                        f"{code} {rel}:{line}: `{macro}!({a}, {b})` — hai vế giống hệt, "
                        "test không có răng"
                    )

        for f in top:
            if not f.is_test:
                continue
            if not (is_whole or in_ranges(f.start, ranges)):
                continue
            n_tests += 1
            body = m[f.body_start : f.body_end]
            # Thân **riêng**: bỏ thân hàm lồng, để assert của `fn shape()` không tính hộ.
            own = list(body)
            for g in fns:
                if g is f:
                    continue
                if f.body_start < g.start < f.body_end:
                    a = g.body_start - f.body_start
                    b = g.body_end - f.body_start
                    for k in range(max(0, a), min(len(own), b + 1)):
                        if own[k] != "\n":
                            own[k] = " "
            own_s = "".join(own)

            # (c) không có răng
            has_token = any(t in own_s for t in ASSERT_TOKENS)
            if not has_token and "panic!" in own_s and CONTROL_FLOW.search(own_s):
                has_token = True
            if not has_token and f.is_should_panic:
                has_token = True
            if not has_token:
                called = set(re.findall(CALL_NAME_RE, own_s))
                # KT-170: bí danh `use … as …` của **tệp này** — `kt(3)` phải tra ra `kiem`.
                file_aliases = use_aliases.get(p, {})
                called |= {file_aliases[c] for c in called if c in file_aliases}
                if any(c in helper_allow for c in called):
                    has_token = True
                elif any(c in helper_teeth and teeth_of(c) for c in called):
                    has_token = True
                elif any(c in g_teeth for c in called):
                    # Trợ giúp ở **tệp khác** (bản đồ toàn cây) — không cần `HELPER_ALLOW`.
                    has_token = True
                    n_global_used += 1
            if not has_token:
                key_na = (rel, f.name)
                if key_na in allow_no_assert:
                    used_no_assert.add(key_na)
                else:
                    rep.errors.append(
                        f"E_NO_ASSERT {rel}:{pf.line_of(f.start)}: `#[test] fn {f.name}` không có "
                        "token khẳng định nào và không gọi trợ giúp đã biết — test xanh-giả"
                    )

            # (e) `#[should_panic]` **trần** (KT-170): bắt panic bất kỳ là răng yếu — một panic
            # không liên quan vẫn làm test xanh. `expected = "…"` (chuỗi khác rỗng) là răng thật.
            if f.is_should_panic and not f.should_panic_has_expected:
                key_sp = (rel, f.name)
                if key_sp in allow_should_panic:
                    used_should_panic.add(key_sp)
                else:
                    rep.errors.append(
                        f"E_SHOULD_PANIC_BARE {rel}:{pf.line_of(f.start)}: `#[test] fn {f.name}` "
                        "có `#[should_panic]` **không** `expected = \"…\"` — bắt panic bất kỳ là "
                        "răng yếu, dễ che lỗi khác; thêm `expected` hoặc allowlist kèm lý do"
                    )

            # (d) `let _ =` nuốt Result. Thứ tự chỉ đếm trên **ứng viên** (đã phân giải
            # `Result`), để thêm một `let _ =` không-Result không làm trôi khoá allowlist.
            ordinal = 0
            for mt in re.finditer(r"\blet\s+_\s*=\s*([^;]*);", body):
                name = callee_of(mt.group(1))
                if name is None:
                    continue
                rts = rets.get(name)
                if not rts:
                    continue
                if not any(is_result_type(rt, aliases) for rt in rts):
                    continue
                ordinal += 1
                line = pf.line_of(f.body_start + mt.start())
                key = (rel, f.name, ordinal)
                if key in allow_let:
                    used_allow.add(key)
                    continue
                rep.errors.append(
                    f"E_LET_UNDERSCORE {rel}:{line}: `let _ = {name}(…)` nuốt `Result` "
                    f"(kiểu trả về: {sorted(rts)[0]}) — kiểm kết quả hoặc thêm allowlist kèm lý do"
                )

    # Allowlist mục ⇒ ĐỎ (fail-closed): mục không còn khớp chỗ nào là mục chết.
    for key in sorted(allow_let):
        if key in used_allow:
            continue
        rel, fname, ordinal = key
        line = allow_let[key][0]
        rep.errors.append(
            f"E_ALLOW_STALE {rel}:{line}: mục allowlist `let _ =` ({fname}, #{ordinal}) "
            "không còn khớp chỗ nào — gỡ mục hoặc sửa khoá"
        )

    for key in sorted(allow_no_assert):
        if key in used_no_assert:
            continue
        rel, fname = key
        line = allow_no_assert[key][0]
        rep.errors.append(
            f"E_ALLOW_STALE {rel}:{line}: mục allowlist `#[test]` không khẳng định ({fname}) "
            "không còn khớp chỗ nào — gỡ mục hoặc sửa khoá"
        )

    for key in sorted(allow_should_panic):
        if key in used_should_panic:
            continue
        rel, fname = key
        line = allow_should_panic[key][0]
        rep.errors.append(
            f"E_ALLOW_STALE {rel}:{line}: mục allowlist `#[should_panic]` trần ({fname}) "
            "không còn khớp chỗ nào — gỡ mục hoặc sửa khoá"
        )

    if n_tests == 0:
        rep.errors.append(
            "E_NO_TESTS v2/: không tìm thấy `#[test]` nào trong phạm vi — fail-closed"
        )

    rep.info.append(f"tệp có phạm vi test: {n_files}")
    rep.info.append(f"hàm `#[test]`: {n_tests}")
    rep.info.append(f"allowlist `let _ =`: {len(allow_let)} mục, khớp {len(used_allow)}")
    rep.info.append(f"trợ giúp khai báo ngoài tệp: {len(helper_allow)}")
    rep.info.append(
        f"bản đồ trợ giúp toàn cây: {len(g_teeth)} tên có răng, "
        f"{n_global_used} test dùng đường này"
    )
    rep.info.append(
        f"allowlist `#[test]` không khẳng định: {len(allow_no_assert)} mục, "
        f"khớp {len(used_no_assert)}"
    )
    rep.info.append(
        f"allowlist `#[should_panic]` trần: {len(allow_should_panic)} mục, "
        f"khớp {len(used_should_panic)}"
    )
    return rep


# ------------------------------------------------------------------ tự kiểm

_FIXTURE_CLEAN = """\
// SPDX-License-Identifier: MIT OR Apache-2.0
fn cong(a: u32, b: u32) -> u32 {
    a + b
}

#[test]
fn cong_dung() {
    assert_eq!(cong(1, 2), 3);
}
"""


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _fixture(root: Path, files: Dict[str, str]) -> None:
    for rel, text in files.items():
        _write(root / rel, text)


def selftest() -> int:
    """Ba mươi mốt ca: mỗi ca một dòng `ok`/`not ok`."""
    cases: List[Tuple[str, bool, str]] = []
    tmp = Path(tempfile.mkdtemp(prefix="kt163-selftest-"))
    try:
        # (1) cây sạch ⇒ xanh (đối chứng dương: máy kiểm không phải luôn-đỏ).
        d = tmp / "clean"
        _fixture(d, {"v2/lib/a/tests/t.rs": _FIXTURE_CLEAN})
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(("cây sạch ⇒ xanh", rep.ok, "; ".join(rep.errors)))

        # (2) `assert!(true)` ⇒ đỏ.
        d = tmp / "const-true"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": "#[test]\nfn gia() {\n    assert!(true);\n}\n",
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`assert!(true)` ⇒ đỏ",
                (not rep.ok) and any("E_CONST_TRUE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (3) `assert_eq!(x, x)` hai vế giống hệt ⇒ đỏ.
        d = tmp / "const-eq"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": (
                    "#[test]\nfn gia() {\n    let x = 1;\n    assert_eq!(x, x);\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`assert_eq!(x, x)` ⇒ đỏ",
                (not rep.ok) and any("E_CONST_TRUE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (3b) `assert_eq!(f(seed), f(seed))` — **ca tất định**, không phải hằng-đúng: gọi
        # hai lần phải ra cùng kết quả. Máy kiểm **không** được đỏ ở đây.
        d = tmp / "determinism"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": (
                    "fn script(seed: u64) -> u64 {\n    seed * 3\n}\n\n"
                    "#[test]\nfn tat_dinh() {\n    for seed in 0..4 {\n"
                    "        assert_eq!(script(seed), script(seed), \"seed {seed}\");\n"
                    "    }\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`assert_eq!(f(x), f(x))` (tất định) ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (3c) `assert_ne!(x, x)` — luôn sai, cùng lớp suy biến ⇒ đỏ.
        d = tmp / "const-ne"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": (
                    "#[test]\nfn gia() {\n    let x = 1;\n    assert_ne!(x, x);\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`assert_ne!(x, x)` ⇒ đỏ",
                (not rep.ok) and any("E_CONST_FALSE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4) `#[ignore]` ⇒ đỏ.
        d = tmp / "ignore"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": (
                    "#[test]\n#[ignore]\nfn bo() {\n    assert_eq!(1, 1 + 0);\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`#[ignore]` ⇒ đỏ",
                (not rep.ok) and any("E_IGNORE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (5) `#[test]` rỗng ⇒ đỏ.
        d = tmp / "no-assert"
        _fixture(d, {"v2/lib/a/tests/t.rs": "#[test]\nfn rong() {}\n"})
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`#[test]` không khẳng định ⇒ đỏ",
                (not rep.ok) and any("E_NO_ASSERT" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (5b) `#[test]` chỉ có `.unwrap()` ⇒ đỏ: `unwrap` chứng minh "không lỗi", **không**
        # chứng minh giá trị đúng — đúng lớp xanh-giả cần bắt.
        d = tmp / "unwrap-only"
        _fixture(
            d,
            {
                "v2/lib/a/src/lib.rs": (
                    "pub fn mo() -> Result<u32, ()> {\n    Ok(1)\n}\n\n"
                    "#[cfg(test)]\nmod tests {\n    use super::*;\n\n"
                    "    #[test]\n    fn chi_unwrap() {\n        let _x = mo().unwrap();\n    }\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={}, allow_no_assert={})
        cases.append(
            (
                "`#[test]` chỉ có `.unwrap()` ⇒ đỏ",
                (not rep.ok) and any("E_NO_ASSERT" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (5c) cùng ca đó + allowlist `#[test]` không khẳng định ⇒ xanh (đối chứng dương).
        d = tmp / "no-assert-allow"
        _fixture(d, {"v2/lib/a/tests/t.rs": "#[test]\nfn khong_panic() {\n    let _x = 1;\n}\n"})
        rep = scan(
            d,
            allow_let={},
            helper_allow={},
            allow_no_assert={("v2/lib/a/tests/t.rs", "khong_panic"): (2, "chỉ đòi không panic")},
        )
        cases.append(
            (
                "`#[test]` không khẳng định + allowlist khớp ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (5d) allowlist `#[test]` **mục** ⇒ đỏ (fail-closed).
        d = tmp / "no-assert-stale"
        _fixture(d, {"v2/lib/a/tests/t.rs": _FIXTURE_CLEAN})
        rep = scan(
            d,
            allow_let={},
            helper_allow={},
            allow_no_assert={("v2/lib/a/tests/t.rs", "khong_ton_tai"): (1, "mục chết")},
        )
        cases.append(
            (
                "allowlist `#[test]` mục ⇒ đỏ",
                (not rep.ok) and any("E_ALLOW_STALE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (6) `#[test]` gọi trợ giúp cùng tệp (transitive) ⇒ xanh.
        d = tmp / "helper"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": (
                    "fn kiem(x: u32) {\n    assert_eq!(x, 3);\n}\n\n"
                    "fn boc(x: u32) {\n    kiem(x);\n}\n\n"
                    "#[test]\nfn dung_tro_giup() {\n    boc(3);\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            ("`#[test]` gọi trợ giúp transitive ⇒ xanh", rep.ok, "; ".join(rep.errors))
        )

        # (6b) `#[test]` gọi trợ giúp generic bằng **turbofish** ⇒ xanh (KT-163b, review L6).
        d = tmp / "helper-turbofish"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": (
                    "fn kiem_chung<F: Default>() {\n    assert_eq!(F::default(), F::default());\n}\n\n"
                    "#[test]\nfn dung_tro_giup_generic() {\n    kiem_chung::<u32>();\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`#[test]` gọi trợ giúp generic `::<T>` ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (7) `#[test]` gọi trợ giúp **khai báo** (tệp khác) ⇒ xanh.
        d = tmp / "helper-allow"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": "#[test]\nfn dung_tro_giup_ngoai() {\n    kiem_ngoai(3);\n}\n",
            },
        )
        rep = scan(d, allow_let={}, helper_allow={"kiem_ngoai": "trợ giúp dùng chung"})
        cases.append(
            (
                "`#[test]` gọi trợ giúp trong HELPER_ALLOW ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (8) `let _ =` nuốt `Result` ⇒ đỏ.
        d = tmp / "let-underscore"
        _fixture(
            d,
            {
                "v2/lib/a/src/lib.rs": (
                    "pub fn mo() -> Result<u32, ()> {\n    Ok(1)\n}\n\n"
                    "#[cfg(test)]\nmod tests {\n    use super::*;\n\n"
                    "    #[test]\n    fn nuot() {\n        let _ = mo();\n        assert_eq!(1, 1 + 0);\n    }\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`let _ =` nuốt `Result` ⇒ đỏ",
                (not rep.ok) and any("E_LET_UNDERSCORE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (9) cùng chỗ đó + allowlist khớp ⇒ xanh (đối chứng dương cho allowlist).
        d = tmp / "let-allow"
        _fixture(
            d,
            {
                "v2/lib/a/src/lib.rs": (
                    "pub fn mo() -> Result<u32, ()> {\n    Ok(1)\n}\n\n"
                    "#[cfg(test)]\nmod tests {\n    use super::*;\n\n"
                    "    #[test]\n    fn nuot() {\n        let _ = mo();\n        assert_eq!(1, 1 + 0);\n    }\n}\n"
                ),
            },
        )
        rep = scan(
            d,
            allow_let={("v2/lib/a/src/lib.rs", "nuot", 1): (10, "lý do thử")},
            helper_allow={},
        )
        cases.append(
            ("`let _ =` + allowlist khớp ⇒ xanh", rep.ok, "; ".join(rep.errors))
        )

        # (10) allowlist **mục** (không còn khớp) ⇒ đỏ (fail-closed).
        d = tmp / "let-stale"
        _fixture(d, {"v2/lib/a/tests/t.rs": _FIXTURE_CLEAN})
        rep = scan(
            d,
            allow_let={("v2/lib/a/tests/t.rs", "khong_ton_tai", 1): (1, "mục chết")},
            helper_allow={},
        )
        cases.append(
            (
                "allowlist mục ⇒ đỏ",
                (not rep.ok) and any("E_ALLOW_STALE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (11) `let _ = foo().unwrap();` ⇒ xanh (không phải nuốt: unwrap panic khi lỗi).
        d = tmp / "let-unwrap"
        _fixture(
            d,
            {
                "v2/lib/a/src/lib.rs": (
                    "pub fn mo() -> Result<u32, ()> {\n    Ok(1)\n}\n\n"
                    "#[cfg(test)]\nmod tests {\n    use super::*;\n\n"
                    "    #[test]\n    fn khong_nuot() {\n        let _ = mo().unwrap();\n"
                    "        assert_eq!(1, 1 + 0);\n    }\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            ("`let _ = foo().unwrap()` ⇒ xanh", rep.ok, "; ".join(rep.errors))
        )

        # (12) `panic!` vô điều kiện ⇒ đỏ; `panic!` trong `if` ⇒ xanh.
        d = tmp / "panic"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": "#[test]\nfn luon_panic() {\n    panic!(\"TODO\");\n}\n",
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        bad = (not rep.ok) and any("E_NO_ASSERT" in e for e in rep.errors)
        d = tmp / "panic-cond"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": (
                    "#[test]\nfn co_dieu_kien() {\n    let x = 1;\n"
                    "    if x != 1 {\n        panic!(\"sai\");\n    }\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`panic!` vô điều kiện ⇒ đỏ, có điều kiện ⇒ xanh",
                bad and rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (13) `#[test]` trong `#[cfg(test)] mod` của `src/` ⇒ quét; `#[test]` ngoài
        # `#[cfg(test)]` ⇒ **không** quét (không báo oan).
        d = tmp / "scope"
        _fixture(
            d,
            {
                "v2/lib/a/src/lib.rs": (
                    "pub fn f() -> u32 {\n    1\n}\n\n"
                    "#[cfg(test)]\nmod tests {\n    #[test]\n    fn trong() {\n        assert!(true);\n    }\n}\n"
                ),
                "v2/lib/a/src/khac.rs": "#[test]\nfn ngoai() {\n    assert!(true);\n}\n",
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        hits = [e for e in rep.errors if "E_CONST_TRUE" in e]
        cases.append(
            (
                "chỉ quét `#[cfg(test)]` trong src/",
                len(hits) == 1 and "lib.rs" in hits[0],
                "; ".join(rep.errors),
            )
        )

        # (14) `#[cfg(all(test, feature = \"alloc\"))] mod` ⇒ quét (không chỉ `cfg(test)`).
        d = tmp / "cfg-all"
        _fixture(
            d,
            {
                "v2/lib/a/src/lib.rs": (
                    "#[cfg(all(test, feature = \"alloc\"))]\nmod tests {\n"
                    "    #[test]\n    fn trong() {\n        assert!(true);\n    }\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`#[cfg(all(test, …))]` ⇒ vẫn quét",
                (not rep.ok) and any("E_CONST_TRUE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (15) `#[should_panic(expected = "…")]` không có assert ⇒ xanh (panic **là** khẳng
        # định, và `expected` làm nó có răng thật). Từ KT-170 dạng **trần** là ĐỎ — ca (17).
        d = tmp / "should-panic"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": (
                    "#[test]\n#[should_panic(expected = \"chia cho 0\")]\n"
                    "fn phai_panic() {\n    let _x = 1 / 0;\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`#[should_panic(expected = …)]` ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (16) cây không có test nào ⇒ đỏ (fail-closed).
        d = tmp / "empty"
        _fixture(d, {"v2/lib/a/src/lib.rs": "pub fn f() -> u32 {\n    1\n}\n"})
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "cây không có test ⇒ đỏ (fail-closed)",
                (not rep.ok) and any("E_NO_TESTS" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (17) `#[should_panic]` **trần** ⇒ đỏ (KT-170, lớp (e)): bắt panic bất kỳ là răng
        # yếu. Ca này **có** assert trong thân, để chứng minh (e) độc lập với (c).
        d = tmp / "should-panic-bare"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": (
                    "#[test]\n#[should_panic]\nfn phai_panic() {\n    assert_eq!(1, 1 + 0);\n"
                    "    let _x = 1 / 0;\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`#[should_panic]` trần ⇒ đỏ",
                (not rep.ok) and any("E_SHOULD_PANIC_BARE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (18) `expected = ""` khớp **mọi** thông điệp ⇒ y như trần ⇒ đỏ.
        d = tmp / "should-panic-empty"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": (
                    "#[test]\n#[should_panic(expected = \"\")]\nfn phai_panic() {\n"
                    "    assert_eq!(1, 1 + 0);\n    let _x = 1 / 0;\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`#[should_panic(expected = \"\")]` ⇒ đỏ (khớp mọi thông điệp)",
                (not rep.ok) and any("E_SHOULD_PANIC_BARE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (19) `#[should_panic]` trần + allowlist khớp ⇒ xanh (đối chứng dương).
        d = tmp / "should-panic-allow"
        _fixture(
            d,
            {
                "v2/lib/a/tests/t.rs": (
                    "#[test]\n#[should_panic]\nfn phai_panic() {\n    assert_eq!(1, 1 + 0);\n"
                    "    let _x = 1 / 0;\n}\n"
                ),
            },
        )
        rep = scan(
            d,
            allow_let={},
            helper_allow={},
            allow_should_panic={("v2/lib/a/tests/t.rs", "phai_panic"): (2, "lý do thử")},
        )
        cases.append(
            (
                "`#[should_panic]` trần + allowlist khớp ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (20) allowlist `#[should_panic]` **mục** ⇒ đỏ (fail-closed).
        d = tmp / "should-panic-stale"
        _fixture(d, {"v2/lib/a/tests/t.rs": _FIXTURE_CLEAN})
        rep = scan(
            d,
            allow_let={},
            helper_allow={},
            allow_should_panic={("v2/lib/a/tests/t.rs", "khong_ton_tai"): (1, "mục chết")},
        )
        cases.append(
            (
                "allowlist `#[should_panic]` mục ⇒ đỏ",
                (not rep.ok) and any("E_ALLOW_STALE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (21) `#[test]` gọi trợ giúp ở **tệp khác** ⇒ xanh **không cần** `HELPER_ALLOW`
        # (KT-170: bản đồ trợ giúp toàn cây).
        d = tmp / "helper-cross-file"
        _fixture(
            d,
            {
                "v2/lib/a/tests/common/mod.rs": (
                    "pub fn kiem_ngoai(x: u32) {\n    assert_eq!(x, 3);\n}\n"
                ),
                "v2/lib/a/tests/t.rs": (
                    "mod common;\n\n#[test]\nfn dung_tro_giup_ngoai() {\n"
                    "    common::kiem_ngoai(3);\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`#[test]` gọi trợ giúp ở tệp khác ⇒ xanh (không cần HELPER_ALLOW)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (22) đối chứng **âm**: trợ giúp ở tệp khác **không** răng ⇒ vẫn đỏ. Nếu không có ca
        # này, một bản vá "mọi lời gọi chéo tệp đều xanh" sẽ lọt qua selftest.
        d = tmp / "helper-cross-file-bare"
        _fixture(
            d,
            {
                "v2/lib/a/tests/common/mod.rs": (
                    "pub fn khong_kiem(x: u32) -> u32 {\n    x + 1\n}\n"
                ),
                "v2/lib/a/tests/t.rs": (
                    "mod common;\n\n#[test]\nfn chi_goi() {\n"
                    "    let _x = common::khong_kiem(3);\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`#[test]` gọi trợ giúp tệp khác **không** răng ⇒ đỏ",
                (not rep.ok) and any("E_NO_ASSERT" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (23) bắc cầu **qua ba tệp**: test -> `boc` (tệp B) -> `kiem` (tệp C, có assert).
        d = tmp / "helper-cross-file-transitive"
        _fixture(
            d,
            {
                "v2/lib/a/tests/common/mot.rs": "pub fn boc(x: u32) {\n    kiem(x);\n}\n",
                "v2/lib/a/tests/common/hai.rs": (
                    "pub fn kiem(x: u32) {\n    assert_eq!(x, 3);\n}\n"
                ),
                "v2/lib/a/tests/t.rs": (
                    "mod common;\n\n#[test]\nfn dung_tro_giup_bac_cau() {\n    common::boc(3);\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "trợ giúp tệp khác **bắc cầu** ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (24) bí danh `use … as …` ⇒ phân giải đúng tên gốc.
        d = tmp / "helper-alias"
        _fixture(
            d,
            {
                "v2/lib/a/tests/common/mod.rs": (
                    "pub fn kiem_ngoai(x: u32) {\n    assert_eq!(x, 3);\n}\n"
                ),
                "v2/lib/a/tests/t.rs": (
                    "mod common;\nuse common::kiem_ngoai as kt;\n\n"
                    "#[test]\nfn dung_biet_danh() {\n    kt(3);\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "`use … as …` ⇒ phân giải tên gốc",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (25) tên **mơ hồ** (hai định nghĩa, một không răng) ⇒ **không** tính là răng
        # (fail-closed): thà đỏ oan còn hơn xanh oan; ca thật thì khai `HELPER_ALLOW`.
        d = tmp / "helper-ambiguous"
        _fixture(
            d,
            {
                "v2/lib/a/tests/common/mot.rs": (
                    "pub fn kiem(x: u32) {\n    assert_eq!(x, 3);\n}\n"
                ),
                "v2/lib/a/tests/common/hai.rs": "pub fn kiem(x: u32) -> u32 {\n    x + 1\n}\n",
                "v2/lib/a/tests/t.rs": (
                    "mod common;\n\n#[test]\nfn goi_ten_mo_ho() {\n    common::kiem(3);\n}\n"
                ),
            },
        )
        rep = scan(d, allow_let={}, helper_allow={})
        cases.append(
            (
                "trợ giúp tệp khác **tên mơ hồ** ⇒ đỏ (fail-closed)",
                (not rep.ok) and any("E_NO_ASSERT" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    fails = 0
    for name, ok, detail in cases:
        if ok:
            print(f"ok - {name}")
        else:
            fails += 1
            print(f"not ok - {name}")
            if detail:
                print(f"#   {detail}")
    print(f"# selftest: {len(cases) - fails}/{len(cases)} ok")
    return 0 if fails == 0 else 1


# ------------------------------------------------------------------ main


def main(argv: Sequence[str]) -> int:
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--root", default=None, help="gốc cây nguồn (mặc định: repo chứa tệp này)")
    ap.add_argument("--json", action="store_true", help="in kết quả dạng JSON")
    ap.add_argument("--selftest", action="store_true", help="chạy 31 ca tự kiểm")
    args = ap.parse_args(argv)

    if args.selftest:
        return selftest()

    here = Path(__file__).resolve()
    root = Path(args.root).resolve() if args.root else here.parent.parent.parent
    if not root.is_dir():
        print(f"check-test-teeth: không có thư mục {root}", file=sys.stderr)
        return 2
    if not (root / "v2").is_dir():
        print(
            f"check-test-teeth: {root} không có `v2/` — fail-closed",
            file=sys.stderr,
        )
        return 1

    rep = scan(root)

    if args.json:
        print(
            json.dumps(
                {
                    "ok": rep.ok,
                    "errors": rep.errors,
                    "warnings": rep.warnings,
                    "info": rep.info,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    else:
        for line in rep.info:
            print(f"# {line}")
        for line in rep.warnings:
            print(f"W {line}")
        for line in rep.errors:
            print(f"E {line}")
        print(
            f"# check-test-teeth: errors={len(rep.errors)} warnings={len(rep.warnings)}"
        )
    return 0 if rep.ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
