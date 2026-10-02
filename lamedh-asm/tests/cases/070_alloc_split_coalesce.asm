; 070_alloc_split_coalesce — issue #548: the data-heap allocator splits
; free runs to satisfy smaller requests and coalesces adjacent free
; runs the moment they are freed (gc.asm, free_run / alloc_granules).
;
; Before this, free lists were exact-fit only: a freed run was reused
; only by a request of exactly its own granule count, and never split
; or merged, so the heap filled up while most of it was free. Every
; group below fails on that allocator; the last one ran it out of heap
; outright ("lamedh: data heap exhausted", exit 1).
;
;  1. Split: a freed 800 KB array is carved into 40 000 conses. The
;     bump pointer does not move (it advanced 640 064 bytes before).
;  2. Coalesce: 20 000 freed [cons | 3-slot array] pairs (64 bytes
;     each, adjacent) merge into one run that holds a 960 KB array.
;     The bump pointer does not move (it advanced 960 080 bytes before).
;  3. The live list from group 1 and the array from group 2 are intact.
;  4. The issue's own repro: a string grown 4 bytes at a time 12 000
;     times (288 MB allocated in total, into a 256 MiB arena). It
;     finishes with the right contents, and the bump pointer grows by
;     under 32 MiB, i.e. twice the collector's 16 MiB byte trigger,
;     which is how much garbage a deferred collector may legitimately
;     hold between collections. Running the whole loop a second time
;     moves it by under 1 MiB: the heap has reached a steady state.
;  5. GC-VERIFY agrees before and after a collection, and after it the
;     high-water mark is within that same 32 MiB of the live bytes.
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
e4: db "(DEFINE AR (ARRAY 100000))"
e4_len: equ $ - e4
e5: db "(DEFINE L0 (HEAP-BYTES-LIVE))"
e5_len: equ $ - e5
e6: db "(SETQ AR NIL)"
e6_len: equ $ - e6
e7: db "(GC-COLLECT)"
e7_len: equ $ - e7
e8: db "(PRINT (< 700000 (- L0 (HEAP-BYTES-LIVE))))"
e8_len: equ $ - e8
e9: db "(NEWLINE)"
e9_len: equ $ - e9
e10: db "(DEFINE U1 (HEAP-BYTES-USED))"
e10_len: equ $ - e10
e11: db "(DEFINE L (MKLIST 40000 NIL))"
e11_len: equ $ - e11
e12: db "(PRINT (< (- (HEAP-BYTES-USED) U1) 65536))"
e12_len: equ $ - e12
e13: db "(NEWLINE)"
e13_len: equ $ - e13
e14: db "(DEFINE MKARRS (LAMBDA (N ACC) (IF (= N 0) ACC (MKARRS (- N 1) (CONS (ARRAY 3) ACC)))))"
e14_len: equ $ - e14
e15: db "(DEFINE AS (MKARRS 20000 NIL))"
e15_len: equ $ - e15
e16: db "(DEFINE L1 (HEAP-BYTES-LIVE))"
e16_len: equ $ - e16
e17: db "(SETQ AS NIL)"
e17_len: equ $ - e17
e18: db "(GC-COLLECT)"
e18_len: equ $ - e18
e19: db "(PRINT (< 1000000 (- L1 (HEAP-BYTES-LIVE))))"
e19_len: equ $ - e19
e20: db "(NEWLINE)"
e20_len: equ $ - e20
e21: db "(DEFINE U2 (HEAP-BYTES-USED))"
e21_len: equ $ - e21
e22: db "(DEFINE BIGA (ARRAY 120000))"
e22_len: equ $ - e22
e23: db "(PRINT (< (- (HEAP-BYTES-USED) U2) 65536))"
e23_len: equ $ - e23
e24: db "(NEWLINE)"
e24_len: equ $ - e24
e25: db "(PRINT (LEN L 0))"
e25_len: equ $ - e25
e26: db "(NEWLINE)"
e26_len: equ $ - e26
e27: db "(PRINT (SUM L 0))"
e27_len: equ $ - e27
e28: db "(NEWLINE)"
e28_len: equ $ - e28
e29: db "(PRINT (FETCH BIGA 119999))"
e29_len: equ $ - e29
e30: db "(NEWLINE)"
e30_len: equ $ - e30
e31: db "(DEFINE GROW (LAMBDA (S N) (IF (= N 0) S (GROW (STRING-APPEND S ", 34, "abcd", 34, ") (- N 1)))))"
e31_len: equ $ - e31
e32: db "(DEFINE U3 (HEAP-BYTES-USED))"
e32_len: equ $ - e32
e33: db "(DEFINE R (GROW ", 34, 34, " 12000))"
e33_len: equ $ - e33
e34: db "(PRINT (STRING-LENGTH R))"
e34_len: equ $ - e34
e35: db "(NEWLINE)"
e35_len: equ $ - e35
e36: db "(PRINT (SUBSTRING R 47996 48000))"
e36_len: equ $ - e36
e37: db "(NEWLINE)"
e37_len: equ $ - e37
e38: db "(PRINT (< (- (HEAP-BYTES-USED) U3) 33554432))"
e38_len: equ $ - e38
e39: db "(NEWLINE)"
e39_len: equ $ - e39
e40: db "(GC-COLLECT)"
e40_len: equ $ - e40
e41: db "(DEFINE U4 (HEAP-BYTES-USED))"
e41_len: equ $ - e41
e42: db "(SETQ R (GROW ", 34, 34, " 12000))"
e42_len: equ $ - e42
e43: db "(PRINT (< (- (HEAP-BYTES-USED) U4) 1048576))"
e43_len: equ $ - e43
e44: db "(NEWLINE)"
e44_len: equ $ - e44
e45: db "(PRINT (STRING-LENGTH R))"
e45_len: equ $ - e45
e46: db "(NEWLINE)"
e46_len: equ $ - e46
e47: db "(PRINT (LEN L 0))"
e47_len: equ $ - e47
e48: db "(NEWLINE)"
e48_len: equ $ - e48
e49: db "(PRINT (SUM L 0))"
e49_len: equ $ - e49
e50: db "(NEWLINE)"
e50_len: equ $ - e50
e51: db "(PRINT (GC-VERIFY))"
e51_len: equ $ - e51
e52: db "(NEWLINE)"
e52_len: equ $ - e52
e53: db "(GC-COLLECT)"
e53_len: equ $ - e53
e54: db "(PRINT (GC-VERIFY))"
e54_len: equ $ - e54
e55: db "(NEWLINE)"
e55_len: equ $ - e55
e56: db "(PRINT (< (HEAP-BYTES-USED) (+ (HEAP-BYTES-LIVE) 33554432)))"
e56_len: equ $ - e56
e57: db "(NEWLINE)"
e57_len: equ $ - e57

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
    RUN e45
    RUN e46
    RUN e47
    RUN e48
    RUN e49
    RUN e50
    RUN e51
    RUN e52
    RUN e53
    RUN e54
    RUN e55
    RUN e56
    RUN e57
    xor rax, rax
    ret
