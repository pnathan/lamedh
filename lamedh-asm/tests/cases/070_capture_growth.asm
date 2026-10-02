; 070_capture_growth — PRIN1-TO-STRING / PRINC-TO-STRING capture past
; the initial 64 KB buffer (#547).
;
; write_buf used to clamp a capture at CAPTURE_BUF_BYTES, so rendering a
; 40 000-element list — 80 001 bytes, "(7 7 ... 7)" — silently came back
; as its first 65 536 bytes with no closing paren, and PRIN1 (the
; prelude's (PRINT (PRIN1-TO-STRING X))) wrote the same truncated text.
; The buffer now doubles on the heap as needed. Pinned down here:
;
;  1. the full length (80 001) and both ends of the rendering, the
;     tail's closing paren included;
;  2. PRINC-TO-STRING of the same list, and PRIN1-TO-STRING of the
;     80 001-byte string itself (80 003: two quotes added);
;  3. a capture that grows twice (160 005 bytes, past 128 KB) and one
;     that mixes a large string with the large list (160 007);
;  4. a small capture after all that still starts from a sane buffer,
;     and GC-VERIFY agrees the freed-and-replaced raw buffers left the
;     heap consistent.
;
; Kernel-only: DEFINE/LAMBDA/IF/CONS and the capture primitives. No
; prelude is loaded for a tests/cases/*.asm binary; PRIN1 itself (a
; prelude definition) is covered by tests/run.sh's lamedhc checks.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk

section .rodata
e1: db "(DEFINE MKLIST (LAMBDA (N ACC) (IF (= N 0) ACC (MKLIST (- N 1) (CONS 7 ACC)))))"
e1_len: equ $ - e1
e2: db "(DEFINE L (MKLIST 40000 NIL))"
e2_len: equ $ - e2
e3: db "(DEFINE S (PRIN1-TO-STRING L))"
e3_len: equ $ - e3
e4: db "(PRINT (STRING-LENGTH* S))"
e4_len: equ $ - e4
e5: db "(NEWLINE)"
e5_len: equ $ - e5
e6: db "(PRINT (SUBSTRING S 0 6))"
e6_len: equ $ - e6
e7: db "(NEWLINE)"
e7_len: equ $ - e7
e8: db "(PRINT (SUBSTRING S 79990 80001))"
e8_len: equ $ - e8
e9: db "(NEWLINE)"
e9_len: equ $ - e9
e10: db "(PRINT (STRING-LENGTH* (PRINC-TO-STRING L)))"
e10_len: equ $ - e10
e11: db "(NEWLINE)"
e11_len: equ $ - e11
e12: db "(PRINT (STRING-LENGTH* (PRIN1-TO-STRING S)))"
e12_len: equ $ - e12
e13: db "(NEWLINE)"
e13_len: equ $ - e13
e14: db "(PRINT (STRING-LENGTH* (PRINC-TO-STRING (CONS S (CONS S NIL)))))"
e14_len: equ $ - e14
e15: db "(NEWLINE)"
e15_len: equ $ - e15
e16: db "(PRINT (STRING-LENGTH* (PRIN1-TO-STRING (CONS S (CONS L NIL)))))"
e16_len: equ $ - e16
e17: db "(NEWLINE)"
e17_len: equ $ - e17
e18: db "(PRINT (STRING-LENGTH* (PRIN1-TO-STRING 42)))"
e18_len: equ $ - e18
e19: db "(NEWLINE)"
e19_len: equ $ - e19
e20: db "(PRINT (GC-VERIFY))"
e20_len: equ $ - e20
e21: db "(NEWLINE)"
e21_len: equ $ - e21

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
    RUN e6
    RUN e7
    RUN e8
    RUN e9
    RUN e10
    RUN e11
    RUN e12
    RUN e13
    RUN e14
    RUN e15
    RUN e16
    RUN e17
    RUN e18
    RUN e19
    RUN e20
    RUN e21

    xor rax, rax
    ret
