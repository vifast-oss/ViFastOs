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
#   14. k7-uu-tien            `write /a xin chao` `write / abc` `truncate / 5` `truncate /a`
#                             `write    abc` `stat /a` `cat /a`
#                                              ⇒ bốn lệnh sai **không** làm mất dữ liệu; `/a` vẫn 8 byte
#   15. k7-thu-muc            `cat /` `stat /` `time 5 6` `cat /README`
#                                              ⇒ `cat` thư mục ⇒ `fs loi code=8`; `stat` thư mục ⇒ `kind=2`;
#                                                `time` hai tham số ⇒ `# shell: ?`; `/README` còn nguyên
#   16. kt137-cdpwd           `pwd` `cd /khongco` `pwd` `cd /README` `pwd` `cd ..` `cd` `cd /` `pwd`
#                             `cd /README/..` `pwd` `cat README` `stat README` `write rel.txt xin chao`
#                             `cat rel.txt` `truncate rel.txt 2` `stat rel.txt` `ls` `ls .`
#                             `cat ../README` `cd /khongco/..` `pwd` `cd /docs` `pwd` `ls`
#                             `cat /docs/note.txt` `cat note.txt` `stat note.txt` `ls .`
#                             `cd /docs/khongco` `pwd` `cd ..` `pwd`
#                             `cd /docs/..` `pwd`
#                                              ⇒ `pwd` tám lần in `/` + **hai** lần in `/docs` (thư mục
#                                                hiện tại **không** đổi khi `cd` hỏng, **đổi** được vào
#                                                thư mục con); `cd` đường không có / vào tệp báo lỗi
#                                                đúng; `cd ..`/`cd`/`cd /`/`cd /README/..`/`cd /khongco/..`
#                                                ⇒ `/`; **đường tương đối** chạy cho `cat`/`stat`/`write`/
#                                                `truncate`/`ls` (ghép với thư mục hiện tại); **KT-158**:
#                                                `cd /docs` ⇒ `pwd` in `/docs`, `ls` trong `/docs` thấy
#                                                `note.txt`, `cat /docs/note.txt` đọc đúng nội dung,
#                                                `cd /docs/khongco` lỗi mà `pwd` **vẫn** `/docs`;
#                                                **KT-166** (review L6): `cat note.txt`/`stat note.txt`/
#                                                `ls .` chạy **trong** `/docs` — đường **tương đối** ở
#                                                thư mục con, nên đột biến "`resolve_path` bỏ qua `Cwd`"
#                                                **phải ĐỎ** (trước KT-166 nó vẫn xanh)
#   17. kt183-cd-sau          `cd /docs/sub` `pwd` `cat deep.txt` `cd ../..` `pwd`
#                             `cd /docs/sub/..` `pwd` `cd /docs/sub` `cd ../../..` `pwd`
#                             `cd /docs` `pwd` `cd /` `cd /docs/sub/` `pwd`
#                                              ⇒ **KT-183** (D2-164): FS sản phẩm **nhiều cấp**
#                                                (`/docs/sub` + `/docs/sub/deep.txt`); `pwd` in
#                                                `/docs/sub`; `cat deep.txt` (tương đối) đọc đúng;
#                                                `cd ../..` ⇒ `/`; `cd /docs/sub/..` ⇒ `/docs`
#                                                (chuẩn hoá `..`); `cd ../../..` từ `/docs/sub` ⇒ `/`
#                                                (**kẹp** gốc, không âm); đối chứng `cd /docs` ⇒
#                                                `/docs` (không hồi quy `kt137-cdpwd`);
#                                                **KT-205** (review KT-204, M2): `cd /docs/sub/`
#                                                (**có** `/` cuối) ⇒ `pwd` **vẫn** `/docs/sub`
#                                                (đột biến "in ra mà **không** đổi `cwd`" phải ĐỎ)
#   18. kt183-bien-path       `cd /docs/sub` `cd /docs/sub/<38×a>` (48 B)
#                             `cd /docs/sub/<39×a>` (49 B) `pwd`
#                                              ⇒ **KT-205** (review KT-204, M4): biên `MAX_PATH`
#                                                48 B — đường **đúng** 48 B ⇒ `cd loi code=11`
#                                                (tên thành phần 38 B > `MAX_NAME` 28 B, **không**
#                                                phải `duong qua dai`); 49 B ⇒ `duong qua dai`;
#                                                `pwd` **vẫn** `/docs/sub` (cd hỏng **không** đổi cwd)
#   (10/11/12 là **KT-130**; 13 là **Q4 vòng ba**; 14 là **K7 lượt bốn**; 15 là **K7 lượt năm §B3.4c**;
#    16 là **KT-137** — `cd`/`pwd` + thư mục hiện tại + đường dẫn tương đối; **KT-158** thêm thư mục
#    con `/docs` vào FS sản phẩm để vế "thư mục hiện tại **đổi được**" kiểm được thật; **KT-166**
#    thêm ba lệnh **tương đối trong** `/docs` để đột biến "`resolve_path` bỏ qua `Cwd`" phải ĐỎ;
#    **KT-183** thêm thư mục **sâu** `/docs/sub` + tệp `deep.txt` để kiểm `cd` nhiều cấp, `..` lùi
#    hai cấp, chuẩn hoá `..` và **kẹp** ở gốc; **KT-205** (review KT-204) đóng hai lỗ hổng ca kiểm:
#    M2 — `cd` có `/` cuối **không** đổi `cwd` (thêm `cd /docs/sub/` + `pwd` vào ca 17); M4 — biên
#    `MAX_PATH` 48 B off-by-one (thêm ca 18 với đường **đúng** 48 B và 49 B).)
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
    echo "shell-edge.sh: dùng: shell-edge.sh --image <ảnh.bin> [--selftest] | --selftest | --check-image <ảnh.bin> | --help"
}

# KT-145: nguồn sự thật của **danh tính ảnh** là `build-identity.sh` (nó tự nạp `build-rev.sh` để
# lấy `VBREV_ALGO`). Không chép lại công thức/định dạng ở đây. Hai tệp ấy chỉ định nghĩa hàm +
# hằng số khi được source (phần chạy được bọc trong `if [ "${BASH_SOURCE[0]}" = "$0" ]`).
# shellcheck source=/dev/null
. "$HERE/build-identity.sh"

# KT-78 (OD-K6-6): **mọi** lần mở QEMU đi qua `qemu_guard.sh` — process group riêng, timeout kill
# **cả group**, trap dọn sạch, watchdog dọn khi cha chết (kể cả SIGKILL). Không sửa guard.
. "$HERE/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG=""
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"

# ------------------------------------------------------------------ trạng thái toàn cục
IMAGE=""
MODE_SELFTEST=0
MODE_CHECK_IMAGE=0   # KT-145: chỉ kiểm danh tính ảnh rồi thoát (không mở QEMU)
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
#   - `write /a`: **K7 (Agent 0, phiếu lượt bốn câu 1)** — hành vi **đã đổi**. Trước: `split_space`
#                 cho path=`/a`, text=**rỗng** ⇒ `cmd_write` gọi `OP_TRUNC | 0` ⇒ cắt tệp về 0 byte
#                 rồi in `bytes=0 sync=ok` (cắt ngầm khi gõ thiếu = rủi ro mất dữ liệu). Nay:
#                 `fs_dispatch` có nhánh `text.is_empty()` ⇒ in `# shell: thieu noi dung` và tệp
#                 **KHÔNG** bị đổi. Muốn cắt rỗng thì dùng `truncate <path> 0`, tường minh.
#                 (shell_user.rs:422-436)
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
# `write /a` (đường dẫn, **KHÔNG** nội dung) ⇒ **LỖI** và tệp **KHÔNG** bị đổi.
#
# K7 (Agent 0, phiếu lượt bốn câu 1) — đổi từ "cắt tệp về rỗng" sang "báo lỗi". Ba vế của luật
# đóng lỗi, và cách ca này cài cả ba:
#
#   1. **Đỏ trên mã cũ**: mã cũ in `# shell: write /a bytes=0 sync=ok` và `stat` thấy `size=0`.
#      Ca này đòi `# shell: thieu noi dung` **và** `size=8` ⇒ mã cũ đỏ ở **cả hai** vế.
#   2. **Xanh trên mã mới**: `size=8` là cỡ **cũ** (do `write /a xin chao` đặt trước), nên nó
#      chứng minh tệp **không bị đụng** — không chỉ chứng minh "có in lỗi".
#   3. **Đột biến bỏ nhánh ⇒ đỏ**: bỏ nhánh `text.is_empty()` thì dòng lỗi biến mất và `size`
#      về 0 ⇒ cả hai vế đỏ lại.
ok_edge3_write_no_text() {
    local log="$1" n8 nerr ns
    n8="$(grep -c -x -F '# shell: write /a bytes=8 sync=ok' "$log" 2>/dev/null || true)"
    if [ "${n8:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: write /a bytes=8 sync=ok' (tiền đề), thấy ${n8:-0}"
        return 1
    fi
    nerr="$(grep -c -x -F '# shell: thieu noi dung' "$log" 2>/dev/null || true)"
    if [ "${nerr:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: thieu noi dung' (write /a khong noi dung phai BAO LOI), thấy ${nerr:-0}"
        return 1
    fi
    # Không được còn dấu vết của hành vi cũ (cắt ngầm).
    if grep -q -x -F '# shell: write /a bytes=0 sync=ok' "$log" 2>/dev/null; then
        echo "còn dòng '# shell: write /a bytes=0 sync=ok' — hành vi CŨ (cắt ngầm) vẫn chạy"
        return 1
    fi
    ns="$(grep -c -E '^# shell: stat path=/a kind=[0-9]+ size=8 ino=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${ns:-0}" -ne 1 ]; then
        echo "stat /a KHÔNG thấy size=8 sau 'write /a' (thấy ${ns:-0} dòng khớp) — tệp đã bị đổi, lẽ ra phải giữ nguyên"
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

# --- k7-uu-tien: bốn ca biên **nhóm ưu tiên** của K7 (Agent 0, phiếu lượt bốn) --------------
#
# Nhóm ưu tiên = **toàn vẹn dữ liệu**: một lệnh gõ sai **không** được làm mất/làm hỏng dữ liệu đã
# có. Bốn ca, mỗi ca **ghim hành vi thật** đọc từ mã (không đoán):
#   - `write / abc`  : `/` là thư mục ⇒ `Fs::write` trả `IsDir` ⇒ `err_code` = 8. Không ghi gì.
#   - `truncate / 5` : `/` là thư mục ⇒ `Fs::truncate` trả `IsDir` ⇒ `err_code` = 8. Không cắt gì.
#   - `truncate /a`  : thiếu kích thước ⇒ `# shell: thieu kich thuoc`; tệp **không** bị đổi.
#   - `write    abc` : đường dẫn rỗng (phần trước dấu cách đầu là rỗng) ⇒ `# shell: thieu duong dan`.
# Cộng một ca **toàn vẹn** ở cuối: sau cả bốn lệnh, `/a` vẫn `size=8` và `cat /a` vẫn đọc đúng
# `xin chao`. Bốn ca riêng lẻ chỉ nói "có in lỗi"; ca toàn vẹn mới nói "dữ liệu còn nguyên".

# `write / abc` ⇒ đúng một `# shell: fs loi code=8` (IsDir), **không** có dòng `write` thành công.
ok_k7_write_dir() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: fs loi code=8' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -lt 1 ]; then
        echo "mong ít nhất 1 dòng '# shell: fs loi code=8' (write/truncate lên thư mục), thấy ${n:-0}"
        return 1
    fi
    if grep -q -E '^# shell: write / bytes=' "$log" 2>/dev/null; then
        echo "có dòng '# shell: write / bytes=…' — ghi lên thư mục mà không báo lỗi"
        return 1
    fi
    return 0
}

# `truncate / 5` ⇒ cũng `code=8`; và **không** có dòng `truncate` thành công nào cho `/`.
ok_k7_trunc_dir() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: fs loi code=8' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -lt 2 ]; then
        echo "mong ít nhất 2 dòng '# shell: fs loi code=8' (write / + truncate /), thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `truncate /a` (thiếu kích thước) ⇒ đúng một `# shell: thieu kich thuoc`.
ok_k7_trunc_no_size() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: thieu kich thuoc' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: thieu kich thuoc' (truncate /a), thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `write    abc` (đường dẫn rỗng) ⇒ đúng một `# shell: thieu duong dan`.
ok_k7_write_space_path() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: thieu duong dan' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: thieu duong dan' (write voi duong dan rong), thấy ${n:-0}"
        return 1
    fi
    return 0
}

