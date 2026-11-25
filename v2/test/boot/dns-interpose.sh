#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-406** — nhánh **`LD_PRELOAD` (`.so`, Linux) song song Mach-O `DYLD_INTERPOSE` (`.dylib`)**
# cho **DNS interpose**: chuyển resolver của tiến trình QEMU về `127.0.0.1:<cổng>` để soak/DNS
# test không phụ thuộc resolver host và **không cần mạng thật** (chỉ loopback).
#
# # Vì sao cần hai nhánh
#
# `soak-aarch64.sh` (KT-300) cần QEMU/slirp hỏi DNS ở server cục bộ. Cơ chế trên macOS là
# `DYLD_INTERPOSE` trên `res_9_*` (libresolv của Apple). Linux/glibc **không có** `DYLD_INTERPOSE`
# và resolver là `_res`/`res_init`/`res_ninit`; cơ chế tương ứng là `LD_PRELOAD` + `dlsym(RTLD_NEXT)`
# để bọc `res_init`/`res_ninit` rồi ghi đè `nsaddr_list[0]`. Tệp này sinh **một** nguồn C với hai
# nhánh `#ifdef __APPLE__`, biên dịch theo `uname -s`, và tự kiểm bằng một chương trình dò
# **không mở socket** (test nhỏ, không cần mạng thật).
#
# # Bằng chứng
#
# In **một** dòng `key=value`:
#
# ```text
#   # dns-interpose: os=macos kind=dylib var=DYLD_INSERT_LIBRARIES server=127.0.0.1 port=5353 ok=1
#   # dns-interpose: os=linux kind=so    var=LD_PRELOAD            server=127.0.0.1 port=5353 ok=1
# ```
#
# `ok=1` chỉ khi chương trình dò (chạy **dưới** injection) đọc resolver = `127.0.0.1:5353` —
# cổng 5353 **không** phải resolver mặc định, nên `ok=1` chứng minh interpose **thật sự** có tác
# dụng, không phải "in ra cho đẹp".
#
# Dùng:
#   bash v2/test/boot/dns-interpose.sh                    # tự kiểm trên OS hiện tại (exit 0/1)
#   bash v2/test/boot/dns-interpose.sh --lib <thư-mục>    # chỉ build, in `lib= kind= var=`
#   bash v2/test/boot/dns-interpose.sh --help
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"

MODE="check"
OUTDIR=""
while [ $# -gt 0 ]; do
    case "$1" in
        --lib) MODE="lib"; OUTDIR="${2:-}"; shift 2 ;;
        --check) MODE="check"; shift ;;
        -h | --help)
            sed -n '1,40p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "dns-interpose.sh: tham số lạ: $1" >&2; exit 2 ;;
    esac
done

