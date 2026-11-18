#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-108 — harness **ca biên** cho shell, chạy trên ảnh **đã ghim** (không build).
#
# Vì sao có tệp này: đêm 2026-11-18 ba lỗi thật của shell được tìm ra **bằng tay** (thử biên),
# không phải bằng test tự động:
#   - KT-103: dòng dài hơn `MAX_LINE` in **một cảnh báo cho mỗi byte** (nay: một lần mỗi dòng).
#   - KT-104: `cat`/`write` không tham số in `# shell: ?` (nay: `# shell: thieu duong dan`).
#   - KT-105: `ps` in `state=Unknown` cho **mọi** tiến trình (nay: trạng thái thật).
# Ba lỗi đó **không** có ca kiểm tự động. Tệp này biến đúng ba thử-biên đó thành ca kiểm hồi quy,
# cộng thêm bốn ca biên khác của shell/FS (mã lỗi, ghi đè co lại, `wait` không treo).
#
# Cách làm: MỘT tiến trình QEMU trên ảnh cho trước; mỗi ca gửi lệnh qua **stdin của QEMU** (UART →
# ring → shell U-mode), chờ đúng chuỗi trong log, rồi khẳng định **đúng chuỗi** trên **lát log của
# riêng ca đó** (không quét cả log: `# shell: ?` của ca khác không được làm nhiễu ca này). Mỗi ca
# kết thúc bằng một lệnh đánh dấu `echo __END_<n>__` — khi dòng đánh dấu xuất hiện thì mọi output
# của ca đã nằm trong log (shell xử lý lệnh **tuần tự**), nên lát log là đủ và không cần `sleep`.
#
# Tám ca (mỗi ca in `# <tên>: PASS/FAIL`):
#   1. cat-khong-tham-so      `cat`            ⇒ `# shell: thieu duong dan`, **không** `# shell: ?`
#   2. write-khong-tham-so    `write`          ⇒ `# shell: thieu duong dan`, **không** `# shell: ?`
#   3. ps-khong-unknown       `ps`             ⇒ có dòng `  pid=… state=…`, **không** `state=Unknown`
#   4. dong-dai-mot-canh-bao  dòng 200 ký tự   ⇒ **đúng một** `# shell: dong qua dai`
#   5. cat-khong-co           `cat /khongco`   ⇒ `# shell: fs loi code=5`
#   6. write-cat-bytes        `write /a xin chao` rồi `cat /a`
#                                              ⇒ `bytes=8` và `first=xin chao`
#   7. write-de-co-lai        `write /g <40 ký tự>` rồi `write /g ab` rồi `cat /g`
#                                              ⇒ `bytes=2` (ghi đè co lại đúng)
#   8. wait-khong-treo        `wait 13` (tiến trình đang chạy) rồi `echo sau-wait`
#                                              ⇒ `wait` trả lời trong hạn, `echo` vẫn chạy
#
# Các phép kiểm **thêm** (không đánh số ca, dùng chung khung `case_begin`/`case_end`):
#   8b. truncate-jobs         `truncate /README 5` + `stat /README` + `jobs` + `ps`
#                                              ⇒ truncate **có hiệu lực** (size=5) và tổng `jobs` = `ps n`
#   10. new-cmds-ok           `time` `uptime` `sync` `stat /README` `ls`
#                                              ⇒ mỗi lệnh in đúng dạng, `ls` liệt kê ≥ 1 mục thật
#   11. new-cmds-edge         `stat` (không tham số) `stat /khongco` `ls /khongco`
#                             `truncate /README abc` `truncate /README 4294967296` `sync`×2 `time`×2
#                                              ⇒ báo đúng lỗi, size ≥ 2^32 bị từ chối, đồng hồ đơn điệu
#   12. new-cmds-edge2        `uptime`×2 `uptime 5` `jobs`×2 `jobs 5`
#                                              ⇒ `uptime` đơn điệu; hai dòng `jobs` **giống nhau từng ký tự**;
#                                                `uptime 5`/`jobs 5` là lệnh lạ (`# shell: ?` ×2)
#   13. new-cmds-edge3        `time 5` `write /a xin chao` `write /a` `stat /a`
#                             `write /khongco/x abc` `ls /README` `sync 5`
#                                              ⇒ `time 5`/`sync 5` là lệnh lạ; `write /a` (thiếu nội dung)
#                                                cắt tệp về 0 byte và **nói thật** `bytes=0`; `write` vào
#                                                thư mục không có ⇒ `fs loi code=5`; `ls` lên **tệp** ⇒
#                                                `fs loi code=7`, **không** liệt kê bừa
#   (10/11/12 là **KT-130**; 13 là **Q4 vòng ba** — cùng Q4 của Agent 0.)
#
# **Phán quyết từng lệnh** (Q4 vòng ba): bốn ca 10/11/12/13 in **một dòng cho mỗi lệnh**
# (`# <ca>[<lệnh>]: PASS/FAIL (lý do)`) và **chạy hết** mọi lệnh rồi mới kết luận — log sai hai
# lệnh thì thông điệp đỏ nêu **cả hai**, không chỉ lệnh đầu. Mã thoát của mỗi ca giữ nguyên
# (0 = xanh, 1 = đỏ) nên khung gọi không phải đổi.
#
# **`--selftest` (bắt buộc theo luật repo: harness mới phải có ca âm)**: dựng **log giả** trong
# `mktemp -d` cho **mỗi** ca — một log **cố tình sai** (đúng đầu ra của lỗi cũ: `# shell: ?`,
# `state=Unknown`, 100 dòng `dong qua dai`, …) và một log **đúng** — rồi chạy **chính** hàm khẳng
# định của harness trên hai log đó. Log sai **phải ĐỎ**, log đúng **phải XANH**. Nhờ vậy selftest
# chứng minh harness **bắt được lỗi thật**, không phải luôn xanh. Không cần QEMU/ảnh cho selftest.
# Với bốn ca đã tách phán quyết từng lệnh, selftest còn đòi **đúng số dòng PASS/FAIL** và **đủ lý
# do của mọi lệnh hỏng** — một log sai hai lệnh thì thông điệp đỏ phải nêu **cả hai**.
#
# Dùng:
#   bash v2/test/boot/shell-edge.sh --image <ảnh.bin>            # chạy 8 ca trên ảnh đã ghim
#   bash v2/test/boot/shell-edge.sh --selftest                  # chỉ chạy ca âm (không cần ảnh)
#   bash v2/test/boot/shell-edge.sh --image <ảnh.bin> --selftest # cả hai (selftest trước)
#   bash v2/test/boot/shell-edge.sh --help
#
# Biến môi trường: `SHELL_EDGE_LOGDIR` (mặc định `/tmp/kt108-shell-edge`), `OPENSBI`.
#
# Exit: 0 = mọi ca PASS (và selftest PASS nếu chạy); 1 = có ca FAIL / selftest FAIL;
#       2 = lỗi cấu hình (thiếu ảnh/QEMU/OpenSBI, ảnh không có shell hoặc thiếu bản vá KT-103/104).
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
# **KT-112** (review chéo): `LOGDIR` cố định ⇒ hai lượt chạy **song song** dùng chung log, và một
# lượt có thể đọc PASS từ log của lượt kia (đã dựng lại được). Nay mỗi lượt một thư mục riêng;
# `SHELL_EDGE_LOGDIR` vẫn cho phép chỉ định (khi đó người dùng tự chịu trách nhiệm không chạy song song).
if [ -n "${SHELL_EDGE_LOGDIR:-}" ]; then
    LOGDIR="$SHELL_EDGE_LOGDIR"
else
    LOGDIR="$(mktemp -d "${TMPDIR:-/tmp}/kt108-shell-edge.XXXXXX")" || {
        echo "shell-edge: không tạo được thư mục log tạm" >&2
        exit 2
    }
    SHELL_EDGE_LOGDIR_TMP="$LOGDIR"
fi
QEMU_MIN_MAJOR=9 # sàn D2-72 Q2: QEMU >= 9.2
QEMU_MIN_MINOR=2
# **KT-112**: `sleep 0,05` trên macOS thực tế tốn ~0,2 s (đo: 200 lần = 41,6 s), nên **không** được
# hứa "20 s"/"10 s" trong thông báo lỗi. Ngân sách dưới đây tính theo **số vòng poll**, và thông báo
# lỗi in **số vòng** thay vì giây để không nói sai.
BOOT_POLLS=400     # 400 vòng chờ `shell>` (D2-72 Q10)
CASE_POLLS=200     # 200 vòng chờ output/marker của một ca
POLL_S=0.05
QEMU_TDS=1200      # trần QEMU 120 s (qg_boot); harness tự kill sớm hơn khi ca quá hạn
WAIT_BUDGET_S=5    # ca 8: `wait 13` phải trả lời trong 5 s
# **KT-120**: sàn **độc lập** cho `ps` — ảnh sản phẩm luôn có ít nhất procsrv + shell + victim.
# Không đọc từ log (nếu đọc từ log thì shell in sai số vẫn khớp chính nó).
PS_MIN_ROWS=3
LONG_LEN=200       # ca 4: dòng dài 200 ký tự (> MAX_LINE 64 của ảnh có FS)
WAIT_PID=13        # ca 8: pid tiến trình đang chạy (procsrv) trên ảnh đã ghim

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() {
    echo "shell-edge.sh: $*" >&2
    exit 2
}

usage() {
    # In **toàn bộ** khối chú thích đầu tệp (đến `set -u`), không neo số dòng: header dài ra
    # (Q4 vòng ba thêm ca 13) thì `--help` vẫn không bị cắt cụt.
    sed -n '2,/^set -u$/p' "${BASH_SOURCE[0]}" 2>/dev/null | sed '$d' | sed 's/^# \{0,1\}//'
    echo "shell-edge.sh: dùng: shell-edge.sh --image <ảnh.bin> [--selftest] | --selftest | --help"
}

# KT-78 (OD-K6-6): **mọi** lần mở QEMU đi qua `qemu_guard.sh` — process group riêng, timeout kill
# **cả group**, trap dọn sạch, watchdog dọn khi cha chết (kể cả SIGKILL). Không sửa guard.
. "$HERE/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG=""
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"

# ------------------------------------------------------------------ trạng thái toàn cục
IMAGE=""
MODE_SELFTEST=0
LOG=""          # log QEMU của lần chạy thật
CASE_LOG=""     # lát log của ca đang kiểm
CASE_FROM=1     # dòng bắt đầu lát log của ca đang kiểm
CASE_TIMEOUT=0  # 1 = ca vừa rồi quá hạn (shell treo) ⇒ bỏ các ca còn lại
QGB_PID=""      # PID subshell chạy `qg_boot` (QEMU nằm trong group của nó)
FIFO=""         # FIFO làm stdin QEMU
SELFTEST_ROOT=""
SE_CLEANED=0

# ------------------------------------------------------------------ dọn dẹp
# Chỉ kill **group do chính script này tạo**: đọc registry của `qemu_guard.sh` (guard ghi PGID
# ngay sau khi launch) rồi `qg_kill_group` — hàm đó chỉ kill group **có** `qemu-system` và chỉ
# group đã đăng ký. Không `pkill -f qemu-system` (AGENTS §9).
kill_own_qemu() {
    local pg
    [ -n "${QG_REG:-}" ] && [ -f "$QG_REG" ] || return 0
    while IFS= read -r pg; do
        [ -n "$pg" ] || continue
        qg_kill_group "$pg"
    done <"$QG_REG"
    return 0
}

