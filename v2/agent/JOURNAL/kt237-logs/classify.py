#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-237: phan loai doi so thu 2 cua transition_tcb."""
import json, re, sys
from collections import Counter, defaultdict

rows = json.load(open('v2/agent/JOURNAL/kt237-logs/callsites.json'))

def split_args(s):
    """Tach doi so theo dau phay muc 0 (da strip comment/string)."""
    out, depth, cur = [], 0, ''
    for ch in s:
        if ch in '([{': depth += 1
        elif ch in ')]}': depth -= 1
        if ch == ',' and depth == 0:
            out.append(cur.strip()); cur = ''
        else:
            cur += ch
    if cur.strip(): out.append(cur.strip())
    return out

def classify(a):
    a = a.strip()
    if a.startswith('TcbState::Ready'): return 'ready'
    if re.match(r'TcbState::(Inactive|Running|BlockedOnSend|BlockedOnRecv|BlockedOnReply|BlockedOnNotification|BlockedOnAddress|Sleeping|Stopping|Dead)\b', a): return 'nonready'
    if re.match(r'^[A-Za-z_][A-Za-z0-9_]*$', a): return 'var'
    if 'TcbState::Ready' in a: return 'ready-expr'
    if 'TcbState::' in a: return 'nonready-expr'
    return 'other'

def is_test(path):
    if '/tests/' in path: return True
    base = path.split('/')[-1]
    if base.startswith('ktest_'): return True
    if base in ('proc_demo.rs','tls_tests.rs','capalloc_tests.rs'): return True
    return False

res = defaultdict(list)
for r in rows:
    if r['func'] != 'transition_tcb': continue
    args = split_args(r['args'])
    # args[0] la doi so 1 (tcb) vi args bat dau sau '('
    second = args[1] if len(args) > 1 else '?'
    c = classify(second)
    res[(is_test(r['file']), c)].append((r['file'], r['line'], r['enclosing'], second[:60]))

for k in sorted(res, key=lambda x: (x[0], x[1])):
    print(f"{'TEST' if k[0] else 'PROD'} {k[1]:14s} {len(res[k])}")
print()
print("=== PROD ready ===")
for x in res[(False,'ready')]: print(x)
print("=== PROD var/other ===")
for k in [(False,'var'),(False,'other'),(False,'ready-expr'),(False,'nonready-expr')]:
    for x in res[k]: print(k[1], x)
print("=== TEST ready (count by file) ===")
c = Counter(x[0] for x in res[(True,'ready')])
for f,n in c.most_common(): print(n, f)
