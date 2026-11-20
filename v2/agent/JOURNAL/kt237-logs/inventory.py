#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-237 inventory: liet ke moi call-site ghi trang thai TCB."""
import os, re, sys, json

ROOTS = ["v2/kernel", "v2/sim", "v2/lib"]
FUNCS = ["transition_tcb", "wake_step", "wake_from_unlink", "wake_from",
         "wake_addr", "apply_mail", "wake"]

def strip_comments_strings(src):
    """Thay comment/string bang khoang trang, giu do dai + xuong dong."""
    out = list(src)
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == '/' and i+1 < n and src[i+1] == '/':
            j = src.find('\n', i)
            if j < 0: j = n
            for k in range(i, j): out[k] = ' '
            i = j
        elif c == '/' and i+1 < n and src[i+1] == '*':
            j = src.find('*/', i+2)
            j = n if j < 0 else j+2
            for k in range(i, j):
                if src[k] != '\n': out[k] = ' '
            i = j
        elif c == '"':
            # raw string?
            j = i+1
            while j < n:
                if src[j] == '\\': j += 2; continue
                if src[j] == '"': j += 1; break
                j += 1
            for k in range(i, min(j,n)):
                if src[k] != '\n': out[k] = ' '
            i = j
        elif c == "'":
            # char literal or lifetime; only strip if looks like char
            m = re.match(r"'(?:\\.|[^\\'])'", src[i:])
            if m:
                for k in range(i, i+m.end()): out[k] = ' '
                i += m.end()
            else:
                i += 1
        else:
            i += 1
    return ''.join(out)

def enclosing_fn(lines, lineno):
    """Tim fn bao quanh (1-based lineno) bang cach quet nguoc tim 'fn name'."""
    # tim dong bat dau fn gan nhat truoc do co '{' chua dong nay
    depth = 0
    for i in range(lineno-1, -1, -1):
        line = lines[i]
        # dem ngoac nguoc
        depth += line.count('}') - line.count('{')
        if depth < 0:
            # da ra khoi block hien tai -> tiep tuc? thuc ra ta can tim fn
            pass
        m = re.search(r'\bfn\s+([A-Za-z_][A-Za-z0-9_]*)', line)
        if m and depth <= 0:
            return m.group(1), i+1
    return "?", 0

def main():
    rows = []
    for root in ROOTS:
        for dirpath, dirnames, filenames in os.walk(root):
            if 'JOURNAL' in dirpath or '/target' in dirpath: continue
            for fn in filenames:
                if not fn.endswith('.rs'): continue
                path = os.path.join(dirpath, fn)
                src = open(path, encoding='utf-8', errors='replace').read()
                clean = strip_comments_strings(src)
                lines = clean.split('\n')
                rawlines = src.split('\n')
                for idx, line in enumerate(lines):
                    for f in FUNCS:
                        for m in re.finditer(r'(?<![A-Za-z0-9_])' + re.escape(f) + r'\s*\(', line):
                            # bo qua dinh nghia fn
                            pre = line[:m.start()]
                            if re.search(r'\bfn\s+$', pre): continue
                            # bo qua neu la khai bao trong comment da strip
                            fnname, fnline = enclosing_fn(lines, idx+1)
                            # lay doi so thu 2 tho
                            rest = line[m.end():]
                            # gom den khi can bang ngoac
                            depth = 1; buf = rest; j = idx
                            while depth > 0 and j+1 < len(lines):
                                for ch in buf:
                                    if ch == '(': depth += 1
                                    elif ch == ')': depth -= 1
                                if depth > 0:
                                    j += 1; buf += ' ' + lines[j]
                            args = buf
                            rows.append(dict(file=path, line=idx+1, func=f,
                                             enclosing=fnname, fnline=fnline,
                                             args=args[:200].strip(),
                                             raw=rawlines[idx].strip()[:200]))
    json.dump(rows, open('v2/agent/JOURNAL/kt237-logs/callsites.json','w'), indent=1)
    print("total", len(rows))
    from collections import Counter
    print(Counter(r['func'] for r in rows))
    print(Counter(r['file'] for r in rows).most_common(20))

main()
