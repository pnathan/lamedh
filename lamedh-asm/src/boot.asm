; boot.asm — process entry point. Freestanding: no libc, no crt0.
;
; Sets up the two heaps, then hands off to `lamedh_main` (provided by
; whichever driver is linked in — a test case or the future REPL) and
; exits with the raw byte it returns in rax.

%include "src/syscalls.inc"

%define DATA_HEAP_BYTES  (16 * 1024 * 1024)
%define CODE_HEAP_BYTES  (16 * 1024 * 1024)

extern heap_init_all
extern lamedh_main

section .text
global _start
_start:
    xor rbp, rbp                  ; mark outermost frame for gdb/backtraces

    mov rdi, DATA_HEAP_BYTES
    mov rsi, CODE_HEAP_BYTES
    call heap_init_all

    call lamedh_main               ; rax = exit code

    mov rdi, rax
    mov eax, SYS_exit
    syscall
    ; unreachable