cleanup() {
    [ "$SE_CLEANED" = 1 ] && return 0
    SE_CLEANED=1
    exec 8>&- 2>/dev/null || true
    kill_own_qemu
    if [ -n "${QGB_PID:-}" ]; then
        kill "$QGB_PID" 2>/dev/null || true
        wait "$QGB_PID" 2>/dev/null || true
    fi
    [ -n "${FIFO:-}" ] && rm -f "$FIFO" 2>/dev/null
    [ -n "${SELFTEST_ROOT:-}" ] && rm -rf "$SELFTEST_ROOT" 2>/dev/null
    # Chỉ xoá thư mục log **do chính script tạo** (không xoá khi người dùng chỉ định qua biến).
    [ -n "${SHELL_EDGE_LOGDIR_TMP:-}" ] && rm -rf "$SHELL_EDGE_LOGDIR_TMP" 2>/dev/null
    qg_cleanup
    return 0
}
trap 'cleanup; exit 130' INT
trap 'cleanup; exit 143' TERM
trap 'cleanup; exit 129' HUP
trap 'cleanup' EXIT

# =====================================================================================
# HÀM KHẲNG ĐỊNH — dùng chung cho lần chạy thật VÀ selftest.
#
# Mỗi hàm nhận **một tệp log** (lát log của ca, hoặc log giả của selftest), trả 0 = PASS,
# 1 = FAIL; khi FAIL in **một dòng lý do** ra stdout (caller chèn vào `# <tên>: FAIL (…)`).
# Không hàm nào đọc biến toàn cục của lần chạy ⇒ selftest kiểm đúng thứ harness dùng.
# =====================================================================================

