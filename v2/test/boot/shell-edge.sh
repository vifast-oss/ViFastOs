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
# Ba phép kiểm **thêm** (không đánh số ca, dùng chung khung `case_begin`/`case_end`):
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
#   (10/11/12 là **KT-130**, Q4 của Agent 0: 9 lệnh thêm phải có ca kiểm trên ảnh sản phẩm.)
#
# **`--selftest` (bắt buộc theo luật repo: harness mới phải có ca âm)**: dựng **log giả** trong
# `mktemp -d` cho **mỗi** ca — một log **cố tình sai** (đúng đầu ra của lỗi cũ: `# shell: ?`,
# `state=Unknown`, 100 dòng `dong qua dai`, …) và một log **đúng** — rồi chạy **chính** hàm khẳng
# định của harness trên hai log đó. Log sai **phải ĐỎ**, log đúng **phải XANH**. Nhờ vậy selftest
# chứng minh harness **bắt được lỗi thật**, không phải luôn xanh. Không cần QEMU/ảnh cho selftest.
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
    sed -n '2,50p' "${BASH_SOURCE[0]}" 2>/dev/null | sed 's/^# \{0,1\}//'
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

# 3d. **KT-130** (Q4 của Agent 0, 18/11/2026) — năm lệnh `time`/`uptime`/`sync`/`stat`/`ls` phải
#     **thật sự** chạy trên ảnh sản phẩm và in đúng dạng. Trước đây chúng chỉ được parity phủ
#     (một dòng mỗi lệnh) — không có ca nào kiểm **nội dung** trên ảnh sản phẩm.
#     `time` in **một** dòng (`time ns=`), `uptime` in **một** dòng (`uptime ms=`) — đòi **đúng
#     một** mỗi loại để một lệnh không thể "mượn" dòng của lệnh kia.
check_new_cmds_ok() {
    local log="$1" n
    n="$(grep -c -E '^# shell: time ns=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: time ns=<n>', thấy ${n:-0}"
        return 1
    fi
    # `time` chỉ in **một** dòng (`time ns=`); `uptime` in **một** dòng (`uptime ms=`). Đòi **đúng
    # một** mỗi loại — nếu ai đó cho `time` in kèm dòng uptime thì ca này đỏ và phải bàn lại.
    n="$(grep -c -E '^# shell: uptime ms=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: uptime ms=<n>', thấy ${n:-0}"
        return 1
    fi
    n="$(grep -c -x -F '# shell: sync ok' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: sync ok', thấy ${n:-0}"
        return 1
    fi
    n="$(grep -c -E '^# shell: stat path=/README kind=[0-9]+ size=[0-9]+ ino=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: stat path=/README kind=… size=… ino=…', thấy ${n:-0}"
        return 1
    fi
    n="$(grep -c -E '^# shell: ls n=[0-9]+ path=/$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: ls n=<n> path=/', thấy ${n:-0}"
        return 1
    fi
    # `ls` phải liệt kê **ít nhất một** mục thật — bảng rỗng không được tính là đạt.
    n="$(grep -c -E '^  [fdl] [A-Za-z0-9_.-]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -lt 1 ]; then
        echo "ls không liệt kê mục nào (bảng rỗng)"
        return 1
    fi
    return 0
}

# 3e. **KT-130** — ca **biên** cho cùng năm lệnh, cộng `truncate`/`jobs` đã có ở 3c:
#     (a) `stat` không tham số ⇒ `thieu duong dan` (không rơi xuống `# shell: ?`);
#     (b) `stat`/`ls` trên đường **không có** ⇒ đúng **hai** dòng `fs loi code=5`;
#     (c) `truncate` size không phải số ⇒ `thieu kich thuoc`;
#     (d) `truncate` size ≥ 2^32 ⇒ `kich thuoc qua lon` (KT-126: từ chối sớm, không cắt âm thầm);
#     (e) `sync` hai lần ⇒ **hai** dòng `sync ok` (idempotent, không treo);
#     (f) `time` hai lần ⇒ **hai** dòng `time ns=`, và lần sau **≥** lần trước (đồng hồ đơn điệu);
#     (g) **không** dòng `# shell: ?` nào — mọi lệnh trên đều là lệnh shell biết.
check_new_cmds_edge() {
    local log="$1" n t1 t2
    n="$(grep -c -x -F '# shell: thieu duong dan' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: thieu duong dan' (stat khong tham so), thấy ${n:-0}"
        return 1
    fi
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
    n="$(grep -c -x -F '# shell: fs loi code=5' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: fs loi code=5' (stat + ls tren duong khong co), thấy ${n:-0}"
        return 1
    fi
    n="$(grep -c -x -F '# shell: sync ok' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: sync ok' (sync hai lan), thấy ${n:-0}"
        return 1
    fi
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
    n="$(grep -c -x -F '# shell: ?' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 0 ]; then
        echo "thấy ${n:-0} dòng '# shell: ?' (lệnh bị coi là lạ)"
        return 1
    fi
    return 0
}

