; boot.asm — process entry point. Freestanding: no libc, no crt0.
;
; Captures argc/argv (only available, per the x86-64 System V ABI, from
; the *original* stack pointer at process entry — a "call" instruction
; would push a return address on top of it and make it unreachable),
; sets up the two heaps, then hands off to `lamedh_main` (provided by
; whichever driver is linked in — a test case or file_runner.asm) and
; exits with the raw byte it returns in rax.

%include "src/syscalls.inc"

; 256 MiB data heap (up from 16 MiB): loading the full accumulated
; reference stdlib (33+ files) plus PRINC-TO-STRING's own per-call
; CAPTURE_BUF_BYTES allocation (print.asm, 64KB, which at the time was
; never freed — that buffer is now data_alloc_raw'd and explicitly
; freed, gc.asm) inside a heavy WITH-MODULE body (e.g.
; lib/31-ports.lisp's 24 exported functions, each renamed via
; SEXPR-RENAME calling PRINC-TO-STRING per reference) genuinely
; exhausted the old 16 MiB arena, corrupting subsequent data_alloc
; calls into unmapped memory rather than erroring — found by bisecting
; an apparently load-order/count-dependent crash down to data_alloc
; simply running out of room. Both heaps are anonymous mmap
; reservations (virtual address space only; physical pages are
; committed lazily as touched), so this costs nothing until actually
; used.
%define DATA_HEAP_BYTES  (256 * 1024 * 1024)
%define CODE_HEAP_BYTES  (64 * 1024 * 1024)

extern heap_init_all
extern stack_base
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

    ; The collector's conservative root scan runs from the current rsp
    ; up to here: the process's original stack pointer, the one word
    ; above every frame this program will ever push. Captured here for
    ; the same reason argc/argv are — after any `call`, the original
    ; value is no longer recoverable (gc.asm).
    mov [stack_base], rsp

    mov rdi, DATA_HEAP_BYTES
    mov rsi, CODE_HEAP_BYTES
    call heap_init_all

    call bootstrap_globals            ; binds the symbol T to itself

    call lamedh_main               ; rax = exit code

    mov rdi, rax
    mov eax, SYS_exit
    syscall
    ; unreachable
