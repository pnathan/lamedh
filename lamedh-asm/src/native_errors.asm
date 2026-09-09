; native_errors.asm — native-failure-to-condition plumbing (KERNEL.md
; Part VIII lists several native-failure classes a host must signal
; *a* condition for): native_throw()/fail_wrong_type(), the real thing,
; a genuine condition signaled through the exact same CATCH/
; HANDLER-CASE/ERRORSET machinery Lisp-level ERROR/THROW already use,
; not a separate, parallel failure path. Two callers use it so far:
; CAR/CDR on a non-cons/non-NIL argument (reader.asm), and calling a
; non-callable value (emit_check_callable, compiler.asm) — the latter
; used to be a separate, cruder fail_not_callable() here (a fixed
; message to stderr plus a hard exit(1), predating native_throw)
; before being switched over to the real thing too.

%include "src/tags.inc"

extern catch_stack
extern catch_stack_top
extern handler_case_tag
extern make_error
extern make_string
extern intern_symbol

section .text

; unwind_protect_marker_tag() -> rax = the single shared interned
; symbol every UNWIND-PROTECT installs its catch-stack marker frame
; with (compile_unwind_protect, compiler.asm) — the same one-shared-
; tag trick handler_case_tag (conditions.asm) already uses, safe for
; the identical reason: native_throw's search below walks the stack
; from the top regardless, so it can recognize *any* marker frame it
; passes on the way to a real match, not just ones a specific
; UNWIND-PROTECT call site is looking for.
global unwind_protect_marker_tag
unwind_protect_marker_tag:
    mov rdi, unwind_marker_tag_name
    mov rsi, 23
    jmp intern_symbol

; invoke_thunk(rdi=tagged closure) -> rax = the closure's own return
; value, called with zero arguments. A tail call into the closure's
; own code pointer (rdi is left exactly as given — the closure's self-
; pointer for reading its own captured free variables, per this
; kernel's calling convention), so the closure's own `ret` returns
; directly to invoke_thunk's own caller. Used for UNWIND-PROTECT
; cleanup thunks, both here (native_throw, when a marker frame is
; passed during an unwind) and in compile_unwind_protect's own normal-
; completion epilogue.
global invoke_thunk
invoke_thunk:
    mov rcx, rdi
    UNTAG_PTR rcx
    mov rcx, [rcx+8]                  ; code ptr
    xor rax, rax                        ; nargs = 0
    jmp rcx

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
;
; UNWIND-PROTECT support (KERNEL.md Part VII): every frame the search
; passes without matching is now checked against
; unwind_protect_marker_tag() above; a match there is not a real catch
; target (nothing ever THROWs to it directly) but a marker
; compile_unwind_protect installed, with the cleanup closure sitting in
; frame[8] where an ordinary frame keeps its saved rbp. Passing one
; during a search fires its cleanup (invoke_thunk) right there, and
; catch_stack_top is shrunk to remove the marker *before* the call —
; not just at the end like an ordinary skipped frame — so a CATCH or
; THROW the cleanup form performs on its own can never re-observe or
; re-fire the very marker it's nested inside. v0 scope: unlike the
; reference, an error raised *by* a cleanup form here is not discarded
; — it propagates as an ordinary new native_throw search of its own,
; which can end up superseding the throw currently being processed
; (see README "KERNEL.md conformance" for the exact tradeoff and why).
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
    mov rcx, [rdx]                          ; rcx = this frame's tag
    cmp rcx, rbx
    je .match

    push rdx                                  ; frame_addr — caller-saved,
    call unwind_protect_marker_tag              ; must survive this call
    mov r8, rax
    pop rdx
    cmp qword [rdx], r8
    jne .continue_search

    ; marker frame: shrink catch_stack_top now (permanently removing
    ; it) before firing its cleanup — see the comment above this
    ; routine for why the ordering matters.
    mov [rel catch_stack_top], r13
    push rbx                                    ; invoke_thunk runs
    push r12                                      ; arbitrary compiled
    push r13                                        ; Lamedh code, which
    mov rdi, [rdx+8]                                  ; this kernel's own
    call invoke_thunk                                   ; codegen never
    pop r13                                               ; promises to
    pop r12                                                 ; preserve —
    pop rbx                                                   ; stack-save
                                                                 ; rather than
                                                                 ; trust any
                                                                 ; register
.continue_search:
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

section .rodata
unwind_marker_tag_name: db "%UNWIND-PROTECT-MARKER%"
