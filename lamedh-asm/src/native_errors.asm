; native_errors.asm — native-failure-to-condition plumbing (KERNEL.md
; Part VIII lists several native-failure classes a host must signal
; *a* condition for). Two distinct maturity levels currently live here:
;
; - fail_not_callable(): calling a non-callable value. compile_call's
;   trampolines used to blindly dereference whatever tagged value a
;   global cell or an expression happened to hold as if it were a
;   closure — an unbound global defaults to IMM_NIL (tag bits 11), and
;   `and rax, ~TAG_MASK` on that yields a null pointer, so calling one
;   segfaulted on a near-NULL dereference rather than failing in any
;   way a caller could act on. Not yet the real thing — no
;   HANDLER-CASE can catch it — but a labeled, deterministic exit(1)
;   on stderr is a real improvement over undefined behavior, and every
;   call site's own not-a-function check routes here identically.
;
; - native_throw()/fail_wrong_type(): the real thing, for one case so
;   far (CAR/CDR on a non-cons/non-NIL argument, reader.asm). This is
;   what fail_not_callable above should eventually become: a genuine
;   condition, signaled through the exact same CATCH/HANDLER-CASE/
;   ERRORSET machinery Lisp-level ERROR/THROW already use, not a
;   separate, parallel failure path.

%include "src/tags.inc"
%include "src/syscalls.inc"

extern catch_stack
extern catch_stack_top
extern handler_case_tag
extern make_error
extern make_string

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

; native_throw(rdi=tag, rsi=value) — never returns. The exact same
; catch-stack search, restore, and jump compile_throw's *generated*
; code performs for a compiled `(THROW tag value)` form
; (compiler.asm), just written once as an ordinary callable host
; routine instead of re-emitted per call site — so a native routine
; (car/cdr below, so far) can signal into the *same* CATCH/HANDLER-
; CASE/ERRORSET machinery Lisp-level THROW/ERROR already use, with no
; separate, parallel failure path for native code to maintain. Frame
; layout matches compile_throw's own comment exactly: each 32-byte
; catch_stack entry is [0]=tag [8]=saved rbp [16]=saved rsp
; [24]=resume target, searched from the top for an EQ (raw pointer/
; immediate) match. Trapping (int3) on no match is the same v0 failure
; mode compile_throw's own generated code falls back to.
global native_throw
native_throw:
    push rbx
    push r12
    push r13
    mov rbx, rdi                  ; tag to search for
    mov r12, rsi                    ; value to deliver
    mov rax, [rel catch_stack_top]    ; count remaining to search
.loop:
    test rax, rax
    jz .unmatched
    mov r13, rax
    dec r13                            ; index = count - 1 (top frame first)
    mov rcx, r13
    imul rcx, rcx, 32
    lea rdx, [rel catch_stack]
    add rdx, rcx                          ; rdx = frame_addr
    cmp qword [rdx], rbx
    je .match
    mov rax, r13                            ; continue searching below this one
    jmp .loop
.match:
    ; count = index + 1 (pre-bumped): resume lands in CATCH's shared
    ; epilogue, which always does its own "-1" on the way out, matching
    ; compile_throw's own comment on why this isn't a plain "= index".
    lea rax, [r13+1]
    mov [rel catch_stack_top], rax
    mov rcx, [rdx+16]                         ; saved rsp
    mov rsi, [rdx+8]                            ; saved rbp
    mov rdi, [rdx+24]                             ; resume target
    mov rsp, rcx
    mov rbp, rsi
    mov rax, r12                                    ; thrown value
    jmp rdi
.unmatched:
    int3

; fail_wrong_type(rdi=culprit value, rsi=msg ptr, rdx=msg len) — never
; returns. Builds an ordinary two-field condition (conditions.asm's
; make_error: message = the given text, data = the culprit value
; itself) and THROWs it to the shared handler_case_tag(), exactly the
; way a Lisp-level `(ERROR message data)` already does (compile_error,
; compiler.asm) — so `(errorset '(car 5))` now genuinely catches
; something instead of segfaulting. v0 scope, narrower than the Rust
; reference on purpose: the reference's own CAR/CDR error message
; interpolates the culprit's printed representation directly into the
; text ("CAR: expected a list, got 5"); this kernel uses a fixed
; message per caller (see reader.asm's car_err_msg/cdr_err_msg) and
; relies on ERROR-DATA to expose the actual culprit value instead —
; programmatically equivalent, just not textually identical.
global fail_wrong_type
fail_wrong_type:
    push rbx
    push r12
    mov rbx, rdi                  ; culprit -> condition's data field
    mov rdi, rsi
    mov rsi, rdx
    call make_string                ; rax = message string
    mov rdi, rax
    mov rsi, rbx
    call make_error                    ; rax = condition
    mov r12, rax
    call handler_case_tag                 ; rax = shared tag
    mov rdi, rax
    mov rsi, r12
    call native_throw                        ; never returns
    ; unreachable
