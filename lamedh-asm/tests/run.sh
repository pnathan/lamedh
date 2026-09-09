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

CORE_SRCS="src/heap.asm src/print.asm src/reader.asm src/symtab.asm src/strings.asm src/floats.asm src/fileio.asm src/arrays.asm src/conditions.asm src/codegen.asm src/compiler.asm"

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
