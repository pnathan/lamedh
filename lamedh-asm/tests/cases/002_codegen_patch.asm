; 002_codegen_patch — emit two native functions into the RWX code heap at
; runtime, wire the caller's `call` site to a *wrong* target, execute
; nothing yet, then rewrite that call site's rel32 in place with
; patch_rel32 and only then invoke it. This is the inline-cache mechanism
; in miniature: a call site is emitted once and its target is decided —
; and rewritten — after the fact, in code that is simultaneously
; executable and writable.

%include "src/tags.inc"

extern codegen_here
extern emit8
extern emit32
extern emit64
extern patch_rel32
extern print_fixnum
extern print_newline

section .text
global lamedh_main
lamedh_main:
    push rbx
    push r12
    push r13

    ; --- callee: mov rax, <tagged 99>; ret ---
    call codegen_here
    mov r12, rax                 ; r12 = callee address

    mov rdi, 0x48
    call emit8                   ; REX.W prefix
    mov rdi, 0xB8
    call emit8                   ; B8 = MOV rax, imm64
    mov rdi, 99
    TO_FIXNUM rdi
    call emit64                  ; imm64 = tagged 99
    mov rdi, 0xC3
    call emit8                   ; RET

    ; --- caller: call <deliberately wrong target>; ret ---
    call codegen_here
    mov r13, rax                 ; r13 = caller address

    mov rdi, 0xE8
    call emit8                   ; E8 = CALL rel32
    call codegen_here
    mov rbx, rax                 ; rbx = address of the rel32 field itself
    mov rdi, 0                   ; placeholder displacement — deliberately wrong
    call emit32
    mov rdi, 0xC3
    call emit8                   ; RET

    ; Rewrite the call site now that the callee's real address is known.
    ; The bytes at rbx were already assembled and are inside a page the
    ; CPU can execute from; we are about to overwrite them and then jump
    ; straight through them.
    mov rdi, rbx
    mov rsi, r12
    call patch_rel32

    ; Execute the freshly self-modified caller.
    call r13

    mov rdi, rax
    call print_fixnum
    call print_newline

    xor rax, rax
    pop r13
    pop r12
    pop rbx
    ret
