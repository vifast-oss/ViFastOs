#!/bin/bash
# Đo cấu hình L0 (D — A0 duyệt "đo rồi chọn"). Cùng cây, cùng binary, chỉ đổi jobs/threads.
set -u
cd /tmp/kt242-perf || exit 1
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.99.1
MANIFEST=v2/Cargo.toml
tmpd="$(mktemp -d /tmp/l0bench.XXXXXX)"
trap 'rm -rf "$tmpd"' EXIT

echo "== build test binaries (một lần)"
cargo test --manifest-path "$MANIFEST" --workspace --no-run --message-format=json >"$tmpd/build.json" 2>"$tmpd/build.err" || { tail -5 "$tmpd/build.err"; exit 1; }
python3 - "$tmpd/build.json" >"$tmpd/bins" <<'PY'
import json, os, sys
seen=set()
for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    try: m=json.loads(line)
    except Exception: continue
    if m.get("reason")!="compiler-artifact" or not m.get("profile",{}).get("test"): continue
    e=m.get("executable")
    if not e or e in seen: continue
    seen.add(e)
    sys.stdout.write("%s\0%s\0" % (e, os.path.dirname(m.get("manifest_path") or "")))
PY

cat >"$tmpd/run-one.sh" <<'SHEOF'
#!/bin/sh
d="$1"; b="$2"; pkg="$3"; n="$(basename "$b")"
[ -n "$pkg" ] && cd "$pkg" || exit 0
if [ -n "${THREADS:-}" ]; then "$b" --test-threads="$THREADS" >"$d/$n.log" 2>&1; else "$b" >"$d/$n.log" 2>&1; fi
echo $? >"$d/$n.rc"
SHEOF
chmod +x "$tmpd/run-one.sh"

: >"$tmpd/serial"; : >"$tmpd/par"
while IFS= read -r -d '' b && IFS= read -r -d '' pkg; do
    n="$(basename "$b")"
    case " kt30_stress spsc " in
        *" ${n%%-*} "*) printf '%s\0%s\0' "$b" "$pkg" >>"$tmpd/serial" ;;
        *) printf '%s\0%s\0' "$b" "$pkg" >>"$tmpd/par" ;;
    esac
done <"$tmpd/bins"

for cfg in "6 auto" "6 2" "8 1" "4 3" "10 1" "12 1"; do
    set -- $cfg; jobs=$1; th=$2
    rm -f "$tmpd"/*.rc "$tmpd"/*.log
    t0=$(perl -MTime::HiRes=time -e 'printf "%.0f", time*1000')
    if [ "$th" = "auto" ]; then
        THREADS="" xargs -0 -P "$jobs" -n 2 sh "$tmpd/run-one.sh" "$tmpd" <"$tmpd/par"
    else
        THREADS="$th" xargs -0 -P "$jobs" -n 2 sh "$tmpd/run-one.sh" "$tmpd" <"$tmpd/par"
    fi
    while IFS= read -r -d '' b && IFS= read -r -d '' pkg; do
        if [ "$th" = "auto" ]; then THREADS="" sh "$tmpd/run-one.sh" "$tmpd" "$b" "$pkg"; else THREADS="$th" sh "$tmpd/run-one.sh" "$tmpd" "$b" "$pkg"; fi
    done <"$tmpd/serial"
    t1=$(perl -MTime::HiRes=time -e 'printf "%.0f", time*1000')
    nfail=0; ntest=0
    while IFS= read -r -d '' b && IFS= read -r -d '' pkg; do
        n="$(basename "$b")"
        [ -f "$tmpd/$n.rc" ] || { nfail=$((nfail+1)); continue; }
        [ "$(cat "$tmpd/$n.rc")" -ne 0 ] && nfail=$((nfail+1))
        ntest=$((ntest + $(grep -oE '[0-9]+ passed' "$tmpd/$n.log" 2>/dev/null | awk '{s+=$1} END {print s+0}')))
    done <"$tmpd/bins"
    echo "jobs=$jobs threads=$th  =>  $((t1-t0)) ms, $ntest test, $nfail đỏ"
done