# **Toàn vẹn**: sau bốn lệnh sai, `/a` vẫn 8 byte và đọc lại đúng nội dung.
ok_k7_khong_mat_du_lieu() {
    local log="$1" ns nc
    ns="$(grep -c -E '^# shell: stat path=/a kind=[0-9]+ size=8 ino=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${ns:-0}" -ne 1 ]; then
        echo "stat /a KHÔNG thấy size=8 sau bốn lệnh sai (thấy ${ns:-0}) — dữ liệu đã bị đổi"
        return 1
    fi
    nc="$(grep -c -x -F '# shell: cat ok bytes=8 first=xin chao' "$log" 2>/dev/null || true)"
    if [ "${nc:-0}" -ne 1 ]; then
        echo "cat /a KHÔNG đọc lại đúng 'xin chao' (thấy ${nc:-0}) — nội dung đã bị đổi"
        return 1
    fi
    return 0
}

check_k7_uu_tien() {
    emit_verdicts "k7-uu-tien" "$1" \
        "write-thu-muc:ok_k7_write_dir" \
        "truncate-thu-muc:ok_k7_trunc_dir" \
        "truncate-thieu-co:ok_k7_trunc_no_size" \
        "write-duong-dan-rong:ok_k7_write_space_path" \
        "khong-mat-du-lieu:ok_k7_khong_mat_du_lieu"
}

# --- k7-thu-muc: ba ca biên **thư mục / tham số thừa** của K7 (Agent 0, phiếu lượt năm §B3.4c) --
#
# Agent 0 giao: "cat/stat lên thư mục, time hai tham số (ghim hành vi thật trước; hành vi làm
# hỏng/mất dữ liệu ⇒ hàng chờ Agent 0, không tự sửa)".
#
# Hành vi **thật** đo trên ảnh sản phẩm (không đoán từ mã):
#   - `cat /`      ⇒ `# shell: fs loi code=8` (IsDir). Không in byte nào của thư mục.
#   - `stat /`     ⇒ `# shell: stat path=/ kind=2 size=512 ino=1` (kind 2 = thư mục).
#   - `time 5 6`   ⇒ `# shell: ?` (lệnh lạ — `time` chỉ nhận **không** tham số).
# Cả ba **không** làm hỏng/mất dữ liệu ⇒ **ghim**, không phải hàng chờ Agent 0. Ca toàn vẹn cuối
# chứng minh `/README` vẫn đọc đúng sau ba lệnh.

# `cat /` ⇒ đúng một `# shell: fs loi code=8`, và **không** có dòng `cat ok` nào cho `/`.
ok_k7_cat_dir() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: fs loi code=8' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: fs loi code=8' (cat lên thư mục), thấy ${n:-0}"
        return 1
    fi
    # Thư mục `/` có `size=512`; nếu `cat /` in nội dung thì sẽ có dòng `cat ok bytes=512`.
    # Đòi **không** có dòng ấy — chặt hơn "không có cat ok nào" (ca này còn `cat /README`).
    if grep -q -E '^# shell: cat ok bytes=512 first=' "$log" 2>/dev/null; then
        echo "có dòng '# shell: cat ok bytes=512 …' — cat lên thư mục mà vẫn in nội dung"
        return 1
    fi
    return 0
}

# `stat /` ⇒ đúng một dòng `kind=2` (thư mục) cho `/`.
ok_k7_stat_dir() {
    local log="$1" n
    n="$(grep -c -E '^# shell: stat path=/ kind=2 size=[0-9]+ ino=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: stat path=/ kind=2 …' (stat lên thư mục), thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `time 5 6` (hai tham số) ⇒ đúng một `# shell: ?` — `time` chỉ nhận **không** tham số.
ok_k7_time_hai_tham_so() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: ?' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: ?' (time hai tham số), thấy ${n:-0}"
        return 1
    fi
    if grep -q -E '^# shell: time ns=[0-9]+$' "$log" 2>/dev/null; then
        echo "có dòng '# shell: time ns=…' — 'time 5 6' bị coi là hợp lệ"
        return 1
    fi
    return 0
}

# **Toàn vẹn**: sau ba lệnh trên, `/README` vẫn đọc đúng 79 byte đầu `ViFastOs`.
ok_k7_thu_muc_khong_mat_du_lieu() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: cat ok bytes=79 first=ViFastOs' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "cat /README KHÔNG đọc lại đúng 'ViFastOs' (thấy ${n:-0}) — dữ liệu đã bị đổi"
        return 1
    fi
    return 0
}

check_k7_thu_muc() {
    emit_verdicts "k7-thu-muc" "$1" \
        "cat-thu-muc:ok_k7_cat_dir" \
        "stat-thu-muc:ok_k7_stat_dir" \
        "time-hai-tham-so:ok_k7_time_hai_tham_so" \
        "khong-mat-du-lieu:ok_k7_thu_muc_khong_mat_du_lieu"
}

# --- kt137-cdpwd: `cd`/`pwd` + thư mục hiện tại + **đường dẫn tương đối** (KT-137) ------------
#
# Vì sao có ca này: KT-132 đã chứng minh v1 tại tag đóng băng **có** `cd`/`pwd` (log
# `v2/agent/JOURNAL/kt132-v1-cdpwd.log`: `cd /bin` rồi `pwd` ⇒ `/bin`), nên hai lệnh thuộc
# "tương đương v1" và phải có trong shell v2. Ca này ghim **hành vi thật** của bản KT-137 trên
# ảnh sản phẩm, **mỗi lệnh một dòng phán quyết** (`emit_verdicts`), và kiểm cả **đường tương đối**
# cho mọi lệnh có tham số đường dẫn.
#
# Vì sao `pwd` phải đếm **cả** số dòng `pwd` khác `/`: shell giữ thư mục hiện tại trong buffer;
# nếu `cd` hỏng mà vẫn ghi đè (lỗi thật đã từng có ở shell khác), `pwd` sau đó in `/khongco` —
# đếm riêng dòng `/` sẽ **không** thấy. Vế thứ hai bắt đúng lỗi ấy.

