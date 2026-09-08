; print.asm — decimal/textual output for observing values in tests.
; No libc formatting: hand-rolled itoa writing straight to a stack buffer,
; then one write(2) syscall.

%include "src/tags.inc"
%include "src/syscalls.inc"

section .text

; write_buf(rsi = buf, rdx = len) -> writes to stdout. Clobbers rax,rdi,rcx,r11.
global write_buf
write_buf:
    mov rdi, STDOUT
    mov eax, SYS_write
    syscall
    ret

; print_newline() -> writes a single '\n'
global print_newline
print_newline:
    push rbx
    sub rsp, 8
    mov byte [rsp], 10
    mov rsi, rsp
    mov rdx, 1
    call write_buf
    add rsp, 8
    pop rbx
    ret

; print_fixnum(rdi = tagged fixnum) -> writes its decimal value, no newline.
; Handles zero and negative values. Clobbers rax,rcx,rdx,rsi,r8,r9,r10,r11.
global print_fixnum
print_fixnum:
    push rbx
    mov rax, rdi
    UNTAG_FIXNUM rax           ; rax = signed machine integer

    sub rsp, 32                ; scratch buffer, digits built end-to-first
    lea rsi, [rsp+31]          ; rsi = write cursor, starts at buffer end
    mov byte [rsi], 0          ; not strictly needed; defensive terminator
    xor r8, r8                 ; r8 = sign flag (1 if negative)

    test rax, rax
    jns .not_neg
    mov r8, 1
    neg rax
.not_neg:
    mov r9, 10                  ; divisor

    test rax, rax
    jnz .loop
    ; value is exactly zero
    dec rsi
    mov byte [rsi], '0'
    jmp .have_digits

.loop:
    test rax, rax
    jz .have_digits
    xor rdx, rdx
    div r9                       ; rax = rax/10, rdx = rax%10
    add dl, '0'
    dec rsi
    mov [rsi], dl
    jmp .loop

.have_digits:
    test r8, r8
    jz .emit
    dec rsi
    mov byte [rsi], '-'

.emit:
    lea rdx, [rsp+31]
    sub rdx, rsi                ; length = end - start
    ; rsi already points at the first digit within the buffer
    call write_buf
    add rsp, 32
    pop rbx
    ret
