; rng.asm — RANDOM/RANDOM-SEED!, matching the Rust reference exactly:
; a SplitMix64 generator (`rng_next`, `../src/evaluator/builtins_extra.rs`)
; over one 64-bit state word, lazily seeded from the clock on first use
; (here: `rdtsc`, since this freestanding binary has no libc clock call
; to make — a cycle counter read needs no syscall at all), or set
; explicitly by RANDOM-SEED!. Same algorithm, same state machine, so a
; program that calls RANDOM-SEED! with a fixed seed sees the same
; sequence on both hosts — the one piece of this that *is* meant to be
; bit-for-bit reproducible, unlike the lazy clock-seeded default case.

%include "src/tags.inc"

extern fail_wrong_type

section .bss
align 8
rng_state: resq 1

section .text

; rng_next() -> rax = raw 64-bit output, advancing rng_state. Not
; itself exposed to Lamedh code — random_tagged below is.
rng_next:
    mov rax, [rng_state]
    test rax, rax
    jnz .seeded
    rdtsc                          ; edx:eax = cycle counter
    shl rdx, 32
    or rax, rdx
    or rax, 1                        ; never leave the state at 0
.seeded:
    mov rcx, 0x9E3779B97F4A7C15
    add rax, rcx
    mov [rng_state], rax
    mov rcx, rax
    shr rcx, 30
    xor rax, rcx
    mov rcx, 0xBF58476D1CE4E5B9
    imul rax, rcx
    mov rcx, rax
    shr rcx, 27
    xor rax, rcx
    mov rcx, 0x94D049BB133111EB
    imul rax, rcx
    mov rcx, rax
    shr rcx, 31
    xor rax, rcx
    ret

; random_tagged(rdi=tagged fixnum n) -> rax = tagged fixnum in [0, n).
; The RANDOM builtin's host half. n must be a positive fixnum — a
; wrong-type or non-positive n is a genuine catchable condition
; (fail_wrong_type/native_throw, native_errors.asm), matching this
; kernel's other native-failure work, not a silent 0 or a crash.
global random_tagged
random_tagged:
    push rbx
    push r12
    mov rbx, rdi
    mov rax, rbx
    and rax, TAG_MASK
    test rax, rax                     ; TAG_FIXNUM == 0
    jnz .bad
    mov rax, rbx
    UNTAG_FIXNUM rax
    cmp rax, 0
    jle .bad
    mov r12, rax                        ; r12 = n (unsigned divisor) —
                                         ; rng_next below uses rcx as
                                         ; scratch internally, so the
                                         ; divisor can't live there
                                         ; across the call
    call rng_next
    mov rcx, r12
    xor rdx, rdx
    div rcx                                ; rdx = rax mod n
    mov rax, rdx
    TO_FIXNUM rax
    pop r12
    pop rbx
    ret
.bad:
    mov rdi, rbx
    pop r12
    pop rbx
    mov rsi, random_range_msg
    mov rdx, random_range_msg_len
    jmp fail_wrong_type

; random_seed_tagged(rdi=tagged fixnum seed) -> rax = rdi unchanged.
; The RANDOM-SEED! builtin's host half; a wrong-type seed is a
; catchable condition, same as random_tagged above.
global random_seed_tagged
random_seed_tagged:
    mov rax, rdi
    and rax, TAG_MASK
    test rax, rax
    jnz .bad
    mov rax, rdi
    UNTAG_FIXNUM rax
    or rax, 1                          ; never leave the state at 0
    mov [rng_state], rax
    mov rax, rdi
    ret
.bad:
    mov rsi, random_seed_msg
    mov rdx, random_seed_msg_len
    jmp fail_wrong_type

section .rodata
random_range_msg: db "RANDOM: expected a positive fixnum"
random_range_msg_len: equ $ - random_range_msg
random_seed_msg: db "RANDOM-SEED!: expected a fixnum"
random_seed_msg_len: equ $ - random_seed_msg
