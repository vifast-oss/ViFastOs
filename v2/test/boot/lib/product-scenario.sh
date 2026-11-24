#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-406** — **kịch bản shell DÙNG CHUNG 3 arch** cho runner `product-{riscv64,aarch64,x86_64}.sh`.
#
# Vì sao có tệp này: trước KT-406 mỗi arch một kiểu "lái shell" (riscv64: FIFO UART; aarch64/x86:
# kernel bơm script vào ring) nên **không** có phép so sánh nào giữa ba arch. Tệp này chốt **một**
# danh sách `lệnh<TAB>regex evidence` cho cả ba runner dùng — thêm/bớt lệnh ở đây là đổi cả ba.
#
# Kịch bản chỉ dùng lệnh **có trên cả ba ảnh** (shell không FS/không net vẫn chạy được):
#   `help` → `echo <text>` → `ps` → `exit`
# Bằng chứng là **dòng do shell in ra** (không tin lời runner), khớp neo `^…$` như các runner cũ.
#
# Dùng:
#   . "$HERE/lib/product-scenario.sh"
#   product_scenario_lines | while IFS=$'\t' read -r cmd re; do send_wait "$cmd" "$re" || bad …; done
#   product_scenario_check <log>                 # trả 0/1, in lý do ra stdout
#   product_uart_irq_check <log>                 # đòi dòng `# uart:` cuối có ok=1, drops=0
#
# Không chạm `run-common.sh` (D2-259): tệp này là thư viện độc lập, không sửa runner cũ.

# In các cặp `<lệnh>\t<regex>` theo **thứ tự** chạy. Regex là ERE cho `grep -E`.
product_scenario_lines() {
    printf '%s\t%s\n' 'help' '^# shell: help cmds=[0-9]+$'
    printf '%s\t%s\n' 'echo KT406' '^# shell: echo KT406$'
    printf '%s\t%s\n' 'ps' '^# shell: ps n=[0-9]+$'
    printf '%s\t%s\n' 'exit' '^# shell: exit code=0$'
}

# Số dòng của kịch bản (để runner đối chiếu đã gửi đủ).
product_scenario_count() {
    product_scenario_lines | wc -l | tr -d ' '
}

# `product_scenario_check <log>` — kiểm **đủ** 4 mốc bằng chứng + bảng `ps` có ít nhất một dòng.
# In mỗi mốc thiếu kèm regex; trả 1 nếu thiếu bất kỳ mốc nào (fail-closed, không "PASS một phần").
product_scenario_check() {
    local log="$1" cmd re bad=0 n=0
    if [ ! -f "$log" ]; then
        echo "# không có log $log"
        return 1
    fi
    while IFS=$'\t' read -r cmd re; do
        n=$((n + 1))
        if ! LC_ALL=C grep -aqE -- "$re" "$log"; then
            echo "# thiếu evidence cho '$cmd' (regex /$re/) — mốc $n"
            bad=1
        fi
    done < <(product_scenario_lines)
    # Bảng `ps`: shell in `# shell: ps n=N` rồi N dòng `  pid=… state=…`. `n>0` mà **không** dòng
    # nào là bảng rỗng giả — đòi ít nhất một dòng đúng khuôn.
    if ! LC_ALL=C grep -aqE '^  pid=[0-9]+ state=(Ready|Running|Inactive|Blocked[^ ]*|Suspended)$' "$log"; then
        echo "# thiếu bảng TCB '  pid=<n> state=<…>' của ps"
        bad=1
    fi
    # Dòng kết thúc của shell: `# shell: exit code=0` (shell tự in) — bằng chứng `exit` thật sự
    # chạy, không phải chỉ gõ chữ.
    if ! LC_ALL=C grep -aqE '^# shell: exit code=0$' "$log"; then
        echo "# thiếu '# shell: exit code=0'"
        bad=1
    fi
    return "$bad"
}

# `product_uart_irq_check <log>` — đòi đường **UART RX IRQ thật** có bằng chứng `key=value`.
#
# Hai dòng được đòi:
#   1. lúc bật: `# uart: … ok=1` (đường ngắt đã sẵn sàng, đọc lại từ thanh ghi);
#   2. lúc kết thúc: `# uart: irq=<n> rx=<byte> … drops=0 … ok=1` với `n ≥ 1`, `rx ≥ 1` — byte
#      đã **thật sự** đi qua thiết bị → ngắt → handler → ring, không phải kernel bơm.
# Tách khỏi `product_scenario_check` để runner arch nào chưa có UART IRQ (nếu có) vẫn báo lý do
# riêng thay vì đỏ chung một chỗ.
product_uart_irq_check() {
    local log="$1" bad=0
    if ! LC_ALL=C grep -aqE '^# uart: .*ok=1$' "$log"; then
        echo "# thiếu dòng bật UART RX IRQ '# uart: … ok=1'"
        bad=1
    fi
    local line
    line="$(LC_ALL=C grep -aE '^# uart: irq=[0-9]+ rx=[0-9]+ ' "$log" | tail -1)"
    if [ -z "$line" ]; then
        echo "# thiếu dòng tổng kết '# uart: irq=<n> rx=<byte> …'"
        return 1
    fi
    local irq rx drops
    irq="$(printf '%s' "$line" | sed -n 's/.* irq=\([0-9][0-9]*\) .*/\1/p')"
    rx="$(printf '%s' "$line" | sed -n 's/.* rx=\([0-9][0-9]*\) .*/\1/p')"
    drops="$(printf '%s' "$line" | sed -n 's/.* drops=\([0-9][0-9]*\) .*/\1/p')"
    [ "${irq:-0}" -ge 1 ] || {
        echo "# uart irq=${irq:-0} — không có ngắt RX nào (đường ngắt chưa chạy)"
        bad=1
    }
    [ "${rx:-0}" -ge 1 ] || {
        echo "# uart rx=${rx:-0} — không byte nào vào ring (input không đi qua UART)"
        bad=1
    }
    [ "${drops:-1}" -eq 0 ] || {
        echo "# uart drops=${drops:-?} — ring đầy, mất byte (phải 0 ở kịch bản thường)"
        bad=1
    }
    case "$line" in
        *' ok=1') ;;
        *)
            echo "# uart ok=0 — handler/ring chưa đủ bằng chứng"
            bad=1
            ;;
    esac
    return "$bad"
}
