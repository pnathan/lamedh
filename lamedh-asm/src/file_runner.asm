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

%define SRC_BUF_BYTES (16 * 1024 * 1024)

section .text

global lamedh_main
lamedh_main:
    push rbx
    push r12
    push r13

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
    call reader_init

    ; Read, compile, and run every top-level form in turn — a program
    ; is a sequence of independent thunks, each run for its side
    ; effects (PRINT, etc.) the moment it's compiled, same as every
    ; existing test's own lamedh_main already does by hand.
.form_loop:
    call read_form
    cmp rax, IMM_EOF
    je .done
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    jmp .form_loop

.done:
    xor rax, rax
    jmp .out

.open_failed:
    mov rax, 1
.out:
    pop r13
    pop r12
    pop rbx
    ret
