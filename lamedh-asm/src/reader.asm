; reader.asm — s-expression reader over an in-memory buffer, plus the
; cons-cell primitives every later stage (compiler, printer extensions)
; builds on.
;
; v0 grammar: integers (optional leading '-'), symbols (uppercased on
; intern, per the Rust reader's convention), proper lists "( form* )",
; and 'x quote sugar. No dotted pairs, floats, strings, or radix
; literals yet — see README roadmap.

%include "src/tags.inc"

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
; see floats.asm); anything else (including a bare trailing '.', not
; used by this project's grammar — no dotted pairs yet) leaves it a
; plain fixnum.
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
    xor rbx, rbx                   ; accumulator (unsigned magnitude)
.loop:
    call reader_peek
    cmp rax, -1
    je .int_done
    cmp al, '0'
    jb .int_done
    cmp al, '9'
    ja .int_done
    imul rbx, rbx, 10
    movzx rax, al
    sub rax, '0'
    add rbx, rax
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
    test r12, r12
    jz .pos
    neg rbx
.pos:
    mov rax, rbx
    TO_FIXNUM rax
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

.float_literal:
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
    cvtsi2sd xmm0, rbx                   ; int part
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

section .bss
align 8
symbuf: resb 256

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
; char is the opening '"'. Minimal escapes only: \n \t \" \\; anything
; else after a backslash is copied through literally. Unterminated
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
    mov [strbuf + rbx], al                ; \" \\ and anything else: literal
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

; read_list() -> rax = tagged proper list, consuming up to and including
; the closing ')'. Caller has already consumed the opening '('.
read_list:
    push r12
    call reader_skip_ws
    call reader_peek
    cmp al, ')'
    jne .have_first
    inc qword [reader_pos]
    mov rax, IMM_NIL
    pop r12
    ret
.have_first:
    call read_form
    mov r12, rax                    ; save first
    call reader_skip_ws
    call read_list                  ; recursive: rest of the list
    mov rsi, rax                     ; cdr = rest
    mov rdi, r12                     ; car = first
    call cons
    pop r12
    ret

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
    call read_number
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
