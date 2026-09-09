; capabilities.asm — Part IX capability-gated I/O: a standing grant
; plus a dynamic-extent attenuation mask, enforced at the primitive
; call site (file_open, fileio.asm, so far — the only host-facing I/O
; this kernel has). "Per-thread" in the spec collapses to plain global
; state here, since this is a single-threaded freestanding binary with
; no environment-as-value and no threading primitive — a documented
; simplification in the same spirit as this kernel's other narrowed-
; but-honest v0 divergences (see README).
;
; Twelve capability names, each one bit: READ-FS CREATE-FS TEMP-FS
; SHELL IO NET-DNS NET-CONNECT NET-LISTEN OS-ENV OS-ENV-WRITE
; OS-PROCESS OS-SIGNAL — exactly KERNEL.md Part IX's own list, in that
; order (bit 0 = READ-FS ... bit 11 = OS-SIGNAL). Only READ-FS and
; CREATE-FS are actually enforced anywhere yet, since this kernel has
; no shell/network/process/env primitives at all to gate — the other
; ten bits exist so FEATURE-ENABLED-P/CAPABILITY-MASK-ALLOWS-P/
; WITH-CAPABILITIES already behave correctly for names a future
; primitive will need, rather than needing another format change then.
;
; The standing grant defaults to *all* capabilities, matching the
; reference CLI's own default (AGENTS.md: "enables all capabilities by
; default (use --sandbox for none)") — lamedhc has no --sandbox flag
; yet (see README roadmap), so there is currently no Lisp- or CLI-
; facing way to narrow the standing grant at all, only the dynamic
; WITH-CAPABILITIES mask (which can only ever narrow, per spec, never
; widen it back).

%include "src/tags.inc"

extern intern_symbol
extern fail_wrong_type
extern car
extern cdr

section .data
align 8
capability_grant: dq 0xFFF        ; all 12 bits granted by default
capability_mask:  dq -1           ; -1 = no active fence (unrestricted)

section .bss
align 8
mask_stack: resq 64
mask_stack_top: resq 1

section .text

; capability_bit_of(rdi=tagged symbol) -> rax = bit index 0..11, or -1
; if not a recognized capability name (or not a symbol at all — v0
; scope: only symbol arguments are supported, not the spec's own
; "symbol or string" for FEATURE-ENABLED-P; a string argument here
; behaves as "unrecognized" rather than being case-folded and matched).
; Interning is canonical (same name always yields the same heap
; object), so this just interns each candidate name and compares by
; pointer — no string comparison needed against the caller's own value.
capability_bit_of:
    push rbx
    push r12
    mov rbx, rdi                  ; candidate
    xor r12, r12                    ; index
.loop:
    cmp r12, 12
    jae .not_found
    mov rax, r12
    shl rax, 4                        ; index*16 (two qwords per entry)
    lea rdx, [rel cap_name_table]
    add rdx, rax
    mov rdi, [rdx]                      ; name ptr
    mov rsi, [rdx+8]                      ; name len
    call intern_symbol
    cmp rax, rbx
    je .found
    inc r12
    jmp .loop
.found:
    mov rax, r12
    pop r12
    pop rbx
    ret
.not_found:
    mov rax, -1
    pop r12
    pop rbx
    ret

; require_capability(rdi=bit index) — never returns if the capability
; is not currently allowed (standing grant AND dynamic mask both must
; permit it); falls through (returns) otherwise. The one enforcement
; point every gated primitive calls before acting — file_open
; (fileio.asm) is the only caller so far.
global require_capability
require_capability:
    push rbx
    mov rbx, rdi                  ; bit index
    mov rcx, rbx
    mov rax, 1
    shl rax, cl                       ; rax = 1 << bit
    mov rdx, [rel capability_grant]
    and rdx, rax
    jz .denied
    mov rdx, [rel capability_mask]
    and rdx, rax
    jz .denied
    pop rbx
    ret
.denied:
    mov rdi, rbx                    ; culprit: the bit index (a fixnum,
                                     ; not the capability name — v0,
                                     ; ERROR-DATA exposes the index
                                     ; rather than a friendlier symbol)
    TO_FIXNUM rdi
    pop rbx
    mov rsi, capability_denied_msg
    mov rdx, capability_denied_msg_len
    jmp fail_wrong_type

; feature_enabled_p(rdi=tagged symbol) -> rax = IMM_TRUE/IMM_NIL. T
; when the name is granted by the standing grant *and* not masked by
; the current fence (KERNEL.md Part IX). An unrecognized name is NIL.
global feature_enabled_p
feature_enabled_p:
    call capability_bit_of
    cmp rax, 0
    jl .no
    mov rcx, rax
    mov rax, 1
    shl rax, cl
    mov rdx, [rel capability_grant]
    and rdx, rax
    jz .no
    mov rdx, [rel capability_mask]
    and rdx, rax
    jz .no
    mov rax, IMM_TRUE
    ret
