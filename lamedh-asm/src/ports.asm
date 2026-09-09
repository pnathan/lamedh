; ports.asm — the reference's own PortObj (lib/31-ports.lisp): binary
; ports over real files, in-memory byte buffers, and stdin/stdout/
; stderr. Every PORT-OPEN-INPUT-FILE*/PORT-READ-BYTE*/... primitive
; below is a genuine Rust-level builtin in the reference (evaluator/
; builtins_ports.rs) — representation-access work (real file
; descriptors, a port's own mutable cursor/buffer state) the Lisp
; layer cannot do on its own, the same category as arrays.asm/
; symtab.asm's own primitives.
;
; HDR_PORT's layout (tags.inc) is fixed-size and, unusually for this
; kernel, mutated in place after creation (flags/mem_buf/mem_pos) —
; the same allowed category of mutation STORE already performs on an
; ordinary Array, just performed here from host code instead of a
; Lisp-visible primitive, since nothing in the reference's own PORTS
; surface needs an arbitrary Lisp-level "mutate this port" operation.
;
; ERROR HANDLING, v0 scope, narrower than the reference on purpose: a
; failed OPEN returns a port that is simply never OPEN (PORT-OPEN-P*
; false, every read/write on it a silent no-op/EOF) rather than
; signaling — the same "no error signaling yet" simplification
; file_read (fileio.asm) already documents for a negative syscall
; result. PORT-SEEK*/PORT-POSITION* on a non-seekable port, and
; PORT-OUTPUT-CONTENTS* on a non-memory-output port, DO signal (via
; fail_wrong_type/native_throw, the same real CATCH/HANDLER-CASE-
; catchable condition CAR/CDR's own wrong-type check already uses) —
; lib/31-ports.lisp's own docstrings promise exactly that ("Signals an
; error on a non-seekable port"), and unlike a failed OPEN there is no
; sensible non-signaling fallback value to return instead. Neither
; in-memory port kind is seekable in this v0 (POSITION/SEEK! always
; signal on one) — the reference only documents OPEN-OUTPUT-BYTES as
; explicitly non-seekable; OPEN-INPUT-BYTES isn't documented either
; way, so this host picks the simpler, honestly-narrower answer for
; both rather than guessing at unstated behavior.
%include "src/tags.inc"
%include "src/syscalls.inc"

extern data_alloc
extern make_array
extern array_length_tagged
extern array_ref
extern array_set
extern intern_symbol
extern string_bytes
extern is_char_tagged
extern char_code_tagged
extern code_char_string
extern is_string
extern require_capability
extern fail_wrong_type
extern make_string

section .rodata
port_kind_file_name:   db "FILE"
port_kind_memory_name: db "MEMORY"
port_kind_stdin_name:  db "STDIN"
port_kind_stdout_name: db "STDOUT"
port_kind_stderr_name: db "STDERR"
port_name_stdin:  db "<stdin>"
port_name_stdin_len: equ $ - port_name_stdin
port_name_stdout: db "<stdout>"
port_name_stdout_len: equ $ - port_name_stdout
port_name_stderr: db "<stderr>"
port_name_stderr_len: equ $ - port_name_stderr
port_name_memory: db "<memory>"
port_name_memory_len: equ $ - port_name_memory
not_seekable_msg: db "PORT: not seekable"
not_seekable_msg_len: equ $ - not_seekable_msg
not_mem_output_msg: db "PORT-OUTPUT-CONTENTS: not a memory output port"
not_mem_output_msg_len: equ $ - not_mem_output_msg
not_a_port_msg: db "PORT: not a port"
not_a_port_msg_len: equ $ - not_a_port_msg

section .text

; --- allocation / type check -----------------------------------------

; port_alloc() -> rax = raw (untagged) address of a fresh HDR_PORT
; record with every slot zeroed/NIL. Every caller below fills in all
; six slots itself before ever returning it to Lisp code.
port_alloc:
    push rdi
    mov rdi, 56
    call data_alloc
    mov qword [rax], HDR_PORT
    mov qword [rax+8], 0
    mov qword [rax+16], IMM_NIL
    mov qword [rax+24], IMM_NIL
    mov qword [rax+32], 0
    mov qword [rax+40], IMM_NIL
    mov qword [rax+48], 0
    pop rdi
    ret

is_port_tagged:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .no
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_PORT
    jne .no
    mov rax, 1
    ret
.no:
    xor rax, rax
    ret

; port_p_tagged(rdi=tagged value) -> rax = IMM_TRUE/IMM_NIL. PORT-P*.
global port_p_tagged
port_p_tagged:
    call is_port_tagged
    test rax, rax
    jz .no
    mov rax, IMM_TRUE
    ret
.no:
    mov rax, IMM_NIL
    ret

; require_port(rdi=tagged value) -> rax = raw port address; never
; returns otherwise (fail_wrong_type). Every primitive below that
; takes a port operand calls this first.
require_port:
    push rdi
    call is_port_tagged
    pop rdi
    test rax, rax
    jnz .ok
    mov rsi, not_a_port_msg
    mov rdx, not_a_port_msg_len
    call fail_wrong_type
.ok:
    mov rax, rdi
    UNTAG_PTR rax
    ret

; byte_value_of(rdi=tagged Char or fixnum) -> rax = raw byte 0-255.
; byte_value_of(rdi=tagged Char, one-character String, or fixnum) ->
; rax = raw byte 0-255. Three shapes, not two, because this kernel's
; own CODE-CHAR (compiler.asm) returns a one-character STRING, not a
; genuine Char immediate (chars.asm's code_char_string — matching
; lib/14-strings.lisp's own documented "accepts a char, a one-
; character string, or an integer" convention) — found by testing:
; STRING-APPEND/CONCAT themselves don't accept a genuine Char
; immediate either (confirmed by direct testing), so any byte a real
; Lisp program builds via CODE-CHAR and expects to pass around
; normally is actually a string, and PORT-WRITE-BYTE!/an Array<Char>
; element built by ordinary Lisp code must be read the same way.
byte_value_of:
    push rdi
    call is_char_tagged
    pop rdi
    test rax, rax
    jz .not_char
    call char_code_tagged
    UNTAG_FIXNUM rax
    ret
.not_char:
    push rdi
    call is_string
    pop rdi
    test rax, rax
    jz .fixnum
    call string_bytes
    movzx eax, byte [rax]
    ret
.fixnum:
    mov rax, rdi
    UNTAG_FIXNUM rax
    ret

; --- construction: real files ----------------------------------------

; finish_file_port(rdi=raw fd, rsi=tagged name, rdx=io flags to OR in
; when fd>=0 — PORT_INPUT for an input open, PORT_OUTPUT for an
; output/append open) -> rax = tagged port. A real file fd is always
; flagged PORT_SEEKABLE; fd<0 (the open(2) failed) leaves flags at 0
; (closed) rather than signaling — see file header.
finish_file_port:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi                    ; raw fd
    mov r12, rsi                      ; tagged name
    mov r13, rdx                        ; io flags to OR in on success
    call port_alloc
    mov r14, rax                          ; raw port addr
    mov [r14+8], rbx
    lea rdi, [rel port_kind_file_name]
    mov rsi, 4
    call intern_symbol
    mov [r14+16], rax
    mov [r14+24], r12
    mov rax, rbx
    test rax, rax
    js .closed
    mov rax, r13
    or rax, PORT_OPEN | PORT_SEEKABLE
    mov [r14+32], rax
    jmp .tag
.closed:
    mov qword [r14+32], 0
.tag:
    mov rax, r14
    or rax, TAG_HEAPOBJ
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; port_open_input_file_tagged(rdi=tagged path string) -> rax = tagged
; port. READ-FS-gated (KERNEL.md Part IX), same rule as FD-OPEN mode 0
; (fileio.asm).
global port_open_input_file_tagged
port_open_input_file_tagged:
    push rbx
    mov rbx, rdi                   ; tagged path (doubles as the port's name)
    mov rdi, 0                       ; READ-FS bit
    call require_capability
    mov rdi, rbx
    call string_bytes
    mov rdi, rax
    mov rsi, O_RDONLY
    xor rdx, rdx
    mov eax, SYS_open
    syscall
    mov rdi, rax
    mov rsi, rbx
    mov rdx, PORT_INPUT
    call finish_file_port
    pop rbx
    ret

; port_open_output_file_tagged(rdi=tagged path string) -> rax = tagged
; port. CREATE-FS-gated. Truncates/creates, matching FD-OPEN mode 1.
global port_open_output_file_tagged
port_open_output_file_tagged:
    push rbx
    mov rbx, rdi
    mov rdi, 1                        ; CREATE-FS bit
    call require_capability
    mov rdi, rbx
    call string_bytes
    mov rdi, rax
    mov rsi, O_WRONLY | O_CREAT | O_TRUNC
    mov rdx, 420
    mov eax, SYS_open
    syscall
    mov rdi, rax
    mov rsi, rbx
    mov rdx, PORT_OUTPUT
    call finish_file_port
    pop rbx
    ret

; port_open_append_file_tagged(rdi=tagged path string) -> rax = tagged
; port. CREATE-FS-gated, matching FD-OPEN mode 2.
global port_open_append_file_tagged
port_open_append_file_tagged:
    push rbx
    mov rbx, rdi
    mov rdi, 1                        ; CREATE-FS bit
    call require_capability
    mov rdi, rbx
    call string_bytes
    mov rdi, rax
    mov rsi, O_WRONLY | O_CREAT | O_APPEND
    mov rdx, 420
    mov eax, SYS_open
    syscall
    mov rdi, rax
    mov rsi, rbx
    mov rdx, PORT_OUTPUT
    call finish_file_port
    pop rbx
    ret

; --- construction: stdin/stdout/stderr --------------------------------

; std_port(rdi=raw fd, rsi=name ptr, rdx=name len, rcx=tagged kind
; symbol, r8=io flags) -> rax = tagged port. No SEEKABLE flag (a
; pipe/tty fd is not generally seekable); no capability gating except
; where the caller has already done it (stdin needs IO, stdout/stderr
; need none, matching PRINC/PRIN1 already writing to stdout
; unconditionally).
std_port:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi                    ; raw fd
    mov r12, rcx                      ; tagged kind
    mov r13, r8                         ; io flags
    mov r14, rsi                          ; name ptr (rsi/rdx must move
                                           ; into make_string's own
                                           ; rdi/rsi slots — std_port's
                                           ; own incoming rdi/rsi are fd/
                                           ; ptr, not ptr/len)
    mov rdi, r14
    mov rsi, rdx
    call make_string                        ; rax = tagged name string
    mov r14, rax
    call port_alloc
    mov [rax+8], rbx
    mov [rax+16], r12
    mov [rax+24], r14
    mov rcx, r13
    or rcx, PORT_OPEN
    mov [rax+32], rcx
    or rax, TAG_HEAPOBJ
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

global port_stdin_tagged
port_stdin_tagged:
    push rbx
    mov rdi, 4                        ; IO bit
    call require_capability
    lea rdi, [rel port_kind_stdin_name]
    mov rsi, 5
    call intern_symbol
    mov rbx, rax
    mov rdi, 0
    lea rsi, [rel port_name_stdin]
    mov rdx, port_name_stdin_len
    mov rcx, rbx
    mov r8, PORT_INPUT
    call std_port
    pop rbx
    ret

global port_stdout_tagged
port_stdout_tagged:
    lea rdi, [rel port_kind_stdout_name]
    mov rsi, 6
    call intern_symbol
    push rax
    mov rdi, 1
    lea rsi, [rel port_name_stdout]
    mov rdx, port_name_stdout_len
    pop rcx
    mov r8, PORT_OUTPUT
    call std_port
    ret

global port_stderr_tagged
port_stderr_tagged:
    lea rdi, [rel port_kind_stderr_name]
    mov rsi, 6
    call intern_symbol
    push rax
    mov rdi, 2
    lea rsi, [rel port_name_stderr]
    mov rdx, port_name_stderr_len
    pop rcx
    mov r8, PORT_OUTPUT
    call std_port
    ret

; --- construction: in-memory byte ports -------------------------------

; port_open_input_bytes_tagged(rdi=tagged Array<Char>) -> rax = tagged
; port over a private copy of the given array (per the reference's own
; documented contract). No capability required: touches no host
; resource.
global port_open_input_bytes_tagged
port_open_input_bytes_tagged:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi                     ; tagged source array
    call array_length_tagged
    mov r12, rax                       ; tagged length
    mov rdi, r12
    call make_array                      ; rax = tagged fresh array, NIL-filled
    mov r13, rax                           ; tagged new array
    mov rax, r12
    UNTAG_FIXNUM rax
    mov r12, rax                             ; raw length
    xor r14, r14                               ; raw i
.copy_loop:
    cmp r14, r12
    jae .copy_done
    mov rdi, rbx
    mov rsi, r14
    TO_FIXNUM rsi
    call array_ref
    mov rdx, rax
    mov rdi, r13
    mov rsi, r14
    TO_FIXNUM rsi
    call array_set
    inc r14
    jmp .copy_loop
.copy_done:
    call port_alloc
    mov rbx, rax
    mov qword [rbx+8], -1
    lea rdi, [rel port_kind_memory_name]
    mov rsi, 6
    call intern_symbol
    mov [rbx+16], rax
    lea rdi, [rel port_name_memory]
    mov rsi, port_name_memory_len
    call make_string
    mov [rbx+24], rax
    mov qword [rbx+32], (PORT_OPEN | PORT_INPUT)
    mov [rbx+40], r13
    mov qword [rbx+48], 0
    mov rax, rbx
    or rax, TAG_HEAPOBJ
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; port_open_output_bytes_tagged() -> rax = tagged port accumulating
; written bytes in a growable private array (mem_grow, below). No
; capability required. Not seekable.
global port_open_output_bytes_tagged
port_open_output_bytes_tagged:
    push rbx
    push r12
    mov rdi, 64
    TO_FIXNUM rdi
    call make_array
    mov rbx, rax                     ; tagged initial buffer
    call port_alloc
    mov r12, rax                       ; raw port addr
    mov qword [r12+8], -1
    lea rdi, [rel port_kind_memory_name]
    mov rsi, 6
    call intern_symbol
    mov [r12+16], rax
    lea rdi, [rel port_name_memory]
    mov rsi, port_name_memory_len
    call make_string
    mov [r12+24], rax
    mov qword [r12+32], (PORT_OPEN | PORT_OUTPUT)
    mov [r12+40], rbx
    mov qword [r12+48], 0
    mov rax, r12
    or rax, TAG_HEAPOBJ
    pop r12
    pop rbx
    ret

; --- output_contents ---------------------------------------------------

; port_output_contents_tagged(rdi=tagged port) -> rax = tagged fresh
; Array<Char> of the bytes written so far. Signals (fail_wrong_type) if
; PORT isn't a memory output port (mem_buf slot is NIL for every other
; port kind).
global port_output_contents_tagged
port_output_contents_tagged:
    push rbx
    push r12
    push r13
    push r14
    mov r12, rdi                  ; tagged port (culprit if we error)
    call require_port
    mov rbx, rax                    ; raw port addr
    mov rax, [rbx+40]
    cmp rax, IMM_NIL
    jne .have_buf
    mov rdi, r12
    lea rsi, [rel not_mem_output_msg]
    mov rdx, not_mem_output_msg_len
    call fail_wrong_type
.have_buf:
    mov r13, rax                    ; tagged source buffer
    mov r14, [rbx+48]                 ; raw used length
    mov rax, r14
    TO_FIXNUM rax
    mov rdi, rax
    call make_array                     ; rax = tagged new array
    mov r12, rax                          ; tagged destination (r12 free again)
    xor rbx, rbx                            ; raw loop counter (port addr no longer needed)
.loop:
    cmp rbx, r14
    jae .done
    mov rdi, r13
    mov rsi, rbx
    TO_FIXNUM rsi
    call array_ref
    mov rdx, rax
    mov rdi, r12
    mov rsi, rbx
    TO_FIXNUM rsi
    call array_set
    inc rbx
    jmp .loop
.done:
    mov rax, r12
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; --- growable in-memory output buffer -----------------------------------

; mem_grow(rdi=raw port addr, rsi=raw needed total length) — grows
; [rdi+40]'s array to at least RSI elements (capacity = max(needed,
; old*2, 64)), copying every existing element across, if it isn't
; already that big. No-op if it already is.
mem_grow:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi                    ; raw port addr
    mov r12, rsi                      ; raw needed length
    mov rdi, [rbx+40]
    call array_length_tagged
    UNTAG_FIXNUM rax
    mov r13, rax                        ; raw old capacity
    cmp r13, r12
    jae .done
    mov rcx, r13
    shl rcx, 1                            ; old*2
    cmp rcx, r12
    jae .use_double
    mov rcx, r12
.use_double:
    cmp rcx, 64
    jae .have_cap
    mov rcx, 64
.have_cap:
    mov rax, rcx
    TO_FIXNUM rax
    mov rdi, rax
    call make_array                         ; rax = tagged new array
    mov r12, rax                              ; tagged new array (needed value no longer used)
    xor r14, r14                                ; raw loop counter
.copy_loop:
    cmp r14, r13
    jae .copy_done
    mov rdi, [rbx+40]
    mov rsi, r14
    TO_FIXNUM rsi
    call array_ref
    mov rdx, rax
    mov rdi, r12
    mov rsi, r14
    TO_FIXNUM rsi
    call array_set
    inc r14
    jmp .copy_loop
.copy_done:
    mov [rbx+40], r12
.done:
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; --- byte-at-a-time I/O --------------------------------------------------

; port_read_byte_tagged(rdi=tagged port) -> rax = tagged fixnum 0-255,
; or NIL (not open, not an input port, or EOF).
global port_read_byte_tagged
port_read_byte_tagged:
    push rbx
    push r12
    call require_port
    mov rbx, rax
    mov rax, [rbx+32]
    test rax, PORT_OPEN
    jz .nil
    test rax, PORT_INPUT
    jz .nil
    mov rax, [rbx+40]
    cmp rax, IMM_NIL
    jne .mem
    ; file port: one-byte scratch via data_alloc (matches file_read's
    ; own style, fileio.asm — avoids any stack-alignment question).
    mov rdi, 1
    call data_alloc
    mov r12, rax                      ; raw scratch ptr
    mov rdi, [rbx+8]
    mov rsi, r12
    mov rdx, 1
    mov eax, SYS_read
    syscall
    cmp rax, 1
    jne .nil
    movzx eax, byte [r12]
    TO_FIXNUM rax
    jmp .out
.mem:
    mov r12, rax                        ; tagged mem_buf
    mov rdi, r12
    call array_length_tagged
    UNTAG_FIXNUM rax
    mov rcx, rax                          ; raw length
    mov rax, [rbx+48]                       ; raw mem_pos
    cmp rax, rcx
    jae .nil
    mov rdi, r12
    mov rsi, rax
    TO_FIXNUM rsi
    call array_ref                            ; rax = tagged Char
    push rax
    mov rax, [rbx+48]
    inc rax
    mov [rbx+48], rax
    pop rdi
    call byte_value_of                          ; rax = raw byte
    TO_FIXNUM rax
    jmp .out
.nil:
    mov rax, IMM_NIL
.out:
    pop r12
    pop rbx
    ret


; port_read_bytes_tagged(rdi=tagged port, rsi=tagged fixnum n) -> rax =
; tagged fresh Array<Char>, up to N bytes, shorter (possibly empty) at
; EOF/on a partial read — never NIL. Register plan throughout: rbx =
; raw port addr, r12 = source (raw scratch ptr / tagged mem buffer),
; r13 = raw take/read count, r14 = raw loop index, r15 = tagged
; destination array — every one of the five dedicated scratch
; registers this project's own convention treats as callee-saved
; across a nested call.
global port_read_bytes_tagged
port_read_bytes_tagged:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov r14, rsi                      ; tagged n (temp, until require_port)
    call require_port
    mov rbx, rax
    mov rax, r14
    UNTAG_FIXNUM rax
    mov r14, rax                        ; raw n
    mov rax, [rbx+32]
    test rax, PORT_OPEN
    jz .empty
    test rax, PORT_INPUT
    jz .empty
    mov rax, [rbx+40]
    cmp rax, IMM_NIL
    jne .mem
    ; ---- file port ----
    mov rdi, r14
    call data_alloc
    mov r12, rax                        ; raw scratch buffer
    mov rdi, [rbx+8]
    mov rsi, r12
    mov rdx, r14
    mov eax, SYS_read
    syscall
    cmp rax, 0
    jns .file_got
    xor rax, rax
.file_got:
    mov r13, rax                          ; raw actual count
    mov rax, r13
    TO_FIXNUM rax
    mov rdi, rax
    call make_array                         ; rax = tagged dest array
    mov r15, rax
    xor r14, r14                              ; raw loop index
.file_loop:
    cmp r14, r13
    jae .file_done
    movzx edi, byte [r12+r14]
    TO_FIXNUM rdi
    call code_char_string                       ; rax = tagged one-char string
    mov rdx, rax
    mov rdi, r15
    mov rsi, r14
    TO_FIXNUM rsi
    call array_set
    inc r14
    jmp .file_loop
.file_done:
    mov rax, r15
    jmp .out
    ; ---- memory port ----
.mem:
    mov r12, rax                          ; tagged source buffer
    mov rdi, r12
    call array_length_tagged
    UNTAG_FIXNUM rax
    mov rcx, rax                            ; raw source length
    mov rax, [rbx+48]                         ; raw mem_pos
    mov r13, rcx
    sub r13, rax                                ; raw bytes available
    jns .avail_ok
    xor r13, r13
.avail_ok:
    cmp r13, r14
    jbe .take_ok
    mov r13, r14
.take_ok:
    mov rax, r13
    TO_FIXNUM rax
    mov rdi, rax
    call make_array                             ; rax = tagged dest array
    mov r15, rax
    xor r14, r14                                  ; raw loop index
.mem_loop:
    cmp r14, r13
    jae .mem_done
    mov rdi, r12
    mov rsi, [rbx+48]
    add rsi, r14
    TO_FIXNUM rsi
    call array_ref                                    ; rax = tagged Char
    mov rdx, rax
    mov rdi, r15
    mov rsi, r14
    TO_FIXNUM rsi
    call array_set
    inc r14
    jmp .mem_loop
.mem_done:
    mov rax, [rbx+48]
    add rax, r13
    mov [rbx+48], rax
    mov rax, r15
    jmp .out
.empty:
    xor rdi, rdi
    TO_FIXNUM rdi
    call make_array
.out:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; --- writing --------------------------------------------------------

; port_write_byte_tagged(rdi=tagged port, rsi=tagged Char or fixnum
; byte) -> rax = tagged fixnum byte value written (0 if the port isn't
; open for output — a silent no-op, see file header).
global port_write_byte_tagged
port_write_byte_tagged:
    push rbx
    push r12
    mov r12, rsi                      ; tagged byte value (temp)
    call require_port
    mov rbx, rax
    mov rdi, r12
    call byte_value_of
    mov r12, rax                        ; raw byte value
    mov rax, [rbx+32]
    test rax, PORT_OPEN
    jz .out
    test rax, PORT_OUTPUT
    jz .out
    mov rax, [rbx+40]
    cmp rax, IMM_NIL
    jne .mem
    ; file port: one-byte scratch via data_alloc, matching
    ; port_read_byte_tagged's own style.
    push r12
    mov rdi, 1
    call data_alloc
    pop r12
    mov byte [rax], r12b
    mov rsi, rax
    mov rdi, [rbx+8]
    mov rdx, 1
    mov eax, SYS_write
    syscall
    jmp .out
.mem:
    mov rdi, rbx
    mov rsi, [rbx+48]
    inc rsi
    call mem_grow
    mov rdi, [rbx+40]
    mov rsi, [rbx+48]
    TO_FIXNUM rsi
    mov rdx, r12
    call tag_char_wrap_and_set
    mov rax, [rbx+48]
    inc rax
    mov [rbx+48], rax
.out:
    mov rax, r12
    TO_FIXNUM rax
    pop r12
    pop rbx
    ret

; tag_char_wrap_and_set(rdi=tagged array, rsi=tagged index, rdx=raw
; byte) -> rax = tagged one-char string stored. Builds via
; code_char_string, not tag_char — a genuine Char immediate (tag_char)
; is not what ordinary Lisp code passing "a byte" around actually
; produces or expects in this kernel (see byte_value_of's own comment)
; — an Array<Char> element here must be interoperable with the
; CODE-CHAR/STRING-APPEND/CONCAT ecosystem lib/30-text.lisp already
; relies on, which only handles one-character strings, not genuine
; Char immediates (confirmed by direct testing: STRING-APPEND/CONCAT
; on a real Char segfault).
tag_char_wrap_and_set:
    push rdi
    push rsi
    mov rdi, rdx
    TO_FIXNUM rdi
    call code_char_string
    mov rdx, rax
    pop rsi
    pop rdi
    call array_set
    ret

; port_write_bytes_tagged(rdi=tagged port, rsi=tagged Array<Char>) ->
; rax = tagged fixnum count actually written (0 if the port isn't open
; for output). Register plan: rbx = raw port addr, r12 = tagged source
; array, r13 = raw length, r14 = raw scratch ptr (file path only),
; r15 = raw loop index.
global port_write_bytes_tagged
port_write_bytes_tagged:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov r12, rsi                      ; tagged bytes array (temp)
    call require_port
    mov rbx, rax
    mov rdi, r12
    call array_length_tagged
    UNTAG_FIXNUM rax
    mov r13, rax                        ; raw length
    mov rax, [rbx+32]
    test rax, PORT_OPEN
    jz .zero
    test rax, PORT_OUTPUT
    jz .zero
    mov rax, [rbx+40]
    cmp rax, IMM_NIL
    jne .mem
    ; ---- file port ----
    mov rdi, r13
    call data_alloc
    mov r14, rax                          ; raw scratch buf
    xor r15, r15                            ; raw loop index
.file_fill:
    cmp r15, r13
    jae .file_write
    mov rdi, r12
    mov rsi, r15
    TO_FIXNUM rsi
    call array_ref                              ; rax = tagged element
    mov rdi, rax
    call byte_value_of                            ; rax = raw byte
    mov [r14+r15], al
    inc r15
    jmp .file_fill
.file_write:
    mov rdi, [rbx+8]
    mov rsi, r14
    mov rdx, r13
    mov eax, SYS_write
    syscall
    cmp rax, 0
    jns .file_count_ok
    xor rax, rax
.file_count_ok:
    TO_FIXNUM rax
    jmp .out
    ; ---- memory port ----
.mem:
    mov rdi, rbx
    mov rsi, [rbx+48]
    add rsi, r13
    call mem_grow
    xor r15, r15                              ; raw loop index
.mem_fill:
    cmp r15, r13
    jae .mem_done
    mov rdi, r12
    mov rsi, r15
    TO_FIXNUM rsi
    call array_ref                                ; rax = tagged element
    mov rdx, rax
    mov rdi, [rbx+40]
    mov rsi, [rbx+48]
    add rsi, r15
    TO_FIXNUM rsi
    call array_set
    inc r15
    jmp .mem_fill
.mem_done:
    mov rax, [rbx+48]
    add rax, r13
    mov [rbx+48], rax
    mov rax, r13
    TO_FIXNUM rax
    jmp .out
.zero:
    xor rax, rax
    TO_FIXNUM rax
.out:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; --- flush / close ----------------------------------------------------

; port_flush_tagged(rdi=tagged port) -> rax = T. No buffering is ever
; done (every write is a direct syscall, or a direct in-memory array
; store), so this only validates PORT is actually a port.
global port_flush_tagged
port_flush_tagged:
    call require_port
    mov rax, IMM_TRUE
    ret

; port_close_tagged(rdi=tagged port) -> rax = T. Idempotent: closing an
; already-closed port only clears a flag bit that is already clear.
global port_close_tagged
port_close_tagged:
    push rbx
    call require_port
    mov rbx, rax
    mov rax, [rbx+32]
    test rax, PORT_OPEN
    jz .done
    mov rcx, [rbx+8]
    cmp rcx, 0
    js .clear_flag                    ; memory port (fd=-1): nothing to close
    mov rdi, rcx
    mov eax, SYS_close
    syscall
.clear_flag:
    mov rax, [rbx+32]
    and rax, ~PORT_OPEN
    mov [rbx+32], rax
.done:
    mov rax, IMM_TRUE
    pop rbx
    ret

; --- introspection ------------------------------------------------------

; port_flag_p_shared(rdi=tagged port, rsi=raw flag bit) -> rax =
; IMM_TRUE/IMM_NIL. Not itself exposed to the compiler dispatch (which
; only ever passes rdi) — PORT-OPEN-P*/PORT-INPUT-P*/PORT-OUTPUT-P*/
; PORT-SEEKABLE-P* below are each a two-instruction wrapper setting
; rsi to their own fixed bit and falling through via jmp.
port_flag_p_shared:
    push rbx
    push r12
    mov r12, rsi
    call require_port
    mov rbx, rax
    mov rax, [rbx+32]
    test rax, r12
    jz .no
    mov rax, IMM_TRUE
    jmp .done
.no:
    mov rax, IMM_NIL
.done:
    pop r12
    pop rbx
    ret

global port_open_p_tagged
port_open_p_tagged:
    mov rsi, PORT_OPEN
    jmp port_flag_p_shared

global port_input_p_tagged
port_input_p_tagged:
    mov rsi, PORT_INPUT
    jmp port_flag_p_shared

global port_output_p_tagged
port_output_p_tagged:
    mov rsi, PORT_OUTPUT
    jmp port_flag_p_shared

global port_seekable_p_tagged
port_seekable_p_tagged:
    mov rsi, PORT_SEEKABLE
    jmp port_flag_p_shared

; port_position_tagged(rdi=tagged port) -> rax = tagged fixnum current
; byte offset, or signals (fail_wrong_type) if PORT isn't seekable.
global port_position_tagged
port_position_tagged:
    push rbx
    push r12
    mov r12, rdi                    ; tagged port (error culprit)
    call require_port
    mov rbx, rax
    mov rax, [rbx+32]
    test rax, PORT_SEEKABLE
    jnz .ok
    mov rdi, r12
    lea rsi, [rel not_seekable_msg]
    mov rdx, not_seekable_msg_len
    call fail_wrong_type
.ok:
    mov rdi, [rbx+8]
    xor rsi, rsi
    mov rdx, SEEK_CUR
    mov eax, SYS_lseek
    syscall
    TO_FIXNUM rax
    pop r12
    pop rbx
    ret

; port_seek_tagged(rdi=tagged port, rsi=tagged fixnum offset) -> rax =
; tagged fixnum new absolute position, or signals if PORT isn't
; seekable.
global port_seek_tagged
port_seek_tagged:
    push rbx
    push r12
    push r13
    mov r12, rdi                    ; tagged port (error culprit)
    mov r13, rsi                      ; tagged offset (temp)
    call require_port
    mov rbx, rax
    mov rax, [rbx+32]
    test rax, PORT_SEEKABLE
    jnz .ok
    mov rdi, r12
    lea rsi, [rel not_seekable_msg]
    mov rdx, not_seekable_msg_len
    call fail_wrong_type
.ok:
    mov rax, r13
    UNTAG_FIXNUM rax
    mov rsi, rax
    mov rdi, [rbx+8]
    mov rdx, SEEK_SET
    mov eax, SYS_lseek
    syscall
    TO_FIXNUM rax
    pop r13
    pop r12
    pop rbx
    ret

; port_name_tagged(rdi=tagged port) -> rax = tagged name string.
global port_name_tagged
port_name_tagged:
    call require_port
    mov rax, [rax+24]
    ret

; port_kind_tagged(rdi=tagged port) -> rax = tagged kind symbol.
global port_kind_tagged
port_kind_tagged:
    call require_port
    mov rax, [rax+16]
    ret
