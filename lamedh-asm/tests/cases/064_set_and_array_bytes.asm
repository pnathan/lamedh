; 064_set_and_array_bytes — two more kernel primitives found missing
; while chasing lib/29-protocols.lisp and lib/30-text.lisp through the
; reference standard library:
;
; 1. SET (Lisp 1.5's `(set sym-form val-form)`, KERNEL.md/the
;    reference's own environment.rs builtin): unlike DEFINE/SETQ, whose
;    target is a literal name known at compile time, SET evaluates its
;    first operand to find out WHICH symbol to assign at runtime
;    (lib/29-protocols.lisp's DEFPROTOCOL rebinds a dynamically-named
;    protocol symbol this way). New set_symbol_value host routine
;    (symtab.asm) plus an ordinary evaluated-both-operands hostcall
;    dispatch (compiler.asm), same shape as the existing
;    SET-SYMBOL-PLIST!.
;
; 2. This kernel's STRING representation already stores a string's raw
;    UTF-8 bytes directly (STRING-LENGTH is a byte count, not a
;    codepoint count) — lib/30-text.lisp's STRING->UTF8/UTF8->STRING
;    (lib/prelude.lisp's own STRING->UTF8*/UTF8->STRING*) are ordinary
;    Lisp over ARRAY/STORE/FETCH/CODE-CHAR/STRING-REF/CONCAT, no new
;    kernel primitive needed — this test exercises exactly that
;    ARRAY<->byte round trip at the kernel-primitive level directly.
;    STRING-APPEND stands in for CONCAT here (CONCAT is itself only a
;    prelude.lisp wrapper over STRING-APPEND, unavailable with no
;    prelude loaded, like every *.asm test in this directory) — both
;    accept CHAR values directly, confirmed by direct testing.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; --- part 1: SET on a dynamically-computed symbol ---
e1: db "(DEFINE TARGET-NAME (QUOTE MY-GLOBAL))"
e1_len: equ $ - e1

e2: db "(SET TARGET-NAME 99)"
e2_len: equ $ - e2

e3: db "(PRINT MY-GLOBAL)"
e3_len: equ $ - e3                              ; 99

; --- part 2: byte array <-> string round trip via kernel primitives
; only (CODE-CHAR/STRING-REF/CONCAT/ARRAY/STORE/FETCH) ---
e4: db "(DEFINE BYTES (ARRAY 2))"
e4_len: equ $ - e4

e5: db '(STORE BYTES 0 (CODE-CHAR (STRING-REF "HI" 0)))'
e5_len: equ $ - e5

e6: db '(STORE BYTES 1 (CODE-CHAR (STRING-REF "HI" 1)))'
e6_len: equ $ - e6

e7: db "(PRINT (STRING-APPEND (FETCH BYTES 0) (FETCH BYTES 1)))"
e7_len: equ $ - e7                              ; HI

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
    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; 99

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard

    mov rdi, e7
    mov rsi, e7_len
    call run_thunk_discard
    call print_newline               ; HI

    xor rax, rax
    ret
