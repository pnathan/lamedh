; print.asm — decimal/textual output for observing values in tests.
; No libc formatting: hand-rolled itoa writing straight to a stack buffer,
; then one write(2) syscall.

%include "src/tags.inc"
%include "src/syscalls.inc"

extern data_alloc_raw
extern data_free
extern print_value
extern make_string

section .bss
align 8
; Redirects write_buf into an in-memory buffer instead of stdout, for
; princ_to_string below — every leaf of print_value's dispatch already
; funnels through write_buf, so this one indirection is enough to turn
; "print a value" into "render a value to a string" with no change to
; print_value/print_string/print_symbol/print_list/print_fixnum/
; float_print at all. Not a stack (a single flat state, like
; reader_buf/pos/end in reader.asm) — princ_to_string itself saves and
; restores it around its own use, so nested calls (a value containing
; something that itself calls PRINC-TO-STRING) still work.
capture_active: resq 1
capture_buf:    resq 1
capture_len:    resq 1
capture_cap:    resq 1

section .text

; write_buf(rsi = buf, rdx = len) -> writes to stdout, or appends to the
; active capture buffer (see princ_to_string) when one is armed,
; silently truncating past its fixed v0 capacity rather than growing it
; or erroring. Clobbers rax,rdi,rcx,r11 on the stdout path (unchanged);
; also rbx,r8,r9 on the capture path.
global write_buf
write_buf:
    cmp qword [capture_active], 0
    jne .capture
    mov rdi, STDOUT
    mov eax, SYS_write
    syscall
    ret
.capture:
    push rbx
    mov rbx, rsi                      ; src buf
    mov r8, [capture_len]
    mov r9, [capture_cap]
    sub r9, r8
    cmp rdx, r9
    jbe .fits
    mov rdx, r9                         ; truncate to remaining capacity
.fits:
    mov rax, [capture_buf]
    add rax, r8
    xor rcx, rcx
.copy:
    cmp rcx, rdx
    jae .done
    mov r9b, [rbx+rcx]
    mov [rax+rcx], r9b
    inc rcx
    jmp .copy
.done:
    add [capture_len], rdx
    pop rbx
    ret

; princ_to_string(rdi=tagged value) -> rax = a fresh tagged HDR_STRING
; heapobj, the same bytes PRINT would write for this value (PRINT's own
; "aesthetic," not "readable," convention — a string's raw bytes, no
; quoting; KERNEL.md Part XI's PRINC-TO-STRING). This is what lets
; `(format nil ...)` return a string instead of writing to a stream:
; the same FORMAT macro expansion that calls PRINT for `(format t ...)`
; calls STRING-APPEND/princ_to_string instead for a nil stream.
%define CAPTURE_BUF_BYTES (64 * 1024)
global princ_to_string
princ_to_string:
    push rbx
    mov rbx, rdi                        ; value

    ; save the previous capture state (nested PRINC-TO-STRING calls —
    ; a value that itself prints via one — must not clobber an
    ; enclosing call's own buffer), same reasoning as READ-FROM-STRING
    ; saving/restoring the reader's own global position (reader.asm).
    push qword [capture_active]
    push qword [capture_buf]
    push qword [capture_len]
    push qword [capture_cap]

    ; A raw scratch buffer, not a Lisp object: nothing ever holds a
    ; reference to it past make_string's copy below, so it is
    ; data_alloc_raw'd and explicitly freed rather than left for the
    ; collector — this is the 64 KB-per-call leak the README names
    ; (docs/spec-tco-capture-gc.md 3.4, the **R** rows).
    mov rdi, CAPTURE_BUF_BYTES
    call data_alloc_raw
    mov [capture_buf], rax
    mov qword [capture_len], 0
    mov qword [capture_cap], CAPTURE_BUF_BYTES
    mov qword [capture_active], 1

    mov rdi, rbx
    call print_value

    mov rdi, [capture_buf]
    mov rsi, [capture_len]
    call make_string
    mov rbx, rax                          ; result string

    mov rdi, [capture_buf]
    call data_free                          ; clobbers nothing

    pop qword [capture_cap]
    pop qword [capture_len]
    pop qword [capture_buf]
    pop qword [capture_active]

    mov rax, rbx
    pop rbx
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
