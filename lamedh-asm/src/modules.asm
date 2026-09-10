; modules.asm — $MODULE-SOURCE-LOOKUP: the embedded half of REQUIRE's
; module resolution (KERNEL.md Part XI's module system, lib/06-require.
; lisp's own `$require-resolve`). A genuine Rust-level builtin in the
; reference (environment.rs's `module_source_lookup`, wired to
; BuiltinFunc::ModuleSourceLookup in evaluator/apply.rs): given a
; module name (a string — `$require-resolve` always calls this via
; `(princ-to-string name)`), returns `(source . origin)` if the name
; is a known optional library module, or NIL otherwise.
;
; This host has no Rust-side embedder API to register additional
; sources at runtime (the reference's own first resolution tier,
; `env.registered_module_source`), so only the second tier — sources
; embedded in the binary at build time, matching the reference's own
; `OPTIONAL_MODULES` table in src/lib.rs — is implemented. The third
; tier (disk, capability-gated) is already ordinary Lisp
; (`$require-resolve-disk`, lib/06-require.lisp) needing no kernel
; support at all.
;
; Every embedded file below is the *reference's own, unmodified*
; ../lib/*.lisp source, pulled in via `incbin` exactly the way
; file_runner.asm already does for lib/prelude.lisp — proof this is
; the real file, not a copy. v0 scope, narrower than the reference on
; purpose: only the modules with no networking/TLS/regex/OS dependency
; are embedded — SHELL through PROTOCOLS in the reference's own
; OPTIONAL_MODULES order, plus TEXT (30-text.lisp): despite an earlier
; version of this comment lumping it in with the networking tier,
; 30-text.lisp's own header is explicit that it is "100% Lisp" over
; three Rust primitives this kernel already had (STRING->UTF8*/
; UTF8->STRING*/UTF8->STRING-LOSSY*, lib/14-strings.lisp) — no
; capability, no I/O, no OS dependency at all. PORTS (31-ports.lisp) is
; embedded too now: it needs real file descriptors, but this host
; already has them (src/ports.asm — PORT-OPEN-INPUT-FILE*/PORT-READ-
; BYTE*/... over ordinary open(2)/read(2)/write(2)/close(2)/lseek(2)
; syscalls, plus in-memory ports over Arrays and stdin/stdout/stderr
; ports). NET (37-net.lisp) through REGEX (44-regex.lisp) — sockets,
; TLS, regex — are where this freestanding, no-libc host's real,
; deliberate boundary actually is (see README Roadmap): DOC-RENDERER
; (97-doc-renderer.lisp), HELP-SYSTEM (98-help-system.lisp), and
; HELP-DATA (99-help-data.lisp) come after the whole networking/OS
; block in file-number order but have no such dependency themselves —
; ordinary Lisp over HASH-TABLE/PRINC/CONS/COND, verified by loading
; each standalone — so they are embedded here too, closing the gap
; back up to the reference's own full OPTIONAL_MODULES table.

%include "src/tags.inc"

extern is_string
extern string_bytes
extern string_len
extern make_string
extern cons
extern bytes_equal
extern reader_init
extern read_form
extern compile_thunk
extern reader_buf
extern reader_pos
extern reader_end
extern intern_symbol

section .rodata

%macro MODULE_SRC 2
%1_src_start:
    incbin %2
%1_src_end:
%endmacro

MODULE_SRC shell, "../lib/07-shell.lisp"
MODULE_SRC lisp15, "../lib/09-lisp15.lisp"
MODULE_SRC testing, "../lib/10-testing.lisp"
MODULE_SRC optimizer_vau, "../lib/11-optimizer-vau.lisp"
MODULE_SRC call_graph, "../lib/19-call-graph.lisp"
MODULE_SRC condensation, "../lib/20-condensation.lisp"
MODULE_SRC guard, "../lib/22-guard.lisp"
MODULE_SRC match, "../lib/23-match.lisp"
MODULE_SRC rules, "../lib/24-rules.lisp"
MODULE_SRC variants, "../lib/25-variants.lisp"
MODULE_SRC instrument, "../lib/26-instrument.lisp"
MODULE_SRC modules_mod, "../lib/27-modules.lisp"
MODULE_SRC types, "../lib/28-types.lisp"
MODULE_SRC protocols, "../lib/29-protocols.lisp"
MODULE_SRC text, "../lib/30-text.lisp"
MODULE_SRC ports, "../lib/31-ports.lisp"
MODULE_SRC doc_renderer, "../lib/97-doc-renderer.lisp"
MODULE_SRC help_system, "../lib/98-help-system.lisp"
MODULE_SRC help_data, "../lib/99-help-data.lisp"

origin_embedded: db "embedded"
origin_embedded_len: equ $ - origin_embedded

; name(db), name_len, source_start, source_end — one row per embedded
; module, matching the reference's own OPTIONAL_MODULES name spelling
; exactly (src/lib.rs). $require-canonical-name already uppercases
; whatever REQUIRE was given before this is ever reached. Takes a
; separate label-safe suffix, since a module name itself (e.g.
; "OPTIMIZER-VAU") is not a valid NASM identifier fragment.
%macro MODULE_NAME 2
module_name_%1: db %2
module_name_%1_len: equ $ - module_name_%1
%endmacro

