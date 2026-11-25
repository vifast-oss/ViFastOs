#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-151 (N7, D2-131) — kiểm **máy** "một cơ chế đánh thức TCB" trên mã nguồn.

**KT-407** — tệp này là **shim** sau khi tách 11.046 dòng thành package
`v2/tools/checkers/wake/` (`core.py` + `selftest.py` + `cli.py`). Nó giữ nguyên:
  * giao diện dòng lệnh (`--root/--strict/--combined/--json/--selftest`);
  * **toàn bộ** namespace `core` (kể cả tên private) để `check-wake-depinfo.py` nạp tệp này như
    module và dùng lại luật — không chép luật, không lệch hành vi.

Dùng
----
    python3 v2/tools/check-wake-paths.py [--root R] [--strict] [--json]
    python3 v2/tools/check-wake-paths.py --combined      # **KT-333e**: một lượt, cả hai chế độ
    python3 v2/tools/check-wake-paths.py --selftest

Exit: 0 = xanh; 1 = đỏ; 2 = dùng sai.
"""

from __future__ import annotations

import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent
if str(_HERE) not in sys.path:
    sys.path.insert(0, str(_HERE))

from checkers.wake import core as _core  # noqa: E402
from checkers.wake.cli import main  # noqa: E402
from checkers.wake.selftest import selftest  # noqa: E402

# Re-export **toàn bộ** namespace core (kể cả tên private) — `check-wake-depinfo.py` dùng lại
# `_check_wake_text`, `find_functions`, `Fn`, `Report`, `_mods_of`, … qua tên `CWP.*`.
globals().update({k: v for k, v in vars(_core).items() if not k.startswith("__")})

if __name__ == "__main__":
    # **KT-407** — xử lý `--selftest` **tường minh** ở shim: `check-selftest-wiring.py` (KT-134)
    # đòi lời gọi cổng trỏ vào tệp **thật sự xử lý** cờ; shim chỉ re-export `main` thì bị báo
    # `W_FAKE_SELFTEST` oan. `main()` vẫn xử lý cờ cho đường `python -m checkers.wake`.
    if "--selftest" in sys.argv[1:]:
        sys.exit(selftest())
    sys.exit(main(sys.argv[1:]))
