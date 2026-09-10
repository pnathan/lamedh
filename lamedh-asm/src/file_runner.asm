; file_runner.asm — the real (non-test) entry point: read a whole
; source text (a file named on the command line, or stdin when none is
; given), then read+compile+run every top-level form in it in order,
; exactly the way each tests/cases/*.asm case already does by hand for
; one hardcoded literal at a time — this just does it in a runtime loop
; over a real, arbitrarily long buffer instead. This is the concrete
; first step toward running ../examples/*/main.lisp (README Roadmap):
; without this, there was no way to feed this compiler more than one
; form baked into an .asm test case at a time.
;
; Usage: `lamedhc path/to/program.lisp` or `some-producer | lamedhc`.
; There is no `-i`/`-s`/argument-binding surface yet (see README
; Roadmap) — just "read this whole text, run every form in it."

%include "src/tags.inc"
%include "src/syscalls.inc"

extern program_argc
extern program_argv
extern reader_init
extern read_form
extern compile_thunk
extern rc_pin_depth
extern compile_nesting_depth
extern current_lambda_depth

%define SRC_BUF_BYTES (16 * 1024 * 1024)

section .rodata
; prelude.lisp, pulled directly into the binary — no filesystem lookup,
; no install location, no argv-relative path resolution to invent for
; a freestanding/no-libc project (see README): the prelude is real
; Lisp source, run through the exact same reader_init/read_form/
; compile_thunk loop the user's own program is, just from an in-memory
; buffer instead of an mmap'd file. `lib/prelude.lisp` explains why
; each form in it needs no compiler change.
prelude_start:
    incbin "lib/prelude.lisp"
prelude_end:
prelude_len: equ prelude_end - prelude_start

section .rodata
open_failed_msg: db "lamedhc: cannot open input file", 10
open_failed_len: equ $ - open_failed_msg

section .text

; run_buffer(rdi=buf, rsi=len) — the read+compile+run loop every
; tests/cases/*.asm case already does by hand for one hardcoded literal
; at a time, generalized to a runtime loop over an arbitrary buffer:
; read a top-level form, compile it to a fresh native thunk, call it
; for its side effects, repeat until EOF. Used for both the prelude
; and the user's own source text below.
run_buffer:
    call reader_init
.loop:
    call read_form
    cmp rax, IMM_EOF
    je .done
    mov rdi, rax
    ; A macro transformer that signals an error mid-expansion longjmps
    ; (native_throw) past compile_thunk's own epilogue, skipping its
    ; `dec qword [compile_nesting_depth]` — resetting to 0 here, before
    ; every top-level compile, is what makes each iteration of this
    ; loop a genuinely fresh top-level compile regardless of how the
    ; previous one ended, rather than trusting balanced inc/dec pairs
    ; across an unwind path that cannot run them (macroexpand_once's
    ; own comment, compiler.asm).
    mov qword [compile_nesting_depth], 0
    ; Same reasoning again for rc_pin_depth (gc.asm): a macro
    ; transformer that errors mid-expansion longjmps past
    ; compile_thunk's own rc_pin_leave, and a pin depth that never
    ; came back down would make every subsequent runtime allocation
    ; immortal — a silent, total defeat of the collector rather than a
    ; crash.
    mov qword [rc_pin_depth], 0
    ; Same reasoning, same fix, for current_lambda_depth
    ; (compiler.asm, docs/spec-tco-capture-gc.md section 2): a macro
    ; transformer error caught mid-LAMBDA-body-compile longjmps past
    ; compile_lambda's own `dec qword [current_lambda_depth]`, so a
    ; fresh top-level compile must not trust the previous one's
    ; inc/dec pairs to have balanced.
    mov qword [current_lambda_depth], 0
    call compile_thunk
    ; Every compiled thunk is called this way, everywhere in this
    ; project (every tests/cases/*.asm lamedh_main does the same
    ; push+call-through-the-stack-slot+pop, not a bare `call rax`):
    ; the extra push keeps rsp 16-byte aligned at the callee's entry,
    ; which floats.asm's host routines rely on for aligned SSE moves —
    ; a bare `call rax` here shifts alignment by 8 and faults (SIGBUS)
    ; the moment compiled code calls into one of them.
    push rax
    call qword [rsp]
    add rsp, 8
    jmp .loop
.done:
    ret

global lamedh_main
lamedh_main:
    push rbx
    push r12
    push r13

    mov rdi, prelude_start
    mov rsi, prelude_len
    call run_buffer

    ; One anonymous mmap holds the entire source text; 16 MiB is the
    ; same generous v0 sizing the data/code heaps already use
    ; (heap.asm) — plenty for this project's own example-sized programs,
    ; not a general answer for an arbitrarily large input (see README
    ; Roadmap's own "v0, not silent" framing elsewhere in this project).
    xor edi, edi
    mov esi, SRC_BUF_BYTES
    mov edx, PROT_READ | PROT_WRITE
    mov r10d, MAP_PRIVATE | MAP_ANONYMOUS
    mov r8d, -1
    xor r9d, r9d
    mov eax, SYS_mmap
    syscall
    mov r12, rax                       ; source buffer base

    ; argv[1] (path) if given, else stdin — the same "file or stdin"
    ; choice a Unix filter makes, so `lamedhc prog.lisp` and
    ; `cat prog.lisp | lamedhc` behave identically.
    mov rax, [program_argc]
    cmp rax, 2
    jb .use_stdin
    mov rax, [program_argv]
    mov rdi, [rax+8]                     ; argv[1]
    mov esi, O_RDONLY
    xor edx, edx
    mov eax, SYS_open
    syscall
    mov rbx, rax                           ; fd, or a negative errno
    cmp rbx, 0
    jl .open_failed
    jmp .have_fd
.use_stdin:
    mov rbx, STDIN
.have_fd:

    ; Read to EOF (0) or a hard error (<0, folded to "stop here" rather
    ; than propagated — there being no conditions this host can raise
    ; yet, the same honest v0 scope fileio.asm's own FD-READ already
    ; documents). Loops on a short read, unlike FD-READ's own
    ; one-shot-syscall primitive: a real file or pipe commonly returns
    ; fewer bytes than requested well before EOF.
    xor r13, r13                             ; total bytes read so far
.read_loop:
    mov rdi, rbx
    lea rsi, [r12+r13]
    mov rdx, SRC_BUF_BYTES
    sub rdx, r13
    mov eax, SYS_read
    syscall
    cmp rax, 0
    jle .read_done
    add r13, rax
    jmp .read_loop
.read_done:

    cmp rbx, STDIN
    je .no_close
    mov rdi, rbx
    mov eax, SYS_close
    syscall
.no_close:

    mov rdi, r12
    mov rsi, r13
    call run_buffer

    xor rax, rax
    jmp .out

.open_failed:
    ; Say so. A silent exit(1) for a mistyped path is indistinguishable
    ; from a program that ran and failed — and the shell's own message
    ; never appears, since there is no shell redirect involved.
    mov edi, STDERR
    lea rsi, [rel open_failed_msg]
    mov edx, open_failed_len
    mov eax, SYS_write
    syscall
    mov rax, 1
.out:
    pop r13
    pop r12
    pop rbx
    ret
