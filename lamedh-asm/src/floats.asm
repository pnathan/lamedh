; floats.asm — the float value type: a boxed IEEE754 double (HDR_FLOAT:
; [0]=header [8]=8 raw bytes). Self-evaluating exactly like a fixnum or
; string literal (see strings.asm's own note on why — the data heap
; never relocates).
;
; Arithmetic is NOT emitted as inline target SSE2 instructions — every
; op here is an ordinary host routine, reached from compiled Lamedh code
; the same way CAR/CDR/CONS are: an absolute-address indirect call via
; compile_binary_hostcall/compile_unary_hostcall (compiler.asm). That
; sidesteps adding any XMM support to codegen.asm at all — codegen.asm's
; own header is explicit that it hand-encodes only the 8 base GPRs — at
; the cost of a real function call per float operation instead of an
; inlined instruction. A JIT worth the name would inline these; this is
; the honestly-scoped v0.

%include "src/tags.inc"

extern data_alloc
extern print_fixnum
extern write_buf
extern fail_wrong_type

section .text

; make_float(xmm0=value) -> rax = tagged HDR_FLOAT heapobj
global make_float
make_float:
    push rbx
    mov rdi, 16
    call data_alloc
    mov rbx, rax
    mov qword [rbx], HDR_FLOAT
    movsd [rbx+8], xmm0
    mov rax, rbx
    or rax, TAG_HEAPOBJ
    pop rbx
    ret

; is_float(rdi=tagged value) -> rax=1/0
global is_float
is_float:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .no
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_FLOAT
    jne .no
    mov rax, 1
    ret
.no:
    xor rax, rax
    ret

; float_val(rdi=tagged float) -> xmm0 = raw double value
global float_val
float_val:
    mov rax, rdi
    UNTAG_PTR rax
    movsd xmm0, [rax+8]
    ret

; float_of_fixnum(rdi=tagged fixnum) -> rax = tagged float. The FLOAT
; builtin's host half (see compiler.asm) — the only way to introduce a
; float value other than a reader literal.
global float_of_fixnum
float_of_fixnum:
    mov rax, rdi
    UNTAG_FIXNUM rax
    cvtsi2sd xmm0, rax
    jmp make_float

; float_add(rdi=tagged float1, rsi=tagged float2) -> rax = tagged float
global float_add
float_add:
    push rbx
    mov rbx, rsi                  ; tagged val2
    call float_val                    ; xmm0 = val1
    movsd xmm1, xmm0
    mov rdi, rbx
    call float_val                        ; xmm0 = val2
    addsd xmm0, xmm1                          ; xmm0 = val2 + val1 (commutative)
    call make_float
    pop rbx
    ret

; float_sub(rdi=tagged float1, rsi=tagged float2) -> rax = tagged
; (val1 - val2), order matters so it can't reuse float_add's shortcut.
global float_sub
float_sub:
    push rbx
    mov rbx, rsi                  ; tagged val2
    call float_val                    ; xmm0 = val1
    movsd xmm1, xmm0
    mov rdi, rbx
    call float_val                        ; xmm0 = val2
    movsd xmm2, xmm0
    movsd xmm0, xmm1
    subsd xmm0, xmm2                          ; xmm0 = val1 - val2
    call make_float
    pop rbx
    ret

; float_mul(rdi=tagged float1, rsi=tagged float2) -> rax = tagged float
global float_mul
float_mul:
    push rbx
    mov rbx, rsi
    call float_val
    movsd xmm1, xmm0
    mov rdi, rbx
    call float_val
    mulsd xmm0, xmm1
    call make_float
    pop rbx
    ret

; float_div(rdi=tagged float1, rsi=tagged float2) -> rax = tagged
; (val1 / val2).
global float_div
float_div:
    push rbx
    mov rbx, rsi                  ; tagged val2
    call float_val                    ; xmm0 = val1
    movsd xmm1, xmm0
    mov rdi, rbx
    call float_val                        ; xmm0 = val2
    divsd xmm1, xmm0                          ; xmm1 = val1 / val2
    movsd xmm0, xmm1
    call make_float
    pop rbx
    ret

; float_lt(rdi=tagged float1, rsi=tagged float2) -> rax = IMM_TRUE/IMM_NIL
global float_lt
float_lt:
    push rbx
    mov rbx, rsi                  ; tagged val2
    call float_val                    ; xmm0 = val1
    movsd xmm1, xmm0
    mov rdi, rbx
    call float_val                        ; xmm0 = val2
    comisd xmm1, xmm0                         ; flags = val1 <=> val2
    setb al                                     ; CF set iff val1 < val2
    movzx eax, al
    imul eax, eax, 4
    add eax, IMM_NIL
    pop rbx
    ret

