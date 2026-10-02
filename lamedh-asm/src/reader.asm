; reader.asm — s-expression reader over an in-memory buffer, plus the
; cons-cell primitives every later stage (compiler, printer extensions)
; builds on.
;
; Grammar: integers (optional leading '-'), floats (a '.' fraction
; and/or an e/E exponent), strings, char literals, symbols (uppercased
; on intern, per the Rust reader's convention), lists "( form* )" with
; an optional dotted tail "( form+ . form )", and the quote/backquote/
; #' sugars. No radix literals yet — see README roadmap.

%include "src/tags.inc"

%define BIGDEC_MAX_LIMBS 64       ; read_number's bigdec_limbs capacity (see .bss)

extern data_alloc_cons
extern rc_inc
extern intern_symbol
extern make_string
extern make_float
extern string_bytes
extern string_len
extern fail_wrong_type
extern tag_char

section .bss
align 8
global reader_buf
global reader_pos
global reader_end
reader_buf: resq 1
reader_pos: resq 1
reader_end: resq 1
; rfs_saved: 0, or the address of the innermost active
; read_from_string_tagged's saved [prev rfs_saved][end][pos][buf] frame
; on the native stack. A read error (reader_fail) throws straight past
; that frame's own restore code, so reader_fail restores the caller's
; reader position from here first — otherwise a READ-FROM-STRING parse
; error caught by HANDLER-CASE inside a file would leave the file's own
; top-level read loop reading the string's bytes.
rfs_saved: resq 1

section .text

; --- cons-cell primitives -------------------------------------------

; cons(rdi=car, rsi=cdr) -> rax = tagged cons pointer
global cons
cons:
    push r8
    push r9
    mov r8, rdi
    mov r9, rsi
    ; data_alloc_cons, not data_alloc: a cons has no header word, so its
    ; granule entry carries the flag that tells the collector's walker
    ; "two tagged slots, don't read [raw+0] as a header" (gc.asm).
    call data_alloc_cons
    mov [rax], r8
    mov [rax+8], r9
    or rax, TAG_CONS
    ; The cell now holds two heap->heap references; count them. This is
    ; the hottest counted site in the system, which is why rc_inc's
    ; fast path bails out on an immediate/fixnum tag in three
    ; instructions (gc.asm).
    mov rdi, r8
    call rc_inc
    mov rdi, r9
    call rc_inc
    pop r9
    pop r8
    ret

; car(rdi=tagged cons) -> rax. KERNEL.md Part IV/XI: (car nil) is nil;
; anything else that isn't a cons is a native failure, now signaled as
; a real catchable condition (fail_wrong_type, native_errors.asm)
; instead of the UNTAG_PTR-and-dereference below simply segfaulting —
; every *internal* caller of car (the compiler/reader/printer walking
; their own, always-proper lists) only ever hits the fast cons path or
; the nil path, exactly like before; only a genuinely malformed
; argument from Lamedh source (`(CAR 5)`) reaches .wrong_type.
global car
car:
    cmp rdi, IMM_NIL
    je .nil_case
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_CONS
    jne .wrong_type
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax]
    ret
.nil_case:
    mov rax, IMM_NIL
    ret
.wrong_type:
    mov rsi, car_err_msg
    mov rdx, car_err_msg_len
    jmp fail_wrong_type

; cdr(rdi=tagged cons) -> rax. Same rule as car above.
global cdr
cdr:
    cmp rdi, IMM_NIL
    je .nil_case
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_CONS
    jne .wrong_type
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+8]
    ret
.nil_case:
    mov rax, IMM_NIL
    ret
.wrong_type:
    mov rsi, cdr_err_msg
    mov rdx, cdr_err_msg_len
    jmp fail_wrong_type

; decode_char_escape(rdi=raw byte right after a '\' inside a char
; literal) -> rax = the decoded byte value, 0..255. KERNEL.md's char-
; literal escapes are the *opposite* convention from read_string's own
; string escapes: here every unrecognized backslash-prefixed byte
; decodes to that byte itself with the backslash dropped (`'\q'` is
; `'q'`) — which is exactly what falling through to "return the byte
; unchanged" already gives, so only the four escapes that need a
; genuinely different byte value (\n \t \r \0) are special-cased;
; backslash-backslash and backslash-quote fall through correctly
; unchanged, same as read_string's own \" / \\ handling above. (A
; trailing backslash at the very end of a comment line is a NASM line-
; continuation marker even inside a ";" comment, so none of these
; comments end with one — see chars.asm's own note on this gotcha.)
decode_char_escape:
    cmp dil, 'n'
    je .n
    cmp dil, 't'
    je .t
    cmp dil, 'r'
    je .r
    cmp dil, '0'
    je .z
    mov rax, rdi
    ret
.n:
    mov rax, 10
    ret
.t:
    mov rax, 9
    ret
.r:
    mov rax, 13
    ret
.z:
    xor rax, rax
    ret

; --- reader -----------------------------------------------------------

; reader_init(rdi=buf, rsi=len)
global reader_init
reader_init:
    mov [reader_buf], rdi
    mov qword [reader_pos], 0
    mov [reader_end], rsi
    ret

