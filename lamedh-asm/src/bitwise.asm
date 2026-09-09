; bitwise.asm — LOGAND/LOGIOR/LOGXOR/LOGNOT/ASH, the reference's
; integer bitwise operations over this kernel's own 62-bit fixnum
; (Part XII axis 1 already documents this width divergence elsewhere —
; see README "Value representation"; these ops inherit it rather than
; introducing a new one). Every fixnum's tag bits are 00, so a plain
; AND/OR/XOR of two *tagged* words already gives the correctly-tagged
; result with no untag/retag at all — the same trick tags.inc's own
; header describes for ADD/SUB: (a<<2) & (b<<2) == (a&b)<<2, tag bits
; 0 & 0 == 0, and likewise for OR/XOR. LOGNOT and ASH still need to
; untag first: flipping every bit of a *tagged* word would corrupt the
; tag itself (00 -> 11), and a shift must operate on the numeric value,
; not the pre-shifted representation.
;
; v0 scope, narrower than the reference on purpose: LOGAND/LOGIOR/
; LOGXOR are fixed-arity (exactly two operands), not the reference's
; own variadic fold (this kernel's compiler has no general variadic-
; primitive-call mechanism yet — the same reason FORMAT/STRING-REF and
; every other multi-operand builtin here is a fixed shape, see the
; kernel surface notes elsewhere in this README).

%include "src/tags.inc"

extern fail_wrong_type
extern set_overflow_flag

section .text

; check_two_fixnums(rdi=tagged a, rsi=tagged b) -> falls through on
; success; tail-calls fail_wrong_type (never returns) if either isn't
; a fixnum. Shared validation for the three tagged-word-trick ops.
check_two_fixnums:
    mov rax, rdi
    and rax, TAG_MASK
    test rax, rax                     ; TAG_FIXNUM == 0
    jnz .bad
    mov rax, rsi
    and rax, TAG_MASK
    test rax, rax
    jnz .bad
    ret
.bad:
    mov rsi, bitwise_range_msg
    mov rdx, bitwise_range_msg_len
    jmp fail_wrong_type

; logand_tagged(rdi=tagged a, rsi=tagged b) -> rax = tagged (a AND b).
; check_two_fixnums touches only rax, so rdi/rsi survive the call
; unchanged with no save/restore needed.
global logand_tagged
logand_tagged:
    call check_two_fixnums
    mov rax, rdi
    and rax, rsi
    ret

; logior_tagged(rdi=tagged a, rsi=tagged b) -> rax = tagged (a OR b)
global logior_tagged
logior_tagged:
    call check_two_fixnums
    mov rax, rdi
    or rax, rsi
    ret

; logxor_tagged(rdi=tagged a, rsi=tagged b) -> rax = tagged (a XOR b)
global logxor_tagged
logxor_tagged:
    call check_two_fixnums
    mov rax, rdi
    xor rax, rsi
    ret

; lognot_tagged(rdi=tagged n) -> rax = tagged (NOT n). NOT(n) == -n-1,
; which stays within this kernel's 62-bit signed range for every n
; that was in range to begin with, so no overflow check is needed
; (unlike ASH below).
global lognot_tagged
lognot_tagged:
    mov rax, rdi
    and rax, TAG_MASK
    test rax, rax
    jnz .bad
    mov rax, rdi
    UNTAG_FIXNUM rax
    not rax
    TO_FIXNUM rax
    ret
.bad:
    mov rsi, bitwise_range_msg
    mov rdx, bitwise_range_msg_len
    jmp fail_wrong_type

; ash_tagged(rdi=tagged n, rsi=tagged shift) -> rax = tagged (n
; arithmetic-shifted by shift; positive shifts left, negative shifts
; right — CL's ASH convention, matching the reference exactly).
; A shift magnitude at or beyond this kernel's 62-bit fixnum width
; behaves the same way this kernel's existing +/- overflow already
; does: a left shift past the width sets the shared OVERFLOW flag
; (overflow.asm, "KERNEL.md conformance" above) and yields 0; a right
; shift past the width sign-extends to 0 or -1, exactly like the
; reference's own >=64 case scaled down to this kernel's own 62 usable
; bits.
global ash_tagged
ash_tagged:
    push rbx
    push r12
    mov rbx, rdi                   ; tagged n — kept live for the
    mov r12, rsi                     ; error-report culprit if either
                                        ; operand turns out to not be a
                                        ; fixnum (reported as n either
                                        ; way, a v0 simplification — see
                                        ; check_two_fixnums's own
                                        ; comment on the same tradeoff)
    mov rax, rbx
    and rax, TAG_MASK
    test rax, rax
    jnz .bad
    mov rax, r12
    and rax, TAG_MASK
    test rax, rax
    jnz .bad

    mov rax, rbx
    UNTAG_FIXNUM rax                 ; rax = n
    mov rcx, r12
    UNTAG_FIXNUM rcx                   ; rcx = shift
    test rcx, rcx
    jz .done
    jns .left

    ; right shift: magnitude = -shift
    neg rcx
    cmp rcx, 62
    jl .do_right
    cmp rax, 0
    jl .neg_all
    xor rax, rax
    jmp .done
.neg_all:
    mov rax, -1
    jmp .done
.do_right:
    sar rax, cl
    jmp .done
.left:
    cmp rcx, 62
    jl .do_left
    call set_overflow_flag
    xor rax, rax
    jmp .done
.do_left:
    shl rax, cl
.done:
    TO_FIXNUM rax
    pop r12
    pop rbx
    ret
.bad:
    mov rdi, rbx
    pop r12
    pop rbx
    mov rsi, bitwise_range_msg
    mov rdx, bitwise_range_msg_len
    jmp fail_wrong_type

section .rodata
bitwise_range_msg: db "expected a fixnum"
bitwise_range_msg_len: equ $ - bitwise_range_msg
