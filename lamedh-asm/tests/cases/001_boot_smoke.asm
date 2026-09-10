; 001_boot_smoke — proves _start, the two mmap'd heaps, data_alloc, and
; print_fixnum all work end to end before anything Lisp-shaped exists.

%include "src/tags.inc"

extern data_alloc
extern print_fixnum
extern print_newline

section .text
global lamedh_main
lamedh_main:
    ; allocate 16 bytes on the data heap, store a fixnum 42 in it, read it
    ; back out and print it — exercises the bump allocator, not just a
    ; register constant.
    mov rdi, 16
    call data_alloc
    mov rdi, 42
    TO_FIXNUM rdi
    mov [rax], rdi

    mov rdi, [rax]
    call print_fixnum
    call print_newline

    xor rax, rax                ; exit code 0
    ret
