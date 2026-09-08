; reader.asm — s-expression reader over an in-memory buffer, plus the
; cons-cell primitives every later stage (compiler, printer extensions)
; builds on.
;
; v0 grammar: integers (optional leading '-'), symbols (uppercased on
; intern, per the Rust reader's convention), proper lists "( form* )",
; and 'x quote sugar. No dotted pairs, floats, strings, or radix
; literals yet — see README roadmap.

%include "src/tags.inc"

extern data_alloc
extern intern_symbol

section .bss
align 8
reader_buf: resq 1
reader_pos: resq 1
reader_end: resq 1

section .text

; --- cons-cell primitives -------------------------------------------

; cons(rdi=car, rsi=cdr) -> rax = tagged cons pointer
global cons
cons:
    push r8
    mov r8, rdi
    mov rdi, 16
    call data_alloc
    mov [rax], r8
    mov [rax+8], rsi
    or rax, TAG_CONS
    pop r8
    ret

; car(rdi=tagged cons) -> rax
global car
car:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax]
    ret

; cdr(rdi=tagged cons) -> rax
global cdr
cdr:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+8]
    ret

; --- reader -----------------------------------------------------------

; reader_init(rdi=buf, rsi=len)
global reader_init
reader_init:
    mov [reader_buf], rdi
    mov qword [reader_pos], 0
    mov [reader_end], rsi
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

; read_number() -> rax = tagged fixnum. Assumes current char is '-' or a digit.
read_number:
    push rbx
    push r12
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
    je .done
    cmp al, '0'
    jb .done
    cmp al, '9'
    ja .done
    imul rbx, rbx, 10
    movzx rax, al
    sub rax, '0'
    add rbx, rax
    inc qword [reader_pos]
    jmp .loop
.done:
    test r12, r12
    jz .pos
    neg rbx
.pos:
    mov rax, rbx
    TO_FIXNUM rax
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
    mov rdi, symbuf
    mov rsi, rbx
    call intern_symbol
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
    inc qword [reader_pos]
    call read_form
    mov r12, rax                      ; quoted datum
    jmp .quote_fixed

.not_quote:
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

.out:
    pop r12
    ret

section .rodata
symbuf_quote: db "QUOTE"
