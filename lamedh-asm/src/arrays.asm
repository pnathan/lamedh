; arrays.asm — the array value type: a fixed-length, index-addressable
; slot vector (HDR_ARRAY: [0]=header [8]=len [16..]=len*8 tagged slot
; bytes, each slot one full tagged value). Fixed size at creation (no
; grow/shrink), and — unlike every value type before it — mutable:
; STORE rewrites a slot in place. Nothing else in this kernel
; mutates a heap value after creation (captured closure values are
; copied by value; the hash table library that used to sit on top of
; just CONS/CAR/CDR was persistent for exactly this reason — no
; RPLACD-equivalent existed). Arrays are that primitive, scoped as
; narrowly as possible: index-addressable in-place slot update, and
; nothing else.
;
; This is what makes a *real* hash table possible as library code: a
; bucket array plus small per-bucket alist chains, both built from
; primitives already in this kernel (CONS for the chains, STORE for
; the bucket slots) — see tests/cases/021_hashtable_array.asm and the
; README's kernel-surface section.

%include "src/tags.inc"

extern data_alloc
extern rc_store_slot
extern intern_symbol
extern make_float
extern float_val
extern fail_wrong_type
extern cons

section .text

; record_brand_tagged(rdi=tagged value) -> rax = the record's brand
; symbol, or IMM_NIL if not a record (RECORD-BRAND, matching the
; reference's own "non-record argument -> NIL" contract, no error).
global record_brand_tagged
record_brand_tagged:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .no
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_RECORD
    jne .no
    mov rax, [rax+8]
    ret
.no:
    mov rax, IMM_NIL
    ret

; record_fields_tagged(rdi=tagged record) -> rax = tagged list of the
; record's own field values, in declared order (RECORD-FIELDS). v0
; scope: no type check on a non-record argument (this host has no
; general error-signaling convenience for "wrong record-ish shape"
; yet outside CAR/CDR/CALL's own dedicated checks) — every real call
; site (record accessors, VARIANT-CASE's own dispatch) already knows
; its argument is a record by construction.
global record_fields_tagged
record_fields_tagged:
    push rbx
    push r12
    mov rbx, rdi
    UNTAG_PTR rbx
    mov r12, [rbx+16]                 ; nfields
    mov rax, IMM_NIL
    dec r12
.loop:
    cmp r12, 0
    jl .done
    mov rdi, [rbx+24+r12*8]
    mov rsi, rax
    call cons
    dec r12
    jmp .loop
.done:
    pop r12
    pop rbx
    ret

; make_array(rdi=tagged fixnum n) -> rax = tagged HDR_ARRAY heapobj, n
; slots, each initialized to IMM_NIL.
global make_array
make_array:
    push rbx
    push r12
    mov rax, rdi
    UNTAG_FIXNUM rax
    mov r12, rax                     ; raw n
    mov rax, r12
    shl rax, 3
    add rax, 16
    mov rdi, rax
    call data_alloc
    mov rbx, rax
    mov qword [rbx], HDR_ARRAY
    mov [rbx+8], r12
    xor rcx, rcx
.init:
    cmp rcx, r12
    jae .done
    mov qword [rbx+16+rcx*8], IMM_NIL
    inc rcx
    jmp .init
.done:
    mov rax, rbx
    or rax, TAG_HEAPOBJ
    pop r12
    pop rbx
    ret

; is_array(rdi=tagged value) -> rax=1/0
global is_array
is_array:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .no
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_ARRAY
    jne .no
    mov rax, 1
    ret
.no:
    xor rax, rax
    ret

; is_typed_array(rdi=tagged value) -> rax=1/0
global is_typed_array
is_typed_array:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .no
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_TYPED_ARRAY
    jne .no
    mov rax, 1
    ret
.no:
    xor rax, rax
    ret

; make_typed_array(rdi=tagged fixnum n, rsi=tagged elem-type symbol) ->
; rax = tagged HDR_TYPED_ARRAY heapobj, n raw slots zero-initialized
; (0 for INT64, 0.0's all-zero bit pattern for FLOAT64 — the same raw
; value either way, so the zero-init loop doesn't need to branch on
; the type). elem-type must be exactly the symbol INT64 or FLOAT64
; (the reader case-folds symbols on intern already, so this needs no
; extra case handling); anything else is a catchable wrong-type
; condition (fail_wrong_type/native_throw), matching KERNEL.md Part IV
; exactly ("any other symbol or a non-symbol is an error").
global make_typed_array
make_typed_array:
    push rbx
    push r12
    push r13
    mov r12, rdi                  ; tagged n
    mov r13, rsi                    ; elem-type symbol (for the error
                                     ; path below, since intern_symbol
                                     ; inside typed_array_elem_type_of
                                     ; would otherwise clobber rsi)
    mov rdi, rsi
    call typed_array_elem_type_of     ; -> rax = 0 (INT64) / 1 (FLOAT64) / -1 (bad)
    cmp rax, 0
    jl .bad
    mov rbx, rax                        ; elem_type (0/1)

    mov rax, r12
    UNTAG_FIXNUM rax
    push rax                            ; [raw n]
    shl rax, 3
    add rax, 24
    mov rdi, rax
    call data_alloc
    pop rcx                               ; raw n
    mov qword [rax], HDR_TYPED_ARRAY
    mov [rax+8], rcx
    mov [rax+16], rbx
    xor rdx, rdx
.init:
    cmp rdx, rcx
    jae .done
    mov qword [rax+24+rdx*8], 0
    inc rdx
    jmp .init
.done:
    or rax, TAG_HEAPOBJ
    pop r13
    pop r12
    pop rbx
    ret
.bad:
    mov rdi, r13
    pop r13
    pop r12
    pop rbx
    mov rsi, typed_array_type_msg
    mov rdx, typed_array_type_msg_len
    jmp fail_wrong_type

; typed_array_elem_type_of(rdi=tagged symbol) -> rax = 0 (INT64), 1
; (FLOAT64), or -1 (neither).
typed_array_elem_type_of:
    push rbx
    mov rbx, rdi
    mov rdi, int64_name
    mov rsi, 5
    call intern_symbol
    cmp rax, rbx
    je .is_int64
    mov rdi, float64_name
    mov rsi, 7
    call intern_symbol
    cmp rax, rbx
    je .is_float64
    mov rax, -1
    pop rbx
    ret
.is_int64:
    xor rax, rax
    pop rbx
    ret
.is_float64:
    mov rax, 1
    pop rbx
    ret

; array_length_tagged(rdi=tagged array) -> rax = tagged fixnum length.
global array_length_tagged
array_length_tagged:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+8]
    TO_FIXNUM rax
    ret

; array_ref(rdi=tagged array-or-typed-array, rsi=tagged fixnum index)
; -> rax = tagged value at that slot. No bounds check (v0 — see
; README; typed arrays share this same divergence, not a new one).
; FETCH is one polymorphic primitive over both representations
; (KERNEL.md Part XI): a plain array's slots are already tagged
; values, returned as-is; a typed array's slots are raw untagged
; int64/double words — "reading always yields the declared type", so
; INT64 re-tags the raw word as a fixnum and FLOAT64 boxes it fresh
; via make_float, on every read.
global array_ref
array_ref:
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_TYPED_ARRAY
    je .typed
    mov rcx, rsi
    UNTAG_FIXNUM rcx
    mov rax, [rax+16+rcx*8]
    ret
.typed:
    push rbx
    mov rbx, rax                  ; raw typed-array address
    mov rcx, rsi
    UNTAG_FIXNUM rcx
    cmp qword [rbx+16], 0           ; elem_type: 0=INT64
    jne .typed_float
    mov rax, [rbx+24+rcx*8]
    TO_FIXNUM rax
    pop rbx
    ret
.typed_float:
    movsd xmm0, [rbx+24+rcx*8]
    pop rbx
    jmp make_float

; array_set(rdi=tagged array-or-typed-array, rsi=tagged fixnum index,
; rdx=tagged value) -> rax = value (also stored into the slot). A
; plain array stores the tagged value as-is (no type check — the
; first primitive in this kernel that mutates a heap object after
; creation). A typed array validates per KERNEL.md Part XI's own
; narrower-than-arithmetic-coercion rule: an INT64 array accepts only
; a fixnum (a Float or Char is a wrong-type condition — Char is
; deliberately *not* coerced to its code point here); a FLOAT64 array
; accepts a Float (stored as-is, including NaN) or a fixnum (converted
; to the nearest f64), and rejects a Char. Storage is always the raw
; untagged word either way, matching FETCH's own unboxing.
global array_set
array_set:
    push rbx
    mov rbx, rdi
    UNTAG_PTR rbx
    cmp qword [rbx], HDR_TYPED_ARRAY
    je .typed
    mov rcx, rsi
    UNTAG_FIXNUM rcx
    ; A plain array slot is a counted heap slot: releasing the value it
    ; held is what makes (STORE a i NIL) actually drop that value's
    ; last heap reference. (The typed-array branches below store raw
    ; untagged int64/double words, which are not references at all.)
    lea rdi, [rbx+16+rcx*8]
    mov rsi, rdx
    push rdx
    call rc_store_slot
    pop rdx
    mov rax, rdx
    pop rbx
    ret
.typed:
    mov rcx, rsi
    UNTAG_FIXNUM rcx
    cmp qword [rbx+16], 0           ; elem_type: 0=INT64
    jne .typed_float
    mov rax, rdx
    and rax, TAG_MASK
    test rax, rax                     ; TAG_FIXNUM == 0
    jnz .bad
    mov rax, rdx
    UNTAG_FIXNUM rax
    mov [rbx+24+rcx*8], rax
    mov rax, rdx
    pop rbx
    ret
.typed_float:
    ; a Float stores as-is; a fixnum converts to the nearest f64;
    ; anything else (a Char, a string, ...) is a wrong-type condition.
    mov rax, rdx
    and rax, TAG_MASK
    test rax, rax                       ; TAG_FIXNUM == 0
    jz .float_from_fixnum
    cmp rax, TAG_HEAPOBJ
    jne .bad
    mov rax, rdx
    UNTAG_PTR rax
    cmp qword [rax], HDR_FLOAT
    jne .bad
    push rdx
    mov rdi, rdx
    call float_val                        ; xmm0 = value
    pop rdx
    movsd [rbx+24+rcx*8], xmm0
    mov rax, rdx
    pop rbx
    ret
.float_from_fixnum:
    push rdx
    mov rax, rdx
    UNTAG_FIXNUM rax
    cvtsi2sd xmm0, rax
    pop rdx
    movsd [rbx+24+rcx*8], xmm0
    mov rax, rdx
    pop rbx
    ret
.bad:
    mov rdi, rdx
    pop rbx
    mov rsi, typed_array_store_msg
    mov rdx, typed_array_store_msg_len
    jmp fail_wrong_type

; hash_code_tagged(rdi=tagged value) -> rax = tagged fixnum hash code,
; always non-negative, masked to 30 bits so a caller can safely scale
; or add it without overflowing a fixnum. Well-defined for fixnum keys
; (their own magnitude), strings (content, fnv1a_hash — symtab.asm),
; floats (the raw bit pattern, normalized first so this agrees with
; lisp_eq's own EQ rule: -0.0 is folded to +0.0's bits since the two
; are EQ, and every NaN bit pattern is folded to one canonical value
; since KERNEL.md Part IV makes NaN EQ to NaN regardless of payload
; bits), and any other heapobj (its own stable heap address, since
; nothing in this kernel relocates a heap value once allocated) — in
; practice symbols and closures/arrays, where only pointer identity is
; ever EQ anyway, so an address-derived hash cannot disagree with EQ
; the way a content-blind hash on strings/floats used to (HASH-CODE
; must agree with EQ for the hash table library, lib/prelude.lisp's
; own SETHASH/GETHASH, to work at all — two EQ keys landing in
; different buckets would make a stored value unfindable). Two conses
; or two structurally-but-not-pointer-equal values are NOT guaranteed
; the same hash, since only EQ, not full structural equality, exists
; as a kernel primitive. Anything else (a cons, an immediate other
; than a fixnum) hashes to a constant 0 — correct but degenerate (a
; single bucket).
extern fnv1a_hash
global hash_code_tagged
hash_code_tagged:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_FIXNUM
    je .fixnum_key
    cmp rax, TAG_HEAPOBJ
    je .heapobj_key
    xor rax, rax
    TO_FIXNUM rax
    ret
.fixnum_key:
    mov rax, rdi
    UNTAG_FIXNUM rax
    mov rcx, rax
    sar rcx, 63                  ; rcx = -1 if negative, else 0
    xor rax, rcx
    sub rax, rcx                   ; rax = |value|
    and rax, 0x3FFFFFFF
    TO_FIXNUM rax
    ret
.heapobj_key:
    push rbx
    mov rbx, rdi
    UNTAG_PTR rbx
    mov rax, [rbx]
    cmp rax, HDR_STRING
    je .string_key
    cmp rax, HDR_FLOAT
    je .float_key
    mov rax, rbx
    shr rax, 4                       ; heap addresses are 16-byte aligned
    and rax, 0x3FFFFFFF
    TO_FIXNUM rax
    pop rbx
    ret
.string_key:
    mov rdx, [rbx+8]                   ; len
    lea rdi, [rbx+16]
    mov rsi, rdx
    call fnv1a_hash
    and rax, 0x3FFFFFFF
    TO_FIXNUM rax
    pop rbx
    ret
.float_key:
    mov rax, [rbx+8]                   ; raw double bits
    ; NaN: exponent all-1 (bits 62..52) and a non-zero mantissa —
    ; fold every payload/sign variant to one canonical bit pattern so
    ; every NaN hashes identically, matching float_eq_exact's own
    ; "NaN is EQ to NaN regardless of bits" rule.
    mov rcx, rax
    mov rdx, 0x7FF0000000000000
    and rcx, rdx
    cmp rcx, rdx
    jne .not_nan
    mov rcx, rax
    and rcx, 0x000FFFFFFFFFFFFF
    test rcx, rcx
    jz .not_nan
    mov rax, 0x7FF8000000000000          ; canonical NaN bit pattern
    jmp .have_bits
.not_nan:
    ; -0.0 and +0.0 are EQ (IEEE ==) but differ only in the sign bit —
    ; fold both to the same all-zero bits so they hash equal. Only a
    ; zero magnitude gets folded: every other value's sign bit is part
    ; of a genuinely different (and correctly non-EQ) number, so it
    ; must stay significant to the hash.
    mov rcx, rax
    and rcx, 0x7FFFFFFFFFFFFFFF     ; magnitude, sign bit cleared
    test rcx, rcx
    jnz .have_bits
    xor rax, rax
.have_bits:
    mov rcx, rax
    shr rcx, 32
    xor rax, rcx
    and rax, 0x3FFFFFFF
    TO_FIXNUM rax
    pop rbx
    ret

; mod_tagged(rdi=tagged a, rsi=tagged b) -> rax = tagged (a mod b), the
; Euclidean remainder: always in 0 <= r < |b| (KERNEL.md Part V — this
; is MOD, distinct from REMAINDER below, and the two disagree exactly
; when the operands' signs differ).
global mod_tagged
mod_tagged:
    mov rax, rdi
    UNTAG_FIXNUM rax
    mov rcx, rsi
    UNTAG_FIXNUM rcx
    cqo
    idiv rcx                     ; rdx = truncated remainder (sign of a)
    test rdx, rdx
    jns .nonneg
    mov rax, rcx
    test rax, rax
    jns .babs
    neg rax
.babs:
    add rdx, rax                   ; rdx += |b|
.nonneg:
    mov rax, rdx
    TO_FIXNUM rax
    ret

; remainder_tagged(rdi=tagged a, rsi=tagged b) -> rax = tagged
; (a remainder b), the truncated remainder: sign follows the dividend,
; not the divisor (KERNEL.md Part V — REMAINDER, distinct from MOD).
global remainder_tagged
remainder_tagged:
    mov rax, rdi
    UNTAG_FIXNUM rax
    mov rcx, rsi
    UNTAG_FIXNUM rcx
    cqo
    idiv rcx
    mov rax, rdx
    TO_FIXNUM rax
    ret

section .rodata
int64_name:   db "INT64"
float64_name: db "FLOAT64"
typed_array_type_msg: db "TYPED-ARRAY: elem-type must be INT64 or FLOAT64"
typed_array_type_msg_len: equ $ - typed_array_type_msg
typed_array_store_msg: db "STORE: value does not match the typed array's element type"
typed_array_store_msg_len: equ $ - typed_array_store_msg
