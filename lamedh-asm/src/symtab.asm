; symtab.asm — symbol interning: same name always yields the same heap
; object, so EQ on symbols is pointer equality and callers may cache a
; resolved symbol address across calls instead of re-hashing a string.
;
; Symbol object layout (heapobj, tag 10), all fields 8 bytes:
;   [0]  header       = HDR_SYMBOL
;   [8]  name_len
;   [16] value        (tagged LispVal; IMM_UNBOUND until DEFINEd)
;   [24] next         (raw ptr to next symbol in this bucket, or 0)
;   [32] name bytes, padded to a multiple of 8
;
; A fixed 512-bucket direct-chained hash table (FNV-1a), sized generously
; for the fixed test/bench corpus this v0 targets — see README roadmap
; for growth-on-load-factor.

%include "src/tags.inc"

extern data_alloc

section .bss
align 8
global symtab_buckets
symtab_buckets: resq 512

section .text

; bytes_equal(rdi=ptr1, rsi=ptr2, rdx=len) -> rax = 1 if equal else 0
bytes_equal:
    xor rax, rax
.loop:
    test rdx, rdx
    jz .eq
    mov cl, [rdi]
    cmp cl, [rsi]
    jne .ne
    inc rdi
    inc rsi
    dec rdx
    jmp .loop
.eq:
    mov rax, 1
    ret
.ne:
    xor rax, rax
    ret

; fnv1a_hash(rdi=ptr, rsi=len) -> rax = 64-bit hash
fnv1a_hash:
    mov rax, 0xcbf29ce484222325     ; FNV offset basis
    mov r8, 0x100000001b3            ; FNV prime
.loop:
    test rsi, rsi
    jz .done
    movzx rcx, byte [rdi]
    xor rax, rcx
    imul rax, r8
    inc rdi
    dec rsi
    jmp .loop
.done:
    ret

; intern_symbol(rdi=name ptr, rsi=len) -> rax = tagged symbol pointer
global intern_symbol
intern_symbol:
    push rbx
    push r12
    push r13
    push r14
    mov r12, rdi                 ; name ptr
    mov r13, rsi                 ; len

    mov rdi, r12
    mov rsi, r13
    call fnv1a_hash
    and rax, 511
    lea r14, [symtab_buckets + rax*8]   ; r14 = &bucket slot

    mov rbx, [r14]                ; rbx = bucket head (raw ptr or 0)
.scan:
    test rbx, rbx
    jz .not_found
    cmp qword [rbx+8], r13         ; compare name_len
    jne .next
    lea rdi, [rbx+32]
    mov rsi, r12
    mov rdx, r13
    call bytes_equal
    test rax, rax
    jz .next
    ; found: return tagged pointer
    mov rax, rbx
    or rax, TAG_HEAPOBJ
    jmp .out
.next:
    mov rbx, [rbx+24]
    jmp .scan

.not_found:
    ; allocate: 32 header bytes + name, padded to 8
    mov rdi, r13
    add rdi, 7
    and rdi, ~7
    add rdi, 32
    call data_alloc                ; rax = raw new symbol address
    mov rbx, rax

    mov qword [rbx], HDR_SYMBOL
    mov [rbx+8], r13
    mov qword [rbx+16], IMM_UNBOUND
    mov rdi, [r14]
    mov [rbx+24], rdi              ; next = old bucket head
    mov [r14], rbx                 ; bucket head = new symbol

    ; copy name bytes
    lea rdi, [rbx+32]
    mov rsi, r12
    mov rcx, r13
    rep movsb

    mov rax, rbx
    or rax, TAG_HEAPOBJ

.out:
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
