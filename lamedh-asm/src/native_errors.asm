; native_errors.asm — a controlled failure for "not a function"
; (KERNEL.md Part VIII lists calling a non-callable value among the
; native-failure classes a host must signal *a* condition for), until
; this kernel has real native-failure-to-condition plumbing. Before
; this, compile_call's trampolines blindly dereferenced whatever tagged
; value a global cell or an expression happened to hold as if it were a
; closure: an unbound global defaults to IMM_NIL (tag bits 11), and
; `and rax, ~TAG_MASK` on that yields a null pointer, so calling one
; segfaulted on a near-NULL dereference rather than failing in any way
; a caller could act on. This is not yet the real thing — no
; HANDLER-CASE can catch it, matching README's own "no native failure
; signals a condition yet" scope — but a labeled, deterministic exit(1)
; on stderr is a real improvement over undefined behavior, and every
; call site's own not-a-function check now routes here identically.

%include "src/tags.inc"
%include "src/syscalls.inc"

section .rodata
not_callable_msg: db "lamedh-asm: not a function", 10
not_callable_msg_len: equ $ - not_callable_msg

section .text

; fail_not_callable() — never returns. No arguments: the message is
; deliberately generic rather than trying to re-derive and print the
; culprit value here, since doing that safely (this may run with the
; data/code heaps in an arbitrary mid-call state) is more machinery
; than a v0 diagnostic needs.
global fail_not_callable
fail_not_callable:
    mov edi, STDERR
    mov rsi, not_callable_msg
    mov edx, not_callable_msg_len
    mov eax, SYS_write
    syscall
    mov edi, 1
    mov eax, SYS_exit
    syscall
    ; unreachable
