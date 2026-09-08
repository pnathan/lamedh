; tags.inc — the lamedh-asm value representation.
;
; Every Lisp value is one 64-bit word. The low 2 bits are the tag:
;
;   00  fixnum     — bits 63:2 are a signed integer (62-bit range)
;   01  cons       — bits 63:2 (<<2) are a heap address of a 16-byte cell
;   10  heapobj    — bits 63:2 (<<2) are a heap address; word at [addr] is
;                    a header tag (HDR_* below) naming the object's kind
;   11  immediate  — bits 63:2 enumerate a fixed, small constant set
;
; Fixnum arithmetic exploits the zero low bits directly: for tag 00,
; (a<<2) + (b<<2) == (a+b)<<2 with the tag untouched, so ADD/SUB need no
; untag/retag dance at all. Only IMUL needs a post-shift correction, and
; comparisons work on the shifted representation unchanged (the shift is
; order-preserving for a two's-complement machine word).
;
; All heap objects are allocated 8-byte aligned (in fact 16-byte, see
; heap.asm), so the low 2 (in fact low 4) address bits are always free
; for tagging: tagging is a pure OR, untagging a pure AND/shift.

%define TAG_MASK        3
%define TAG_FIXNUM       0
%define TAG_CONS         1
%define TAG_HEAPOBJ      2
%define TAG_IMMEDIATE    3

; --- immediates (tag 11), enumerated in bits 63:2 ---
%define IMM_NIL         ( (0 << 2) | TAG_IMMEDIATE )   ; 3
%define IMM_TRUE        ( (1 << 2) | TAG_IMMEDIATE )   ; 7
%define IMM_FALSE       ( (2 << 2) | TAG_IMMEDIATE )   ; 11
%define IMM_UNBOUND     ( (3 << 2) | TAG_IMMEDIATE )   ; 15
%define IMM_EOF         ( (4 << 2) | TAG_IMMEDIATE )   ; 19

; --- heapobj header tags (word at [addr], untagged raw address) ---
%define HDR_SYMBOL      1     ; [8 name-len][8*n name bytes][8 value][8 next-in-bucket]
%define HDR_CLOSURE     2     ; [8 native-code-ptr][8 nargs][8 nfree][8*nfree captured]
%define HDR_STRING      3     ; [8 len][len bytes, padded]
%define HDR_FLOAT       4     ; [8 bytes: one IEEE754 double]

; fixnum <-> machine-integer conversion
%macro TO_FIXNUM 1        ; reg <- (reg << 2), tag 00 is already 0 after shift
    shl %1, 2
%endmacro

%macro UNTAG_FIXNUM 1     ; reg <- (reg >> 2) arithmetic, sign-preserving
    sar %1, 2
%endmacro

%macro UNTAG_PTR 1        ; reg <- reg & ~TAG_MASK  (strip low 2 tag bits)
    and %1, ~TAG_MASK
%endmacro