; read_from_string_tagged(rdi=tagged string) -> rax = the first tagged
; form read from the string's bytes, or IMM_EOF if it holds no form
; (KERNEL.md Part XI: READ-FROM-STRING). reader_buf/reader_pos/
; reader_end are single global cells, not a stack — reading one whole
; file is normally a single top-level loop with nothing else touching
; them, but this primitive can itself be *called from currently
; running compiled code* (e.g. inside an EVAL'd form, itself invoked
; from a file_runner.asm-style driver loop that is mid-file), so the
; caller's own reader position must survive a call here exactly the
; way a callee-saved register would: saved before, restored after,
; even though this reads and discards only one form and leaves any
; further bytes in the given string unread.
global read_from_string_tagged
read_from_string_tagged:
    push rbx
    push r12
    push r13
    mov rbx, rdi                      ; source string

    mov r12, [reader_buf]
    push r12
    mov r12, [reader_pos]
    push r12
    mov r12, [reader_end]
    push r12                            ; [saved_end, saved_pos, saved_buf]
    push qword [rfs_saved]
    mov [rfs_saved], rsp                  ; reader_fail restores from here

    mov rdi, rbx
    call string_bytes
    mov r12, rax
    mov rdi, rbx
    call string_len
    mov r13, rax
    mov rdi, r12
    mov rsi, r13
    call reader_init
    call read_form
    mov rbx, rax                          ; result (rbx: source string is
                                           ; dead by now)

    pop qword [rfs_saved]
    pop r12
    mov [reader_end], r12
    pop r12
    mov [reader_pos], r12
    pop r12
    mov [reader_buf], r12

    mov rax, rbx
    pop r13
    pop r12
    pop rbx
    ret

; reader_peek() -> rax = zero-extended char, or -1 if at end. Clobbers rax,rcx.
reader_peek:
    mov rcx, [reader_pos]
    cmp rcx, [reader_end]
    jae .eof
    mov rax, [reader_buf]
    movzx rax, byte [rax+rcx]
    ret
.eof:
    mov rax, -1
    ret

; is_delim(dil=char, expects char already validated not EOF by caller when
; needed) -> al = 1 if char ends a token, else 0. Clobbers rax only.
is_delim:
    cmp dil, ' '
    je .yes
    cmp dil, 9                    ; tab
    je .yes
    cmp dil, 10                   ; newline
    je .yes
    cmp dil, 13                   ; CR
    je .yes
    cmp dil, '('
    je .yes
    cmp dil, ')'
    je .yes
    cmp dil, 39                   ; '
    je .yes
    cmp dil, ';'
    je .yes
    cmp dil, '"'
    je .yes
    xor eax, eax
    ret
.yes:
    mov eax, 1
    ret

; reader_skip_ws() — consumes whitespace and ';' line comments.
reader_skip_ws:
.loop:
    call reader_peek
    cmp rax, -1
    je .done
    cmp al, ' '
    je .adv
    cmp al, 9
    je .adv
    cmp al, 10
    je .adv
    cmp al, 13
    je .adv
    cmp al, ';'
    je .comment
    jmp .done
.adv:
    inc qword [reader_pos]
    jmp .loop
.comment:
    inc qword [reader_pos]
.comment_loop:
    call reader_peek
    cmp rax, -1
    je .done
    cmp al, 10
    je .loop
    inc qword [reader_pos]
    jmp .comment_loop
.done:
    ret