# ------------------------------------------------------------------ nguồn C (một nguồn, hai nhánh)
# Cổng/địa chỉ đích đọc từ môi trường **lúc chạy** (mặc định 127.0.0.1:53) để cùng một thư viện
# dùng cho soak (cổng động) và cho phép tự kiểm (cổng 5353 — không phải resolver mặc định).
write_c() { # <tệp>
    cat >"$1" <<'C'
/* SPDX-License-Identifier: MIT OR Apache-2.0 */
/* KT-406 — interpose resolver: macOS (DYLD_INTERPOSE) + Linux (LD_PRELOAD). */
#include <stdlib.h>
#include <string.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <resolv.h>

#define KT406_MAX 8

static void kt406_target(struct sockaddr_in *sin) {
    const char *addr = getenv("VIFAST_DNS_ADDR");
    const char *port = getenv("VIFAST_DNS_PORT");
    memset(sin, 0, sizeof(*sin));
    sin->sin_family = AF_INET;
    sin->sin_port = htons((unsigned short)(port && *port ? atoi(port) : 53));
    if (!addr || !*addr) {
        addr = "127.0.0.1";
    }
    inet_pton(AF_INET, addr, &sin->sin_addr);
}

#if defined(__APPLE__)
/* ---- macOS: thay ba hàm resolver của libresolv (cùng khuôn soak KT-300) ---- */
static int kt406_ninit(struct __res_state *statp) {
    memset(statp, 0, sizeof(*statp));
    return 0;
}

static int kt406_getservers(struct __res_state *statp, union res_sockaddr_union *u, int max) {
    (void)statp;
    if (max < 1) {
        return 0;
    }
    memset(u, 0, sizeof(u[0]));
    kt406_target(&u[0].sin);
    return 1;
}

static void kt406_ndestroy(struct __res_state *statp) {
    (void)statp;
}

#define KT406_INTERPOSE(_replacement, _replacee)                                    \
    __attribute__((used)) static struct {                                           \
        const void *replacement;                                                    \
        const void *replacee;                                                       \
    } _kt406_interpose_##_replacee __attribute__((section("__DATA,__interpose"))) = \
        { (const void *)(unsigned long)&_replacement,                               \
          (const void *)(unsigned long)&_replacee };

KT406_INTERPOSE(kt406_ninit, res_9_ninit)
KT406_INTERPOSE(kt406_getservers, res_9_getservers)
KT406_INTERPOSE(kt406_ndestroy, res_9_ndestroy)

#else
/* ---- Linux/glibc: `LD_PRELOAD` + `dlsym(RTLD_NEXT)` bọc `res_init`/`res_ninit` ---- */
#define _GNU_SOURCE
#include <dlfcn.h>

extern struct __res_state _res;

static int kt406_override(struct __res_state *st) {
    struct sockaddr_in sin;
    kt406_target(&sin);
    if (!st) {
        return 0;
    }
    st->nscount = 1;
    memset(&st->nsaddr_list[0], 0, sizeof(st->nsaddr_list[0]));
    st->nsaddr_list[0].sin_family = AF_INET;
    st->nsaddr_list[0].sin_port = sin.sin_port;
    st->nsaddr_list[0].sin_addr = sin.sin_addr;
    return 1;
}

int res_init(void) {
    static int (*real)(void);
    if (!real) {
        real = (int (*)(void))dlsym(RTLD_NEXT, "res_init");
    }
    int rc = real ? real() : 0;
    kt406_override(&_res);
    return rc;
}

int res_ninit(res_state st) {
    static int (*real)(res_state);
    if (!real) {
        real = (int (*)(res_state))dlsym(RTLD_NEXT, "res_ninit");
    }
    int rc = real ? real(st) : 0;
    kt406_override(st);
    return rc;
}
#endif
C
}

# ------------------------------------------------------------------ chương trình dò (không mạng)
write_probe() { # <tệp>
    cat >"$1" <<'C'
/* SPDX-License-Identifier: MIT OR Apache-2.0 */
/* KT-406 — đọc resolver đang có hiệu lực và in `server=… port=…`. KHÔNG mở socket. */
#include <stdio.h>
#include <string.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <resolv.h>

int main(void) {
#if defined(__APPLE__)
    struct __res_state st;
    union res_sockaddr_union u[8];
    memset(&st, 0, sizeof(st));
    if (res_9_ninit(&st) != 0) {
        return 2;
    }
    int n = res_9_getservers(&st, u, 8);
    if (n < 1) {
        return 3;
    }
    if (u[0].sin.sin_family != AF_INET) {
        return 4;
    }
    char buf[64];
    inet_ntop(AF_INET, &u[0].sin.sin_addr, buf, sizeof(buf));
    printf("server=%s port=%u\n", buf, (unsigned)ntohs(u[0].sin.sin_port));
    res_9_ndestroy(&st);
#else
    res_init();
    if (_res.nscount < 1 || _res.nsaddr_list[0].sin_family != AF_INET) {
        return 4;
    }
    char buf[64];
    inet_ntop(AF_INET, &_res.nsaddr_list[0].sin_addr, buf, sizeof(buf));
    printf("server=%s port=%u\n", buf,
           (unsigned)ntohs(_res.nsaddr_list[0].sin_port));
#endif
    return 0;
}
C
}

# ------------------------------------------------------------------ build
os_kind() {
    case "$(uname -s)" in
        Darwin) echo "macos dylib DYLD_INSERT_LIBRARIES" ;;
        Linux) echo "linux so LD_PRELOAD" ;;
        *) echo "unsupported" ;;
    esac
}

