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

; float_print(rdi=tagged float) -> writes the float's shortest
; round-trip decimal form to stdout (or the capture buffer), exactly as
; KERNEL.md Part I specifies and Rust's f64::to_string produces it:
; the fewest significant digits that read back to the same double (the
; closest such string when several exist), laid out positionally with no
; exponent, ".0" appended when no fraction results, the sign taken from
; the sign bit ("-0.0"), and "inf" / "-inf" / "NaN" for the non-finite
; values. The FLOAT-printing half of print_value's runtime dispatch
; (strings.asm).
;
; Digit generation is Steele & White / Burger & Dybvig free-format
; printing (Dragon4) over fixed-size bignums: value = f * 2^e is held as
; the exact ratio R/S with the half-gaps M+ / M- to the neighbouring
; doubles, S is scaled by 10 until the first digit sits just below the
; point, and digits are shaken out by repeated subtraction until the
; remainder falls within a half-gap (inclusive when f is even, as the
; reader's round-to-nearest-even makes the boundary itself read back).
; Every intermediate is exact, so the output is correct for all doubles
; including denormals and the unequal gap below a power of two.
;
; Frame (rbp-relative): the bignums R, S, M+, M-, a scratch T, the
; digit string, two decision flags, and the output text.
NL      equ 24                  ; limbs per bignum: 1536 bits > the ~1135 needed
BN      equ NL*8
FP_R    equ 0
FP_S    equ BN
FP_MP   equ 2*BN
FP_MM   equ 3*BN
FP_T    equ 4*BN
FP_DIG  equ 5*BN                ; 32 bytes: at most 17 digits are produced
FP_TC   equ FP_DIG+32           ; tc1 (dword), tc2 (dword)
FP_OUT  equ FP_TC+8             ; 400 bytes: "-0." + 323 zeros + 17 digits < 400
FP_FRAME equ FP_OUT+400

%macro BN_MUL10 1               ; multiply bignum at [rbp+%1] by 10
    lea rdi, [rbp+%1]
    mov esi, 10
    call bn_mul_small
%endmacro

global float_print
float_print:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, FP_FRAME
    mov rbp, rsp
    call float_val                      ; xmm0 = value

    movq rax, xmm0
    xor r12d, r12d                      ; sign flag, from the sign bit
    btr rax, 63                         ; rax = |value|'s bits, CF = sign
    jnc .sign_done
    mov r12d, 1
.sign_done:
    mov rcx, 0x7FF0000000000000         ; exponent all ones:
    cmp rax, rcx                        ; = inf, > NaN
    ja .nan
    je .inf
    test rax, rax
    jz .zero

    mov rcx, rax
    shr rcx, 52                         ; rcx = biased exponent
    mov rdx, 0xFFFFFFFFFFFFF
    and rax, rdx                        ; rax = 52-bit fraction
    xor r15d, r15d                      ; r15 = 1 when the gap below is half the gap above
    test rcx, rcx
    jz .denormal
    test rax, rax
    jnz .normal_f
    cmp rcx, 1
    jbe .normal_f                       ; smallest normal: both gaps equal
    mov r15d, 1                         ; a power of two above the smallest normal
.normal_f:
    bts rax, 52                         ; implicit leading bit
    lea rbx, [rcx-1075]                 ; e
    jmp .have_fe
.denormal:
    mov rbx, -1074
.have_fe:
    mov r14, rax                        ; f
    mov r13d, r14d
    and r13d, 1
    xor r13d, 1                         ; r13 = 1 when f is even (boundaries inclusive)

    test rbx, rbx
    js .e_neg
    ; e >= 0: R = f << (e+1+asym), S = 2 (4), M+ = 1 << (e+asym), M- = 1 << e
    lea rdi, [rbp+FP_R]
    mov rsi, r14
    call bn_set
    lea rdi, [rbp+FP_R]
    lea rsi, [rbx+r15+1]
    call bn_shl
    lea rdi, [rbp+FP_S]
    lea rsi, [r15*2+2]
    call bn_set
    lea rdi, [rbp+FP_MP]
    mov esi, 1
    call bn_set
    lea rdi, [rbp+FP_MP]
    lea rsi, [rbx+r15]
    call bn_shl
    lea rdi, [rbp+FP_MM]
    mov esi, 1
    call bn_set
    lea rdi, [rbp+FP_MM]
    mov rsi, rbx
    call bn_shl
    jmp .scale
.e_neg:
    ; e < 0: R = f << (1+asym), S = 1 << (1-e+asym), M+ = 1 (2), M- = 1
    lea rdi, [rbp+FP_R]
    mov rsi, r14
    call bn_set
    lea rdi, [rbp+FP_R]
    lea rsi, [r15+1]
    call bn_shl
    lea rdi, [rbp+FP_S]
    mov esi, 1
    call bn_set
    lea rdi, [rbp+FP_S]
    lea rsi, [r15+1]
    sub rsi, rbx
    call bn_shl
    lea rdi, [rbp+FP_MP]
    lea rsi, [r15+1]
    call bn_set
    lea rdi, [rbp+FP_MM]
    mov esi, 1
    call bn_set

.scale:
    xor ebx, ebx                        ; rbx = k: value = 0.DIGITS * 10^k
.up:                                    ; raise k while R+M+ reaches S
    lea rdi, [rbp+FP_T]
    lea rsi, [rbp+FP_R]
    call bn_copy
    lea rdi, [rbp+FP_T]
    lea rsi, [rbp+FP_MP]
    call bn_add
    lea rdi, [rbp+FP_T]
    lea rsi, [rbp+FP_S]
    call bn_cmp
    test r13d, r13d
    jz .up_odd
    test eax, eax
    js .down                            ; even: stop when R+M+ < S
    jmp .up_step
.up_odd:
    test eax, eax
    jle .down                           ; odd: stop when R+M+ <= S
.up_step:
    BN_MUL10 FP_S
    inc rbx
    jmp .up
.down:                                  ; lower k while (R+M+)*10 stays under S
    lea rdi, [rbp+FP_T]
    lea rsi, [rbp+FP_R]
    call bn_copy
    lea rdi, [rbp+FP_T]
    lea rsi, [rbp+FP_MP]
    call bn_add
    BN_MUL10 FP_T
    lea rdi, [rbp+FP_T]
    lea rsi, [rbp+FP_S]
    call bn_cmp
    test r13d, r13d
    jz .down_odd
    test eax, eax
    jns .gen                            ; even: stop when (R+M+)*10 >= S
    jmp .down_step
.down_odd:
    test eax, eax
    jg .gen                             ; odd: stop when (R+M+)*10 > S
.down_step:
    BN_MUL10 FP_R
    BN_MUL10 FP_MP
    BN_MUL10 FP_MM
    dec rbx
    jmp .down

.gen:
    xor r14d, r14d                      ; r14 = digits produced
.gen_loop:
    BN_MUL10 FP_R
    BN_MUL10 FP_MP
    BN_MUL10 FP_MM
    xor r15d, r15d                      ; r15 = the digit: floor(R/S), R < 10*S
.dsub:
    lea rdi, [rbp+FP_R]
    lea rsi, [rbp+FP_S]
    call bn_cmp
    test eax, eax
    js .dsub_done
    lea rdi, [rbp+FP_R]
    lea rsi, [rbp+FP_S]
    call bn_sub
    inc r15d
    jmp .dsub
.dsub_done:
    ; tc1: R within M- of zero (R <= M- when even, R < M- when odd)
    lea rdi, [rbp+FP_R]
    lea rsi, [rbp+FP_MM]
    call bn_cmp
    mov edx, 1
    sub edx, r13d
    add edx, eax                        ; c + (1-even) <= 0
    xor ecx, ecx
    test edx, edx
    setle cl
    mov [rbp+FP_TC], ecx
    ; tc2: R + M+ reaches S (>= when even, > when odd)
    lea rdi, [rbp+FP_T]
    lea rsi, [rbp+FP_R]
    call bn_copy
    lea rdi, [rbp+FP_T]
    lea rsi, [rbp+FP_MP]
    call bn_add
    lea rdi, [rbp+FP_T]
    lea rsi, [rbp+FP_S]
    call bn_cmp
    lea edx, [rax+r13-1]                ; c - (1-even) >= 0
    xor ecx, ecx
    test edx, edx
    setns cl
    mov [rbp+FP_TC+4], ecx

    mov eax, [rbp+FP_TC]
    mov edx, [rbp+FP_TC+4]
    test eax, eax
    jz .no_tc1
    test edx, edx
    jz .last_d                          ; only tc1: round down
    lea rdi, [rbp+FP_T]                 ; both: nearer of d and d+1, tie up
    lea rsi, [rbp+FP_R]
    call bn_copy
    lea rdi, [rbp+FP_T]
    mov esi, 2
    call bn_mul_small
    lea rdi, [rbp+FP_T]
    lea rsi, [rbp+FP_S]
    call bn_cmp
    test eax, eax
    js .last_d
    jmp .last_d1
.no_tc1:
    test edx, edx
    jnz .last_d1                        ; only tc2: round up
    lea rax, [r15+'0']                  ; neither: emit d, keep going
    mov [rbp+FP_DIG+r14], al
    inc r14d
    jmp .gen_loop
.last_d1:
    inc r15d
.last_d:
    lea rax, [r15+'0']
    mov [rbp+FP_DIG+r14], al
    inc r14d

    ; Layout: n = r14 digits, value = 0.DIGITS * 10^k (k = rbx)
    lea r15, [rbp+FP_OUT]
    test r12d, r12d
    jz .lay
    mov byte [r15], '-'
    inc r15
.lay:
    test rbx, rbx
    jg .k_pos
    mov byte [r15], '0'                 ; k <= 0: 0.000DIGITS
    mov byte [r15+1], '.'
    add r15, 2
    mov rcx, rbx
    neg rcx
    call fp_zeros
    xor ecx, ecx
    mov rdx, r14
    call fp_digits
    jmp .emit
.k_pos:
    cmp rbx, r14
    jl .point_inside
    xor ecx, ecx                        ; k >= n: DIGITS000.0
    mov rdx, r14
    call fp_digits
    mov rcx, rbx
    sub rcx, r14
    call fp_zeros
    mov byte [r15], '.'
    mov byte [r15+1], '0'
    add r15, 2
    jmp .emit
.point_inside:                          ; 0 < k < n: DIG.ITS
    xor ecx, ecx
    mov rdx, rbx
    call fp_digits
    mov byte [r15], '.'
    inc r15
    mov rcx, rbx
    mov rdx, r14
    call fp_digits
.emit:
    lea rsi, [rbp+FP_OUT]
    mov rdx, r15
    sub rdx, rsi
    call write_buf
    jmp .out
.zero:
    test r12d, r12d
    jz .zero_text
    mov rsi, minus_buf
    mov rdx, 1
    call write_buf
.zero_text:
    mov rsi, zero_buf
    mov rdx, 3
    call write_buf
    jmp .out
.inf:
    test r12d, r12d
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
    add rsp, FP_FRAME
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret

; fp_zeros(rcx=count, r15=out) -> writes rcx '0' bytes at r15, advancing it.
fp_zeros:
    test rcx, rcx
    jz .done
.loop:
    mov byte [r15], '0'
    inc r15
    dec rcx
    jnz .loop
.done:
    ret

; fp_digits(rcx=start, rdx=end, rbp=frame, r15=out) -> copies digits
; [start, end) to r15, advancing it.
fp_digits:
.loop:
    cmp rcx, rdx
    jae .done
    mov al, [rbp+FP_DIG+rcx]
    mov [r15], al
    inc r15
    inc rcx
    jmp .loop
.done:
    ret

; ---- fixed-size little-endian bignums (NL 64-bit limbs) ----------------
; All take the bignum in rdi (and a second operand in rsi) and preserve
; rdi; they clobber rax, rcx, rdx and r8. All but bn_shl preserve rsi;
; bn_shl also clobbers rsi, r10 and r11. Values never outgrow NL limbs (see FP_* above), so
; carries out of the top limb are dropped.

; bn_zero(rdi)
bn_zero:
    xor eax, eax
    xor ecx, ecx
.loop:
    mov [rdi+rcx*8], rax
    inc ecx
    cmp ecx, NL
    jb .loop
    ret

; bn_set(rdi, rsi=u64) -> bignum = rsi
bn_set:
    call bn_zero
    mov [rdi], rsi
    ret

; bn_copy(rdi=dst, rsi=src)
bn_copy:
    xor ecx, ecx
.loop:
    mov rax, [rsi+rcx*8]
    mov [rdi+rcx*8], rax
    inc ecx
    cmp ecx, NL
    jb .loop
    ret

; bn_mul_small(rdi, rsi=u64 multiplier) -> bignum *= rsi
bn_mul_small:
    xor ecx, ecx
    xor r8d, r8d                        ; carry
.loop:
    mov rax, [rdi+rcx*8]
    mul rsi
    add rax, r8
    adc rdx, 0
    mov [rdi+rcx*8], rax
    mov r8, rdx
    inc ecx
    cmp ecx, NL
    jb .loop
    ret

; bn_shl(rdi, rsi=bit count) -> bignum <<= rsi, in chunks of at most 60 bits
bn_shl:
    mov r10, rsi
.chunk:
    test r10, r10
    jz .done
    mov r11, r10
    cmp r11, 60
    jbe .take
    mov r11d, 60
.take:
    mov ecx, r11d
    mov esi, 1
    shl rsi, cl
    sub r10, r11
    call bn_mul_small
    jmp .chunk
.done:
    ret

; bn_add(rdi, rsi) -> bignum += [rsi]
bn_add:
    xor ecx, ecx
    xor r8d, r8d                        ; carry
.loop:
    mov rax, [rdi+rcx*8]
    xor edx, edx
    add rax, [rsi+rcx*8]
    adc edx, 0
    add rax, r8
    adc edx, 0
    mov [rdi+rcx*8], rax
    mov r8, rdx
    inc ecx
    cmp ecx, NL
    jb .loop
    ret

; bn_sub(rdi, rsi) -> bignum -= [rsi] (caller guarantees no underflow)
bn_sub:
    xor ecx, ecx
    xor r8d, r8d                        ; borrow
.loop:
    mov rax, [rdi+rcx*8]
    xor edx, edx
    sub rax, [rsi+rcx*8]
    adc edx, 0
    sub rax, r8
    adc edx, 0
    mov [rdi+rcx*8], rax
    mov r8, rdx
    inc ecx
    cmp ecx, NL
    jb .loop
    ret

; bn_cmp(rdi, rsi) -> eax = -1, 0 or 1 as [rdi] <, ==, > [rsi]
bn_cmp:
    mov ecx, NL-1
.loop:
    mov rax, [rdi+rcx*8]
    cmp rax, [rsi+rcx*8]
    ja .gt
    jb .lt
    dec ecx
    jns .loop
    xor eax, eax
    ret
.gt:
    mov eax, 1
    ret
.lt:
    mov eax, -1
    ret

section .rodata
minus_buf: db "-"
zero_buf: db "0.0"
inf_buf: db "inf"
nan_buf: db "NaN"

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