.no:
    mov rax, IMM_NIL
    ret

; capability_mask_allows_p(rdi=tagged symbol) -> rax = IMM_TRUE/
; IMM_NIL. Checks only the dynamic mask layer (ignoring the standing
; grant) — T when no fence is active, or the active fence permits the
; name. An unrecognized name is NIL.
global capability_mask_allows_p
capability_mask_allows_p:
    call capability_bit_of
    cmp rax, 0
    jl .no
    mov rcx, rax
    mov rax, 1
    shl rax, cl
    mov rdx, [rel capability_mask]
    and rdx, rax
    jz .no
    mov rax, IMM_TRUE
    ret
.no:
    mov rax, IMM_NIL
    ret

; list_to_capability_mask(rdi=tagged list of symbols) -> rax = a raw
; (untagged) bitmask, the OR of each element's bit. v0 scope: an
; unrecognized element (not a known capability name, or not a symbol)
; is silently skipped rather than signaling the spec's own "a
; non-symbol is an error" — WITH-CAPABILITIES below is meant for
; trusted embedding/library code narrowing its own authority, not a
; boundary this kernel treats as adversarial input yet.
list_to_capability_mask:
    push rbx
    push r12
    mov rbx, rdi                  ; list cursor
    xor r12, r12                    ; accumulated mask
.loop:
    cmp rbx, IMM_NIL
    je .done
    mov rdi, rbx
    call car
    mov rdi, rax
    call capability_bit_of
    cmp rax, 0
    jl .skip
    mov rcx, rax
    mov rax, 1
    shl rax, cl
    or r12, rax
.skip:
    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .loop
.done:
    mov rax, r12
    pop r12
    pop rbx
    ret

; push_capability_mask(rdi=tagged list of symbols) -> rax = ignored.
; The PUSH-CAPABILITY-MASK! builtin's host half: computes the
; requested mask, intersects it with the currently active mask (or
; takes it outright when no mask is active — capability_mask == -1),
; saves the *previous* mask on mask_stack, and installs the new one.
; WITH-CAPABILITIES (lib/prelude.lisp) pairs this with
; POP-CAPABILITY-MASK! inside an UNWIND-PROTECT, so the previous mask
; is restored on every exit path — normal completion, an error, or any
; other non-local exit — without this primitive needing to know
; anything about how the body exits.
global push_capability_mask
push_capability_mask:
    call list_to_capability_mask
    mov rcx, rax                    ; requested mask
    mov rax, [rel capability_mask]    ; current (possibly -1 = unset)
    mov rdx, [rel mask_stack_top]
    lea r8, [rel mask_stack]
    mov [r8+rdx*8], rax                ; save previous mask
    inc rdx
    mov [rel mask_stack_top], rdx
    cmp rax, -1
    jne .intersect
    mov rax, rcx
    jmp .install
.intersect:
    and rax, rcx
.install:
    mov [rel capability_mask], rax
    ret

; pop_capability_mask() -> rax = ignored. Restores the mask
; push_capability_mask most recently saved.
global pop_capability_mask
pop_capability_mask:
    mov rdx, [rel mask_stack_top]
    dec rdx
    mov [rel mask_stack_top], rdx
    lea r8, [rel mask_stack]
    mov rax, [r8+rdx*8]
    mov [rel capability_mask], rax
    ret

section .rodata
cap_name_table:
    dq cn_read_fs, 7
    dq cn_create_fs, 9
    dq cn_temp_fs, 7
    dq cn_shell, 5
    dq cn_io, 2
    dq cn_net_dns, 7
    dq cn_net_connect, 11
    dq cn_net_listen, 10
    dq cn_os_env, 6
    dq cn_os_env_write, 12
    dq cn_os_process, 10
    dq cn_os_signal, 9
cn_read_fs:      db "READ-FS"
cn_create_fs:    db "CREATE-FS"
cn_temp_fs:      db "TEMP-FS"
cn_shell:        db "SHELL"
cn_io:           db "IO"
cn_net_dns:      db "NET-DNS"
cn_net_connect:  db "NET-CONNECT"
cn_net_listen:   db "NET-LISTEN"
cn_os_env:       db "OS-ENV"
cn_os_env_write: db "OS-ENV-WRITE"
cn_os_process:   db "OS-PROCESS"
cn_os_signal:    db "OS-SIGNAL"
capability_denied_msg: db "capability not granted or masked"
capability_denied_msg_len: equ $ - capability_denied_msg
