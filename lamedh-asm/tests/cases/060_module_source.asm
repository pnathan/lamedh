; 060_module_source — $MODULE-SOURCE-LOOKUP and $EVAL-MODULE-SOURCE:
; the two Rust-level builtins (environment.rs) behind REQUIRE's own
; module resolution (lib/06-require.lisp's $require-resolve/
; $require-load). $MODULE-SOURCE-LOOKUP("NAME") returns
; (source-string . origin-string) for a known embedded module
; (modules.asm's own module_table — the reference's own, unmodified
; ../lib/*.lisp source, pulled in via incbin exactly the way
; file_runner.asm already does for lib/prelude.lisp) or NIL otherwise.
; $EVAL-MODULE-SOURCE(name, source) parses and evaluates every
; top-level form in source, the same read/compile/run loop
; file_runner.asm's own run_buffer uses for a real file, saving and
; restoring the reader's own global position around it so a nested
; call (this is exactly how REQUIRE reaches it, mid-file) doesn't
; corrupt whatever buffer the caller was itself still reading.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
e1: db '(PRINT (IF ($MODULE-SOURCE-LOOKUP "MODULES") T (QUOTE ())))'
e1_len: equ $ - e1                                                ; T

e2: db '(PRINT ($MODULE-SOURCE-LOOKUP "NOT-A-REAL-MODULE"))'
e2_len: equ $ - e2                                                  ; ()

e2b: db '(PRINT (IF ($MODULE-SOURCE-LOOKUP "TEXT") T (QUOTE ())))'
e2b_len: equ $ - e2b                              ; T — 30-text.lisp is
                                                   ; pure Lisp (its own
                                                   ; header: "100%
                                                   ; Lisp"), no OS/
                                                   ; capability
                                                   ; dependency, unlike
                                                   ; every module past
                                                   ; it (PORTS onward).

e2c: db '(PRINT (IF ($MODULE-SOURCE-LOOKUP "HELP-DATA") T (QUOTE ())))'
e2c_len: equ $ - e2c                              ; T — 99-help-data.lisp
                                                   ; (and 97-doc-renderer/
                                                   ; 98-help-system) come
                                                   ; after the OS-
                                                   ; dependent tier in
                                                   ; file-number order
                                                   ; but have no such
                                                   ; dependency themselves.

e3: db '($EVAL-MODULE-SOURCE "test" "(DEFINE FROM-MODULE 42)")'
e3_len: equ $ - e3

e4: db "(PRINT FROM-MODULE)"
e4_len: equ $ - e4                                                    ; 42

section .text

run_thunk_discard:
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    ret

global lamedh_main
lamedh_main:
    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; ()

    mov rdi, e2b
    mov rsi, e2b_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e2c
    mov rsi, e2c_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; 42

    xor rax, rax
    ret
