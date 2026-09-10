; syscall.asm — the raw-syscall escape hatch: (SYSCALL nr arg1 ... arg6)
; performs Linux system call NR with up to six arguments and returns
; the raw result as a fixnum (negative = -errno, exactly what the
; kernel returns). Its purpose is to keep OS surface OUT of this file
; and out of assembly altogether: SHELL, CHMOD, FILE-P and anything
; else a program needs from the OS are plain Lisp in lib/prelude.lisp
; over this one primitive.
;
; Argument conversion (syscall_tagged, below):
;   fixnum            -> the machine word itself
;   string            -> the address of its bytes (every string this
;                        kernel builds is NUL-terminated past its own
;                        length, strings.asm, so it serves as a C path
;                        as-is; and as an OUTPUT buffer, since read(2)
;                        and friends write straight into it — pair with
;                        (MAKE-STRING n) and STRING-REF, lib/prelude.lisp)
;   NIL               -> 0 (a NULL pointer)
;   list of strings   -> a fresh NULL-terminated array of byte pointers
;                        (execve's argv/envp), freed after the call
;   anything else     -> a real condition
;
; Gated on the SHELL capability (bit 3) at every call: arbitrary OS
; access is at least as powerful as running a shell. Anything built on
; it therefore needs SHELL too, even FILE-P and CHMOD, which the
; reference gates on READ-FS/CREATE-FS — the one deliberate coarsening
; this design accepts in exchange for no per-feature assembly.
;
; The result is tagged with TO_FIXNUM: a pointer-valued result (mmap)
; fits the 62-bit fixnum range on x86-64 (47-bit user addresses).

%include "src/tags.inc"

extern require_capability
extern fail_wrong_type
extern data_alloc_raw
extern data_free
extern is_string
extern string_bytes
extern car
extern cdr

section .rodata
syscall_arg_msg: db "SYSCALL: argument must be a fixnum, string, NIL, or list of strings"
syscall_arg_msg_len: equ $ - syscall_arg_msg
syscall_nr_msg: db "SYSCALL: system call number must be a fixnum"
syscall_nr_msg_len: equ $ - syscall_nr_msg
syscall_argc_msg: db "SYSCALL: at most six arguments"
syscall_argc_msg_len: equ $ - syscall_argc_msg

section .text

; is_cons_raw(rdi=tagged) -> rax = 1/0
is_cons_raw:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_CONS
    sete al
    movzx rax, al
    ret

; syscall_arg_word(rdi=tagged value) -> rax = machine word,
; rdx = an allocated pointer array to free afterwards, or 0.
syscall_arg_word:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi
    xor r14, r14                          ; nothing to free
    mov rax, rbx
    and rax, TAG_MASK
    jnz .not_fixnum
    mov rax, rbx
    UNTAG_FIXNUM rax
    jmp .out
.not_fixnum:
    cmp rbx, IMM_NIL
    jne .not_nil
    xor eax, eax
    jmp .out
.not_nil:
    mov rdi, rbx
    call is_string
    test rax, rax
    jz .not_string
    mov rdi, rbx
    call string_bytes
    jmp .out
.not_string:
    mov rdi, rbx
    call is_cons_raw
    test rax, rax
    jz .bad
    ; a list of strings -> NULL-terminated char*[]
    xor r12, r12                          ; count
    mov r13, rbx
.count:
    mov rdi, r13
    call is_cons_raw
    test rax, rax
    jz .counted
    inc r12
    mov rdi, r13
    call cdr
    mov r13, rax
    jmp .count
.counted:
    lea rdi, [r12*8+8]
    call data_alloc_raw
    mov r14, rax                          ; the array (to free)
    mov r13, rbx
    xor r12, r12
.fill:
    mov rdi, r13
    call is_cons_raw
    test rax, rax
    jz .filled
    mov rdi, r13
    call car
    mov rdi, rax
    push rax
    call is_string
    pop rdi
    test rax, rax
    jz .bad
    call string_bytes
    mov [r14+r12*8], rax
    inc r12
    mov rdi, r13
    call cdr
    mov r13, rax
    jmp .fill
.filled:
    mov qword [r14+r12*8], 0
    mov rax, r14
.out:
    mov rdx, r14
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.bad:
    mov rdi, rbx
    mov rsi, syscall_arg_msg
    mov rdx, syscall_arg_msg_len
    call fail_wrong_type                  ; never returns

; syscall_tagged(rdi=list (nr arg1 ... arg6)) -> rax = tagged fixnum result
global syscall_tagged
syscall_tagged:
    push rbx
    push r12
    push r13
    push r14
    push r15
    sub rsp, 112                          ; [0..48) words, [56..104) frees, [104] nr
    mov rbx, rdi
    mov rdi, 3                            ; SHELL capability bit
    call require_capability
    ; number
    mov rdi, rbx
    call is_cons_raw
    test rax, rax
    jz .bad_nr
    mov rdi, rbx
    call car
    mov r12, rax
    and r12, TAG_MASK
    jnz .bad_nr
    UNTAG_FIXNUM rax
    mov [rsp+104], rax
    mov rdi, rbx
    call cdr
    mov rbx, rax                          ; args cursor
    xor r13, r13                          ; arg index
.args:
    mov rdi, rbx
    call is_cons_raw
    test rax, rax
    jz .args_done
    cmp r13, 6
    jae .too_many
    mov rdi, rbx
    call car
    mov rdi, rax
    call syscall_arg_word
    mov [rsp+r13*8], rax
    mov [rsp+56+r13*8], rdx
    inc r13
    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .args
.args_done:
    ; unset arguments are 0
.zero_rest:
    cmp r13, 6
    jae .go
    mov qword [rsp+r13*8], 0
    mov qword [rsp+56+r13*8], 0
    inc r13
    jmp .zero_rest
.go:
    mov rdi, [rsp]
    mov rsi, [rsp+8]
    mov rdx, [rsp+16]
    mov r10, [rsp+24]
    mov r8, [rsp+32]
    mov r9, [rsp+40]
    mov rax, [rsp+104]
    syscall
    mov r15, rax
    ; free any pointer arrays built for list arguments
    xor r13, r13
.free:
    cmp r13, 6
    jae .freed
    mov rdi, [rsp+56+r13*8]
    test rdi, rdi
    jz .next_free
    call data_free
.next_free:
    inc r13
    jmp .free
.freed:
    mov rax, r15
    TO_FIXNUM rax
    add rsp, 112
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.bad_nr:
    mov rdi, rbx
    mov rsi, syscall_nr_msg
    mov rdx, syscall_nr_msg_len
    call fail_wrong_type
.too_many:
    mov rdi, rbx
    mov rsi, syscall_argc_msg
    mov rdx, syscall_argc_msg_len
    call fail_wrong_type
