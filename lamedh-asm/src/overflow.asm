; overflow.asm — KERNEL.md Part V / Part XII axis 1's OVERFLOW signal.
;
; Fixnum +/- on this kernel run directly on the tagged (shifted-left-by-2)
; representation (compile_binop, compiler.asm): the tag bits are zero and
; cancel out, so `add`/`sub` on the raw 64-bit tagged words is bit-for-bit
; the same operation the reference's own 64-bit wraparound model performs,
; just on a value pre-multiplied by 4. This is the exact "tag-cancellation
; wraparound arithmetic" KERNEL.md Part I names as one of the two reasons
; axis 1 exists: the arithmetic already wraps; what Part V additionally
; requires of the fixed-width model is that the wrap be *observable*.
;
; This file supplies that observability: one global flag, set by target
; code compile_binop now emits right after a compiled `+`/`-` whose
; native OF flag comes back set, queried with FLAG-SET-P and cleared with
; CLEAR-FLAG/CLEAR-ALL-FLAGS — the exact three names Part V's own prose
; gives (`(flag-set-p 'OVERFLOW)`, `(clear-flag 'OVERFLOW)`,
; `(clear-all-flags)`). `*` is deliberately not wired to this yet: its
; compiled form runs `imul` on the tagged (already-shifted) operands and
; then corrects with a `sar` (see compile_binop), so the hardware OF from
; the `imul` itself reflects overflow of the pre-correction, extra-shifted
; product, not of the represented fixnum multiplication — reusing it here
; would just be a different bug wearing this feature's name, not the same
; fix `+`/`-` get. That gap is tracked as a follow-up, not silently
; dropped.

%include "src/tags.inc"

extern intern_symbol

section .bss
overflow_flag: resq 1

section .text

; set_overflow_flag() — called from *compiled* target code, right after
; an emitted +/- whose native OF flag came back set (compile_binop).
; No arguments and no meaningful return value: every general-purpose
; register is caller-saved in this ABI, so the emitted call site saves
; and restores its own result register (rax) around this call itself;
; this routine is free to clobber whatever it likes.
global set_overflow_flag
set_overflow_flag:
    mov qword [rel overflow_flag], 1
    ret

; overflow_sym() -> rax = the interned OVERFLOW symbol, for identity
; comparison against a FLAG-SET-P/CLEAR-FLAG argument. Re-interning on
; every call is a few extra cycles, not a new symbol object (intern_symbol
; already dedupes) — simplicity over caching a global for the one flag
; name this kernel has.
overflow_sym:
    mov rdi, ovf_name
    mov rsi, 8
    jmp intern_symbol

; flag_set_p(rdi=tagged value) -> rax = IMM_TRUE if rdi is the symbol
; OVERFLOW and the flag is currently set, else IMM_NIL. A non-OVERFLOW
; argument is well-defined, not an error: this kernel has exactly one
; flag, and asking about any other name is simply always NIL.
global flag_set_p
flag_set_p:
    push rbx
    mov rbx, rdi
    call overflow_sym
    cmp rax, rbx
    jne .no
    cmp qword [rel overflow_flag], 0
    je .no
    mov rax, IMM_TRUE
    jmp .out
.no:
    mov rax, IMM_NIL
.out:
    pop rbx
    ret

; clear_flag(rdi=tagged value) -> rax = IMM_NIL. Clears OVERFLOW when
; rdi names it; any other argument is a silent no-op, matching SETQ's
; own "unknown name is never an error" convention (KERNEL.md Part VI).
global clear_flag
clear_flag:
    push rbx
    mov rbx, rdi
    call overflow_sym
    cmp rax, rbx
    jne .out
    mov qword [rel overflow_flag], 0
.out:
    mov rax, IMM_NIL
    pop rbx
    ret

; clear_all_flags() -> rax = IMM_NIL.
global clear_all_flags
clear_all_flags:
    mov qword [rel overflow_flag], 0
    mov rax, IMM_NIL
    ret

section .rodata
ovf_name: db "OVERFLOW"
