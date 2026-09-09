#!/usr/bin/env bash
# tests/run.sh — assemble+link+run every tests/cases/*.asm, diff stdout and
# exit code against its .expected / .exitcode siblings.
#
# A case file defines `global lamedh_main`; it is linked against boot.asm
# and the shared core objects to produce one freestanding ELF binary.
set -u
cd "$(dirname "${BASH_SOURCE[0]}")/.."

BUILD=build
mkdir -p "$BUILD"

AS=nasm
ASFLAGS="-f elf64 -g -F dwarf -w+all -Isrc/"
LD=ld

CORE_SRCS="src/heap.asm src/print.asm src/reader.asm src/symtab.asm src/strings.asm src/floats.asm src/fileio.asm src/arrays.asm src/conditions.asm src/overflow.asm src/native_errors.asm src/chars.asm src/rng.asm src/bitwise.asm src/capabilities.asm src/modules.asm src/ports.asm src/codegen.asm src/compiler.asm"

pass=0
fail=0

echo "== assembling shared core =="
core_objs=""
for f in src/boot.asm $CORE_SRCS; do
    obj="$BUILD/$(basename "${f%.asm}").o"
    if ! $AS $ASFLAGS "$f" -o "$obj" 2>"$BUILD/asm.err"; then
        echo "FAIL  core asm: $f"
        cat "$BUILD/asm.err"
        exit 1
    fi
    core_objs="$core_objs $obj"
done

echo "== file_runner (lamedhc) =="
runner_obj="$BUILD/file_runner.o"
runner_bin="$BUILD/lamedhc"
if ! $AS $ASFLAGS src/file_runner.asm -o "$runner_obj" 2>"$BUILD/asm.err"; then
    echo "FAIL  file_runner  (assemble)"
    cat "$BUILD/asm.err"
    fail=$((fail+1))
elif ! $LD -static -nostdlib -o "$runner_bin" $core_objs "$runner_obj" 2>"$BUILD/ld.err"; then
    echo "FAIL  file_runner  (link)"
    cat "$BUILD/ld.err"
    fail=$((fail+1))
else
    tmp_prog="$BUILD/file_runner_case.lisp"
    printf '(DEFINE SQUARE (LAMBDA (X) (* X X)))\n(PRINT (SQUARE 7))\n(PRINT (QUOTE OK))\n' > "$tmp_prog"

    got_out=$("$runner_bin" "$tmp_prog")
    got_exit=$?
    if [ "$got_out" = "49OK" ] && [ "$got_exit" = "0" ]; then
        echo "ok    file_runner_file_arg"
        pass=$((pass+1))
    else
        echo "FAIL  file_runner_file_arg  stdout: got [$got_out] exit: got $got_exit"
        fail=$((fail+1))
    fi

    got_out=$("$runner_bin" < "$tmp_prog")
    got_exit=$?
    if [ "$got_out" = "49OK" ] && [ "$got_exit" = "0" ]; then
        echo "ok    file_runner_stdin"
        pass=$((pass+1))
    else
        echo "FAIL  file_runner_stdin  stdout: got [$got_out] exit: got $got_exit"
        fail=$((fail+1))
    fi

    got_exit=0
    "$runner_bin" "$BUILD/file_runner_does_not_exist.lisp" >/dev/null 2>&1 || got_exit=$?
    if [ "$got_exit" = "1" ]; then
        echo "ok    file_runner_missing_file"
        pass=$((pass+1))
    else
        echo "FAIL  file_runner_missing_file  exit: got $got_exit want 1"
        fail=$((fail+1))
    fi

    # lib/prelude.lisp, loaded automatically before this file: DEFUN,
    # NOT, WHEN, UNLESS, LIST, and the bare symbol T evaluated as a
    # variable (bootstrap_globals, symtab.asm) — every one of these
    # found a real bug the first time it was actually exercised this
    # way (see the commit history), so this is a real regression net,
    # not a formality. Also FORMAT/1+/APPEND/IOTA/REDUCE/DOTIMES/#' —
    # the exact surface examples/factorial/main.lisp needs, whose own
    # (dotimes ...) loop over (format t "~a! = ~a~%" ...) now runs
    # correctly end to end (only its final self-check still fails, on
    # 20! exceeding this kernel's 62-bit fixnum range — a documented
    # representational difference from the reference's 64-bit fixnums,
    # not a bug; see the README).
    prelude_prog="$BUILD/file_runner_prelude_case.lisp"
    cat > "$prelude_prog" <<'LISP'
