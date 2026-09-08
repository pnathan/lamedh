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
