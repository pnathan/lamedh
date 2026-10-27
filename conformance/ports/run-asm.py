#!/usr/bin/env python3
"""Conformance runner for lamedh-asm.

Build the driver, then run from the repo root:

    make -C lamedh-asm lamedhc
    python3 conformance/ports/run-asm.py [--bin PATH] [--verbose]

lamedhc runs one program per process with the prelude only, so each case is one
process. Cases run in a thread pool; the suite takes a few seconds.

Prints PASS / KNOWN-FAIL / FAIL / STALE per case and a summary. Exit status:
  0  every case passes, or fails and is listed in asm-known-divergences.txt
  1  a case fails and is not listed (regression), a listed case now passes
     (stale entry), an ID repeats, or the case count is not EXPECTED_TOTAL.
"""
import argparse
import glob
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

EXPECTED_TOTAL = 143
TIMEOUT_S = 5

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
LIT = r'"((?:[^"\\]|\\.)*)"'
CASE_RE = re.compile(
    r'^\(case\s+' + LIT + r'\s+' + LIT + r'\s+' + LIT + r'\s+' + LIT + r'\)\s*$',
    re.M,
)


def unescape(s):
    return re.sub(
        r'\\(.)', lambda m: {'n': '\n', 't': '\t'}.get(m.group(1), m.group(1)), s
    )


def load_cases():
    cases = []
    skipped = 0
    for fn in sorted(glob.glob(os.path.join(ROOT, 'conformance/cases/*.lisp'))):
        text = open(fn).read()
        found = [tuple(unescape(x) for x in m.groups()) for m in CASE_RE.finditer(text)]
        # Every "(case" at line start must have parsed, or the regex is stale.
        n_forms = len(re.findall(r'^\(case\b', text, re.M))
        if n_forms != len(found):
            sys.exit(f"{fn}: {n_forms} case forms, parsed {len(found)}")
        if os.path.basename(fn).startswith('99-'):
            skipped += len(found)
        else:
            cases.extend(found)
    return cases, skipped


def read_allowlist(path):
    ids = []
    for line in open(path):
        line = line.strip()
        if line and not line.startswith('#'):
            ids.append(line)
    return ids


def run_case(binary, tmp, idx, src):
    path = os.path.join(tmp, f'case{idx}.lisp')
    with open(path, 'w') as f:
        f.write('(prin1 (progn ' + src + '))')
    try:
        r = subprocess.run([binary, path], capture_output=True, text=True, timeout=TIMEOUT_S)
        return 'ERROR' if r.returncode != 0 else r.stdout
    except subprocess.TimeoutExpired:
        return 'TIMEOUT'


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--bin', default=os.path.join(ROOT, 'lamedh-asm/build/lamedhc'))
    ap.add_argument('--verbose', action='store_true',
                    help='print src/want/got for known failures too')
    args = ap.parse_args()
    if not os.path.exists(args.bin):
        sys.exit(f"{args.bin} not found; run: make -C lamedh-asm lamedhc")

    allow = read_allowlist(os.path.join(HERE, 'asm-known-divergences.txt'))
    cases, skipped = load_cases()
    ids = [c[0] for c in cases]
    dups = sorted({i for i in ids if ids.count(i) > 1})

    with tempfile.TemporaryDirectory() as tmp, ThreadPoolExecutor(max_workers=8) as ex:
        got = list(ex.map(lambda ic: run_case(args.bin, tmp, ic[0], ic[1][2]),
                          enumerate(cases)))

    pass_n = known = 0
    regress, stale = [], []
    for (cid, _spec, src, expect), g in zip(cases, got):
        ok = g == expect
        listed = cid in allow
        if ok and listed:
            stale.append(cid)
            print(f"STALE {cid} (allowlisted but now passes)")
        elif ok:
            pass_n += 1
            print(f"PASS {cid}")
        else:
            show = listed and not args.verbose
            if listed:
                known += 1
                print(f"KNOWN-FAIL {cid}")
            else:
                regress.append(cid)
                print(f"FAIL {cid}")
            if not show:
                print(f"  src:  {src}\n  want: {expect}\n  got:  {g}")
    for cid in allow:
        if cid not in ids:
            stale.append(cid)
            print(f"STALE {cid} (allowlisted but no such case)")

    print(f"ASM-CONFORMANCE total={len(cases)} pass={pass_n} known-fail={known} "
          f"regress={len(regress)} stale={len(stale)} skipped-disputed={skipped}")
    if dups:
        print("DUPLICATE IDs:", ' '.join(dups))
    if len(cases) != EXPECTED_TOTAL:
        print(f"case count {len(cases)}, expected {EXPECTED_TOTAL}")
    bad = regress or stale or dups or len(cases) != EXPECTED_TOTAL
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
