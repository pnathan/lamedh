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

CORE_SRCS="src/heap.asm src/print.asm src/reader.asm src/symtab.asm src/strings.asm src/floats.asm src/fileio.asm src/arrays.asm src/conditions.asm src/overflow.asm src/native_errors.asm src/chars.asm src/rng.asm src/bitwise.asm src/codegen.asm src/compiler.asm"

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
8
16
T
(1 2 3)
(A 5 7 8 9 B)
(A (QUASIQUOTE (B 3)))
T'
    got_out=$("$runner_bin" "$prelude_prog")
    got_exit=$?
    if [ "$got_out" = "$want_out" ] && [ "$got_exit" = "0" ]; then
        echo "ok    file_runner_prelude"
        pass=$((pass+1))
    else
        echo "FAIL  file_runner_prelude  stdout: got [$got_out] want [$want_out] exit: got $got_exit"
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