; read_number() -> rax = tagged fixnum or tagged float. Assumes current
; char is '-' or a digit. A '.' followed by at least one digit right
; after the integer part switches this to a float literal (HDR_FLOAT,
; see floats.asm); anything else (including a bare trailing '.') leaves
; it a plain integer. An e/E exponent after the digits is read by
; read_form's caller side (read_exponent_float), which rescans the
; whole token.
;
; Integer range (issue #550): a fixnum here is 62 bits wide (tags.inc),
; so the representable range is [-2^61, 2^61-1], narrower than KERNEL.md
; Part II's i64. A decimal integer token outside that range reads as a
; Float — KERNEL.md's own rule for a token outside *its* range, applied
; at this kernel's narrower width — never as a silently wrapped fixnum.
; The magnitude is accumulated exactly in a u64 while it fits (mul/add
; carry detects the spill), then exactly in bigdec_limbs (base 2^32) up
; to 2048 bits; either way it converts to the correctly rounded double
; (u64_to_xmm0 / bigdec_to_xmm0). Past 2048 bits (~616 digits) the value
; exceeds the largest double, so it reads as +/-inf, as the reference's
; f64 parse does.
read_number:
    push rbx
    push r12
    push r13
    push r14
    xor r12, r12                  ; sign flag: 0 = positive
    call reader_peek
    cmp al, '-'
    jne .digits
    mov r12, 1
    inc qword [reader_pos]
.digits:
    xor rbx, rbx                   ; exact unsigned magnitude (r13 == 0)
    xor r13, r13                     ; 1 once the magnitude spilled past
                                       ; u64 into bigdec_limbs; 2 once it
                                       ; outgrew those too (reads as inf)
.loop:
    call reader_peek
    cmp rax, -1
    je .int_done
    cmp al, '0'
    jb .int_done
    cmp al, '9'
    ja .int_done
    movzx r14, al
    sub r14, '0'                       ; r14 = this digit's value
    test r13, r13
    jnz .loop_big
    mov rax, rbx
    mov rcx, 10
    mul rcx                              ; rdx:rax = rbx*10; CF iff rdx != 0
    jc .spill
    add rax, r14
    jc .spill
    mov rbx, rax
    jmp .next_digit
.spill:
    mov rax, rbx                           ; the magnitude *before* this
    mov ecx, eax                             ; digit, as two base-2^32
    mov [rel bigdec_limbs], rcx                ; limbs; the digit itself
    shr rax, 32                                  ; folds in just below
    mov [rel bigdec_limbs+8], rax
    mov qword [rel bigdec_n], 2
    mov r13, 1
.loop_big:
    cmp r13, 2
    je .next_digit                     ; already past the cap: digits no
    mov rdi, r14                         ; longer change the (infinite)
    call bigdec_mul10_add                  ; result
    test rax, rax
    jz .next_digit
    mov r13, 2
.next_digit:
    inc qword [reader_pos]
    jmp .loop
.int_done:
    ; float literal? needs '.' followed by at least one digit
    mov rax, [reader_pos]
    mov rcx, [reader_buf]
    cmp rax, [reader_end]
    jae .fixnum_done
    movzx rax, byte [rcx+rax]
    cmp al, '.'
    jne .fixnum_done
    mov rax, [reader_pos]
    inc rax
    cmp rax, [reader_end]
    jae .fixnum_done
    mov rcx, [reader_buf]
    movzx rax, byte [rcx+rax]
    cmp al, '0'
    jb .fixnum_done
    cmp al, '9'
    ja .fixnum_done
    jmp .float_literal

.fixnum_done:
    test r13, r13
    jnz .int_as_float
    test r12, r12
    jz .pos
    mov rax, FIXNUM_NEG_LIMIT
    cmp rbx, rax                     ; magnitude of -2^61 is the most a
    ja .int_as_float                   ; negative fixnum can carry
    neg rbx
    jmp .emit_fixnum
.pos:
    mov rax, FIXNUM_MAX
    cmp rbx, rax
    ja .int_as_float
.emit_fixnum:
    mov rax, rbx
    TO_FIXNUM rax
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

.int_as_float:
    call .int_part_to_xmm3
    movsd xmm0, xmm3
    jmp .apply_sign

; xmm3 = the integer part as a correctly rounded f64, from whichever of
; rbx / bigdec_limbs holds it (r13). Clobbers rax, rcx, rdx, rsi, rdi,
; r8, r9, xmm0, xmm1.
.int_part_to_xmm3:
    cmp r13, 1
    je .int_part_big
    ja .int_part_inf
    mov rdi, rbx
    call u64_to_xmm0
    movsd xmm3, xmm0
    ret
.int_part_big:
    call bigdec_to_xmm0
    movsd xmm3, xmm0
    ret
.int_part_inf:
    mov rax, 0x7FF0000000000000
    movq xmm3, rax
    ret

.float_literal:
    call .int_part_to_xmm3                ; before r13 is reused below
    inc qword [reader_pos]              ; consume '.'
    xor r13, r13                          ; fractional digit accumulator
    xor r14, r14                            ; count of fractional digits
.frac_loop:
    call reader_peek
    cmp rax, -1
    je .frac_done
    cmp al, '0'
    jb .frac_done
    cmp al, '9'
    ja .frac_done
    imul r13, r13, 10
    movzx rax, al
    sub rax, '0'
    add r13, rax
    inc r14
    inc qword [reader_pos]
    jmp .frac_loop
.frac_done:
    movsd xmm0, xmm3                     ; int part
    cvtsi2sd xmm1, r13                      ; fractional numerator
    mov rax, 1
    mov rcx, r14
.pow_loop:
    test rcx, rcx
    jz .pow_done
    imul rax, rax, 10
    dec rcx
    jmp .pow_loop
.pow_done:
    cvtsi2sd xmm2, rax                       ; 10^(fractional digit count)
    divsd xmm1, xmm2
    addsd xmm0, xmm1
.apply_sign:
    test r12, r12
    jz .float_pos
    mov rax, 0x8000000000000000                ; flip sign bit
    movq xmm2, rax
    xorpd xmm0, xmm2
.float_pos:
    call make_float
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; bigdec_mul10_add(rdi=digit 0..9) -> bigdec := bigdec*10 + digit;
; rax = 0, or 1 if the result no longer fits BIGDEC_MAX_LIMBS (bigdec
; is then stale; read_number stops using it). Clobbers rcx, rdx, rsi,
; r8, r9.
bigdec_mul10_add:
    mov r9, rdi                            ; carry in
    xor rcx, rcx
    mov r8, [rel bigdec_n]
    lea rsi, [rel bigdec_limbs]
.limb:
    cmp rcx, r8
    jae .limbs_done
    mov rax, [rsi+rcx*8]
    imul rax, rax, 10
    add rax, r9                              ; < 2^36: no 64-bit overflow
    mov r9, rax
    shr r9, 32
    mov eax, eax                               ; low 32 bits, zero-extended
    mov [rsi+rcx*8], rax
    inc rcx
    jmp .limb
.limbs_done:
    test r9, r9
    jz .fits
    cmp r8, BIGDEC_MAX_LIMBS
    jae .full
    mov [rsi+r8*8], r9
    inc r8
    mov [rel bigdec_n], r8
.fits:
    xor eax, eax
    ret
.full:
    mov eax, 1
    ret

; bigdec_to_xmm0() -> xmm0 = bigdec (>= 2^64, so >= 3 limbs, top limb
; nonzero) as the correctly rounded f64: its top 64 bits, with every
; lower bit OR'd into bit 0 as a sticky bit, go through u64_to_xmm0
; (one rounding, to nearest-even), then an exact power-of-two scale
; (which can only overflow to inf — no second rounding). Clobbers rax,
; rcx, rdx, rsi, rdi, r8, r9, xmm1.
bigdec_to_xmm0:
    push rbx
    lea rsi, [rel bigdec_limbs]
    mov r8, [rel bigdec_n]
    mov rax, [rsi+r8*8-8]                  ; top limb
    bsr rcx, rax                             ; its highest set bit, 0..31
    mov r9, 31
    sub r9, rcx                                ; r9 = leading zeros in it
    lea rbx, [r8-1]
    shl rbx, 5
    lea rbx, [rbx+rcx+1]                         ; rbx = bit length
    lea rcx, [r9+32]
    shl rax, cl                                    ; top limb -> bit 63 down
    mov rdx, [rsi+r8*8-16]
    mov rcx, r9
    shl rdx, cl
    or rax, rdx                                      ; next limb below it
    mov rdx, [rsi+r8*8-24]
    mov rdi, rdx
    mov rcx, 32
    sub rcx, r9                                        ; 1..32
    shr rdx, cl
    or rax, rdx                                          ; third limb's top bits
    mov rdx, 1
    shl rdx, cl
    dec rdx
    and rdi, rdx                          ; sticky: third limb's dropped bits
    lea rcx, [r8-4]
.sticky:
    test rcx, rcx
    js .sticky_done
    or rdi, [rsi+rcx*8]                     ; ...and every limb below it
    dec rcx
    jmp .sticky
.sticky_done:
    test rdi, rdi
    jz .no_sticky
    or rax, 1
.no_sticky:
    mov rdi, rax
    call u64_to_xmm0
    sub rbx, 64                             ; scale = bit length - 64, >= 1
.scale:
    cmp rbx, 1000
    jle .scale_last
    mov rax, (1000 + 1023) << 52              ; 2^1000
    movq xmm1, rax
    mulsd xmm0, xmm1
    sub rbx, 1000
    jmp .scale
.scale_last:
    lea rax, [rbx+1023]
    shl rax, 52                                 ; 2^rbx
    movq xmm1, rax
    mulsd xmm0, xmm1
    pop rbx
    ret

; u64_to_xmm0(rdi=unsigned 64-bit) -> xmm0 = rdi as f64, correctly
; rounded. cvtsi2sd is signed-only, so a value with bit 63 set is halved
; first (keeping the shifted-out bit as a sticky bit, so the final
; rounding is still correct) and doubled back. Clobbers rax, rcx.
u64_to_xmm0:
    test rdi, rdi
    js .big
    cvtsi2sd xmm0, rdi
    ret
.big:
    mov rax, rdi
    mov rcx, rdi
    shr rax, 1
    and rcx, 1
    or rax, rcx
    cvtsi2sd xmm0, rax
    addsd xmm0, xmm0
    ret

section .text
; read_exponent_float(rdi = buffer index of a number token that
; read_number has just read, and that continues with a valid exponent)
; -> rax = the whole token, "-? digits (. digits)? [eE] [+-]? digits",
; as a tagged float; reader_pos is left just past the exponent. The
; token is rescanned from its start so every digit takes part: the
; significand's digits become one integer M and the value is
; M * 10^(exp - fraction digits). When M <= 2^53 and that power of ten
; is within 10^22 both are exact doubles, so the one multiply or divide
; rounds correctly (Clinger's fast path) — "1.5e2" is exactly 150.0 and
; "1.5e-3" the double nearest 0.0015. Outside that range the scaling is
; repeated by 10^22 steps, which can be off in the last place (the
; fraction reader above has the same limit; a correctly rounded
; general path needs big-integer arithmetic). Significand digits past
; the 18th are dropped (an integer digit still scales the exponent).
read_exponent_float:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov r8, [reader_buf]
    mov rcx, rdi                        ; scan index
    xor r12, r12                          ; sign: 1 if negative
    xor rbx, rbx                            ; M
    xor r13, r13                              ; 1 once M stopped growing
    xor r14, r14                                ; decimal exponent adjust
    cmp byte [r8+rcx], '-'
    jne .int_digits
    mov r12, 1
    inc rcx
.int_digits:
    movzx eax, byte [r8+rcx]
    sub eax, '0'
    cmp eax, 9
    ja .int_done
    inc rcx
    test r13, r13
    jnz .int_dropped
    mov rdx, 100000000000000000         ; 10^17: M*10+9 stays below 2^63
    cmp rbx, rdx
    jae .int_full
    imul rbx, rbx, 10
    add rbx, rax
    jmp .int_digits
.int_full:
    mov r13, 1
.int_dropped:
    inc r14                               ; a dropped integer digit: x10
    jmp .int_digits
.int_done:
    cmp byte [r8+rcx], '.'
    jne .exponent
    inc rcx
.frac_digits:
    movzx eax, byte [r8+rcx]
    sub eax, '0'
    cmp eax, 9
    ja .exponent
    inc rcx
    test r13, r13
    jnz .frac_digits                      ; a dropped fraction digit
    mov rdx, 100000000000000000
    cmp rbx, rdx
    jae .frac_full
    imul rbx, rbx, 10
    add rbx, rax
    dec r14
    jmp .frac_digits
.frac_full:
    mov r13, 1
    jmp .frac_digits
.exponent:
    inc rcx                               ; the 'e' / 'E' (caller checked)
    xor r15, r15                            ; exponent sign: 1 if negative
    movzx eax, byte [r8+rcx]
    cmp al, '+'
    je .exp_sign_done
    cmp al, '-'
    jne .exp_digits_start
    mov r15, 1
.exp_sign_done:
    inc rcx
.exp_digits_start:
    xor r9, r9                               ; exponent magnitude
.exp_digits:
    cmp rcx, [reader_end]
    jae .exp_done
    movzx eax, byte [r8+rcx]
    sub eax, '0'
    cmp eax, 9
    ja .exp_done
    inc rcx
    cmp r9, 100000                             ; far past any double's
    jae .exp_digits                              ; range: stop growing
    imul r9, r9, 10
    add r9, rax
    jmp .exp_digits
.exp_done:
    mov [reader_pos], rcx
    test r15, r15
    jz .exp_pos
    neg r9
.exp_pos:
    add r14, r9                              ; r14 = total power of ten
    cvtsi2sd xmm0, rbx                         ; exact while M <= 2^53
.scale_up:
    cmp r14, 22
    jle .scale_down
    mulsd xmm0, [rel pow10_table + 22*8]
    sub r14, 22
    jmp .scale_up
.scale_down:
    cmp r14, -22
    jge .scale_last
    divsd xmm0, [rel pow10_table + 22*8]
    add r14, 22
    jmp .scale_down
.scale_last:
    test r14, r14
    js .scale_div
    lea rax, [rel pow10_table]
    mulsd xmm0, [rax + r14*8]
    jmp .signed
.scale_div:
    neg r14
    lea rax, [rel pow10_table]
    divsd xmm0, [rax + r14*8]
.signed:
    test r12, r12
    jz .make
    mov rax, 0x8000000000000000                  ; flip the sign bit, so
    movq xmm1, rax                                 ; "-0e5" is -0.0
    xorpd xmm0, xmm1
.make:
    call make_float
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

section .rodata
align 8
; 10^0 .. 10^22: every one exactly representable as a double.
pow10_table:
    dq 1.0e0, 1.0e1, 1.0e2, 1.0e3, 1.0e4, 1.0e5, 1.0e6, 1.0e7
    dq 1.0e8, 1.0e9, 1.0e10, 1.0e11, 1.0e12, 1.0e13, 1.0e14, 1.0e15
    dq 1.0e16, 1.0e17, 1.0e18, 1.0e19, 1.0e20, 1.0e21, 1.0e22

section .bss
align 8
symbuf: resb 256
; read_number's exact accumulator for integer tokens past u64 (base-2^32
; limbs, least significant first, one per qword). 64 limbs = 2048 bits,
; past the largest double (< 2^1024), so the cap never changes a result.
bigdec_limbs: resq BIGDEC_MAX_LIMBS
bigdec_n: resq 1

section .text

; read_symbol() -> rax = tagged interned symbol. Uppercases into a scratch
; buffer (the source buffer itself is not mutated).
read_symbol:
    push rbx
    xor rbx, rbx                   ; length so far
.loop:
    call reader_peek
    cmp rax, -1
    je .done
    mov dil, al
    call is_delim
    test al, al
    jnz .done
    call reader_peek
    cmp al, 'a'
    jb .store
    cmp al, 'z'
    ja .store
    sub al, 32                     ; lowercase -> uppercase
.store:
    mov [symbuf + rbx], al
    inc rbx
    inc qword [reader_pos]
    cmp rbx, 255
    jae .done                       ; defensive cap; see README roadmap
    jmp .loop
.done:
    ; "NIL" is read as the literal empty-list immediate directly, never
    ; interned as a symbol at all — matching the Rust reference's own
    ; reader (reader.rs: `"NIL" => LispVal::Nil`, distinct from "T",
    ; which the reference reads as an ordinary interned symbol needing
    ; its own self-binding bootstrap — see symtab.asm's
    ; bootstrap_globals). An earlier version of this reader treated
    ; bareword NIL as an ordinary (permanently unbound) symbol instead,
    ; a real conformance bug: `(IF NIL 1 2)` evaluated NIL as an
    ; unbound global variable reference (truthy, since only the literal
    ; NIL immediate is false) and returned 1, and reference stdlib code
    ; uses bareword `nil` constantly as a self-evaluating literal.
    cmp rbx, 3
    jne .intern
    cmp byte [symbuf], 'N'
    jne .intern
    cmp byte [symbuf+1], 'I'
    jne .intern
    cmp byte [symbuf+2], 'L'
    jne .intern
    mov rax, IMM_NIL
    pop rbx
    ret
.intern:
    mov rdi, symbuf
    mov rsi, rbx
    call intern_symbol
    pop rbx
    ret

section .bss
align 8
strbuf: resb 4096
one_plus_minus_buf: resb 2

section .text

; read_string() -> rax = tagged HDR_STRING heapobj. Assumes the current
; char is the opening '"'. Escapes are the reference's (reader.rs
; parse_string): \n \t \r \0 \" and a doubled backslash decode; any
; other backslash-prefixed character keeps its backslash ("\a" is the two
; characters backslash, a), rather than silently dropping it. Unterminated
; input or a literal longer than the scratch buffer simply stops early
; (v0 — no reader error reporting yet, see README roadmap).
read_string:
    push rbx
    inc qword [reader_pos]          ; consume opening '"'
    xor rbx, rbx                     ; length so far