MODULE_NAME shell, "SHELL"
MODULE_NAME lisp15, "LISP15"
MODULE_NAME testing, "TESTING"
MODULE_NAME optimizer_vau, "OPTIMIZER-VAU"
MODULE_NAME call_graph, "CALL-GRAPH"
MODULE_NAME condensation, "CONDENSATION"
MODULE_NAME guard, "GUARD"
MODULE_NAME match, "MATCH"
MODULE_NAME rules, "RULES"
MODULE_NAME variants, "VARIANTS"
MODULE_NAME instrument, "INSTRUMENT"
MODULE_NAME modules_mod, "MODULES"
MODULE_NAME types, "TYPES"
MODULE_NAME protocols, "PROTOCOLS"
MODULE_NAME text, "TEXT"
MODULE_NAME ports, "PORTS"
MODULE_NAME doc_renderer, "DOC-RENDERER"
MODULE_NAME help_system, "HELP-SYSTEM"
MODULE_NAME help_data, "HELP-DATA"

align 8
module_table:
    dq module_name_shell, module_name_shell_len, shell_src_start, shell_src_end
    dq module_name_lisp15, module_name_lisp15_len, lisp15_src_start, lisp15_src_end
    dq module_name_testing, module_name_testing_len, testing_src_start, testing_src_end
    dq module_name_optimizer_vau, module_name_optimizer_vau_len, optimizer_vau_src_start, optimizer_vau_src_end
    dq module_name_call_graph, module_name_call_graph_len, call_graph_src_start, call_graph_src_end
    dq module_name_condensation, module_name_condensation_len, condensation_src_start, condensation_src_end
    dq module_name_guard, module_name_guard_len, guard_src_start, guard_src_end
    dq module_name_match, module_name_match_len, match_src_start, match_src_end
    dq module_name_rules, module_name_rules_len, rules_src_start, rules_src_end
    dq module_name_variants, module_name_variants_len, variants_src_start, variants_src_end
    dq module_name_instrument, module_name_instrument_len, instrument_src_start, instrument_src_end
    dq module_name_modules_mod, module_name_modules_mod_len, modules_mod_src_start, modules_mod_src_end
    dq module_name_types, module_name_types_len, types_src_start, types_src_end
    dq module_name_protocols, module_name_protocols_len, protocols_src_start, protocols_src_end
    dq module_name_text, module_name_text_len, text_src_start, text_src_end
    dq module_name_ports, module_name_ports_len, ports_src_start, ports_src_end
    dq module_name_doc_renderer, module_name_doc_renderer_len, doc_renderer_src_start, doc_renderer_src_end
    dq module_name_help_system, module_name_help_system_len, help_system_src_start, help_system_src_end
    dq module_name_help_data, module_name_help_data_len, help_data_src_start, help_data_src_end
module_table_end:
%define MODULE_ROW_BYTES 32
%define MODULE_TABLE_COUNT ((module_table_end - module_table) / MODULE_ROW_BYTES)

section .text

; eval_module_source_tagged(rdi=tagged display-name string [unused —
; this v0 doesn't yet prefix errors with it, unlike the reference],
; rsi=tagged source-text string) -> rax = the symbol T. The other half
; of REQUIRE's module loading ($EVAL-MODULE-SOURCE, a genuine Rust-
; level builtin in the reference, environment.rs/evaluator/apply.rs):
; parses and evaluates every top-level form in SOURCE, exactly the way
; file_runner.asm's own run_buffer does for a real file — reader_init/
; read_form/compile_thunk in a loop until IMM_EOF, invoking each
; compiled thunk via the same safe push/call/add idiom every other
; thunk invocation in this project uses.
;
; reader_buf/reader_pos/reader_end are single global cells, not a
; stack (see read_from_string_tagged's own comment, reader.asm) — this
; can itself be called from currently running compiled code (REQUIRE
; is Lisp-level library code, itself typically invoked mid-file, e.g.
; from another file_runner.asm-driven top-level form), so the caller's
; own reader position is saved before and restored after, the same way
; read_from_string_tagged already does for its own single-form read.
global eval_module_source_tagged
eval_module_source_tagged:
    push rbx
    push r12
    push r13
    mov rbx, rsi                      ; source string

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
.loop:
    call read_form
    cmp rax, IMM_EOF
    je .done
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    jmp .loop
.done:

    pop r12
    mov [reader_end], r12
    pop r12
    mov [reader_pos], r12
    pop r12
    mov [reader_buf], r12

    mov rdi, t_module_src_name
    mov rsi, 1
    call intern_symbol

    pop r13
    pop r12
    pop rbx
    ret

t_module_src_name: db "T"

; module_source_lookup_tagged(rdi=tagged value) -> rax = tagged
; (source-string . origin-string) cons, or IMM_NIL if NAME isn't a
; known embedded module (or isn't a String at all — every real
; $require-resolve call passes one via princ-to-string, so a non-
; string argument is out of v0 scope here rather than a real case).
global module_source_lookup_tagged
module_source_lookup_tagged:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi
    call is_string
    test rax, rax
    jz .none
    mov rdi, rbx
    call string_bytes
    mov r12, rax                    ; input ptr
    mov rdi, rbx
    call string_len
    mov r13, rax                      ; input len

    lea rbx, [rel module_table]
    xor r14, r14                        ; row index
.loop:
    cmp r14, MODULE_TABLE_COUNT
    jae .none
    mov rax, r13
    cmp rax, [rbx+8]                      ; this row's name_len
    jne .next
    mov rdi, [rbx]                          ; name ptr
    mov rsi, r12                              ; input ptr
    mov rdx, r13                                ; len
    call bytes_equal
    test rax, rax
    jnz .found
.next:
    add rbx, MODULE_ROW_BYTES
    inc r14
    jmp .loop

.found:
    mov rdi, [rbx+16]                ; source_start
    mov rsi, [rbx+24]                  ; source_end
    sub rsi, rdi                         ; source len
    call make_string
    push rax                               ; [source_string]
    mov rdi, origin_embedded
    mov rsi, origin_embedded_len
    call make_string
    mov rsi, rax
    pop rdi
    call cons                                ; (source . origin)
    jmp .done
.none:
    mov rax, IMM_NIL
.done:
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
