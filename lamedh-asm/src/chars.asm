; chars.asm — the Char value type (KERNEL.md Part II/IV): a code point
; in 0..=255, distinct from Number/fixnum even when numerically equal
; (`(eq 'a' 97)` is `NIL` — Part IV; arithmetic/comparison contagion
; coerces a Char to its code, Part V, but this v0 kernel doesn't wire
; that contagion into `+`/`-`/`*`/`<`/`=` yet, see README v0 limits).
;
; Represented as an immediate (tag 11, like NIL/T/UNBOUND/EOF in
; tags.inc), not a heapobj — a Char is one byte of payload, cheaper to
; pack directly into the tagged word than to allocate. The immediate
; enumeration in tags.inc only used values 0..4 before this; a Char's
; enumeration index is 256+code, leaving both ranges completely
; disjoint (a fixnum's own tag bits, and every other tag, rule out any
; collision with the small existing IMM_* set here, and 256+255=511
; fits trivially in the 62 free bits above the tag).
;
; This gives EQ on two Chars for free: is_char_tagged below is only
; needed by print/CHAR-CODE/MAKE-CHAR, never by lisp_eq — two equal
; Chars are the identical tagged word already, so lisp_eq's own fast
; `cmp rdi, rsi` path (strings.asm) already returns true before ever
; asking what kind of value either operand is, and two different Chars
; are two different immediate words, correctly never EQ.

%include "src/tags.inc"

extern write_buf
extern fail_wrong_type

extern is_string
extern string_len
extern string_bytes
section .rodata
char_code_type_msg: db "CHAR-CODE: expected a character or a one-character string"
char_code_type_msg_len: equ $ - char_code_type_msg
section .text

; is_char_tagged(rdi=tagged value) -> rax = 1/0
global is_char_tagged
is_char_tagged:
    mov rax, rdi
    and rax, TAG_MASK
    xor rcx, rcx
    cmp rax, TAG_IMMEDIATE
    jne .no
    mov rax, rdi
    shr rax, 2
    cmp rax, IMM_CHAR_BASE
    jb .no
    cmp rax, IMM_CHAR_BASE + 255
    ja .no
    mov rax, 1
    ret
.no:
    xor rax, rax
    ret

; tag_char(rdi=code, 0..255) -> rax = tagged Char. No range check —
; callers (reader.asm's char-literal production, make_char_from_fixnum
; below) are responsible for the value already being in range.
global tag_char
tag_char:
    mov rax, rdi
    add rax, IMM_CHAR_BASE
    shl rax, 2
    or rax, TAG_IMMEDIATE
    ret

; char_code_tagged(rdi=tagged Char) -> rax = tagged fixnum code. The
; CHAR-CODE builtin's host half; caller (compiler.asm) guarantees rdi
; is actually a Char via is_char_tagged first, so no check here.
; char_code_tagged(rdi=a Char, or a one-character string) -> rax =
; tagged fixnum code. The reference's CHAR-CODE accepts a one-character
; string as well as a Char (lib/14-strings.lisp's char->code relies on
; it, and this kernel's own CODE-CHAR returns a one-character STRING —
; README "v0 limits"), so a string's first byte is its code here. It
; used to shift whatever it was given: a string argument came back as
; a pointer-sized garbage number, and base64's "hi" encoded as "//A=".
; Anything else is a real condition.
global char_code_tagged
char_code_tagged:
    push rbx
    mov rbx, rdi
    call is_char_tagged
    test rax, rax
    jz .not_char
    mov rax, rbx
    shr rax, 2
    sub rax, IMM_CHAR_BASE
    TO_FIXNUM rax
    pop rbx
    ret
.not_char:
    mov rdi, rbx
    call is_string
    test rax, rax
    jz .bad
    mov rdi, rbx
    call string_len
    test rax, rax
    jz .bad
    mov rdi, rbx
    call string_bytes
    movzx eax, byte [rax]
    TO_FIXNUM rax
    pop rbx
    ret
.bad:
    mov rdi, rbx
    mov rsi, char_code_type_msg
    mov rdx, char_code_type_msg_len
    call fail_wrong_type                  ; never returns