.loop:
    call reader_peek
    cmp rax, -1
    je .done
    cmp al, '"'
    je .close
    cmp al, '\'
    je .escape
    mov [strbuf + rbx], al
    inc rbx
    inc qword [reader_pos]
    cmp rbx, 4095
    jae .done
    jmp .loop
.escape:
    inc qword [reader_pos]              ; consume backslash
    call reader_peek
    cmp rax, -1
    je .done
    cmp al, 'n'
    je .esc_n
    cmp al, 't'
    je .esc_t
    cmp al, 'r'
    je .esc_r
    cmp al, '0'
    je .esc_0
    cmp al, '"'
    je .esc_literal
    cmp al, 92                            ; a doubled backslash
    je .esc_literal
    mov byte [strbuf + rbx], 92           ; unknown escape: keep the
    inc rbx                                 ; backslash, then the char
    cmp rbx, 4095
    jae .done
.esc_literal:
    mov [strbuf + rbx], al
    inc rbx
    inc qword [reader_pos]
    cmp rbx, 4095
    jae .done
    jmp .loop
.esc_r:
    mov byte [strbuf + rbx], 13
    inc rbx
    inc qword [reader_pos]
    cmp rbx, 4095
    jae .done
    jmp .loop
.esc_0:
    mov byte [strbuf + rbx], 0
    inc rbx
    inc qword [reader_pos]
    cmp rbx, 4095
    jae .done
    jmp .loop
