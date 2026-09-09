; 034_not_callable — calling a non-callable value is a controlled
; failure (native_errors.asm's fail_not_callable: a message to stderr
; and exit(1)), not undefined behavior. KERNEL.md Part VIII lists this
; among the native-failure classes a host must signal for; before
; emit_check_callable existed, both compile_call paths blindly
; dereferenced whatever tagged value they were given as if it were a
; closure — an unbound global defaults to IMM_UNBOUND, whose tag bits
; mask to a near-NULL pointer, so calling one segfaulted rather than
; failing in any way a caller (or this test) could observe. This case
; is the named-global inline-cache path (a global that was never
; DEFINEd); 035_not_callable_indirect.asm is the other compile_call
; path (a lexically bound local holding a non-closure value).
;
; This is not yet a HANDLER-CASE-catchable condition (see README) —
; there is no way to test *that* here yet — only that the process
; fails deterministically (exit 1, tests/run.sh's `.exitcode` file)
; instead of crashing on undefined memory.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(PRINT (QUOTE BEFORE))"
d1_len: equ $ - d1
; FOOBAR was never DEFINEd: the named-global inline-cache path.
e1: db "(FOOBAR 1 2 3)"
e1_len: equ $ - e1

section .text

run_thunk_discard:
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    ret

global lamedh_main
lamedh_main:
    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard        ; prints "BEFORE" — proves the
                                   ; process was running normally right
                                   ; up until the failing call below

    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard        ; never returns: exit(1)

    ; unreachable
    mov rax, 99
    ret
