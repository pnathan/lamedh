; strings.asm — the string value type: a length-prefixed byte buffer
; heapobj (HDR_STRING: [0]=header [8]=len [16..]=raw bytes). Immutable
; and self-evaluating exactly like a fixnum literal — the data heap
; never relocates, so a string literal's absolute address bakes as a
; code immediate the same way any other QUOTE'd/literal datum does (see
; compiler.asm's compile_form ".literal" path; no compiler change was
; needed to make string literals self-evaluating).
;
; No mutation, no unicode, no interning — a string is just bytes.

%include "src/tags.inc"

extern data_alloc
extern write_buf

section .text

; make_string(rdi=byte buf, rsi=len) -> rax = tagged HDR_STRING heapobj,
; copying (rsi) bytes out of (rdi) into the new object. Always writes
; one extra NUL byte right after the string's own data (not counted in
; its length) so a string's byte pointer is also safe to hand directly
; to a raw syscall expecting a C string (see fileio.asm's file_open) —
; without this, an "open"-style path string would need a copy into a
; scratch NUL-terminated buffer at every call site.
global make_string
make_string:
    push rbx
    push r12
    push r13
    mov rbx, rdi                  ; src buf
    mov r12, rsi                    ; len
    lea rdi, [r12+17]                 ; header + len fields + bytes + NUL
    call data_alloc
    mov r13, rax
    mov qword [r13], HDR_STRING
    mov [r13+8], r12
    xor rcx, rcx
.copy:
    cmp rcx, r12
    jae .done
    mov dl, [rbx+rcx]
    mov [r13+16+rcx], dl
    inc rcx
    jmp .copy
.done:
    mov byte [r13+16+r12], 0
    mov rax, r13
    or rax, TAG_HEAPOBJ
    pop r13
    pop r12
    pop rbx
    ret

; is_string(rdi=tagged value) -> rax=1/0
global is_string
is_string:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .no
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_STRING
    jne .no
    mov rax, 1
    ret
.no:
    xor rax, rax
    ret

; string_len(rdi=tagged string) -> rax = raw (untagged) byte length
global string_len
string_len:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+8]
    ret

; string_length_tagged(rdi=tagged string) -> rax = tagged fixnum length.
; The STRING-LENGTH builtin's host half (see compiler.asm).
global string_length_tagged
string_length_tagged:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+8]
    TO_FIXNUM rax
    ret

; string_bytes(rdi=tagged string) -> rax = address of the first byte
global string_bytes
string_bytes:
    mov rax, rdi
    UNTAG_PTR rax
    add rax, 16
    ret

; print_string(rdi=tagged string) -> writes its raw bytes to stdout, no
; trailing newline (matches print_fixnum's own convention).
global print_string
print_string:
    push rbx
    mov rbx, rdi
    call string_len
    mov rdx, rax
    mov rdi, rbx
    call string_bytes
    mov rsi, rax
    call write_buf
    pop rbx
    ret

; print_value(rdi=tagged value) -> writes a string's raw bytes, or a
; fixnum's decimal value, dispatching on the tag at runtime (the
; argument's type isn't known until then). This is what PRINT actually
; calls; compile_print itself is unchanged — only the host address it
; bakes moved from print_fixnum to this dispatcher.
extern print_fixnum
global print_value
print_value:
    push rbx
    mov rbx, rdi
    call is_string
    test rax, rax
    jz .fixnum
    mov rdi, rbx
    call print_string
    jmp .out
.fixnum:
    mov rdi, rbx
    call print_fixnum
.out:
    pop rbx
    ret