.esc_n:
    mov byte [strbuf + rbx], 10
    inc rbx
    inc qword [reader_pos]
    cmp rbx, 4095
    jae .done
    jmp .loop
.esc_t:
    mov byte [strbuf + rbx], 9
    inc rbx
    inc qword [reader_pos]
    cmp rbx, 4095
    jae .done
    jmp .loop
.close:
    inc qword [reader_pos]                ; consume closing '"'
.done:
    mov rdi, strbuf
    mov rsi, rbx
    call make_string
    pop rbx
    ret

; read_list() -> rax = tagged list, consuming up to and including the
; closing ')'. Caller has already consumed the opening '('. A dotted
; tail follows the reference (reader.rs parse_list_contents): a '.' at
; the start of an element, after at least one element, is the dot — the
; next form is the list's final cdr and must be followed by ')'. So
; (A . B) is a pair, (A . (B C)) is (A B C), and (. A), (A .), (A . B C)
; and (A . B . C) are read errors, as is input ending inside a list.
read_list:
    push r12
    call reader_skip_ws
    call reader_peek
    cmp rax, -1
    je read_fail_unterminated
    cmp al, ')'
    jne .have_first
    inc qword [reader_pos]
    mov rax, IMM_NIL
    pop r12
    ret
.have_first:
    cmp al, '.'
    je read_fail_dot                ; "(. x)": no element before the dot
    call read_form
    mov r12, rax                    ; save first
    call read_list_rest             ; the rest, after >= 1 element
    mov rsi, rax                     ; cdr = rest
    mov rdi, r12                     ; car = first
    call cons
    pop r12
    ret

