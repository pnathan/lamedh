; 018_fileio — file descriptor I/O as raw syscalls: FD-OPEN/FD-CLOSE/
; FD-WRITE/FD-READ. STDOUT/STDERR need no separate primitive since a
; plain fixnum fd (1 or 2) already works with FD-WRITE. Round-trips a
; scratch file: opens for write, writes a string, closes, reopens for
; read, reads it back, and prints what came back — proving the whole
; write->close->open->read path, not just one half of it.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; FD-WRITE to stderr (fd 2) — a plain fixnum, no separate STDERR
; primitive needed. Written first so the test driver's own stdout
; check isn't polluted by it (run.sh only diffs stdout).
e0: db '(FD-WRITE 2 "to stderr, not stdout")'
e0_len: equ $ - e0

d1: db '(DEFINE PATH "/tmp/lamedh_asm_test_018.txt")'
d1_len: equ $ - d1
d2: db "(DEFINE WFD (FD-OPEN PATH 1))"
d2_len: equ $ - d2
e1: db '(FD-WRITE WFD "round-trip")'
e1_len: equ $ - e1
d3: db "(FD-CLOSE WFD)"
d3_len: equ $ - d3
d4: db "(DEFINE RFD (FD-OPEN PATH 0))"
d4_len: equ $ - d4
d5: db "(DEFINE GOT (FD-READ RFD 64))"
d5_len: equ $ - d5
e2: db "(PRINT GOT)"
e2_len: equ $ - e2
d6: db "(FD-CLOSE RFD)"
d6_len: equ $ - d6
e3: db "(STRING-LENGTH GOT)"
e3_len: equ $ - e3

section .text

run_thunk_discard:
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    ret

extern print_fixnum
run_and_print_fixnum:
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    mov rdi, rax
    call print_fixnum
    call print_newline
    ret

global lamedh_main
lamedh_main:
    mov rdi, e0
    mov rsi, e0_len
    call run_thunk_discard

    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard
    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard
    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard
    mov rdi, d4
    mov rsi, d4_len
    call run_thunk_discard
    mov rdi, d5
    mov rsi, d5_len
    call run_thunk_discard

    mov rdi, e2
    mov rsi, e2_len
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    call print_newline

    mov rdi, d6
    mov rsi, d6_len
    call run_thunk_discard

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum          ; 10

    xor rax, rax
    ret
