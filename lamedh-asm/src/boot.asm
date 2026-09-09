; boot.asm — process entry point. Freestanding: no libc, no crt0.
;
; Captures argc/argv (only available, per the x86-64 System V ABI, from
; the *original* stack pointer at process entry — a "call" instruction
; would push a return address on top of it and make it unreachable),
; sets up the two heaps, then hands off to `lamedh_main` (provided by
; whichever driver is linked in — a test case or file_runner.asm) and
; exits with the raw byte it returns in rax.

%include "src/syscalls.inc"

%define DATA_HEAP_BYTES  (16 * 1024 * 1024)
%define CODE_HEAP_BYTES  (16 * 1024 * 1024)

extern heap_init_all
extern lamedh_main
extern bootstrap_globals

section .bss
align 8
; program_argc/program_argv: this process's argc and a pointer to its
; argv[0] (Linux's own layout: an array of argc pointers, no trailing
; NULL counted). Read by file_runner.asm's lamedh_main; a test-case
; lamedh_main simply never references them. Global (not passed in a
; register to lamedh_main) so every driver — none of which take
; arguments today — keeps its existing zero-argument signature.
global program_argc
global program_argv
program_argc: resq 1
program_argv: resq 1

section .text
global _start
_start:
    xor rbp, rbp                  ; mark outermost frame for gdb/backtraces

    mov rax, [rsp]                  ; argc
    mov [program_argc], rax
    lea rax, [rsp+8]                  ; &argv[0]
    mov [program_argv], rax

    mov rdi, DATA_HEAP_BYTES
    mov rsi, CODE_HEAP_BYTES
    call heap_init_all

    call bootstrap_globals            ; binds the symbol T to itself

    call lamedh_main               ; rax = exit code

    mov rdi, rax
    mov eax, SYS_exit
    syscall
    ; unreachable
