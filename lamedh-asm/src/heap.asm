; heap.asm — two bump-allocated arenas, no GC (v0 scope; see README roadmap).
;
;   data heap: PROT_READ|PROT_WRITE   — cons cells, symbols, strings.
;   code heap: PROT_READ|WRITE|EXEC   — compiled native functions.
;
; The code heap is mapped RWX and stays RWX for the process lifetime: this
; is the substrate self-modifying code needs. Patching an already-emitted
; call site (inline caching, on-stack respecialization) rewrites bytes in
; a page the CPU can fetch from at the same time — precisely the freedom
; a strict Harvard split (or a W^X-hardened OS) forbids. Same-core writes
; to code about to be fetched are made visible by the next control transfer
; through that address (Intel SDM Vol.3 §8.1.3 "self-modifying code" —
; a jmp/call/ret is itself the serializing event here), so no explicit
; cache-flush instruction is required on this architecture.

%include "src/syscalls.inc"

section .bss
align 8
global data_heap_base
global data_heap_cur
global data_heap_end
global code_heap_base
global code_heap_cur
global code_heap_end
data_heap_base: resq 1
data_heap_cur:  resq 1
data_heap_end:  resq 1
code_heap_base: resq 1
code_heap_cur:  resq 1
code_heap_end:  resq 1

section .text

; heap_init_all(rdi = data heap bytes, rsi = code heap bytes)
;
; Internal calling convention (this project has no C ABI to honor, so we
; define our own and hold to it consistently): rbx, rbp, r12-r15 are
; callee-saved across lamedh-asm functions; everything else is scratch.
global heap_init_all
heap_init_all:
    push rbx
    push r12
    mov r12, rdi                 ; save data_len
    mov rbx, rsi                 ; save code_len

    ; --- data heap: RW, private anonymous mmap ---
    xor edi, edi                  ; addr = NULL
    mov rsi, r12
    mov edx, PROT_READ | PROT_WRITE
    mov r10d, MAP_PRIVATE | MAP_ANONYMOUS
    mov r8d, -1                   ; fd
    xor r9d, r9d                  ; offset
    mov eax, SYS_mmap
    syscall
    mov [data_heap_base], rax
    mov [data_heap_cur], rax
    add rax, r12
    mov [data_heap_end], rax

    ; --- code heap: RWX, private anonymous mmap ---
    xor edi, edi
    mov rsi, rbx
    mov edx, PROT_READ | PROT_WRITE | PROT_EXEC
    mov r10d, MAP_PRIVATE | MAP_ANONYMOUS
    mov r8d, -1
    xor r9d, r9d
    mov eax, SYS_mmap
    syscall
    mov [code_heap_base], rax
    mov [code_heap_cur], rax
    add rax, rbx
    mov [code_heap_end], rax

    pop r12
    pop rbx
    ret

; data_alloc(rdi = size in bytes) -> rax = raw pointer, 16-byte aligned bump
global data_alloc
data_alloc:
    add rdi, 15
    and rdi, ~15
    mov rax, [data_heap_cur]
    add rax, rdi
    ; (no bounds check in v0 — a fixed arena is pre-sized generously;
    ;  see README roadmap for growth/GC.)
    mov [data_heap_cur], rax
    sub rax, rdi
    ret

; code_alloc(rdi = size in bytes) -> rax = raw pointer, 16-byte aligned bump
global code_alloc
code_alloc:
    add rdi, 15
    and rdi, ~15
    mov rax, [code_heap_cur]
    add rax, rdi
    mov [code_heap_cur], rax
    sub rax, rdi
    ret