; read_list_rest() -> rax = the rest of a list at least one element of
; which has been read: NIL at ')', a dotted tail's datum, or the next
; element consed onto the rest. Recursive, like read_list.
read_list_rest:
    push r12
    call reader_skip_ws
    call reader_peek
    cmp rax, -1
    je read_fail_unterminated
    cmp al, ')'
    je .close
    cmp al, '.'
    je .dot
    call read_form
    mov r12, rax
    call read_list_rest
    mov rsi, rax
    mov rdi, r12
    call cons
    pop r12
    ret
.close:
    inc qword [reader_pos]
    mov rax, IMM_NIL
    pop r12
    ret
.dot:
    inc qword [reader_pos]          ; consume '.'
    call reader_skip_ws
    call reader_peek
    cmp rax, -1
    je read_fail_unterminated
    cmp al, ')'
    je read_fail_dot                ; "(a .)": nothing after the dot
    cmp al, '.'
    je read_fail_dot                ; "(a . . b)"
    call read_form
    mov r12, rax                    ; the tail datum
    call reader_skip_ws
    call reader_peek
    cmp rax, -1
    je read_fail_unterminated
    cmp al, ')'
    jne read_fail_dot               ; "(a . b c)" / "(a . b . c)"
    inc qword [reader_pos]          ; consume ')'
    mov rax, r12
    pop r12
    ret

; reader_fail(rsi=message, rdx=length) — never returns. Signals a READ
; error as an ordinary catchable condition (fail_wrong_type), after
; restoring an enclosing read_from_string_tagged's caller's reader
; position (see rfs_saved). Jumped to (never called) from read_list/
; read_list_rest with exactly their own `push r12` outstanding; popping
; it leaves rsp as at their entry, the same alignment car's own
; `jmp fail_wrong_type` has.
read_fail_dot:
    mov rsi, read_dot_err_msg
    mov rdx, read_dot_err_msg_len
    jmp reader_fail
