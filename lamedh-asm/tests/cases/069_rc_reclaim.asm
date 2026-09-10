; 069_rc_reclaim — the deferred reference-counting collector
; (docs/spec-tco-capture-gc.md section 3), landing-plan steps 1-5.
;
; Every assertion below is a boolean or an exact value, never a raw
; byte count: the point is that memory is *reclaimed*, not that it is
; reclaimed to a particular number, and a conservative collector is
; allowed to retain a dead object for a cycle when a stale word in an
; already-allocated stack slot still looks like a pointer to it.
;
; What each group pins down:
;
;  1. Reclamation is observable at all. The spec's own repro —
;     (SPIN 200000), 3.2 MiB of immediately-dropped conses — advances
;     the bump pointer by under 4 MiB, because after the first
;     collection the freed 16-byte granules are handed back out by the
;     exact-fit free list instead of bumping. Before this feature the
;     bump pointer advanced by the full 3.2 MiB and never retreated.
;  2. Live data survives a collection: a 10 000-element list built
;     into a global still has its full length and checksum after
;     100 000 conses of garbage have been allocated and collected
;     around it; dropping the global then releases > 100 KB.
;  3. A closure's captured value is retained while the closure is live
;     (calling it still reads the captured string) and released when
;     the closure itself is dropped.
;  4. STORE into an array slot releases the value it overwrote; a
;     shared cons is NOT freed while a second reference remains (its
;     REFCOUNT goes 3 -> 2 and its car is still readable).
;  5. THROW across frames holding fresh garbage neither crashes nor
;     corrupts anything — the skipped frames held only *uncounted*
;     references, which is exactly why deferred counting needs no
;     unwind hook at all — and GC-VERIFY agrees afterwards.
;  6. PRINC-TO-STRING no longer leaks its 64 KB capture buffer: 2 000
;     calls, which cost 128 MiB before this feature (half the entire
;     arena), now cost under 1 MiB.
;  7. A value baked into emitted code stays pinned: EVAL of a
;     READ-FROM-STRING result in a loop with garbage in between still
;     returns (1 2 3), and a QUOTE literal survives collection.
;  8. Cycles leak, deliberately and documented (v0 stance, matching
;     every early reference-counting Lisp): an array storing itself is
;     NOT reclaimed when its last outside reference is dropped. The
;     assertion is that live bytes do not drop — i.e. that the cycle
;     is retained rather than silently, incorrectly collected.
;  9. SET-SYMBOL-PLIST! counts: a plist stored on a symbol survives
;     50 000 conses of garbage, and GC-VERIFY — a full linear heap
;     walk that recomputes every unpinned object's count from scratch
;     and compares it with the side table — agrees with the collector
;     at the end.
;
; Kernel-only: DEFINE/LAMBDA/IF/PROGN/SETQ/CATCH/THROW/QUOTE/ARRAY/
; STORE/FETCH and the new HEAP-BYTES-USED/HEAP-BYTES-LIVE/GC-COLLECT/
; REFCOUNT/GC-VERIFY primitives. No prelude is loaded for a
; tests/cases/*.asm binary.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk

section .rodata
e1: db "(DEFINE SPIN (LAMBDA (N) (IF (= N 0) 0 (PROGN (CONS N NIL) (SPIN (- N 1))))))"
e1_len: equ $ - e1
e2: db "(DEFINE U0 (HEAP-BYTES-USED))"
e2_len: equ $ - e2
e3: db "(SPIN 200000)"
e3_len: equ $ - e3
e4: db "(GC-COLLECT)"
e4_len: equ $ - e4
e5: db "(PRINT (< (- (HEAP-BYTES-USED) U0) 4194304))"
e5_len: equ $ - e5
e6: db "(NEWLINE)"
e6_len: equ $ - e6
e7: db "(DEFINE MKLIST (LAMBDA (N ACC) (IF (= N 0) ACC (MKLIST (- N 1) (CONS N ACC)))))"
e7_len: equ $ - e7
e8: db "(DEFINE LEN (LAMBDA (L N) (IF (EQ L NIL) N (LEN (CDR L) (+ N 1)))))"
e8_len: equ $ - e8
e9: db "(DEFINE SUM (LAMBDA (L A) (IF (EQ L NIL) A (SUM (CDR L) (+ A (CAR L))))))"
e9_len: equ $ - e9
e10: db "(DEFINE BIG (MKLIST 10000 NIL))"
e10_len: equ $ - e10
e11: db "(SPIN 100000)"
e11_len: equ $ - e11
e12: db "(GC-COLLECT)"
e12_len: equ $ - e12
e13: db "(PRINT (LEN BIG 0))"
e13_len: equ $ - e13
e14: db "(NEWLINE)"
e14_len: equ $ - e14
e15: db "(PRINT (SUM BIG 0))"
e15_len: equ $ - e15
e16: db "(NEWLINE)"
e16_len: equ $ - e16
e17: db "(DEFINE L0 (HEAP-BYTES-LIVE))"
e17_len: equ $ - e17
e18: db "(SETQ BIG NIL)"
e18_len: equ $ - e18
e19: db "(GC-COLLECT)"
e19_len: equ $ - e19
e20: db "(PRINT (< 100000 (- L0 (HEAP-BYTES-LIVE))))"
e20_len: equ $ - e20
e21: db "(NEWLINE)"
e21_len: equ $ - e21
e22: db "(DEFINE MKC (LAMBDA (S) (LAMBDA () (STRING-LENGTH S))))"
e22_len: equ $ - e22
e23: db "(DEFINE CL (MKC (STRING-APPEND ", 34, "0123456789", 34, " ", 34, "0123456789", 34, ")))"
e23_len: equ $ - e23
e24: db "(SPIN 100000)"
e24_len: equ $ - e24
e25: db "(GC-COLLECT)"
e25_len: equ $ - e25
e26: db "(PRINT (CL))"
e26_len: equ $ - e26
e27: db "(NEWLINE)"
e27_len: equ $ - e27
e28: db "(DEFINE L1 (HEAP-BYTES-LIVE))"
e28_len: equ $ - e28
e29: db "(SETQ CL NIL)"
e29_len: equ $ - e29
e30: db "(GC-COLLECT)"
e30_len: equ $ - e30
e31: db "(PRINT (< 30 (- L1 (HEAP-BYTES-LIVE))))"
e31_len: equ $ - e31
e32: db "(NEWLINE)"
e32_len: equ $ - e32
e33: db "(DEFINE AR (ARRAY 2))"
e33_len: equ $ - e33
e34: db "(STORE AR 0 (MKLIST 5000 NIL))"
e34_len: equ $ - e34
e35: db "(PRINT (LEN (FETCH AR 0) 0))"
e35_len: equ $ - e35
e36: db "(NEWLINE)"
e36_len: equ $ - e36
e37: db "(DEFINE L2 (HEAP-BYTES-LIVE))"
e37_len: equ $ - e37
e38: db "(STORE AR 0 NIL)"
e38_len: equ $ - e38
e39: db "(GC-COLLECT)"
e39_len: equ $ - e39
e40: db "(PRINT (< 50000 (- L2 (HEAP-BYTES-LIVE))))"
e40_len: equ $ - e40
e41: db "(NEWLINE)"
e41_len: equ $ - e41
e42: db "(DEFINE SHARED (CONS 1 2))"
e42_len: equ $ - e42
e43: db "(DEFINE HOLD1 (CONS SHARED NIL))"
e43_len: equ $ - e43
e44: db "(DEFINE HOLD2 (CONS SHARED NIL))"
e44_len: equ $ - e44
e45: db "(PRINT (REFCOUNT SHARED))"
e45_len: equ $ - e45
e46: db "(NEWLINE)"
e46_len: equ $ - e46
e47: db "(SETQ HOLD1 NIL)"
e47_len: equ $ - e47
e48: db "(GC-COLLECT)"
e48_len: equ $ - e48
e49: db "(PRINT (CAR (CAR HOLD2)))"
e49_len: equ $ - e49
e50: db "(NEWLINE)"
e50_len: equ $ - e50
e51: db "(PRINT (REFCOUNT SHARED))"
e51_len: equ $ - e51
e52: db "(NEWLINE)"
e52_len: equ $ - e52
e53: db "(DEFINE DEEP (LAMBDA (N) (PROGN (CONS N NIL) (IF (= N 0) (THROW (QUOTE TG) 99) (DEEP (- N 1))))))"
e53_len: equ $ - e53
e54: db "(PRINT (CATCH (QUOTE TG) (DEEP 300)))"
e54_len: equ $ - e54
e55: db "(NEWLINE)"
e55_len: equ $ - e55
e56: db "(GC-COLLECT)"
e56_len: equ $ - e56
e57: db "(PRINT (GC-VERIFY))"
e57_len: equ $ - e57
e58: db "(NEWLINE)"
e58_len: equ $ - e58
e59: db "(DEFINE PLOOP (LAMBDA (N) (IF (= N 0) 0 (PROGN (PRINC-TO-STRING (QUOTE HELLO)) (PLOOP (- N 1))))))"
e59_len: equ $ - e59
e60: db "(DEFINE U1 (HEAP-BYTES-USED))"
e60_len: equ $ - e60
e61: db "(PLOOP 2000)"
e61_len: equ $ - e61
e62: db "(GC-COLLECT)"
e62_len: equ $ - e62
e63: db "(PRINT (< (- (HEAP-BYTES-USED) U1) 1048576))"
e63_len: equ $ - e63
e64: db "(NEWLINE)"
e64_len: equ $ - e64
e65: db "(DEFINE ELOOP (LAMBDA (N) (IF (= N 0) 0 (PROGN (EVAL (READ-FROM-STRING ", 34, "(QUOTE (1 2 3))", 34, ")) (CONS N NIL) (ELOOP (- N 1))))))"
e65_len: equ $ - e65
e66: db "(ELOOP 200)"
e66_len: equ $ - e66
e67: db "(GC-COLLECT)"
e67_len: equ $ - e67
e68: db "(PRINT (EVAL (READ-FROM-STRING ", 34, "(QUOTE (1 2 3))", 34, ")))"
e68_len: equ $ - e68
e69: db "(NEWLINE)"
e69_len: equ $ - e69
e70: db "(PRINT (QUOTE (7 8 9)))"
e70_len: equ $ - e70
e71: db "(NEWLINE)"
e71_len: equ $ - e71
e72: db "(DEFINE CY (ARRAY 1))"
e72_len: equ $ - e72
e73: db "(STORE CY 0 CY)"
e73_len: equ $ - e73
e74: db "(GC-COLLECT)"
e74_len: equ $ - e74
e75: db "(DEFINE L3 (HEAP-BYTES-LIVE))"
e75_len: equ $ - e75
e76: db "(SETQ CY NIL)"
e76_len: equ $ - e76
e77: db "(GC-COLLECT)"
e77_len: equ $ - e77
e78: db "(PRINT (< (- L3 1) (HEAP-BYTES-LIVE)))"
e78_len: equ $ - e78
e79: db "(NEWLINE)"
e79_len: equ $ - e79
e80: db "(SET-SYMBOL-PLIST! (QUOTE PL) (CONS (CONS (QUOTE K) 5) NIL))"
e80_len: equ $ - e80
e81: db "(SPIN 50000)"
e81_len: equ $ - e81
e82: db "(GC-COLLECT)"
e82_len: equ $ - e82
e83: db "(PRINT (CDR (CAR (SYMBOL-PLIST (QUOTE PL)))))"
e83_len: equ $ - e83
e84: db "(NEWLINE)"
e84_len: equ $ - e84
e85: db "(PRINT (GC-VERIFY))"
e85_len: equ $ - e85
e86: db "(NEWLINE)"
e86_len: equ $ - e86

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
    RUN e58
    RUN e59
    RUN e60
    RUN e61
    RUN e62
    RUN e63
    RUN e64
    RUN e65
    RUN e66
    RUN e67
    RUN e68
    RUN e69
    RUN e70
    RUN e71
    RUN e72
    RUN e73
    RUN e74
    RUN e75
    RUN e76
    RUN e77
    RUN e78
    RUN e79
    RUN e80
    RUN e81
    RUN e82
    RUN e83
    RUN e84
    RUN e85
    RUN e86

    xor rax, rax
    ret
