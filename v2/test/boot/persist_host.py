#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-75 — mô hình byte của `test-persist` 7 pha + kiểm chứng **độc lập phía host**.

Ba việc, không việc nào tin lời khai của kernel:

1. `plan`  — sinh **kịch bản** 32 B cho LBA 0 (runner `dd` vào ảnh trước mỗi boot).
2. `check` — kiểm **log QEMU**: TAP + mọi trường `# persist:` phải khớp **công thức** (payload
   sinh lại độc lập ở đây, D2-106) và **bất biến** của pha (dương: `pending_blocks=0` +
   `device_flushes>0`; âm: `pending_blocks>0` + `device_flushes=0`).
3. `host`  — kiểm **ảnh đĩa** bằng `vifast-fsutil` (fsck + ls + stat + get) rồi so **từng byte**
   với mô hình: đây là bằng chứng mạnh nhất (không qua kernel/QEMU).

Công thức payload **phải** trùng `payload_byte` trong
`v2/kernel/vifast-kbin-riscv64/src/ktest_persist.rs`:

    b[i] = (phase*37 + i*13 + (i//512)*101) & 0xFF

Dùng:
    python3 v2/test/boot/persist_host.py plan --verify 1 --write 2 --flags 0 --mode 0 \
        --nonce 7 --out plan.bin
    python3 v2/test/boot/persist_host.py check --log boot.log --kind verify --phase 1 --nonce 7
    python3 v2/test/boot/persist_host.py host --disk disk.img --fsutil ./vifast-fsutil \
        --phase 3 --records 5
Exit 0 = khớp toàn bộ; 1 = có khác biệt (in `FAIL …`).
"""

import argparse
import hashlib
import os
import re
import subprocess
import sys

BLOCK = 512
PHASES = 7
P7_BYTES = 144 * BLOCK
P7_BLOCKS = P7_BYTES // BLOCK
PSTATE = "/pstate"
# `KT-408d` — chế độ cắt điện tất định giữa pha 7 + vị trí `crash_after: u32` trong kịch bản 32 B.
MODE_CRASH_MID_WRITE = 3
CRASH_AFTER_OFFSET = 17


def payload(phase, size):
    return bytes(((phase * 37 + i * 13 + (i // BLOCK) * 101) & 0xFF) for i in range(size))


def segments(phase):
    """Cùng thứ tự với `segments(phase)` trong ảnh kernel."""
    return {
        1: [("/p1.txt", 0, payload(1, 512))],
        2: [("/p1.txt", 0, payload(2, 2048))],
        3: [("/p1.txt", 0, payload(2, 2048)), ("/p1.txt", 2048, payload(3, 1024))],
        4: [("/p1.txt", 0, payload(4, 700))],
        5: [("/p5.bin", 0, payload(4, 700))],
        6: [("/p5.bin", 0, payload(6, 1536))],
        7: [("/p5.bin", 0, payload(6, 1536)), ("/p7.bin", 0, payload(7, P7_BYTES))],
    }[phase]


ABSENT = {
    1: [],
    2: [],
    3: [],
    4: [],
    5: ["/p1.txt"],
    6: ["/p1.txt"],
    7: ["/p1.txt"],
}
# Số byte **ghi** của từng pha (pha rename không ghi byte dữ liệu nào).
WROTE = {1: 512, 2: 2048, 3: 1024, 4: 700, 5: 0, 6: 1536, 7: P7_BYTES}


def written_bytes(phase):
    """Đúng **những byte pha đó ghi** — kernel băm phần này, không băm cả trạng thái."""
    return {1: payload(1, 512), 2: payload(2, 2048), 3: payload(3, 1024), 4: payload(4, 700),
            5: b"", 6: payload(6, 1536), 7: payload(7, P7_BYTES)}[phase]


def files_after(phase):
    """`{path: bytes}` sau pha `phase` (ghép các đoạn theo offset)."""
    out = {}
    for path, offset, data in segments(phase):
        buffer = out.setdefault(path, bytearray())
        end = offset + len(data)
        if len(buffer) < end:
            buffer.extend(b"\x00" * (end - len(buffer)))
        buffer[offset:end] = data
    return {path: bytes(buffer) for path, buffer in out.items()}


def seg_sha(phase):
    """SHA-256 của **nối các đoạn** theo đúng thứ tự kernel băm."""
    digest = hashlib.sha256()
    for _path, _offset, data in segments(phase):
        digest.update(data)
    return digest.hexdigest()


def bytes_total(phase):
    return sum(len(data) for _path, _offset, data in segments(phase))


def pstate_bytes(records):
    """Nội dung `/pstate` sau `records` bản ghi (4 B: phase, action, ok, 0).

    Thứ tự boot: (1,w) → (1,v),(2,w) → (2,v),(3,w) … ⇒ sau `n` pha **đã ghi** có `2n-1` bản ghi;
    sau boot đọc lại cuối có `2*7` bản ghi.
    """
    out = bytearray()
    if records == 2 * PHASES:
        for phase in range(1, PHASES + 1):
            out += bytes([phase, 1, 1, 0])
            out += bytes([phase, 2, 1, 0])
        return bytes(out)
    # records = 2n-1 ⇒ n = (records+1)/2 pha đã ghi
    assert records % 2 == 1 and 1 <= records <= 2 * PHASES
    written = (records + 1) // 2
    for phase in range(1, written):
        out += bytes([phase, 1, 1, 0])
        out += bytes([phase, 2, 1, 0])
    out += bytes([written, 1, 1, 0])
    return bytes(out)


# ------------------------------------------------------------------------------------- plan
def cmd_plan(args):
    blob = bytearray(32)
    blob[0:4] = b"PL75"
    blob[4] = 1
    blob[5] = args.verify
    blob[6] = args.write
    blob[7] = args.flags
    blob[8] = args.mode
    blob[9:17] = int(args.nonce).to_bytes(8, "little")
    # `KT-408d`: byte dự phòng 17..20 = `crash_after` (u32 LE). Kịch bản cũ để 0 ⇒ tương thích.
    if args.mode == MODE_CRASH_MID_WRITE:
        if not 1 <= args.crash_after < P7_BLOCKS:
            print(
                f"FAIL plan: mode={MODE_CRASH_MID_WRITE} cần 1 <= crash_after < {P7_BLOCKS} "
                f"(nhận {args.crash_after})",
                file=sys.stderr,
            )
            return 1
    elif args.crash_after != 0:
        print(
            f"FAIL plan: crash_after={args.crash_after} chỉ hợp lệ với mode={MODE_CRASH_MID_WRITE}",
            file=sys.stderr,
        )
        return 1
    blob[CRASH_AFTER_OFFSET : CRASH_AFTER_OFFSET + 4] = int(args.crash_after).to_bytes(4, "little")
    with open(args.out, "wb") as handle:
        handle.write(bytes(blob))
    print(
        f"# plan: verify={args.verify} write={args.write} flags={args.flags} mode={args.mode} "
        f"crash_after={args.crash_after} nonce={args.nonce} bytes={len(blob)} out={args.out}"
    )
    return 0


# ----------------------------------------------------------------------------------- check log
def fields(line):
    out = {}
    for token in line.split():
        if "=" in token:
            key, value = token.split("=", 1)
            out[key] = value
    return out


def find_lines(text, needle):
    return [line for line in text.splitlines() if needle in line]


def check_log(args):
    fails = []

    def fail(message):
        fails.append(message)

    with open(args.log, encoding="utf-8", errors="replace") as handle:
        text = handle.read()
    lines = text.splitlines()

    taps = [line for line in lines if line.strip() == "TAP version 13"]
    if len(taps) != 1:
        fail(f"TAP version 13 xuất hiện {len(taps)} lần (cần đúng 1)")

    plans = find_lines(text, "# persist: plan ")
    if len(plans) != 1:
        fail(f"dòng plan xuất hiện {len(plans)} lần")
    else:
        plan = fields(plans[0])
        if plan.get("nonce") != str(args.nonce):
            fail(f"nonce log={plan.get('nonce')} kịch bản={args.nonce}")
        if plan.get("verify") != str(args.verify):
            fail(f"verify log={plan.get('verify')} kịch bản={args.verify}")
        if plan.get("write") != str(args.write):
            fail(f"write log={plan.get('write')} kịch bản={args.write}")
        if plan.get("mode") != str(args.mode):
            fail(f"mode log={plan.get('mode')} kịch bản={args.mode}")

    actions = (1 if args.verify > 0 else 0) + (1 if args.write > 0 else 0)
    if args.mode == MODE_CRASH_MID_WRITE:
        # `KT-408d`: plan khai 2 hành động (đọc lại pha 6, ghi pha 7) nhưng cắt giữa pha 7 nên
        # TAP thứ hai **không bao giờ** in ra — log của một lần cắt điện là TAP dở **có chủ ý**.
        actions = (1 if args.verify > 0 else 0) + (1 if args.write > 0 else 0)
    elif args.mode != 0:
        actions = 1
    if f"1..{actions}" not in [line.strip() for line in lines]:
        fail(f"thiếu TAP plan '1..{actions}'")
    if args.mode == MODE_CRASH_MID_WRITE or args.crash_after != 0:
        plan = fields(plans[0]) if len(plans) == 1 else {}
        if plan.get("crash_after") != str(args.crash_after):
            fail(f"crash_after log={plan.get('crash_after')} kịch bản={args.crash_after}")

    if args.kind == "write":
        matched = find_lines(text, f"# persist: phase={args.phase} action=write ")
        if len(matched) != 1:
            fail(f"dòng write pha {args.phase} xuất hiện {len(matched)} lần")
        else:
            got = fields(matched[0])
            want = {
                "wrote": WROTE[args.phase],
                "recorded": 1,
                "fsync": 1,
                "durable": 1,
                "flush_honored": 1,
                "flushes_ignored": 0,
                "pending_blocks": 0,
                "overflow": 0,
            }
            for key, value in want.items():
                if got.get(key) != str(value):
                    fail(f"write pha {args.phase}: {key}={got.get(key)} (cần {value})")
            if int(got.get("device_flushes", "0")) <= 0:
                fail("write pha: device_flushes=0 — pha không hề gửi T_FLUSH")
            expected_sha = hashlib.sha256(written_bytes(args.phase)).hexdigest()[:8]
            if got.get("sha_prefix") != expected_sha:
                fail(
                    f"write pha: sha_prefix={got.get('sha_prefix')} "
                    f"host={expected_sha} (payload lệch công thức)"
                )
            if args.phase == 7:
                # D2-106b: `detail` là **suy ra** — log phải in cả công thức và hai thành phần;
                # kiểm cả đẳng thức `detail == single<<32|double`, không chỉ đọc một số.
                pattern = re.compile(
                    r"detail=(\d+) \(=single\((\d+)\)<<32\|double\((\d+)\)\)"
                )
                match = pattern.search(matched[0])
                if not match:
                    fail("pha 7: thiếu công thức 'detail=<n> (=single(..)<<32|double(..))'")
                else:
                    detail, single, double = (int(value) for value in match.groups())
                    if single == 0 or double == 0:
                        fail(f"pha 7: thiếu bảng gián tiếp single={single} double={double}")
                    # **KT-111**: kiểm **ngữ nghĩa** trước, rồi mới kiểm công thức. Nếu chỉ kiểm
                    # `detail == (single << 32) | double` thì harness và kernel dùng **cùng một
                    # công thức** ⇒ đổi cả hai (ví dụ `<< 16`) vẫn xanh (KT-107 mục 3). Ba bất biến
                    # dưới đây **không** phụ thuộc cách đóng gói:
                    #   (i)  hai khối phải **khác nhau** (một khối không thể vừa single vừa double);
                    #   (ii) cả hai phải **dương** (khối 0 là superblock, không phải bảng gián tiếp);
                    #   (iii) `detail` phải **lớn hơn** cả hai (nó gộp hai khối, không thể bằng một).
                    if single == double:
                        fail(f"pha 7: single={single} trùng double — hai bảng gián tiếp phải khác khối")
                    #   (iv) `detail` phải **≥ 2^32**: nó gộp hai khối 32 bit, nên nếu ai đổi dịch
                    #        bit (ví dụ `<< 16`) thì `detail` tụt xuống dưới 2^32 — bất biến này
                    #        **không** phụ thuộc con số dịch cụ thể.
                    if detail < (1 << 32):
                        fail(
                            f"pha 7: detail={detail} < 2^32 — không thể gộp hai khối 32 bit "
                            f"(single={single} double={double})"
                        )
                    if detail <= single or detail <= double:
                        fail(
                            f"pha 7: detail={detail} không lớn hơn single={single}/double={double} "
                            "— cách đóng gói đã làm mất một khối"
                        )
                    if detail != (single << 32) | double:
                        fail(f"pha 7: detail={detail} != single<<32|double={((single << 32) | double)}")
            if args.phase == 3 and int(got.get("detail", "0")) != 2048:
                fail(f"pha 3: detail={got.get('detail')} (cần offset append 2048)")
        line = f"ok {actions} - persist write phase {args.phase}"
        if line not in [item.strip() for item in lines]:
            fail(f"thiếu dòng TAP '{line}'")

    if args.kind in ("verify", "verify-last"):
        matched = find_lines(text, f"# persist: phase={args.phase} action=verify ")
        if len(matched) != 1:
            fail(f"dòng verify pha {args.phase} xuất hiện {len(matched)} lần")
        else:
            got = fields(matched[0])
            want = {
                "bytes": bytes_total(args.phase),
                "read": bytes_total(args.phase),
                "match": 1,
                "mismatched": 0,
                "size_bad": 0,
                "absent_bad": 0,
                "absent": len(ABSENT[args.phase]),
                "recorded": 1,
                "pending_blocks": 0,
                "files": len(files_after(args.phase)),
            }
            for key, value in want.items():
                if got.get(key) != str(value):
                    fail(f"verify pha {args.phase}: {key}={got.get(key)} (cần {value})")
            if got.get("sha_prefix") != seg_sha(args.phase)[:8]:
                fail(
                    f"verify pha {args.phase}: sha_prefix={got.get('sha_prefix')} "
                    f"host={seg_sha(args.phase)[:8]}"
                )
        line = f"ok 1 - persist verify phase {args.phase}"
        if line not in [item.strip() for item in lines]:
            fail(f"thiếu dòng TAP '{line}'")

    if args.kind == "verify-last":
        totals = find_lines(text, "# persist: phases=")
        if len(totals) != 1:
            fail(f"dòng tổng kết 'phases=' xuất hiện {len(totals)} lần")
        else:
            total = fields(totals[0])
            if total.get("phases") != str(PHASES) or total.get("ok") != str(PHASES):
                fail(f"tổng kết: phases={total.get('phases')} ok={total.get('ok')} (cần 7/7)")
            if total.get("records") != str(2 * PHASES):
                fail(f"tổng kết: records={total.get('records')} (cần {2 * PHASES})")

    if args.kind == "neg-write":
        matched = find_lines(text, "# persist: phase=neg action=noflush_write ")
        if len(matched) != 1:
            fail(f"dòng noflush_write xuất hiện {len(matched)} lần")
        else:
            got = fields(matched[0])
            want = {"ok": 1, "flush_honored": 0, "device_flushes": 0, "pushed": 0, "overflow": 0}
            for key, value in want.items():
                if got.get(key) != str(value):
                    fail(f"neg-write: {key}={got.get(key)} (cần {value})")
            if int(got.get("wrote", "0")) != 600:
                fail(f"neg-write: wrote={got.get('wrote')} (cần 600)")
            if int(got.get("pending_blocks", "0")) <= 0:
                fail("neg-write: pending_blocks=0 — không có gì trong cache để mất")
            # D2-106b: `bytes_lost` là suy ra từ `pending_blocks` — kiểm cả công thức trong log.
            pattern = re.compile(r"bytes_lost=(\d+) \(=pending\((\d+)\)\*512\)")
            match = pattern.search(matched[0])
            if not match:
                fail("neg-write: thiếu công thức 'bytes_lost=<n> (=pending(<n>)*512)'")
            else:
                lost, pending = (int(value) for value in match.groups())
                if lost <= 0:
                    fail("neg-write: bytes_lost=0")
                if lost != pending * 512:
                    fail(f"neg-write: bytes_lost={lost} != pending({pending})*512")
        if "ok 1 - persist noflush write" not in [item.strip() for item in lines]:
            fail("thiếu dòng TAP 'ok 1 - persist noflush write'")

    if args.kind == "neg-verify":
        matched = find_lines(text, "# persist: phase=neg action=verify_absent ")
        if len(matched) != 1:
            fail(f"dòng verify_absent xuất hiện {len(matched)} lần")
        else:
            got = fields(matched[0])
            want = {"absent": 1, "p7_match": 1, "mismatched": 0, "phases": PHASES}
            for key, value in want.items():
                if got.get(key) != str(value):
                    fail(f"neg-verify: {key}={got.get(key)} (cần {value})")
            if int(got.get("records", "0")) != 2 * PHASES:
                fail(f"neg-verify: records={got.get('records')} (cần {2 * PHASES})")
        if "ok 1 - persist noflush verify" not in [item.strip() for item in lines]:
            fail("thiếu dòng TAP 'ok 1 - persist noflush verify'")

    if args.kind == "crash":
        # `KT-408d (Q7)` — log của lần cắt điện **tất định**: đọc lại pha 6 xanh, rồi đúng MỘT
        # dòng `crash_cut` ở sector `crash_after`, cache còn block bẩn (chưa flush), và **không**
        # có dòng ghi pha 7 / TAP thứ hai (đã halt giữa transaction của sector kế tiếp).
        if args.mode != MODE_CRASH_MID_WRITE:
            fail(f"crash: mode={args.mode} (cần {MODE_CRASH_MID_WRITE})")
        line = f"ok 1 - persist verify phase {PHASES - 1}"
        if line not in [item.strip() for item in lines]:
            fail(f"thiếu TAP '{line}' (dữ liệu đã commit phải đọc được NGAY trước khi cắt)")
        cut = find_lines(text, "# persist: crash_cut ")
        if len(cut) != 1:
            fail(f"dòng crash_cut xuất hiện {len(cut)} lần (cần đúng 1)")
        else:
            got = fields(cut[0])
            if got.get("mode") != str(MODE_CRASH_MID_WRITE):
                fail(f"crash_cut: mode={got.get('mode')} (cần {MODE_CRASH_MID_WRITE})")
            if got.get("writes") != str(args.crash_after):
                fail(f"crash_cut: writes={got.get('writes')} kịch bản={args.crash_after}")
            pending = got.get("pending", "")
            if not pending.isdigit() or int(pending) < 1:
                fail(f"crash_cut: pending={pending or '?'} — cache KHÔNG còn block bẩn (không phải cắt giữa lúc ghi)")
            slots = got.get("cache_slots", "")
            if slots.isdigit() and int(pending) > int(slots):
                fail(f"crash_cut: pending={pending} > cache_slots={slots}")
        if find_lines(text, f"# persist: phase={PHASES} action=write "):
            fail("crash: có dòng GHI pha 7 hoàn tất — cắt điện đã rơi SAU khi ghi xong")
        if f"ok 2 - persist write phase {PHASES}" in [item.strip() for item in lines]:
            fail("crash: có TAP ghi pha 7 hoàn tất — cắt điện không xảy ra giữa pha 7")
        if find_lines(text, "# persist: stats "):
            fail("crash: có dòng tổng kết stats — kernel đã chạy hết, không bị cắt")

    not_ok = [line for line in lines if line.startswith("not ok ")]
    if not_ok:
        fail(f"log có 'not ok': {not_ok[0][:120]}")

    for message in fails:
        print(f"FAIL log {os.path.basename(args.log)}: {message}")
    print(
        f"# log-check: log={os.path.basename(args.log)} kind={args.kind} "
        f"phase={args.phase} nonce={args.nonce} fail={len(fails)}"
    )
    return 0 if not fails else 1


# --------------------------------------------------------------------------------- host checks
def run(args, **kwargs):
    return subprocess.run(args, capture_output=True, **kwargs)


def fsutil_listing(fsutil, disk, path):
    result = run([fsutil, "ls", disk, path])
    if result.returncode != 0:
        raise SystemExit(f"FAIL fsutil ls {path}: {result.stderr.decode(errors='replace')}")
    names = set()
    for line in result.stdout.decode(errors="replace").splitlines():
        parts = line.split(" ", 1)
        if len(parts) == 2 and parts[0].isdigit():
            names.add(parts[1])
    return names


def fsutil_get(fsutil, disk, path):
    result = run([fsutil, "get", disk, path])
    if result.returncode != 0:
        raise SystemExit(f"FAIL fsutil get {path}: {result.stderr.decode(errors='replace')}")
    return result.stdout


def fsutil_stat_size(fsutil, disk, path):
    result = run([fsutil, "stat", disk, path])
    if result.returncode != 0:
        return None
    for token in result.stdout.decode(errors="replace").split():
        if token.startswith("size="):
            return int(token.split("=", 1)[1])
    return None


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        while True:
            chunk = handle.read(1 << 20)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def cmd_host(args):
    fails = []

    def fail(message):
        fails.append(message)

    before = sha256_file(args.disk)
    fsck = run([args.fsutil, "fsck", args.disk])
    if fsck.returncode != 0:
        fail(f"fsutil fsck exit={fsck.returncode}: {fsck.stderr.decode(errors='replace')[:160]}")

    expected = files_after(args.phase)
    expected[PSTATE] = pstate_bytes(args.records)
    listed = fsutil_listing(args.fsutil, args.disk, "/")
    for name in expected:
        bare = name.lstrip("/")
        if bare not in listed:
            fail(f"ls / thiếu {bare}")
    for name in ABSENT[args.phase] + ["/pneg.txt"]:
        bare = name.lstrip("/")
        if bare in listed:
            fail(f"ls / còn {bare} (đáng lẽ vắng)")
    extras = listed - {"."} - {".."} - {name.lstrip("/") for name in expected}
    if extras:
        fail(f"ls / có entry lạ: {sorted(extras)}")

    for name, want in expected.items():
        size = fsutil_stat_size(args.fsutil, args.disk, name)
        if size != len(want):
            fail(f"stat {name} size={size} (cần {len(want)})")
        got = fsutil_get(args.fsutil, args.disk, name)
        if got != want:
            fail(f"get {name}: {len(got)} B khác mô hình {len(want)} B / nội dung lệch")
            if len(got) == len(want):
                first = next(i for i in range(len(want)) if got[i] != want[i])
                fail(f"  byte đầu tiên lệch tại offset {first}")
        want_sha = hashlib.sha256(want).hexdigest()
        got_sha = hashlib.sha256(got).hexdigest() if got != want else want_sha
        print(
            f"# host file: name={name} bytes={len(got)} size_ok={int(size == len(want))} "
            f"sha256={got_sha} want_sha256={want_sha} match={int(got == want)}"
        )
    for name in ABSENT[args.phase]:
        size = fsutil_stat_size(args.fsutil, args.disk, name)
        if size is not None:
            fail(f"stat {name} vẫn tồn tại (size={size})")
        print(f"# host file: name={name} absent={int(size is None)} const:want_absent=1")

    after = sha256_file(args.disk)
    if before != after:
        fail(f"host verify ĐÃ GHI vào ảnh: sha256 {before[:16]}… -> {after[:16]}…")
    print(
        f"# host: phase={args.phase} records={args.records} files={len(expected)} "
        f"fsck={int(fsck.returncode == 0)} fail={len(fails)} "
        f"img_sha256={after} fsutil_mutated={int(before != after)}"
    )
    for message in fails:
        print(f"FAIL host {os.path.basename(args.disk)}: {message}")
    return 0 if not fails else 1


# ------------------------------------------------------------------- KT-136: kiểm sau "cắt điện"
def cmd_crash_host(args):
    """Kiểm ảnh đĩa **sau khi kernel tự halt giữa lúc đang ghi** (KT-136; tất định từ `KT-408d`).

    Không đòi "ảnh phải đúng trạng thái pha N" — cắt điện giữa lúc ghi thì trạng thái là **tiền
    tố** xác định bởi `--crash-after`. Đòi đúng bốn điều mà một FS có journal **phải** giữ:

    1. **fsck sạch** — không có cấu trúc hỏng (superblock/bitmap/inode/dir/journal).
    2. **Dữ liệu đã commit trước đó còn nguyên từng byte** — ở đây là trạng thái sau pha 6
       (`/p5.bin` = payload 6, `/p1.txt` vắng). Đây là điều quan trọng nhất: một lần cắt điện
       **không** được làm mất dữ liệu đã báo "đã ghi".
    3. **Tệp ghi dở là TIỀN TỐ của payload pha 7** và cỡ **đúng** `crash_after * 512 B` (bất biến
       của điểm cắt trong `ktest_persist` — không phụ thuộc thời gian/tải); vắng hoặc đủ 144 block
       ⇒ cắt điện **không** xảy ra giữa pha 7 ⇒ ĐỎ (đúng lỗi flake `records=13` cũ).
    4. `/pstate` có **đúng 12** bản ghi (11 bản ghi hết pha 6 + 1 verify pha 6 của lần cắt điện),
       mọi bản ghi đúng định dạng và đúng thứ tự pha; bản ghi write pha 7 phải **vắng**.
    """
    fails = []

    def fail(message):
        fails.append(message)

    # `fsck` **được phép ghi**: sau một lần cắt điện, journal có thể còn bản ghi chưa áp dụng và
    # `fsck` phát lại nó (replay) — đó là hành vi đúng của FS có journal, không phải hỏng. Vì vậy
    # đo sha **sau** `fsck` rồi mới kiểm "các phép đọc không ghi vào ảnh"; đo trước `fsck` sẽ báo
    # động giả (đã đo được ở lần chạy đầu: `fsutil_mutated=1` dù fsck sạch).
    fsck = run([args.fsutil, "fsck", args.disk])
    if fsck.returncode != 0:
        fail(f"fsutil fsck exit={fsck.returncode}: {fsck.stderr.decode(errors='replace')[:160]}")
    before = sha256_file(args.disk)

    # (2) dữ liệu đã commit ở pha 6
    committed = files_after(6)
    for name, want in committed.items():
        got = fsutil_get(args.fsutil, args.disk, name)
        if got != want:
            fail(f"get {name}: {len(got)} B khác mô hình đã commit {len(want)} B / nội dung lệch")
        print(
            f"# crash file: name={name} bytes={len(got)} want={len(want)} "
            f"match={int(got == want)} const:committed=1"
        )
    for name in ABSENT[6]:
        size = fsutil_stat_size(args.fsutil, args.disk, name)
        if size is not None:
            fail(f"stat {name} vẫn tồn tại sau crash (size={size}) — dữ liệu đã xoá sống lại")
        print(f"# crash file: name={name} absent={int(size is None)} const:want_absent=1")

    # (3) `KT-408d` — điểm cắt **tất định**: `/p7.bin` phải tồn tại, là **tiền tố** payload pha 7
    # và cỡ đúng `crash_after * 512 B` (FS commit trọn một sector mỗi transaction ⇒ cỡ là bất biến
    # của phép đo, không phụ thuộc lúc QEMU bị dừng). Vắng tệp hoặc đủ 144 block ⇒ cắt KHÔNG xảy
    # ra giữa pha 7 ⇒ ĐỎ (đây chính là lỗi flake `records=13` cũ).
    p7 = "/p7.bin"
    want = payload(7, P7_BYTES)
    want_bytes = args.crash_after * BLOCK
    size = fsutil_stat_size(args.fsutil, args.disk, p7)
    if size is None:
        fail(f"stat {p7}: VẮNG — cắt điện không xảy ra SAU khi tạo tệp (partial=0)")
    else:
        if size > P7_BYTES:
            fail(f"stat {p7} size={size} > cỡ đích {P7_BYTES} — ghi quá phạm vi")
        got = fsutil_get(args.fsutil, args.disk, p7)
        prefix_ok = got == want[: len(got)]
        if not prefix_ok:
            first = next((i for i in range(min(len(got), len(want))) if got[i] != want[i]), None)
            fail(f"get {p7}: {len(got)} B KHÔNG phải tiền tố payload pha 7 (byte lệch đầu {first})")
        if len(got) != want_bytes:
            fail(
                f"get {p7}: {len(got)} B khác điểm cắt tất định {want_bytes} B "
                f"(crash_after={args.crash_after} sector)"
            )
        print(
            f"# crash file: name={p7} bytes={len(got)} want={want_bytes} want_max={P7_BYTES} "
            f"prefix_ok={int(prefix_ok)} partial={int(0 < len(got) < P7_BYTES)} "
            f"const:crash_after={args.crash_after}"
        )

    # `/pstate`: **đúng 12** bản ghi — 11 bản ghi hết pha 6 + 1 bản ghi verify pha 6 của lần boot
    # cắt điện; bản ghi write pha 7 KHÔNG được có (halt trước `append_record`). Con số này là bất
    # biến của điểm cắt, không phải "11 hoặc 12" như cơ chế poll cũ.
    st = fsutil_get(args.fsutil, args.disk, PSTATE)
    n = len(st) // 4
    want_records = 2 * (PHASES - 1)
    if len(st) % 4 != 0:
        fail(f"get {PSTATE}: {len(st)} B không chia hết 4 — bản ghi rách")
    if n != want_records:
        fail(f"get {PSTATE}: {n} bản ghi (cần đúng {want_records} sau khi cắt giữa pha 7 — nợ Q7)")
    for i in range(min(n, 2 * PHASES)):
        phase, action, ok, pad = st[i * 4 : i * 4 + 4]
        want_phase = i // 2 + 1
        want_action = 1 if i % 2 == 0 else 2
        if (phase, action, ok, pad) != (want_phase, want_action, 1, 0):
            fail(
                f"get {PSTATE}: bản ghi {i} = ({phase},{action},{ok},{pad})"
                f" (cần ({want_phase},{want_action},1,0))"
            )
    print(
        f"# crash state: records={n} want={want_records} "
        f"well_formed={int(not any('bản ghi' in f for f in fails))}"
    )

    after = sha256_file(args.disk)
    if before != after:
        fail(f"host verify ĐÃ GHI vào ảnh: sha256 {before[:16]}… -> {after[:16]}…")
    print(
        f"# crash host: fsck={int(fsck.returncode == 0)} fail={len(fails)} "
        f"img_sha256={after} fsutil_mutated={int(before != after)}"
    )
    for message in fails:
        print(f"FAIL crash-host {os.path.basename(args.disk)}: {message}")
    return 0 if not fails else 1


# ------------------------------------------------------------------------------------------ main
def main():
    parser = argparse.ArgumentParser(description="KT-75 host model/verifier cho test-persist")
    sub = parser.add_subparsers(dest="command", required=True)

    plan = sub.add_parser("plan", help="sinh kịch bản 32 B cho LBA 0")
    plan.add_argument("--verify", type=int, default=0)
    plan.add_argument("--write", type=int, default=0)
    plan.add_argument("--flags", type=int, default=0)
    plan.add_argument("--mode", type=int, default=0)
    plan.add_argument("--crash-after", type=int, default=0)
    plan.add_argument("--nonce", type=int, required=True)
    plan.add_argument("--out", required=True)
    plan.set_defaults(func=cmd_plan)

    check = sub.add_parser("check", help="kiểm log QEMU")
    check.add_argument("--log", required=True)
    check.add_argument(
        "--kind",
        required=True,
        choices=["write", "verify", "verify-last", "neg-write", "neg-verify", "crash"],
    )
    check.add_argument("--phase", type=int, default=0)
    check.add_argument("--verify", type=int, default=0)
    check.add_argument("--write", type=int, default=0)
    check.add_argument("--mode", type=int, default=0)
    check.add_argument("--crash-after", type=int, default=0)
    check.add_argument("--nonce", type=int, required=True)
    check.set_defaults(func=check_log)

    host = sub.add_parser("host", help="kiểm ảnh đĩa bằng vifast-fsutil")
    host.add_argument("--disk", required=True)
    host.add_argument("--fsutil", required=True)
    host.add_argument("--phase", type=int, choices=range(1, PHASES + 1), required=True)
    host.add_argument("--records", type=int, required=True)
    host.set_defaults(func=cmd_host)

    crash = sub.add_parser(
        "crash-host",
        help="kiểm ảnh đĩa sau khi kernel TỰ halt giữa pha 7 (KT-136/KT-408d)",
    )
    crash.add_argument("--disk", required=True)
    crash.add_argument("--fsutil", required=True)
    crash.add_argument(
        "--crash-after",
        type=int,
        required=True,
        help="số sector 512 B của /p7.bin phải bền trước khi cắt (mode=3)",
    )
    crash.set_defaults(func=cmd_crash_host)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