# 1. `cat` không tham số ⇒ đúng một `# shell: thieu duong dan`, không `# shell: ?` (KT-104).
check_cat_no_arg() {
    local log="$1" n_ok n_bad
    n_ok="$(grep -c -x -F '# shell: thieu duong dan' "$log" 2>/dev/null || true)"
    n_bad="$(grep -c -x -F '# shell: ?' "$log" 2>/dev/null || true)"
    if [ "${n_ok:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: thieu duong dan', thấy ${n_ok:-0}"
        return 1
    fi
    if [ "${n_bad:-0}" -ne 0 ]; then
        echo "thấy ${n_bad:-0} dòng '# shell: ?' (lỗi KT-104 tái phát)"
        return 1
    fi
    return 0
}

# 2. `write` không tham số ⇒ đúng một `# shell: thieu duong dan`, không `# shell: ?` (KT-104).
check_write_no_arg() {
    local log="$1" n_ok n_bad
    n_ok="$(grep -c -x -F '# shell: thieu duong dan' "$log" 2>/dev/null || true)"
    n_bad="$(grep -c -x -F '# shell: ?' "$log" 2>/dev/null || true)"
    if [ "${n_ok:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: thieu duong dan', thấy ${n_ok:-0}"
        return 1
    fi
    if [ "${n_bad:-0}" -ne 0 ]; then
        echo "thấy ${n_bad:-0} dòng '# shell: ?' (lỗi KT-104 tái phát)"
        return 1
    fi
    return 0
}

# 3. `ps` ⇒ có bảng thật (`# shell: ps n=…` + ≥1 dòng `  pid=… state=…`) và **không** dòng nào
#    chứa `state=Unknown` (KT-105). Đòi có bảng để `ps` im lặng không thể "xanh vì rỗng".
check_ps_no_unknown() {
    local log="$1" n_ps n_rows n_unknown
    n_ps="$(grep -c -E '^# shell: ps n=[0-9]+$' "$log" 2>/dev/null || true)"
    n_rows="$(grep -c -E '^  pid=[0-9]+ state=[A-Za-z]+( suspended)?$' "$log" 2>/dev/null || true)"
    n_unknown="$(grep -c -F 'state=Unknown' "$log" 2>/dev/null || true)"
    if [ "${n_ps:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: ps n=<n>', thấy ${n_ps:-0}"
        return 1
    fi
    # **KT-120** (review toàn dự án): bản KT-112 ở đây là **kiểm rỗng** — `n_rows` và `n_declared`
    # cùng sinh từ **một** biến `count` của shell, nên `n_rows == n_declared` **luôn** đúng; `ps`
    # thoái hoá còn một tiến trình vẫn xanh (reviewer dựng lại được). Nay đòi thêm một **sàn độc
    # lập**: ảnh sản phẩm luôn có ít nhất procsrv + shell + victim ⇒ `ps` phải in **≥ 3** dòng.
    # Sàn này không đọc từ log, nên shell in sai số vẫn bị bắt.
    n_declared="$(sed -n 's/^# shell: ps n=\([0-9][0-9]*\).*/\1/p' "$log" 2>/dev/null | head -1)"
    if [ "${n_rows:-0}" -lt 1 ]; then
        echo "ps không in dòng '  pid=… state=…' nào (bảng rỗng)"
        return 1
    fi
    if [ "${n_rows:-0}" -lt "$PS_MIN_ROWS" ]; then
        echo "ps chỉ in $n_rows dòng pid, dưới sàn $PS_MIN_ROWS (procsrv + shell + victim)"
        return 1
    fi
    if [ -n "$n_declared" ] && [ "${n_rows:-0}" -ne "$n_declared" ]; then
        echo "ps khai n=$n_declared nhưng in $n_rows dòng pid — bảng thiếu dòng"
        return 1
    fi
    if [ "${n_unknown:-0}" -ne 0 ]; then
        echo "thấy ${n_unknown:-0} dòng 'state=Unknown' (lỗi KT-105 tái phát)"
        return 1
    fi
    return 0
}

# 3c. **KT-126** — hai lệnh mới `truncate` và `jobs` (KT-125) phải **thật sự** chạy trên ảnh sản
#     phẩm. Trước đây chúng không có ca nào: journal dẫn "shell-edge tám trên tám PASS" là thật
#     nhưng **không phủ** mã mới — đúng dạng xanh giả đợt này đang chống.
#     Kiểm ba vế **độc lập**:
#       (a) `truncate /README 5` in đúng dòng xác nhận;
#       (b) `stat /README` sau đó in `size=5` — chứng minh truncate **có hiệu lực**, không chỉ in;
#       (c) `jobs` in đủ bốn số và **tổng** bằng `n` của `ps` (hai lệnh đọc cùng nguồn `PS_REQ`).
check_truncate_jobs() {
    local log="$1" n_trunc n_stat n_jobs n_ps total
    n_trunc="$(grep -c -x -F '# shell: truncate path=/README size=5' "$log" 2>/dev/null || true)"
    if [ "${n_trunc:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: truncate path=/README size=5', thấy ${n_trunc:-0}"
        return 1
    fi
    n_stat="$(grep -c -x -F '# shell: stat path=/README kind=1 size=5 ino=2' "$log" 2>/dev/null || true)"
    if [ "${n_stat:-0}" -ne 1 ]; then
        echo "truncate báo xong nhưng stat không thấy size=5 (thấy ${n_stat:-0} dòng khớp)"
        return 1
    fi
    n_jobs="$(grep -c -E '^# shell: jobs ready=[0-9]+ running=[0-9]+ blocked=[0-9]+ other=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n_jobs:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: jobs ready=… running=… blocked=… other=…', thấy ${n_jobs:-0}"
        return 1
    fi
    total="$(sed -n 's/^# shell: jobs ready=\([0-9]*\) running=\([0-9]*\) blocked=\([0-9]*\) other=\([0-9]*\)$/\1+\2+\3+\4/p' "$log" 2>/dev/null | head -1)"
    if [ -z "$total" ]; then
        echo "không đọc được bốn số của jobs"
        return 1
    fi
    total=$((total))
    n_ps="$(sed -n 's/^# shell: ps n=\([0-9][0-9]*\).*/\1/p' "$log" 2>/dev/null | head -1)"
    if [ -n "$n_ps" ] && [ "$total" -ne "$n_ps" ]; then
        echo "jobs đếm tổng $total nhưng ps khai n=$n_ps — hai lệnh lệch nhau"
        return 1
    fi
    return 0
}

# =====================================================================================
# **Q4 vòng ba** (Agent 0, 2026-11-18) — phán quyết **từng lệnh**, không gộp.
#
# Vì sao: bản KT-130 in **một** dòng PASS/FAIL cho **năm** lệnh và `return 1` ngay chỗ hỏng đầu
# tiên. Hệ quả: (a) người đọc không biết lệnh nào đỏ; (b) lệnh hỏng thứ hai..năm **không được
# kiểm** trong lượt đó, nên một lỗi ở lệnh đầu che mất bốn lỗi phía sau. Nay mỗi lệnh một dòng
# `# <ca>[<lệnh>]: PASS/FAIL (lý do)`, và hàm gộp **chạy hết** mọi lệnh rồi mới kết luận.
# Mã thoát giữ nguyên (0 = xanh, 1 = đỏ) để khung gọi `run_real` không phải đổi.
# =====================================================================================

# `nth_line <log> <ERE> <n>` — số dòng (1-based) của dòng thứ `n` khớp ERE; rỗng nếu không có.
# Dùng để **quy lỗi đúng lệnh** khi hai lệnh in cùng một chuỗi (vd `stat /khongco` và
# `ls /khongco` cùng in `fs loi code=5`): shell xử lý lệnh **tuần tự**, nên thứ tự dòng trong log
# là thứ tự lệnh — dòng thứ nhất thuộc lệnh gửi trước.
nth_line() {
    grep -n -E "$2" "$1" 2>/dev/null | sed -n "${3}p" | cut -d: -f1
}

# `last_line <log> <ERE>` — số dòng của dòng **cuối** khớp ERE; rỗng nếu không có.
last_line() {
    grep -n -E "$2" "$1" 2>/dev/null | tail -1 | cut -d: -f1
}

# `last_result_line <log>` — số dòng của dòng kết quả `# shell: …` **cuối** của lát log, **bỏ**
# dòng đánh dấu `echo __END_<n>__`. Vì sao phải bỏ: `case_end` cắt lát log **sau** khi marker đã
# vào log, nên marker là dòng `# shell: …` cuối cùng; không lọc thì "dòng kết quả cuối" luôn là
# marker và phép kiểm vị trí của `sync 5` sẽ đỏ oan.
last_result_line() {
    grep -n -E '^# shell: ' "$1" 2>/dev/null | grep -v -F '# shell: echo __END_' | tail -1 | cut -d: -f1
}

# `emit_verdicts <nhãn-ca> <log> <cặp "tên-lệnh:hàm-kiểm"…>`
#
# Chạy **hết** mọi cặp (không `return` sớm), in **một** dòng cho mỗi lệnh:
#   `# <ca>[<lệnh>]: PASS`
#   `# <ca>[<lệnh>]: FAIL (<lý do>)`
# Trả 0 nếu **mọi** lệnh xanh, 1 nếu có ít nhất một lệnh đỏ — đúng mã thoát cũ để `run_real`
# không phải đổi khung. Vì sao chạy hết: `return 1` ngay chỗ hỏng đầu tiên che các lỗi sau
# (Agent 0 bắt đúng: một log sai hai lệnh chỉ in lý do lệnh đầu).
emit_verdicts() {
    local case_name="$1" log="$2"
    shift 2
    local pair name fn reason rc=0
    for pair in "$@"; do
        name="${pair%%:*}"
        fn="${pair#*:}"
        if reason="$("$fn" "$log")"; then
            echo "# ${case_name}[${name}]: PASS"
        else
            echo "# ${case_name}[${name}]: FAIL (${reason})"
            rc=1
        fi
    done
    return "$rc"
}

# --- new-cmds-ok: năm lệnh, mỗi lệnh một hàm -----------------------------------------------

# `time` ⇒ **đúng một** dòng `# shell: time ns=<n>`.
ok_time() {
    local log="$1" n
    n="$(grep -c -E '^# shell: time ns=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: time ns=<n>', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `uptime` ⇒ **đúng một** dòng `# shell: uptime ms=<n>`. Đòi **đúng một** (không phải ≥1) để
# `time` không thể "mượn" dòng của `uptime` và ngược lại.
ok_uptime() {
    local log="$1" n
    n="$(grep -c -E '^# shell: uptime ms=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: uptime ms=<n>', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `sync` ⇒ **đúng một** dòng `# shell: sync ok`.
ok_sync() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: sync ok' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: sync ok', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `stat /README` ⇒ **đúng một** dòng đủ bốn trường.
ok_stat_readme() {
    local log="$1" n
    n="$(grep -c -E '^# shell: stat path=/README kind=[0-9]+ size=[0-9]+ ino=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: stat path=/README kind=… size=… ino=…', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `ls` ⇒ **đúng một** dòng tổng kết `ls n=<n> path=/` **và** ≥1 mục thật. Bảng rỗng không đạt:
# `ls` in `n=0` mà không có mục nào là "liệt kê bừa" theo nghĩa không chứng minh được nó đọc được
# thư mục.
ok_ls_root() {
    local log="$1" n
    n="$(grep -c -E '^# shell: ls n=[0-9]+ path=/$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: ls n=<n> path=/', thấy ${n:-0}"
        return 1
    fi
    n="$(grep -c -E '^  [fdl] [A-Za-z0-9_.-]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -lt 1 ]; then
        echo "ls không liệt kê mục nào (bảng rỗng)"
        return 1
    fi
    return 0
}

# 3d. **KT-130** + **Q4 vòng ba** — ca ĐÚNG của năm lệnh `time`/`uptime`/`sync`/`stat`/`ls`.
#     Mỗi lệnh một dòng phán quyết; chạy hết rồi mới kết luận.
check_new_cmds_ok() {
    emit_verdicts "new-cmds-ok" "$1" \
        "time:ok_time" \
        "uptime:ok_uptime" \
        "sync:ok_sync" \
        "stat:ok_stat_readme" \
        "ls:ok_ls_root"
}

# --- new-cmds-edge: năm lệnh, mỗi lệnh một hàm ---------------------------------------------

# `stat` trần ⇒ `thieu duong dan`; `stat /khongco` ⇒ lỗi FS code=5. Hai vế **độc lập**: vế đầu
# chứng minh lệnh biết mình thiếu tham số (không rơi xuống `?`), vế sau chứng minh đường không có
# được báo lỗi thật.
ok_edge_stat() {
    local log="$1" n dd c1
    n="$(grep -c -x -F '# shell: thieu duong dan' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: thieu duong dan' (stat khong tham so), thấy ${n:-0}"
        return 1
    fi
    n="$(grep -c -x -F '# shell: fs loi code=5' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -lt 1 ]; then
        echo "stat /khongco không báo lỗi FS code=5"
        return 1
    fi
    # Quy lỗi đúng lệnh: dòng `thieu duong dan` của `stat` trần phải đứng **trước** dòng lỗi
    # code=5 đầu tiên (của `stat /khongco`).
    dd="$(nth_line "$log" '^# shell: thieu duong dan$' 1)"
    c1="$(nth_line "$log" '^# shell: fs loi code=5$' 1)"
    if [ -n "$dd" ] && [ -n "$c1" ] && [ "$c1" -le "$dd" ]; then
        echo "dòng lỗi code=5 đầu tiên (dòng $c1) không đứng sau 'stat' trần (dòng $dd)"
        return 1
    fi
    return 0
}

# `ls /khongco` ⇒ lỗi FS code=5. Dòng lỗi **thứ hai** phải đứng sau dòng thứ nhất và **trước**
# `thieu kich thuoc` của `truncate` — nhờ vậy dòng thứ hai chắc chắn thuộc `ls`, không phải
# `stat` in hai lần.
ok_edge_ls() {
    local log="$1" n c1 c2 tk
    n="$(grep -c -x -F '# shell: fs loi code=5' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -lt 1 ]; then
        echo "ls /khongco không báo lỗi FS code=5"
        return 1
    fi
    c1="$(nth_line "$log" '^# shell: fs loi code=5$' 1)"
    c2="$(nth_line "$log" '^# shell: fs loi code=5$' 2)"
    tk="$(nth_line "$log" '^# shell: thieu kich thuoc$' 1)"
    if [ -z "$c2" ]; then
        echo "chỉ thấy 1 dòng '# shell: fs loi code=5' — thiếu lỗi của ls /khongco"
        return 1
    fi
    if [ -n "$c1" ] && [ -n "$tk" ] && { [ "$c2" -le "$c1" ] || [ "$c2" -ge "$tk" ]; }; then
        echo "dòng lỗi code=5 thứ hai (dòng $c2) không nằm giữa stat (dòng $c1) và truncate (dòng $tk)"
        return 1
    fi
    return 0
}

# `truncate` ⇒ size không phải số báo `thieu kich thuoc`; size ≥ 2^32 bị từ chối **sớm** (không
# cắt âm thầm — KT-126).
ok_edge_truncate() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: thieu kich thuoc' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: thieu kich thuoc', thấy ${n:-0}"
        return 1
    fi
    n="$(grep -c -x -F '# shell: kich thuoc qua lon (toi da 4294967295)' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng 'kich thuoc qua lon', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `sync` hai lần ⇒ **hai** dòng `sync ok` (idempotent, không treo).
ok_edge_sync() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: sync ok' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: sync ok' (sync hai lan), thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `time` hai lần ⇒ **hai** dòng, và lần sau **≥** lần trước (đồng hồ đơn điệu).
ok_edge_time() {
    local log="$1" n t1 t2
    n="$(grep -c -E '^# shell: time ns=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: time ns=<n>' (time hai lan), thấy ${n:-0}"
        return 1
    fi
    t1="$(sed -n 's/^# shell: time ns=\([0-9][0-9]*\)$/\1/p' "$log" 2>/dev/null | sed -n '1p')"
    t2="$(sed -n 's/^# shell: time ns=\([0-9][0-9]*\)$/\1/p' "$log" 2>/dev/null | sed -n '2p')"
    if [ -z "$t1" ] || [ -z "$t2" ]; then
        echo "không đọc được hai mốc thời gian của time"
        return 1
    fi
    if [ "$t2" -lt "$t1" ]; then
        echo "đồng hồ lùi: time lần hai ($t2) < lần một ($t1)"
        return 1
    fi
    return 0
}

# Vế **chung** của ca: đúng **hai** dòng lỗi code=5 (một của `stat /khongco`, một của
# `ls /khongco`) và **không** dòng `# shell: ?` nào (mọi lệnh gửi đều là lệnh shell biết).
# Vì sao vẫn cần vế chung khi đã có vế từng lệnh: hai lệnh `stat`/`ls` in **cùng** một chuỗi, nên
# chỉ đếm tổng mới chứng minh được **cả hai** đều lỗi (mỗi lệnh in tối đa một dòng lỗi).
ok_edge_tong() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: fs loi code=5' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: fs loi code=5' (stat + ls tren duong khong co), thấy ${n:-0}"
        return 1
    fi
    n="$(grep -c -x -F '# shell: ?' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 0 ]; then
        echo "thấy ${n:-0} dòng '# shell: ?' (lệnh bị coi là lạ)"
        return 1
    fi
    return 0
}

# 3e. **KT-130** + **Q4 vòng ba** — ca **biên** cho cùng năm lệnh, cộng `truncate`/`jobs` đã có
#     ở 3c. Mỗi lệnh một dòng phán quyết; chạy hết rồi mới kết luận.
check_new_cmds_edge() {
    emit_verdicts "new-cmds-edge" "$1" \
        "stat:ok_edge_stat" \
        "ls:ok_edge_ls" \
        "truncate:ok_edge_truncate" \
        "sync:ok_edge_sync" \
        "time:ok_edge_time" \
        "tong:ok_edge_tong"
}

# --- new-cmds-edge2: `uptime` + `jobs` + hai lệnh có tham số --------------------------------

# `uptime` hai lần ⇒ **hai** dòng, đơn điệu.
ok_edge2_uptime() {
    local log="$1" n u1 u2
    n="$(grep -c -E '^# shell: uptime ms=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: uptime ms=<n>' (uptime hai lan), thấy ${n:-0}"
        return 1
    fi
    u1="$(sed -n 's/^# shell: uptime ms=\([0-9][0-9]*\)$/\1/p' "$log" 2>/dev/null | sed -n '1p')"
    u2="$(sed -n 's/^# shell: uptime ms=\([0-9][0-9]*\)$/\1/p' "$log" 2>/dev/null | sed -n '2p')"
    if [ -z "$u1" ] || [ -z "$u2" ]; then
        echo "không đọc được hai mốc uptime"
        return 1
    fi
    if [ "$u2" -lt "$u1" ]; then
        echo "đồng hồ lùi: uptime lần hai ($u2 ms) < lần một ($u1 ms)"
        return 1
    fi
    return 0
}

# `jobs` hai lần ⇒ **hai** dòng **giống nhau từng ký tự** (cùng một reply `PS_REQ`).
ok_edge2_jobs() {
    local log="$1" n j1 j2
    n="$(grep -c -E '^# shell: jobs ready=[0-9]+ running=[0-9]+ blocked=[0-9]+ other=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: jobs ready=… running=… blocked=… other=…', thấy ${n:-0}"
        return 1
    fi
    j1="$(grep -E '^# shell: jobs ready=[0-9]+ running=[0-9]+ blocked=[0-9]+ other=[0-9]+$' "$log" 2>/dev/null | sed -n '1p')"
    j2="$(grep -E '^# shell: jobs ready=[0-9]+ running=[0-9]+ blocked=[0-9]+ other=[0-9]+$' "$log" 2>/dev/null | sed -n '2p')"
    if [ "$j1" != "$j2" ]; then
        echo "jobs không idempotent: lần một '$j1' ≠ lần hai '$j2'"
        return 1
    fi
    return 0
}

# `uptime 5` ⇒ **một** dòng `# shell: ?`, và dòng đó phải nằm **giữa** `uptime` thứ hai và
# `jobs` đầu tiên — đúng vị trí lệnh `uptime 5` trong dãy gửi. Nhờ vậy nếu `uptime 5` bị bỏ qua
# im lặng (không in gì) thì dòng `?` duy nhất còn lại (của `jobs 5`) **không** nằm giữa hai mốc
# này, và ca đỏ đúng chỗ.
ok_edge2_uptime_5() {
    local log="$1" q1 u2 j1
    q1="$(nth_line "$log" '^# shell: \?$' 1)"
    u2="$(nth_line "$log" '^# shell: uptime ms=[0-9]+$' 2)"
    j1="$(nth_line "$log" '^# shell: jobs ready=' 1)"
    if [ -z "$q1" ]; then
        echo "không thấy dòng '# shell: ?' nào (uptime 5 phải là lệnh lạ)"
        return 1
    fi
    if [ -z "$u2" ] || [ -z "$j1" ]; then
        echo "thiếu dòng neo uptime/jobs để xác định vị trí của 'uptime 5'"
        return 1
    fi
    if [ "$q1" -le "$u2" ] || [ "$q1" -ge "$j1" ]; then
        echo "dòng '?' đầu tiên (dòng $q1) không nằm giữa uptime thứ hai (dòng $u2) và jobs đầu (dòng $j1) — uptime 5 bị bỏ qua im lặng?"
        return 1
    fi
    return 0
}

# `jobs 5` ⇒ **một** dòng `# shell: ?`, nằm **sau** `jobs` thứ hai (vị trí lệnh `jobs 5`). Dùng
# dòng `?` **cuối** (không phải thứ hai) để nếu `uptime 5` im lặng thì `jobs 5` vẫn được phán
# đúng — lỗi thuộc về `uptime 5`, không phải `jobs 5`.
ok_edge2_jobs_5() {
    local log="$1" q_last j2
    q_last="$(last_line "$log" '^# shell: \?$')"
    j2="$(nth_line "$log" '^# shell: jobs ready=' 2)"
    if [ -z "$q_last" ]; then
        echo "không thấy dòng '# shell: ?' nào (jobs 5 phải là lệnh lạ)"
        return 1
    fi
    if [ -z "$j2" ]; then
        echo "thiếu dòng jobs thứ hai để xác định vị trí của 'jobs 5'"
        return 1
    fi
    if [ "$q_last" -le "$j2" ]; then
        echo "dòng '?' cuối (dòng $q_last) không nằm sau jobs thứ hai (dòng $j2) — jobs 5 bị bỏ qua im lặng?"
        return 1
    fi
    return 0
}

# Vế chung: đúng **hai** dòng `?` (một cho `uptime 5`, một cho `jobs 5`).
ok_edge2_tong() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: ?' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: ?' (uptime 5 + jobs 5), thấy ${n:-0} (tham số bị bỏ qua im lặng?)"
        return 1
    fi
    return 0
}

# 3f. **KT-130** vòng hai + **Q4 vòng ba** — `uptime` + `jobs`. Mỗi lệnh một dòng phán quyết;
#     chạy hết rồi mới kết luận.
check_new_cmds_edge2() {
    emit_verdicts "new-cmds-edge2" "$1" \
        "uptime:ok_edge2_uptime" \
        "jobs:ok_edge2_jobs" \
        "uptime-5:ok_edge2_uptime_5" \
        "jobs-5:ok_edge2_jobs_5" \
        "tong:ok_edge2_tong"
}

# --- new-cmds-edge3: năm ca biên Agent 0 giao (Q4 vòng ba) ---------------------------------
#
# Hành vi **thật** đọc từ `v2/kernel/vifast-kbin-riscv64/init/shell_user.rs` (không đoán):
#   - `time 5`  : `fs_dispatch` chỉ khớp **nguyên dòng** `time`; không có nhánh `time ` ⇒ rơi
#                 xuống `run_line` ⇒ `# shell: ?`.  (shell_user.rs:430, 797)
#   - `write /a`: `starts_with(line, b"write ")` ⇒ `split_space` cho path=`/a`, text=**rỗng**;
#                 `cmd_write` bỏ vòng ghi, gọi `OP_TRUNC | 0` ⇒ cắt tệp về **0 byte** rồi `OP_SYNC`
#                 ⇒ in `# shell: write /a bytes=0 sync=ok`. **Không** báo lỗi, **không** treo.
#                 (shell_user.rs:422-428, 635-685) — đây là **chỗ lệch** với kỳ vọng "phải báo
#                 lỗi, không được ghi rỗng"; ca này ghim hành vi thật, xem báo cáo Q4 §4.
#   - `write /khongco/x abc`: `fs_set_path` chỉ **nạp** đường dẫn (không kiểm), rồi `OP_WRITE`
#                 ⇒ kernel `lookup` trượt ⇒ `create` trượt vì thư mục cha không có ⇒
#                 `FsError::NotFound` ⇒ `err_code` = 5 ⇒ `# shell: fs loi code=5`.
#                 (proc_fs.rs:546-570, 224-241)
#   - `ls /README`: `fs.lookup` thấy tệp, rồi `read_dir_entry_full` gặp `kind != INODE_DIR` ⇒
#                 `FsError::NotDir` ⇒ `err_code` = 7 ⇒ `# shell: fs loi code=7`; **không** in
#                 dòng `ls n=…` nào (không liệt kê bừa). (proc_fs.rs:479-514, 2445-2453)
#   - `sync 5`  : như `time 5` — chỉ khớp nguyên dòng `sync` ⇒ `# shell: ?`. (shell_user.rs:436)

# `time 5` ⇒ **một** dòng `# shell: ?` là **dòng kết quả đầu tiên** của ca (vì `time 5` là lệnh
# đầu), và **không** dòng `time ns=` nào — tham số không được bỏ qua im lặng, cũng không được
# "chạy như không có tham số".
ok_edge3_time_arg() {
    local log="$1" q1 l1 n
    q1="$(nth_line "$log" '^# shell: \?$' 1)"
    l1="$(nth_line "$log" '^# shell: ' 1)"
    if [ -z "$q1" ]; then
        echo "không thấy dòng '# shell: ?' nào (time 5 phải là lệnh lạ)"
        return 1
    fi
    if [ -z "$l1" ]; then
        echo "không thấy dòng kết quả '# shell: …' nào trong lát log"
        return 1
    fi
    if [ "$q1" -ne "$l1" ]; then
        echo "dòng '?' đầu tiên (dòng $q1) không phải dòng kết quả đầu (dòng $l1) — time 5 bị bỏ qua im lặng?"
        return 1
    fi
    n="$(grep -c -E '^# shell: time ns=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 0 ]; then
        echo "thấy ${n:-0} dòng '# shell: time ns=…' — 'time 5' chạy như không có tham số"
        return 1
    fi
    return 0
}

# `write /a` (đường dẫn, **không** nội dung) ⇒ hành vi thật: cắt tệp về 0 byte và **nói thật**
# `bytes=0`; `stat /a` sau đó phải thấy `size=0`. Đòi cả hai để một `write` im lặng (không in gì)
# hoặc một `write` ghi rỗng mà không nói cũng đỏ.
ok_edge3_write_no_text() {
    local log="$1" n8 n0 ns
    n8="$(grep -c -x -F '# shell: write /a bytes=8 sync=ok' "$log" 2>/dev/null || true)"
    if [ "${n8:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: write /a bytes=8 sync=ok' (tiền đề), thấy ${n8:-0}"
        return 1
    fi
    n0="$(grep -c -x -F '# shell: write /a bytes=0 sync=ok' "$log" 2>/dev/null || true)"
    if [ "${n0:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: write /a bytes=0 sync=ok' (write /a khong noi dung), thấy ${n0:-0}"
        return 1
    fi
    ns="$(grep -c -E '^# shell: stat path=/a kind=[0-9]+ size=0 ino=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${ns:-0}" -ne 1 ]; then
        echo "stat /a không thấy size=0 sau 'write /a' (thấy ${ns:-0} dòng khớp) — ghi rỗng không có hiệu lực?"
        return 1
    fi
    return 0
}

# `write /khongco/x abc` ⇒ **một** dòng `# shell: fs loi code=5` (NotFound), nằm **sau** `stat /a`
# (đúng vị trí lệnh trong dãy gửi) — không treo, không tạo tệp rỗng.
ok_edge3_write_missing_dir() {
    local log="$1" n c5 sa
    n="$(grep -c -x -F '# shell: fs loi code=5' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: fs loi code=5' (write vao thu muc khong co), thấy ${n:-0}"
        return 1
    fi
    c5="$(nth_line "$log" '^# shell: fs loi code=5$' 1)"
    sa="$(nth_line "$log" '^# shell: stat path=/a ' 1)"
    if [ -n "$c5" ] && [ -n "$sa" ] && [ "$c5" -le "$sa" ]; then
        echo "dòng lỗi code=5 (dòng $c5) không đứng sau 'stat /a' (dòng $sa) — không phải lỗi của write /khongco/x"
        return 1
    fi
    return 0
}

# `ls /README` (tệp, không phải thư mục) ⇒ **một** dòng `# shell: fs loi code=7` (NotDir) và
# **không** dòng `ls n=…` nào — không được liệt kê bừa.
ok_edge3_ls_file() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: fs loi code=7' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: fs loi code=7' (ls len tep), thấy ${n:-0}"
        return 1
    fi
    n="$(grep -c -E '^# shell: ls n=[0-9]+ path=' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 0 ]; then
        echo "thấy ${n:-0} dòng '# shell: ls n=…' — ls lên tệp vẫn liệt kê (liệt kê bừa)"
        return 1
    fi
    return 0
}

# `sync 5` ⇒ **một** dòng `# shell: ?` là **dòng kết quả cuối** của ca (vì `sync 5` là lệnh
# cuối), và **không** dòng `sync ok` nào — tham số không được bỏ qua im lặng.
ok_edge3_sync_arg() {
    local log="$1" q_last l_last n
    q_last="$(last_line "$log" '^# shell: \?$')"
    l_last="$(last_result_line "$log")"
    if [ -z "$q_last" ]; then
        echo "không thấy dòng '# shell: ?' nào (sync 5 phải là lệnh lạ)"
        return 1
    fi
    if [ -z "$l_last" ]; then
        echo "không thấy dòng kết quả '# shell: …' nào trong lát log"
        return 1
    fi
    if [ "$q_last" -ne "$l_last" ]; then
        echo "dòng '?' cuối (dòng $q_last) không phải dòng kết quả cuối (dòng $l_last) — sync 5 bị bỏ qua im lặng?"
        return 1
    fi
    n="$(grep -c -x -F '# shell: sync ok' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 0 ]; then
        echo "thấy ${n:-0} dòng '# shell: sync ok' — 'sync 5' chạy như không có tham số"
        return 1
    fi
    return 0
}

# 3g. **Q4 vòng ba** — năm ca biên còn thiếu: `time` có tham số, `write` có đường dẫn nhưng
#     thiếu nội dung, `write` vào thư mục không có, `ls` lên **tệp**, `sync` có tham số.
check_new_cmds_edge3() {
    emit_verdicts "new-cmds-edge3" "$1" \
        "time-tham-so:ok_edge3_time_arg" \
        "write-thieu-noi-dung:ok_edge3_write_no_text" \
        "write-thu-muc-khong-co:ok_edge3_write_missing_dir" \
        "ls-tep:ok_edge3_ls_file" \
        "sync-tham-so:ok_edge3_sync_arg"
}

# 4. Dòng dài hơn `MAX_LINE` ⇒ **đúng một** `# shell: dong qua dai` (KT-103). 0 dòng = im lặng
#    (không báo), >1 = báo mỗi byte (lỗi cũ) — cả hai đều ĐỎ.
check_long_line_once() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: dong qua dai' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: dong qua dai', thấy ${n:-0} (KT-103: một lần mỗi dòng)"
        return 1
    fi
    return 0
}

# 5. `cat /khongco` ⇒ `# shell: fs loi code=5` (không tìm thấy).
check_cat_missing() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: fs loi code=5' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: fs loi code=5', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# 6. `write /a xin chao` rồi `cat /a` ⇒ `bytes=8` và `first=xin chao`.
check_write_cat() {
    local log="$1" nw nc
    nw="$(grep -c -x -F '# shell: write /a bytes=8 sync=ok' "$log" 2>/dev/null || true)"
    nc="$(grep -c -x -F '# shell: cat ok bytes=8 first=xin chao' "$log" 2>/dev/null || true)"
    if [ "${nw:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: write /a bytes=8 sync=ok', thấy ${nw:-0}"
        return 1
    fi
    if [ "${nc:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: cat ok bytes=8 first=xin chao', thấy ${nc:-0}"
        return 1
    fi
    return 0
}

# 7. `write /g <40>` rồi `write /g ab` rồi `cat /g` ⇒ `bytes=2` (ghi đè **co lại** đúng: TRUNC).
check_write_shrink() {
    local log="$1" n40 n2 nc
    n40="$(grep -c -x -F '# shell: write /g bytes=40 sync=ok' "$log" 2>/dev/null || true)"
    n2="$(grep -c -x -F '# shell: write /g bytes=2 sync=ok' "$log" 2>/dev/null || true)"
    nc="$(grep -c -x -F '# shell: cat ok bytes=2 first=ab' "$log" 2>/dev/null || true)"
    if [ "${n40:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: write /g bytes=40 sync=ok', thấy ${n40:-0}"
        return 1
    fi
    if [ "${n2:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: write /g bytes=2 sync=ok', thấy ${n2:-0}"
        return 1
    fi
    if [ "${nc:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: cat ok bytes=2 first=ab', thấy ${nc:-0} (ghi đè không co lại?)"
        return 1
    fi
    return 0
}

# 8. `wait 13` (tiến trình đang chạy) ⇒ trả lời trong `WAIT_BUDGET_S` giây và shell **không treo**
#    (lệnh `echo sau-wait` gửi ngay sau đó vẫn chạy). `$2` = số giây đo được từ lúc gửi tới lúc
#    thấy dòng trả lời; 999 = không thấy (treo).
check_wait_no_hang() {
    local log="$1" dt="${2:-999}" nw ne
    case "$dt" in '' | *[!0-9]*) dt=999 ;; esac
    nw="$(grep -c -E "^# shell: wait pid=${WAIT_PID} code=[0-9]+$" "$log" 2>/dev/null || true)"
    ne="$(grep -c -x -F '# shell: echo sau-wait' "$log" 2>/dev/null || true)"
    if [ "${nw:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: wait pid=${WAIT_PID} code=<n>', thấy ${nw:-0} (shell treo ở wait?)"
        return 1
    fi
    if [ "${ne:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: echo sau-wait', thấy ${ne:-0} (shell treo sau wait)"
        return 1
    fi
    if [ "$dt" -gt "$WAIT_BUDGET_S" ]; then
        echo "wait trả lời sau ${dt}s > ${WAIT_BUDGET_S}s (quá hạn)"
        return 1
    fi
    return 0
}

# =====================================================================================
# SELFTEST — chứng minh các hàm khẳng định ở trên **bắt được lỗi thật**.
#
# Mỗi ca: một thư mục `mktemp -d` riêng, hai log giả:
#   - `wrong.log`: đầu ra **của lỗi cũ** (đúng thứ lỗi KT-103/104/105 từng in) ⇒ hàm phải trả 1;
#   - `right.log`: đầu ra **đúng** ⇒ hàm phải trả 0.
# Ca 8 có thêm một log "đúng nội dung nhưng quá chậm" (dt=99 s) ⇒ vẫn phải ĐỎ.
# =====================================================================================
SELFTEST_CASES=8
SELFTEST_CHECKS=0
SELFTEST_OK=0
SELFTEST_FAIL=0
# **KT-130** (vòng hai): `SELFTEST_CASES` đếm **ca có số** (1..8) trong chú thích đầu tệp, **không**
# phải số log-đúng. Từ ca 3b trở đi mỗi ca có **nhiều** cặp log, nên dòng tổng kết cũ in
# `$((SELFTEST_CHECKS - SELFTEST_CASES)) sai→ĐỎ + $SELFTEST_CASES đúng→XANH` — sai số (27 kiểm mà
# in "19 sai + 8 đúng", thật là 15 sai + 12 đúng). Đếm riêng số log-đúng để dòng tổng kết không nói
# sai nữa; **không** đổi phán quyết PASS/FAIL nào.
SELFTEST_RIGHT=0

# `selftest_pair <nhãn> <hàm> <log-sai> <log-đúng> [tham số thêm…]`
selftest_pair() {
    local label="$1" checker="$2" wrong="$3" right="$4" reason
    shift 4
    SELFTEST_CHECKS=$((SELFTEST_CHECKS + 1))
    if reason="$("$checker" "$wrong" "$@")"; then
        echo "# selftest $label: log SAI -> FAIL (harness KHÔNG bắt được; lỗi giả lọt qua)"
        SELFTEST_FAIL=$((SELFTEST_FAIL + 1))
    else
        echo "# selftest $label: log SAI -> ĐỎ đúng ($reason)"
        SELFTEST_OK=$((SELFTEST_OK + 1))
    fi
    SELFTEST_CHECKS=$((SELFTEST_CHECKS + 1))
    if reason="$("$checker" "$right" "$@")"; then
        echo "# selftest $label: log ĐÚNG -> XANH đúng"
        SELFTEST_OK=$((SELFTEST_OK + 1))
        SELFTEST_RIGHT=$((SELFTEST_RIGHT + 1))
    else
        echo "# selftest $label: log ĐÚNG -> FAIL (harness báo đỏ giả: $reason)"
        SELFTEST_FAIL=$((SELFTEST_FAIL + 1))
    fi
    return 0
}

# `selftest_red <nhãn> <hàm> <log> [tham số thêm…]` — log phải ĐỎ (ca phụ: nhánh "không ?",
# và ca 8 "đúng nội dung nhưng quá chậm").
selftest_red() {
    local label="$1" checker="$2" log="$3" reason
    shift 3
    SELFTEST_CHECKS=$((SELFTEST_CHECKS + 1))
    if reason="$("$checker" "$log" "$@")"; then
        echo "# selftest $label: log SAI -> FAIL (harness KHÔNG bắt được; lỗi giả lọt qua)"
        SELFTEST_FAIL=$((SELFTEST_FAIL + 1))
    else
        echo "# selftest $label: log SAI -> ĐỎ đúng ($reason)"
        SELFTEST_OK=$((SELFTEST_OK + 1))
    fi
    return 0
}

# `selftest_verdicts <nhãn> <hàm-gộp> <log> <mã-thoát-mong> <số-PASS-mong> <số-FAIL-mong> [mẫu…]`
#
# Dùng cho bốn ca đã tách phán quyết từng lệnh. Khác `selftest_pair` ở chỗ nó kiểm **cả** mã
# thoát, **cả** số dòng phán quyết, **cả** nội dung lý do — nhờ vậy chứng minh được yêu cầu
# "log sai hai lệnh ⇒ thông điệp đỏ nêu **cả hai**": truyền hai mẫu `[time]: FAIL` và
# `[sync]: FAIL` và cả hai đều phải có. Nếu hàm gộp `return 1` ngay lệnh hỏng đầu (lỗi cũ) thì
# mẫu thứ hai sẽ thiếu và ca selftest này đỏ — đúng thứ nó cần bắt.
selftest_verdicts() {
    local label="$1" checker="$2" log="$3" want_rc="$4" want_pass="$5" want_fail="$6"
    shift 6
    local out rc npass nfail pat missing=""
    SELFTEST_CHECKS=$((SELFTEST_CHECKS + 1))
    out="$("$checker" "$log")"
    rc=$?
    npass="$(printf '%s\n' "$out" | grep -c -E '^# [A-Za-z0-9-]+\[[A-Za-z0-9-]+\]: PASS$' || true)"
    nfail="$(printf '%s\n' "$out" | grep -c -E '^# [A-Za-z0-9-]+\[[A-Za-z0-9-]+\]: FAIL \(' || true)"
    for pat in "$@"; do
        printf '%s\n' "$out" | grep -q -F "$pat" || missing="$missing [$pat]"
    done
    if [ "$rc" -ne "$want_rc" ]; then
        echo "# selftest $label: SAI mã thoát (mong $want_rc, được $rc)"
        SELFTEST_FAIL=$((SELFTEST_FAIL + 1))
        return 0
    fi
    if [ "$npass" -ne "$want_pass" ] || [ "$nfail" -ne "$want_fail" ]; then
        echo "# selftest $label: SAI số dòng phán quyết (mong $want_pass PASS + $want_fail FAIL, được $npass PASS + $nfail FAIL)"
        SELFTEST_FAIL=$((SELFTEST_FAIL + 1))
        return 0
    fi
    if [ -n "$missing" ]; then
        echo "# selftest $label: thiếu lý do:$missing"
        SELFTEST_FAIL=$((SELFTEST_FAIL + 1))
        return 0
    fi
    echo "# selftest $label: ĐÚNG ($npass PASS + $nfail FAIL, mã thoát $rc)"
    SELFTEST_OK=$((SELFTEST_OK + 1))
    if [ "$want_rc" -eq 0 ]; then
        SELFTEST_RIGHT=$((SELFTEST_RIGHT + 1))
    fi
    return 0
}

run_selftest() {
    local root d wrong right both i
    root="$(mktemp -d "${TMPDIR:-/tmp}/kt108-selftest.XXXXXX" 2>/dev/null)" ||
        {
            echo "# shell-edge-selftest: FAIL (không tạo được mktemp -d)"
            return 1
        }
    SELFTEST_ROOT="$root"
    echo "# shell-edge-selftest: bắt đầu — log giả trong $root (không cần QEMU/ảnh)"

    # ---- ca 1: `cat` không tham số. Lỗi cũ in `# shell: ?`.
    d="$(mktemp -d "$root/c1.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    printf '# shell: ?\n' >"$wrong"
    printf '# shell: thieu duong dan\n' >"$right"
    selftest_pair "1 cat-khong-tham-so" check_cat_no_arg "$wrong" "$right"
    # Ca phụ: log vừa có 'thieu duong dan' vừa có '# shell: ?' ⇒ vẫn phải ĐỎ (nhánh "không ?").
    d="$(mktemp -d "$root/c1b.XXXXXX")"
    both="$d/both.log"
    {
        printf '# shell: thieu duong dan\n'
        printf '# shell: ?\n'
    } >"$both"
    selftest_red "1 cat-khong-tham-so (co ca hai)" check_cat_no_arg "$both"

    # ---- ca 2: `write` không tham số. Lỗi cũ in `# shell: ?`.
    d="$(mktemp -d "$root/c2.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    printf '# shell: ?\n' >"$wrong"
    printf '# shell: thieu duong dan\n' >"$right"
    selftest_pair "2 write-khong-tham-so" check_write_no_arg "$wrong" "$right"
    # Ca phụ: vừa có 'thieu duong dan' vừa có '# shell: ?' ⇒ vẫn phải ĐỎ.
    d="$(mktemp -d "$root/c2b.XXXXXX")"
    both="$d/both.log"
    {
        printf '# shell: thieu duong dan\n'
        printf '# shell: ?\n'
    } >"$both"
    selftest_red "2 write-khong-tham-so (co ca hai)" check_write_no_arg "$both"

    # ---- ca 3: `ps`. Lỗi cũ in `state=Unknown` cho mọi tiến trình.
    d="$(mktemp -d "$root/c3.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: ps n=3\n'
        printf '  pid=13 state=Unknown\n'
        printf '  pid=15 state=Unknown\n'
        printf '  pid=17 state=Unknown suspended\n'
    } >"$wrong"
    {
        printf '# shell: ps n=3\n'
        printf '  pid=13 state=Ready\n'
        printf '  pid=15 state=Ready\n'
        printf '  pid=17 state=Ready\n'
    } >"$right"
    selftest_pair "3 ps-khong-unknown" check_ps_no_unknown "$wrong" "$right"

    # ---- ca 3b (**KT-120**): `ps` thoái hoá còn **một** tiến trình — không có `state=Unknown`,
    #      `n` khớp số dòng, nên bản KT-112 để lọt. Sàn độc lập `PS_MIN_ROWS` phải bắt.
    d="$(mktemp -d "$root/c3b.XXXXXX")"
    wrong="$d/wrong.log"
    {
        printf '# shell: ps n=1\n'
        printf '  pid=13 state=Ready\n'
    } >"$wrong"
    # **KT-123** (review chéo): ca 3b phải có `right.log` **riêng** — trước dùng lại `$right` của
    # ca 3, nên nếu ai sửa ca 3 thành một dòng thì 3b sẽ đỏ oan.
    right="$d/right.log"
    {
        printf '# shell: ps n=3\n'
        printf '  pid=13 state=Ready\n'
        printf '  pid=15 state=Ready\n'
        printf '  pid=17 state=Ready\n'
    } >"$right"
    selftest_pair "3b ps-mot-dong" check_ps_no_unknown "$wrong" "$right"

    # ---- ca 3c (**KT-126**): `truncate` + `jobs` — hai lệnh mới phải có ca thật.
    d="$(mktemp -d "$root/c3c.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: truncate path=/README size=5\n'
        printf '# shell: stat path=/README kind=1 size=79 ino=2\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: ps n=3\n'
    } >"$wrong"
    {
        printf '# shell: truncate path=/README size=5\n'
        printf '# shell: stat path=/README kind=1 size=5 ino=2\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: ps n=3\n'
    } >"$right"
    selftest_pair "3c truncate-jobs" check_truncate_jobs "$wrong" "$right"

    # ---- ca 3d (**KT-130** + **Q4 vòng ba**): năm lệnh `time`/`uptime`/`sync`/`stat`/`ls` — ca ĐÚNG.
    #      Log SAI cố ý hỏng **ba** lệnh (time thiếu, sync thiếu, ls bảng rỗng) để chứng minh
    #      thông điệp đỏ nêu **cả ba**, không chỉ lệnh đầu — đúng yêu cầu Agent 0 (log sai hai
    #      lệnh trở lên phải nêu hết). Đòi **đúng** 2 PASS + 3 FAIL: nếu hàm gộp `return 1` ngay
    #      lệnh `time` (lỗi cũ) thì chỉ còn 1 FAIL và ca selftest này đỏ.
    d="$(mktemp -d "$root/c3d.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: uptime ms=0\n'
        printf '# shell: stat path=/README kind=1 size=79 ino=2\n'
        printf '# shell: ls n=0 path=/\n'
        printf '# shell: echo __END_10__\n'
    } >"$wrong"
    {
        printf '# shell: time ns=1000\n'
        printf '# shell: uptime ms=0\n'
        printf '# shell: sync ok\n'
        printf '# shell: stat path=/README kind=1 size=79 ino=2\n'
        printf '# shell: ls n=1 path=/\n'
        printf '  f README\n'
        printf '# shell: echo __END_10__\n'
    } >"$right"
    selftest_verdicts "3d new-cmds-ok (sai)" check_new_cmds_ok "$wrong" 1 2 3 \
        'new-cmds-ok[time]: FAIL' 'new-cmds-ok[sync]: FAIL' 'new-cmds-ok[ls]: FAIL'
    selftest_verdicts "3d new-cmds-ok (dung)" check_new_cmds_ok "$right" 0 5 0

    # ---- ca 3e (**KT-130** + **Q4 vòng ba**): ca BIÊN của cùng bộ lệnh.
    #      Log SAI cố ý hỏng **hai** lệnh: `truncate` size quá lớn **không** bị từ chối (in dòng
    #      thành công) và đồng hồ `time` **lùi** ⇒ thông điệp đỏ phải nêu **cả hai**.
    d="$(mktemp -d "$root/c3e.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: thieu duong dan\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: thieu kich thuoc\n'
        printf '# shell: truncate path=/README size=4294967296\n'
        printf '# shell: sync ok\n'
        printf '# shell: sync ok\n'
        printf '# shell: time ns=2000\n'
        printf '# shell: time ns=1000\n'
        printf '# shell: echo __END_11__\n'
    } >"$wrong"
    {
        printf '# shell: thieu duong dan\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: thieu kich thuoc\n'
        printf '# shell: kich thuoc qua lon (toi da 4294967295)\n'
        printf '# shell: sync ok\n'
        printf '# shell: sync ok\n'
        printf '# shell: time ns=1000\n'
        printf '# shell: time ns=2000\n'
        printf '# shell: echo __END_11__\n'
    } >"$right"
    selftest_verdicts "3e new-cmds-edge (sai)" check_new_cmds_edge "$wrong" 1 4 2 \
        'new-cmds-edge[truncate]: FAIL' 'new-cmds-edge[time]: FAIL'
    selftest_verdicts "3e new-cmds-edge (dung)" check_new_cmds_edge "$right" 0 6 0

    # ---- ca 3f (**KT-130** vòng hai + **Q4 vòng ba**): `uptime` + `jobs`.
    #      Log SAI cố ý hỏng **hai** lệnh: `uptime` lùi và `jobs` không idempotent ⇒ cả hai lý do
    #      phải xuất hiện.
    d="$(mktemp -d "$root/c3f.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: uptime ms=2000\n'
        printf '# shell: uptime ms=1000\n'
        printf '# shell: ?\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: jobs ready=2 running=1 blocked=0 other=0\n'
        printf '# shell: ?\n'
        printf '# shell: echo __END_12__\n'
    } >"$wrong"
    {
        printf '# shell: uptime ms=1000\n'
        printf '# shell: uptime ms=2000\n'
        printf '# shell: ?\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: ?\n'
        printf '# shell: echo __END_12__\n'
    } >"$right"
    selftest_verdicts "3f new-cmds-edge2 (sai)" check_new_cmds_edge2 "$wrong" 1 3 2 \
        'new-cmds-edge2[uptime]: FAIL' 'new-cmds-edge2[jobs]: FAIL'
    selftest_verdicts "3f new-cmds-edge2 (dung)" check_new_cmds_edge2 "$right" 0 5 0
    # Ca phụ: `uptime 5` **bị bỏ qua im lặng** (chỉ còn một dòng `?`, của `jobs 5`) ⇒ phán quyết
    # `uptime-5` phải đỏ và nêu đúng lý do — chứng minh vế **vị trí** bắt được đúng lệnh, không
    # chỉ đếm tổng. `jobs-5` vẫn xanh (nó thật sự in `?`), nên lỗi không bị quy oan.
    d="$(mktemp -d "$root/c3f1.XXXXXX")"
    wrong="$d/wrong.log"
    {
        printf '# shell: uptime ms=1000\n'
        printf '# shell: uptime ms=2000\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: ?\n'
        printf '# shell: echo __END_12__\n'
    } >"$wrong"
    selftest_verdicts "3f new-cmds-edge2 (uptime 5 bi bo qua)" check_new_cmds_edge2 "$wrong" 1 3 2 \
        'new-cmds-edge2[uptime-5]: FAIL' 'new-cmds-edge2[tong]: FAIL'

    # ---- ca 3g (**Q4 vòng ba**): năm ca biên mới — `time 5`, `write /a` (thiếu nội dung),
    #      `write /khongco/x abc`, `ls /README`, `sync 5`.
    #      Log SAI cố ý hỏng **hai** lệnh: `time 5` bị bỏ qua im lặng (không in `?`) và
    #      `ls /README` liệt kê thay vì báo lỗi ⇒ thông điệp đỏ phải nêu **cả hai**.
    d="$(mktemp -d "$root/c3g.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: write /a bytes=8 sync=ok\n'
        printf '# shell: write /a bytes=0 sync=ok\n'
        printf '# shell: stat path=/a kind=1 size=0 ino=3\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: ls n=1 path=/README\n'
        printf '  f README\n'
        printf '# shell: ?\n'
        printf '# shell: echo __END_13__\n'
    } >"$wrong"
    {
        printf '# shell: ?\n'
        printf '# shell: write /a bytes=8 sync=ok\n'
        printf '# shell: write /a bytes=0 sync=ok\n'
        printf '# shell: stat path=/a kind=1 size=0 ino=3\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: fs loi code=7\n'
        printf '# shell: ?\n'
        printf '# shell: echo __END_13__\n'
    } >"$right"
    selftest_verdicts "3g new-cmds-edge3 (sai)" check_new_cmds_edge3 "$wrong" 1 3 2 \
        'new-cmds-edge3[time-tham-so]: FAIL' 'new-cmds-edge3[ls-tep]: FAIL'
    selftest_verdicts "3g new-cmds-edge3 (dung)" check_new_cmds_edge3 "$right" 0 5 0

    # ---- ca 4: dòng dài. Lỗi cũ in một cảnh báo **mỗi byte** (100 dòng cho dòng 200 ký tự).
    d="$(mktemp -d "$root/c4.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    : >"$wrong"
    i=0
    while [ "$i" -lt 100 ]; do
        printf '# shell: dong qua dai\n' >>"$wrong"
        i=$((i + 1))
    done
    printf '# shell: dong qua dai\n' >"$right"
    selftest_pair "4 dong-dai-mot-canh-bao" check_long_line_once "$wrong" "$right"

    # ---- ca 5: `cat /khongco`. Log sai dùng mã lỗi khác (2) — phải ĐỎ.
    d="$(mktemp -d "$root/c5.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    printf '# shell: fs loi code=2\n' >"$wrong"
    printf '# shell: fs loi code=5\n' >"$right"
    selftest_pair "5 cat-khong-co" check_cat_missing "$wrong" "$right"

    # ---- ca 6: `write /a xin chao` + `cat /a`. Log sai: bytes/first lệch.
    d="$(mktemp -d "$root/c6.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: write /a bytes=8 sync=ok\n'
        printf 'xin cha\n'
        printf '# shell: cat ok bytes=7 first=xin cha\n'
    } >"$wrong"
    {
        printf '# shell: write /a bytes=8 sync=ok\n'
        printf 'xin chao\n'
        printf '# shell: cat ok bytes=8 first=xin chao\n'
    } >"$right"
    selftest_pair "6 write-cat-bytes" check_write_cat "$wrong" "$right"

    # ---- ca 7: ghi đè co lại. Log sai: `cat` vẫn thấy 40 byte (TRUNC không chạy).
    d="$(mktemp -d "$root/c7.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: write /g bytes=40 sync=ok\n'
        printf '# shell: write /g bytes=2 sync=ok\n'
        printf 'BBBBBBBB\n'
        printf '# shell: cat ok bytes=40 first=BBBBBBBB\n'
    } >"$wrong"
    {
        printf '# shell: write /g bytes=40 sync=ok\n'
        printf '# shell: write /g bytes=2 sync=ok\n'
        printf 'ab\n'
        printf '# shell: cat ok bytes=2 first=ab\n'
    } >"$right"
    selftest_pair "7 write-de-co-lai" check_write_shrink "$wrong" "$right"

    # ---- ca 8: `wait 13` không treo. Log sai: không có dòng trả lời (treo) + dt=99 s.
    d="$(mktemp -d "$root/c8.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    printf '# shell: echo truoc-wait\n' >"$wrong"
    {
        printf '# shell: wait pid=13 code=65535\n'
        printf '# shell: echo sau-wait\n'
    } >"$right"
    selftest_pair "8 wait-khong-treo" check_wait_no_hang "$wrong" "$right" 0
    # Ca phụ: nội dung đúng nhưng `wait` trả lời quá chậm (99 s) ⇒ vẫn phải ĐỎ.
    selftest_red "8 wait-khong-treo (cham)" check_wait_no_hang "$right" 99

    if [ "$SELFTEST_FAIL" -eq 0 ]; then
        echo "# shell-edge-selftest: PASS ($SELFTEST_CASES/$SELFTEST_CASES ca, $SELFTEST_OK/$SELFTEST_CHECKS kiểm: $((SELFTEST_CHECKS - SELFTEST_RIGHT)) sai→ĐỎ + $SELFTEST_RIGHT đúng→XANH)"
        return 0
    fi
    echo "# shell-edge-selftest: FAIL ($SELFTEST_FAIL/$SELFTEST_CHECKS kiểm sai)"
    return 1
}

# =====================================================================================
# LẦN CHẠY THẬT — một QEMU trên ảnh đã ghim, gửi lệnh qua stdin, kiểm lát log từng ca.
# =====================================================================================

# Gửi **một** dòng lệnh tới shell qua stdin QEMU (UART → ring → shell).
send_line() {
    printf '%s\n' "$1" >&8 2>/dev/null || return 1
    return 0
}

# `wait_re <ERE> <số vòng>` — chờ dòng khớp trong `$LOG`; 0 = thấy, 1 = hết hạn/QEMU chết.
wait_re() {
    local re="$1" max="$2" i=0
    while [ "$i" -lt "$max" ]; do
        grep -qE "$re" "$LOG" 2>/dev/null && return 0
        kill -0 "$QGB_PID" 2>/dev/null || break
        sleep "$POLL_S"
        i=$((i + 1))
    done
    grep -qE "$re" "$LOG" 2>/dev/null
}

# Bắt đầu một ca: ghi lại dòng log hiện tại để lát log chỉ chứa output của ca này.
case_begin() {
    CASE_FROM=$(( $(wc -l <"$LOG" 2>/dev/null || echo 0) + 1 ))
}

# Kết thúc một ca: gửi lệnh đánh dấu, chờ nó, rồi cắt lát log `[CASE_FROM, hết]` vào `$CASE_LOG`.
# 0 = thấy marker (mọi output của ca đã vào log); 1 = quá hạn (shell treo).
case_end() {
    local n="$1"
    send_line "echo __END_${n}__" || return 1
    wait_re "^# shell: echo __END_${n}__$" "$CASE_POLLS" || return 1
    tail -n +"$CASE_FROM" "$LOG" >"$CASE_LOG" 2>/dev/null || return 1
    return 0
}

# `run_case <n> <tên> <hàm-kiểm> <lệnh…>` — gửi lệnh, chờ marker, kiểm lát log, in PASS/FAIL.
run_case() {
    local n="$1" name="$2" checker="$3" c reason
    shift 3
    case_begin
    for c in "$@"; do
        send_line "$c" || {
            echo "# $name: FAIL (không gửi được lệnh tới stdin QEMU)"
            CASE_TIMEOUT=1
            return 1
        }
    done
    if ! case_end "$n"; then
        echo "# $name: FAIL (quá hạn: không thấy marker __END_${n}__ trong $((CASE_POLLS * 5 / 100)) s — shell treo?)"
        CASE_TIMEOUT=1
        return 1
    fi
    if reason="$("$checker" "$CASE_LOG")"; then
        echo "# $name: PASS"
        return 0
    fi
    echo "# $name: FAIL ($reason)"
    return 1
}

# Ca 8 riêng: đo thời gian `wait 13` trả lời (không dùng `run_case` vì cần mốc thời gian).
run_case_wait() {
    local name="wait-khong-treo" t0 t1 dt reason
    case_begin
    t0="$(date +%s)"
    if ! send_line "wait ${WAIT_PID}"; then
        echo "# $name: FAIL (không gửi được 'wait ${WAIT_PID}')"
        CASE_TIMEOUT=1
        return 1
    fi
    if ! wait_re "^# shell: wait pid=${WAIT_PID} code=[0-9]+$" "$CASE_POLLS"; then
        echo "# $name: FAIL (quá hạn: 'wait ${WAIT_PID}' không trả lời trong $((CASE_POLLS * 5 / 100)) s — shell treo)"
        CASE_TIMEOUT=1
        return 1
    fi
    t1="$(date +%s)"
    dt=$((t1 - t0))
    if ! send_line 'echo sau-wait'; then
        echo "# $name: FAIL (không gửi được 'echo sau-wait')"
        CASE_TIMEOUT=1
        return 1
    fi
    if ! case_end 8; then
        echo "# $name: FAIL (quá hạn: 'echo sau-wait' không chạy sau wait — shell treo)"
        CASE_TIMEOUT=1
        return 1
    fi
    if reason="$(check_wait_no_hang "$CASE_LOG" "$dt")"; then
        echo "# $name: PASS"
        echo "#   wait ${WAIT_PID}: trả lời sau ${dt}s (hạn ${WAIT_BUDGET_S}s); 'echo sau-wait' chạy"
        return 0
    fi
    echo "# $name: FAIL ($reason)"
    return 1
}

# In FAIL cho các ca chưa chạy (khi một ca trước đã treo shell) — không giả xanh.
mark_remaining_fail() {
    local from="$1" to="$2" i n
    local names="cat-khong-tham-so write-khong-tham-so ps-khong-unknown dong-dai-mot-canh-bao cat-khong-co write-cat-bytes write-de-co-lai wait-khong-treo"
    i=1
    for n in $names; do
        if [ "$i" -ge "$from" ] && [ "$i" -le "$to" ]; then
            echo "# $n: FAIL (không chạy — shell treo ở ca trước)"
        fi
        i=$((i + 1))
    done
}

# ------------------------------------------------------------------ preflight ảnh
preflight_image() {
    [ -f "$IMAGE" ] || die "không thấy ảnh: $IMAGE"
    # Ảnh phải có shell (nếu không, mọi ca sẽ chờ `shell>` vô ích).
    # **KT-114**: khớp **tập lệnh** (`help` rồi các tên `[a-z]+` ngăn bằng `/`) thay vì chuỗi cứng
    # `help/echo/ps/wait/kill/ls/cat/write/exit` — shell sản phẩm thêm lệnh (9 → 13) thì script
    # **không** phải sửa. Không neo `^`: chuỗi nằm trong `.rodata` của ELF nên byte trước nó không
    # phải newline; neo `$` được vì literal kết thúc bằng `\n`.
    LC_ALL=C grep -a -q -m1 -E '# shell: up cmds=help(/[a-z]+)+$' "$IMAGE" 2>/dev/null ||
        die "ảnh '$IMAGE' không có shell ('# shell: up cmds=help/...') — không phải ảnh product-shell"
    # Ảnh phải có **bản vá** KT-103/KT-104; ảnh cũ sẽ đỏ vì lý do sai (thiếu chuỗi), nên từ chối sớm.
    LC_ALL=C grep -a -q -m1 '# shell: thieu duong dan' "$IMAGE" 2>/dev/null ||
        die "ảnh thiếu bản vá KT-104 ('# shell: thieu duong dan') — dùng ảnh đã ghim mới"
    LC_ALL=C grep -a -q -m1 '# shell: dong qua dai' "$IMAGE" 2>/dev/null ||
        die "ảnh thiếu bản vá KT-103 ('# shell: dong qua dai') — dùng ảnh đã ghim mới"
    # Các ca 5..7 cần dịch vụ FS của kernel.
    LC_ALL=C grep -a -q -m1 '# product fs: medium=' "$IMAGE" 2>/dev/null ||
        die "ảnh không có FS ('# product fs: medium=') — ca cat/write không chạy được"
    return 0
}

# ------------------------------------------------------------------ chạy thật
run_real() {
    local qemu sbi qv qmaj qrest qmin img_sha img_bytes build_line rc=0 i c reason
    local long_line g40

    preflight_image

    qemu="$(command -v qemu-system-riscv64 2>/dev/null || true)"
    [ -n "$qemu" ] || die "không có qemu-system-riscv64"
    qv="$("$qemu" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p')"
    [ -n "$qv" ] || die "không đọc được phiên bản QEMU"
    case "$qv" in *.*) ;; *) die "phiên bản QEMU '$qv' không có dạng major.minor" ;; esac
    qmaj="${qv%%.*}"
    qrest="${qv#*.}"
    qmin="${qrest%%.*}"
    case "$qmaj" in '' | *[!0-9]*) die "không đọc được major của QEMU '$qv'" ;; esac
    case "$qmin" in '' | *[!0-9]*) die "không đọc được minor của QEMU '$qv'" ;; esac
    if [ "$qmaj" -lt "$QEMU_MIN_MAJOR" ] ||
        { [ "$qmaj" -eq "$QEMU_MIN_MAJOR" ] && [ "$qmin" -lt "$QEMU_MIN_MINOR" ]; }; then
        die "QEMU $qv < sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR (D2-72 Q2) — không tự hạ"
    fi

    sbi="${OPENSBI:-}"
    if [ -z "$sbi" ]; then
        for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
            /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
            /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
            [ -f "$c" ] && {
                sbi="$c"
                break
            }
        done
    fi
    [ -f "${sbi:-/nonexistent}" ] || die "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"

    mkdir -p "$LOGDIR" || die "không tạo được $LOGDIR"
    LOG="$LOGDIR/qemu.log"
    CASE_LOG="$LOGDIR/case.log"
    FIFO="$LOGDIR/stdin.fifo"
    rm -f "$LOG" "$CASE_LOG" "$FIFO" 2>/dev/null
    : >"$LOG" || die "không ghi được $LOG"

    if command -v shasum >/dev/null 2>&1; then
        img_sha="$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
    else
        img_sha="$(sha256sum "$IMAGE" | awk '{print $1}')"
    fi
    img_bytes="$(wc -c <"$IMAGE" | tr -d ' ')"
    build_line="$(LC_ALL=C grep -a -o -m1 '# build: [a-z]*=[0-9a-f]* dirty=[01]' "$IMAGE" 2>/dev/null || true)"

    echo "# shell-edge KT-108 — $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    echo "# image: $IMAGE"
    echo "# image: sha256=$img_sha bytes=$img_bytes"
    [ -n "$build_line" ] && echo "# image: $build_line"
    echo "# qemu: $qv ($qemu); opensbi: $sbi"
    echo "# cấu hình: -machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -no-reboot (không icount)"
    echo "# log: $LOG"

    # KT-78/OD-K6-6: hạ tầng trước phép đo.
    qg_infra before "$LOGDIR/infra.log" || true

    # FIFO làm stdin QEMU. `exec 8<>` giữ **cả** đầu đọc lẫn ghi ⇒ `open` của QEMU không chặn và
    # FIFO không bao giờ EOF khi harness còn sống.
    mkfifo "$FIFO" || die "không tạo được FIFO $FIFO"
    exec 8<>"$FIFO" || die "không mở được FIFO $FIFO"
    # `qg_boot` chạy QEMU ở nền nên stdin mặc định là `/dev/null`; wrapper `/bin/bash -c` mở lại
    # FIFO làm stdin cho QEMU **trước** `exec` — guard vẫn là tiến trình cha trong group riêng.
    export SE_FIFO="$FIFO"
    qg_boot "$LOG" "$QEMU_TDS" /bin/bash -c 'exec "$@" <"$SE_FIFO"' bash "$qemu" \
        -machine virt,aia=none,aclint=on -bios "$sbi" -kernel "$IMAGE" \
        -nographic -cpu rv64 -m 128M -smp 1 -no-reboot &
    QGB_PID=$!

    # (0) chờ shell lên (prompt `shell>` do shell U-mode in).
    if ! wait_re '^shell>$' "$BOOT_POLLS"; then
        echo "# boot: FAIL (không thấy 'shell>' sau $BOOT_POLLS vòng poll)"
        tail -5 "$LOG" 2>/dev/null | sed 's/^/#   /'
        kill_own_qemu
        wait "$QGB_PID" 2>/dev/null || true
        echo "# shell-edge: 0/8 ca PASS"
        echo "# shell-edge.sh: FAIL (shell không lên)"
        return 1
    fi
    echo "# boot: shell> đã lên"

    # (1) `cat` không tham số.
    if run_case 1 "cat-khong-tham-so" check_cat_no_arg 'cat'; then
        echo "#   | $(grep -m1 -x -F '# shell: thieu duong dan' "$CASE_LOG")"
    else
        rc=1
    fi
    [ "$CASE_TIMEOUT" -eq 1 ] && {
        mark_remaining_fail 2 8
        rc=1
    }

    # (2) `write` không tham số.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        if run_case 2 "write-khong-tham-so" check_write_no_arg 'write'; then
            echo "#   | $(grep -m1 -x -F '# shell: thieu duong dan' "$CASE_LOG")"
        else
            rc=1
        fi
        [ "$CASE_TIMEOUT" -eq 1 ] && {
            mark_remaining_fail 3 8
            rc=1
        }
    fi

    # (3) `ps` không `state=Unknown`.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        if run_case 3 "ps-khong-unknown" check_ps_no_unknown 'ps'; then
            echo "#   | $(grep -c -E '^  pid=[0-9]+ state=' "$CASE_LOG") dòng pid, state=Unknown=$(grep -c -F 'state=Unknown' "$CASE_LOG" || true)"
        else
            rc=1
        fi
        [ "$CASE_TIMEOUT" -eq 1 ] && {
            mark_remaining_fail 4 8
            rc=1
        }
    fi

    # (4) dòng dài 200 ký tự ⇒ đúng một cảnh báo. Dùng `echo ` + 195 chữ A để dòng bị cắt (64 B)
    #     vẫn là lệnh `echo` hợp lệ — không sinh `# shell: ?` làm nhiễu ca 1/2.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        long_line="echo $(printf 'A%.0s' $(seq 1 $((LONG_LEN - 5))))"
        if run_case 4 "dong-dai-mot-canh-bao" check_long_line_once "$long_line"; then
            echo "#   | dòng ${LONG_LEN} ký tự -> $(grep -c -x -F '# shell: dong qua dai' "$CASE_LOG") dòng '# shell: dong qua dai'"
        else
            rc=1
        fi
        [ "$CASE_TIMEOUT" -eq 1 ] && {
            mark_remaining_fail 5 8
            rc=1
        }
    fi

    # (5) `cat /khongco` ⇒ mã lỗi 5.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        if run_case 5 "cat-khong-co" check_cat_missing 'cat /khongco'; then
            echo "#   | $(grep -m1 -x -F '# shell: fs loi code=5' "$CASE_LOG")"
        else
            rc=1
        fi
        [ "$CASE_TIMEOUT" -eq 1 ] && {
            mark_remaining_fail 6 8
            rc=1
        }
    fi

    # (6) `write /a xin chao` rồi `cat /a` ⇒ bytes=8, first=xin chao.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        if run_case 6 "write-cat-bytes" check_write_cat 'write /a xin chao' 'cat /a'; then
            echo "#   | $(grep -m1 -x -F '# shell: cat ok bytes=8 first=xin chao' "$CASE_LOG")"
        else
            rc=1
        fi
        [ "$CASE_TIMEOUT" -eq 1 ] && {
            mark_remaining_fail 7 8
            rc=1
        }
    fi

    # (7) ghi đè co lại: 40 B rồi 2 B ⇒ `cat` thấy 2 B.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        g40="$(printf 'B%.0s' $(seq 1 40))"
        if run_case 7 "write-de-co-lai" check_write_shrink "write /g $g40" 'write /g ab' 'cat /g'; then
            echo "#   | $(grep -m1 -x -F '# shell: cat ok bytes=2 first=ab' "$CASE_LOG")"
        else
            rc=1
        fi
        [ "$CASE_TIMEOUT" -eq 1 ] && {
            mark_remaining_fail 8 8
            rc=1
        }
    fi

    # (8) `wait 13` không treo shell.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        run_case_wait || rc=1
    fi

    # (10) **KT-130** (Q4 của Agent 0, 18/11/2026): năm lệnh `time`/`uptime`/`sync`/`stat`/`ls` —
    #      ca **ĐÚNG** trên ảnh sản phẩm. Đặt **trước** (8b) để `/README` còn nguyên cỡ gốc.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        case_begin
        # KT-130: gửi **cách nhau** một nhịp. Đo được: gửi năm dòng liền một hơi thì `uptime`
        # mất (FIFO 16 B của 16550 + shell in hai dòng cho `time` ⇒ byte bị rơi). Nhịp 0,15 s
        # đủ và không làm chậm đáng kể.
        for c in 'time' 'uptime' 'sync' 'stat /README' 'ls'; do
            send_line "$c" || true
            sleep 0.15
        done
        if case_end 10; then
            # **Q4 vòng ba**: hàm kiểm in **một dòng cho mỗi lệnh** (PASS/FAIL kèm lý do) rồi trả
            # 0/1; dòng dưới chỉ là phán quyết **của ca**. Không bọc `$(...)` nữa — bọc thì các
            # dòng từng lệnh bị nuốt vào biến và chỉ in ra khi đỏ.
            if check_new_cmds_ok "$CASE_LOG"; then
                echo "# new-cmds-ok: PASS ($(grep -m1 -E '^# shell: stat path=/README ' "$CASE_LOG"))"
            else
                echo "# new-cmds-ok: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)"
                rc=1
            fi
        else
            echo "# new-cmds-ok: FAIL (quá hạn — shell treo ở time/uptime/sync/stat/ls)"
            CASE_TIMEOUT=1
            rc=1
        fi
    fi

    # (11) **KT-130**: ca **BIÊN** của cùng bộ lệnh (tham số sai, đường không có, size quá lớn,
    #      sync lặp, đồng hồ đơn điệu). Vẫn trước (8b) để `/README` chưa bị truncate.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        case_begin
        for c in 'stat' 'stat /khongco' 'ls /khongco' 'truncate /README abc' \
            'truncate /README 4294967296' 'sync' 'sync' 'time' 'time'; do
            send_line "$c" || true
            sleep 0.15
        done
        if case_end 11; then
            if check_new_cmds_edge "$CASE_LOG"; then
                echo "# new-cmds-edge: PASS ($(grep -m1 -x -F '# shell: kich thuoc qua lon (toi da 4294967295)' "$CASE_LOG"))"
            else
                echo "# new-cmds-edge: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)"
                rc=1
            fi
        else
            echo "# new-cmds-edge: FAIL (quá hạn — shell treo ở ca biên)"
            CASE_TIMEOUT=1
            rc=1
        fi
    fi

    # (12) **KT-130** (vòng hai, Q4 của Agent 0): `uptime` + `jobs` — hai lệnh trước đây **chỉ** có ca
    #      đúng. Kiểm ba vế: đồng hồ `uptime` đơn điệu, `jobs` idempotent **từng ký tự**, và tham số
    #      thừa (`uptime 5`, `jobs 5`) **không** được bỏ qua im lặng — phải rơi xuống `# shell: ?`.
    #      Vẫn đặt trước (8b) để không dính `truncate /README`; hai lệnh này không đọc `/README`.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        case_begin
        # KT-130: gửi **cách nhau** một nhịp — gửi liền một hơi thì byte bị rơi (FIFO 16 B của 16550).
        for c in 'uptime' 'uptime' 'uptime 5' 'jobs' 'jobs' 'jobs 5'; do
            send_line "$c" || true
            sleep 0.15
        done
        if case_end 12; then
            if check_new_cmds_edge2 "$CASE_LOG"; then
                echo "# new-cmds-edge2: PASS ($(grep -m1 -E '^# shell: jobs ready=' "$CASE_LOG"))"
            else
                echo "# new-cmds-edge2: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)"
                rc=1
            fi
        else
            echo "# new-cmds-edge2: FAIL (quá hạn — shell treo ở uptime/jobs)"
            CASE_TIMEOUT=1
            rc=1
        fi
    fi

    # (13) **Q4 vòng ba** (Agent 0, 2026-11-18): năm ca biên còn thiếu — `time` có tham số,
    #      `write` có đường dẫn nhưng **thiếu nội dung**, `write` vào thư mục **không có**,
    #      `ls` lên **tệp**, `sync` có tham số. Đặt **trước** (8b) để `/README` còn nguyên (ca
    #      này chỉ `ls` nó, không sửa). `/a` được tạo **ngay trong ca** nên ca **tự đủ**, không
    #      phụ thuộc ca 6.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        case_begin
        # KT-130: gửi **cách nhau** một nhịp — gửi liền một hơi thì byte bị rơi (FIFO 16 B của 16550).
        for c in 'time 5' 'write /a xin chao' 'write /a' 'stat /a' \
            'write /khongco/x abc' 'ls /README' 'sync 5'; do
            send_line "$c" || true
            sleep 0.15
        done
        if case_end 13; then
            if check_new_cmds_edge3 "$CASE_LOG"; then
                echo "# new-cmds-edge3: PASS"
            else
                echo "# new-cmds-edge3: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)"
                rc=1
            fi
        else
            echo "# new-cmds-edge3: FAIL (quá hạn — shell treo ở ca biên mới)"
            CASE_TIMEOUT=1
            rc=1
        fi
    fi

    # (8b) **KT-126**: hai lệnh mới `truncate` + `jobs` — chạy **ngay trước** `exit` để không ảnh
    #      hưởng ca nào phía sau (truncate **đổi** `/README`). Không đánh số ca mới: dùng chung
    #      khung của ca 9 và chỉ thêm phép kiểm, nên `mark_remaining_fail` và bảng tổng kết không đổi.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        case_begin
        for c in 'ps' 'truncate /README 5' 'stat /README' 'jobs'; do
            send_line "$c" || true
        done
        if case_end 9; then
            if reason="$(check_truncate_jobs "$CASE_LOG")"; then
                echo "# truncate-jobs: PASS ($(grep -m1 -x -F '# shell: stat path=/README kind=1 size=5 ino=2' "$CASE_LOG"))"
            else
                echo "# truncate-jobs: FAIL ($reason)"
                rc=1
            fi
        else
            echo "# truncate-jobs: FAIL (quá hạn — shell treo ở truncate/jobs)"
            CASE_TIMEOUT=1
            rc=1
        fi
    fi

    # (9) thoát sạch: `exit` ⇒ shell in `# shell: exit code=0`, kernel in `# product halt:`.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        send_line 'exit' || true
        i=0
        while [ "$i" -lt 200 ] && kill -0 "$QGB_PID" 2>/dev/null; do
            sleep "$POLL_S"
            i=$((i + 1))
        done
        if kill -0 "$QGB_PID" 2>/dev/null; then
            echo "# exit: FAIL (QEMU không tự thoát sau 'exit' — kill group của chính mình)"
            kill_own_qemu
            rc=1
        fi
        wait "$QGB_PID" 2>/dev/null || true
        if grep -q -x -F '# shell: exit code=0' "$LOG" 2>/dev/null &&
            grep -q '^# product halt:' "$LOG" 2>/dev/null; then
            echo "# exit: PASS (# shell: exit code=0, # product halt:)"
        else
            echo "# exit: FAIL (thiếu '# shell: exit code=0' hoặc '# product halt:')"
            rc=1
        fi
    else
        kill_own_qemu
        wait "$QGB_PID" 2>/dev/null || true
    fi

    exec 8>&- 2>/dev/null || true
    rm -f "$FIFO" 2>/dev/null
    FIFO=""

    # KT-78/OD-K6-6: hạ tầng sau phép đo.
    qg_infra after "$LOGDIR/infra.log" || true

    if [ "$rc" -eq 0 ]; then
        echo "# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge + new-cmds-edge2 + new-cmds-edge3 PASS"
        echo "# shell-edge.sh: PASS"
        return 0
    fi
    echo "# shell-edge: có ca ĐỎ (xem log $LOG)"
    echo "# shell-edge.sh: FAIL"
    return 1
}

# =====================================================================================
# main
# =====================================================================================
while [ "$#" -gt 0 ]; do
    case "$1" in
        --selftest)
            MODE_SELFTEST=1
            shift
            ;;
        --image)
            [ "$#" -ge 2 ] || die "--image cần đường dẫn ảnh"
            IMAGE="$2"
            shift 2
            ;;
        --image=*)
            IMAGE="${1#--image=}"
            shift
            ;;
        --help | -h)
            usage
            exit 0
            ;;
        *)
            die "tham số lạ: '$1' (dùng --image <ảnh> | --selftest | --help)"
            ;;
    esac
done

if [ "$MODE_SELFTEST" -eq 0 ] && [ -z "$IMAGE" ]; then
    die "cần --image <ảnh.bin> hoặc --selftest"
fi

RC=0
if [ "$MODE_SELFTEST" -eq 1 ]; then
    run_selftest || RC=1
fi
if [ -n "$IMAGE" ]; then
    run_real || RC=1
fi
exit "$RC"
