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
global float_print
float_print:
    push rbx
    push r12
    mov rbx, rdi
    call float_val                    ; xmm0 = value

    xor r12, r12                        ; sign flag
    pxor xmm1, xmm1
    comisd xmm0, xmm1
    jae .nonneg
    mov r12, 1
    mov rax, 0x8000000000000000           ; sign-bit mask
    movq xmm2, rax
    xorpd xmm0, xmm2                        ; xmm0 = |value|
.nonneg:
    test r12, r12
    jz .print_int_part
    mov rsi, minus_buf
    mov rdx, 1
    call write_buf
.print_int_part:
    cvttsd2si rax, xmm0                    ; truncate toward zero -> int part
    mov rbx, rax                              ; keep the raw value for the
                                               ; fractional-part math below —
                                               ; print_fixnum wants a *tagged*
                                               ; fixnum (it untags on entry),
                                               ; so only a tagged copy goes in
    mov rdi, rax
    TO_FIXNUM rdi
    call print_fixnum
    mov rax, rbx

    mov rsi, dot_buf
    mov rdx, 1
    call write_buf

    ; fractional part: (|value| - int_part) * 10^6, truncated, zero-padded
    ; int_part is reloaded from rbx HERE, after the write: write_buf
    ; clobbers rax (the stdout path leaves the syscall's byte count, 1,
    ; in it — which made the fraction come out right by arithmetic
    ; accident, since (v-1)*10^6 and (v-int)*10^6 share their last six
    ; digits; the capture path used by PRINC-TO-STRING leaves a buffer
    ; address there, which printed 2.5 as "2.775808").
    mov rax, rbx
    cvtsi2sd xmm1, rax
    subsd xmm0, xmm1
    mov rax, 1000000
    cvtsi2sd xmm1, rax
    mulsd xmm0, xmm1
    cvttsd2si rax, xmm0
    test rax, rax
    jns .frac_nonneg
    neg rax
.frac_nonneg:
    mov rdi, rax
    call print_fixnum6
    pop r12
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
