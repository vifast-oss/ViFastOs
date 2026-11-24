#!/usr/bin/env bash
# Kiểm tra hậu vá — chạy từ gốc repo
set -u
R=review/work/CHECK.md
: > "$R"
say(){ echo "$*" >> "$R"; }
chk(){ # chk "mô tả" "lệnh" "kỳ vọng"
  local desc="$1" cmd="$2" want="$3" got
  got=$(eval "$cmd" 2>/dev/null | wc -l | tr -d ' ')
  if [ "$got" = "$want" ]; then say "- OK   $desc ($got)"; else say "- FAIL $desc: got=$got want=$want"; fi
}
have(){ local desc="$1" pat="$2" f="$3"; if grep -qF -- "$pat" "$f"; then say "- OK   $desc"; else say "- FAIL $desc (thiếu: $pat)"; fi; }
lack(){ local desc="$1" pat="$2" f="$3"; if grep -qF -- "$pat" "$f"; then say "- FAIL $desc (còn: $pat)"; else say "- OK   $desc"; fi; }

V=docs/v2/ViFastOs-v2-thiet-ke.md
S=docs/viscript/ViScript-thiet-ke.md
L=docs/viscript/SPEC-lex.md

say "## docs/v2"
chk "D2-xx xuất hiện dòng bảng" "grep -c '^| D2-' $V" 24
chk "task KT- trong §13" "grep -c '^| KT-' $V" 14
have "Exit syscall" "| 14 | Exit(code)" "$V"
have "48 B slot" "48 B" "$V"
lack "không còn 32 byte slot" "Slot cap** 32 byte" "$V"
lack "không còn 750" "750" "$V"
lack "không còn 90 ngày" "90 ngày" "$V"
have "K0.5" "K0.5" "$V"
have "K3.5" "K3.5" "$V"
have "KError 19" "19 mã" "$V"
lack "không còn D-22" "D-22" "$V"
have "phụ lục sổ lỗi" "Sổ lỗi v1" "$V"
have "phụ lục Q1-Q12" "Q1–Q12" "$V"
have "thay đổi sau review" "Thay đổi sau review" "$V"

say "## docs/viscript"
have "CmdCST" "CmdCST" "$S"
have "50pct" "50pct" "$S"
lack "không còn 50% literal" "| Phần trăm | \`50%\`" "$S"
have "xs[i]?" "xs[i]?" "$S"
have "bảng ưu tiên" "Bảng ưu tiên" "$S"
have "ST-02a" "ST-02a" "$S"
have "VD-11" "VD-11" "$S"
have "Future/Task" "Task[T]" "$S"
have "2 pha" "hai pha" "$S"
chk "luật EBNF (dòng ::=)" "grep -c '::=' $S" 55

say "## SPEC-lex"
[ -f "$L" ] && say "- OK   tồn tại" || say "- FAIL thiếu SPEC-lex"
have "bảng token" "Token" "$L"
have "escape" "\\\\u{" "$L"
have "mã lỗi" "VS0107" "$L"
chk "golden token (dòng =>)" "grep -c '=>' $L" 40

say "## điều khiển"
for f in AGENTS.md v2/AGENTS.md viscript/AGENTS.md \
         v2/agent/QUEUE.md v2/agent/DECISIONS.md v2/agent/STATE.md v2/agent/BLOCKED.md \
         viscript/agent/QUEUE.md viscript/agent/DECISIONS.md viscript/agent/STATE.md viscript/agent/BLOCKED.md \
         docs/v1/KNOWN-ISSUES.md; do
  [ -f "$f" ] && say "- OK   $f" || say "- FAIL thiếu $f"
done
chk "QUEUE v2 KT rows" "grep -c '^| KT-' v2/agent/QUEUE.md" 14
chk "QUEUE ST rows" "grep -c '^| ST-' viscript/agent/QUEUE.md" 10
chk "DECISIONS v2 D2 rows" "grep -c '^| D2-' v2/agent/DECISIONS.md" 24
chk "DECISIONS v2 D-01 rows" "grep -c '^| D-0' v2/agent/DECISIONS.md" 8
chk "KNOWN-ISSUES mục" "grep -c '^## ' docs/v1/KNOWN-ISSUES.md" 7
say "## archive"
[ -d docs/v1/decisions ] && say "- OK   docs/v1/decisions" || say "- FAIL docs/decisions chưa chuyển"
[ -f docs/v1/overnight-log.md ] && say "- OK   overnight-log" || say "- FAIL overnight-log"
n=$(ls docs/agent2-report-*.md 2>/dev/null | wc -l | tr -d ' '); [ "$n" = "0" ] && say "- OK   reports đã chuyển" || say "- FAIL còn $n report ở docs/"
echo "xem $R"