(PRINT T)
(NEWLINE)
(PRINT (NOT (QUOTE ())))
(NEWLINE)
(PRINT (NOT 1))
(NEWLINE)
(DEFUN SQUARE2 (X) (* X X))
(PRINT (SQUARE2 6))
(NEWLINE)
(WHEN T (PRINT (QUOTE WHEN-TRUE)))
(NEWLINE)
(WHEN (QUOTE ()) (PRINT (QUOTE WHEN-FALSE-UNREACHABLE)))
(UNLESS (QUOTE ()) (PRINT (QUOTE UNLESS-TRUE)))
(NEWLINE)
(PRINT (LIST 1 2 3))
(NEWLINE)
(PRINT (APPEND (LIST 1 2) (LIST 3 4)))
(NEWLINE)
(PRINT (IOTA 5 1))
(NEWLINE)
(PRINT (1+ 41))
(NEWLINE)
(PRINT (1- 41))
(NEWLINE)
(PRINT (REDUCE #'* (IOTA 5 1) 1))
(NEWLINE)
(DOTIMES (I 3) (FORMAT T "i=~a~%" I))
(PRINT (GETP (QUOTE ZORP) (QUOTE COLOR)))
(NEWLINE)
(PUTP (QUOTE ZORP) (QUOTE COLOR) (QUOTE RED))
(PUTP (QUOTE ZORP) (QUOTE COLOR) (QUOTE BLUE))
(PRINT (GETP (QUOTE ZORP) (QUOTE COLOR)))
(NEWLINE)
(PRINT (EQUAL (MAPCAR NUMBER->STRING (LIST 1 2 3)) (LIST "1" "2" "3")))
(NEWLINE)
(PRINT (RPLACA (CONS 1 2) 9))
(NEWLINE)
(PRINT (RPLACD (CONS 1 2) 9))
(NEWLINE)
(DEFINE PAIR (CONS 1 2))
(RPLACA PAIR 9)
(PRINT PAIR)
(NEWLINE)
(DEF $ANSWER 42)
(PRINT $ANSWER)
(NEWLINE)
(PRINT (APPLY #'+ (LIST 3 4)))
(NEWLINE)
(PRINT (FUNCALL #'LIST 1 2 3 4 5))
(NEWLINE)
(PRINT (LIST (> 5 3) (> 3 5) (>= 5 5) (<= 4 5) (MAX 3 9) (MIN 3 9)))
(NEWLINE)
(FOR-EACH (LAMBDA (X) (PRINT X)) (LIST 1 2 3))
(NEWLINE)
(PRINT (FILTER (LAMBDA (X) (> X 2)) (LIST 1 2 3 4)))
(NEWLINE)
(PRINT (LIST (SOME (LAMBDA (X) (> X 3)) (LIST 1 2 3)) (EVERY (LAMBDA (X) (> X 0)) (LIST 1 2 3))))
(NEWLINE)
(DEFINE HT (MAKE-HASH-TABLE))
(SETHASH HT (QUOTE A) 1)
(SETHASH HT (QUOTE B) 2)
(SETHASH HT (QUOTE A) 99)
(PRINT (LIST (GETHASH HT (QUOTE A)) (GETHASH HT (QUOTE B)) (GETHASH HT (QUOTE C))))
(NEWLINE)
(SETHASH HT "STR-KEY" 42)
(PRINT (GETHASH HT (STRING-APPEND "STR-" "KEY")))
(NEWLINE)
(SETHASH HT 0.0 111)
(PRINT (GETHASH HT -0.0))
(NEWLINE)
(PRINT (REMHASH HT (QUOTE A)))
(NEWLINE)
(PRINT (GETHASH HT (QUOTE A)))
(NEWLINE)
(PRINT (EQUAL (KEYS HT) (KEYS HT)))
(NEWLINE)
(SET-BANG HT (QUOTE D) 7)
(PRINT (GETHASH HT (QUOTE D)))
(NEWLINE)
(PRINT (LOGAND 12 10))
(NEWLINE)
(PRINT (ASH 1 4))
(NEWLINE)
(RANDOM-SEED! 7)
(PRINT (< (RANDOM 1000000) 1000000))
(NEWLINE)
(PRINT `(1 2 3))
(NEWLINE)
(DEFINE QQ-X 5)
(PRINT `(A ,QQ-X ,@(LIST 7 8 9) B))
(NEWLINE)
(PRINT `(A `(B ,(+ 1 2))))
(NEWLINE)
(PRINT (EQ (QUOTE Z) `Z))
(NEWLINE)
(FOR (I 1 3) (PRINT I))
(NEWLINE)
(FOR (I 3 1 -1) (PRINT I))
(NEWLINE)
(PRINT (FOR (I 1 3) (PRINT I)))
(NEWLINE)
(PRINT (HANDLER-CASE (FOR (I 1 5 0) (PRINT I)) (E (X) (QUOTE CAUGHT))))
(NEWLINE)
(PRINT (FEATURE-ENABLED-P (QUOTE READ-FS)))
(NEWLINE)
(WITH-CAPABILITIES (QUOTE (READ-FS)) (PRINT (FEATURE-ENABLED-P (QUOTE READ-FS))))
(NEWLINE)
(WITH-CAPABILITIES (QUOTE (READ-FS)) (PRINT (FEATURE-ENABLED-P (QUOTE CREATE-FS))))
(NEWLINE)
(PRINT (FEATURE-ENABLED-P (QUOTE CREATE-FS)))
(NEWLINE)
(WITH-CAPABILITIES (QUOTE (READ-FS)) (WITH-CAPABILITIES (QUOTE (SHELL)) (PRINT (FEATURE-ENABLED-P (QUOTE READ-FS)))))
(NEWLINE)
(CATCH (QUOTE TAG) (WITH-CAPABILITIES (QUOTE (READ-FS)) (THROW (QUOTE TAG) 0)))
(PRINT (FEATURE-ENABLED-P (QUOTE SHELL)))
(NEWLINE)
(PRINT (HANDLER-CASE (WITH-CAPABILITIES (QUOTE (SHELL)) (FD-OPEN (QUOTE X) 0)) (E (X) (QUOTE CAUGHT))))
(NEWLINE)
(PRINT (PROG (I) (SETQ I 0) LOOP (WHEN (= I 5) (RETURN I)) (PRINT I) (SETQ I (+ I 1)) (GO LOOP)))
(NEWLINE)
(PRINT (PROG (X) (SETQ X 42) X))
(NEWLINE)
(PRINT (PROG (I ACC) (SETQ I 1) (SETQ ACC 0) LOOP (WHEN (> I 5) (RETURN ACC)) (SETQ ACC (+ ACC I)) (SETQ I (+ I 1)) (GO LOOP)))
(NEWLINE)
(PRINT (PROG (X) (GO SKIP) (SETQ X 1) SKIP (SETQ X 2) (RETURN X)))
(NEWLINE)
(PRINT (PROG (I) (SETQ I 0) OUTER (WHEN (= I 3) (RETURN I)) (PROG (J) (SETQ J 0) INNER (WHEN (= J 2) (RETURN 0)) (SETQ J (+ J 1)) (GO INNER)) (SETQ I (+ I 1)) (GO OUTER)))
(NEWLINE)
(DEF $DOCUMENTED-GLOBAL 7 "a docstring")
(PRINT $DOCUMENTED-GLOBAL)
(NEWLINE)
(PRINT (GETP (QUOTE $DOCUMENTED-GLOBAL) "docstring"))
(NEWLINE)
(PUTP (QUOTE PROPTEST) "a" 1)
(PUTP (QUOTE PROPTEST) "b" 2)
(REMPROP (QUOTE PROPTEST) "a")
(PRINT (LIST (GETP (QUOTE PROPTEST) "a") (GETP (QUOTE PROPTEST) "b")))
(NEWLINE)
(PRINT (LIST NIL (EQ NIL (QUOTE ())) (IF NIL 1 2) (IF NIL 1 NIL)))
(NEWLINE)
(DEFMACRO DOC-MACRO (X)
  "a docstring, followed by a second body form — a single-body-form-only
DEFMACRO would silently keep only this string and discard the real
template below, the exact shape lib/02-cxr.lisp's own `defcxr` macro
uses to build CADR/CADDR/etc."
  (LIST (QUOTE QUOTE) (LIST (QUOTE EXPANDED) X)))
(PRINT (DOC-MACRO 5))
(NEWLINE)
(PRINT (ASSOC (QUOTE B) (LIST (CONS (QUOTE A) 1) (CONS (QUOTE B) 2))))
(NEWLINE)
(PRINT (ASSOC (QUOTE Z) (LIST (CONS (QUOTE A) 1))))
(NEWLINE)
(PRINT (CONCAT "A" "B" "C"))
LISP
    want_out='T
T
()
36
WHEN-TRUE
UNLESS-TRUE
(1 2 3)
(1 2 3 4)
(1 2 3 4 5)
42
40
120
i=0
i=1
i=2
()
BLUE
T
(9 . 2)
(1 . 9)
(1 . 2)
42
7
(1 2 3 4 5)
(T () T T 9 3)
123
(3 4)
(() T)
(99 2 ())
42
111
T
()
T
7
8
16
T
(1 2 3)
(A 5 7 8 9 B)
(A (QUASIQUOTE (B 3)))
T
123
321
123()
CAUGHT
T
T
()
T
()
T
CAUGHT
012345
()
15
2
3
7
a docstring
(() 2)
(() T 2 ())
(EXPANDED 5)
(B . 2)
()
ABC'
    got_out=$("$runner_bin" "$prelude_prog")
    got_exit=$?
    if [ "$got_out" = "$want_out" ] && [ "$got_exit" = "0" ]; then
        echo "ok    file_runner_prelude"
        pass=$((pass+1))
    else
        echo "FAIL  file_runner_prelude  stdout: got [$got_out] want [$want_out] exit: got $got_exit"
        fail=$((fail+1))
    fi

    # stdlib_conformance — the actual conformance target this project
    # tracks in README's "KERNEL.md conformance" section: every one of
    # the reference's own 33 non-OS-dependent STDLIB_SOURCES files
    # (../lib/*.lisp, the Rust reference implementation's own stdlib,
    # untouched), concatenated in src/lib.rs's own STDLIB_SOURCES load
    # order — 00-core through 99-help-data, skipping only the
    # genuinely file-descriptor/socket/TLS/regex-dependent tier
    # (31-ports through 44-regex) this freestanding, no-libc host does
    # not implement — loaded as one buffer through lamedhc, exactly
    # the way Environment::with_stdlib() loads them unconditionally in
    # the reference. This is a stronger check than any single
    # accumulated-file bisection this project's own commit history
    # describes: it catches cross-file interaction bugs a single
    # file's own isolated load-test cannot (this exact test caught
    # none at the time it was added, but is here so a regression would
    # be caught here rather than being separately rediscovered).
    stdlib_files="00-core 01-list 02-cxr 03-meta 04-predicates 05-math 06-require 08-vau 12-control 13-functional 14-strings 15-sets-hash 16-conditions 17-arrays 18-format 21-cl-compat 20-condensation 27-modules 11-optimizer-vau 19-call-graph 07-shell 09-lisp15 10-testing 22-guard 23-match 24-rules 25-variants 26-instrument 28-types 29-protocols 30-text 31-ports 32-base64 33-hex 34-url 35-json 36-mime 97-doc-renderer 98-help-system 99-help-data"
    stdlib_prog="$BUILD/stdlib_conformance.lisp"
    : > "$stdlib_prog"
    for f in $stdlib_files; do
        cat "../lib/$f.lisp" >> "$stdlib_prog"
        echo >> "$stdlib_prog"
    done
    cat >> "$stdlib_prog" <<'LISP'
(PRINT (LENGTH (LIST 1 2 3)))
(NEWLINE)
(PRINT (MAP (LAMBDA (X) (* X X)) (LIST 1 2 3)))
(NEWLINE)
(DEFVARIANT (OPTION A) (SOME (VALUE A)) (NONE))
(PRINT (SOME 42))
(NEWLINE)
(PRINT (GET-DOC (QUOTE +)))
(NEWLINE)
(PORTS:WITH-OPEN-PORT (P (PORTS:OPEN-OUTPUT-BYTES))
  (PORTS:WRITE-STRING! P "ports-ok"))
(DEFINE MP (PORTS:OPEN-INPUT-BYTES (TEXT:STRING->UTF8 "a
b")))
(PRINT (PORTS:READ-LINE! MP))
(PRINT (PORTS:READ-LINE! MP))
LISP
    stdlib_want='3
(1 4 9)
#S(SOME 42)
((NAME . +) (TYPE . FUNCTION) (SYNTAX . (+ number...)) (CATEGORY . ARITHMETIC) (DESCRIPTION . Returns the sum of all arguments. With no arguments, returns 0.) (ARGS (NUMBERS Zero or more numbers to add)) (RETURNS . Sum of arguments (float if any argument is float)) (EXAMPLES ((+ 1 2 3) 6) ((+ 1.500000 2.500000) 4.000000) ((+) 0)) (SEE-ALSO - * /))
ab'
    stdlib_got=$("$runner_bin" "$stdlib_prog")
    stdlib_exit=$?
    if [ "$stdlib_got" = "$stdlib_want" ] && [ "$stdlib_exit" = "0" ]; then
        echo "ok    stdlib_conformance"
        pass=$((pass+1))
    else
        echo "FAIL  stdlib_conformance  stdout: got [$stdlib_got] want [$stdlib_want] exit: got $stdlib_exit"
        fail=$((fail+1))
    fi
fi

for case_asm in tests/cases/*.asm; do
    name=$(basename "${case_asm%.asm}")
    expected="tests/cases/$name.expected"
    exitfile="tests/cases/$name.exitcode"
    want_exit=0
    [ -f "$exitfile" ] && want_exit=$(cat "$exitfile")
    want_out=""
    [ -f "$expected" ] && want_out=$(cat "$expected")

    obj="$BUILD/case_$name.o"
    bin="$BUILD/case_$name"

    if ! $AS $ASFLAGS "$case_asm" -o "$obj" 2>"$BUILD/asm.err"; then
        echo "FAIL  $name  (assemble)"
        cat "$BUILD/asm.err"
        fail=$((fail+1))
        continue
    fi

    if ! $LD -static -nostdlib -o "$bin" $core_objs "$obj" 2>"$BUILD/ld.err"; then
        echo "FAIL  $name  (link)"
        cat "$BUILD/ld.err"
        fail=$((fail+1))
        continue
    fi

    got_out=$("$bin")
    got_exit=$?

    ok=1
    if [ "$got_out" != "$want_out" ]; then
        ok=0
        echo "FAIL  $name  stdout: got [$got_out] want [$want_out]"
    fi
    if [ "$got_exit" != "$want_exit" ]; then
        ok=0
        echo "FAIL  $name  exit: got $got_exit want $want_exit"
    fi

    if [ "$ok" = 1 ]; then
        echo "ok    $name"
        pass=$((pass+1))
    else
        fail=$((fail+1))
    fi
done

echo "== $pass passed, $fail failed =="
[ "$fail" -eq 0 ]
