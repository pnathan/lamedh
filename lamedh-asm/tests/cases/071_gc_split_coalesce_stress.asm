; 071_gc_split_coalesce_stress — issue #548's collector stress: the
; split/coalescing allocator must never hand out a granule that is
; still reachable, and must keep the side-table tiling that GC-VERIFY
; and the collector's linear sweep walk intact.
;
; 10^6 conses are allocated and dropped at once, while a 100 000-cell
; live list stays reachable from a global. Every 997th step also builds
; a string of 0..299 bytes one byte at a time, so every size class up
; to ~19 granules is freed, split and coalesced around the conses, and
; a list of some of those strings is kept (every 5 003rd step drops one
; again). Checked afterwards: the live list's length, sum and element
; order; every kept string still equal to a freshly built one of its
; recorded length; GC-VERIFY before and after a real collection; and
; a bump pointer that grew by under 8 MiB for 16 MB of conses plus
; roughly 18 MB of strings (which fails on the old exact-fit
; allocator; everything else here passes on it too). The churn then runs a second time with both lists
; dropped, and the same checks hold.
;
; Kernel-only, like every tests/cases/*.asm binary: no prelude.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk

section .rodata
e1: db "(DEFINE MKLIST (LAMBDA (N ACC) (IF (= N 0) ACC (MKLIST (- N 1) (CONS N ACC)))))"
e1_len: equ $ - e1
e2: db "(DEFINE LEN (LAMBDA (L N) (IF (EQ L NIL) N (LEN (CDR L) (+ N 1)))))"
e2_len: equ $ - e2
e3: db "(DEFINE SUM (LAMBDA (L A) (IF (EQ L NIL) A (SUM (CDR L) (+ A (CAR L))))))"
e3_len: equ $ - e3
e4: db "(DEFINE ORDERED (LAMBDA (L K) (IF (EQ L NIL) K (IF (= (CAR L) K) (ORDERED (CDR L) (+ K 1)) (- 0 K)))))"
e4_len: equ $ - e4
e5: db "(DEFINE REP (LAMBDA (N ACC) (IF (= N 0) ACC (REP (- N 1) (STRING-APPEND ACC ", 34, "x", 34, ")))))"
e5_len: equ $ - e5
e6: db "(DEFINE KEEP-OK (LAMBDA (L) (IF (EQ L NIL) T (IF (EQ (CDR (CAR L)) (REP (CAR (CAR L)) ", 34, 34, ")) (KEEP-OK (CDR L)) (CAR L)))))"
e6_len: equ $ - e6
e7: db "(DEFINE LIVE (MKLIST 100000 NIL))"
e7_len: equ $ - e7
e8: db "(DEFINE KEEP NIL)"
e8_len: equ $ - e8
e9: db "(DEFINE CHURN (LAMBDA (N) (IF (= N 0) 0 (PROGN (CONS N N) (IF (= (MOD N 997) 0) (SETQ KEEP (CONS (CONS (MOD N 300) (REP (MOD N 300) ", 34, 34, ")) KEEP)) NIL) (IF (= (MOD N 5003) 0) (SETQ KEEP (CDR KEEP)) NIL) (CHURN (- N 1))))))"
e9_len: equ $ - e9
e10: db "(DEFINE U0 (HEAP-BYTES-USED))"
e10_len: equ $ - e10
e11: db "(CHURN 1000000)"
e11_len: equ $ - e11
e12: db "(PRINT (LEN LIVE 0))"
e12_len: equ $ - e12
e13: db "(NEWLINE)"
e13_len: equ $ - e13
e14: db "(PRINT (SUM LIVE 0))"
e14_len: equ $ - e14
e15: db "(NEWLINE)"
e15_len: equ $ - e15
e16: db "(PRINT (ORDERED LIVE 1))"
e16_len: equ $ - e16
e17: db "(NEWLINE)"
e17_len: equ $ - e17
e18: db "(PRINT (LEN KEEP 0))"
e18_len: equ $ - e18
e19: db "(NEWLINE)"
e19_len: equ $ - e19
e20: db "(PRINT (KEEP-OK KEEP))"
e20_len: equ $ - e20
e21: db "(NEWLINE)"
e21_len: equ $ - e21
e22: db "(PRINT (< (- (HEAP-BYTES-USED) U0) 8388608))"
e22_len: equ $ - e22
e23: db "(NEWLINE)"
e23_len: equ $ - e23
e24: db "(PRINT (GC-VERIFY))"
e24_len: equ $ - e24
e25: db "(NEWLINE)"
e25_len: equ $ - e25
e26: db "(GC-COLLECT)"
e26_len: equ $ - e26
e27: db "(PRINT (GC-VERIFY))"
e27_len: equ $ - e27
e28: db "(NEWLINE)"
e28_len: equ $ - e28
e29: db "(PRINT (ORDERED LIVE 1))"
e29_len: equ $ - e29
e30: db "(NEWLINE)"
e30_len: equ $ - e30
e31: db "(PRINT (KEEP-OK KEEP))"
e31_len: equ $ - e31
e32: db "(NEWLINE)"
e32_len: equ $ - e32
e33: db "(SETQ LIVE NIL)"
e33_len: equ $ - e33
e34: db "(SETQ KEEP NIL)"
e34_len: equ $ - e34
e35: db "(CHURN 1000000)"
e35_len: equ $ - e35
e36: db "(PRINT (LEN KEEP 0))"
e36_len: equ $ - e36
e37: db "(NEWLINE)"
e37_len: equ $ - e37
e38: db "(PRINT (KEEP-OK KEEP))"
e38_len: equ $ - e38
e39: db "(NEWLINE)"
e39_len: equ $ - e39
e40: db "(GC-COLLECT)"
e40_len: equ $ - e40
e41: db "(PRINT (GC-VERIFY))"
e41_len: equ $ - e41
e42: db "(NEWLINE)"
e42_len: equ $ - e42
e43: db "(PRINT (< (- (HEAP-BYTES-USED) U0) 8388608))"
e43_len: equ $ - e43
e44: db "(NEWLINE)"
e44_len: equ $ - e44

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
    RUN e22
    RUN e23
    RUN e24
    RUN e25
    RUN e26
    RUN e27
    RUN e28
    RUN e29
    RUN e30
    RUN e31
    RUN e32
    RUN e33
    RUN e34
    RUN e35
    RUN e36
    RUN e37
    RUN e38
    RUN e39
    RUN e40
    RUN e41
    RUN e42
    RUN e43
    RUN e44
    xor rax, rax
    ret
