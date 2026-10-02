#!/usr/bin/env python3
"""Turn results/json/*.json (hyperfine) + outputs.tsv into a markdown summary.

net = median(kernel) - median(startup of that host). Relative speed is
net(rust-interp) / net(impl): >1 means faster than the Rust interpreter.
A net time under 3 sigma of the host's startup noise is flagged '~' (the
kernel is startup-bound on that host and the ratio is only indicative).
"""
import csv, json, os, sys
from collections import defaultdict

res = sys.argv[1]
J = os.path.join(res, "json")
IMPLS = ["rust-interp", "rust-default", "rust-jit", "sbcl", "asm"]
SCALED = ["rust-jit-scaled", "asm-scaled"]
HOST = {"rust-interp": "rust", "rust-default": "rust", "rust-jit": "rust", "sbcl": "sbcl", "asm": "asm",
        "rust-jit-scaled": "rust", "asm-scaled": "asm"}

def load(path):
    with open(path) as f:
        r = json.load(f)["results"][0]
    return r

startup = {}
for h in ("rust", "sbcl", "asm"):
    p = os.path.join(J, f"startup-{h}.json")
    if os.path.exists(p):
        startup[h] = load(p)

outputs = defaultdict(dict)
with open(os.path.join(res, "outputs.tsv")) as f:
    for row in csv.DictReader(f, delimiter="\t"):
        outputs[row["kernel"]][row["impl"]] = row["output"]

kernels = sorted(outputs)
cells = {}
for k in kernels:
    for i in IMPLS + SCALED:
        p = os.path.join(J, f"{k}--{i}.json")
        if os.path.exists(p):
            cells[(k, i)] = load(p)

def net(k, i):
    r = cells.get((k, i))
    if r is None:
        return None, False
    s = startup[HOST[i]]
    n = r["median"] - s["median"]
    return n, n < 3 * max(s["stddev"] or 0.0, 0.002)

print("# Tri-implementation benchmark summary\n")
print("## Startup (empty program)\n")
print("| host | median | min | stddev |")
print("|---|---:|---:|---:|")
for h, s in startup.items():
    print(f"| {h} | {s['median']*1000:.1f} ms | {s['min']*1000:.1f} ms | {s['stddev']*1000:.1f} ms |")

print("\n## Kernel times (median of runs, startup subtracted)\n")
print("Cell: net median seconds (raw median, min) and speed relative to rust-interp. "
      "`~` = within 3σ of startup noise (startup-bound). `—` = not expressible on that tier.\n")
print("| kernel | " + " | ".join(IMPLS) + " |")
print("|---|" + "---:|" * len(IMPLS))
for k in kernels:
    base, _ = net(k, "rust-interp")
    row = []
    for i in IMPLS:
        n, flag = net(k, i)
        if n is None:
            row.append("—"); continue
        r = cells[(k, i)]
        if i == "rust-interp":
            row.append(f"{n:.3f} s ({r['median']:.3f}, {r['min']:.3f}) 1.0×")
        elif flag:
            row.append(f"~{n:.3f} s ({r['median']:.3f}, {r['min']:.3f}) *startup-bound*")
        else:
            row.append(f"{n:.3f} s ({r['median']:.3f}, {r['min']:.3f}) **{base / n:.1f}×**")
    print(f"| {k} | " + " | ".join(row) + " |")

print("\n## Native tiers at scaled inputs (asm vs typed JIT)\n")
print("Same kernels with the `;; scaled:` input (see each kernel file). "
      "Ratio = net(asm) / net(rust-jit): >1 means the typed JIT is faster.\n")
print("| kernel | rust-jit-scaled | asm-scaled | JIT speedup over asm |")
print("|---|---:|---:|---:|")
for k in kernels:
    if (k, "asm-scaled") not in cells:
        continue
    nj, fj = net(k, "rust-jit-scaled")
    na, fa = net(k, "asm-scaled")
    rj, ra = cells[(k, "rust-jit-scaled")], cells[(k, "asm-scaled")]
    print(f"| {k} | {'~' if fj else ''}{nj:.3f} s ({rj['median']:.3f}, {rj['min']:.3f}) "
          f"| {'~' if fa else ''}{na:.3f} s ({ra['median']:.3f}, {ra['min']:.3f}) | **{na/nj:.1f}×** |")

print("\n## Output agreement\n")
bad = []
for k in kernels:
    for group in (IMPLS, SCALED):
        vals = {i: v for i, v in outputs[k].items() if v != "N/A" and i in group}
        if len(set(vals.values())) > 1:
            bad.append((k, vals))
if not bad:
    print("All implementations printed identical results for every kernel.")
else:
    for k, vals in bad:
        print(f"- **{k}**: " + ", ".join(f"{i}={v}" for i, v in vals.items()))
