import re, sys
def parse(path):
    rows=[]; prev_t=None; prev_name=None
    for line in open(path, encoding='utf-8', errors='replace'):
        m=re.match(r'^(\d+\.\d+) (.*)$', line.rstrip('\n'))
        if not m: continue
        t=float(m.group(1)); txt=m.group(2)
        if not txt.startswith('---'): continue
        name=txt[4:].strip()
        mm = re.search(r': (OK|FAIL)\b', name)
        if mm:
            ok = mm.group(1)=='OK'
            nm = name[:mm.start()].strip()
            if prev_t is not None:
                rows.append((nm, t-prev_t, ok))
            prev_t=t; prev_name=nm
        else:
            prev_t=t; prev_name=name
    return rows
for p in sys.argv[1:]:
    rows=parse(p)
    tot=sum(r[1] for r in rows)
    print(f"### {p}  (sum of steps = {tot:.2f}s)")
    for nm,d,ok in rows:
        print(f"{d:7.2f}  {'OK ' if ok else 'FAIL'} {nm}")
    print()