; make_char_from_fixnum(rdi=tagged fixnum) -> rax = tagged Char, or a
; wrong-type condition (fail_wrong_type, native_errors.asm — the same
; real CATCH/HANDLER-CASE/ERRORSET-signaling machinery CAR/CDR and
; "calling a non-callable value" already use) if the value isn't a
; fixnum in 0..255. The MAKE-CHAR builtin's host half.
global make_char_from_fixnum
make_char_from_fixnum:
    mov rax, rdi
    and rax, TAG_MASK
    test rax, rax                     ; TAG_FIXNUM == 0
    jnz .bad
    mov rax, rdi
    UNTAG_FIXNUM rax
    cmp rax, 0
    jl .bad
    cmp rax, 255
    jg .bad
    mov rdi, rax
    jmp tag_char
.bad:
    mov rsi, make_char_range_msg
    mov rdx, make_char_range_msg_len
    jmp fail_wrong_type

; code_char_string(rdi=tagged fixnum) -> rax = a fresh one-character
; HDR_STRING, per KERNEL.md's own explicit "(code-char n) returns a
; one-character string, not a Char" — an asymmetric pair with MAKE-CHAR
; (which returns a genuine Char) that the spec states outright, not an
; inconsistency this kernel introduced. v0 scope: like MAKE-CHAR,
; restricted to 0..255 (a single byte) rather than the reference's full
; Unicode code point range, since this kernel's strings are plain byte
; buffers with no UTF-8 encoder yet.
extern make_string
global code_char_string
code_char_string:
    mov rax, rdi
    and rax, TAG_MASK
    test rax, rax
    jnz .bad
    mov rax, rdi
    UNTAG_FIXNUM rax
    cmp rax, 0
    jl .bad
    cmp rax, 255
    jg .bad
    push rax
    mov rdi, rsp
    mov rsi, 1
    call make_string
    add rsp, 8
    ret
.bad:
    mov rsi, make_char_range_msg
    mov rdx, make_char_range_msg_len
    jmp fail_wrong_type

; print_char(rdi=tagged Char) — writes the PRIN1-style "'x'" form
; (KERNEL.md Part III): the same six escapes the reader recognizes
; (\n \t \r \\ \' \0), every other byte raw (including other control
; bytes and anything above ASCII — this kernel treats a Char as one
; raw byte, matching its own 0..255 representation exactly).
global print_char
print_char:
    push rbx
    mov rbx, rdi
    mov rsi, quote_buf
    mov rdx, 1
    call write_buf

    mov rax, rbx
    shr rax, 2
    sub rax, IMM_CHAR_BASE           ; rax = raw code 0..255

    cmp rax, 10
    je .esc_n
    cmp rax, 9
    je .esc_t
    cmp rax, 13
    je .esc_r
    cmp rax, 92
    je .esc_bs
    cmp rax, 39
    je .esc_q
    cmp rax, 0
    je .esc_z
    mov [char_byte_buf], al
    mov rsi, char_byte_buf
    mov rdx, 1
    call write_buf
    jmp .close
.esc_n:
    mov rsi, esc_n_buf
    jmp .esc_out
.esc_t:
    mov rsi, esc_t_buf
    jmp .esc_out
.esc_r:
    mov rsi, esc_r_buf
    jmp .esc_out
.esc_bs:
    mov rsi, esc_bs_buf
    jmp .esc_out
.esc_q:
    mov rsi, esc_q_buf
    jmp .esc_out
.esc_z:
    mov rsi, esc_z_buf
.esc_out:
    mov rdx, 2
    call write_buf
.close:
    mov rsi, quote_buf
    mov rdx, 1
    call write_buf
    pop rbx
    ret

section .bss
char_byte_buf: resb 1

section .rodata
; byte values, not embedded backslash escapes — NASM string literals
; don't interpret backslash escapes by default, and every other file
; in this project already spells a control byte numerically (10, 13,
; 9, ...) for exactly that reason.
; NASM treats a trailing backslash as a line-continuation marker even
; inside a ";" comment, so none of these comments end with one —
; "backslash-backslash" spelled out instead of "\\", which otherwise
; splices the next line into this one and corrupts the symbol table.
quote_buf:  db 39                  ; a single quote
esc_n_buf:  db 92, 'n'             ; backslash-n
esc_t_buf:  db 92, 't'             ; backslash-t
esc_r_buf:  db 92, 'r'             ; backslash-r
esc_bs_buf: db 92, 92              ; backslash-backslash
esc_q_buf:  db 92, 39              ; backslash-quote
esc_z_buf:  db 92, '0'             ; backslash-zero
make_char_range_msg: db "MAKE-CHAR: expected a fixnum in 0..255"
make_char_range_msg_len: equ $ - make_char_range_msg
