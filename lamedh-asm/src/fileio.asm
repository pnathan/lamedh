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

extern data_alloc
extern make_string
extern string_len
extern string_bytes

section .text

; file_open(rdi=tagged string path, rsi=tagged mode fixnum) -> rax =
; tagged fd fixnum (negative on error, e.g. -2 = ENOENT, same as errno
; but negated — no errno translation is done).
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
    mov rdx, O_RDONLY
    jmp .have_flags
.write_mode:
    mov rdx, O_WRONLY | O_CREAT | O_TRUNC
    jmp .have_flags
.append_mode:
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
; Writes the string's raw bytes to fd in one syscall; does not loop on
; a short write (v0 — fine for a pipe/regular file under this project's
; own message sizes, see README roadmap).
global file_write
file_write:
    push rbx
    push r12
    mov rbx, rdi                     ; tagged fd
    mov r12, rsi                       ; tagged string
    mov rdi, r12
    call string_len
    mov rdx, rax                            ; len
    mov rdi, r12
    call string_bytes
    mov rsi, rax                               ; buf
    mov rax, rbx
    UNTAG_FIXNUM rax
    mov rdi, rax                                  ; fd
    mov eax, SYS_write
    syscall
    mov rax, r12
    pop r12
    pop rbx
    ret

; file_read(rdi=tagged fd fixnum, rsi=tagged max-len fixnum) -> rax =
; tagged string of however many bytes were actually read — shorter
; than max-len at EOF, empty (not NIL) at EOF-with-nothing-left. A
; negative syscall result (an error) is folded to an empty string
; rather than propagated (v0 — no error signaling yet, see README).
global file_read
file_read:
    push rbx
    push r12
    push r13
    mov rbx, rdi                      ; tagged fd
    mov rax, rsi
    UNTAG_FIXNUM rax
    mov r12, rax                        ; raw maxlen

    mov rdi, r12
    call data_alloc                        ; scratch buffer
    mov r13, rax

    mov rax, rbx
    UNTAG_FIXNUM rax
    mov rdi, rax                              ; fd
    mov rsi, r13                                ; buf
    mov rdx, r12                                  ; count
    mov eax, SYS_read
    syscall                                          ; rax = bytes read (or <0)
    cmp rax, 0
    jns .ok
    xor rax, rax
.ok:
    mov rdi, r13
    mov rsi, rax
    call make_string
    pop r13
    pop r12
    pop rbx
    ret
