; arrays.asm — the array value type: a fixed-length, index-addressable
; slot vector (HDR_ARRAY: [0]=header [8]=len [16..]=len*8 tagged slot
; bytes, each slot one full tagged value). Fixed size at creation (no
; grow/shrink), and — unlike every value type before it — mutable:
; ARRAY-SET rewrites a slot in place. Nothing else in this kernel
; mutates a heap value after creation (captured closure values are
; copied by value; the hash table library that used to sit on top of
; just CONS/CAR/CDR was persistent for exactly this reason — no
; RPLACD-equivalent existed). Arrays are that primitive, scoped as
; narrowly as possible: index-addressable in-place slot update, and
; nothing else.
;
; This is what makes a *real* hash table possible as library code: a
; bucket array plus small per-bucket alist chains, both built from
; primitives already in this kernel (CONS for the chains, ARRAY-SET for
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
; (their own magnitude) and any heapobj (its own stable heap address,
; since nothing in this kernel relocates a heap value once allocated) —
; which in practice means symbols, the hash table library's only
; well-distributed key type; strings/floats/arrays/closures also hash
; (by address, not content — two equal-content strings hash
; differently) rather than erroring, but two conses or two structurally
; equal values are NOT guaranteed the same hash, since only EQ (pointer
; identity), not structural equality, exists in this kernel. Anything
; else (a cons, an immediate other than a fixnum) hashes to a constant
; 0 — correct but degenerate (a single bucket).
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
    mov rax, rdi
    UNTAG_PTR rax
    shr rax, 4                       ; heap addresses are 16-byte aligned
    and rax, 0x3FFFFFFF
    TO_FIXNUM rax
    ret

; mod_tagged(rdi=tagged a, rsi=tagged b) -> rax = tagged (a mod b).
; Assumes both are non-negative (true of everything hash_code_tagged
; produces and every bucket count this library uses) — no defensive
; sign correction, unlike a general-purpose MOD.
global mod_tagged
mod_tagged:
    mov rax, rdi
    UNTAG_FIXNUM rax
    mov rcx, rsi
    UNTAG_FIXNUM rcx
    cqo
    idiv rcx
    mov rax, rdx
    TO_FIXNUM rax
    ret