# 3f. **KT-130** (Q4 của Agent 0, vòng hai) — `uptime` và `jobs` **thiếu ca biên**.
#     Vòng trước tôi khai "chín lệnh, mỗi lệnh một ca đúng + một ca biên", nhưng `new-cmds-edge`
#     **không** gửi `uptime` lẫn `jobs`: hai lệnh đó chỉ có ca **đúng** (`new-cmds-ok` cho `uptime`,
#     `truncate-jobs` cho `jobs`). Agent 0 bắt đúng chỗ hụt này. Ca này vá đúng hai lỗ đó, và kiểm
#     thêm một vế mà ca đúng **không** kiểm được: **tham số thừa không được bỏ qua im lặng**.
#     `fs_dispatch` so khớp **nguyên dòng** (`line == b"uptime"`, `line == b"jobs"`), không có nhánh
#     `starts_with(line, b"uptime ")` ⇒ `uptime 5`/`jobs 5` rơi xuống nhánh "lệnh lạ" và in
#     `# shell: ?`. Đòi **đúng hai** dòng `?` (một cho mỗi lệnh) để một lệnh không thể "mượn" dòng
#     của lệnh kia; và nếu sau này ai "tiện tay" cho hai lệnh này nhận tham số thì ca này đỏ — bỏ
#     qua tham số trong im lặng là dạng sai tệ nhất: người dùng tưởng lệnh đã hiểu ý mình.
#     `jobs` phải **idempotent từng ký tự**: hai lần gọi liền nhau đọc **cùng** một reply `PS_REQ`,
#     nên hai dòng lệch nhau nghĩa là bộ đếm trạng thái bị nhiễu (hoặc shell tự đổi trạng thái
#     giữa hai lần) — đúng thứ `truncate-jobs` (một lần gọi) không thể thấy.
check_new_cmds_edge2() {
    local log="$1" n u1 u2 j1 j2
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
    n="$(grep -c -x -F '# shell: ?' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: ?' (uptime 5 + jobs 5), thấy ${n:-0} (tham số bị bỏ qua im lặng?)"
        return 1
    fi
    return 0
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

    # ---- ca 3d (**KT-130**): năm lệnh `time`/`uptime`/`sync`/`stat`/`ls` — ca ĐÚNG.
    #      Log sai: thiếu `sync ok` và `ls` không liệt kê mục nào (bảng rỗng) ⇒ phải ĐỎ.
    d="$(mktemp -d "$root/c3d.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: time ns=1000\n'
        printf '# shell: uptime ms=0\n'
        printf '# shell: stat path=/README kind=1 size=79 ino=2\n'
        printf '# shell: ls n=0 path=/\n'
    } >"$wrong"
    {
        printf '# shell: time ns=1000\n'
        printf '# shell: uptime ms=0\n'
        printf '# shell: sync ok\n'
        printf '# shell: stat path=/README kind=1 size=79 ino=2\n'
        printf '# shell: ls n=1 path=/\n'
        printf '  f README\n'
    } >"$right"
    selftest_pair "3d new-cmds-ok" check_new_cmds_ok "$wrong" "$right"

    # ---- ca 3e (**KT-130**): ca BIÊN của cùng bộ lệnh.
    #      Log sai: `truncate` size quá lớn **không** bị từ chối (in dòng thành công) và đồng hồ
    #      **lùi** ⇒ phải ĐỎ.
    d="$(mktemp -d "$root/c3e.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: thieu duong dan\n'
        printf '# shell: thieu kich thuoc\n'
        printf '# shell: truncate path=/README size=4294967296\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: sync ok\n'
        printf '# shell: sync ok\n'
        printf '# shell: time ns=2000\n'
        printf '# shell: time ns=1000\n'
    } >"$wrong"
    {
        printf '# shell: thieu duong dan\n'
        printf '# shell: thieu kich thuoc\n'
        printf '# shell: kich thuoc qua lon (toi da 4294967295)\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: sync ok\n'
        printf '# shell: sync ok\n'
        printf '# shell: time ns=1000\n'
        printf '# shell: time ns=2000\n'
    } >"$right"
    selftest_pair "3e new-cmds-edge" check_new_cmds_edge "$wrong" "$right"

    # ---- ca 3f (**KT-130**, vòng hai): `uptime` + `jobs` — đúng hai lệnh Agent 0 chỉ ra là **thiếu
    #      ca biên**. Log sai: `uptime` **lùi** (đồng hồ không đơn điệu) ⇒ phải ĐỎ.
    d="$(mktemp -d "$root/c3f.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: uptime ms=2000\n'
        printf '# shell: uptime ms=1000\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: ?\n'
        printf '# shell: ?\n'
    } >"$wrong"
    {
        printf '# shell: uptime ms=1000\n'
        printf '# shell: uptime ms=2000\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: ?\n'
        printf '# shell: ?\n'
    } >"$right"
    selftest_pair "3f new-cmds-edge2" check_new_cmds_edge2 "$wrong" "$right"
    # Ca phụ 1: `jobs` **không** idempotent (hai dòng lệch nhau) ⇒ phải ĐỎ. Tách khỏi log sai chính
    # để chứng minh **nhánh idempotent** cũng bắt được, không chỉ nhánh đồng hồ.
    d="$(mktemp -d "$root/c3f1.XXXXXX")"
    wrong="$d/wrong.log"
    {
        printf '# shell: uptime ms=1000\n'
        printf '# shell: uptime ms=2000\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: jobs ready=2 running=1 blocked=0 other=0\n'
        printf '# shell: ?\n'
        printf '# shell: ?\n'
    } >"$wrong"
    selftest_red "3f new-cmds-edge2 (jobs lech)" check_new_cmds_edge2 "$wrong"
    # Ca phụ 2: `uptime 5`/`jobs 5` **không** bị coi là lệnh lạ (chỉ một dòng `?`) ⇒ phải ĐỎ: đó
    # đúng là dấu hiệu tham số bị bỏ qua im lặng.
    d="$(mktemp -d "$root/c3f2.XXXXXX")"
    wrong="$d/wrong.log"
    {
        printf '# shell: uptime ms=1000\n'
        printf '# shell: uptime ms=2000\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: jobs ready=3 running=0 blocked=0 other=0\n'
        printf '# shell: ?\n'
    } >"$wrong"
    selftest_red "3f new-cmds-edge2 (tham so bi bo qua)" check_new_cmds_edge2 "$wrong"

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
            if reason="$(check_new_cmds_ok "$CASE_LOG")"; then
                echo "# new-cmds-ok: PASS ($(grep -m1 -E '^# shell: stat path=/README ' "$CASE_LOG"))"
            else
                echo "# new-cmds-ok: FAIL ($reason)"
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
            if reason="$(check_new_cmds_edge "$CASE_LOG")"; then
                echo "# new-cmds-edge: PASS ($(grep -m1 -x -F '# shell: kich thuoc qua lon (toi da 4294967295)' "$CASE_LOG"))"
            else
                echo "# new-cmds-edge: FAIL ($reason)"
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
            if reason="$(check_new_cmds_edge2 "$CASE_LOG")"; then
                echo "# new-cmds-edge2: PASS ($(grep -m1 -E '^# shell: jobs ready=' "$CASE_LOG"))"
            else
                echo "# new-cmds-edge2: FAIL ($reason)"
                rc=1
            fi
        else
            echo "# new-cmds-edge2: FAIL (quá hạn — shell treo ở uptime/jobs)"
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
        echo "# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge + new-cmds-edge2 PASS"
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
