; conditions.asm — the condition system's data half (KERNEL.md Part
; VIII): a condition value is exactly two fields, message and data,
; boxed as a heapobj (HDR_CONDITION: [0]=header [8]=message [16]=data).
; No type tag, no class hierarchy — matching the spec exactly, this is
; deliberately the whole of it. Signaling (ERROR) and catching
; (HANDLER-CASE, ERRORSET) are compiler.asm's job, built on the
; existing CATCH/THROW machinery with a single shared internal tag —
; see compile_handler_case/compile_errorset there.

%include "src/tags.inc"

extern data_alloc
extern rc_inc
extern intern_symbol

section .text

; make_error(rdi=tagged message, rsi=tagged data) -> rax = tagged
; HDR_CONDITION heapobj.
global make_error
make_error:
    push rbx
    push r12
    mov rbx, rdi
    mov r12, rsi
    ; 24, not 16: a condition is [header][message][data] — three words.
    ; The old request was one word short, so the *next* allocation
    ; overlapped this object's data slot and clobbered it. Pre-existing
    ; and invisible under pure bump allocation only because the two
    ; writes raced in the caller's favour often enough; a collector
    ; that enumerates [raw+16] as a child cannot tolerate it at all.
    mov rdi, 24
    call data_alloc
    mov qword [rax], HDR_CONDITION
    mov [rax+8], rbx
    mov [rax+16], r12
    or rax, TAG_HEAPOBJ
    push rax
    mov rdi, rbx
    call rc_inc
    mov rdi, r12
    call rc_inc
    pop rax
    pop r12
    pop rbx
    ret

; is_condition(rdi=tagged value) -> rax=1/0
global is_condition
is_condition:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .no
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_CONDITION
    jne .no
    mov rax, 1
    ret
.no:
    xor rax, rax
    ret

; error_message_tagged(rdi=tagged condition) -> rax = message
global error_message_tagged
error_message_tagged:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+8]
    ret

; error_data_tagged(rdi=tagged condition) -> rax = data
global error_data_tagged
error_data_tagged:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+16]
    ret

; error_of(rdi=tagged value) -> rax = tagged condition: rdi unchanged
; if it's already one (KERNEL.md: "(error c) re-signals it unchanged"
; when c is a condition), else make_error(rdi, NIL) (treating rdi as
; the message, matching "(error message) signals (make-error message)").
; This is the (ERROR c)-with-one-argument builtin's host half — the
; dynamic type check has to happen here, at runtime, because whether c
; is a condition isn't known until the argument's value exists.
global error_of
error_of:
    push rbx
    mov rbx, rdi
    call is_condition
    test rax, rax
    jnz .already
    mov rdi, rbx
    mov rsi, IMM_NIL
    call make_error
    jmp .out
.already:
    mov rax, rbx
.out:
    pop rbx
    ret

; handler_case_tag() -> rax = the single shared interned symbol every
; HANDLER-CASE/ERRORSET installs its catch frame with, and every ERROR
; throws to. One shared tag is safe (not a collision risk) precisely
; because CATCH/THROW's own matching already walks the catch stack
; from the top for the *nearest* frame with a matching tag — the
; dynamically closest HANDLER-CASE always wins, exactly the nesting
; behavior a condition system needs, without each call site needing
; its own distinct tag the way independent CATCH/THROW pairs do.
global handler_case_tag
handler_case_tag:
    mov rdi, handler_case_tag_name
    mov rsi, 18
    jmp intern_symbol

section .rodata
handler_case_tag_name: db "%HANDLER-CASE-TAG%"
