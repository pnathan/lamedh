#!/usr/bin/env python3
"""Inventory of the builtins that lib/*.lisp calls (KERNEL.md Part XIII).

Method: read every operator-position symbol and #'name reference in
lib/[0-9]*.lisp, ask the Rust reference (a booted world) which of them are
bound to a Rust builtin, then tier each builtin.  Usage:

    cargo build --release && scripts/kernel-inventory.py [> docs/kernel-inventory.tsv]

Tiers: K kernel · L Lisp-definable · A accelerator (Lisp-definable, native
override wanted) · U Unicode tables · R reflection (reference-specific) ·
H-io / H-net / H-regex / H-conc host-auxiliary libraries.
"""
import collections, glob, os, re, subprocess, sys, tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LAMEDH = os.path.join(ROOT, "target/release/lamedh")

def strip(src):
    out, i, n = [], 0, len(src)
    while i < n:
        c = src[i]
        if c == ";":
            while i < n and src[i] != "\n": i += 1
        elif c == '"':
            i += 1
            while i < n and src[i] != '"': i += 2 if src[i] == "\\" else 1
            i += 1; out.append(" ")
        elif src.startswith("#\\", i):
            i += 3
            while i < n and src[i] not in " ()\n\t": i += 1
            out.append(" ")
        else:
            out.append(c); i += 1
    return "".join(out)

def operators():
    use = collections.defaultdict(collections.Counter)
    for path in sorted(glob.glob(os.path.join(ROOT, "lib/[0-9]*.lisp"))):
        text, base = strip(open(path).read()), os.path.basename(path)[:-5]
        for m in re.finditer(r"\(\s*([^\s()'`,\"#;]+)", text): use[m.group(1).upper()][base] += 1
        for m in re.finditer(r"#'([^\s()]+)", text): use[m.group(1).upper()][base] += 1
    ok = re.compile(r"^[A-Z*+/<>=!?%~^_-][A-Z0-9*+/<>=!?%~^_:-]*$")
    return {n: u for n, u in use.items() if ok.match(n) and n not in ("NIL", "T")
            and not re.fullmatch(r"[+-]?\d.*", n)}

def builtins(names):
    kinds, todo = {}, sorted(names)
    while todo:
        prog = "\n".join(f'(print (list "{n}" (if (boundp (quote {n})) (prin1-to-string {n}) "-")))' for n in todo)
        with tempfile.NamedTemporaryFile("w", suffix=".lisp", delete=False) as f: f.write(prog)
        p = subprocess.run([LAMEDH, f.name], capture_output=True, text=True)
        os.unlink(f.name)
        for line in p.stdout.splitlines():
            m = re.match(r'\("(.*)" "(.*)"\)$', line)
            if m: kinds[m.group(1)] = m.group(2)
        todo = [n for n in todo if n not in kinds]
        if p.returncode == 0 or not todo: break
        kinds[todo.pop(0)] = "?"          # a name the probe could not evaluate
    return sorted(n for n, k in kinds.items() if k == "<builtin>")

W = lambda s: set(s.split())
LISP = W("add1 sub1 plus times difference quotient greaterp lessp append assoc nth nthcdr last list mapcar maplist not delete subst sublis explode implode maknam evenp oddp plusp zerop signum gcd lcm isqrt rot leftshift float-equal float-greaterp float-lessp equal-number array-fetch* array-store* deflist efface get put plist set-bang evlis evcon spaces index rplaca rplacd sexpr-rename")
ACCEL = W("string->list* string-split* string-join* string->utf8* utf8->string* utf8->string-lossy* sort array-add! array-sub! array-mul! array-div!")
UNICODE = W("char-alphabetic-p* char-lowercase-p* char-numeric-p* char-uppercase-p* string-casefold* string-downcase* string-upcase*")
REFLECT = W("describe signature see-source see-type why-not-typed optimize compiled-p disassemble")
HOSTIO = W("chmod create-directory delete-file directory-files directory-p file-executable-p file-exists-p file-newer-p file-p file-readable-p file-size file-writable-p make-temp-directory make-temp-file rename-file read-file write-file read-file-byte read-file-section read-file-section-bytes read-file-section-lossy load-file read-all-positioned shell monotonic-micros random")

def tier(n):
    if n in LISP: return "L"
    if n in ACCEL: return "A"
    if n in UNICODE: return "U"
    if n in REFLECT: return "R"
    if n.startswith("regex-"): return "H-regex"
    if n in ("spawn-thread", "channel-recv"): return "H-conc"
    if re.match(r"(tcp-|udp-|tls-|net-)", n): return "H-net"
    if n in HOSTIO or re.match(r"(port-|os-)", n): return "H-io"
    return "K"

if __name__ == "__main__":
    use = operators()
    rows = [(n.lower(), tier(n.lower()), sum(use[n].values()), len(use[n])) for n in builtins(use)]
    print("name\ttier\tuses\tfiles")
    for n, t, u, f in sorted(rows, key=lambda r: (r[1], -r[2], r[0])): print(f"{n}\t{t}\t{u}\t{f}")
    print(dict(sorted(collections.Counter(t for _, t, _, _ in rows).items())), file=sys.stderr)