# `dns_interpose_build <thư-mục>` — build và in **ba** trường `lib=`, `kind=`, `var=` (mỗi dòng).
# Trả 0/2. Dùng được cả khi source tệp này (cùng tên hàm, không `exit`).
dns_interpose_build() { # <thư-mục>
    local dir="${1:-}" kind os var lib
    [ -n "$dir" ] || {
        echo "dns-interpose: thiếu thư mục build" >&2
        return 2
    }
    mkdir -p "$dir" || return 2
    read -r os kind var <<<"$(os_kind)"
    if [ "$os" = "unsupported" ]; then
        echo "dns-interpose: OS '$(uname -s)' chưa hỗ trợ (cần Darwin hoặc Linux)" >&2
        return 2
    fi
    command -v cc >/dev/null 2>&1 || {
        echo "dns-interpose: không có cc" >&2
        return 2
    }
    write_c "$dir/kt406-dns-interpose.c"
    if [ "$kind" = "dylib" ]; then
        lib="$dir/kt406-dns-interpose.dylib"
        rm -f "$lib"
        if ! cc -dynamiclib -o "$lib" "$dir/kt406-dns-interpose.c" -lresolv \
            >"$dir/kt406-dns-interpose-build.log" 2>&1; then
            tail -20 "$dir/kt406-dns-interpose-build.log" >&2
            return 2
        fi
        if command -v codesign >/dev/null 2>&1; then
            codesign -s - "$lib" >/dev/null 2>&1 || true
        fi
    else
        lib="$dir/kt406-dns-interpose.so"
        rm -f "$lib"
        if ! cc -shared -fPIC -o "$lib" "$dir/kt406-dns-interpose.c" -ldl -lresolv \
            >"$dir/kt406-dns-interpose-build.log" 2>&1; then
            tail -20 "$dir/kt406-dns-interpose-build.log" >&2
            return 2
        fi
    fi
    [ -f "$lib" ] || {
        echo "dns-interpose: không thấy $lib sau build" >&2
        return 2
    }
    printf 'lib=%s\nkind=%s\nvar=%s\nos=%s\n' "$lib" "$kind" "$var" "$os"
    return 0
}

# ------------------------------------------------------------------ chế độ `--lib`
if [ "$MODE" = "lib" ]; then
    dns_interpose_build "$OUTDIR"
    exit $?
fi

# ------------------------------------------------------------------ tự kiểm trên OS hiện tại
# Cổng 5353 **không** phải resolver mặc định ⇒ `ok=1` chứng minh injection có tác dụng.
TMPD="$(mktemp -d "${TMPDIR:-/tmp}/kt406-dnsi.XXXXXX")" || {
    echo "dns-interpose: không tạo được thư mục tạm" >&2
    exit 2
}
trap 'rm -rf "$TMPD"' EXIT
BUILD_OUT="$(dns_interpose_build "$TMPD")" || exit 2
LIB="$(printf '%s\n' "$BUILD_OUT" | sed -n 's/^lib=//p')"
KIND="$(printf '%s\n' "$BUILD_OUT" | sed -n 's/^kind=//p')"
VAR="$(printf '%s\n' "$BUILD_OUT" | sed -n 's/^var=//p')"
OS="$(printf '%s\n' "$BUILD_OUT" | sed -n 's/^os=//p')"

write_probe "$TMPD/kt406-dns-probe.c"
cc -o "$TMPD/kt406-dns-probe" "$TMPD/kt406-dns-probe.c" -lresolv >"$TMPD/probe-build.log" 2>&1 || {
    tail -20 "$TMPD/probe-build.log" >&2
    exit 2
}

PORT=5353
if ! env VIFAST_DNS_ADDR=127.0.0.1 VIFAST_DNS_PORT="$PORT" \
    "$VAR=$LIB" "$TMPD/kt406-dns-probe" >"$TMPD/probe.out" 2>"$TMPD/probe.err"; then
    echo "# dns-interpose: probe FAIL rc=$? (stderr: $(tr '\n' ' ' <"$TMPD/probe.err"))"
    echo "# dns-interpose: os=$OS kind=$KIND var=$VAR server=- port=- ok=0"
    exit 1
fi
SERVER="$(sed -n 's/^server=\([^ ]*\) port=.*/\1/p' "$TMPD/probe.out")"
GOT_PORT="$(sed -n 's/^server=[^ ]* port=\([0-9][0-9]*\).*/\1/p' "$TMPD/probe.out")"
OK=0
if [ "$SERVER" = "127.0.0.1" ] && [ "${GOT_PORT:-0}" = "$PORT" ]; then
    OK=1
fi
echo "# dns-interpose: os=$OS kind=$KIND var=$VAR server=${SERVER:--} port=${GOT_PORT:--} ok=$OK"
[ "$OK" -eq 1 ] || exit 1
exit 0
