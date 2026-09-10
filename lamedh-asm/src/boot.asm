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
    call install_segv_handler

    call bootstrap_globals            ; binds the symbol T to itself

    call lamedh_main               ; rax = exit code

    mov rdi, rax
    mov eax, SYS_exit
    syscall
    ; unreachable

; ---------------------------------------------------------------------
; SIGSEGV reporting. Compiled code runs on the process's own native
; stack with no guard of its own, so a deep enough non-tail recursion
; ends in SIGSEGV — which, uncaught, is a bare "Segmentation fault"
; from the shell (exit 139) with nothing to say what happened. A
; handler on its own alternate stack (the main one is exactly what has
; just run out) writes one line to STDERR naming the most likely cause
; and then exits with the same 139 the shell would have reported, so
; nothing that was checking that status sees a difference.
%define SIGSEGV          11
%define SYS_rt_sigaction 13
%define SYS_rt_sigreturn 15
%define SYS_sigaltstack  131
%define SA_ONSTACK       0x08000000
%define SA_RESTORER      0x04000000
%define SEGV_STACK_BYTES 65536

section .bss
segv_stack: resb SEGV_STACK_BYTES

section .rodata
segv_msg: db "lamedhc: fatal: SIGSEGV - most likely the native stack is exhausted (deep non-tail recursion; see README v0 limits)", 10
segv_msg_len: equ $ - segv_msg

section .text
install_segv_handler:
    ; sigaltstack({ss_sp, ss_flags=0, ss_size}, NULL)
    sub rsp, 24
    lea rax, [rel segv_stack]
    mov [rsp], rax
    mov qword [rsp+8], 0
    mov qword [rsp+16], SEGV_STACK_BYTES
    mov rdi, rsp
    xor esi, esi
    mov eax, SYS_sigaltstack
    syscall
    add rsp, 24
    ; rt_sigaction(SIGSEGV, {handler, flags, restorer, mask=0}, NULL, 8)
    sub rsp, 32
    lea rax, [rel segv_handler]
    mov [rsp], rax
    mov qword [rsp+8], SA_ONSTACK | SA_RESTORER
    lea rax, [rel segv_restorer]
    mov [rsp+16], rax
    mov qword [rsp+24], 0
    mov edi, SIGSEGV
    mov rsi, rsp
    xor edx, edx
    mov r10d, 8
    mov eax, SYS_rt_sigaction
    syscall
    add rsp, 32
    ret

segv_handler:
    mov edi, STDERR
    lea rsi, [rel segv_msg]
    mov edx, segv_msg_len
    mov eax, SYS_write
    syscall
    mov edi, 139
    mov eax, SYS_exit
    syscall
    ; unreachable

segv_restorer:                        ; never actually returned into
    mov eax, SYS_rt_sigreturn
    syscall
