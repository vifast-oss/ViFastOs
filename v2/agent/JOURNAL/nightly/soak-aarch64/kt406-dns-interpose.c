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
