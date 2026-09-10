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
# EXTRA_ASFLAGS lets the whole suite be re-run under an %ifdef-gated
# assertion build, e.g.
#   EXTRA_ASFLAGS=-DCAPTURE_CHECK bash tests/run.sh
# which cross-checks every capture list the new analysis produces
# against the old scan_free_vars oracle and traps (int3, i.e. a case
# exiting on SIGTRAP) on a violation — docs/spec-tco-capture-gc.md
# section 1.4.
ASFLAGS="-f elf64 -g -F dwarf -w+all -Isrc/ ${EXTRA_ASFLAGS:-}"
LD=ld

CORE_SRCS="src/heap.asm src/gc.asm src/print.asm src/reader.asm src/symtab.asm src/strings.asm src/floats.asm src/fileio.asm src/arrays.asm src/conditions.asm src/overflow.asm src/native_errors.asm src/chars.asm src/rng.asm src/bitwise.asm src/capabilities.asm src/modules.asm src/ports.asm src/syscall.asm src/codegen.asm src/compiler.asm"

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
    got_err=$("$runner_bin" "$BUILD/file_runner_does_not_exist.lisp" 2>&1 >/dev/null) || got_exit=$?
    if [ "$got_exit" = "1" ] && [[ "$got_err" == *"cannot open input file"* ]]; then
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
(PRINT (LIST (> 5 3) (> 3 5) (>= 5 5) (<= 4 5) (MAX 3 9) (MIN 3 9) (MAX 1 3 9) (MIN 5 2 8 1 9)))
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
(NEWLINE)
; &OPTIONAL/&KEY parameter lists (DEFUN-level sugar, lib/prelude.lisp's
; $EXTENDED-LAMBDA) plus the keyword-self-evaluation compiler fix
; (compile_form) they depend on -- a bare :FOO must reach a callee as
; the tagged keyword symbol itself, not an evaluated (and previously
; always-unbound) variable reference.
(DEFUN OPT1 (A &OPTIONAL B) (LIST A B))
(PRINT (OPT1 1))
(NEWLINE)
(PRINT (OPT1 1 2))
(NEWLINE)
; later defaults may reference earlier parameters (LET* is sequential)
(DEFUN OPT2 (A &OPTIONAL (B 10) (C (+ B 1))) (LIST A B C))
(PRINT (OPT2 1))
(NEWLINE)
(PRINT (OPT2 1 2))
(NEWLINE)
(PRINT (OPT2 1 2 99))
(NEWLINE)
(DEFUN KEY1 (&KEY (D 2) E) (LIST D E))
(PRINT (KEY1))
(NEWLINE)
(PRINT (KEY1 :D 5))
(NEWLINE)
(PRINT (KEY1 :E 7))
(NEWLINE)
(PRINT (KEY1 :D 5 :E 7))
(NEWLINE)
; &OPTIONAL, &KEY and &REST together: &REST binds to the same raw
; remainder &KEY parses from, matching Common Lisp's own convention.
(DEFUN OPT-KEY-REST (A &OPTIONAL B &KEY (D 2) &REST R) (LIST A B D R))
(PRINT (OPT-KEY-REST 1))
(NEWLINE)
(PRINT (OPT-KEY-REST 1 2 (QUOTE :D) 9 (QUOTE :EXTRA) 1))
(NEWLINE)
; a keyword self-evaluates identically quoted or bare, and two
; references to the same keyword are EQ (one interned symbol, not a
; fresh one per occurrence).
(PRINT (LIST (EQ :FOO :FOO) (EQ :FOO (QUOTE :FOO))))
(NEWLINE)
(PRINT (GC-VERIFY))
(NEWLINE)
(GC-COLLECT)
(PRINT (GC-VERIFY))
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
(T () T T 9 3 9 1)
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
ABC
(1 ())
(1 2)
(1 10 11)
(1 2 3)
(1 2 99)
(2 ())
(5 ())
(2 7)
(5 7)
(1 () 2 ())
(1 2 9 (:D 9 :EXTRA 1))
(T T)
T
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

    # file_runner_errors — the user-facing failure contract. Each case
    # is one small program, checked for exit status, a stdout
    # substring and a stderr substring. Every one of these was a
    # silent wrong answer or a bare, message-less SIGTRAP/SIGSEGV
    # before the fix it pins: no arity checking at all (`(F 1)` against
    # a 2-parameter F returned (1 0)), `&OPTIONAL` in a bare LAMBDA
    # binding a parameter literally named &OPTIONAL, `(+ 1 2 3)` = 3,
    # an uncaught ERROR/THROW/undefined function exiting 133 with no
    # text, a 4+-argument tail loop or deep non-tail recursion dying
    # as "Segmentation fault", and a deep &REST recursion taking a
    # minute because the collector re-scanned the whole stack at every
    # call once its zero-count table filled with live entries.
    err_case() {
        local name=$1 want_exit=$2 want_out=$3 want_err=$4 prog=$5
        local f="$BUILD/err_$name.lisp" got_out got_err got_exit
        printf '%s\n' "$prog" > "$f"
        got_exit=0
        got_out=$("$runner_bin" "$f" 2>"$f.stderr" </dev/null) || got_exit=$?
        got_err=$(cat "$f.stderr")
        if [ "$got_exit" = "$want_exit" ] && [[ "$got_out" == *"$want_out"* ]] && [[ "$got_err" == *"$want_err"* ]]; then
            echo "ok    file_runner_errors/$name"
            pass=$((pass+1))
        else
            echo "FAIL  file_runner_errors/$name  exit: got $got_exit want $want_exit stdout: [$got_out] want *[$want_out]* stderr: [$got_err] want *[$want_err]*"
            fail=$((fail+1))
        fi
    }
    err_case undefined_function 1 "1" "lamedhc: unhandled error: not a function: LENGTH" \
        '(PRINT 1) (LENGTH (LIST 1 2))'
    err_case arity_too_few 1 "" "wrong number of arguments (got . expected): (1 . 2)" \
        '(DEFUN F2 (A B) (LIST A B)) (PRINT (F2 1))'
    err_case arity_too_many 1 "" "wrong number of arguments (got . expected): (3 . 1)" \
        '(DEFUN F1 (A) A) (PRINT (F1 1 2 3))'
    err_case arity_rest_too_few 1 "" "too few arguments (got . minimum): (0 . 1)" \
        '(DEFUN R (A &REST XS) XS) (PRINT (R))'
    err_case arity_indirect 1 "" "wrong number of arguments (got . expected): (1 . 2)" \
        '(DEFINE F (LAMBDA (A B) A)) (PRINT ((LAMBDA (G) (G 1)) F))'
    err_case arity_macro 1 "" "wrong number of arguments (got . expected): (1 . 2)" \
        '(DEFMACRO TWO (A B) (LIST (QUOTE LIST) A B)) (PRINT (TWO 1))'
    err_case arity_caught 0 "(CAUGHT wrong number of arguments (got . expected) (1 . 2))" "" \
        '(DEFUN F2 (A B) A) (PRINT (HANDLER-CASE (F2 1) (ERROR (E) (LIST (QUOTE CAUGHT) (ERROR-MESSAGE E) (ERROR-DATA E)))))'
    err_case lambda_optional 1 "" "unsupported lambda-list keyword" \
        '(DEFINE F (LAMBDA (A &OPTIONAL B) (LIST A B))) (PRINT (F 1 2))'
    err_case defmacro_optional 1 "" "unsupported lambda-list keyword" \
        '(DEFMACRO M (A &OPTIONAL B) (LIST (QUOTE LIST) A B)) (PRINT (M 1 2))'
    err_case rest_malformed 1 "" "&REST must be followed by exactly one parameter name" \
        '(DEFINE F (LAMBDA (A &REST) A)) (PRINT (F 1))'
    err_case param_not_symbol 1 "" "LAMBDA: parameter is not a symbol" \
        '(DEFINE F (LAMBDA ((A 1)) A)) (PRINT (F 1))'
    err_case supplied_p 1 "" "supplied-p variables are not supported" \
        '(DEFUN F (A &OPTIONAL (B 10 B-P)) (LIST A B B-P)) (PRINT (F 1))'
    err_case uncaught_error 1 "" "lamedhc: unhandled error: boom: (1 2)" \
        '(ERROR "boom" (LIST 1 2))'
    err_case uncaught_throw 1 "" "lamedhc: unhandled THROW to MYTAG: 42" \
        '(THROW (QUOTE MYTAG) 42)'
    err_case car_of_fixnum 1 "" "CAR: expected a cons or NIL: 5" \
        '(PRINT (CAR 5))'
    err_case variadic_arith 0 "(6 7 24 T () T 0 1 5 -5 T)" "" \
        '(PRINT (LIST (+ 1 2 3) (- 10 1 2) (* 2 3 4) (< 1 2 3) (< 1 3 2) (= 1 1 1) (+) (*) (+ 5) (- 5) (< 7)))'
    err_case variadic_once 0 "(T 3)" "" \
        '(DEFINE N 0) (DEFUN BUMP () (SETQ N (+ N 1)) N) (DEFINE R (< (BUMP) (BUMP) (BUMP))) (PRINT (LIST R N))'
    err_case let_in_argument_position 0 "(3 10 20 30 40 50)" "" \
        '(PRINT (LIST (LET ((A 1) (B 2)) (+ A B)) 10 20 30 40 50))'
    err_case let_star_prog_in_argument_position 0 "(7 10 20 30 40 50 7)" "" \
        '(DEFUN F7 (A B C D E F G) (LIST A B C D E F G)) (PRINT (F7 (LET* ((X 7) (Y X)) Y) 10 20 30 40 50 (PROG (A) (SETQ A 7) (RETURN A))))'
    err_case let_as_binop_lhs 0 "6" "" \
        '(PRINT (+ (LET ((A 1)) A) 5))'
    err_case nested_let_in_init 0 "(1 2)" "" \
        '(PRINT (LET ((A 1) (B (LET ((C 2)) C))) (LIST A B)))'
    err_case handler_var_in_argument_position 0 "(boom 10 20 30 40 50)" "" \
        '(DEFUN F6 (A B C D E F) (LIST A B C D E F)) (PRINT (F6 (HANDLER-CASE (ERROR "boom" 1) (ERROR (E) (ERROR-MESSAGE E))) 10 20 30 40 50))'
    err_case variadic_minus_none 1 "" "requires at least one operand: -" \
        '(PRINT (-))'
    err_case tail_4args_named 0 "(1 2 3)" "" \
        '(DEFUN L4 (A B C N) (IF (EQ N 0) (LIST A B C) (L4 A B C (- N 1)))) (PRINT (L4 1 2 3 1000000))'
    err_case tail_5args_indirect 0 "(1 2 3 4)" "" \
        '(DEFUN L5 (F A B C D N) (IF (EQ N 0) (LIST A B C D) (F F A B C D (- N 1)))) (PRINT (L5 L5 1 2 3 4 1000000))'
    err_case tail_4args_in_let 0 "6" "" \
        '(DEFUN L4 (A B C N) (LET ((M (- N 1))) (IF (EQ N 0) (+ A B C) (L4 A B C M)))) (PRINT (L4 1 2 3 1000000))'
    err_case tail_5_to_4_args 0 "(1 2 3)" "" \
        '(DEFUN G4 (A B C N) (IF (EQ N 0) (LIST A B C) (G4 A B C (- N 1)))) (DEFUN H5 (A B C D N) (G4 A B C N)) (PRINT (H5 1 2 3 4 1000000))'
    err_case stack_overflow_reported 139 "" "lamedhc: fatal: SIGSEGV" \
        '(DEFUN S (N) (IF (EQ N 0) 0 (+ 1 (S (- N 1))))) (PRINT (S 50000000))'
    err_case deep_rest_recursion_fast 0 "(1 2 3 4)" "" \
        '(DEFUN R (N &REST XS) (IF (EQ N 0) XS (R (- N 1) 1 2 3 4))) (PRINT (R 100000))'
    err_case math_library 0 "(4.000000 1.500000 2 -3 3 3 -3 2 -2 1.000000 0.000000 3.000000 0.000000 1.000000 0.000000 2 1 1 -2305843009213693952 2)" "" \
        '(PRINT (LIST (SQRT 16) (SQRT 2.25) (FLOOR 2.5) (FLOOR -2.5) (CEILING 2.1) (ROUND 2.5) (ROUND -2.5) (ROUND 2.4) (TRUNCATE -2.7) (EXP 0) (LOG 1) (LOG 8 2) (SIN 0) (COS 0) (TAN 0) (ROT 1 1) (ROT 2 -1) (ROT 1 62) (ROT 1 -1) (ROT (ROT 2 -1) 1)))'
    err_case math_as_values 0 "(4.000000 3 (1 4 9))" "" \
        '(PRINT (LIST (FUNCALL (FUNCTION SQRT) 16) (APPLY (FUNCTION ROUND) (LIST 2.5)) (MAPCAR (LAMBDA (X) (TRUNCATE (SQRT (* X X X X)))) (LIST 1 2 3))))'
    err_case math_type_error 1 "" "expected a number (fixnum or float): x" \
        '(PRINT (SQRT "x"))'
    err_case exp_log_roundtrip 0 "(2.718281 1.000000 7.389056 20.085536)" "" \
        '(PRINT (LIST (EXP 1) (LOG (EXP 1)) (EXP 2) (EXP 3)))'
    err_case read_eof 1 "" "READ: end of input" \
        '(PRINT (READ))'
    err_case write_stderr 0 "out" "to stderr" \
        '(WRITE-STRING "out") (WRITE-LINE "to stderr" *STDERR*) (PRINT-TO *STDERR* (LIST 1 2))'
    err_case fd_read_line_eof 0 "()" "" \
        '(PRINT (FD-READ-LINE *STDIN*))'
    err_case fd_write_bad_fd 1 "" "FD-WRITE: write failed (data: -errno): -9" \
        '(FD-WRITE 999 "x")'
    err_case syscall_raw 0 "(T 3 -2 T)" "" \
        '(PRINT (LIST (< 0 (SYSCALL SYS-GETPID)) (SYSCALL SYS-WRITE 1 "abc" 3) (SYSCALL SYS-OPEN "/nonexistent/x" 0 0) (EQ (SYSCALL 1 1 "hello" 5) 5)))'
    err_case syscall_bad_arg 1 "" "SYSCALL: argument must be a fixnum, string, NIL, or list of strings: 1.500000" \
        '(SYSCALL 39 1.5)'
    err_case file_p 0 "(T () ())" "" \
        '(PRINT (LIST (FILE-P "/etc/passwd") (FILE-P "/etc") (FILE-P "/nonexistent")))'
    # SHELL returns (code stdout stderr) like the reference; printed
    # readably so the strings show their quotes and newlines. A
    # one-argument SHELL goes through sh -c (so a missing program is
    # sh's own complaint on stderr, code 127); the direct form execs
    # the program itself (a missing one: exit 127, nothing written).
    err_case chmod_and_shell 0 '((0 "hello\n" "") (0 "a b\n" "") (3 "" "to err\n") 127 "" T)' "" \
        '(PRINT (PRIN1-TO-STRING (LIST (SHELL "echo hello") (SHELL "/bin/echo" "a" "b") (SHELL "echo to err >&2; exit 3") (CAR (SHELL "/nonexistent/prog" "x")) (CAR (CDR (SHELL "/nonexistent/prog" "x"))) (PROGN (FD-CLOSE (FD-OPEN "/tmp/lamedh-chmod-test" 1)) (CHMOD "/tmp/lamedh-chmod-test" "600") (CHMOD "/tmp/lamedh-chmod-test" 420)))))'
    # The stream layer against a real stdin: READ twice, then READ-LINE
    # for the rest (a final unterminated line comes back once), then
    # NIL at end of input.
    printf '(1 2 3)\nfoo bar\nlast line\ntail' > "$BUILD/err_read_stdin.txt"
    printf '(PRINT (READ))\n(NEWLINE)\n(PRINT (READ))\n(NEWLINE)\n(PRINT (READ-LINE))\n(NEWLINE)\n(PRINT (READ-LINE))\n(NEWLINE)\n(PRINT (READ-LINE))\n' > "$BUILD/err_read_stdin.lisp"
    got_exit=0
    got_out=$("$runner_bin" "$BUILD/err_read_stdin.lisp" < "$BUILD/err_read_stdin.txt" 2>/dev/null) || got_exit=$?
    if [ "$got_exit" = "0" ] && [ "$got_out" = "$(printf '(1 2 3)\nFOO\nlast line\ntail\n()')" ]; then
        echo "ok    file_runner_errors/read_stdin"
        pass=$((pass+1))
    else
        echo "FAIL  file_runner_errors/read_stdin  exit: got $got_exit stdout: [$got_out]"
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
(NEWLINE)
; GC-VERIFY recomputes every unpinned object's reference count from a
; full linear heap walk and compares it with the side table. Running it
; here, after loading 40 reference stdlib files — which exercise every
; creation and mutation site the reference library actually uses — is
; what turns "did I instrument every site?" into a test: a missed
; rc_inc anywhere shows up as a count mismatch right here instead of
; as a corrupted list three programs later. Once before any collection
; (pure bookkeeping) and once after a real one.
(PRINT (GC-VERIFY))
(NEWLINE)
(GC-COLLECT)
(PRINT (GC-VERIFY))
(NEWLINE)
; The codec/text tier of the reference stdlib, end to end through the
; reference's own DEFUN (00-core's, with its &KEY parameter lists,
; JIT-OPTIMIZE hook and docstrings), the protocol dispatchers
; (LENGTH twice from one site: a named call to a global closure WITH
; captured variables, which the old inline cache broke on the second
; call), PRIN1-TO-STRING, and a float through PRINC-TO-STRING's
; capture buffer.
(PRINT (BASE64:ENCODE (TEXT:STRING->UTF8 "hi")))
(NEWLINE)
(PRINT (TEXT:UTF8->STRING (BASE64:DECODE "aGk=")))
(NEWLINE)
(PRINT (HEX:ENCODE (TEXT:STRING->UTF8 "hi")))
(NEWLINE)
(PRINT (TEXT:UTF8->STRING (HEX:DECODE "6869")))
(NEWLINE)
(PRINT (URL:ENCODE-QUERY-COMPONENT "a b&c"))
(NEWLINE)
(PRINT (URL:DECODE "a%20b%26c"))
(NEWLINE)
(PRINT (MIME:PARSE-CONTENT-TYPE "text/html; charset=utf-8"))
(NEWLINE)
(PRINT (JSON:STRINGIFY (JSON:PARSE "{\"k\": [1, 2.5, \"s\", true, null]}")))
(NEWLINE)
(PRINT (LIST (LENGTH (LIST 1 2)) (LENGTH (LIST 1 2 3)) (PRIN1-TO-STRING "a\"b") (SORT (LIST 3 1 2) <) (PRINC-TO-STRING 2.5)))
LISP
    stdlib_want='3
(1 4 9)
#S(SOME 42)
((NAME . +) (TYPE . FUNCTION) (SYNTAX . (+ number...)) (CATEGORY . ARITHMETIC) (DESCRIPTION . Returns the sum of all arguments. With no arguments, returns 0.) (ARGS (NUMBERS Zero or more numbers to add)) (RETURNS . Sum of arguments (float if any argument is float)) (EXAMPLES ((+ 1 2 3) 6) ((+ 1.500000 2.500000) 4.000000) ((+) 0)) (SEE-ALSO - * /))
ab
T
T
aGk=
hi
6869
hi
a%20b%26c
a b&c
((TYPE . text) (SUBTYPE . html) (PARAMETERS (charset . utf-8)))
{"k":[1,2.500000,"s",true,null]}
(2 3 "a\"b" (1 2 3) 2.500000)'
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