read_fail_unterminated:
    mov rsi, read_eof_err_msg
    mov rdx, read_eof_err_msg_len
reader_fail:
    mov rax, [rfs_saved]
    test rax, rax
    jz .signal
    mov rcx, [rax]                  ; prev rfs_saved
    mov [rfs_saved], rcx
    mov rcx, [rax+8]
    mov [reader_end], rcx
    mov rcx, [rax+16]
    mov [reader_pos], rcx
    mov rcx, [rax+24]
    mov [reader_buf], rcx
.signal:
    pop r12
    mov rdi, IMM_NIL
    jmp fail_wrong_type

; read_form() -> rax = next tagged value, or IMM_EOF if input is exhausted.
global read_form
read_form:
    push r12
    call reader_skip_ws
    call reader_peek
    cmp rax, -1
    je .eof

    cmp al, '('
    jne .not_list
    inc qword [reader_pos]
    call read_list
    jmp .out

.not_list:
    cmp al, 39                       ; '
    jne .not_quote
    ; Char literal ("'x'", KERNEL.md Part II) is tried before quote
    ; sugar: a '\'' followed by exactly one character (or one \n \t \r
    ; \\ \' \0 escape) and a closing '\''. `'a'` is the character `a`,
    ; but `'a` followed by a delimiter — no closing '\'' right after —
    ; falls through to ordinary quote sugar, `(QUOTE A)`; `''` (nothing
    ; between the quotes) is likewise not a char literal, per spec.
    mov rdx, [reader_pos]              ; index of the opening '\''
    mov rcx, [reader_buf]
    lea r8, [rdx+1]
    cmp r8, [reader_end]
    jae .quote_sugar                   ; nothing after the opening '\''
    movzx r9, byte [rcx+r8]            ; byte right after '\''
    cmp r9b, 92                        ; '\\' — a possible escape
    je .maybe_escaped_char
    cmp r9b, 39                        ; '\'' immediately — "''" is empty
    je .quote_sugar
    lea r10, [rdx+2]
    cmp r10, [reader_end]
    jae .quote_sugar
    movzx r11, byte [rcx+r10]
    cmp r11b, 39                       ; closing '\''?
    jne .quote_sugar
    add qword [reader_pos], 3
    movzx rdi, r9b
    call tag_char
    jmp .out
.maybe_escaped_char:
    lea r10, [rdx+2]
    cmp r10, [reader_end]
    jae .quote_sugar
    movzx r11, byte [rcx+r10]           ; the escaped character
    lea r9, [rdx+3]
    cmp r9, [reader_end]
    jae .quote_sugar
    movzx r9, byte [rcx+r9]
    cmp r9b, 39                         ; closing '\''?
    jne .quote_sugar
    add qword [reader_pos], 4
    mov rdi, r11
    call decode_char_escape
    mov rdi, rax
    call tag_char
    jmp .out
.quote_sugar:
    inc qword [reader_pos]
    call read_form
    mov r12, rax                      ; quoted datum
    jmp .quote_fixed

.not_quote:
    cmp al, 96                         ; ` (backtick)
    jne .not_quasiquote
    inc qword [reader_pos]
    call read_form
    mov r12, rax                        ; templated datum
    jmp .quasiquote_fixed

.not_quasiquote:
    cmp al, ','                        ; ,  or  ,@
    jne .not_unquote
    mov rax, [reader_pos]
    mov rcx, [reader_buf]
    inc rax
    cmp rax, [reader_end]
    jae .plain_unquote
    movzx rax, byte [rcx+rax]
    cmp al, '@'
    jne .plain_unquote
    add qword [reader_pos], 2             ; consume ',' and '@'
    call read_form
    mov r12, rax
    jmp .unquote_splicing_fixed
.plain_unquote:
    inc qword [reader_pos]                  ; consume ','
    call read_form
    mov r12, rax
    jmp .unquote_fixed

.not_unquote:
    cmp al, '#'
    jne .not_sharp_quote
    mov rax, [reader_pos]
    mov rcx, [reader_buf]
    inc rax
    cmp rax, [reader_end]
    jae .not_sharp_quote
    movzx rax, byte [rcx+rax]
    cmp al, 39                          ; '
    jne .not_sharp_quote
    add qword [reader_pos], 2             ; consume '#' and '\''
    call read_form
    mov r12, rax                            ; #'-quoted datum
    jmp .function_fixed

.not_sharp_quote:
    cmp al, '"'
    jne .not_string
    call read_string
    jmp .out

.not_string:
    ; "1+"/"1-": two-character literal symbols, tried before ordinary
    ; number parsing (KERNEL.md Part II) — no boundary guard, so "1+x"
    ; reads as the symbol 1+ followed by X. No other digit-leading
    ; symbol exists; this is the one exception to "a leading digit
    ; always starts a number" below. al must hold the *original* first
    ; character again before falling through to .not_one_plus_minus —
    ; every check below it assumes that.
    cmp al, '1'
    jne .not_one_plus_minus
    mov rdx, [reader_pos]
    mov rcx, [reader_buf]
    lea r8, [rdx+1]
    cmp r8, [reader_end]
    jae .not_one_plus_minus
    movzx r8, byte [rcx+r8]
    cmp r8b, '+'
    je .one_plus_minus
    cmp r8b, '-'
    jne .not_one_plus_minus
.one_plus_minus:
    mov byte [one_plus_minus_buf], '1'
    mov [one_plus_minus_buf+1], r8b              ; '+' or '-'
    add qword [reader_pos], 2                      ; consume both characters
    mov rdi, one_plus_minus_buf
    mov rsi, 2
    call intern_symbol
    jmp .out
.not_one_plus_minus:
    cmp al, '-'
    je .maybe_number
    cmp al, '0'
    jb .symbol
    cmp al, '9'
    ja .symbol
    jmp .number

.maybe_number:
    ; '-' starts a number only if followed by a digit; otherwise it is a
    ; symbol (e.g. a bare '-' or '-foo').
    mov rax, [reader_pos]
    mov rcx, [reader_buf]
    inc rax
    cmp rax, [reader_end]
    jae .symbol
    movzx rax, byte [rcx+rax]
    cmp al, '0'
    jb .symbol
    cmp al, '9'
    ja .symbol
    jmp .number

.number:
    mov r12, [reader_pos]              ; token start, for an exponent rescan
    call read_number
    ; An e/E exponent ([+-]? digit+) right after the number makes the
    ; whole token one float ("1.5e2", "1e5", "-2E-3"), as the
    ; reference's parse_float reads it — never a number followed by a
    ; symbol E2. Anything else after the digits is left as before.
    mov rcx, [reader_pos]
    mov rdx, [reader_buf]
    cmp rcx, [reader_end]
    jae .out
    movzx r8d, byte [rdx+rcx]
    or r8b, 0x20                        ; E -> e
    cmp r8b, 'e'
    jne .out
    lea r9, [rcx+1]
    cmp r9, [reader_end]
    jae .out
    movzx r8d, byte [rdx+r9]
    cmp r8b, '+'
    je .exp_signed
    cmp r8b, '-'
    jne .exp_digit
.exp_signed:
    inc r9
    cmp r9, [reader_end]
    jae .out
    movzx r8d, byte [rdx+r9]
.exp_digit:
    cmp r8b, '0'
    jb .out
    cmp r8b, '9'
    ja .out
    mov rdi, r12
    call read_exponent_float
    jmp .out

.symbol:
    call read_symbol
    jmp .out

.eof:
    mov rax, IMM_EOF
    jmp .out

.quote_fixed:
    ; datum is in r12; build (QUOTE datum) = (QUOTE . (datum . NIL))
    mov rdi, r12
    mov rsi, IMM_NIL
    call cons                          ; (datum . nil)
    mov r12, rax
    mov rdi, symbuf_quote
    mov rsi, 5
    call intern_symbol
    mov rdi, rax
    mov rsi, r12
    call cons                          ; (QUOTE . (datum . nil))
    jmp .out

.function_fixed:
    ; datum is in r12; build (FUNCTION datum), same shape as QUOTE above.
    mov rdi, r12
    mov rsi, IMM_NIL
    call cons
    mov r12, rax
    mov rdi, symbuf_function
    mov rsi, 8
    call intern_symbol
    mov rdi, rax
    mov rsi, r12
    call cons                          ; (FUNCTION . (datum . nil))
    jmp .out

.quasiquote_fixed:
    ; datum is in r12; build (QUASIQUOTE datum), same shape as QUOTE.
    mov rdi, r12
    mov rsi, IMM_NIL
    call cons
    mov r12, rax
    mov rdi, symbuf_quasiquote
    mov rsi, 10
    call intern_symbol
    mov rdi, rax
    mov rsi, r12
    call cons
    jmp .out

.unquote_fixed:
    ; datum is in r12; build (UNQUOTE datum), same shape as QUOTE.
    mov rdi, r12
    mov rsi, IMM_NIL
    call cons
    mov r12, rax
    mov rdi, symbuf_unquote
    mov rsi, 7
    call intern_symbol
    mov rdi, rax
    mov rsi, r12
    call cons
    jmp .out

.unquote_splicing_fixed:
    ; datum is in r12; build (UNQUOTE-SPLICING datum), same shape as QUOTE.
    mov rdi, r12
    mov rsi, IMM_NIL
    call cons
    mov r12, rax
    mov rdi, symbuf_unquote_splicing
    mov rsi, 16
    call intern_symbol
    mov rdi, rax
    mov rsi, r12
    call cons

.out:
    pop r12
    ret

section .rodata
symbuf_quote: db "QUOTE"
symbuf_function: db "FUNCTION"
symbuf_quasiquote: db "QUASIQUOTE"
symbuf_unquote: db "UNQUOTE"
symbuf_unquote_splicing: db "UNQUOTE-SPLICING"
car_err_msg: db "CAR: expected a cons or NIL"
car_err_msg_len: equ $ - car_err_msg
cdr_err_msg: db "CDR: expected a cons or NIL"
cdr_err_msg_len: equ $ - cdr_err_msg
read_dot_err_msg: db "READ: malformed dotted list"
read_dot_err_msg_len: equ $ - read_dot_err_msg
read_eof_err_msg: db "READ: end of input inside a list"
read_eof_err_msg_len: equ $ - read_eof_err_msg