; float_eq_exact(rdi=tagged float a, rsi=tagged float b) -> rax = 1/0.
; KERNEL.md Part IV: IEEE `==` plus an explicit carve-out that NaN is
; EQ to NaN — so (eq 0.0 -0.0) is true (IEEE says they're equal) and
; (eq nan nan) is also true (the carve-out), neither of which a bare
; native float comparison alone gives. The EQ builtin's slow path for
; two HDR_FLOAT heapobjs (lisp_eq, strings.asm) — a plain tagged-value
; compare there would treat two boxed floats with the same numeric
; value as unequal, since each holds its own distinct address.
global float_eq_exact
float_eq_exact:
    push rbx
    mov rbx, rsi
    call float_val                    ; xmm0 = a
    movsd xmm2, xmm0
    mov rdi, rbx
    call float_val                       ; xmm0 = b
    movsd xmm3, xmm0

    ucomisd xmm2, xmm2                      ; PF set iff a is NaN
    setp r8b
    ucomisd xmm3, xmm3                        ; PF set iff b is NaN
    setp r9b
    movzx eax, r8b
    movzx ecx, r9b
    and eax, ecx
    test eax, eax
    jnz .true                                   ; both NaN -> EQ

    ucomisd xmm2, xmm3
    setz al                                       ; ZF: numerically equal
    setnp cl                                        ; NP: ordered (neither NaN)
    movzx eax, al
    movzx ecx, cl
    and eax, ecx
    jmp .out
.true:
    mov eax, 1
.out:
    pop rbx
    ret

; float_print(rdi=tagged float) -> writes a fixed 6-decimal-place
; representation to stdout (no scientific notation, no shortest
; round-trip formatting — see README roadmap). The FLOAT-printing half
; of print_value's runtime dispatch (strings.asm).
;
; The sign comes from the sign bit, not a compare, so -0.0 prints as
; "-0.000000" (a compare treats it as equal to 0.0 and lost the sign).
; The six decimals are the fraction rounded to nearest, ties to even
; (cvtsd2si under the default MXCSR rounding), carrying into the
; integer part — 3.14159265 is "3.141593" and 0.9999999 is "1.000000",
; not the truncated "3.141592"/"0.999999". Infinities and NaN print as
; the reference prints them: "inf", "-inf", "NaN". A magnitude of 2^61
; or more — beyond the fixnum range, and from 2^63 beyond cvttsd2si
; itself — prints its exact integral value through print_big_integral
; (issue #550: out-of-fixnum-range integer literals read as floats, so
; they must print as themselves); such a float has no fractional bits
; worth rounding, so the fraction is a literal ".000000".
global float_print
float_print:
    push rbx
    push r12
    push r13
    call float_val                    ; xmm0 = value

    movq rax, xmm0
    xor r12, r12                        ; sign flag
    btr rax, 63                           ; rax = |value|'s bits, CF = sign
    jnc .sign_done
    mov r12, 1
.sign_done:
    mov rcx, 0x7FF0000000000000             ; exponent all ones:
    cmp rax, rcx                              ; = inf, > NaN
    ja .nan
    je .inf
    mov rbx, rax                                ; rbx = |value|'s bits
    mov rcx, 0x43C0000000000000                  ; 2^61 as f64; positive
    cmp rax, rcx                                   ; doubles order as integers
    jae .big
    movq xmm0, rax                              ; xmm0 = |value|
    roundsd xmm1, xmm0, 3                         ; xmm1 = trunc(|value|) < 2^61
    subsd xmm0, xmm1                                ; fraction (exact)
    mulsd xmm0, [rel float_1e6]
    cvtsd2si r13, xmm0                                ; round, ties to even
    cvttsd2si rbx, xmm1
    cmp r13, 1000000
    jb .emit
    sub r13, 1000000                                      ; x.9999996 -> x+1
    inc rbx
.emit:
    ; every value lives in rbx/r12/r13 (callee-saved) from here on:
    ; write_buf clobbers rax (the stdout path leaves the syscall's byte
    ; count in it; the capture path used by PRINC-TO-STRING leaves a
    ; buffer address there, which once printed 2.5 as "2.775808").
    test r12, r12
    jz .int_part
    mov rsi, minus_buf
    mov rdx, 1
    call write_buf
.int_part:
    mov rdi, rbx
    call print_u64
    mov rsi, dot_buf
    mov rdx, 1
    call write_buf
    mov rdi, r13
    call print_fixnum6
    jmp .out
.big:
    test r12, r12
    jz .big_digits
    mov rsi, minus_buf
    mov rdx, 1
    call write_buf
.big_digits:
    mov rdi, rbx                              ; |value|'s bits: sign clear, finite
    call print_big_integral
    mov rsi, zero_frac_buf
    mov rdx, zero_frac_buf_len
    call write_buf
    jmp .out
.inf:
    test r12, r12
    jz .inf_text
    mov rsi, minus_buf
    mov rdx, 1
    call write_buf
.inf_text:
    mov rsi, inf_buf
    mov rdx, 3
    call write_buf
    jmp .out
.nan:
    mov rsi, nan_buf
    mov rdx, 3
    call write_buf
.out:
    pop r13
    pop r12
    pop rbx
    ret


; print_big_integral(rdi=bits of a finite f64 >= 2^61) -> writes its
; exact decimal integer value. value = m * 2^e with m the 53-bit
; significand and e = biased exponent - 1075 >= 9, so the exact digits
; come from m in base-10^9 limbs doubled e times (at most 971 doublings
; over at most 35 limbs: 1.8e308 has 309 digits). Clobbers rax, rcx,
; rdx, rsi, rdi, r8, r9, r11.
print_big_integral:
    push rbx
    push r12
    push r13
    push r14
    push r15
    sub rsp, 336                     ; [rsp, +320): 40 limbs; [+320, +336): digits
    mov rax, rdi
    mov r12, rax
    shr r12, 52
    sub r12, 1075                      ; r12 = doublings still to apply
    mov rdx, 0xFFFFFFFFFFFFF
    and rax, rdx
    bts rax, 52                          ; rax = m, 2^52 <= m < 2^53
    mov r8, 1000000000
    xor rdx, rdx
    div r8
    mov [rsp], rdx                         ; low limb
    mov [rsp+8], rax                         ; m < 1e18: high limb, nonzero
    mov r13, 2                                 ; limb count
.dbl:
    test r12, r12
    jz .emit
    xor rcx, rcx
    xor r9, r9                                   ; carry
.dbl_limb:
    cmp rcx, r13
    jae .dbl_end
    mov rax, [rsp+rcx*8]
    add rax, rax
    add rax, r9
    xor r9, r9
    cmp rax, r8
    jb .dbl_store
    sub rax, r8
    mov r9, 1
.dbl_store:
    mov [rsp+rcx*8], rax
    inc rcx
    jmp .dbl_limb
.dbl_end:
    test r9, r9
    jz .dbl_next
    mov qword [rsp+r13*8], 1
    inc r13
.dbl_next:
    dec r12
    jmp .dbl
.emit:
    lea r14, [r13-1]                 ; most significant limb first
    xor r15, r15                       ; its zero-pad width: none
.emit_limb:
    mov rax, [rsp+r14*8]
    lea rsi, [rsp+336]
    xor rcx, rcx
    mov r8, 10
.digit:
    xor rdx, rdx
    div r8
    add dl, '0'
    dec rsi
    mov [rsi], dl
    inc rcx
    test rax, rax
    jnz .digit
    cmp rcx, r15
    jb .digit                            ; rax is 0 now: pads with '0'
    lea rdx, [rsp+336]
    sub rdx, rsi
    call write_buf
    mov r15, 9                             ; every later limb: 9 digits
    dec r14
    jns .emit_limb
    add rsp, 336
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; print_u64(rdi=raw unsigned int) -> writes its decimal digits
; (float_print's integer part, which can exceed the fixnum range
; print_fixnum takes).
print_u64:
    push rbx
    sub rsp, 32
    mov rax, rdi
    mov r9, 10
    lea rsi, [rsp+31]
.loop:
    xor rdx, rdx
    div r9
    add dl, '0'
    dec rsi
    mov [rsi], dl
    test rax, rax
    jnz .loop
    lea rdx, [rsp+31]
    sub rdx, rsi
    call write_buf
    add rsp, 32
    pop rbx
    ret

; print_fixnum6(rdi=raw int, 0..999999) -> writes exactly 6 digits,
; zero-padded (float_print's fractional part).
print_fixnum6:
    push rbx
    sub rsp, 16
    mov rax, rdi
    mov r9, 10
    lea rsi, [rsp+15]
    mov byte [rsi], 0
    mov rcx, 6
.loop:
    xor rdx, rdx
    div r9
    add dl, '0'
    dec rsi
    mov [rsi], dl
    dec rcx
    jnz .loop
    lea rdx, [rsp+15]
    sub rdx, rsi
    call write_buf
    add rsp, 16
    pop rbx
    ret

section .rodata
minus_buf: db "-"
dot_buf: db "."
zero_frac_buf: db ".000000"
zero_frac_buf_len equ $ - zero_frac_buf
inf_buf: db "inf"
nan_buf: db "NaN"
align 8
float_1e6: dq 1.0e6

; ---------------------------------------------------------------------
; Math library (the reference's SQRT/SIN/COS/TAN/EXP/LOG/FLOOR/CEILING/
; ROUND/TRUNCATE builtins, builtins_core.rs's apply_math_lib). No libc
; here, so the transcendentals are the x87 instructions themselves
; (fsin/fcos/fptan/f2xm1/fyl2x — the same hardware libm ultimately
; reduces to for these on x86-64) and the rest is SSE (sqrtsd,
; roundsd). Every routine takes a tagged fixnum OR float (as_f64's own
; contract in the reference) and signals a real condition for anything
; else; SQRT/SIN/COS/TAN/EXP/LOG return a float, FLOOR/CEILING/ROUND/
; TRUNCATE a fixnum — exactly the reference's result types.

section .text
; float_arg(rdi=tagged fixnum or float) -> xmm0 = the value as a double
float_arg:
    mov rax, rdi
    and rax, TAG_MASK
    jnz .not_fixnum
    mov rax, rdi
    UNTAG_FIXNUM rax
    cvtsi2sd xmm0, rax
    ret
.not_fixnum:
    push rdi
    call is_float
    pop rdi
    test rax, rax
    jz .bad
    jmp float_val
.bad:
    mov rsi, math_type_msg
    mov rdx, math_type_msg_len
    call fail_wrong_type                  ; never returns

; generic_binop(rdi='+'/'-'/'*'/'<'/'=' as ASCII, rsi=lhs, rdx=rhs) -> rax
; The out-of-line slow path of every compiled binary `+ - * < =`
; (compile_binop, compiler.asm): the inline code there runs only when
; both tagged operands are fixnums (Chars already coerced to their code
; points), and calls this for everything else. KERNEL.md Part V
; contagion: every operand is converted to f64 (float_arg — a fixnum
; widened, a float unboxed, anything else a HANDLER-CASE-catchable
; condition whose ERROR-DATA is the culprit) and the operation runs in
; floating point. + - * return a new float; < and = return
; IMM_TRUE/IMM_NIL, both NIL when either side is NaN (`=` is exact IEEE
; `==`, so (= NaN NaN) is NIL — unlike EQ, Part IV).
;
; Precondition: at least one operand is not a fixnum. Two fixnums never
; reach this routine from compiled code; if they did, the result would
; be a float where Part V requires the integer path.
global generic_binop
generic_binop:
    push rbx
    push r12
    sub rsp, 8                     ; spill slot for the lhs double
    mov rbx, rdi                   ; op
    mov r12, rdx                   ; rhs
    mov rdi, rsi
    call float_arg                   ; xmm0 = lhs (or signals)
    movsd [rsp], xmm0
    mov rdi, r12
    call float_arg                     ; xmm0 = rhs (or signals)
    movsd xmm1, xmm0                     ; xmm1 = rhs
    movsd xmm0, [rsp]                      ; xmm0 = lhs
    movzx ecx, bl                            ; op, freed of rbx
    add rsp, 8
    pop r12
    pop rbx
    cmp cl, '+'
    jne .not_add
    addsd xmm0, xmm1
    jmp make_float
.not_add:
    cmp cl, '-'
    jne .not_sub
    subsd xmm0, xmm1
    jmp make_float
.not_sub:
    cmp cl, '*'
    jne .not_mul
    mulsd xmm0, xmm1
    jmp make_float
.not_mul:
    cmp cl, '<'
    jne .not_lt
    comisd xmm1, xmm0                ; rhs <=> lhs
    seta al                            ; lhs < rhs; unordered sets CF -> 0
    jmp .bool_from_al
.not_lt:
    ; '='
    ucomisd xmm0, xmm1
    sete al                              ; ZF: equal, or unordered
    setnp cl                               ; PF clear: ordered
    and al, cl
.bool_from_al:
    movzx eax, al
    lea eax, [rax*4 + IMM_NIL]           ; 0 -> IMM_NIL, 1 -> IMM_TRUE
    ret

global float_sqrt
float_sqrt:
    call float_arg
    sqrtsd xmm0, xmm0
    jmp make_float

; x87 helpers: the value travels through a 16-byte stack scratch,
; xmm0 -> st0 -> xmm0.
global float_sin
float_sin:
    call float_arg
    sub rsp, 16
    movsd [rsp], xmm0
    fld qword [rsp]
    fsin
    fstp qword [rsp]
    movsd xmm0, [rsp]
    add rsp, 16
    jmp make_float

global float_cos
float_cos:
    call float_arg
    sub rsp, 16
    movsd [rsp], xmm0
    fld qword [rsp]
    fcos
    fstp qword [rsp]
    movsd xmm0, [rsp]
    add rsp, 16
    jmp make_float

global float_tan
float_tan:
    call float_arg
    sub rsp, 16
    movsd [rsp], xmm0
    fld qword [rsp]
    fptan                                 ; pushes 1.0 on top of tan(x)
    fstp st0                              ; discard the 1.0
    fstp qword [rsp]
    movsd xmm0, [rsp]
    add rsp, 16
    jmp make_float

; e^x = 2^(x*log2 e): split x*log2e into integer and fraction, f2xm1
; on the fraction (its domain is [-1,1]), fscale by the integer.
global float_exp
float_exp:
    call float_arg
    sub rsp, 16
    movsd [rsp], xmm0
    fldl2e                                ; st0 = log2(e)
    fmul qword [rsp]                      ; st0 = x*log2(e)
    fld st0                               ; st0 = t, st1 = t
    frndint                               ; st0 = n = rint(t)
    fsub st1, st0                         ; st1 = t - n (fraction)
    fxch st1                              ; st0 = frac, st1 = n
    f2xm1                                 ; st0 = 2^frac - 1
    fld1
    faddp st1, st0                        ; st0 = 2^frac, st1 = n
    fscale                                ; st0 = 2^frac * 2^n
    fstp st1                              ; pop n, keep result
    fstp qword [rsp]
    movsd xmm0, [rsp]
    add rsp, 16
    jmp make_float

; ln x = ln2 * log2(x): fyl2x computes st1 * log2(st0).
global float_log
float_log:
    call float_arg
    sub rsp, 16
    movsd [rsp], xmm0
    fldln2                                ; st0 = ln 2
    fld qword [rsp]                       ; st0 = x, st1 = ln 2
    fyl2x                                 ; st0 = ln2 * log2(x) = ln x
    fstp qword [rsp]
    movsd xmm0, [rsp]
    add rsp, 16
    jmp make_float

; float_log_base(rdi=x, rsi=base) -> ln x / ln base (the reference's
; two-argument LOG).
global float_log_base
float_log_base:
    push rbx
    push r12
    mov rbx, rsi
    call float_log                        ; rax = tagged ln x
    mov r12, rax
    mov rdi, rbx
    call float_log                        ; rax = tagged ln base
    mov rdi, r12
    mov rsi, rax
    call float_div
    pop r12
    pop rbx
    ret

; roundsd immediates: 9 = floor, 10 = ceiling, 11 = truncate (each with
; the inexact exception suppressed).
global float_floor
float_floor:
    call float_arg
    roundsd xmm0, xmm0, 9
    jmp float_to_fixnum_result
global float_ceiling
float_ceiling:
    call float_arg
    roundsd xmm0, xmm0, 10
    jmp float_to_fixnum_result
global float_truncate
float_truncate:
    call float_arg
    roundsd xmm0, xmm0, 11
float_to_fixnum_result:
    cvttsd2si rax, xmm0
    TO_FIXNUM rax
    ret

; ROUND: half away from zero (Rust's f64::round, the reference's
; documented choice — not roundsd's half-to-even). r = trunc(x); the
; remainder x - r is exact in binary floating point, so comparing it
; against +-0.5 decides the adjustment exactly.
global float_round
float_round:
    call float_arg
    roundsd xmm1, xmm0, 11                ; xmm1 = trunc(x)
    subsd xmm0, xmm1                      ; xmm0 = x - trunc(x), in (-1, 1)
    cvttsd2si rax, xmm1                   ; rax = trunc(x)
    mov rcx, 0x3FE0000000000000           ; 0.5
    movq xmm2, rcx
    comisd xmm0, xmm2
    jb .not_up                            ; remainder < 0.5
    inc rax
    jmp .done
.not_up:
    mov rcx, 0xBFE0000000000000           ; -0.5
    movq xmm2, rcx
    comisd xmm0, xmm2
    ja .done                              ; remainder > -0.5
    dec rax
.done:
    TO_FIXNUM rax
    ret

section .rodata
math_type_msg: db "expected a number (fixnum or float)"
math_type_msg_len: equ $ - math_type_msg
