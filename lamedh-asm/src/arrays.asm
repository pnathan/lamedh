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

section .text

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

; array_length_tagged(rdi=tagged array) -> rax = tagged fixnum length.
global array_length_tagged
array_length_tagged:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+8]
    TO_FIXNUM rax
    ret

; array_ref(rdi=tagged array, rsi=tagged fixnum index) -> rax = tagged
; value at that slot. No bounds check (v0 — see README).
global array_ref
array_ref:
    mov rax, rdi
    UNTAG_PTR rax
    mov rcx, rsi
    UNTAG_FIXNUM rcx
    mov rax, [rax+16+rcx*8]
    ret

; array_set(rdi=tagged array, rsi=tagged fixnum index, rdx=tagged
; value) -> rax = value (also stored into the slot). The first
; primitive in this kernel that mutates a heap object after creation.
global array_set
array_set:
    mov rax, rdi
    UNTAG_PTR rax
    mov rcx, rsi
    UNTAG_FIXNUM rcx
    mov [rax+16+rcx*8], rdx
    mov rax, rdx
    ret

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
