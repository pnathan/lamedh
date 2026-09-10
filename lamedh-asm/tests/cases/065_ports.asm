; 065_ports — src/ports.asm: the reference's own PortObj
; (lib/31-ports.lisp), backing synchronous binary ports over real
; files, in-memory byte buffers, and stdin/stdout/stderr. Found
; missing while responding to explicit review feedback asking for
; "file handling capabilities including file descriptors" — this
; kernel already had bare fd primitives (FD-OPEN/FD-READ/FD-WRITE/
; FD-CLOSE, fileio.asm) but lib/31-ports.lisp needs a real PortObj
; abstraction (PORT-OPEN-INPUT-FILE*/PORT-READ-BYTE*/... — 24 Rust-
; level builtins in the reference, evaluator/builtins_ports.rs) with
; its own mutable cursor/buffer state, which nothing here provided.
;
; Deliberately uses only kernel-primitive special forms (ARRAY/STORE/
; FETCH/CODE-CHAR/STRING-REF/STRING-APPEND/DEFINE/QUOTE/PROGN/PRINT) —
; no LIST/FUNCALL/DEFUN/CONCAT — same discipline as 063/064, since
; standalone tests/cases/*.asm run with no prelude loaded at all.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; --- in-memory ports round-trip ---
e1: db "(DEFINE BYTES (ARRAY 3))"
e1_len: equ $ - e1

e2: db '(STORE BYTES 0 (CODE-CHAR (STRING-REF "H" 0)))'
e2_len: equ $ - e2

e3: db '(STORE BYTES 1 (CODE-CHAR (STRING-REF "I" 0)))'
e3_len: equ $ - e3

e4: db '(STORE BYTES 2 (CODE-CHAR (STRING-REF "!" 0)))'
e4_len: equ $ - e4

e5: db "(DEFINE IP (PORT-OPEN-INPUT-BYTES* BYTES))"
e5_len: equ $ - e5

e6: db "(PRINT (PORT-P* IP))"
e6_len: equ $ - e6                              ; T

e7: db "(PRINT (PORT-READ-BYTE* IP))"
e7_len: equ $ - e7                              ; 72

e8: db "(PRINT (PORT-READ-BYTE* IP))"
e8_len: equ $ - e8                              ; 73

e9: db "(PRINT (PORT-READ-BYTE* IP))"
e9_len: equ $ - e9                              ; 33

e10: db "(PRINT (PORT-READ-BYTE* IP))"
e10_len: equ $ - e10                            ; ()

; --- output-bytes port + write-byte/output-contents ---
e11: db "(DEFINE OP (PORT-OPEN-OUTPUT-BYTES*))"
e11_len: equ $ - e11

e12: db "(PRINT (PORT-WRITE-BYTE* OP 65))"
e12_len: equ $ - e12                            ; 65

e13: db "(PRINT (PORT-WRITE-BYTE* OP 66))"
e13_len: equ $ - e13                            ; 66

e14: db "(PRINT (STRING-APPEND (FETCH (PORT-OUTPUT-CONTENTS* OP) 0) (FETCH (PORT-OUTPUT-CONTENTS* OP) 1)))"
e14_len: equ $ - e14                            ; AB

; --- a real file: write, close, reopen, seek, read, close twice ---
e15: db '(DEFINE FP (PORT-OPEN-OUTPUT-FILE* "/tmp/lamedh_asm_test_065_ports.bin"))'
e15_len: equ $ - e15

e16: db "(PRINT (PORT-WRITE-BYTES* FP BYTES))"
e16_len: equ $ - e16                            ; 3

e17: db "(PRINT (PORT-CLOSE* FP))"
e17_len: equ $ - e17                            ; T

e18: db '(DEFINE RP (PORT-OPEN-INPUT-FILE* "/tmp/lamedh_asm_test_065_ports.bin"))'
e18_len: equ $ - e18

e19: db "(PRINT (PORT-SEEKABLE-P* RP))"
e19_len: equ $ - e19                            ; T

e20: db "(PRINT (PORT-SEEK* RP 1))"
e20_len: equ $ - e20                            ; 1

e21: db "(PRINT (PORT-POSITION* RP))"
e21_len: equ $ - e21                            ; 1

e22: db "(PRINT (PORT-READ-BYTE* RP))"
e22_len: equ $ - e22                            ; 73

e23: db "(PRINT (PORT-CLOSE* RP))"
e23_len: equ $ - e23                            ; T

e24: db "(PRINT (PORT-CLOSE* RP))"
e24_len: equ $ - e24                            ; T (idempotent)

; --- stdout port ---
e25: db "(DEFINE SO (PORT-STDOUT*))"
e25_len: equ $ - e25

e26: db "(PRINT (PORT-KIND* SO))"
e26_len: equ $ - e26                            ; STDOUT

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

%macro RUN_PRINTLN 1
    mov rdi, %1
    mov rsi, %1 %+ _len
    call run_thunk_discard
    call print_newline
%endmacro

%macro RUN 1
    mov rdi, %1
    mov rsi, %1 %+ _len
    call run_thunk_discard
%endmacro

global lamedh_main
lamedh_main:
    RUN e1
    RUN e2
    RUN e3
    RUN e4
    RUN e5
    RUN_PRINTLN e6
    RUN_PRINTLN e7
    RUN_PRINTLN e8
    RUN_PRINTLN e9
    RUN_PRINTLN e10
    RUN e11
    RUN_PRINTLN e12
    RUN_PRINTLN e13
    RUN_PRINTLN e14
    RUN e15
    RUN_PRINTLN e16
    RUN_PRINTLN e17
    RUN e18
    RUN_PRINTLN e19
    RUN_PRINTLN e20
    RUN_PRINTLN e21
    RUN_PRINTLN e22
    RUN_PRINTLN e23
    RUN_PRINTLN e24
    RUN e25
    RUN_PRINTLN e26

    xor rax, rax
    ret