# `pwd` mười lần ⇒ **tám** dòng `# shell: pwd /` (gốc) + **hai** dòng `# shell: pwd /docs` (trong
# thư mục con) và **không** dòng `pwd` nào khác.
#
# Vế `/docs` là **KT-158**: nếu `pwd` in **hằng** `/` (đúng đột biến D2-138 nêu là trước đây
# **không phân biệt được**) thì hai dòng `/docs` biến mất ⇒ phép kiểm này ĐỎ. Trước KT-158 FS sản
# phẩm chỉ có gốc `/` nên vế này **không** kiểm được.
ok_kt137_pwd() {
    local log="$1" n d all
    n="$(grep -c -x -F '# shell: pwd /' "$log" 2>/dev/null || true)"
    d="$(grep -c -x -F '# shell: pwd /docs' "$log" 2>/dev/null || true)"
    all="$(grep -c -E '^# shell: pwd ' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 8 ]; then
        echo "mong đúng 8 dòng '# shell: pwd /', thấy ${n:-0}"
        return 1
    fi
    if [ "${d:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: pwd /docs' (cd vào thư mục con), thấy ${d:-0}"
        return 1
    fi
    if [ "${all:-0}" -ne 10 ]; then
        echo "có ${all:-0} dòng '# shell: pwd …' nhưng chỉ $(( ${n:-0} + ${d:-0} )) dòng in '/' hoặc '/docs' — thư mục hiện tại bị đổi"
        return 1
    fi
    return 0
}

# `cd /khongco` ⇒ **đúng một** `# shell: cd khong co /khongco` (vế "không đổi thư mục hiện tại" do
# `ok_kt137_pwd` chứng minh).
ok_kt137_cd_khong_co() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: cd khong co /khongco' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: cd khong co /khongco', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `cd /README` (tệp, không phải thư mục) ⇒ **đúng một** `# shell: cd khong phai thu muc /README`.
ok_kt137_cd_tep() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: cd khong phai thu muc /README' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: cd khong phai thu muc /README', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# Mười một lệnh `cd` in **đúng thứ tự** mười một dòng. Vì sao so **chuỗi** chứ không đếm tổng:
# bảy lệnh cùng in `# shell: cd /` (`cd ..`, `cd`, `cd /`, `cd /README/..`, `cd /khongco/..`,
# `cd ..` trong `/docs`, `cd /docs/..`) — đếm tổng không quy được lỗi cho lệnh nào; so thứ tự thì
# lệnh nào in sai cũng làm lệch chuỗi. **KT-158** thêm `cd /docs` (thư mục con **thật**) và
# `cd /docs/khongco` (lỗi **trong** thư mục con).
ok_kt137_cd_thu_tu() {
    local log="$1" got want
    got="$(grep -E '^# shell: cd ' "$log" 2>/dev/null || true)"
    want="$(printf '%s\n' \
        '# shell: cd khong co /khongco' \
        '# shell: cd khong phai thu muc /README' \
        '# shell: cd /' \
        '# shell: cd /' \
        '# shell: cd /' \
        '# shell: cd /' \
        '# shell: cd /' \
        '# shell: cd /docs' \
        '# shell: cd khong co /docs/khongco' \
        '# shell: cd /' \
        '# shell: cd /')"
    if [ "$got" != "$want" ]; then
        echo "chuỗi dòng 'cd' lệch: thấy [$(printf '%s' "$got" | tr '\n' '|')]"
        return 1
    fi
    return 0
}

# **KT-158**: `cd /docs` ⇒ **đúng một** `# shell: cd /docs` — chứng minh `cd` vào **thư mục con**
# thật. Trước KT-158 FS sản phẩm chỉ có gốc `/` nên D2-138 ghi "chưa kiểm được"; nay kiểm được.
ok_kt137_cd_docs() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: cd /docs' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: cd /docs', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# **KT-158** + **KT-166** + **KT-183**: `ls` (không tham số) và `ls .` trong `/docs` ⇒ **đúng hai**
# dòng `# shell: ls n=2 path=/docs`, **hai** dòng `  f note.txt` và **hai** dòng `  d sub` — chứng
# minh `ls` liệt kê **thư mục hiện tại** (không phải luôn `/`) **và** `ls .` phân giải **tương đối**
# qua `Cwd`. **KT-183** thêm `/docs/sub` nên `/docs` có **hai** mục (`note.txt` + `sub`); vế
# `  d sub` còn chứng minh thư mục **sâu** hiện ra trong bảng `ls` của thư mục cha. Đột biến
# "`resolve_path` bỏ qua `Cwd`" (reviewer L6, KT-161 §1.4) làm `ls .` thành `ls /` ⇒ chỉ còn **một**
# dòng `path=/docs` ⇒ phép kiểm này ĐỎ.
ok_kt137_ls_docs() {
    local log="$1" n m k
    n="$(grep -c -x -F '# shell: ls n=2 path=/docs' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: ls n=2 path=/docs' (ls + ls .), thấy ${n:-0}"
        return 1
    fi
    m="$(grep -c -x -F '  f note.txt' "$log" 2>/dev/null || true)"
    if [ "${m:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '  f note.txt' (ls + ls .), thấy ${m:-0}"
        return 1
    fi
    k="$(grep -c -x -F '  d sub' "$log" 2>/dev/null || true)"
    if [ "${k:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '  d sub' (ls + ls .), thấy ${k:-0}"
        return 1
    fi
    return 0
}

# **KT-158** + **KT-166**: `cat /docs/note.txt` (tuyệt đối) **và** `cat note.txt` (tương đối,
# trong `/docs`) ⇒ **hai** dòng nội dung `ghi chu cua docs` + **hai** dòng
# `# shell: cat ok bytes=16 first=ghi chu …`. Đột biến "`resolve_path` bỏ qua `Cwd`" làm
# `cat note.txt` thành `cat /note.txt` ⇒ `fs loi code=5` ⇒ chỉ còn **một** dòng mỗi loại.
ok_kt137_cat_docs() {
    local log="$1" n m
    n="$(grep -c -x -F 'ghi chu cua docs' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng nội dung 'ghi chu cua docs' (cat /docs/note.txt + cat note.txt), thấy ${n:-0}"
        return 1
    fi
    m="$(grep -c -E '^# shell: cat ok bytes=16 first=ghi chu' "$log" 2>/dev/null || true)"
    if [ "${m:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: cat ok bytes=16 first=ghi chu…' (tuyệt đối + tương đối), thấy ${m:-0}"
        return 1
    fi
    return 0
}

# **KT-166**: `stat note.txt` (tương đối, trong `/docs`) ⇒ **đúng một** dòng
# `# shell: stat path=/docs/note.txt kind=1 size=16 ino=<n>` — chứng minh `stat` **phân giải**
# đường tương đối qua `Cwd`. Đột biến "`resolve_path` bỏ qua `Cwd`" ⇒ `stat /note.txt` ⇒
# `fs loi code=5`, **không** có dòng `stat` nào ⇒ phép kiểm này ĐỎ.
ok_kt137_stat_docs() {
    local log="$1" n
    n="$(grep -c -E '^# shell: stat path=/docs/note\.txt kind=1 size=16 ino=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: stat path=/docs/note.txt kind=1 size=16 ino=<n>', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `cat README` và `cat ../README` ⇒ **hai** dòng `cat ok bytes=79 first=ViFastOs` (đường tương đối
# ghép với thư mục hiện tại `/`). Trước KT-137 shell gửi nguyên `README` xuống FS ⇒ `fs loi code=2`.
ok_kt137_cat_tuong_doi() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: cat ok bytes=79 first=ViFastOs' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: cat ok bytes=79 first=ViFastOs' (cat README + cat ../README), thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `stat README` ⇒ `# shell: stat path=/README kind=1 size=79 ino=2` — đường **tuyệt đối** sau khi
# ghép, chứng minh shell đã phân giải chứ không gửi `README` thô xuống FS.
ok_kt137_stat_tuong_doi() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: stat path=/README kind=1 size=79 ino=2' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: stat path=/README kind=1 size=79 ino=2', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `write rel.txt xin chao` ⇒ `# shell: write /rel.txt bytes=8 sync=ok` (tạo tệp ở thư mục hiện tại).
ok_kt137_write_tuong_doi() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: write /rel.txt bytes=8 sync=ok' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: write /rel.txt bytes=8 sync=ok', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `cat rel.txt` ⇒ `# shell: cat ok bytes=8 first=xin chao` (đọc lại đúng tệp vừa ghi bằng đường
# tương đối).
ok_kt137_cat_rel() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: cat ok bytes=8 first=xin chao' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: cat ok bytes=8 first=xin chao', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `truncate rel.txt 2` ⇒ `# shell: truncate path=/rel.txt size=2` (đường tương đối + cỡ đúng).
ok_kt137_truncate_tuong_doi() {
    local log="$1" n
    n="$(grep -c -x -F '# shell: truncate path=/rel.txt size=2' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: truncate path=/rel.txt size=2', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `stat rel.txt` ⇒ `# shell: stat path=/rel.txt kind=1 size=2 ino=<n>` (truncate **có** hiệu lực).
ok_kt137_stat_rel() {
    local log="$1" n
    n="$(grep -c -E '^# shell: stat path=/rel.txt kind=1 size=2 ino=[0-9]+$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: stat path=/rel.txt kind=1 size=2 ino=<n>', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `ls` (không tham số) và `ls .` ⇒ **hai** dòng `ls n=<n> path=/` và bảng có `rel.txt` — liệt kê
# **thư mục hiện tại** (trước KT-137 `ls` không tham số luôn liệt kê `/`; ở gốc hai hành vi trùng
# nhau nên vế này chỉ ghim dạng dòng, vế mạnh là các phép kiểm đường tương đối ở trên).
ok_kt137_ls_cwd() {
    local log="$1" n m
    n="$(grep -c -E '^# shell: ls n=[0-9]+ path=/$' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 2 ]; then
        echo "mong đúng 2 dòng '# shell: ls n=<n> path=/' (ls + ls .), thấy ${n:-0}"
        return 1
    fi
    m="$(grep -c -x -F '  f rel.txt' "$log" 2>/dev/null || true)"
    if [ "${m:-0}" -lt 1 ]; then
        echo "bảng 'ls' không có 'rel.txt' — không liệt kê thư mục hiện tại"
        return 1
    fi
    return 0
}

check_kt137_cdpwd() {
    emit_verdicts "kt137-cdpwd" "$1" \
        "pwd:ok_kt137_pwd" \
        "cd-khong-co:ok_kt137_cd_khong_co" \
        "cd-tep:ok_kt137_cd_tep" \
        "cd-thu-tu:ok_kt137_cd_thu_tu" \
        "cat-tuong-doi:ok_kt137_cat_tuong_doi" \
        "stat-tuong-doi:ok_kt137_stat_tuong_doi" \
        "write-tuong-doi:ok_kt137_write_tuong_doi" \
        "cat-rel:ok_kt137_cat_rel" \
        "truncate-tuong-doi:ok_kt137_truncate_tuong_doi" \
        "stat-rel:ok_kt137_stat_rel" \
        "ls-cwd:ok_kt137_ls_cwd" \
        "cd-docs:ok_kt137_cd_docs" \
        "ls-docs:ok_kt137_ls_docs" \
        "cat-docs:ok_kt137_cat_docs" \
        "stat-docs:ok_kt137_stat_docs"
}

# --- kt183-cd-sau: `cd` vào thư mục con **sâu** (FS sản phẩm nhiều cấp) — KT-183 (D2-164) -----
#
# Vì sao có ca này: KT-158 mới cho FS sản phẩm **một** cấp `/docs`; D2-164 yêu cầu kiểm `cd` vào
# thư mục con **sâu** (nhiều cấp). KT-183 seed thêm `/docs/sub` + `/docs/sub/deep.txt`, nên ca này
# ghim **hành vi thật** của `cd`/`pwd`/`cat` ở cấp sâu trên ảnh sản phẩm:
#   - `cd /docs/sub` ⇒ `pwd` in `/docs/sub` (thư mục hiện tại **đổi** được tới cấp 2);
#   - `cat deep.txt` (tương đối) ⇒ đọc đúng nội dung tệp trong thư mục sâu;
#   - `cd ../..` từ `/docs/sub` ⇒ `/` (lùi **hai** cấp);
#   - `cd /docs/sub/..` ⇒ `/docs` (chuẩn hoá `..` **từ vựng**, không cần `sub` tồn tại trên đĩa);
#   - `cd ../../..` từ `/docs/sub` ⇒ `/` (**kẹp** ở gốc, không âm);
#   - đối chứng `cd /docs` ⇒ `/docs` (không hồi quy ca `kt137-cdpwd`).
# Mỗi lệnh một dòng phán quyết (`emit_verdicts`); các phép kiểm **vị trí** (dòng `cd` thứ n + dòng
# `pwd` ngay sau) quy lỗi đúng lệnh, không chỉ đếm tổng — `cd /` xuất hiện **hai** lần và `cd /docs`
# **hai** lần nên đếm tổng không phân biệt được lệnh nào hỏng.

# `kt183_next_pwd <log> <dòng>` — số dòng của dòng `# shell: pwd …` **đầu tiên** sau dòng cho trước.
# Shell xử lý lệnh **tuần tự** nên dòng `pwd` ngay sau một dòng `cd` là kết quả của lệnh `pwd` gửi
# ngay sau `cd` đó.
kt183_next_pwd() {
    grep -n -E '^# shell: pwd ' "$1" 2>/dev/null | awk -F: -v l="$2" '$1 > l { print $1; exit }'
}

# `ok_kt183_cd_sub`: dòng `cd` **đầu** là `# shell: cd /docs/sub`; dòng `pwd` ngay sau là
# `# shell: pwd /docs/sub`. Đột biến "`pwd` in hằng `/`" ⇒ vế `pwd` mất ⇒ ĐỎ.
ok_kt183_cd_sub() {
    local log="$1" ln pn
    ln="$(nth_line "$log" '^# shell: cd ' 1)"
    if [ -z "$ln" ]; then
        echo "không thấy dòng 'cd' đầu tiên (cd /docs/sub)"
        return 1
    fi
    if ! sed -n "${ln}p" "$log" | grep -q -x -F '# shell: cd /docs/sub'; then
        echo "dòng 'cd' đầu là '$(sed -n "${ln}p" "$log")', mong '# shell: cd /docs/sub'"
        return 1
    fi
    pn="$(kt183_next_pwd "$log" "$ln")"
    if [ -z "$pn" ]; then
        echo "không thấy dòng 'pwd' sau 'cd /docs/sub'"
        return 1
    fi
    if ! sed -n "${pn}p" "$log" | grep -q -x -F '# shell: pwd /docs/sub'; then
        echo "sau 'cd /docs/sub' là '$(sed -n "${pn}p" "$log")', mong '# shell: pwd /docs/sub'"
        return 1
    fi
    return 0
}

# `ok_kt183_cat_deep`: `cat deep.txt` (tương đối, trong `/docs/sub`) ⇒ **đúng một** dòng nội dung
# `deep` + **đúng một** `# shell: cat ok bytes=4 first=deep`. Đột biến "`resolve_path` bỏ qua `Cwd`"
# ⇒ `cat /deep.txt` ⇒ `fs loi code=5` ⇒ cả hai vế mất ⇒ ĐỎ.
ok_kt183_cat_deep() {
    local log="$1" n m
    n="$(grep -c -x -F 'deep' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng nội dung 'deep' (cat deep.txt), thấy ${n:-0}"
        return 1
    fi
    m="$(grep -c -x -F '# shell: cat ok bytes=4 first=deep' "$log" 2>/dev/null || true)"
    if [ "${m:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: cat ok bytes=4 first=deep', thấy ${m:-0}"
        return 1
    fi
    return 0
}

# `ok_kt183_cd_hai_cap`: `cd ../..` từ `/docs/sub` ⇒ dòng `cd` thứ **hai** là `# shell: cd /`;
# dòng `pwd` ngay sau là `# shell: pwd /`. Đột biến "`..` không được xử lý" ⇒
# `cd khong co /docs/sub/../..` ⇒ ĐỎ.
ok_kt183_cd_hai_cap() {
    local log="$1" ln pn
    ln="$(nth_line "$log" '^# shell: cd ' 2)"
    if [ -z "$ln" ]; then
        echo "không thấy dòng 'cd' thứ 2 (cd ../..)"
        return 1
    fi
    if ! sed -n "${ln}p" "$log" | grep -q -x -F '# shell: cd /'; then
        echo "dòng 'cd' thứ 2 là '$(sed -n "${ln}p" "$log")', mong '# shell: cd /'"
        return 1
    fi
    pn="$(kt183_next_pwd "$log" "$ln")"
    if [ -z "$pn" ] || ! sed -n "${pn}p" "$log" | grep -q -x -F '# shell: pwd /'; then
        echo "sau 'cd ../..' không phải '# shell: pwd /'"
        return 1
    fi
    return 0
}

# `ok_kt183_cd_chuan_hoa`: `cd /docs/sub/..` ⇒ dòng `cd` thứ **ba** là `# shell: cd /docs`; dòng
# `pwd` ngay sau là `# shell: pwd /docs`. Đột biến "`..` không được xử lý" ⇒
# `cd khong co /docs/sub/..` ⇒ ĐỎ.
ok_kt183_cd_chuan_hoa() {
    local log="$1" ln pn
    ln="$(nth_line "$log" '^# shell: cd ' 3)"
    if [ -z "$ln" ]; then
        echo "không thấy dòng 'cd' thứ 3 (cd /docs/sub/..)"
        return 1
    fi
    if ! sed -n "${ln}p" "$log" | grep -q -x -F '# shell: cd /docs'; then
        echo "dòng 'cd' thứ 3 là '$(sed -n "${ln}p" "$log")', mong '# shell: cd /docs'"
        return 1
    fi
    pn="$(kt183_next_pwd "$log" "$ln")"
    if [ -z "$pn" ] || ! sed -n "${pn}p" "$log" | grep -q -x -F '# shell: pwd /docs'; then
        echo "sau 'cd /docs/sub/..' không phải '# shell: pwd /docs'"
        return 1
    fi
    return 0
}

# `ok_kt183_cd_kep_goc`: `cd ../../..` từ `/docs/sub` ⇒ dòng `cd` thứ **năm** là `# shell: cd /`
# (**kẹp** ở gốc, không âm) và **không** có dòng lỗi `cd` nào trong ca. Nếu `..` vượt gốc bị coi là
# lỗi, `cd ../../..` sẽ in `cd khong co …` ⇒ ĐỎ.
ok_kt183_cd_kep_goc() {
    local log="$1" ln n
    ln="$(nth_line "$log" '^# shell: cd ' 5)"
    if [ -z "$ln" ]; then
        echo "không thấy dòng 'cd' thứ 5 (cd ../../..)"
        return 1
    fi
    if ! sed -n "${ln}p" "$log" | grep -q -x -F '# shell: cd /'; then
        echo "dòng 'cd' thứ 5 là '$(sed -n "${ln}p" "$log")', mong '# shell: cd /' (kẹp ở gốc)"
        return 1
    fi
    n="$(grep -c -E '^# shell: cd (khong co|khong phai thu muc|loi code=)' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 0 ]; then
        echo "mong 0 dòng lỗi 'cd …' (mọi cd trong ca phải thành công), thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `ok_kt183_doi_chung`: dòng `cd /docs` **cuối** (đối chứng, sau khối sâu) phải được theo sau bởi
# `# shell: pwd /docs` — chứng minh `cd /docs` vẫn đúng như ca `kt137-cdpwd` (không hồi quy).
ok_kt183_doi_chung() {
    local log="$1" ln pn
    ln="$(last_line "$log" '^# shell: cd /docs$')"
    if [ -z "$ln" ]; then
        echo "không thấy dòng '# shell: cd /docs' (đối chứng)"
        return 1
    fi
    pn="$(kt183_next_pwd "$log" "$ln")"
    if [ -z "$pn" ]; then
        echo "không thấy dòng 'pwd' sau 'cd /docs' (đối chứng)"
        return 1
    fi
    if ! sed -n "${pn}p" "$log" | grep -q -x -F '# shell: pwd /docs'; then
        echo "sau 'cd /docs' là '$(sed -n "${pn}p" "$log")', mong '# shell: pwd /docs'"
        return 1
    fi
    return 0
}

# Tám lệnh `cd` in **đúng thứ tự** tám dòng. So **chuỗi** chứ không đếm tổng: `cd /` xuất hiện
# **hai** lần (`cd ../..`, `cd ../../..`) và `cd /docs` **hai** lần (`cd /docs/sub/..`, `cd /docs`),
# nên đếm tổng không quy được lỗi cho lệnh nào; so thứ tự thì lệnh nào in sai cũng làm lệch chuỗi.
# Dòng **cuối** là `cd /docs/sub/` (**KT-205**, M2) — `resolve_path` bỏ `/` cuối nên in
# `# shell: cd /docs/sub`; đột biến "in ra mà **không** đổi `cwd`" vẫn in đúng dòng này, nên phép
# kiểm `cd-slash-cuoi` (vị trí `pwd` ngay sau) mới là vế bắt nó.
ok_kt183_cd_thu_tu() {
    local log="$1" got want
    got="$(grep -E '^# shell: cd ' "$log" 2>/dev/null || true)"
    want="$(printf '%s\n' \
        '# shell: cd /docs/sub' \
        '# shell: cd /' \
        '# shell: cd /docs' \
        '# shell: cd /docs/sub' \
        '# shell: cd /' \
        '# shell: cd /docs' \
        '# shell: cd /' \
        '# shell: cd /docs/sub')"
    if [ "$got" != "$want" ]; then
        echo "chuỗi dòng 'cd' lệch: thấy [$(printf '%s' "$got" | tr '\n' '|')]"
        return 1
    fi
    return 0
}

# Sáu lệnh `pwd` in **đúng thứ tự** sáu dòng: `/docs/sub`, `/`, `/docs`, `/`, `/docs`, `/docs/sub`.
ok_kt183_pwd_thu_tu() {
    local log="$1" got want
    got="$(grep -E '^# shell: pwd ' "$log" 2>/dev/null || true)"
    want="$(printf '%s\n' \
        '# shell: pwd /docs/sub' \
        '# shell: pwd /' \
        '# shell: pwd /docs' \
        '# shell: pwd /' \
        '# shell: pwd /docs' \
        '# shell: pwd /docs/sub')"
    if [ "$got" != "$want" ]; then
        echo "chuỗi dòng 'pwd' lệch: thấy [$(printf '%s' "$got" | tr '\n' '|')]"
        return 1
    fi
    return 0
}

# `ok_kt183_cd_slash_cuoi` — **KT-205** (review KT-204, **M2**): `cd /docs/sub/` (**có** `/` cuối)
# phải **đổi** thư mục hiện tại. Dòng `cd` **cuối** của ca là kết quả của lệnh này (`resolve_path`
# bỏ `/` cuối ⇒ in `# shell: cd /docs/sub`), và dòng `pwd` ngay sau phải là `# shell: pwd /docs/sub`.
#
# Vì sao cần phép kiểm **riêng**: đột biến M2 của review KT-204 (`cmd_cd`: nếu `arg` kết thúc `/`
# và `len>1` thì in `# shell: cd <path>` rồi `return`, **không** `cwd.set`) in ra **đúng** dòng
# `# shell: cd /docs/sub` — nên `thu-tu-cd` vẫn xanh; chỉ `pwd` ngay sau mới lộ (`/` thay vì
# `/docs/sub`). Trước KT-205 đột biến này đi qua **toàn bộ** harness (ca 17 8/8 XANH, `kt137-cdpwd`
# cũng XANH) vì không ca nào có lệnh `cd` kết thúc bằng `/` (trừ `cd /`).
ok_kt183_cd_slash_cuoi() {
    local log="$1" ln pn
    ln="$(last_line "$log" '^# shell: cd ')"
    if [ -z "$ln" ]; then
        echo "không thấy dòng 'cd' nào (cd /docs/sub/ có / cuối)"
        return 1
    fi
    if ! sed -n "${ln}p" "$log" | grep -q -x -F '# shell: cd /docs/sub'; then
        echo "dòng 'cd' cuối là '$(sed -n "${ln}p" "$log")', mong '# shell: cd /docs/sub' (cd /docs/sub/ có / cuối)"
        return 1
    fi
    pn="$(kt183_next_pwd "$log" "$ln")"
    if [ -z "$pn" ]; then
        echo "không thấy dòng 'pwd' sau 'cd /docs/sub/'"
        return 1
    fi
    if ! sed -n "${pn}p" "$log" | grep -q -x -F '# shell: pwd /docs/sub'; then
        echo "sau 'cd /docs/sub/' là '$(sed -n "${pn}p" "$log")', mong '# shell: pwd /docs/sub'"
        return 1
    fi
    return 0
}

check_kt183_cd_sau() {
    emit_verdicts "kt183-cd-sau" "$1" \
        "cd-sub:ok_kt183_cd_sub" \
        "cat-deep:ok_kt183_cat_deep" \
        "cd-hai-cap:ok_kt183_cd_hai_cap" \
        "cd-chuan-hoa:ok_kt183_cd_chuan_hoa" \
        "cd-kep-goc:ok_kt183_cd_kep_goc" \
        "doi-chung:ok_kt183_doi_chung" \
        "thu-tu-cd:ok_kt183_cd_thu_tu" \
        "thu-tu-pwd:ok_kt183_pwd_thu_tu" \
        "cd-slash-cuoi:ok_kt183_cd_slash_cuoi"
}

# --- kt183-bien-path: biên `MAX_PATH` 48 B của `cd` — **KT-205** (review KT-204, M4) -----------
#
# Vì sao có ca này: `shell_user.rs` có `CWD_MAX = 48` (= `proc_fs::MAX_PATH`) và `resolve_path`
# từ chối khi `len + sep + comp.len() > CWD_MAX`; `vifast-fs` có `MAX_NAME = 28` và trả
# `FsError::NameTooLong` (mã **11**) khi một **thành phần** dài hơn 28 B. Hai biên **khác nhau**:
#   - đường **đúng** 48 B (`/docs/sub/` + 38×`a` = 10 + 38) ⇒ **qua** phép kiểm `MAX_PATH`, rồi FS
#     từ chối vì tên thành phần 38 B > 28 B ⇒ `# shell: cd loi code=11 <đường 48 B>`;
#   - đường 49 B (`/docs/sub/` + 39×`a`) ⇒ shell từ chối **sớm** ⇒ `# shell: duong qua dai`.
# Đột biến M4 của review KT-204 (`>` → `>=` ở kiểm độ dài) làm đường **đúng** 48 B in
# `duong qua dai` thay vì `cd loi code=11` — trước KT-205 **không** ca nào dùng đường gần 48 B nên
# đột biến đi qua toàn harness. Ca này ghim **cả hai** vế và ghim thêm "cd hỏng **không** đổi cwd".

# `ok_kt183_bien_48`: đúng **một** dòng `# shell: cd loi code=11 <đường 48 B>`; đường phải **đúng**
# 48 B (tự kiểm của harness, không phải "gần đúng"). Đột biến `>` → `>=` ⇒ không có dòng này ⇒ ĐỎ.
ok_kt183_bien_48() {
    local log="$1" a path want n
    a="$(printf 'a%.0s' $(seq 1 38))"
    path="/docs/sub/$a"
    if [ "${#path}" -ne 48 ]; then
        echo "lỗi harness: đường thử dài ${#path} B, mong **đúng** 48 B"
        return 1
    fi
    want="# shell: cd loi code=11 $path"
    n="$(grep -c -x -F "$want" "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '$want', thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `ok_kt183_bien_49`: đúng **một** dòng `# shell: duong qua dai` (shell chặn **sớm** đường 49 B).
# Đột biến `>` → `>=` làm đường 48 B **cũng** in `duong qua dai` ⇒ 2 dòng ⇒ ĐỎ; shell nhận đường
# 49 B (không chặn) ⇒ 0 dòng ⇒ ĐỎ.
ok_kt183_bien_49() {
    local log="$1" a path n
    a="$(printf 'a%.0s' $(seq 1 39))"
    path="/docs/sub/$a"
    if [ "${#path}" -ne 49 ]; then
        echo "lỗi harness: đường thử dài ${#path} B, mong **đúng** 49 B"
        return 1
    fi
    n="$(grep -c -x -F '# shell: duong qua dai' "$log" 2>/dev/null || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "mong đúng 1 dòng '# shell: duong qua dai' (đường 49 B), thấy ${n:-0}"
        return 1
    fi
    return 0
}

# `ok_kt183_pwd_sau_loi`: sau **hai** lệnh `cd` hỏng ở biên, `pwd` **vẫn** `/docs/sub` — cd hỏng
# **không** được đổi thư mục hiện tại (cùng bất biến `kt137-cdpwd` ghim cho `cd` hỏng thường).
ok_kt183_pwd_sau_loi() {
    local log="$1" ln pn
    ln="$(last_line "$log" '^# shell: pwd ')"
    if [ -z "$ln" ]; then
        echo "không thấy dòng 'pwd' nào (sau hai cd hỏng ở biên)"
        return 1
    fi
    if ! sed -n "${ln}p" "$log" | grep -q -x -F '# shell: pwd /docs/sub'; then
        echo "dòng 'pwd' cuối là '$(sed -n "${ln}p" "$log")', mong '# shell: pwd /docs/sub' (cd hỏng không đổi cwd)"
        return 1
    fi
    return 0
}

check_kt183_bien_path() {
    emit_verdicts "kt183-bien-path" "$1" \
        "bien-48:ok_kt183_bien_48" \
        "bien-49:ok_kt183_bien_49" \
        "pwd-sau-loi:ok_kt183_pwd_sau_loi"
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
SELFTEST_CASES=11
SELFTEST_CHECKS=0
SELFTEST_OK=0
SELFTEST_FAIL=0
# **KT-130** (vòng hai): `SELFTEST_CASES` đếm **ca có số** (1..8, **KT-137** thêm 16) trong chú thích đầu tệp, **không**
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
    local root d wrong right both i a38 a39
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
        printf '# shell: thieu noi dung\n'
        printf '# shell: stat path=/a kind=1 size=8 ino=3\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: fs loi code=7\n'
        printf '# shell: ?\n'
        printf '# shell: echo __END_13__\n'
    } >"$right"
    # Log SAI ở trên cố ý giữ **hành vi CŨ** của `write /a` (`bytes=0`, `size=0`) ⇒ nay phải đỏ
    # **ba** lệnh, không phải hai: `time 5` bị bỏ qua, `ls /README` liệt kê bừa, và `write /a`
    # cắt ngầm. Đây là vế "đỏ trên mã cũ" của luật đóng lỗi, cài ngay trong selftest.
    selftest_verdicts "3g new-cmds-edge3 (sai)" check_new_cmds_edge3 "$wrong" 1 2 3 \
        'new-cmds-edge3[time-tham-so]: FAIL' 'new-cmds-edge3[ls-tep]: FAIL' \
        'new-cmds-edge3[write-thieu-noi-dung]: FAIL'
    selftest_verdicts "3g new-cmds-edge3 (dung)" check_new_cmds_edge3 "$right" 0 5 0

    # ---- ca 14 (**K7 nhóm ưu tiên**): bốn ca biên toàn vẹn dữ liệu. Log SAI cố ý hỏng **ba**
    #      lệnh: `write /` báo thành công (ghi lên thư mục), `truncate /` không báo `code=8`, và
    #      `/a` bị đổi còn 0 byte ⇒ thông điệp đỏ phải nêu **cả ba**.
    d="$(mktemp -d "$root/c14.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: write /a bytes=8 sync=ok\n'
        printf '# shell: write / bytes=8 sync=ok\n'
        printf '# shell: fs loi code=8\n'
        printf '# shell: thieu kich thuoc\n'
        printf '# shell: thieu duong dan\n'
        printf '# shell: stat path=/a kind=1 size=0 ino=3\n'
        printf '# shell: cat ok bytes=0 first=\n'
        printf '# shell: echo __END_14__\n'
    } >"$wrong"
    {
        printf '# shell: write /a bytes=8 sync=ok\n'
        printf '# shell: fs loi code=8\n'
        printf '# shell: fs loi code=8\n'
        printf '# shell: thieu kich thuoc\n'
        printf '# shell: thieu duong dan\n'
        printf '# shell: stat path=/a kind=1 size=8 ino=3\n'
        printf '# shell: cat ok bytes=8 first=xin chao\n'
        printf '# shell: echo __END_14__\n'
    } >"$right"
    selftest_verdicts "14 k7-uu-tien (sai)" check_k7_uu_tien "$wrong" 1 2 3 \
        'k7-uu-tien[write-thu-muc]: FAIL' 'k7-uu-tien[truncate-thu-muc]: FAIL' \
        'k7-uu-tien[khong-mat-du-lieu]: FAIL'
    selftest_verdicts "14 k7-uu-tien (dung)" check_k7_uu_tien "$right" 0 5 0

    # ---- ca 15 (**K7 thư mục / tham số thừa**): `cat /`, `stat /`, `time 5 6`. Log SAI cố ý hỏng
    #      **ba** lệnh: `cat /` in nội dung thay vì `code=8`, `stat /` báo `kind=1` (tệp) thay vì
    #      `kind=2` (thư mục), `time 5 6` chạy như `time` hợp lệ ⇒ thông điệp đỏ phải nêu **cả ba**.
    d="$(mktemp -d "$root/c15.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: cat ok bytes=512 first=ViFastOs\n'
        printf '# shell: stat path=/ kind=1 size=512 ino=1\n'
        printf '# shell: time ns=12345\n'
        printf '# shell: cat ok bytes=79 first=ViFastOs\n'
        printf '# shell: echo __END_15__\n'
    } >"$wrong"
    {
        printf '# shell: fs loi code=8\n'
        printf '# shell: stat path=/ kind=2 size=512 ino=1\n'
        printf '# shell: ?\n'
        printf '# shell: cat ok bytes=79 first=ViFastOs\n'
        printf '# shell: echo __END_15__\n'
    } >"$right"
    selftest_verdicts "15 k7-thu-muc (sai)" check_k7_thu_muc "$wrong" 1 1 3 \
        'k7-thu-muc[cat-thu-muc]: FAIL' 'k7-thu-muc[stat-thu-muc]: FAIL' \
        'k7-thu-muc[time-hai-tham-so]: FAIL'
    selftest_verdicts "15 k7-thu-muc (dung)" check_k7_thu_muc "$right" 0 4 0

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

    # ---- ca 16 (**KT-137**, mở rộng **KT-158**): `cd`/`pwd` + thư mục hiện tại + đường dẫn
    #      tương đối + **thư mục con `/docs`**.
    #      Log ĐÚNG = đầu ra thật của ca 16 trên ảnh sản phẩm (mỗi lệnh một dòng, đúng thứ tự).
    #      Log SAI cố ý giữ **hành vi trước KT-137** ở ba chỗ: (a) `cd` hỏng vẫn đổi thư mục hiện
    #      tại (`pwd` in `/khongco`), (b) `cd /README` (tệp) **thành công**, (c) mọi lệnh có tham số
    #      đường dẫn gửi **nguyên** đường tương đối xuống FS ⇒ `fs loi code=2`/`code=5`. Thêm hành
    #      vi **trước KT-158** (FS sản phẩm **không** có `/docs`): `cd /docs` báo không có, `ls`
    #      trong `/docs` và `cat /docs/note.txt` đều `fs loi code=5`, và `pwd` **không** in `/docs`
    #      lần nào. **KT-166** thêm ba lệnh tương đối trong `/docs` (`cat note.txt`, `stat note.txt`,
    #      `ls .`) — log sai cho chúng `fs loi code=2` (đường tương đối gửi nguyên xuống FS). Vì vậy
    #      **mười ba** phép kiểm phải đỏ, hai phép kiểm còn lại (`cd-khong-co`, `ls-cwd`) vẫn xanh —
    #      chứng minh phán quyết **quy đúng lệnh**, không phải "đỏ cả ca".
    d="$(mktemp -d "$root/c16.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: pwd /\n'
        printf '# shell: cd khong co /khongco\n'
        printf '# shell: pwd /khongco\n'
        printf '# shell: cd /README\n'
        printf '# shell: pwd /README\n'
        printf '# shell: cd /\n'
        printf '# shell: cd /\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: fs loi code=2\n'
        printf '# shell: fs loi code=2\n'
        printf '# shell: fs loi code=2\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: fs loi code=2\n'
        printf '# shell: fs loi code=5\n'
        printf '  f README\n'
        printf '  f rel.txt\n'
        printf '# shell: ls n=2 path=/\n'
        printf '  f README\n'
        printf '  f rel.txt\n'
        printf '# shell: ls n=2 path=/\n'
        printf '# shell: fs loi code=2\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd khong co /docs\n'
        printf '# shell: pwd /\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: fs loi code=2\n'
        printf '# shell: fs loi code=2\n'
        printf '# shell: fs loi code=2\n'
        printf '# shell: cd khong co /docs/khongco\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: echo __END_16__\n'
    } >"$wrong"
    {
        printf '# shell: pwd /\n'
        printf '# shell: cd khong co /khongco\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd khong phai thu muc /README\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /\n'
        printf '# shell: cd /\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf 'ViFastOs v2 - FS tren anh san pham\n'
        printf 'lenh: ls | cat <path> | write <path> <text>\n'
        printf '\n'
        printf '# shell: cat ok bytes=79 first=ViFastOs\n'
        printf '# shell: stat path=/README kind=1 size=79 ino=2\n'
        printf '# shell: write /rel.txt bytes=8 sync=ok\n'
        printf 'xin chao\n'
        printf '# shell: cat ok bytes=8 first=xin chao\n'
        printf '# shell: truncate path=/rel.txt size=2\n'
        printf '# shell: stat path=/rel.txt kind=1 size=2 ino=7\n'
        printf '  f README\n'
        printf '  d docs\n'
        printf '  f a\n'
        printf '  f g\n'
        printf '  f rel.txt\n'
        printf '# shell: ls n=5 path=/\n'
        printf '  f README\n'
        printf '  d docs\n'
        printf '  f a\n'
        printf '  f g\n'
        printf '  f rel.txt\n'
        printf '# shell: ls n=5 path=/\n'
        printf 'ViFastOs v2 - FS tren anh san pham\n'
        printf 'lenh: ls | cat <path> | write <path> <text>\n'
        printf '\n'
        printf '# shell: cat ok bytes=79 first=ViFastOs\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /docs\n'
        printf '# shell: pwd /docs\n'
        printf '  f note.txt\n'
        printf '  d sub\n'
        printf '# shell: ls n=2 path=/docs\n'
        printf 'ghi chu cua docs\n'
        printf '# shell: cat ok bytes=16 first=ghi chu \n'
        printf 'ghi chu cua docs\n'
        printf '# shell: cat ok bytes=16 first=ghi chu \n'
        printf '# shell: stat path=/docs/note.txt kind=1 size=16 ino=4\n'
        printf '  f note.txt\n'
        printf '  d sub\n'
        printf '# shell: ls n=2 path=/docs\n'
        printf '# shell: cd khong co /docs/khongco\n'
        printf '# shell: pwd /docs\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: echo __END_16__\n'
    } >"$right"
    selftest_verdicts "16 kt137-cdpwd (sai)" check_kt137_cdpwd "$wrong" 1 2 13 \
        'kt137-cdpwd[pwd]: FAIL' 'kt137-cdpwd[cd-tep]: FAIL' \
        'kt137-cdpwd[cd-thu-tu]: FAIL' 'kt137-cdpwd[cat-tuong-doi]: FAIL' \
        'kt137-cdpwd[stat-tuong-doi]: FAIL' 'kt137-cdpwd[write-tuong-doi]: FAIL' \
        'kt137-cdpwd[cat-rel]: FAIL' 'kt137-cdpwd[truncate-tuong-doi]: FAIL' \
        'kt137-cdpwd[stat-rel]: FAIL' 'kt137-cdpwd[cd-docs]: FAIL' \
        'kt137-cdpwd[ls-docs]: FAIL' 'kt137-cdpwd[cat-docs]: FAIL' \
        'kt137-cdpwd[stat-docs]: FAIL'
    selftest_verdicts "16 kt137-cdpwd (dung)" check_kt137_cdpwd "$right" 0 15 0
    # Hai phép kiểm **vẫn xanh** trong log sai ở trên cần ca âm **riêng** (nếu không, chúng chưa
    # được chứng minh là bắt được lỗi): `cd-khong-co` với **hai** dòng (lệnh chạy hai lần) và
    # `ls-cwd` với **một** dòng `ls` (thiếu `ls .`).
    d="$(mktemp -d "$root/c16b.XXXXXX")"
    w2="$d/w2.log"
    {
        printf '# shell: cd khong co /khongco\n'
        printf '# shell: cd khong co /khongco\n'
    } >"$w2"
    selftest_pair "16 cd-khong-co" ok_kt137_cd_khong_co "$w2" "$right"
    d="$(mktemp -d "$root/c16c.XXXXXX")"
    w3="$d/w3.log"
    {
        printf '  f rel.txt\n'
        printf '# shell: ls n=2 path=/\n'
    } >"$w3"
    selftest_pair "16 ls-cwd" ok_kt137_ls_cwd "$w3" "$right"

    # ---- ca 17 (**KT-183**, D2-164): `cd` vào thư mục con **sâu** (`/docs/sub`).
    #      Log ĐÚNG = đầu ra thật của ca 17 trên ảnh sản phẩm (mỗi lệnh một dòng, đúng thứ tự).
    #      **Bốn** log SAI, mỗi log ứng với **một** kiểu hỏng thật:
    #        (a) **trước KT-183**: FS sản phẩm **không** có `/docs/sub` ⇒ `cd /docs/sub` báo không
    #            có, `pwd` vẫn `/`, `cat deep.txt` ⇒ `fs loi code=5`; `cd ../..` từ `/` và
    #            `cd /docs/sub/..` (chuẩn hoá từ vựng) **vẫn** ra `/` và `/docs` nên hai phép kiểm
    #            ấy xanh — chứng minh phán quyết **quy đúng lệnh**, không phải "đỏ cả ca";
    #        (b) **đột biến "`..` không được xử lý"**: `cd ../..`/`cd /docs/sub/..`/`cd ../../..`
    #            gửi nguyên đường có `..` xuống FS ⇒ `cd khong co …` ⇒ ba phép kiểm vị trí + hai
    #            phép kiểm chuỗi ĐỎ;
    #        (c) **đột biến "`pwd` in hằng `/`"**: `pwd` không bao giờ in `/docs/sub`/`/docs` ⇒
    #            `cd-sub`, `cd-chuan-hoa`, `doi-chung`, `thu-tu-pwd` ĐỎ;
    #        (d) **đột biến M2 của review KT-204** (**KT-205**): `cd /docs/sub/` (**có** `/` cuối)
    #            in `# shell: cd /docs/sub` nhưng **không** đổi `cwd` ⇒ `pwd` ngay sau in `/` ⇒
    #            `cd-slash-cuoi` + `thu-tu-pwd` ĐỎ (bảy phép kiểm còn lại **xanh** — chứng minh
    #            phép kiểm mới bắt đúng vế này, không phải "đỏ cả ca").
    d="$(mktemp -d "$root/c17.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: cd khong co /docs/sub\n'
        printf '# shell: pwd /\n'
        printf '# shell: fs loi code=5\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /docs\n'
        printf '# shell: pwd /docs\n'
        printf '# shell: cd khong co /docs/sub\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /docs\n'
        printf '# shell: pwd /docs\n'
        printf '# shell: cd /\n'
        printf '# shell: cd khong co /docs/sub\n'
        printf '# shell: pwd /\n'
        printf '# shell: echo __END_17__\n'
    } >"$wrong"
    selftest_verdicts "17 kt183-cd-sau (truoc KT-183)" check_kt183_cd_sau "$wrong" 1 3 6 \
        'kt183-cd-sau[cd-sub]: FAIL' 'kt183-cd-sau[cat-deep]: FAIL' \
        'kt183-cd-sau[cd-kep-goc]: FAIL' 'kt183-cd-sau[thu-tu-cd]: FAIL' \
        'kt183-cd-sau[thu-tu-pwd]: FAIL' 'kt183-cd-sau[cd-slash-cuoi]: FAIL'
    d="$(mktemp -d "$root/c17b.XXXXXX")"
    wrong="$d/wrong.log"
    {
        printf '# shell: cd /docs/sub\n'
        printf '# shell: pwd /docs/sub\n'
        printf 'deep\n'
        printf '# shell: cat ok bytes=4 first=deep\n'
        printf '# shell: cd khong co /docs/sub/../..\n'
        printf '# shell: pwd /docs/sub\n'
        printf '# shell: cd khong co /docs/sub/..\n'
        printf '# shell: pwd /docs/sub\n'
        printf '# shell: cd /docs/sub\n'
        printf '# shell: cd khong co /docs/sub/../../..\n'
        printf '# shell: pwd /docs/sub\n'
        printf '# shell: cd /docs\n'
        printf '# shell: pwd /docs\n'
        printf '# shell: cd /\n'
        printf '# shell: cd /docs/sub\n'
        printf '# shell: pwd /docs/sub\n'
        printf '# shell: echo __END_17__\n'
    } >"$wrong"
    selftest_verdicts "17 kt183-cd-sau (dot bien .. khong xu ly)" check_kt183_cd_sau "$wrong" 1 4 5 \
        'kt183-cd-sau[cd-hai-cap]: FAIL' 'kt183-cd-sau[cd-chuan-hoa]: FAIL' \
        'kt183-cd-sau[cd-kep-goc]: FAIL' 'kt183-cd-sau[thu-tu-cd]: FAIL' \
        'kt183-cd-sau[thu-tu-pwd]: FAIL'
    d="$(mktemp -d "$root/c17c.XXXXXX")"
    wrong="$d/wrong.log"
    {
        printf '# shell: cd /docs/sub\n'
        printf '# shell: pwd /\n'
        printf 'deep\n'
        printf '# shell: cat ok bytes=4 first=deep\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /docs\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /docs/sub\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /docs\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /\n'
        printf '# shell: cd /docs/sub\n'
        printf '# shell: pwd /\n'
        printf '# shell: echo __END_17__\n'
    } >"$wrong"
    selftest_verdicts "17 kt183-cd-sau (dot bien pwd hang /)" check_kt183_cd_sau "$wrong" 1 4 5 \
        'kt183-cd-sau[cd-sub]: FAIL' 'kt183-cd-sau[cd-chuan-hoa]: FAIL' \
        'kt183-cd-sau[doi-chung]: FAIL' 'kt183-cd-sau[thu-tu-pwd]: FAIL' \
        'kt183-cd-sau[cd-slash-cuoi]: FAIL'
    # (d) **KT-205** — đột biến M2 của review KT-204: `cd /docs/sub/` in ra **đúng** dòng
    # `# shell: cd /docs/sub` nhưng `cwd` **không** đổi ⇒ chỉ `pwd` ngay sau lộ ra.
    d="$(mktemp -d "$root/c17d.XXXXXX")"
    wrong="$d/wrong.log"
    {
        printf '# shell: cd /docs/sub\n'
        printf '# shell: pwd /docs/sub\n'
        printf 'deep\n'
        printf '# shell: cat ok bytes=4 first=deep\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /docs\n'
        printf '# shell: pwd /docs\n'
        printf '# shell: cd /docs/sub\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /docs\n'
        printf '# shell: pwd /docs\n'
        printf '# shell: cd /\n'
        printf '# shell: cd /docs/sub\n'
        printf '# shell: pwd /\n'
        printf '# shell: echo __END_17__\n'
    } >"$wrong"
    selftest_verdicts "17 kt183-cd-sau (dot bien M2: / cuoi khong doi cwd)" check_kt183_cd_sau "$wrong" 1 7 2 \
        'kt183-cd-sau[thu-tu-pwd]: FAIL' 'kt183-cd-sau[cd-slash-cuoi]: FAIL'
    {
        printf '# shell: cd /docs/sub\n'
        printf '# shell: pwd /docs/sub\n'
        printf 'deep\n'
        printf '# shell: cat ok bytes=4 first=deep\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /docs\n'
        printf '# shell: pwd /docs\n'
        printf '# shell: cd /docs/sub\n'
        printf '# shell: cd /\n'
        printf '# shell: pwd /\n'
        printf '# shell: cd /docs\n'
        printf '# shell: pwd /docs\n'
        printf '# shell: cd /\n'
        printf '# shell: cd /docs/sub\n'
        printf '# shell: pwd /docs/sub\n'
        printf '# shell: echo __END_17__\n'
    } >"$right"
    selftest_verdicts "17 kt183-cd-sau (dung)" check_kt183_cd_sau "$right" 0 9 0

    # ---- ca 18 (**KT-205**, review KT-204 **M4**): biên `MAX_PATH` 48 B của `cd`.
    #      Log ĐÚNG = đầu ra thật của ca 18 trên ảnh sản phẩm: đường **đúng** 48 B ⇒
    #      `cd loi code=11` (tên thành phần 38 B > `MAX_NAME` 28 B), đường 49 B ⇒ `duong qua dai`,
    #      `pwd` **vẫn** `/docs/sub`. Ba log SAI:
    #        (a) **đột biến M4** (`>` → `>=`): đường 48 B **cũng** in `duong qua dai` ⇒ `bien-48`
    #            + `bien-49` ĐỎ (2 dòng `duong qua dai`), `pwd-sau-loi` xanh;
    #        (b) shell **nhận** đường 49 B (không chặn sớm) ⇒ 0 dòng `duong qua dai` ⇒ `bien-49` ĐỎ;
    #        (c) `cd` hỏng **đổi** `cwd` ⇒ `pwd` in `/` ⇒ `pwd-sau-loi` ĐỎ.
    a38="$(printf 'a%.0s' $(seq 1 38))"
    a39="$(printf 'a%.0s' $(seq 1 39))"
    d="$(mktemp -d "$root/c18.XXXXXX")"
    wrong="$d/wrong.log"
    right="$d/right.log"
    {
        printf '# shell: cd /docs/sub\n'
        printf '# shell: duong qua dai\n'
        printf '# shell: duong qua dai\n'
        printf '# shell: pwd /docs/sub\n'
        printf '# shell: echo __END_18__\n'
    } >"$wrong"
    selftest_verdicts "18 kt183-bien-path (dot bien M4: > thanh >=)" check_kt183_bien_path "$wrong" 1 1 2 \
        'kt183-bien-path[bien-48]: FAIL' 'kt183-bien-path[bien-49]: FAIL'
    d="$(mktemp -d "$root/c18b.XXXXXX")"
    wrong="$d/wrong.log"
    {
        printf '# shell: cd /docs/sub\n'
        printf '# shell: cd loi code=11 /docs/sub/%s\n' "$a38"
        printf '# shell: cd loi code=11 /docs/sub/%s\n' "$a39"
        printf '# shell: pwd /docs/sub\n'
        printf '# shell: echo __END_18__\n'
    } >"$wrong"
    selftest_verdicts "18 kt183-bien-path (49 B khong bi chan som)" check_kt183_bien_path "$wrong" 1 2 1 \
        'kt183-bien-path[bien-49]: FAIL'
    d="$(mktemp -d "$root/c18c.XXXXXX")"
    wrong="$d/wrong.log"
    {
        printf '# shell: cd /docs/sub\n'
        printf '# shell: cd loi code=11 /docs/sub/%s\n' "$a38"
        printf '# shell: duong qua dai\n'
        printf '# shell: pwd /\n'
        printf '# shell: echo __END_18__\n'
    } >"$wrong"
    selftest_verdicts "18 kt183-bien-path (cd hong doi cwd)" check_kt183_bien_path "$wrong" 1 2 1 \
        'kt183-bien-path[pwd-sau-loi]: FAIL'
    {
        printf '# shell: cd /docs/sub\n'
        printf '# shell: cd loi code=11 /docs/sub/%s\n' "$a38"
        printf '# shell: duong qua dai\n'
        printf '# shell: pwd /docs/sub\n'
        printf '# shell: echo __END_18__\n'
    } >"$right"
    selftest_verdicts "18 kt183-bien-path (dung)" check_kt183_bien_path "$right" 0 3 0

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
    # KT-145 (Agent 0 phiếu lượt 5 §A4): **đường đo** chỉ nhận ảnh tự khai ĐÚNG định nghĩa dấu vân
    # tay hiện hành. Ảnh dạng cũ (thiếu `algo=`) hoặc `algo=` khác ⇒ TỪ CHỐI, không đo. Chỉ
    # `v2/tools/status.sh` và `v2/tools/pin-artifact.sh` được nhận dạng cũ (hiển thị/ghim ảnh lịch sử).
    if ! vbrev_image_identity "$IMAGE"; then
        echo "REV-GATE: REFUSE reason=$VBREV_IMG_REASON image=$IMAGE algo=${VBREV_IMG_ALGO:-<none>} current_algo=$VBREV_ALGO" >&2
        die "ảnh '$IMAGE' KHÔNG cùng định nghĩa dấu vân tay ($VBREV_IMG_REASON) — đường đo từ chối (KT-145)"
    fi
    # Ảnh phải có shell (nếu không, mọi ca sẽ chờ `shell>` vô ích).
    # **KT-114**: khớp **tập lệnh** (`help` rồi các tên `[a-z]+` ngăn bằng `/`) thay vì chuỗi cứng
    # `help/echo/ps/wait/kill/ls/cat/write/exit` — shell sản phẩm thêm lệnh (9 → 13 → 17, KT-137)
    # thì script **không** phải sửa. Không neo `^`: chuỗi nằm trong `.rodata` của ELF nên byte trước nó không
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
    local long_line g40 a38 a39

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
    # KT-143: dòng danh tính nay có `algo=` (tên định nghĩa dấu vân tay) đứng trước `inputs=`.
    # **KT-145** (§A4): chỉ nhận **dạng mới**. `preflight_image` đã TỪ CHỐI ảnh dạng cũ / `algo=`
    # khác hiện hành trước khi tới đây, nên nhánh "nhận dạng cũ để in dòng danh tính" không còn
    # đường dùng — giữ nó chỉ làm phép kiểm yếu đi mà không ai thấy.
    build_line="$(LC_ALL=C grep -a -o -m1 -e '# build: algo=[0-9A-Za-z._-]* inputs=[0-9a-f]* dirty=[01]' "$IMAGE" 2>/dev/null || true)"

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

    # (14) **K7 nhóm ưu tiên** (Agent 0, phiếu lượt bốn): bốn ca biên **toàn vẹn dữ liệu** —
    #      `write` lên thư mục, `truncate` lên thư mục, `truncate` thiếu cỡ, `write` với đường dẫn
    #      rỗng. `/a` được tạo **ngay trong ca** (8 byte) rồi bốn lệnh sai lần lượt chạy; cuối cùng
    #      `stat` + `cat` chứng minh dữ liệu **còn nguyên**. Đặt **trước** (8b) để `/README` còn
    #      nguyên (ca này không chạm `/README`).
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        case_begin
        for c in 'write /a xin chao' 'write / abc' 'truncate / 5' 'truncate /a' \
            'write    abc' 'stat /a' 'cat /a'; do
            send_line "$c" || true
            sleep 0.15
        done
        if case_end 14; then
            if check_k7_uu_tien "$CASE_LOG"; then
                echo "# k7-uu-tien: PASS"
            else
                echo "# k7-uu-tien: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)"
                rc=1
            fi
        else
            echo "# k7-uu-tien: FAIL (quá hạn — shell treo ở ca K7)"
            CASE_TIMEOUT=1
            rc=1
        fi
    fi

    # (15) **K7 thư mục / tham số thừa** (Agent 0, phiếu lượt năm §B3.4c): `cat` lên thư mục,
    #      `stat` lên thư mục, `time` **hai** tham số. Cả ba **ghim hành vi thật** đã đo trên ảnh
    #      sản phẩm; không lệnh nào làm hỏng/mất dữ liệu nên **không** phải hàng chờ Agent 0.
    #      Đặt **trước** (8b) để `/README` còn nguyên (ca này chỉ đọc nó).
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        case_begin
        for c in 'cat /' 'stat /' 'time 5 6' 'cat /README'; do
            send_line "$c" || true
            sleep 0.15
        done
        if case_end 15; then
            if check_k7_thu_muc "$CASE_LOG"; then
                echo "# k7-thu-muc: PASS"
            else
                echo "# k7-thu-muc: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)"
                rc=1
            fi
        else
            echo "# k7-thu-muc: FAIL (quá hạn — shell treo ở ca thư mục)"
            CASE_TIMEOUT=1
            rc=1
        fi
    fi

    # (16) **KT-137** (`cd`/`pwd` + thư mục hiện tại + **đường dẫn tương đối**) + **KT-158** (thư mục
    #      con `/docs`) + **KT-166** (đường tương đối **trong** thư mục con): mười lăm phép kiểm,
    #      mỗi lệnh một dòng phán quyết. Đặt **trước** (8b) để `/README` còn 79 byte (8b truncate nó
    #      về 5). Ca này **tạo** `/rel.txt` ở gốc — không ca nào sau đọc **số mục** của gốc, và
    #      `stat /README … ino=2` của (8b) không đổi (README được seed **trước** mọi tệp khác).
    #      Khối `/docs` ở **cuối** ca: `cd /docs` ⇒ `pwd` in `/docs`, `ls` trong `/docs` thấy
    #      `note.txt`, `cat /docs/note.txt` đọc đúng nội dung, rồi **KT-166** chạy `cat note.txt`/
    #      `stat note.txt`/`ls .` — **đường tương đối trong thư mục con** (đột biến "`resolve_path`
    #      bỏ qua `Cwd`" phải ĐỎ), `cd /docs/khongco` lỗi mà `pwd` **vẫn** `/docs`, rồi
    #      `cd ..`/`cd /docs/..` về `/`.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        case_begin
        for c in 'pwd' 'cd /khongco' 'pwd' 'cd /README' 'pwd' 'cd ..' 'cd' 'cd /' 'pwd' \
            'cd /README/..' 'pwd' 'cat README' 'stat README' 'write rel.txt xin chao' \
            'cat rel.txt' 'truncate rel.txt 2' 'stat rel.txt' 'ls' 'ls .' 'cat ../README' \
            'cd /khongco/..' 'pwd' \
            'cd /docs' 'pwd' 'ls' 'cat /docs/note.txt' 'cat note.txt' 'stat note.txt' 'ls .' \
            'cd /docs/khongco' 'pwd' 'cd ..' 'pwd' 'cd /docs/..' 'pwd'; do
            send_line "$c" || true
            sleep 0.15
        done
        if case_end 16; then
            if check_kt137_cdpwd "$CASE_LOG"; then
                echo "# kt137-cdpwd: PASS ($(grep -m1 -x -F '# shell: pwd /' "$CASE_LOG"))"
            else
                echo "# kt137-cdpwd: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)"
                rc=1
            fi
        else
            echo "# kt137-cdpwd: FAIL (quá hạn — shell treo ở ca cd/pwd)"
            CASE_TIMEOUT=1
            rc=1
        fi
    fi

    # (17) **KT-183** (D2-164): `cd` vào thư mục con **sâu** — FS sản phẩm nay có **nhiều cấp**
    #      (`/docs/sub` + `/docs/sub/deep.txt`). Mười lăm lệnh, **chín** phép kiểm (mỗi lệnh một
    #      dòng phán quyết): `cd /docs/sub` ⇒ `pwd` in `/docs/sub`; `cat deep.txt` (tương đối) đọc
    #      đúng; `cd ../..` ⇒ `/`; `cd /docs/sub/..` ⇒ `/docs` (chuẩn hoá `..`); `cd ../../..` từ
    #      `/docs/sub` ⇒ `/` (**kẹp** gốc); đối chứng `cd /docs` ⇒ `/docs` (không hồi quy
    #      `kt137-cdpwd`); **KT-205** (review KT-204, M2): `cd /docs/sub/` (**có** `/` cuối) ⇒
    #      `pwd` **vẫn** `/docs/sub` — đột biến "in ra mà **không** đổi `cwd`" phải ĐỎ. Đặt
    #      **trước** (8b) để `/README` còn 79 byte (8b truncate nó về 5); ca này **không** tạo tệp
    #      ở gốc nên không đổi `ls n=` của ca nào khác.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        case_begin
        for c in 'cd /docs/sub' 'pwd' 'cat deep.txt' 'cd ../..' 'pwd' 'cd /docs/sub/..' 'pwd' \
            'cd /docs/sub' 'cd ../../..' 'pwd' 'cd /docs' 'pwd' 'cd /' 'cd /docs/sub/' 'pwd'; do
            send_line "$c" || true
            sleep 0.15
        done
        if case_end 17; then
            if check_kt183_cd_sau "$CASE_LOG"; then
                echo "# kt183-cd-sau: PASS ($(grep -m1 -x -F '# shell: pwd /docs/sub' "$CASE_LOG"))"
            else
                echo "# kt183-cd-sau: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)"
                rc=1
            fi
        else
            echo "# kt183-cd-sau: FAIL (quá hạn — shell treo ở ca cd thư mục sâu)"
            CASE_TIMEOUT=1
            rc=1
        fi
    fi

    # (18) **KT-205** (review KT-204, **M4**): biên `MAX_PATH` **48 B** của `cd`. Hai lệnh `cd`
    #      dùng đường **đúng** 48 B (`/docs/sub/` + 38×`a`) và 49 B (`/docs/sub/` + 39×`a`), rồi
    #      `pwd` chứng minh cd hỏng **không** đổi thư mục hiện tại. Vì sao **ca riêng** (không nhét
    #      vào ca 17): ca 17 có phép kiểm `cd-kep-goc` đòi **0** dòng lỗi `cd` — hai lệnh biên ở
    #      đây **cố ý** sinh lỗi, nhét vào sẽ làm phép kiểm ấy mất nghĩa. Đặt **trước** (8b) để
    #      `/README` còn 79 byte; ca **không** tạo tệp nên không đổi `ls n=` của ca nào khác.
    if [ "$CASE_TIMEOUT" -eq 0 ]; then
        case_begin
        a38="$(printf 'a%.0s' $(seq 1 38))"
        a39="$(printf 'a%.0s' $(seq 1 39))"
        for c in 'cd /docs/sub' "cd /docs/sub/$a38" "cd /docs/sub/$a39" 'pwd'; do
            send_line "$c" || true
            sleep 0.15
        done
        if case_end 18; then
            if check_kt183_bien_path "$CASE_LOG"; then
                echo "# kt183-bien-path: PASS ($(grep -m1 -F '# shell: cd loi code=11 ' "$CASE_LOG" | cut -c1-60)…)"
            else
                echo "# kt183-bien-path: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)"
                rc=1
            fi
        else
            echo "# kt183-bien-path: FAIL (quá hạn — shell treo ở ca biên MAX_PATH)"
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
        echo "# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge + new-cmds-edge2 + new-cmds-edge3 + k7-uu-tien + k7-thu-muc + kt137-cdpwd + kt183-cd-sau + kt183-bien-path PASS"
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
        --check-image)
            [ "$#" -ge 2 ] || die "--check-image cần đường dẫn ảnh"
            IMAGE="$2"
            MODE_CHECK_IMAGE=1
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

# KT-145: `--check-image` chỉ chạy phép kiểm danh tính ảnh rồi thoát — dùng cho ca hồi quy
# `v2/test/boot/algo-strict.sh` (không mở QEMU nên ca kiểm chạy trong vài giây).
if [ "$MODE_CHECK_IMAGE" -eq 1 ]; then
    if vbrev_image_identity "$IMAGE"; then
        echo "REV-GATE: ACCEPT image=$IMAGE algo=$VBREV_IMG_ALGO inputs=$VBREV_IMG_INPUTS dirty=$VBREV_IMG_DIRTY"
        exit 0
    fi
    echo "REV-GATE: REFUSE reason=$VBREV_IMG_REASON image=$IMAGE algo=${VBREV_IMG_ALGO:-<none>} current_algo=$VBREV_ALGO"
    exit 1
fi

RC=0
if [ "$MODE_SELFTEST" -eq 1 ]; then
    run_selftest || RC=1
fi
if [ -n "$IMAGE" ]; then
    run_real || RC=1
fi
exit "$RC"
