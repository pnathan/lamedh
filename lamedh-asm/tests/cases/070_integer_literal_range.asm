; 070_integer_literal_range — issue #550. A fixnum here is 62 bits wide
; (tags.inc: [-2^61, 2^61-1]), narrower than KERNEL.md's i64. A decimal
; integer token outside that range used to wrap silently (2^63-1 read as
; -1, 2^61 as -2^61, 99999999999999999999 as garbage); it now reads as
; the correctly rounded Float, KERNEL.md Part II's own rule for a token
; outside the integer range, applied at this kernel's width. The float
; printer prints such magnitudes exactly (it used to route them through
; a 62-bit fixnum / cvttsd2si), and inf/NaN print as `inf`/`-inf`/`NaN`.
; Also (ASH 1 61), whose result leaves the fixnum range, now sets
; OVERFLOW (it wrapped silently); (ASH 1 60) and (ASH -1 61) still fit.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

%macro FORM 2
%1: db %2
%1_len: equ $ - %1
%endmacro

section .rodata
; --- fixnum boundaries still read as fixnums ---
FORM f1,  "(PRINT 2305843009213693951)"             ; 2^61-1: fixnum
FORM f2,  "(PRINT -2305843009213693952)"            ; -2^61: fixnum
FORM f3,  "(PRINT (FIXP 2305843009213693951))"      ; T
; --- just outside the fixnum range: Float, exact ---
FORM f4,  "(PRINT 2305843009213693952)"             ; 2^61
FORM f5,  "(PRINT (FLOATP 2305843009213693952))"    ; T
FORM f6,  "(PRINT -2305843009213693953)"            ; rounds to -2^61
; --- the issue's own literals ---
FORM f7,  "(PRINT 9223372036854775807)"             ; rounds to 2^63
FORM f8,  "(PRINT 99999999999999999999)"            ; rounds to 1e20
; --- u64 edge, and past u64 (exact bigdec path, correctly rounded) ---
FORM f9,  "(PRINT 18446744073709551615)"            ; rounds to 2^64
FORM f10, "(PRINT 1267650600228229401496703205376)" ; 2^100 exactly
FORM f11, "(PRINT 123456789012345678901234567890)"
FORM f12, "(PRINT 12345678901234567890123.5)"       ; float literal, big int part
; --- past the largest double: inf; and the other non-finite prints ---
FORM f13, "(PRINT 1000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000)"
FORM f14, "(PRINT -1000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000)"
FORM f15, "(PRINT (F/ 0.0 0.0))"                    ; NaN
; --- small floats unchanged ---
FORM f16, "(PRINT -12.25)"
; --- ASH: in range, no flag ---
FORM f17, "(PRINT (ASH 1 60))"                      ; 2^60
FORM f18, "(PRINT (ASH -1 61))"                     ; -2^61 fits
FORM f19, "(PRINT (ASH -5 3))"                      ; -40
FORM f20, "(PRINT (FLAG-SET-P (QUOTE OVERFLOW)))"   ; ()
; --- ASH leaving the range: wraps to 62 bits and sets OVERFLOW ---
FORM f21, "(PRINT (ASH 1 61))"                      ; -2^61, wrapped
FORM f22, "(PRINT (FLAG-SET-P (QUOTE OVERFLOW)))"   ; T
FORM f23, "(CLEAR-FLAG (QUOTE OVERFLOW))"
FORM f24, "(PRINT (ASH 3 60))"                      ; wraps
FORM f25, "(PRINT (FLAG-SET-P (QUOTE OVERFLOW)))"   ; T

align 8
forms:
%assign i 1
%rep 25
    dq f %+ i, f %+ i %+ _len
%assign i i+1
%endrep
forms_end:

section .text

global lamedh_main
lamedh_main:
    push rbx
    mov rbx, forms
.next:
    cmp rbx, forms_end
    jae .done
    mov rdi, [rbx]
    mov rsi, [rbx+8]
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    call print_newline
    add rbx, 16
    jmp .next
.done:
    pop rbx
    xor rax, rax
    ret
