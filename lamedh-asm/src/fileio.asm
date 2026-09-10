; fileio.asm — file descriptor I/O: open/close/read/write as raw Linux
; syscalls, no libc, no buffering. Every fd is a plain tagged fixnum;
; STDIN/STDOUT/STDERR (0/1/2) are already valid fds under this scheme,
; so stdout/stderr need no separate primitive — (FD-WRITE 1 s) and
; (FD-WRITE 2 s) already work.
;
; FD-OPEN's mode is a plain fixnum, not a symbol: a symbol argument in
; call position would be read as a *variable reference* (compile_form's
; global/local lookup), not the symbol's own identity — avoiding that
; ambiguity is simpler than requiring the caller to quote it.
;   0 = read-only
;   1 = write (create/truncate)
;   2 = append (create if absent)

%include "src/tags.inc"
%include "src/syscalls.inc"

extern data_alloc_raw
extern data_free
extern make_string
extern string_len
extern string_bytes
extern require_capability
extern fail_wrong_type

section .text

; file_open(rdi=tagged string path, rsi=tagged mode fixnum) -> rax =
; tagged fd fixnum (negative on error, e.g. -2 = ENOENT, same as errno
; but negated — no errno translation is done). Capability-gated
; (KERNEL.md Part IX): mode 0 (read) requires READ-FS (bit 0), modes 1
; and 2 (write/append, both O_CREAT) require CREATE-FS (bit 1) —
; enforced at this call site via require_capability
; (capabilities.asm), which never returns (a catchable condition, the
; same fail_wrong_type/native_throw machinery every other native
; failure this kernel signals uses) if the capability isn't currently
; granted and unmasked. Acquisition is the gate — file_read/file_write/
; file_close on an already-open fd are not re-gated, matching the
; spec's own "operations on an already-acquired handle... are not
; re-gated" rule exactly.
global file_open
file_open:
    push rbx
    push r12
    mov r12, rsi                      ; tagged mode
    call string_bytes                    ; rdi (tagged path, still entry
                                          ; value) -> rax = raw path ptr
    mov rbx, rax                            ; path ptr

    mov rax, r12
    UNTAG_FIXNUM rax
    cmp rax, 1
    je .write_mode
    cmp rax, 2
    je .append_mode
    mov rdi, 0                              ; READ-FS bit
    call require_capability
    mov rdx, O_RDONLY
    jmp .have_flags
.write_mode:
    mov rdi, 1                              ; CREATE-FS bit
    call require_capability
    mov rdx, O_WRONLY | O_CREAT | O_TRUNC
    jmp .have_flags
.append_mode:
    mov rdi, 1                              ; CREATE-FS bit
    call require_capability
    mov rdx, O_WRONLY | O_CREAT | O_APPEND
.have_flags:
    mov rdi, rbx
    mov rsi, rdx
    mov rdx, 420                          ; mode bits if created: 0644
    mov eax, SYS_open
    syscall
    TO_FIXNUM rax
    pop r12
    pop rbx
    ret

; file_close(rdi=tagged fd fixnum) -> rax = tagged result fixnum (0 on
; success, negative on error).
global file_close
file_close:
    mov rax, rdi
    UNTAG_FIXNUM rax
    mov rdi, rax
    mov eax, SYS_close
    syscall
    TO_FIXNUM rax
    ret

; file_write(rdi=tagged fd fixnum, rsi=tagged string) -> rax = the
; string itself (PRINT's own return-what-you-were-given convention).
; Writes the string's raw bytes to fd, looping on a short write until
; every byte is out (a pipe or a terminal can accept less than asked);
; a negative result (an error) is a real condition, data = the negated
; errno as a fixnum. This is the one write primitive: stdout and
; stderr are fds 1 and 2, and the prelude's WRITE-STRING/WRITE-LINE
; are plain Lisp over it.
global file_write
file_write:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi                     ; tagged fd
    mov r12, rsi                       ; tagged string
    mov rdi, r12
    call string_len
    mov r14, rax                            ; bytes left
    mov rdi, r12
    call string_bytes
    mov r13, rax                               ; cursor
.loop:
    test r14, r14
    jz .done
    mov rax, rbx
    UNTAG_FIXNUM rax
    mov rdi, rax                                  ; fd
    mov rsi, r13
    mov rdx, r14
    mov eax, SYS_write
    syscall
    cmp rax, 0
    jl .error
    add r13, rax
    sub r14, rax
    jmp .loop
.done:
    mov rax, r12
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.error:
    mov rdi, rax
    TO_FIXNUM rdi                                 ; -errno, tagged
    mov rsi, write_err_msg
    mov rdx, write_err_msg_len
    call fail_wrong_type                          ; never returns

; file_read(rdi=tagged fd fixnum, rsi=tagged max-len fixnum) -> rax =
; tagged string of however many bytes ONE read(2) returned — up to
; max-len, fewer when fewer were available (a pipe, a terminal line),
; empty (not NIL) at end of input. Exactly read(2)'s own contract; a
; caller wanting a line or a whole file loops (the prelude's
; FD-READ-LINE does). A negative result is a real condition, data =
; the negated errno. Reading fd 0 requires the IO capability (the
; reference's own "stdin-consuming read operations" gate; a file fd
; was gated when FD-OPEN acquired it).
global file_read
file_read:
    push rbx
    push r12
    push r13
    mov rbx, rdi                      ; tagged fd
    mov rax, rsi
    UNTAG_FIXNUM rax
    mov r12, rax                        ; raw maxlen
    mov rax, rbx
    UNTAG_FIXNUM rax
    test rax, rax
    jnz .gated
    mov rdi, 4                          ; IO capability bit
    call require_capability
.gated:
    mov rdi, r12
    call data_alloc_raw                    ; scratch buffer, explicitly freed
    mov r13, rax
    mov rax, rbx
    UNTAG_FIXNUM rax
    mov rdi, rax                              ; fd
    mov rsi, r13                                ; buf
    mov rdx, r12                                  ; count
    mov eax, SYS_read
    syscall                                          ; rax = bytes read (or <0)
    cmp rax, 0
    jl .error
    mov rdi, r13
    mov rsi, rax
    call make_string
    push rax
    mov rdi, r13
    call data_free
    pop rax
    pop r13
    pop r12
    pop rbx
    ret
.error:
    push rax
    mov rdi, r13
    call data_free
    pop rdi
    TO_FIXNUM rdi                                 ; -errno, tagged
    mov rsi, read_err_msg
    mov rdx, read_err_msg_len
    call fail_wrong_type                          ; never returns

section .rodata
write_err_msg: db "FD-WRITE: write failed (data: -errno)"
write_err_msg_len: equ $ - write_err_msg
read_err_msg: db "FD-READ: read failed (data: -errno)"
read_err_msg_len: equ $ - read_err_msg
section .text
