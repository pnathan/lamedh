; compiler.asm — compiles a Lisp form directly to native x86-64 code.
; This is a single-pass, no-IR compiler: `compile_form` recurses over the
; already-parsed s-expression (which is already a heap structure — see
; reader.asm) and calls the codegen.asm emitters as it goes. There is no
; bytecode and no separate optimization pass; the recursion structure of
; the compiler *is* the code-generation strategy.
;
; v0 special forms (this stage): QUOTE, IF, DEFINE, and binary + - * < =.
; Global variable reference compiles to a direct absolute-address load
; from the symbol's own value cell — resolved once, at compile time, to
; a fixed heap address, because a symbol's heap address never changes
; once interned. That is already a real win over a tree-walking
; interpreter's per-reference environment-chain lookup: a name reference
; here costs one load, full stop, decided before the program ever runs.
;
; LAMBDA, closures, and CALL are the next stage (see compiler2.asm).

%include "src/tags.inc"
%include "src/regs.inc"

extern car
extern cdr
extern cons
extern lisp_eq
extern intern_symbol
extern codegen_here
extern patch_rel32
extern emit8
extern emit_mov_reg_imm64
extern emit_mov_rr
extern emit_add_rr
extern emit_sub_rr
extern emit_cmp_rr
extern emit_imul_rr
extern emit_sar_imm8
extern emit_push_reg
extern emit_pop_reg
extern emit_load_local
extern emit_store_local
extern emit_sete_al
extern emit_setl_al
extern emit_movzx_eax_al
extern emit_add_rax_imm32
extern emit_imul_rax_imm32
extern emit_ret
extern emit_push_rbp_frame
extern emit_leave
extern emit_cmp_rax_imm64
extern emit_je
extern emit_jne
extern emit_jno
extern emit_jmp32
extern emit_call32
extern emit_load_mem64
extern emit_store_mem64
extern emit_load_based
extern emit_store_based
extern emit_or_rax_imm32
extern emit_and_rax_imm32
extern emit_sub_rax_imm32
extern emit_sub_rsp_imm32
extern emit_load_rsp_disp8
extern emit_jmp_reg
extern emit_call_reg
extern emit_load_local_zero_disp
extern data_alloc
extern emit_setne_al
extern patch_imm64
extern emit_add_reg_imm32
extern print_fixnum
extern print_newline
extern print_value
extern string_length_tagged
extern string_ref_tagged
extern string_append
extern substring
extern read_from_string_tagged
extern princ_to_string
extern file_open
extern file_close
extern file_write
extern file_read
extern float_of_fixnum
extern float_add
extern float_sub
extern float_mul
extern float_div
extern float_lt
extern make_array
extern make_typed_array
extern array_ref
extern array_set
extern array_length_tagged
extern hash_code_tagged
extern mod_tagged
extern remainder_tagged
extern emit_jl
extern emit_load_stack_arg
extern set_overflow_flag
extern flag_set_p
extern clear_flag
extern clear_all_flags
extern fail_wrong_type
extern gensym
extern heap_bytes_used
extern gc_verify
extern rc_collect
extern rc_safepoint
extern rc_register
extern rc_store_cell
extern rc_pin_deep
extern rc_pin_enter
extern rc_pin_leave
extern rc_refcount
extern heap_bytes_live
extern make_char_from_fixnum
extern char_code_tagged
extern stringp_tagged
extern symbolp_tagged
extern module_source_lookup_tagged
extern eval_module_source_tagged
extern intern_tagged
extern record_brand_tagged
extern record_fields_tagged
extern boundp_tagged
extern code_char_string
extern random_tagged
extern random_seed_tagged
extern feature_enabled_p
extern capability_mask_allows_p
extern push_capability_mask
extern pop_capability_mask
extern lognot_tagged
extern logand_tagged
extern logior_tagged
extern logxor_tagged
extern ash_tagged
extern symbol_plist
extern set_symbol_plist
extern set_symbol_value
extern port_open_input_file_tagged
extern port_open_output_file_tagged
extern port_open_append_file_tagged
extern port_open_input_bytes_tagged
extern port_open_output_bytes_tagged
extern port_output_contents_tagged
extern port_stdin_tagged
extern port_stdout_tagged
extern port_stderr_tagged
extern port_read_byte_tagged
extern port_read_bytes_tagged
extern port_write_byte_tagged
extern port_write_bytes_tagged
extern port_flush_tagged
extern port_close_tagged
extern port_open_p_tagged
extern port_input_p_tagged
extern port_output_p_tagged
extern port_seekable_p_tagged
extern port_position_tagged
extern port_seek_tagged
extern port_p_tagged
extern port_name_tagged
extern port_kind_tagged

%define FRAME_NOT_FOUND 0x7FFFFFFF

section .rodata
kw_quote:  db "QUOTE"
kw_function: db "FUNCTION"
kw_gensym: db "GENSYM"
kw_heap_bytes_used: db "HEAP-BYTES-USED"
kw_gc_verify: db "GC-VERIFY"
kw_gc_collect: db "GC-COLLECT"
kw_refcount: db "REFCOUNT"
kw_heap_bytes_live: db "HEAP-BYTES-LIVE"
kw_jit_optimize: db "JIT-OPTIMIZE"
kw_stringp: db "STRINGP"
kw_symbolp: db "SYMBOLP"
kw_module_source_lookup: db "$MODULE-SOURCE-LOOKUP"
kw_eval_module_source: db "$EVAL-MODULE-SOURCE"
kw_intern: db "INTERN"
kw_record_new: db "RECORD-NEW"
kw_record_brand: db "RECORD-BRAND"
kw_record_fields: db "RECORD-FIELDS"
kw_closure_nfree: db "CLOSURE-NFREE"
kw_boundp: db "BOUNDP"
not_callable_err_msg: db "not a function"
not_callable_err_msg_len: equ $ - not_callable_err_msg
kw_symbol_plist: db "SYMBOL-PLIST"
kw_set_symbol_plist: db "SET-SYMBOL-PLIST!"
kw_set: db "SET"
kw_port_open_input_file: db "PORT-OPEN-INPUT-FILE*"
kw_port_open_output_file: db "PORT-OPEN-OUTPUT-FILE*"
kw_port_open_append_file: db "PORT-OPEN-APPEND-FILE*"
kw_port_open_input_bytes: db "PORT-OPEN-INPUT-BYTES*"
kw_port_open_output_bytes: db "PORT-OPEN-OUTPUT-BYTES*"
kw_port_output_contents: db "PORT-OUTPUT-CONTENTS*"
kw_port_stdin: db "PORT-STDIN*"
kw_port_stdout: db "PORT-STDOUT*"
kw_port_stderr: db "PORT-STDERR*"
kw_port_read_byte: db "PORT-READ-BYTE*"
kw_port_read_bytes: db "PORT-READ-BYTES*"
kw_port_write_byte: db "PORT-WRITE-BYTE*"
kw_port_write_bytes: db "PORT-WRITE-BYTES*"
kw_port_flush: db "PORT-FLUSH*"
kw_port_close: db "PORT-CLOSE*"
kw_port_open_p: db "PORT-OPEN-P*"
kw_port_input_p: db "PORT-INPUT-P*"
kw_port_output_p: db "PORT-OUTPUT-P*"
kw_port_seekable_p: db "PORT-SEEKABLE-P*"
kw_port_position: db "PORT-POSITION*"
kw_port_seek: db "PORT-SEEK*"
kw_port_p: db "PORT-P*"
kw_port_name: db "PORT-NAME*"
kw_port_kind: db "PORT-KIND*"
kw_apply: db "APPLY"
kw_if:     db "IF"
kw_define: db "DEFINE"
kw_defdynamic: db "DEFDYNAMIC"
kw_vau: db "$VAU"
kw_defexpr: db "DEFEXPR"
kw_lambda: db "LAMBDA"
kw_defmacro: db "DEFMACRO"
kw_cons:   db "CONS"
kw_car:    db "CAR"
kw_cdr:    db "CDR"
kw_eqp:    db "EQ"
kw_atom:   db "ATOM"
kw_nullp:  db "NULL"
kw_add:    db "+"
kw_sub:    db "-"
kw_mul:    db "*"
kw_lt:     db "<"
kw_eq:     db "="
kw_catch:  db "CATCH"
kw_throw:  db "THROW"
kw_print:  db "PRINT"
kw_newline: db "NEWLINE"
kw_progn:  db "PROGN"
kw_cond:   db "COND"
kw_and:    db "AND"
kw_or:     db "OR"
kw_let:      db "LET"
kw_let_star: db "LET*"
kw_setq:     db "SETQ"
kw_error:         db "ERROR"
kw_handler_case:  db "HANDLER-CASE"
kw_errorset:      db "ERRORSET"
kw_unwind_protect: db "UNWIND-PROTECT"
kw_error_p:       db "ERROR-P"
kw_error_message: db "ERROR-MESSAGE"
kw_error_data:    db "ERROR-DATA"
kw_block:       db "BLOCK"
kw_prog:        db "PROG"
kw_go:          db "GO"
kw_return:      db "RETURN"
kw_return_from: db "RETURN-FROM"
kw_while:       db "WHILE"
kw_string_length: db "STRING-LENGTH"
kw_string_length_star: db "STRING-LENGTH*"
kw_string_ref:    db "STRING-REF"
kw_string_append: db "STRING-APPEND"
kw_substring:     db "SUBSTRING"
kw_read_from_string: db "READ-FROM-STRING"
kw_princ_to_string:  db "PRINC-TO-STRING"
kw_eval:             db "EVAL"
kw_fd_open:  db "FD-OPEN"
kw_fd_close: db "FD-CLOSE"
kw_fd_write: db "FD-WRITE"
kw_fd_read:  db "FD-READ"
kw_float:    db "FLOAT"
kw_fadd:     db "F+"
kw_fsub:     db "F-"
kw_fmul:     db "F*"
kw_fdiv:     db "F/"
kw_flt:      db "F<"
kw_make_array:   db "ARRAY"
kw_typed_array:  db "TYPED-ARRAY"
kw_array_ref:    db "FETCH"
kw_array_set:    db "STORE"
kw_array_length: db "ARRAY-LENGTH*"
kw_hash_code:    db "HASH-CODE"
kw_make_char:    db "MAKE-CHAR"
kw_char_code:    db "CHAR-CODE"
kw_code_char:    db "CODE-CHAR"
kw_random:       db "RANDOM"
kw_feature_enabled_p:        db "FEATURE-ENABLED-P"
kw_capability_mask_allows_p: db "CAPABILITY-MASK-ALLOWS-P"
kw_push_capability_mask:     db "PUSH-CAPABILITY-MASK!"
kw_pop_capability_mask:      db "POP-CAPABILITY-MASK!"
kw_random_seed:  db "RANDOM-SEED!"
kw_lognot:       db "LOGNOT"
kw_logand:       db "LOGAND"
kw_logior:       db "LOGIOR"
kw_logxor:       db "LOGXOR"
kw_ash:          db "ASH"
kw_mod:          db "MOD"
kw_remainder:    db "REMAINDER"
kw_flag_set_p:      db "FLAG-SET-P"
kw_clear_flag:      db "CLEAR-FLAG"
kw_clear_all_flags: db "CLEAR-ALL-FLAGS"
kw_rest:   db "&REST"

section .data
align 8
global current_scope
global current_scope
current_scope: dq IMM_NIL     ; compile-time lexical scope: a list of
                              ; (symbol . rbp-disp) pairs for whichever
                              ; function is currently being compiled;
                              ; IMM_NIL means "top level, globals only".

section .bss
align 8
; The non-local-exit primitive CATCH/THROW is built on: a fixed-depth
; stack of installed catch points. Each 32-byte frame is
; [0]=tag [8]=saved rbp [16]=saved rsp [24]=resume target. THROW walks
; it from the top for an EQ tag match, then restores rbp/rsp to that
; frame's saved state and jumps to its resume point with the thrown
; value in rax — an ordinary longjmp, expressible because nothing here
; answers to a C ABI's notion of what may cross a call boundary.
global catch_stack
global catch_stack_top
catch_stack: resb (256 * 32)
catch_stack_top: resq 1

; Host-side scratch cell for compile_lambda's &REST handling: holds the
; rest-parameter symbol (or IMM_NIL) across the span between
; split_rest_params and the point where the prologue's rest-list-building
; loop is emitted. Safe as a single global cell (not a stack/recursion
; slot) because nothing in that span recurses into compile_lambda —
; recursive compilation only happens later, while compiling the body.
global rest_sym_scratch
rest_sym_scratch: resq 1

; compile_nesting_depth/macroexpand_memo: memoizes macro expansion so
; that scanning a lambda body's free variables (scan_free_vars) and
; later actually compiling it (compile_form) see the identical
; expansion object, instead of each independently re-running the
; transformer (docs/spec-tco-capture-gc.md section 1 — measured 2D+1
; re-expansions of a macro at lambda-nesting depth D before this).
; Keyed by the call FORM's own cons-cell address (0 = empty slot;
; never a real key, since a real form is a live heap address): forms
; are heap-allocated, immutable once read, and — with no GC in this
; kernel yet — never relocate or have their address reused within one
; process, so identity-by-address is sound for as long as the memo is
; kept. compile_nesting_depth (incremented/decremented in
; compile_thunk, alongside its own current_scope/current_frame_depth/
; current_prog_ctx save-restore) distinguishes a fresh top-level
; compile from a nested one (EVAL reached from inside a still-running
; macro transformer, compile_thunk's own header comment) — the memo is
; cleared only on the 0->1 transition, bounding its size to one
; top-level form's own macro-call population rather than growing
; across an entire program.
global compile_nesting_depth
compile_nesting_depth: resq 1
%define MACROEXPAND_MEMO_SIZE 65536
; Each entry is a (key, value, generation) triple — clearing the whole
; table on every top-level form (a bare rep-stosq over the first
; version of this table measurably slowed the stdlib_conformance load
; down, since most top-level forms never populate more than a handful
; of a 65536-entry table) is a single `inc` of a generation counter
; instead: a slot is live only while its own stored generation matches
; [macroexpand_generation], so bumping the counter invalidates every
; slot in O(1) with no memory traffic at all.
global macroexpand_memo
global macroexpand_memo_end
macroexpand_memo: resq (MACROEXPAND_MEMO_SIZE*3)
macroexpand_memo_end:
macroexpand_generation: resq 1

; current_frame_depth: how many rbp-relative local slots are already
; considered reserved in the *current* function's own frame (params +
; frees + REST slot, plus whatever LET/LET* nesting is currently
; active) — the same "how deep am I" count compile_lambda already
; computes for its own prologue, just kept live across LET/LET* so
; they know where their own slots start. Saved/restored around a
; nested LAMBDA's or LET's body exactly like current_scope is.
current_frame_depth: resq 1

; current_prog_ctx: the host address of the innermost active PROG's own
; scratch bookkeeping buffer (labels_seen/pending_gos/pending_returns
; and their counts — compile_prog's own local machine-stack allocation,
; see its comment), or 0 when no PROG is active. compile_form's GO/
; RETURN dispatch reads this to find its way back to the enclosing
; PROG, however deeply nested the GO/RETURN form itself is inside
; ordinary IF/WHEN/LET expressions within a PROG item — the same
; "compiler-global, saved/restored around nesting" technique
; current_scope/current_frame_depth already use. v0 scope: GO/RETURN
; are lexical only, not the spec's own dynamic-extent version — reset
; to 0 (not saved-and-restored-to-the-outer-value) around a nested
; LAMBDA's own body compilation (compile_lambda), so a GO/RETURN
; textually inside a closure defined within a PROG item correctly
; fails to resolve (an unknown-label compile-time trap) rather than
; silently jumping into a different, already-returned function's code.
current_prog_ctx: resq 1

; tail_ctx: consume-on-entry compile-time flag (docs/spec-tco-capture-
; gc.md section 2) — 1 means "the form compile_form is about to
; compile is in tail position", 0 otherwise. compile_form reads this
; into its own r15 at entry and immediately zeroes it, so every
; subform starts non-tail by default; only the small set of tail-
; transparent special-form helpers (compile_progn/compile_if/
; compile_cond/compile_and/compile_or/compile_let/compile_let_star,
; and compile_call for the call itself) re-arm it — writing their own
; saved snapshot back into this cell — immediately before compiling
; whichever of their own subforms is itself in tail position. Forgetting
; to re-arm is always the safe failure (a missed tail-call optimization,
; not a wrongly-taken one), which is why this is consume-on-entry
; rather than push/pop-restored: a stale "still armed" value could
; otherwise leak into an unrelated, unaudited call site.
tail_ctx: resq 1

global current_lambda_depth
; current_lambda_depth: how many LAMBDA bodies are currently being
; compiled, one inside the other (incremented/decremented around
; compile_lambda's own body compile, saved/restored like
; current_prog_ctx). compile_call's own tail-call codegen additionally
; requires this to be nonzero before ever emitting a frame-reuse jump,
; independently of what tail_ctx says — a defensive belt-and-suspenders
; check (docs/spec-tco-capture-gc.md section 2.3) so a tail jump can
; never be emitted while compiling a top-level thunk (compile_thunk
; never increments this), regardless of any bug in the tail_ctx
; plumbing above.
current_lambda_depth: resq 1

; lambda_frame_depth_scratch: holds a LAMBDA's own computed base_index
; (params+frees+REST slot count) from where it's computed (prologue
; emission) until where it's needed (installing current_frame_depth
; right before the body compiles) — same one-cell-is-safe reasoning as
; rest_sym_scratch: nothing recurses into compile_lambda in between.
lambda_frame_depth_scratch: resq 1

; nregslots_scratch: how many of this LAMBDA's local slots are consumed
; by "register-convention" positions before free vars start — 4 (3
; register-passed argument slots plus the REST slot itself) whenever
; this lambda has a &REST param, regardless of its own fixed-parameter
; count, since the calling convention (compile_call_args) always passes
; global argument indices 0/1/2 in rsi/rdx/rcx no matter how many of
; them a given callee treats as fixed versus REST; otherwise
; min(nfixed,3), same as before &REST existed. Computed once and reused
; at every site that used to recompute this inline, so the three sites
; (free-var frame start, this function's own frame depth, and each free
; var's own slot index) can never disagree with each other or with the
; REST slot's own fixed disp of -32. Same one-cell-is-safe reasoning as
; rest_sym_scratch above.
nregslots_scratch: resq 1

section .text

; cadr(rdi=cons) -> car(cdr(x)).  caddr -> car(cdr(cdr(x))). Small host-side
; conveniences over the reader's car/cdr; not part of the target language.
cadr:
    call cdr
    mov rdi, rax
    jmp car

caddr:
    call cdr
    mov rdi, rax
    call cdr
    mov rdi, rax
    jmp car

cadddr:
    call cdr
    mov rdi, rax
    call cdr
    mov rdi, rax
    call cdr
    mov rdi, rax
    jmp car

; fold_binop_ast(rdi=head symbol (tagged), rsi=operand form list, at
; least 2 elements) -> rax = a fresh cons AST folding every operand
; left-to-right into nested 2-operand calls under HEAD: `(op0 op1 op2
; op3)` becomes `(HEAD (HEAD (HEAD op0 op1) op2) op3)`. Used by
; LOGAND/LOGIOR/LOGXOR's own dispatch (compile_form, further down) to
; give those real compiler special forms the reference's own variadic
; behavior without teaching compile_binary_hostcall anything about
; operand counts — the fold happens once, host-side, over the raw
; (unevaluated) operand forms themselves, and the result is handed
; back to compile_form to compile normally, terminating at the
; existing exact-2-operand case.
fold_binop_ast:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi                    ; head symbol
    mov rdi, rsi
    call car
    mov r12, rax                      ; acc = op0
    mov rdi, rsi
    call cdr
    mov r13, rax                        ; rest = (op1 op2 ...)
.loop:
    cmp r13, IMM_NIL
    je .done
    mov rdi, r13
    call car
    mov r14, rax                          ; op
    mov rdi, r14
    mov rsi, IMM_NIL
    call cons                                ; (op)
    mov rsi, rax
    mov rdi, r12
    call cons                                  ; (acc op)
    mov rsi, rax
    mov rdi, rbx
    call cons                                    ; (head acc op)
    mov r12, rax
    mov rdi, r13
    call cdr
    mov r13, rax
    jmp .loop
.done:
    mov rax, r12
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; sym_is(rdi=tagged value, rsi=name ptr, rdx=name len) -> rax=1/0.
; True only if rdi is itself the (unique, interned) symbol named by
; (rsi,rdx) — an EQ pointer compare after one intern lookup.
sym_is:
    push rbx
    mov rbx, rdi
    mov rdi, rsi
    mov rsi, rdx
    call intern_symbol
    xor rdi, rdi
    cmp rax, rbx
    sete dil
    movzx rax, dil
    pop rbx
    ret

; is_cons(rdi) -> rax=1/0
global is_cons
is_cons:
    mov rax, rdi
    and rax, TAG_MASK
    xor rcx, rcx
    cmp rax, TAG_CONS
    sete cl
    mov rax, rcx
    ret

; frame_lookup(rdi=symbol, rsi=frame — a list of (symbol . disp) pairs)
; -> rax = disp if found, else FRAME_NOT_FOUND.
frame_lookup:
    push rbx
    push r12
    push r13
    mov rbx, rdi
    mov r12, rsi
.loop:
    cmp r12, IMM_NIL
    je .nf
    mov rdi, r12
    call car
    mov r13, rax                     ; pair
    mov rdi, r13
    call car                            ; pair's symbol
    cmp rax, rbx
    jne .next
    mov rdi, r13
    call cdr                               ; pair's disp
    jmp .found
.next:
    mov rdi, r12
    call cdr
    mov r12, rax
    jmp .loop
.nf:
    mov rax, FRAME_NOT_FOUND
.found:
    pop r13
    pop r12
    pop rbx
    ret

; member(rdi=symbol, rsi=flat list of symbols) -> rax=1/0
member_sym:
    push rbx
    push r12
    mov rbx, rdi
    mov r12, rsi
.loop:
    cmp r12, IMM_NIL
    je .no
    mov rdi, r12
    call car
    cmp rax, rbx
    je .yes
    mov rdi, r12
    call cdr
    mov r12, rax
    jmp .loop
.yes:
    mov rax, 1
    jmp .out
.no:
    xor rax, rax
.out:
    pop r12
    pop rbx
    ret

; list_length(rdi=list) -> rax=count
list_length:
    push rbx
    push r12
    mov r12, rdi
    xor rbx, rbx
.loop:
    cmp r12, IMM_NIL
    je .done
    inc rbx
    mov rdi, r12
    call cdr
    mov r12, rax
    jmp .loop
.done:
    mov rax, rbx
    pop r12
    pop rbx
    ret

; build_frame_from_list(rdi=symbol list, rsi=base index)
; -> rax = frame list of (symbol . disp) pairs, disp = -8*(base+i+1) for
; the i'th symbol (0-based) in the input list.
build_frame_from_list:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi                  ; cursor
    mov r12, rsi                    ; index (mutable)
    mov r13, IMM_NIL                  ; acc
.loop:
    cmp rbx, IMM_NIL
    je .done
    mov rdi, rbx
    call car
    mov r14, rax                        ; symbol
    mov rax, r12
    inc rax
    imul rax, rax, -8
    mov rdi, r14
    mov rsi, rax
    call cons                              ; pair
    mov rdi, rax
    mov rsi, r13
    call cons                                ; acc = (pair . acc)
    mov r13, rax
    inc r12
    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .loop
.done:
    mov rax, r13
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; build_param_frame(rdi=params list) -> rax = frame list of (symbol . disp)
; pairs. The first 3 params arrive in registers and are spilled to fixed
; local slots (disp -8,-16,-24, same as before); the 4th onward arrive
; already on the caller's stack (pushed before the call) and are simply
; addressed in place at their fixed positive offset — no copy needed,
; since a disp32 is a disp32 regardless of sign. This is what lets a
; call take more than 3 arguments: params[3] sits at [rbp+16], params[4]
; at [rbp+24], and so on — the same layout compile_call's callers must
; leave on the stack (see compile_call_args).
build_param_frame:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi                  ; cursor
    xor r12, r12                    ; index (mutable)
    mov r13, IMM_NIL                  ; acc
.loop:
    cmp rbx, IMM_NIL
    je .done
    mov rdi, rbx
    call car
    mov r14, rax                        ; symbol
    cmp r12, 3
    jae .stack_disp
    mov rax, r12
    inc rax
    imul rax, rax, -8                     ; register-spilled: -8,-16,-24
    jmp .have_disp
.stack_disp:
    mov rax, r12
    sub rax, 3
    imul rax, rax, 8
    add rax, 16                            ; caller-stack: 16,24,32,...
.have_disp:
    mov rdi, r14
    mov rsi, rax
    call cons                              ; pair
    mov rdi, rax
    mov rsi, r13
    call cons                                ; acc = (pair . acc)
    mov r13, rax
    inc r12
    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .loop
.done:
    mov rax, r13
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; split_rest_params(rdi=params list) -> rax=fixed_list, rdx=rest_sym.
; Splits a LAMBDA params list at "&REST": everything before it becomes
; fixed_list (order preserved); the symbol immediately following &REST
; is returned as rest_sym. No &REST present -> fixed_list = the whole
; input list, rest_sym = IMM_NIL.
split_rest_params:
    push rbx
    push r12
    mov rbx, rdi
    cmp rbx, IMM_NIL
    jne .have_head
    mov rax, IMM_NIL
    mov rdx, IMM_NIL
    jmp .out
.have_head:
    mov rdi, rbx
    call car
    mov r12, rax                      ; head symbol
    mov rdi, r12
    mov rsi, kw_rest
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_rest
    mov rdi, rbx
    call cadr
    mov rdx, rax                        ; rest_sym
    mov rax, IMM_NIL                      ; fixed_list = NIL (nothing after &REST)
    jmp .out
.not_rest:
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call split_rest_params
    push rax                                ; fixed_tail
    push rdx                                  ; rest_sym
    mov rdi, r12
    mov rsi, [rsp+8]
    call cons
    mov rdx, [rsp]
    add rsp, 16
.out:
    pop r12
    pop rbx
    ret

; append_lists(rdi=list1, rsi=list2) -> rax = list1 with list2 as its tail
append_lists:
    push rbx
    push r12
    mov rbx, rdi
    mov r12, rsi
    cmp rbx, IMM_NIL
    jne .cons_case
    mov rax, r12
    jmp .out
.cons_case:
    mov rdi, rbx
    call car
    push rax
    mov rdi, rbx
    call cdr
    mov rdi, rax
    mov rsi, r12
    call append_lists
    pop rdi
    mov rsi, rax
    call cons
.out:
    pop r12
    pop rbx
    ret

; build_list2(rdi=a, rsi=b) -> rax = (a b). build_list3(rdi=a, rsi=b,
; rdx=c) -> rax = (a b c). Small host-side AST-construction helpers,
; the same cons-chaining idiom compile_unwind_protect's own synthetic
; (LAMBDA () cleanup...) already uses, factored out because
; compile_let's dynamic-variable rewrite (below) builds several
; two/three-element forms — (SETQ name val), (tmp name) bindings.
build_list2:
    push rbx
    mov rbx, rdi
    mov rdi, rsi
    mov rsi, IMM_NIL
    call cons
    mov rdi, rbx
    mov rsi, rax
    call cons
    pop rbx
    ret

build_list3:
    push rbx
    push r12
    mov rbx, rdi
    mov r12, rsi
    mov rdi, rdx
    mov rsi, IMM_NIL
    call cons
    mov rdi, r12
    mov rsi, rax
    call cons
    mov rdi, rbx
    mov rsi, rax
    call cons
    pop r12
    pop rbx
    ret

; DEFDYNAMIC marks a symbol as dynamic by PUTP-ing a dedicated marker
; indicator onto its plist directly (host-side cons/symbol_plist/
; set_symbol_plist calls, bypassing the compiled PUTP function
; entirely) — the same "compile-time symbol metadata, visible
; immediately to every later compile-time check in this process"
; idiom DEFMACRO's own macro slot already relies on, applied to the
; plist instead of a dedicated header field so no symbol layout change
; is needed.
kw_dynamic_marker: db "LAMEDH-ASM-DYNAMIC-MARKER"
kw_dynamic_marker_len: equ $ - kw_dynamic_marker

; mark_symbol_dynamic(rdi=tagged symbol) — no return value used.
mark_symbol_dynamic:
    push rbx
    mov rbx, rdi
    call symbol_plist                  ; rdi unchanged = symbol; rax = old plist
    push rax                             ; [old_plist]
    mov rdi, kw_dynamic_marker
    mov rsi, kw_dynamic_marker_len
    call intern_symbol
    mov rdi, rax
    mov rsi, IMM_TRUE
    call cons                              ; (marker . T)
    mov rdi, rax
    pop rsi                                  ; old_plist
    call cons                                  ; ((marker . T) . old_plist)
    mov rdi, rbx
    mov rsi, rax
    call set_symbol_plist
    pop rbx
    ret

; is_symbol_dynamic(rdi=tagged symbol) -> rax = 1/0. Walks the plist
; (an ordinary alist) looking for the dynamic marker indicator by EQ
; (pointer identity — both sides are the one interned marker symbol),
; the same indicator-equality rule GETP/PUTP already document.
is_symbol_dynamic:
    push rbx
    push r12
    push r13
    call symbol_plist                    ; rdi unchanged = symbol; rax = plist
    mov r12, rax                            ; plist cursor
    mov rdi, kw_dynamic_marker
    mov rsi, kw_dynamic_marker_len
    call intern_symbol
    mov r13, rax                              ; marker symbol
.loop:
    cmp r12, IMM_NIL
    je .no
    mov rdi, r12
    call car
    mov rbx, rax                                ; pair
    mov rdi, rbx
    call car
    cmp rax, r13
    je .yes
    mov rdi, r12
    call cdr
    mov r12, rax
    jmp .loop
.yes:
    mov rax, 1
    jmp .done
.no:
    xor rax, rax
.done:
    pop r13
    pop r12
    pop rbx
    ret

; scan_free_vars(rdi=form, rsi=enclosing frame, rdx=acc list of symbols)
; -> rax = acc', with every symbol in `form` that resolves in the
; enclosing frame added (deduplicated). QUOTE'd data is not descended
; into. This is the free-variable analysis that decides what a nested
; LAMBDA must capture — a real (if single-level, see README) closure
; conversion pass, run before a single byte of the lambda's body is
; emitted.
global scan_free_vars
scan_free_vars:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi                    ; form
    mov r12, rsi                      ; enclosing frame
    mov r13, rdx                        ; acc

    mov rdi, rbx
    call is_cons
    test rax, rax
    jnz .list_case

    mov rax, rbx
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .done
    mov rax, rbx
    UNTAG_PTR rax
    cmp qword [rax], HDR_SYMBOL
    jne .done

    mov rdi, rbx
    mov rsi, r12
    call frame_lookup
    cmp rax, FRAME_NOT_FOUND
    je .done
    mov rdi, rbx
    mov rsi, r13
    call member_sym
    test rax, rax
    jnz .done
    mov rdi, rbx
    mov rsi, r13
    call cons
    mov r13, rax
    jmp .done

.list_case:
    mov rdi, rbx
    call car
    mov r14, rax
    mov rdi, r14
    mov rsi, kw_quote
    mov rdx, 5
    call sym_is
    test rax, rax
    jnz .done                            ; (QUOTE ...) — do not descend

    ; If the head is a global bound as a macro, this call's own literal
    ; syntax `(head arg...)` never mentions whatever free variables the
    ; EXPANSION actually references (e.g. a variable that only appears
    ; inside a backquote template the transformer builds, like
    ; lib/25-variants.lisp's `` `(forall ,params ...) ``) — expand right
    ; here, exactly the way compile_form's own macro dispatch
    ; (invoke_macro, further down) does, and scan the expansion in
    ; place of the raw call. Without this, a nested LAMBDA whose body
    ; macro-expands to reference an outer free variable never captures
    ; it into the closure, and the reference resolves to whatever
    ; garbage/NIL happens to sit at the wrong frame slot once the
    ; expansion is actually compiled.
    mov rax, r14
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .not_macro_head
    mov rax, r14
    UNTAG_PTR rax
    cmp qword [rax], HDR_SYMBOL
    jne .not_macro_head
    mov rax, [rax+24]                    ; macro slot
    cmp rax, IMM_NIL
    je .not_macro_head
    mov r14, rax                           ; macro closure (tagged)
    mov rdi, rbx
    call cdr
    mov rdx, rax                             ; args list
    mov rsi, r14
    mov rdi, rbx                               ; call form (memo key)
    call macroexpand_once                        ; rax = expansion
    mov rdi, rax
    mov rsi, r12
    mov rdx, r13
    call scan_free_vars
    mov r13, rax
    jmp .done

.not_macro_head:
    mov rdi, r14
    mov rsi, r12
    mov rdx, r13
    call scan_free_vars
    mov r13, rax

    mov rdi, rbx
    call cdr
    mov rbx, rax
.walk:
    mov rdi, rbx
    call is_cons
    test rax, rax
    jz .done
    mov rdi, rbx
    call car
    mov r14, rax
    mov rdi, r14
    mov rsi, r12
    mov rdx, r13
    call scan_free_vars
    mov r13, rax
    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .walk

.done:
    mov rax, r13
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; clear_macroexpand_memo() — invalidate the whole table in O(1): bump
; the generation counter, touching no memory at all. Called from
; compile_thunk on the compile_nesting_depth 0->1 transition (a fresh
; top-level form), purely to bound the table's own effective
; population across a long-running program's worth of top-level forms
; — not for correctness (see macroexpand_memo's own comment on address
; stability). An earlier version of this routine zeroed the whole
; table with rep stosq on every top-level form, which measurably
; slowed the stdlib_conformance load down (most top-level forms never
; populate more than a handful of a 65536-entry table, so the O(table
; size) clear dominated the O(macro calls) work it was meant to save).
clear_macroexpand_memo:
    inc qword [macroexpand_generation]
    ret

; macroexpand_once(rdi=call form (tagged cons, the memo key), rsi=macro
; closure (tagged), rdx=raw args list) -> rax = expansion (tagged).
; Open-addressing lookup (linear probe, wrapping) into
; macroexpand_memo keyed by rdi's own address; on a miss, invokes the
; macro exactly as a bare invoke_macro call already did at both of
; this routine's two call sites (compile_form's own macro dispatch,
; and scan_free_vars's macro branch) and remembers the result — so a
; transformer with observable side effects (GENSYM, PUTP, ...) runs
; once per call site, not once per scan plus once to actually compile.
; A slot is live only while its own stored generation equals
; [macroexpand_generation]; anything else (including .bss's own
; zero-initialized state, which never matches the generation counter's
; own post-first-clear value of 1) reads as empty regardless of
; whatever key/value garbage is still sitting there. Falls back to an
; unmemoized expansion (matching pre-memo behavior exactly) if linear
; probing exhausts the whole table without finding either a match or
; an empty slot — correctness never depends on the memo hitting, only
; on scan and compile agreeing when it does.
global macroexpand_once
macroexpand_once:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov rbx, rdi                    ; key (call form)
    mov r12, rsi                      ; macro closure
    mov r13, rdx                        ; args list

    mov rax, rbx
    shr rax, 4
    and rax, (MACROEXPAND_MEMO_SIZE-1)
    imul rax, rax, 24                      ; entry index -> byte offset (3 qwords/entry)
    lea r14, [rel macroexpand_memo]
    add r14, rax                             ; slot ptr
    xor r15, r15                               ; probes so far
.probe:
    cmp r15, MACROEXPAND_MEMO_SIZE
    jae .full
    mov rax, [rel macroexpand_generation]
    cmp [r14+16], rax
    jne .miss                                    ; stale/never-written slot = empty
    mov rax, [r14]
    cmp rax, rbx
    je .hit
    add r14, 24
    inc r15
    lea rax, [rel macroexpand_memo]
    lea rax, [rax + (MACROEXPAND_MEMO_SIZE*24)]
    cmp r14, rax
    jb .probe
    lea r14, [rel macroexpand_memo]          ; wrap to the table start
    jmp .probe
.hit:
    mov rax, [r14+8]
    jmp .out
.miss:
    mov rdi, r12
    mov rsi, r13
    call invoke_macro                          ; rax = expansion
    mov [r14], rbx
    mov [r14+8], rax
    mov rcx, [rel macroexpand_generation]
    mov [r14+16], rcx
    jmp .out
.full:
    mov rdi, r12
    mov rsi, r13
    call invoke_macro
.out:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; ---------------------------------------------------------------------
; Transitive free-variable capture analysis
; (docs/spec-tco-capture-gc.md section 1.2)
;
; scan_free_vars above answers "which symbols mentioned anywhere in this
; subtree resolve in the enclosing frame?" — shadowing-blind, so a name
; re-bound *inside* a nested LAMBDA/LET/PROG is captured into the outer
; closure anyway (spec D2: a wasted slot per closure creation, a larger
; captured array for a future GC to walk, and a standing hygiene hazard).
; The routines below answer the sharper question — "which symbols are
; genuinely FREE in this lambda, and visible where it appears?" — with
; one walk per outermost LAMBDA whose per-lambda results are memoized by
; the lambda form's own cons address (capture_memo) and read back by
; compile_lambda at BOTH the frame-sizing site and the closure-copy-loop
; site, so the two can no longer disagree by construction.
;
; The spec describes the analysis as phase 1 (bottom-up raw free sets)
; then phase 2 (top-down intersection with what is actually in scope).
; This implementation fuses the two into a single walk, which is
; provably the same answer: writing `boundstack(L)` for the binders of
; every lambda/LET/PROG/HANDLER-CASE lexically enclosing L (but not L's
; own), and `avail(L)` for the compile-time scope that will be
; current_scope when L is compiled (params ++ rest ++ free_frame of the
; enclosing lambda, plus any intervening LET/PROG/HANDLER-CASE frame),
; spec phase 2 defines
;
;     capture(L) = raw_fv(L) INTERSECT avail(L)
;
; and avail(L) = boundstack(L) INTERSECT-COMPLEMENT-free ... concretely:
; every name in boundstack(L) that L references reaches L through the
; enclosing lambda's own capture list (the same recursion applied one
; level out), and every other name L references is either resolvable in
; the outermost enclosing scope (a captured global-frame local) or is a
; true global. So
;
;     capture(L) = { s in raw_fv(L) : s in boundstack(L)
;                                   or frame_lookup(s, outer scope) hits }
;
; which is exactly one predicate evaluated during the same walk that
; computes raw_fv(L). `bound` below is boundstack(L) with L's own
; binders consed on the front, and `cut` is the pointer *into* that same
; list where L's own binders end and the enclosing ones begin — so
; "shadowed by this lambda" is member-before-cut and "bound by an
; enclosing lambda" is member-from-cut-on, with no set copying at all.
; (append_lists shares its second argument's spine, so the cut pointer
; stays valid as binders are pushed.)
;
; Under-capture is the dangerous direction (spec section 1.4 risk 1): a
; name wrongly believed bound compiles to a *global* load instead of a
; compile error. Everything here therefore fails toward over-capture —
; only the small set of forms whose binding structure is known exactly
; (LAMBDA/$VAU/DEFMACRO/DEFEXPR/LET/LET*/PROG/HANDLER-CASE) removes
; anything; every other form falls through to a plain "walk the head and
; every argument" that can only ever add. -DCAPTURE_CHECK (below) builds
; an assertion mode that cross-checks every answer against
; scan_free_vars itself.

%define CAPTURE_MEMO_SIZE 65536
section .bss
align 8
; capture_memo — same shape, same generation-counter invalidation, and
; same "keyed by the form's own cons address" reasoning as
; macroexpand_memo above (see its comment for why address identity is
; sound here). Value = that lambda's capture list, in the exact order
; free-slot indices are assigned to it.
global capture_memo
global capture_memo_end
capture_memo: resq (CAPTURE_MEMO_SIZE*3)
capture_memo_end:
capture_generation: resq 1

section .data
align 8
; capture_scope — the enclosing compile-time scope the current analysis
; resolves its outermost free variables against ([current_scope] at the
; moment compile_lambda started). Saved/restored around
; analyze_lambda_captures AND around lambda_capture_list (which needs it
; set even on a memo hit, where no analysis runs) like current_scope
; itself, since a macro transformer invoked *during* the walk can
; re-enter the compiler (EVAL -> compile_thunk -> compile_lambda ->
; analyze_lambda_captures). Lives in .data, not .bss, for the same
; reason current_scope does: its "empty" value is IMM_NIL, and a
; zero-initialized cell would be read as a tagged fixnum 0 and walked
; as a frame list (frame_lookup -> car -> fail_wrong_type).
capture_scope: dq IMM_NIL

section .text

; car_safe/cdr_safe(rdi=value) -> rax. Like car/cdr but answer IMM_NIL
; for a non-cons instead of signaling. The analysis walks raw source it
; has not validated (a malformed (LET) with no binding list, a dotted
; body) and must never turn "this form would not have compiled" into
; "the compiler faulted while deciding what to capture". Leaf routines:
; they clobber rax and flags only, which is what lets the walk below
; keep intermediates in r10 across them.
car_safe:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_CONS
    jne .nil
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax]
    ret
.nil:
    mov rax, IMM_NIL
    ret

cdr_safe:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_CONS
    jne .nil
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+8]
    ret
.nil:
    mov rax, IMM_NIL
    ret

; is_symbol_p(rdi=value) -> rax=1/0
is_symbol_p:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .no
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_SYMBOL
    jne .no
    mov rax, 1
    ret
.no:
    xor rax, rax
    ret

; macro_slot_of(rdi=value) -> rax = the macro closure bound to this
; symbol, or IMM_NIL. Exactly the test compile_form's own macro dispatch
; and scan_free_vars's macro branch already perform inline.
macro_slot_of:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .none
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_SYMBOL
    jne .none
    mov rax, [rax+24]
    ret
.none:
    mov rax, IMM_NIL
    ret

; member_sym_until(rdi=symbol, rsi=list, rdx=stop) -> rax=1/0. member_sym
; over the prefix of `list` that ends at the cons cell `stop` (or at
; IMM_NIL, whichever comes first) — "is this name bound by the lambda
; being analyzed itself, as opposed to by one of its enclosing lambdas?"
member_sym_until:
    push rbx
    push r12
    push r13
    mov rbx, rdi
    mov r12, rsi
    mov r13, rdx
.loop:
    cmp r12, r13
    je .no
    cmp r12, IMM_NIL
    je .no
    mov rdi, r12
    call car_safe
    cmp rax, rbx
    je .yes
    mov rdi, r12
    call cdr_safe
    mov r12, rax
    jmp .loop
.yes:
    mov rax, 1
    jmp .out
.no:
    xor rax, rax
.out:
    pop r13
    pop r12
    pop rbx
    ret

; param_names(rdi=parameter list) -> rax = the symbols it binds, with
; the literal &REST marker dropped (the symbol after it is a real
; binding and is kept — split_rest_params's own contract, restated as a
; set rather than a split). Order is irrelevant: the result is only ever
; used as a membership set.
param_names:
    push rbx
    push r12
    mov rbx, rdi
    mov r12, IMM_NIL
.loop:
    mov rdi, rbx
    call is_cons
    test rax, rax
    jz .done
    mov rdi, rbx
    call car_safe
    push rax
    mov rdi, rax
    mov rsi, kw_rest
    mov rdx, 5
    call sym_is
    test rax, rax
    jnz .skip
    pop rdi
    mov rsi, r12
    call cons
    mov r12, rax
    jmp .next
.skip:
    add rsp, 8
.next:
    mov rdi, rbx
    call cdr_safe
    mov rbx, rax
    jmp .loop
.done:
    mov rax, r12
    pop r12
    pop rbx
    ret

; binding_names(rdi=a LET/LET* binding list) -> rax = the names it
; binds. A binding is (name init); a bare symbol binds itself.
binding_names:
    push rbx
    push r12
    mov rbx, rdi
    mov r12, IMM_NIL
.loop:
    mov rdi, rbx
    call is_cons
    test rax, rax
    jz .done
    mov rdi, rbx
    call car_safe
    mov rdi, rax
    call is_cons
    test rax, rax
    jz .bare
    mov rdi, rbx
    call car_safe
    mov rdi, rax
    call car_safe
    jmp .have
.bare:
    mov rdi, rbx
    call car_safe
.have:
    mov rdi, rax
    mov rsi, r12
    call cons
    mov r12, rax
    mov rdi, rbx
    call cdr_safe
    mov rbx, rax
    jmp .loop
.done:
    mov rax, r12
    pop r12
    pop rbx
    ret

; clear_capture_memo() — O(1) invalidation, same generation-counter
; technique (and same reason) as clear_macroexpand_memo. Called from
; compile_thunk on the compile_nesting_depth 0->1 transition, right
; beside it: the two memos are keyed the same way and must expire
; together, since a capture list is only meaningful alongside the
; macro expansions the walk that produced it saw.
clear_capture_memo:
    inc qword [capture_generation]
    ret

; capture_memo_find(rdi=key) -> rax = slot address (0 if the table is
; full), rdx = 1 on a live hit / 0 if the slot is free. Linear probe,
; wrapping, generation-checked — a transcription of macroexpand_once's
; own probe loop over its own table.
capture_memo_find:
    push rbx
    mov rbx, rdi
    mov rax, rbx
    shr rax, 4
    and rax, (CAPTURE_MEMO_SIZE-1)
    imul rax, rax, 24
    lea rcx, [rel capture_memo]
    add rcx, rax
    xor r8, r8
.probe:
    cmp r8, CAPTURE_MEMO_SIZE
    jae .full
    mov rax, [rel capture_generation]
    cmp [rcx+16], rax
    jne .empty
    mov rax, [rcx]
    cmp rax, rbx
    je .hit
    add rcx, 24
    inc r8
    lea rax, [rel capture_memo]
    lea rax, [rax + (CAPTURE_MEMO_SIZE*24)]
    cmp rcx, rax
    jb .probe
    lea rcx, [rel capture_memo]
    jmp .probe
.hit:
    mov rax, rcx
    mov rdx, 1
    jmp .out
.empty:
    mov rax, rcx
    xor rdx, rdx
    jmp .out
.full:
    xor rax, rax
    xor rdx, rdx
.out:
    pop rbx
    ret

; capture_memo_store(rdi=key, rsi=capture list). A full table simply
; drops the entry: correctness never depends on the memo hitting (a
; miss re-derives the identical list from the identical inputs), only
; on the two compile_lambda sites agreeing — and they agree because
; compile_lambda computes the list once and keeps it on its own host
; stack across the body compile, not because it looks it up twice.
capture_memo_store:
    push rbx
    push r12
    mov rbx, rdi
    mov r12, rsi
    call capture_memo_find
    test rax, rax
    jz .out
    mov [rax], rbx
    mov [rax+8], r12
    mov rcx, [rel capture_generation]
    mov [rax+16], rcx
.out:
    pop r12
    pop rbx
    ret

; capture_memo_lookup(rdi=key) -> rax = capture list, rdx = 1 on a hit
capture_memo_lookup:
    call capture_memo_find
    test rax, rax
    jz .miss
    test rdx, rdx
    jz .miss
    mov rax, [rax+8]
    mov rdx, 1
    ret
.miss:
    mov rax, IMM_NIL
    xor rdx, rdx
    ret

; operative_head_p(rdi=head symbol, rsi=bound) -> rax=1/0 — is this call
; site's head a global currently bound to a $VAU operative? Mirrors
; compile_form's own operative dispatch exactly, including its
; "lexically bound names are never operatives" guard: here that guard is
; "not bound by any enclosing binder, and not resolvable in the
; enclosing scope", which is the same predicate one compile step early
; (a name that IS resolvable gets captured by this very analysis, so it
; is in current_scope by the time compile_form asks).
operative_head_p:
    push rbx
    push r12
    mov rbx, rdi
    mov r12, rsi
    mov rdi, rbx
    call is_symbol_p
    test rax, rax
    jz .no
    mov rdi, rbx
    mov rsi, r12
    call member_sym
    test rax, rax
    jnz .no
    mov rdi, rbx
    mov rsi, [capture_scope]
    call frame_lookup
    cmp rax, FRAME_NOT_FOUND
    jne .no
    mov rax, rbx
    UNTAG_PTR rax
    mov rax, [rax+16]
    cmp rax, IMM_UNBOUND
    je .no
    mov rdx, rax
    and rdx, TAG_MASK
    cmp rdx, TAG_HEAPOBJ
    jne .no
    UNTAG_PTR rax
    cmp qword [rax], HDR_OPERATIVE
    jne .no
    mov rax, 1
    jmp .out
.no:
    xor rax, rax
.out:
    pop r12
    pop rbx
    ret

; fv_walk_list(rdi=list of forms, rsi=bound, rdx=cut, rcx=acc)
;   -> rax = acc'. Walks a *list of forms* (a lambda body, a PROG's
; items) rather than a single form — unlike scan_free_vars, which is
; handed a body list and falls into its own .list_case, treating the
; body's first form as if it were the head of a call (so a body whose
; first form is a bare symbol naming a macro would be macro-expanded).
fv_walk_list:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi
    mov r12, rsi
    mov r13, rdx
    mov r14, rcx
.loop:
    mov rdi, rbx
    call is_cons
    test rax, rax
    jz .out
    mov rdi, rbx
    call car_safe
    mov rdi, rax
    mov rsi, r12
    mov rdx, r13
    mov rcx, r14
    call fv_walk
    mov r14, rax
    mov rdi, rbx
    call cdr_safe
    mov rbx, rax
    jmp .loop
.out:
    mov rax, r14
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; fv_lambda(rdi=params, rsi=body, rdx=bound, rcx=cut, r8=acc,
;           r9=memo key or 0) -> rax = acc'
;
; The nested-lambda case of the walk, shared by LAMBDA/$VAU/DEFMACRO/
; DEFEXPR (spec open question Q2: a transformer body binds its own
; parameter list exactly like a lambda body does). Computes the inner
; lambda's own capture list against `bound` extended with its
; parameters and a cut placed at `bound` itself, memoizes it under the
; inner form's address (LAMBDA only — the other three are compiled
; through a *synthetic* (LAMBDA params . body) built with cons, whose
; address this analysis never sees, so compile_lambda re-derives those
; from current_scope through the miss path), then folds it into the
; enclosing lambda's own accumulator: every name the inner closure
; needs that this lambda does not itself bind, this lambda must capture
; too — that is precisely what makes capture transitive.
fv_lambda:
    push rbx
    push r12
    push r13
    push r14
    push r15
    push rbp
    mov rbx, rdi                        ; params
    mov r12, rsi                          ; body
    mov r13, rdx                            ; bound
    mov r14, rcx                              ; cut
    mov r15, r8                                 ; acc
    mov rbp, r9                                   ; memo key (0 = none)

    mov rdi, rbx
    call param_names
    mov rdi, rax
    mov rsi, r13
    call append_lists                   ; inner_bound = params ++ bound
    mov rdi, r12
    mov rsi, rax
    mov rdx, r13                          ; inner_cut = bound
    mov rcx, IMM_NIL
    call fv_walk_list
    push rax                              ; [inner capture list]

    cmp rbp, 0
    je .no_memo
    mov rdi, rbp
    mov rsi, [rsp]
    call capture_memo_store
.no_memo:
    pop rbx                               ; cursor over the inner list
.merge:
    mov rdi, rbx
    call is_cons
    test rax, rax
    jz .out
    mov rdi, rbx
    call car_safe
    mov rbp, rax                          ; symbol
    mov rdi, rbp
    mov rsi, r13
    mov rdx, r14
    call member_sym_until
    test rax, rax
    jnz .next                             ; this lambda binds it itself
    mov rdi, rbp
    mov rsi, r15
    call member_sym
    test rax, rax
    jnz .next
    mov rdi, rbp
    mov rsi, r15
    call cons
    mov r15, rax
.next:
    mov rdi, rbx
    call cdr_safe
    mov rbx, rax
    jmp .merge
.out:
    mov rax, r15
    pop rbp
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; fv_walk(rdi=form, rsi=bound, rdx=cut, rcx=acc) -> rax = acc'
global fv_walk
fv_walk:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov rbx, rdi                        ; form
    mov r12, rsi                          ; bound
    mov r13, rdx                            ; cut
    mov r14, rcx                              ; acc

    mov rdi, rbx
    call is_cons
    test rax, rax
    jnz .list

    ; --- atom: only a symbol can be a variable reference ---
    mov rdi, rbx
    call is_symbol_p
    test rax, rax
    jz .done

    mov rdi, rbx
    mov rsi, r12
    mov rdx, r13
    call member_sym_until
    test rax, rax
    jnz .done                           ; shadowed: bound by this lambda
    mov rdi, rbx
    mov rsi, r14
    call member_sym
    test rax, rax
    jnz .done                           ; already captured
    mov rdi, rbx
    mov rsi, r13
    call member_sym
    test rax, rax
    jnz .add                            ; bound by an enclosing lambda
    mov rdi, rbx
    mov rsi, [capture_scope]
    call frame_lookup
    cmp rax, FRAME_NOT_FOUND
    je .done                            ; a global — never captured
.add:
    mov rdi, rbx
    mov rsi, r14
    call cons
    mov r14, rax
    jmp .done

.list:
    mov rdi, rbx
    call car_safe
    mov r15, rax                        ; head

    mov rdi, r15
    mov rsi, kw_quote
    mov rdx, 5
    call sym_is
    test rax, rax
    jnz .done                           ; (QUOTE ...) — data, not code

    ; --- (LAMBDA params . body) ---
    mov rdi, r15
    mov rsi, kw_lambda
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_lambda
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov r10, rax                        ; params
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe                       ; body
    mov rsi, rax
    mov rdi, r10
    mov rdx, r12
    mov rcx, r13
    mov r8, r14
    mov r9, rbx                         ; memo key = this form
    call fv_lambda
    mov r14, rax
    jmp .done

.not_lambda:
    ; --- ($VAU (operands-param env-param) . body) — identical binding
    ; structure to LAMBDA (compile_vau literally rewrites it to one) ---
    mov rdi, r15
    mov rsi, kw_vau
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_vau
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov r10, rax
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe
    mov rsi, rax
    mov rdi, r10
    mov rdx, r12
    mov rcx, r13
    mov r8, r14
    xor r9, r9                          ; synthetic lambda at compile time
    call fv_lambda
    mov r14, rax
    jmp .done

.not_vau:
    ; --- (DEFMACRO name params . body) / (DEFEXPR name params . body) —
    ; spec Q2: a transformer body is a lambda body over its own params.
    mov rdi, r15
    mov rsi, kw_defmacro
    mov rdx, 8
    call sym_is
    test rax, rax
    jnz .macro_def
    mov rdi, r15
    mov rsi, kw_defexpr
    mov rdx, 7
    call sym_is
    test rax, rax
    jz .not_macro_def
.macro_def:
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov r10, rax                        ; params
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe
    mov rdi, rax
    call cdr_safe                       ; body
    mov rsi, rax
    mov rdi, r10
    mov rdx, r12
    mov rcx, r13
    mov r8, r14
    xor r9, r9
    call fv_lambda
    mov r14, rax
    jmp .done

.not_macro_def:
    ; --- (DEFINE name value) — spec Q2: fv(value). `name` is stored
    ; into the symbol's own global value cell (compile_define), never
    ; compiled as a variable reference, so it is not a use of whatever
    ; enclosing binding shares its spelling.
    mov rdi, r15
    mov rsi, kw_define
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_define
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov rdi, rax
    mov rsi, r12
    mov rdx, r13
    mov rcx, r14
    call fv_walk
    mov r14, rax
    jmp .done

.not_define:
    ; --- (LET ((name init)...) . body) — PARALLEL binding: every init
    ; is evaluated in the OUTER scope before any name is visible
    ; (compile_let's own contract). So `(LET ((X 1) (Y X)) ...)` inside
    ; a nested lambda refers to the *outer* X in Y's init and must
    ; capture it — spec section 1.4's named classic mistake.
    mov rdi, r15
    mov rsi, kw_let
    mov rdx, 3
    call sym_is
    test rax, rax
    jz .not_let
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov r15, rax                        ; bindings cursor
    push qword IMM_NIL                        ; [names]
.let_bind:
    mov rdi, r15
    call is_cons
    test rax, rax
    jz .let_body
    mov rdi, r15
    call car_safe
    mov rdi, rax
    call is_cons
    test rax, rax
    jz .let_name
    mov rdi, r15
    call car_safe
    mov rdi, rax
    call cdr_safe
    mov rdi, rax
    call car_safe                       ; init
    mov rdi, rax
    mov rsi, r12                          ; the OUTER bound set
    mov rdx, r13
    mov rcx, r14
    call fv_walk
    mov r14, rax
.let_name:
    mov rdi, r15
    call car_safe
    mov rdi, rax
    call is_cons
    test rax, rax
    jz .let_name_bare
    mov rdi, r15
    call car_safe
    mov rdi, rax
    call car_safe
    jmp .let_have_name
.let_name_bare:
    mov rdi, r15
    call car_safe
.let_have_name:
    mov rdi, rax
    mov rsi, [rsp]
    call cons
    mov [rsp], rax
    mov rdi, r15
    call cdr_safe
    mov r15, rax
    jmp .let_bind
.let_body:
    pop rdi                             ; names
    mov rsi, r12
    call append_lists
    push rax                            ; [new bound]
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe                       ; body
    mov rdi, rax
    pop rsi
    mov rdx, r13
    mov rcx, r14
    call fv_walk_list
    mov r14, rax
    jmp .done

.not_let:
    ; --- (LET* ((name init)...) . body) — SEQUENTIAL: each init sees
    ; every name bound before it, so `(LET* ((X 1) (Y X)))` must NOT
    ; capture an outer X.
    mov rdi, r15
    mov rsi, kw_let_star
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_let_star
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov r15, rax                        ; bindings cursor
    push r12                            ; [running bound set]
.ls_bind:
    mov rdi, r15
    call is_cons
    test rax, rax
    jz .ls_body
    mov rdi, r15
    call car_safe
    mov rdi, rax
    call is_cons
    test rax, rax
    jz .ls_name
    mov rdi, r15
    call car_safe
    mov rdi, rax
    call cdr_safe
    mov rdi, rax
    call car_safe                       ; init
    mov rdi, rax
    mov rsi, [rsp]                        ; everything bound so far
    mov rdx, r13
    mov rcx, r14
    call fv_walk
    mov r14, rax
    mov rdi, r15
    call car_safe
    mov rdi, rax
    call car_safe
    jmp .ls_have_name
.ls_name:
    mov rdi, r15
    call car_safe
.ls_have_name:
    mov rdi, rax
    mov rsi, [rsp]
    call cons
    mov [rsp], rax
    mov rdi, r15
    call cdr_safe
    mov r15, rax
    jmp .ls_bind
.ls_body:
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe                       ; body
    mov rdi, rax
    pop rsi                             ; running bound set
    mov rdx, r13
    mov rcx, r14
    call fv_walk_list
    mov r14, rax
    jmp .done

.not_let_star:
    ; --- (PROG (var...) item...) — vars bind over every item; an item
    ; that is a bare symbol is a LABEL (compile_prog), not a reference.
    mov rdi, r15
    mov rsi, kw_prog
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_prog
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov rdi, rax
    call param_names
    mov rdi, rax
    mov rsi, r12
    call append_lists
    push rax                            ; [new bound]
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe
    mov r15, rax                        ; items cursor
.prog_loop:
    mov rdi, r15
    call is_cons
    test rax, rax
    jz .prog_out
    mov rdi, r15
    call car_safe
    mov rdi, rax
    call is_symbol_p
    test rax, rax
    jnz .prog_next                      ; a label
    mov rdi, r15
    call car_safe
    mov rdi, rax
    mov rsi, [rsp]
    mov rdx, r13
    mov rcx, r14
    call fv_walk
    mov r14, rax
.prog_next:
    mov rdi, r15
    call cdr_safe
    mov r15, rax
    jmp .prog_loop
.prog_out:
    add rsp, 8
    jmp .done

.not_prog:
    ; --- (HANDLER-CASE protected (head (var) . handler-body)) — the
    ; clause head is never compiled (compile_handler_case ignores it,
    ; matching on one fixed tag); `var` binds over the handler body.
    mov rdi, r15
    mov rsi, kw_handler_case
    mov rdx, 12
    call sym_is
    test rax, rax
    jz .not_handler_case
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call car_safe                       ; protected form
    mov rdi, rax
    mov rsi, r12
    mov rdx, r13
    mov rcx, r14
    call fv_walk
    mov r14, rax
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov r15, rax                        ; clause
    mov rdi, r15
    call cdr_safe
    mov rdi, rax
    call car_safe                       ; (var) or NIL
    mov rdi, rax
    call param_names
    mov rdi, rax
    mov rsi, r12
    call append_lists
    push rax                            ; [new bound]
    mov rdi, r15
    call cdr_safe
    mov rdi, rax
    call cdr_safe                       ; handler body
    mov rdi, rax
    pop rsi
    mov rdx, r13
    mov rcx, r14
    call fv_walk_list
    mov r14, rax
    jmp .done

.not_handler_case:
    ; --- a macro call: the literal call syntax never mentions the free
    ; variables the EXPANSION references, so expand (once — through
    ; macroexpand_once, so scan and compile share one invocation) and
    ; walk that instead. Checked after every special form above, in the
    ; same order compile_form itself dispatches.
    mov rdi, r15
    call macro_slot_of
    cmp rax, IMM_NIL
    je .not_macro_call
    mov rsi, rax                        ; macro closure
    mov rdi, rbx
    call cdr_safe
    mov rdx, rax                          ; raw args
    mov rdi, rbx                            ; call form (memo key)
    call macroexpand_once
    mov rdi, rax
    mov rsi, r12
    mov rdx, r13
    mov rcx, r14
    call fv_walk
    mov r14, rax
    jmp .done

.not_macro_call:
    ; --- an operative call: spec open question Q1, answered "opaque".
    ; compile_form bakes an operative call's operands into a (QUOTE
    ; ...) literal — no operand is ever compiled as an expression, and
    ; the operative's own EVAL runs in the global environment
    ; (compile_vau's global_environment_sentinel), so no operand can
    ; ever name a captured local. Descending anyway (what
    ; scan_free_vars does) only over-captures.
    mov rdi, r15
    mov rsi, r12
    call operative_head_p
    test rax, rax
    jnz .done

    ; --- an ordinary application: head plus every argument ---
    mov rdi, r15
    mov rsi, r12
    mov rdx, r13
    mov rcx, r14
    call fv_walk
    mov r14, rax
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    mov rsi, r12
    mov rdx, r13
    mov rcx, r14
    call fv_walk_list
    mov r14, rax

.done:
    mov rax, r14
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; analyze_lambda_captures(rdi=lambda form, rsi=enclosing scope)
;   -> rax = this lambda's capture list (also memoized, along with one
;      entry for every nested LAMBDA form reachable from its body).
global analyze_lambda_captures
analyze_lambda_captures:
    push rbx
    push r12
    mov rbx, rdi
    mov rax, [capture_scope]
    push rax                            ; [saved capture_scope]
    mov [capture_scope], rsi

    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov rdi, rax
    call param_names
    mov r12, rax                        ; bound = this lambda's params,
                                         ; cut = IMM_NIL (nothing lexically
                                         ; encloses the root of an analysis;
                                         ; its free names are resolved
                                         ; against capture_scope instead)
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe                       ; body
    mov rdi, rax
    mov rsi, r12
    mov rdx, IMM_NIL
    mov rcx, IMM_NIL
    call fv_walk_list
    push rax
    mov rdi, rbx
    mov rsi, rax
    call capture_memo_store
    pop rax

    pop rcx
    mov [capture_scope], rcx
    pop r12
    pop rbx
    ret

; filter_resolvable(rdi=symbol list, rsi=scope) -> rax = the same list,
; order preserved, minus any symbol that does not resolve in `scope`.
; Belt-and-braces: compile_lambda's closure-construction copy loop emits
; one `mov rax,[rbp+disp]` per captured name and has no representation
; for "no disp", so this restores by construction the invariant
; scan_free_vars used to give for free (it only ever added names it had
; just resolved). The analysis should never produce an unresolvable
; name; if it somehow does — a lambda form physically shared between
; two different scopes, so that one memo entry serves both — dropping
; it here is the only representable answer, and CAPTURE_CHECK below
; catches the case in an assertion build.
filter_resolvable:
    push rbx
    push r12
    push r13
    mov rbx, rdi
    mov r12, rsi
    mov rdi, rbx
    call is_cons
    test rax, rax
    jz .nil
    mov rdi, rbx
    call car_safe
    mov r13, rax
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    mov rsi, r12
    call filter_resolvable
    push rax
    mov rdi, r13
    mov rsi, r12
    call frame_lookup
    cmp rax, FRAME_NOT_FOUND
    pop rax
    je .out
    mov rdi, r13
    mov rsi, rax
    call cons
    jmp .out
.nil:
    mov rax, IMM_NIL
.out:
    pop r13
    pop r12
    pop rbx
    ret

; lambda_capture_list(rdi=lambda form, rsi=enclosing scope) -> rax =
; the ordered capture list compile_lambda uses for BOTH its free-slot
; frame layout and its closure-construction copy loop. Memo hit for any
; lambda the enclosing analysis already saw; a fresh analysis rooted
; here otherwise — which is the right answer for the synthetic lambda
; forms this compiler builds with cons at compile time
; (compile_vau/compile_defmacro/compile_defexpr, compile_let's dynamic
; rewrite, compile_unwind_protect's cleanup thunk): rooting an analysis
; at one of those resolves its free names directly against the scope
; that is current right now, which is exactly avail() for it.
global lambda_capture_list
lambda_capture_list:
    push rbx
    push r12
    push r13
    mov rbx, rdi
    mov r12, rsi
    mov rax, [capture_scope]
    push rax                            ; [saved capture_scope]
    mov [capture_scope], r12
    mov rdi, rbx
    call capture_memo_lookup
    test rdx, rdx
    jnz .have
    mov rdi, rbx
    mov rsi, r12
    call analyze_lambda_captures
.have:
    mov rdi, rax
    mov rsi, r12
    call filter_resolvable
    mov r13, rax
%ifdef CAPTURE_CHECK
    mov rdi, rbx
    mov rsi, r12
    mov rdx, r13
    call capture_check_assert
%endif
    mov rax, r13
    pop rcx
    mov [capture_scope], rcx
    pop r13
    pop r12
    pop rbx
    ret

%ifdef CAPTURE_CHECK
; ---------------------------------------------------------------------
; -DCAPTURE_CHECK: the oracle build (spec section 1.4's last paragraph).
; After every capture list is computed, re-run the OLD shadowing-blind
; scan_free_vars over the same body and same scope and assert
;
;   (a) new is a subset of old  — the new analysis never captures
;       anything the old one would not have, so it can never introduce
;       a slot the copy loop cannot fill; and
;   (b) every symbol in old \ new is one the new analysis is entitled
;       to drop: a name bound somewhere *inside* the lambda (shadowed
;       over-capture, the whole point of the change), a DEFINE'd name,
;       a PROG label, a HANDLER-CASE clause head, or a name occurring
;       only inside an operative call's opaque operands.
;
; A violation is a trap (int3), not a message: this is a build you run
; the suite under, and the first violating compile is the one you want
; to be looking at in a debugger.

; dropped_ok_p(rdi=lambda form, rsi=symbol) -> rax=1/0 — an independent
; second implementation of "the new analysis was allowed to drop this",
; deliberately written as a flat occurrence scan rather than by reusing
; any part of fv_walk, so that a bug in fv_walk's bound-set bookkeeping
; cannot excuse itself.
dropped_ok_p:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi                        ; form
    mov r12, rsi                          ; symbol
    mov rdi, rbx
    call is_cons
    test rax, rax
    jz .no
    mov rdi, rbx
    call car_safe
    mov r13, rax                        ; head

    mov rdi, r13
    mov rsi, kw_quote
    mov rdx, 5
    call sym_is
    test rax, rax
    jnz .no

    ; binder positions
    mov rdi, r13
    mov rsi, kw_lambda
    mov rdx, 6
    call sym_is
    test rax, rax
    jnz .params_cadr
    mov rdi, r13
    mov rsi, kw_vau
    mov rdx, 4
    call sym_is
    test rax, rax
    jnz .params_cadr
    mov rdi, r13
    mov rsi, kw_defmacro
    mov rdx, 8
    call sym_is
    test rax, rax
    jnz .params_caddr
    mov rdi, r13
    mov rsi, kw_defexpr
    mov rdx, 7
    call sym_is
    test rax, rax
    jnz .params_caddr
    mov rdi, r13
    mov rsi, kw_define
    mov rdx, 6
    call sym_is
    test rax, rax
    jnz .define_name
    mov rdi, r13
    mov rsi, kw_let
    mov rdx, 3
    call sym_is
    test rax, rax
    jnz .let_names
    mov rdi, r13
    mov rsi, kw_let_star
    mov rdx, 4
    call sym_is
    test rax, rax
    jnz .let_names
    mov rdi, r13
    mov rsi, kw_prog
    mov rdx, 4
    call sym_is
    test rax, rax
    jnz .prog_names
    mov rdi, r13
    mov rsi, kw_handler_case
    mov rdx, 12
    call sym_is
    test rax, rax
    jnz .handler_names
    jmp .subforms

.params_cadr:
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov rdi, rax
    call param_names
    mov rdi, r12
    mov rsi, rax
    call member_sym
    test rax, rax
    jnz .yes
    jmp .subforms
.params_caddr:
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov rdi, rax
    call param_names
    mov rdi, r12
    mov rsi, rax
    call member_sym
    test rax, rax
    jnz .yes
    jmp .subforms
.define_name:
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call car_safe
    cmp rax, r12
    je .yes
    jmp .subforms
.let_names:
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov rdi, rax
    call binding_names
    mov rdi, r12
    mov rsi, rax
    call member_sym
    test rax, rax
    jnz .yes
    jmp .subforms
.prog_names:
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov rdi, rax
    call param_names
    mov rdi, r12
    mov rsi, rax
    call member_sym
    test rax, rax
    jnz .yes
    ; a bare-symbol item is a label
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe
    mov r14, rax
.prog_items:
    mov rdi, r14
    call is_cons
    test rax, rax
    jz .subforms
    mov rdi, r14
    call car_safe
    cmp rax, r12
    je .yes
    mov rdi, r14
    call cdr_safe
    mov r14, rax
    jmp .prog_items
.handler_names:
    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov r14, rax                        ; clause
    mov rdi, r14
    call car_safe
    cmp rax, r12
    je .yes                             ; the clause head, never compiled
    mov rdi, r14
    call cdr_safe
    mov rdi, rax
    call car_safe
    mov rdi, rax
    call param_names
    mov rdi, r12
    mov rsi, rax
    call member_sym
    test rax, rax
    jnz .yes
    jmp .subforms

.subforms:
    ; a macro call: the analysis saw the expansion, so judge that
    mov rdi, r13
    call macro_slot_of
    cmp rax, IMM_NIL
    je .not_macro
    mov rsi, rax
    mov rdi, rbx
    call cdr_safe
    mov rdx, rax
    mov rdi, rbx
    call macroexpand_once
    mov rdi, rax
    mov rsi, r12
    call dropped_ok_p
    jmp .out
.not_macro:
    ; an operative call: every operand is baked as QUOTE'd data, so any
    ; name occurring only under one is legitimately not captured
    mov rdi, r13
    mov rsi, IMM_NIL
    call operative_head_p
    test rax, rax
    jnz .yes
    mov r14, rbx
.walk:
    mov rdi, r14
    call is_cons
    test rax, rax
    jz .no
    mov rdi, r14
    call car_safe
    mov rdi, rax
    mov rsi, r12
    call dropped_ok_p
    test rax, rax
    jnz .yes
    mov rdi, r14
    call cdr_safe
    mov r14, rax
    jmp .walk
.yes:
    mov rax, 1
    jmp .out
.no:
    xor rax, rax
.out:
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; capture_check_assert(rdi=lambda form, rsi=scope, rdx=new capture list)
capture_check_assert:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov rbx, rdi                        ; lambda form
    mov r12, rsi                          ; scope
    mov r13, rdx                            ; new list

    mov rdi, rbx
    call cdr_safe
    mov rdi, rax
    call cdr_safe                       ; body
    mov rdi, rax
    mov rsi, r12
    mov rdx, IMM_NIL
    call scan_free_vars
    mov r14, rax                        ; old list

    ; (a) new subset old
    mov r15, r13
.subset:
    mov rdi, r15
    call is_cons
    test rax, rax
    jz .subset_ok
    mov rdi, r15
    call car_safe
    mov rdi, rax
    mov rsi, r14
    call member_sym
    test rax, rax
    jz .violation
    mov rdi, r15
    call cdr_safe
    mov r15, rax
    jmp .subset
.subset_ok:

    ; (b) every dropped name is one we are entitled to drop
    mov r15, r14
.dropped:
    mov rdi, r15
    call is_cons
    test rax, rax
    jz .out
    mov rdi, r15
    call car_safe
    push rax
    mov rdi, rax
    mov rsi, r13
    call member_sym
    test rax, rax
    jnz .dropped_next
    mov rdi, rbx
    mov rsi, [rsp]
    call dropped_ok_p
    test rax, rax
    jz .violation_pop
.dropped_next:
    add rsp, 8
    mov rdi, r15
    call cdr_safe
    mov r15, rax
    jmp .dropped

.violation_pop:
    add rsp, 8
.violation:
    int3
.out:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
%endif

; closure_nfree_tagged(rdi=tagged value) -> rax = fixnum count of the
; free variables a compiled closure (or $VAU operative — same layout)
; actually captured, or IMM_NIL for anything else. (CLOSURE-NFREE f) is
; a debug/introspection primitive in the same spirit as RECORD-BRAND
; and HASH-CODE: it reads one representation field, [raw+24], that
; nothing else in the language exposes. It exists to make the capture
; analysis above *testable* — over-capture is invisible to results by
; construction (an over-captured slot is simply never read), so without
; this a shadowing fix could only be observed as "the same answer as
; before", which is not a test.
global closure_nfree_tagged
closure_nfree_tagged:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .no
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_CLOSURE
    je .yes
    cmp qword [rax], HDR_OPERATIVE
    jne .no
.yes:
    mov rax, [rax+24]
    TO_FIXNUM rax
    ret
.no:
    mov rax, IMM_NIL
    ret


; --- primitive list/predicate ops: compiled calls into the kernel's own
; host routines (car/cdr/cons), reachable from Lamedh source for the
; first time. Same technique as compile_lambda's data_alloc call: the
; host routine's address is baked as a target immediate and invoked
; through an absolute-address indirect call, never a direct rel32 (the
; code heap and the host binary's .text are farther apart than a rel32
; call can reach).

; compile_nullary_hostcall(rsi=host fn address) — CLEAR-ALL-FLAGS's
; shape: no argument form to compile, target rax ends up holding
; whatever the host routine itself returns.
compile_nullary_hostcall:
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg
    ret

; emit_rc_store_cell(rsi = absolute address of a symbol's value/macro
; cell) — emits, in place of a bare `mov [cell], rax`, a call to
; rc_store_cell(rdi=cell, rsi=rax): decrement whatever the cell held,
; increment what replaces it, store, and hand the value back in rax so
; the form still evaluates to what it assigned.
;
; This is the M half of the counting discipline for global bindings:
; a symbol is a pinned heap object, so its value/macro/plist cells are
; heap slots, and rebinding a global is a genuine heap->heap reference
; replacement — the one that releases the previous value.
emit_rc_store_cell:
    push rbx
    mov rbx, rsi
    mov dil, REG_RSI
    mov sil, REG_RAX
    call emit_mov_rr                        ; target: rsi = value
    mov rsi, rbx
    mov dil, REG_RDI
    call emit_mov_reg_imm64                   ; target: rdi = &cell
    lea rsi, [rel rc_store_cell]
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg                          ; target: rax = value
    pop rbx
    ret

; emit_safepoint() — emits `push rax; mov rax, imm64(rc_safepoint);
; call rax; pop rax` at a compiled function's entry.
;
; The push/pop around it is what lets this be spliced in without
; knowing whether rax is still live from the calling convention (it
; carries nargs in); rc_safepoint itself preserves every other
; register, so the stub is invisible to the code on either side of it.
; A direct rel32 call is impossible here for the usual reason: the code
; heap and the host binary's .text are farther apart than rel32 reaches.
emit_safepoint:
    mov dil, REG_RAX
    call emit_push_reg
    lea rsi, [rel rc_safepoint]
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg
    mov dil, REG_RAX
    call emit_pop_reg
    ret

; compile_unary_hostcall(rdi=arg form, rsi=host fn address)
compile_unary_hostcall:
    push rbx
    mov rbx, rsi
    call compile_form                    ; arg -> target rax
    mov dil, REG_RDI
    mov sil, REG_RAX
    call emit_mov_rr                        ; target: rdi = arg
    mov rsi, rbx
    mov dil, REG_RAX
    call emit_mov_reg_imm64                    ; target: rax = host fn addr
    mov dil, REG_RAX
    call emit_call_reg                           ; target: call rax -> rax = result
    pop rbx
    ret

; compile_binary_hostcall(rdi=arg1 form, rsi=arg2 form, rdx=host fn address)
; Host convention rdi=arg1,rsi=arg2 (matches car/cons's own signatures).
compile_binary_hostcall:
    push rbx
    push r12
    mov rbx, rsi                          ; arg2 form
    mov r12, rdx                            ; host fn addr
    call compile_form                          ; arg1 -> rax
    mov dil, REG_RAX
    call emit_push_reg                            ; save arg1
    mov rdi, rbx
    call compile_form                                ; arg2 -> rax
    mov dil, REG_RSI
    mov sil, REG_RAX
    call emit_mov_rr                                    ; target: rsi = arg2
    mov dil, REG_RDI
    call emit_pop_reg                                      ; target: rdi = arg1
    mov rsi, r12
    mov dil, REG_RAX
    call emit_mov_reg_imm64                                   ; target: rax = host fn addr
    mov dil, REG_RAX
    call emit_call_reg                                          ; call -> rax = result
    pop r12
    pop rbx
    ret

; compile_ternary_hostcall(rdi=arg1 form, rsi=arg2 form, rdx=arg3 form,
; rcx=host fn address). Host convention rdi=arg1,rsi=arg2,rdx=arg3
; (matches array_set's own signature) — the same shape as
; compile_binary_hostcall, one argument deeper.
compile_ternary_hostcall:
    push rbx
    push r12
    push r13
    mov rbx, rsi                          ; arg2 form
    mov r12, rdx                            ; arg3 form
    mov r13, rcx                              ; host fn addr
    call compile_form                            ; arg1 -> rax
    mov dil, REG_RAX
    call emit_push_reg                              ; save arg1
    mov rdi, rbx
    call compile_form                                  ; arg2 -> rax
    mov dil, REG_RAX
    call emit_push_reg                                    ; save arg2
    mov rdi, r12
    call compile_form                                        ; arg3 -> rax
    mov dil, REG_RDX
    mov sil, REG_RAX
    call emit_mov_rr                                            ; target: rdx = arg3
    mov dil, REG_RSI
    call emit_pop_reg                                              ; target: rsi = arg2
    mov dil, REG_RDI
    call emit_pop_reg                                                ; target: rdi = arg1
    mov rsi, r13
    mov dil, REG_RAX
    call emit_mov_reg_imm64                                            ; target: rax = host fn addr
    mov dil, REG_RAX
    call emit_call_reg                                                   ; call -> rax = result
    pop r13
    pop r12
    pop rbx
    ret

; bool_from_al() — target: al (0/1 from a set*) -> rax (IMM_NIL/IMM_TRUE).
; Same conversion compile_binop's comparisons use.
bool_from_al:
    call emit_movzx_eax_al
    mov edi, 4
    call emit_imul_rax_imm32
    mov edi, IMM_NIL
    jmp emit_add_rax_imm32

; compile_eq(rdi=arg1 form, rsi=arg2 form) — lisp_eq (strings.asm):
; a raw tagged-value compare gets fixnums/characters/symbols/every
; immediate right for free (their tagged bits *are* their value), but
; is wrong for two separately heap-allocated strings or floats with
; identical content (KERNEL.md Part IV's "value equality" for those
; types) — an earlier version of this function was exactly that bare
; compare, silently nonconformant on both types the moment they were
; used as EQ operands rather than compared some other way.
compile_eq:
    lea rdx, [rel lisp_eq]
    jmp compile_binary_hostcall

; compile_atom(rdi=arg form) — true unless the value is a cons.
compile_atom:
    call compile_form
    mov edi, TAG_MASK
    call emit_and_rax_imm32
    mov rsi, TAG_CONS
    call emit_cmp_rax_imm64
    call emit_setne_al
    jmp bool_from_al

; compile_nullp(rdi=arg form) — true iff the value is NIL.
compile_nullp:
    call compile_form
    mov rsi, IMM_NIL
    call emit_cmp_rax_imm64
    call emit_sete_al
    jmp bool_from_al

; compile_print(rdi=arg form) — prints a fixnum, returns it (PRINT's
; own value, the way most Lisps' PRINT returns what it was given). The
; first primitive that lets *compiled* Lamedh code produce any output
; at all — every test before this called print_fixnum from the hand-
; written host driver, never from within a compiled program.
compile_print:
    call compile_form                     ; arg -> target rax
    mov dil, REG_RAX
    call emit_push_reg                       ; save arg (the result to
                                              ; return once printing is
                                              ; done)
    mov dil, REG_RDI
    mov sil, REG_RAX
    call emit_mov_rr                            ; target: rdi = arg
    lea rax, [rel print_value]                     ; dispatches on the
                                                    ; argument's runtime
                                                    ; tag: string bytes or
                                                    ; a fixnum's decimal
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64                        ; target: rax = &print_fixnum
    mov dil, REG_RAX
    call emit_call_reg                                ; call print_fixnum(rdi=arg)
    mov dil, REG_RAX
    call emit_pop_reg                                   ; rax = arg (restored)
    ret

; compile_newline() — 0-arg form; writes a newline, returns NIL.
compile_newline_form:
    lea rax, [rel print_newline]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg
    mov rsi, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    ret

; compile_binop_overflow_guard() — emits target code testing the hardware
; overflow flag (OF) left by the +/- instruction compile_binop just
; emitted, and calling set_overflow_flag (overflow.asm) when it's set:
;
;     jno .skip
;     push rax          ; result register — every GPR is caller-saved
;     mov rcx, set_overflow_flag
;     call rcx
;     pop rax
; .skip:
;
; The tagged (shifted-left-by-2) representation means +/- on it is
; bit-for-bit ordinary 64-bit two's-complement arithmetic on a value
; already multiplied by 4 (see overflow.asm's own header), so OF here
; already means exactly what KERNEL.md Part V's fixed-width model
; requires it to mean for this representation. `*` is not wired to this
; (see overflow.asm) — the same emitted-code technique would report the
; wrong condition there, not the right one with extra steps.
compile_binop_overflow_guard:
    push rbx
    call emit_jno                          ; target: jno rel32; rax=patch site
    mov rbx, rax
    mov dil, REG_RAX
    call emit_push_reg                       ; target: push rax
    lea rsi, [rel set_overflow_flag]
    mov dil, REG_RCX
    call emit_mov_reg_imm64                    ; target: rcx = set_overflow_flag
    mov dil, REG_RCX
    call emit_call_reg                           ; target: call rcx
    mov dil, REG_RAX
    call emit_pop_reg                              ; target: pop rax
    call codegen_here
    mov rdi, rbx
    mov rsi, rax
    call patch_rel32                                 ; .skip: lands here
    pop rbx
    ret

; emit_coerce_char_in_rax() — target: if rax currently holds a tagged
; Char, replaces it in place with a tagged fixnum holding that Char's
; code point; otherwise leaves rax exactly as given. KERNEL.md Part V:
; "A Char operand is unconditionally coerced to its code point" in
; +/-/*/</=' own contagion rule — compile_binop below calls this right
; after compiling each operand, while it is still fresh in target rax
; and before being relocated to rbx or the machine stack, so both
; operands of every compiled arithmetic/comparison op get the same
; treatment regardless of which one this is.
;
; A Char is a tagged immediate (chars.asm/tags.inc: enumeration index
; 256+code, tag bits 11), never overlapping a fixnum's own tag bits
; (00) or any other immediate's enumeration index (0..4) — so the
; check is: tag bits == TAG_IMMEDIATE, and the enumeration index falls
; in [256, 511]. Only rax-specific codegen helpers exist
; (emit_and_rax_imm32 and friends), which is exactly why this only
; ever operates on rax rather than an arbitrary target register —
; compile_binop's own call sites are placed accordingly.
emit_coerce_char_in_rax:
    push rbx
    push r12
    push r13
    push r14

    mov dil, REG_RCX
    mov sil, REG_RAX
    call emit_mov_rr                     ; target: rcx = rax (original)

    mov edi, TAG_MASK
    call emit_and_rax_imm32                ; target: rax &= TAG_MASK
    mov edi, TAG_IMMEDIATE
    call emit_sub_rax_imm32                  ; target: rax -= TAG_IMMEDIATE
    call emit_jne                              ; not an immediate -> skip
    mov rbx, rax                                 ; [site: not_immediate]

    mov dil, REG_RAX
    mov sil, REG_RCX
    call emit_mov_rr                               ; target: rax = rcx (original)
    mov dil, REG_RAX
    mov sil, 2
    call emit_sar_imm8                                ; target: rax = enum index
    mov edi, IMM_CHAR_BASE
    call emit_sub_rax_imm32                             ; target: rax -= 256

    mov rsi, 0
    call emit_cmp_rax_imm64                               ; target: cmp rax, 0
    call emit_jl                                            ; rax<0 -> skip (index was <256)
    mov r12, rax                                              ; [site: below_range]

    mov rsi, 255
    mov dil, REG_RDX
    call emit_mov_reg_imm64                                     ; target: rdx = 255
    mov dil, REG_RDX
    mov sil, REG_RAX
    call emit_cmp_rr                                              ; target: cmp rdx, rax
    call emit_jl                                                    ; 255<rax -> skip (index was >511)
    mov r13, rax                                                      ; [site: above_range]

    ; in range: rax already holds the raw code point (0..255) —
    ; tagging it as a fixnum is exactly a <<2, done here via *4 (no
    ; general left-shift emitter exists, only emit_sar_imm8's right
    ; shift; a small multiply is the same bit pattern for a value this
    ; small and produces the correct tag-00 result either way). This
    ; success path must jump clean over the restore stub below — every
    ; failure site left rax holding an intermediate scratch value
    ; (tag bits, or enum_index-256), never the original operand, so
    ; each of them needs rax explicitly restored from rcx before
    ; reaching the shared exit; the success path must not re-run that
    ; restore, or it would clobber the very fixnum it just computed.
    mov edi, 4
    call emit_imul_rax_imm32
    call emit_jmp32
    mov r14, rax                     ; [site: success, over the restore stub]

    call codegen_here
    mov rdi, rbx
    mov rsi, rax
    call patch_rel32                    ; not_immediate -> restore stub
    mov rdi, r12
    mov rsi, rax
    call patch_rel32                      ; below_range -> restore stub
    mov rdi, r13
    mov rsi, rax
    call patch_rel32                        ; above_range -> restore stub

    mov dil, REG_RAX
    mov sil, REG_RCX
    call emit_mov_rr                          ; restore stub: rax = rcx

    call codegen_here
    mov rdi, r14
    mov rsi, rax
    call patch_rel32                            ; success -> here (past
                                                 ; the restore stub)

    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_binop(rdi=lhs form, rsi=rhs form, dl='+'/'-'/'*'/'<'/'=' as ASCII)
; Compiles both operands (lhs pushed across rhs's own compilation, since
; rhs may itself contain calls that would otherwise clobber rax), then
; emits the fixnum-tagged operation. Leaves the result in rax.
compile_binop:
    push rbx
    push r12
    mov bl, dl                    ; remember which op
    mov r12, rsi                  ; save rhs form (rdi is about to change)

    call compile_form              ; rdi = lhs -> rax
    call emit_coerce_char_in_rax     ; Part V contagion: a Char lhs
                                      ; becomes its code point here
    mov dil, REG_RAX
    call emit_push_reg               ; push lhs

    mov rdi, r12
    call compile_form                 ; rhs -> rax
    call emit_coerce_char_in_rax        ; same contagion for rhs
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr                   ; rbx = rhs
    mov dil, REG_RAX
    call emit_pop_reg                    ; rax = lhs

    cmp bl, '+'
    jne .not_add
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_add_rr
    call compile_binop_overflow_guard
    jmp .done
.not_add:
    cmp bl, '-'
    jne .not_sub
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_sub_rr
    call compile_binop_overflow_guard
    jmp .done
.not_sub:
    cmp bl, '*'
    jne .not_mul
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_imul_rr
    mov dil, REG_RAX
    mov sil, 2
    call emit_sar_imm8                     ; correct the <<4 back to <<2
    jmp .done
.not_mul:
    cmp bl, '<'
    jne .not_lt
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_cmp_rr
    call emit_setl_al
    jmp .bool_from_al
.not_lt:
    ; '='
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_cmp_rr
    call emit_sete_al
.bool_from_al:
    call emit_movzx_eax_al
    mov edi, 4
    call emit_imul_rax_imm32
    mov edi, IMM_NIL
    call emit_add_rax_imm32
.done:
    pop r12
    pop rbx
    ret

; compile_progn(rdi = forms list, rsi = tail) — emits code evaluating
; each form in order; target rax holds the last one's value, or NIL for
; an empty list (KERNEL.md Part VI: "(progn) is NIL"). This is what a
; multi-form LAMBDA/LET/LET* body compiles through — compile_form's
; callers already preserve rbx/r12/r13/r14 across a nested compile_form
; call, so a plain host-side loop over the list is enough; no special
; backpatching is needed since nothing branches here.
;
; rsi=tail is forwarded to the LAST form only (PROGN is tail-transparent
; only in its final position, docs/spec-tco-capture-gc.md section 2):
; every earlier form is compiled non-tail, which is already the default
; since compile_form zeroes [tail_ctx] on entry — only the last form's
; compile_form call needs [tail_ctx] re-armed from the saved rsi first.
compile_progn:
    push rbx
    push r14
    push r15
    mov rbx, rdi
    mov r15, rsi                        ; saved tail flag
    cmp rbx, IMM_NIL
    jne .have
    mov rsi, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    jmp .out
.have:
.loop:
    mov rdi, rbx
    call cdr
    mov r14, rax                        ; next cursor: is THIS form last?
    cmp r14, IMM_NIL
    jne .not_last
    mov [tail_ctx], r15                 ; last form: forward tail-ness
.not_last:
    mov rdi, rbx
    call car
    mov rdi, rax
    call compile_form
    mov rbx, r14
    cmp rbx, IMM_NIL
    jne .loop
.out:
    pop r15
    pop r14
    pop rbx
    ret

; compile_cond(rdi = the full (COND clause...) form, rsi = tail)
;
; rsi=tail is forwarded only to the matched clause's own body (its last
; form, via compile_progn) — never to the test forms, per the tail-
; position table in docs/spec-tco-capture-gc.md section 2.
;
; Each clause is (test body...). Tests are tried in order; the first
; truthy one's body (compile_progn'd) becomes the result and every
; later clause is skipped. No clause matching -> NIL. Each clause's
; "test was NIL" branch is a backpatched forward jump to wherever the
; *next* clause starts being generated (unknown until then, same
; reasoning as compile_if); each clause's "body is done" jump instead
; targets the form's overall end, unknown until every clause has been
; compiled, so those end-jump sites accumulate in a host-side list
; (consed as compile-time data, not target code) and get patched in
; one pass once the real end address is known.
compile_cond:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov r15, rsi                        ; saved tail flag, forwarded only
                                         ; to the matched clause's own body
    mov rbx, rdi
    call cdr
    mov rbx, rax                      ; clauses cursor
    mov r12, IMM_NIL                    ; acc: end-jump patch sites
.loop:
    cmp rbx, IMM_NIL
    je .no_match
    mov rdi, rbx
    call car
    mov r14, rax                          ; clause
    mov rdi, r14
    call car
    mov rdi, rax
    call compile_form                         ; test -> target rax
    mov rsi, IMM_NIL
    call emit_cmp_rax_imm64
    call emit_je                                ; -> rax = skip-clause site
    push rax                                      ; [skip_site, ...]

    mov rdi, r14
    call cdr
    cmp rax, IMM_NIL
    je .no_body                                       ; "(test)" with no body:
                                                        ; the test's own value
                                                        ; (already in target
                                                        ; rax) stands
    mov rdi, rax
    mov rsi, r15
    call compile_progn                              ; body -> target rax
.no_body:
    call emit_jmp32                                    ; -> rax = end-jump site
    mov rdi, rax
    mov rsi, r12
    call cons
    mov r12, rax

    call codegen_here                                     ; next-clause label
    pop rdi                                                  ; skip_site
    mov rsi, rax
    call patch_rel32

    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .loop

.no_match:
    mov rsi, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64                                     ; rax = NIL

    call codegen_here                                              ; end label
    mov r13, rax                                                     ; end addr (rax gets clobbered below)
    mov rbx, r12
.patch_loop:
    cmp rbx, IMM_NIL
    je .out
    mov rdi, rbx
    call car
    mov rdi, rax
    mov rsi, r13
    call patch_rel32
    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .patch_loop
.out:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_and(rdi = the full (AND form...) form, rsi = tail)
; (AND) -> T. Otherwise forms are evaluated left to right; the first
; NIL short-circuits the rest with NIL as the result; if none is NIL,
; the last form's value is the result. Same end-jump-list-then-patch
; technique as compile_cond, one jump per short-circuiting form.
;
; rsi=tail is forwarded only to the LAST form (only it can be the whole
; AND's own value with no short-circuit test left to run afterward) —
; every earlier form is a test, never tail, per
; docs/spec-tco-capture-gc.md section 2.
compile_and:
    push rbx
    push r12
    push r13
    push r14
    mov r14, rsi                       ; saved tail flag
    mov rbx, rdi
    call cdr
    mov rbx, rax                     ; forms cursor
    mov r12, IMM_NIL                   ; acc: short-circuit end-jump sites
    cmp rbx, IMM_NIL
    jne .have
    mov rsi, IMM_TRUE
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    jmp .out
.have:
.loop:
    mov rdi, rbx
    call cdr
    mov r13, rax                       ; next cursor: is THIS form last?
    cmp r13, IMM_NIL
    jne .not_last
    mov [tail_ctx], r14                ; last form: forward tail-ness
.not_last:
    mov rdi, rbx
    call car
    mov rdi, rax
    call compile_form                    ; form -> target rax
    mov rbx, r13
    cmp rbx, IMM_NIL
    je .last_done                          ; last form: its value stands
    mov rsi, IMM_NIL
    call emit_cmp_rax_imm64
    call emit_je                             ; -> rax = end-jump site (NIL case)
    mov rdi, rax
    mov rsi, r12
    call cons
    mov r12, rax
    jmp .loop
.last_done:
    call codegen_here
    mov r13, rax
    mov rbx, r12
.patch_loop:
    cmp rbx, IMM_NIL
    je .out
    mov rdi, rbx
    call car
    mov rdi, rax
    mov rsi, r13
    call patch_rel32
    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .patch_loop
.out:
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_or(rdi = the full (OR form...) form, rsi = tail)
; (OR) -> NIL. Otherwise forms are evaluated left to right; the first
; non-NIL short-circuits the rest with that value as the result; if
; every form is NIL, the result is NIL (the last form's own NIL value,
; already correct with no extra work). Mirrors compile_and exactly,
; short-circuiting on "not NIL" (emit_jne) instead of "is NIL".
;
; rsi=tail: same last-form-only forwarding as compile_and.
compile_or:
    push rbx
    push r12
    push r13
    push r14
    mov r14, rsi                       ; saved tail flag
    mov rbx, rdi
    call cdr
    mov rbx, rax
    mov r12, IMM_NIL
    cmp rbx, IMM_NIL
    jne .have
    mov rsi, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    jmp .out
.have:
.loop:
    mov rdi, rbx
    call cdr
    mov r13, rax                       ; next cursor: is THIS form last?
    cmp r13, IMM_NIL
    jne .not_last
    mov [tail_ctx], r14                ; last form: forward tail-ness
.not_last:
    mov rdi, rbx
    call car
    mov rdi, rax
    call compile_form
    mov rbx, r13
    cmp rbx, IMM_NIL
    je .last_done
    mov rsi, IMM_NIL
    call emit_cmp_rax_imm64
    call emit_jne                            ; -> rax = end-jump site (non-NIL case)
    mov rdi, rax
    mov rsi, r12
    call cons
    mov r12, rax
    jmp .loop
.last_done:
    call codegen_here
    mov r13, rax
    mov rbx, r12
.patch_loop:
    cmp rbx, IMM_NIL
    je .out
    mov rdi, rbx
    call car
    mov rdi, rax
    mov rsi, r13
    call patch_rel32
    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .patch_loop
.out:
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; let_binding_names(rdi = ((name init) (name init) ...) bindings list)
; -> rax = (name name ...), order preserved. The name-only projection
; build_frame_from_list needs (it wants a plain symbol list, and a LET
; binding is a 2-element list, not a bare symbol).
let_binding_names:
    push rbx
    mov rbx, rdi
    cmp rbx, IMM_NIL
    jne .have
    mov rax, IMM_NIL
    jmp .out
.have:
    mov rdi, rbx
    call car
    mov rdi, rax
    call car                            ; name
    push rax
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call let_binding_names
    pop rdi
    mov rsi, rax
    call cons
.out:
    pop rbx
    ret

; compile_let(rdi = the full (LET ((name init)...) body...) form, rsi = tail)
;
; rsi=tail is forwarded only to the body's last form (via compile_progn)
; — never to any init, which always runs in the OUTER scope before this
; LET's own frame even exists (docs/spec-tco-capture-gc.md section 2).
; Saved across this whole function in rbp, the one general-purpose
; register the fast path below never otherwise touches (the
; .dynamic_rewrite path below has its own nested push rbp/pop rbp
; around its unrelated scratch use of rbp, which round-trips the saved
; tail flag back unchanged).
;
; Parallel binding: every init is compiled and evaluated in the OUTER
; scope, in order — none can see any other binding this same LET
; introduces — before any of them becomes visible. This works by
; building the LET's own (name . disp) frame *before* evaluating any
; init (via build_frame_from_list, disps starting right after however
; many rbp-relative slots are already reserved, tracked by
; current_frame_depth) but not *installing* it into current_scope
; until every init has been compiled; each init's computed value is
; stored into its slot via a lookup against that not-yet-installed
; frame directly (frame_lookup), not through current_scope.
;
; A LET shares its enclosing function's own stack frame (no push rbp
; of its own — it isn't a call): it reserves its own slots with a
; plain `sub rsp` at entry and releases them with `add rsp` at exit,
; nested cleanly inside whatever the enclosing function already
; reserved. This only works because every compile_XXX helper in this
; compiler leaves the target's rsp exactly as it found it across its
; own call (compile_binop's own push/pop of operands is the same
; discipline) — so by the time compile_form dispatches to compile_let,
; rsp is guaranteed back at its enclosing-function baseline, and it is
; again by the time compile_let returns.
compile_let:
    push rbx
    push r12
    push r13
    push r14
    push r15
    push rbp
    mov rbp, rsi                        ; saved tail flag
    mov rbx, rdi
    call cadr
    mov r12, rax                        ; bindings list (original head)
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call cdr
    mov r13, rax                          ; body forms list

    ; --- prescan: does any binding name carry the DEFDYNAMIC marker? ---
    ; A dynamic variable's whole point is that a *different*,
    ; separately-compiled function can read the same global while
    ; lexically inside this LET's dynamic extent (KERNEL.md Part VI) —
    ; 06-require.lisp's own *require-stack* is exactly this shape:
    ; $require-load's LET rebinds it, and $require-note-dependency, an
    ; entirely separate function, reads it as a plain global while that
    ; LET's dynamic extent is still active on the call stack. Ordinary
    ; lexical shadowing (the fast path below) only ever affects
    ; references written textually inside this LET's own body, which is
    ; silently wrong for that case, so a dynamic binding needs the
    ; genuinely different save-global/restore-global treatment in
    ; .dynamic_rewrite below instead.
    mov rbx, r12
.dynp_scan:
    cmp rbx, IMM_NIL
    je .dynp_scan_done
    mov rdi, rbx
    call car
    mov rdi, rax
    call car
    mov rdi, rax
    call is_symbol_dynamic
    test rax, rax
    jnz .dynamic_rewrite
    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .dynp_scan
.dynp_scan_done:

    mov rdi, r12
    call list_length
    mov r14, rax                              ; k = number of bindings

    mov eax, r14d
    imul eax, eax, 8
    mov edi, eax
    call emit_sub_rsp_imm32

    mov rdi, r12
    call let_binding_names
    mov rdi, rax
    mov rsi, [current_frame_depth]
    call build_frame_from_list
    push rax                                    ; [new_frame]

    mov rbx, r12                                  ; cursor over original bindings
.init_loop:
    cmp rbx, IMM_NIL
    je .inits_done
    mov rdi, rbx
    call car
    mov r15, rax                                    ; binding = (name init)
    mov rdi, r15
    call cadr
    mov rdi, rax
    call compile_form                                   ; init -> target rax
    mov rdi, r15
    call car                                              ; name
    mov rdi, rax
    mov rsi, [rsp]                                          ; new_frame
    call frame_lookup                                         ; -> disp
    mov esi, eax
    mov dil, REG_RAX
    call emit_store_local
    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .init_loop
.inits_done:
    pop r15                                            ; new_frame

    mov rdi, r15
    mov rsi, [current_scope]
    call append_lists
    mov r15, rax                                          ; new_scope

    mov rax, [current_scope]
    push rax                                                ; [old_scope]
    mov [current_scope], r15

    mov rax, [current_frame_depth]
    push rax                                                  ; [old_frame_depth, old_scope]
    add rax, r14
    mov [current_frame_depth], rax

    mov rdi, r13
    mov rsi, rbp                                                ; tail flag
    call compile_progn                                          ; body -> target rax

    pop rax
    mov [current_frame_depth], rax
    pop rax
    mov [current_scope], rax

    mov dil, REG_RSP
    mov eax, r14d
    imul eax, eax, 8
    mov esi, eax
    call emit_add_reg_imm32

    pop rbp
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; --- dynamic-variable rewrite (KERNEL.md Part VI) ---
;
; At least one binding name in this LET carries the DEFDYNAMIC marker.
; Rather than teach the lexical binding machinery above a second mode,
; this rewrites the whole LET into an equivalent form built entirely
; from existing special forms — the same "derive it, don't hand-roll
; new codegen" philosophy compile_unwind_protect's own synthetic
; (LAMBDA () cleanup...) already demonstrates, and UNWIND-PROTECT is
; exactly the primitive dynamic rebinding needs: restore-on-every-exit,
; including a THROW/ERROR passing through:
;
;   (LET ((tmp1 dyn1) (val1 init1) (tmp2 dyn2) (val2 init2) ...)
;     (UNWIND-PROTECT
;         (PROGN (SETQ dyn1 val1) (SETQ dyn2 val2) ... body...)
;       (SETQ dyn1 tmp1) (SETQ dyn2 tmp2) ...))
;
; tmp_i/val_i are fresh GENSYMs, ordinary LEXICAL bindings of the
; *outer* LET (never touching dyn_i itself, so a plain variable
; reference to dyn_i anywhere — including inside the UNWIND-PROTECT's
; cleanup closure, or in a wholly separate function called during this
; dynamic extent — still resolves to the real global slot, per
; compile_setq/the ordinary global-reference fallback). Evaluating
; every tmp_i (dyn_i's OLD value) and val_i (the new init) up front, in
; one outer LET, preserves ordinary LET's parallel-evaluation semantics
; even though the actual global writes happen sequentially afterward.
; v0 scope, narrower than the spec on purpose: every binding in a LET
; that has *any* dynamic binding is given this save/restore treatment,
; even one that isn't itself DEFDYNAMIC'd — mixing dynamic and
; ordinary lexical bindings in one LET is not yet distinguished (no
; reference stdlib file exercises this yet); LET* does not support
; dynamic bindings at all yet, only plain LET.
.dynamic_rewrite:
    push rbp
    sub rsp, 40                       ; [rsp+0]=outer_bindings acc
                                       ; [rsp+8]=setq_forms acc
                                       ; [rsp+16]=restore_forms acc
                                       ; [rsp+24]=this iteration's VAL_i
                                       ; [rsp+32]=this iteration's TMP_i
    mov qword [rsp+0], IMM_NIL
    mov qword [rsp+8], IMM_NIL
    mov qword [rsp+16], IMM_NIL

    mov rbx, r12                        ; cursor over original bindings
.dynr_loop:
    cmp rbx, IMM_NIL
    je .dynr_done
    mov rdi, rbx
    call car
    mov r12, rax                            ; binding = (name init)
    mov rdi, r12
    call car
    mov r14, rax                              ; name (dyn_i)
    mov rdi, r12
    call cdr
    mov rdi, rax
    call car
    mov r15, rax                                ; init

    call gensym
    mov [rsp+24], rax                             ; VAL_i
    call gensym
    mov [rsp+32], rax                               ; TMP_i

    ; pair1 = (TMP_i dyn_i) — captures the OLD global value
    mov rdi, [rsp+32]
    mov rsi, r14
    call build_list2
    mov r12, rax
    ; pair2 = (VAL_i init) — captures the NEW value
    mov rdi, [rsp+24]
    mov rsi, r15
    call build_list2
    mov rbp, rax

    ; outer_bindings = cons(pair1, cons(pair2, outer_bindings))
    mov rdi, rbp
    mov rsi, [rsp+0]
    call cons
    mov rdi, r12
    mov rsi, rax
    call cons
    mov [rsp+0], rax

    ; setq_forms += (SETQ dyn_i VAL_i)
    mov rdi, kw_setq
    mov rsi, 4
    call intern_symbol
    mov rbp, rax
    mov rdi, rbp
    mov rsi, r14
    mov rdx, [rsp+24]
    call build_list3
    mov rdi, rax
    mov rsi, [rsp+8]
    call cons
    mov [rsp+8], rax

    ; restore_forms += (SETQ dyn_i TMP_i)
    mov rdi, kw_setq
    mov rsi, 4
    call intern_symbol
    mov rbp, rax
    mov rdi, rbp
    mov rsi, r14
    mov rdx, [rsp+32]
    call build_list3
    mov rdi, rax
    mov rsi, [rsp+16]
    call cons
    mov [rsp+16], rax

    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .dynr_loop
.dynr_done:
    ; protected_form = (PROGN setq_forms... body...)
    mov rdi, [rsp+8]
    mov rsi, r13
    call append_lists
    mov rbp, rax
    mov rdi, kw_progn
    mov rsi, 5
    call intern_symbol
    mov rdi, rax
    mov rsi, rbp
    call cons
    mov rbp, rax                          ; protected_form

    ; unwind_protect_form = (UNWIND-PROTECT protected_form . restore_forms)
    mov rdi, rbp
    mov rsi, [rsp+16]
    call cons
    mov rbp, rax
    mov rdi, kw_unwind_protect
    mov rsi, 14
    call intern_symbol
    mov rdi, rax
    mov rsi, rbp
    call cons
    mov rbp, rax                          ; unwind_protect_form

    ; synthetic = (LET outer_bindings unwind_protect_form)
    mov rdi, rbp
    mov rsi, IMM_NIL
    call cons
    mov rbp, rax
    mov rdi, [rsp+0]
    mov rsi, rbp
    call cons
    mov rbp, rax
    mov rdi, kw_let
    mov rsi, 3
    call intern_symbol
    mov rdi, rax
    mov rsi, rbp
    call cons
    mov rbp, rax                          ; synthetic LET form

    add rsp, 40
    mov rdi, rbp
    pop rbp                                 ; restores the outer tail flag
                                             ; this function's own prologue
                                             ; saved in rbp — deliberately
                                             ; NOT forwarded into [tail_ctx]
                                             ; here: whether the rewritten
                                             ; synthetic LET's own last body
                                             ; form (an UNWIND-PROTECT,
                                             ; never tail regardless) sees
                                             ; it makes no codegen
                                             ; difference, so this path
                                             ; conservatively treats itself
                                             ; as non-tail rather than
                                             ; special-casing forwarding.
    call compile_form                       ; -> target rax = result

    pop rbp
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_let_star(rdi = the full (LET* ((name init)...) body...) form,
; rsi = tail)
;
; Sequential binding: each init sees every earlier binding of the same
; LET* (but not later ones). Structurally identical to compile_let
; except each binding's frame entry is installed into current_scope
; immediately after its own init is stored, one at a time, instead of
; building the whole frame up front and installing it only once every
; init has run.
;
; rsi=tail is forwarded only to the body's last form, same as
; compile_let, and saved the same way (rbp — every other
; general-purpose register is already committed to this function's own
; bookkeeping).
compile_let_star:
    push rbx
    push r12
    push r13
    push r14
    push r15
    push rbp
    mov rbp, rsi                        ; saved tail flag
    mov rbx, rdi
    call cadr
    mov r12, rax                        ; bindings list
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call cdr
    mov r13, rax                          ; body forms list

    mov rdi, r12
    call list_length
    mov r14, rax                              ; k

    mov eax, r14d
    imul eax, eax, 8
    mov edi, eax
    call emit_sub_rsp_imm32

    mov rax, [current_frame_depth]
    push rax                                    ; [old_frame_depth]
    mov rax, [current_scope]
    push rax                                      ; [old_scope, old_frame_depth]

    mov rbx, r12                                    ; cursor over bindings
.loop:
    cmp rbx, IMM_NIL
    je .bindings_done
    mov rdi, rbx
    call car
    mov r15, rax                                        ; binding = (name init)

    mov rdi, r15
    call cadr
    mov rdi, rax
    call compile_form                                        ; init -> target rax (sees every
                                                              ; earlier LET* binding already
                                                              ; installed below)

    mov rax, [current_frame_depth]
    mov esi, eax
    add esi, 1
    imul esi, esi, -8
    mov dil, REG_RAX
    call emit_store_local                                       ; slot at -8*(depth+1)

    mov rdi, r15
    call car                                                       ; name
    push rax
    mov rax, [current_frame_depth]
    add rax, 1
    imul rax, rax, -8
    mov rsi, rax
    pop rdi
    call cons                                                        ; (name . disp)
    mov rdi, rax
    mov rsi, [current_scope]
    call cons                                                          ; (pair . scope)
    mov [current_scope], rax

    mov rax, [current_frame_depth]
    inc rax
    mov [current_frame_depth], rax

    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .loop
.bindings_done:
    mov rdi, r13
    mov rsi, rbp                                                    ; tail flag
    call compile_progn                                              ; body -> target rax

    pop rax
    mov [current_scope], rax                                          ; [old_frame_depth]
    pop rax
    mov [current_frame_depth], rax

    mov dil, REG_RSP
    mov eax, r14d
    imul eax, eax, 8
    mov esi, eax
    call emit_add_reg_imm32

    pop rbp
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_if(rdi = the full (IF test then else) form, rsi = tail)
;
; Backpatched forward branches: the je/jmp targets aren't known until the
; then/else branches have themselves been compiled (their length depends
; on what's inside them), so each is emitted first against a placeholder
; rel32 and fixed up afterward with the same patch_rel32 primitive the
; runtime inline cache uses on already-executed code — compile-time
; backpatching and runtime self-modification are the same mechanism.
;
; rsi=tail is forwarded to BOTH branches (IF is tail-transparent in each,
; docs/spec-tco-capture-gc.md section 2) — never to the test.
compile_if:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov r15, rsi                     ; saved tail flag
    mov r14, rdi                     ; whole form
    call cadr
    mov r12, rax                        ; test form
    mov rdi, r14
    call caddr
    mov r13, rax                          ; then form
    mov rdi, r14
    call cadddr
    mov rbx, rax                           ; else form

    mov rdi, r12
    call compile_form                        ; test -> rax
    mov rsi, IMM_NIL
    call emit_cmp_rax_imm64
    call emit_je                               ; rax = je rel32 field addr
    mov r14, rax                                 ; (form no longer needed)

    mov [tail_ctx], r15
    mov rdi, r13
    call compile_form                              ; then branch
    call emit_jmp32                                  ; rax = jmp rel32 field addr
    mov r12, rax

    call codegen_here
    mov rdi, r14
    mov rsi, rax
    call patch_rel32                                    ; je -> else branch start

    mov [tail_ctx], r15
    mov rdi, rbx
    call compile_form                                     ; else branch

    call codegen_here
    mov rdi, r12
    mov rsi, rax
    call patch_rel32                                        ; jmp -> end

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_setq(rdi = the full (SETQ var1 val1 var2 val2 ...) form)
; Each pair is processed left to right: the val is compiled and
; evaluated, then stored into var's *first* lexically bound frame slot
; (frame_lookup against current_scope — this reaches an enclosing
; LET/LET*'s or a LAMBDA's own param/free slot, whichever is nearest),
; or into var's global value cell if it isn't lexically bound anywhere
; (the same absolute-address store DEFINE itself uses). This kernel
; has no dynamic-variable mechanism yet (no DEFDYNAMIC/DEFVAR), so
; that half of KERNEL.md Part VI's SETQ resolution doesn't apply here;
; nor does "create a fresh binding in the enclosing frame" for a name
; that's neither lexically bound nor previously DEFINE'd — such a name
; simply gets its (always-existing, symtab-reserved) global cell
; written, a narrower but still useful approximation. Leaves the last
; val's value in rax, matching the spec.
compile_setq:
    push rbx
    push r12
    mov rbx, rdi
    call cdr
    mov rbx, rax                    ; cursor over the flat var/val list
.loop:
    cmp rbx, IMM_NIL
    je .out
    mov rdi, rbx
    call car
    mov r12, rax                        ; var symbol
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call car                                ; val form
    mov rdi, rax
    call compile_form                          ; -> target rax = val

    mov rdi, r12
    mov rsi, [current_scope]
    call frame_lookup
    cmp rax, FRAME_NOT_FOUND
    je .global
    mov esi, eax
    mov dil, REG_RAX
    call emit_store_local
    jmp .next
.global:
    mov rdi, r12
    UNTAG_PTR rdi
    add rdi, 16
    mov rsi, rdi
    call emit_rc_store_cell
.next:
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call cdr
    mov rbx, rax
    jmp .loop
.out:
    pop r12
    pop rbx
    ret

; compile_define(rdi = the full (DEFINE name value-form) form)
; The name's global value cell lives at a fixed heap address decided at
; intern time, so storing into it compiles to one absolute-address store
; — no runtime name resolution at all. Leaves the defined value in rax.
compile_define:
    push rbx
    mov rbx, rdi
    call cadr                          ; name symbol
    push rax
    mov rdi, rbx
    call caddr                          ; value form
    mov rdi, rax
    call compile_form                     ; -> rax
    pop rdi                                ; name symbol
    UNTAG_PTR rdi
    add rdi, 16                              ; &value cell
    mov rsi, rdi
    call emit_rc_store_cell
    pop rbx
    ret

; compile_defdynamic(rdi = the full (DEFDYNAMIC name init-form
; [docstring]) form) — KERNEL.md Part VI's dynamic variables. Beyond
; DEFINE's own "store the value" (name unevaluated, init-form
; compiled/evaluated normally, docstring ignored — no plist storage
; for it yet, unlike DEF's), this marks the symbol dynamic
; (mark_symbol_dynamic, immediate host-side effect: visible to every
; later compile_let's own prescan in this same process, exactly the
; way DEFMACRO's own macro-slot install already is), which is what
; makes a *later* `(LET ((name ...)) ...)` rewrite into a genuine
; save/restore-the-global binding instead of ordinary lexical shadowing
; (see compile_let's own .dynamic_rewrite). Requires DEFDYNAMIC to run,
; as an ordinary top-level form, before any LET that rebinds the name —
; the same top-level-and-before-use discipline DEFMACRO already needs
; for its own compile-time visibility.
compile_defdynamic:
    push rbx
    mov rbx, rdi
    call cadr                          ; name symbol
    push rax
    mov rdi, rax
    call mark_symbol_dynamic
    mov rdi, rbx
    call caddr                          ; init form
    mov rdi, rax
    call compile_form                     ; -> rax
    pop rdi                                ; name symbol
    UNTAG_PTR rdi
    add rdi, 16                              ; &value cell
    mov rsi, rdi
    call emit_rc_store_cell
    pop rbx
    ret

; compile_lambda(rdi = (LAMBDA (params...) body) form) -> emits (a) the
; lambda's own native function, guarded by a jmp-over so the enclosing
; function's straight-line code never falls into it, and (b) runtime
; code, back in the enclosing function's own stream, that builds a fresh
; closure object every time this form actually executes. Leaves the
; tagged closure pointer in rax.
;
; v0 limits (see README roadmap): at most 3 params; a single body
; expression; free variables may only be captured from the *immediately*
; enclosing lambda's own frame, not further out.
global compile_lambda
compile_lambda:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov rbx, rdi

    ; --- capture analysis (docs/spec-tco-capture-gc.md section 1.2) ---
    ; One shadowing-aware answer, computed (or memo-read) exactly once,
    ; and kept on this function's own host stack across the whole body
    ; compile — so the frame-sizing pass below and the closure-
    ; construction copy loop at the very end read the identical list
    ; object rather than each re-deriving one and trusting the two
    ; derivations to agree (which is what the two scan_free_vars calls
    ; this replaces did, and what made every macro in the body run its
    ; transformer once per scan). Done before the jmp-over is emitted
    ; because the walk can expand macros, and a transformer that
    ; re-enters the compiler (EVAL) emits its own thunk into the code
    ; heap — which must land before this lambda's own entry point, not
    ; spliced into the middle of it.
    mov rdi, rbx
    mov rsi, [current_scope]
    call lambda_capture_list
    push rax                              ; [captures]

    mov rdi, rbx
    call cadr
    mov rdi, rax
    call split_rest_params
    mov r12, rax                        ; fixed params list (&REST stripped)
    mov [rest_sym_scratch], rdx           ; rest_sym, or IMM_NIL if none
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call cdr
    mov r13, rax                       ; body forms list (cddr of whole form;
                                        ; scan_free_vars walks a list of forms
                                        ; exactly like it walks a list of
                                        ; subforms already, so this needs no
                                        ; change there — only the final
                                        ; compile_form call below becomes
                                        ; compile_progn)

    call emit_jmp32
    push rax                              ; [jmp_over_site]
    call codegen_here
    mov r15, rax                            ; entry = this lambda's code pointer

    mov rdi, r12
    call build_param_frame
    push rax                                  ; [param_frame, jmp_over_site]

    mov rdi, r12
    call list_length
    mov r14, rax                                ; nparams

    mov r12, [rsp+16]                             ; free_syms (the capture
                                                   ; list computed at entry:
                                                   ; [param_frame,
                                                   ;  jmp_over_site,
                                                   ;  captures])

    mov rdi, r12
    call list_length
    mov rbx, rax                                    ; nfree

    ; free vars continue right after however many *local slots* params
    ; and REST together use. Without &REST that's simply min(nfixed,3)
    ; — only the first 3 params are register-spilled to a local slot;
    ; beyond that they live on the caller's stack and use none. With
    ; &REST it's always 4 (3 register-argument slots plus the REST slot
    ; itself), regardless of nfixed: compile_call_args always places
    ; global argument indices 0/1/2 in rsi/rdx/rcx no matter how many of
    ; them *this* callee treats as fixed parameters versus REST, so all
    ; 3 slots are reserved whenever REST exists, even when nfixed<3 (see
    ; the spill step and the register-fold step below). Computed once
    ; into nregslots_scratch and reused at every site that needs it, so
    ; free-var frame start, this function's own frame depth, and each
    ; free var's own slot index can never disagree with each other or
    ; with the REST slot's own fixed disp of -32. r15 already holds
    ; `entry` (needed later, for the closure's code pointer).
    mov rax, r14
    cmp qword [rest_sym_scratch], IMM_NIL
    jne .has_rest_nregslots
    cmp rax, 3
    jbe .nregslots_computed
    mov rax, 3
    jmp .nregslots_computed
.has_rest_nregslots:
    mov rax, 4
.nregslots_computed:
    mov [nregslots_scratch], rax
    mov rsi, rax
    mov rdi, r12
    call build_frame_from_list                        ; free_frame

    ; If this lambda has a &REST param, give it its own (symbol . disp)
    ; frame entry — disp is always -32 here: nregslots_scratch is always
    ; 4 whenever &REST exists (see above), so the REST slot always lands
    ; right after the 3 register-argument slots, at -32.
    mov r12, rax                                            ; free_frame (r12 is dead here: last read by build_frame_from_list above)
    mov rdi, [rest_sym_scratch]
    cmp rdi, IMM_NIL
    je .no_rest_frame
    mov rsi, -32
    call cons                                                 ; (rest_sym . -32)
    mov rdi, rax
    mov rsi, r12
    call cons                                                   ; (rest_pair . free_frame)
    jmp .have_combined_frame
.no_rest_frame:
    mov rax, r12
.have_combined_frame:
    mov rsi, rax                                                  ; combined = rest-entry? ++ free_frame
    mov rdi, [rsp]                                      ; param_frame (top of [param_frame, jmp_over_site])
    call append_lists
    push rax                                                ; [new_scope, param_frame, jmp_over_site]

    ; --- prologue ---
    call emit_push_rbp_frame
    mov rax, [nregslots_scratch]
    add rax, rbx
    mov [lambda_frame_depth_scratch], rax     ; stash base_index for
                                               ; current_frame_depth,
                                               ; installed right before
                                               ; the body compiles below
    imul eax, eax, 8
    mov edi, eax
    call emit_sub_rsp_imm32

    ; Spill up to 3 incoming params (rsi,rdx,rcx) into their slots.
    ; Whenever this lambda has a &REST param, all 3 are spilled
    ; unconditionally regardless of nfixed — not just up to r14 of
    ; them — because the calling convention always places global
    ; argument indices 0/1/2 in rsi/rdx/rcx, and for nfixed<3 some of
    ; those registers hold REST data rather than fixed-parameter data.
    ; Spilling a register the caller didn't actually set (nargs too
    ; small) is harmless: that slot is simply never read back, since
    ; the register-fold step below only reads a slot after checking the
    ; real nargs at runtime.
    mov r12, r14
    cmp qword [rest_sym_scratch], IMM_NIL
    je .spillcount_ok
    mov r12, 3
.spillcount_ok:
    cmp r12, 1
    jb .no_p0
    mov dil, REG_RSI
    mov esi, -8
    call emit_store_local
.no_p0:
    cmp r12, 2
    jb .no_p1
    mov dil, REG_RDX
    mov esi, -16
    call emit_store_local
.no_p1:
    cmp r12, 3
    jb .no_p2
    mov dil, REG_RCX
    mov esi, -24
    call emit_store_local
.no_p2:

    ; If &REST, stash nargs (still in rax — untouched by the register
    ; spills above, which only ever move rsi/rdx/rcx) into the REST
    ; slot itself: it's not read as the REST param until the loop below
    ; writes the real list there, so it's free scratch until then, and
    ; this is the last point before free-var extraction gets a chance to
    ; clobber rax.
    cmp qword [rest_sym_scratch], IMM_NIL
    je .no_rest_save
    mov dil, REG_RAX
    mov esi, -32
    call emit_store_local
.no_rest_save:

    ; extract captured free vars from the closure object (still in rdi)
    xor r12, r12                       ; i
.free_loop:
    cmp r12, rbx
    jae .free_done
    mov dil, REG_RAX
    mov sil, REG_RDI
    call emit_mov_rr                      ; rax = tagged closure
    mov edi, 0xFFFFFFFC
    call emit_and_rax_imm32                 ; rax = raw closure addr
    mov eax, r12d
    imul eax, eax, 8
    add eax, 32
    mov edx, eax
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_load_based                       ; rbx = *(raw+32+8*i)
    mov eax, [nregslots_scratch]                  ; where register/REST slots
                                                   ; end and free vars begin
                                                   ; (same value computed once,
                                                   ; above)
    add eax, r12d
    inc eax
    imul eax, eax, -8
    mov esi, eax
    mov dil, REG_RBX
    call emit_store_local
    inc r12
    jmp .free_loop
.free_done:

    ; --- &REST: build the rest-arg list from the stack-passed tail ---
    ; Walks from the last actual argument down to max(nfixed,3), consing
    ; each onto an accumulator, so the final list is in left-to-right
    ; order. Stack-resident args only exist for global index>=3 at all,
    ; so the lower bound is clamped there even when nfixed<3 — indices
    ; below 3 are register-resident and handled by the fold step below
    ; instead, not by this loop.
    ;
    ; The loop index lives in RDX, not RCX: emit_cmp_rax_imm64 loads its
    ; own immediate into RCX as scratch (see codegen.asm), so a loop
    ; index kept in RCX gets silently destroyed the first time the
    ; loop-continuation test runs.
    cmp qword [rest_sym_scratch], IMM_NIL
    je .no_rest_loop

    mov dil, REG_RDX
    mov esi, -32
    call emit_load_local                            ; target: rdx = nargs (stashed above)
    mov dil, REG_RDX
    mov esi, -4
    call emit_add_reg_imm32                           ; target: rdx = nargs-4 (index of the last stack arg)
    mov rsi, IMM_NIL
    mov dil, REG_RBX
    call emit_mov_reg_imm64                             ; target: rbx = acc = NIL

    call codegen_here
    push rax                                              ; [loop_start]

    mov dil, REG_RAX
    mov sil, REG_RDX
    call emit_mov_rr                                        ; target: rax = rdx (index)
    mov rsi, r14
    cmp rsi, 3
    jae .stack_thresh_ok
    mov rsi, 3
.stack_thresh_ok:
    sub rsi, 3                                                ; max(nfixed,3)-3, a compile-time
                                                               ; constant, always >=0
    call emit_cmp_rax_imm64                                     ; target: cmp rax, that (clobbers rcx)
    call emit_jl                                                  ; target: jl -> done (rax = rel32 field addr)
    push rax                                                        ; [jl_site, loop_start]

    mov dil, REG_RAX
    mov sil, REG_RDX
    call emit_load_stack_arg                                          ; target: rax = stack_arg[rdx]
    mov dil, REG_RDI
    mov sil, REG_RAX
    call emit_mov_rr                                                    ; target: rdi = arg value
    mov dil, REG_RSI
    mov sil, REG_RBX
    call emit_mov_rr                                                      ; target: rsi = acc
    lea rax, [rel cons]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64                                                 ; target: rax = &cons
    mov dil, REG_RAX
    call emit_call_reg                                                        ; target: call rax -> rax = new pair
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr                                                           ; target: acc = new pair
    mov dil, REG_RDX
    mov esi, -1
    call emit_add_reg_imm32                                                      ; target: rdx -= 1

    call emit_jmp32                                                                ; target: jmp -> loop_start
    mov rdi, rax
    mov rsi, [rsp+8]                                                                 ; loop_start
    call patch_rel32

    call codegen_here                                                                  ; done:
    mov rdi, [rsp]                                                                       ; jl_site
    mov rsi, rax
    call patch_rel32
    add rsp, 16                                                                            ; discard jl_site,loop_start

    ; --- &REST, nfixed<3: fold register-resident "extra" args (global
    ; index in [nfixed,2]) onto the front of acc ---
    ; Regardless of nfixed, compile_call_args always places global
    ; argument indices 0/1/2 in rsi/rdx/rcx (compile_call), unconditionally
    ; spilled above into local slots -8/-16/-24 once any &REST param
    ; exists. When nfixed<3, some of those indices are REST elements,
    ; not fixed parameters — fold them onto the front of whatever the
    ; stack loop above built, processing from the highest index down so
    ; each cons lands in the right place. Each slot's presence is a
    ; runtime fact (nargs > i) checked independently, since unlike the
    ; stack loop's single running index these three don't form one
    ; contiguous runtime-counted range. A no-op loop (nfixed>=3) when
    ; this lambda predates &REST's nfixed<3 support.
    mov r12, 2
.fold_loop:
    cmp r12, r14
    jl .fold_done
    mov dil, REG_RAX
    mov esi, -32
    call emit_load_local                     ; target: rax = nargs (still stashed)
    mov rsi, r12
    inc rsi                                    ; i+1, a compile-time constant
    call emit_cmp_rax_imm64                      ; target: cmp rax, i+1 (clobbers rcx)
    call emit_jl                                   ; target: jl -> skip (nargs<=i: slot i absent)
    push rax                                         ; [skip_site]

    mov eax, r12d
    inc eax
    imul eax, eax, -8
    mov esi, eax
    mov dil, REG_RDI
    call emit_load_local                                 ; target: rdi = slot[i]
    mov dil, REG_RSI
    mov sil, REG_RBX
    call emit_mov_rr                                       ; target: rsi = acc
    lea rax, [rel cons]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64                                  ; target: rax = &cons
    mov dil, REG_RAX
    call emit_call_reg                                         ; target: call rax -> rax = new pair
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr                                            ; target: acc = new pair

    call codegen_here                                            ; skip:
    pop rdi                                                        ; skip_site
    mov rsi, rax
    call patch_rel32

    dec r12
    jmp .fold_loop
.fold_done:

    mov dil, REG_RBX
    mov esi, -32
    call emit_store_local                                                                   ; target: REST slot = acc
.no_rest_loop:

    ; The prologue is complete: every incoming value is now a tagged
    ; word in a frame slot or an argument register, which is exactly
    ; the condition the conservative root scan needs. This is the
    ; collector's safe point (gc.asm).
    call emit_safepoint

    ; compile the body with the new scope AND a fresh current_frame_depth
    ; installed (this lambda's own base_index, stashed above — a LET/
    ; LET* inside the body allocates its own slots starting right after
    ; this function's own params/frees/REST slot, never colliding with
    ; an *enclosing* function's LET nesting, which is exactly why the
    ; old value must be saved and restored around this, same as
    ; current_scope just above it).
    mov rax, [current_scope]
    push rax                                    ; [old_scope, new_scope, param_frame, jmp_over_site]
    mov rax, [rsp+8]
    mov [current_scope], rax

    mov rax, [current_frame_depth]
    push rax                                      ; [old_frame_depth, old_scope, new_scope, param_frame, jmp_over_site]
    mov rax, [lambda_frame_depth_scratch]
    mov [current_frame_depth], rax

    mov rax, [current_prog_ctx]
    push rax                                        ; [old_prog_ctx, old_frame_depth, ...]
    mov qword [current_prog_ctx], 0                    ; a nested LAMBDA body
                                                        ; starts with no active
                                                        ; PROG of its own — see
                                                        ; current_prog_ctx's own
                                                        ; comment on why GO/
                                                        ; RETURN mustn't reach
                                                        ; through this boundary

    ; The body's own last form is always in tail position (it's a
    ; function's own return, unconditionally, regardless of whatever
    ; called compile_lambda's caller) — rsi=1, a literal, not forwarded
    ; from anywhere. current_lambda_depth brackets the whole body so
    ; nothing below (docs/spec-tco-capture-gc.md section 2's tail-call
    ; site codegen, a later landing-plan step) ever emits a tail jump
    ; while compiling something that isn't actually inside some
    ; function body — compile_thunk's own top-level forms, notably,
    ; leave this at 0.
    inc qword [current_lambda_depth]
    mov rdi, r13
    mov rsi, 1
    call compile_progn
    dec qword [current_lambda_depth]

    pop rax
    mov [current_prog_ctx], rax
    pop rax
    mov [current_frame_depth], rax
    pop rax
    mov [current_scope], rax                       ; [new_scope, param_frame, jmp_over_site]

    call emit_leave
    call emit_ret

    call codegen_here
    mov rdi, [rsp+16]                                ; jmp_over_site
    mov rsi, rax
    call patch_rel32
    add rsp, 24                                        ; discard new_scope,param_frame,jmp_over_site

    ; --- runtime closure construction (back in the enclosing stream) ---
    mov eax, ebx
    imul eax, eax, 8
    add eax, 32
    mov rsi, rax
    mov dil, REG_RDI
    call emit_mov_reg_imm64                       ; target: rdi = alloc size

    ; The code heap (mmap'd high in the address space) and the host
    ; binary's own .text (a low static address) are farther apart than a
    ; rel32 call can reach, so a call to a host runtime function is
    ; always an absolute-address indirect call, never a direct rel32 —
    ; unlike calls between two JIT-compiled functions, which always share
    ; the same 16MB code heap and so are always in rel32 range.
    lea rax, [rel data_alloc]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg                               ; target: call rax -> rax = raw addr

    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr                                  ; target: rbx = raw closure addr

    mov rsi, HDR_CLOSURE
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov edx, 0
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_store_based

    mov rsi, r15
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov edx, 8
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_store_based

    mov rsi, r14
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov edx, 16
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_store_based

    mov rsi, rbx
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov edx, 24
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_store_based

    ; Copy each free var's *current* value, from the enclosing scope,
    ; into the closure's captured array — walking the very same list
    ; object the frame layout above was built from ([rsp] now, after
    ; the new_scope/param_frame/jmp_over_site triple was discarded), so
    ; slot i here is by construction the same name as slot i there.
    mov r12, [rsp]                                      ; free_syms cursor
    xor r13, r13                                           ; i
.copy_loop:
    cmp r12, IMM_NIL
    je .copy_done
    mov rdi, r12
    call car
    push rax                                                  ; sym (host stack)
    mov rdi, rax
    mov rsi, [current_scope]
    call frame_lookup                                            ; rax = enclosing disp
    mov esi, eax
    mov dil, REG_RAX
    call emit_load_local                                            ; target: rax = *(rbp+disp)
    mov eax, r13d
    imul eax, eax, 8
    add eax, 32
    mov edx, eax
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_store_based                                             ; target: *(rbx+disp) = rax
    add rsp, 8                                                            ; discard saved sym
    inc r13
    mov rdi, r12
    call cdr
    mov r12, rax
    jmp .copy_loop
.copy_done:

    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_mov_rr
    mov edi, TAG_HEAPOBJ
    call emit_or_rax_imm32                       ; target: rax = tagged closure — final result

    ; The closure's captured slots now hold heap->heap references —
    ; count them. One emitted hostcall per closure *creation* (not per
    ; call), and rc_register returns its argument, so the tagged
    ; closure is still in rax afterwards and this splices in without
    ; disturbing the sequence around it.
    mov dil, REG_RDI
    mov sil, REG_RAX
    call emit_mov_rr
    lea rsi, [rel rc_register]
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg

    add rsp, 8                                     ; discard captures
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; invoke_macro(rdi=tagged closure, rsi=raw args list) -> rax = result.
; Calls an *already-compiled* Lamedh closure directly from host code,
; synchronously, right now, with the call site's raw unevaluated
; operand forms as arguments — not by emitting target instructions.
; This works because a compiled closure and the compiler itself are
; both just x86-64 machine code in the same process; there is no
; barrier between "host" and "target" beyond which code is calling
; which. It is the entire mechanism a macro transformer needs: the
; transformer is an ordinary compiled closure, and expanding a macro
; call means invoking it now instead of emitting a call to it.
;
; Supersedes an earlier raw_args_to_regs+invoke_closure_host pair that
; only ever forwarded the first 3 operand forms (silently dropping the
; rest) and did not pass the real argument count at all — a real,
; previously-latent bug lib/prelude.lisp's own DEFUN
; ((NAME PARAMS &REST BODY), nfixed=2) exposed, since a &REST-taking
; transformer's prologue (compile_lambda) reads the incoming nargs (in
; target rax, per the ordinary compiled-code calling convention every
; compile_call site also honors) to decide which register-argument
; slots hold real REST data. This version handles any number of
; call-site operands (capped at MAX_MACRO_ARGS, matching this
; project's own "generous fixed size, not an unbounded general answer"
; v0 sizing elsewhere — see README): the first 3 go in rsi/rdx/rcx as
; before, and any beyond that are pushed onto the *real* host stack
; immediately before the call, in the same order compile_call_args'
; own target-code convention produces (operand index 3 ends up closest
; to the return address, index 4 next, and so on), so a transformer
; with more than 3 fixed/REST parameters sees its stack-passed operands
; at exactly the offsets build_param_frame already expects — the exact
; layout an ordinary compiled call site would produce, just assembled
; by hand here instead of emitted.
%define MAX_MACRO_ARGS 32
global invoke_macro
invoke_macro:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov r12, rdi                    ; closure
    mov r13, rsi                      ; args list cursor

    sub rsp, MAX_MACRO_ARGS*8           ; scratch array, host-stack-resident
    mov r14, rsp                          ; scratch array base
    xor r15, r15                            ; count so far
.collect:
    cmp r13, IMM_NIL
    je .collected
    cmp r15, MAX_MACRO_ARGS
    jae .collected
    mov rdi, r13
    call car
    mov [r14+r15*8], rax
    mov rdi, r13
    call cdr
    mov r13, rax
    inc r15
    jmp .collect
.collected:
    ; r15 = n, capped. Registers first.
    xor rsi, rsi
    xor rdx, rdx
    xor rcx, rcx
    cmp r15, 1
    jb .regs_done
    mov rsi, [r14+0]
    cmp r15, 2
    jb .regs_done
    mov rdx, [r14+8]
    cmp r15, 3
    jb .regs_done
    mov rcx, [r14+16]
.regs_done:

    ; Extra operands (index 3..n-1), pushed highest-index first so
    ; index 3 ends up topmost — closest to the return address `call`
    ; is about to push, matching [rbp+16] once the callee's own
    ; `push rbp` lands.
    mov rbx, r15
    dec rbx
.push_extra:
    cmp rbx, 3
    jl .extra_done
    push qword [r14+rbx*8]
    dec rbx
    jmp .push_extra
.extra_done:

    mov rbx, r12
    UNTAG_PTR rbx
    mov rbx, [rbx+8]                   ; code_ptr
    mov rdi, r12                         ; tagged closure (self) — a
                                          ; transformer's own prologue
                                          ; extracts captured free vars
                                          ; from this, same as any
                                          ; other compiled closure
    mov rax, r15                           ; nargs
    call rbx                                 ; -> rax = result

    mov rbx, r15
    sub rbx, 3
    jle .no_extra_cleanup
    lea rsp, [rsp + rbx*8]                     ; discard the extra
                                                ; operands this call
                                                ; itself pushed
.no_extra_cleanup:
    add rsp, MAX_MACRO_ARGS*8                    ; discard scratch array

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_vau(rdi = the full ($VAU (operands-param env-param) body...)
; form) — Kernel-style vau (John Shutt's vau-calculus), the one
; primitive KERNEL.md's own `defvau` needs beneath it (already
; present, unmodified, in the reference's own lib/00-core.lisp, which
; this host already loads — see README Roadmap): identical to LAMBDA
; (params is exactly the fixed 2-element `(operands-param env-param)`
; list `defvau` always builds; body compiles unchanged) except the
; resulting closure is retagged HDR_OPERATIVE instead of HDR_CLOSURE at
; the moment it's built — see tags.inc's own comment for why an
; operative can otherwise be a completely ordinary LAMBDA-built closure
; under the hood. compile_form's own operative-call check (the
; ".not_macro_call" dispatch, further down) is the only other place
; that cares about the distinction: it decides whether a call site
; evaluates its operands normally or bakes them as a raw QUOTE'd list
; plus a placeholder "caller's environment" value — exactly the way
; DEFMACRO's own macro slot already decides between an ordinary
; application and a macro expansion. v0 scope, narrower than the spec
; on purpose: an operative is only recognized at a literal call site
; `(name arg...)` whose head is a global already bound to one at
; compile time (the same top-level-and-before-use discipline DEFMACRO
; needs, and the same "not lexically shadowed" gap DEFMACRO also has —
; see README) — not via APPLY/FUNCALL, and not as a first-class value
; passed around and called indirectly; "the caller's environment" is a
; fixed placeholder value (global_environment_sentinel below), since
; this kernel has no first-class environments — EVAL's own existing
; 2-argument form already tolerates this for free (compile_unary_
; hostcall only ever compiles EVAL's first operand, silently ignoring
; a second one, so `(eval form e)` already evaluates `form` in the one
; global environment this kernel has, exactly like 1-argument EVAL
; always did) — meaning an operative body's `(eval x e)` on a call-site
; *lexical* silently evaluates against the wrong (global) scope rather
; than erroring; see README for which reference stdlib usages this
; does and doesn't affect.
compile_vau:
    push rbx
    mov rbx, rdi
    call cdr
    mov rbx, rax                        ; (params body...) — already
                                         ; exactly LAMBDA's own cdr shape
    mov rdi, kw_lambda
    mov rsi, 6
    call intern_symbol
    mov rdi, rax
    mov rsi, rbx
    call cons                             ; (LAMBDA params body...)
    mov rdi, rax
    call compile_lambda                     ; target rax = tagged closure
                                             ; (HDR_CLOSURE)

    ; --- retag the freshly built closure as HDR_OPERATIVE ---
    mov dil, REG_RCX
    mov sil, REG_RAX
    call emit_mov_rr                          ; target: rcx = rax (save
                                               ; tagged ptr)

    mov edi, 0xFFFFFFFC
    call emit_and_rax_imm32                     ; target: rax = raw addr
                                                 ; (untagged)

    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr                              ; target: rbx = raw
                                                   ; addr (store base)

    mov rsi, HDR_OPERATIVE
    mov dil, REG_RAX
    call emit_mov_reg_imm64                         ; target: rax =
                                                     ; HDR_OPERATIVE

    mov edx, 0
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_store_based                             ; target:
                                                       ; [rbx+0] = rax

    mov dil, REG_RAX
    mov sil, REG_RCX
    call emit_mov_rr                                    ; target: rax =
                                                         ; rcx (restore
                                                         ; tagged ptr)

    pop rbx
    ret

; global_environment_sentinel() -> rax = a dedicated interned symbol,
; the placeholder "caller's environment" value an operative's own
; env-param is bound to at a call site (compile_form's operative-call
; check, below) — this kernel has exactly one environment (global), so
; there is nothing more meaningful to pass; see compile_vau's own
; comment.
kw_global_environment_sentinel: db "LAMEDH-ASM-GLOBAL-ENVIRONMENT"
kw_global_environment_sentinel_len: equ $ - kw_global_environment_sentinel
global_environment_sentinel:
    mov rdi, kw_global_environment_sentinel
    mov rsi, kw_global_environment_sentinel_len
    call intern_symbol
    ret

; compile_defexpr(rdi = the full (DEFEXPR name (params) body...) form)
; — Lisp 1.5's FEXPR (KERNEL.md/the reference's own SpecialForm::
; Defexpr, evaluator/special_forms.rs): like DEFMACRO, a FEXPR receives
; its call's raw, unevaluated argument list — but unlike a macro, that
; is the WHOLE story: there is no separate expansion step recompiled
; in the call's place, the FEXPR's own body runs directly and ITS
; return value is the call's result. `params` is always a single
; symbol (`(defexpr select (args) ...)` — never the 2-element
; `(operands-param env-param)` shape `$VAU`/`DEFVAU` always use), bound
; to the raw argument list; the body typically calls 1-argument
; `(eval ...)` explicitly on pieces of it to get values.
;
; This host has no separate FEXPR representation at all: a FEXPR is
; simply an Operative (compile_vau/HDR_OPERATIVE) built with a second,
; auto-appended, never-referenced GENSYM parameter — DEFEXPR is sugar
; over `$VAU`, not a new kernel mechanism, needing no change to the
; operative-call check or emit_check_callable. This works because a
; FEXPR body only ever calls 1-argument EVAL, and this kernel's EVAL
; already ignores any second operand unconditionally (compile_unary_
; hostcall only ever compiles the first) — exactly the same "no
; first-class environments, EVAL always evaluates in the one global
; environment this kernel has" v0 divergence $VAU's own comment
; already documents; a FEXPR calling `(eval x)` on something that
; resolves through a *caller-local* lexical, the way the reference's
; own 1-argument EVAL (which evaluates in the caller's own environment,
; not a fixed global one) would get right, is silently wrong here for
; the same reason. Multiple body forms (and an optional leading
; docstring, evaluated and discarded like any other non-final PROGN
; form) are supported, matching DEFMACRO's own fix above.
compile_defexpr:
    push rbx
    push r12
    push r13
    mov rbx, rdi
    call cadr
    mov r12, rax                    ; name
    mov rdi, rbx
    call caddr
    mov r13, rax                      ; params, e.g. (ARGS)
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call cdr
    mov rdi, rax
    call cdr
    push rax                            ; [body forms list]

    call gensym
    mov rdi, rax
    mov rsi, IMM_NIL
    call cons                             ; (fresh-unused-env-param)
    mov rdi, r13
    mov rsi, rax
    call append_lists                       ; params2 = (ARGS fresh-env-param)

    mov rdi, rax
    pop rsi                                   ; body
    call cons                                   ; (params2 . body)
    mov rdi, r12
    mov rsi, rax
    call cons                                     ; (name params2 . body) —
                                                   ; head discarded by
                                                   ; compile_vau's own cdr
    mov rdi, rax
    call compile_vau                                ; target rax = tagged
                                                     ; operative

    mov rdi, r12
    UNTAG_PTR rdi
    add rdi, 16
    mov rsi, rdi
    call emit_rc_store_cell                           ; symbol.value =
                                                       ; operative (same
                                                       ; store idiom
                                                       ; compile_define
                                                       ; uses)
    pop r13
    pop r12
    pop rbx
    ret

; compile_defmacro(rdi = (DEFMACRO name (params) body) form)
; A macro transformer is compiled exactly like a LAMBDA — the name is
; simply skipped, giving a synthetic (LAMBDA params body) built with
; `cons` — and the resulting closure-construction code is emitted into
; whichever function is currently open (same as DEFINE: running that
; code is what registers the macro). It is stored in the name symbol's
; macro slot, not its ordinary value cell, so an application and a macro
; use of the same name can never be confused.
global compile_defmacro
compile_defmacro:
    push rbx
    push r12
    mov rbx, rdi
    call cadr
    mov r12, rax                      ; name symbol
    mov rdi, rbx
    call caddr
    push rax                            ; [params]
    ; body = every form after params — not just the first one — the
    ; same "multiple body forms, implicitly PROGN-wrapped" support
    ; compile_lambda's own body already has. A single-form-only body
    ; silently discarded everything past a leading docstring: the
    ; reference's own lib/02-cxr.lisp defines its `defcxr` macro with
    ; exactly that shape (a docstring, then the real backquote
    ; template), so `cadddr` alone returned only the docstring text as
    ; the "expansion" every time, and the actual DEFUN template that
    ; builds CADR/CADDR/etc. never ran at all — CADR ended up
    ; permanently unbound, not merely wrong.
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call cdr
    mov rdi, rax
    call cdr
    mov rsi, rax                              ; body forms list
    mov rdi, [rsp]                              ; params
    call cons                                     ; (params body-forms...)
    add rsp, 8                                      ; [ ]

    push rax                                       ; save (params body) across intern_symbol's own args

    mov rdi, kw_lambda
    mov rsi, 6
    call intern_symbol                               ; rax = LAMBDA symbol
    mov rdi, rax
    pop rsi                                            ; (params body) restored
    call cons                                            ; synth = (LAMBDA params body)

    mov rdi, rax
    call compile_lambda                                    ; emits closure-construction code (target)

    mov rax, r12
    UNTAG_PTR rax
    add rax, 24                                              ; macro slot address
    mov rsi, rax
    call emit_rc_store_cell                                    ; target: symbol.macro = closure

    pop r12
    pop rbx
    ret

; compile_call_args(rdi=args list) -> emits target code that evaluates
; and pushes every argument (no count limit short of available stack —
; tested up to 32); rax = count pushed.
;
; Evaluated right-to-left (recurse to the end of the list before
; evaluating the head), which is what makes the resulting stack layout
; come out right without a second pass: after this returns, the target
; stack (top to bottom) holds arg0, arg1, arg2, arg3, ... argN-1 — the
; first 3 are exactly what compile_call's callers pop into rsi/rdx/rcx,
; and whatever remains below them is already in the exact layout a
; callee's stack-passed params expect (see build_param_frame): arg3 at
; [rbp+16], arg4 at [rbp+24], and so on, once the callee's own `call`
; pushes a return address on top. This is lamedh-asm's own convention
; (there being no C ABI to honor) — right-to-left evaluation order is
; simply what a hybrid register+stack layout falls out of cleanly; a
; real Lisp would document this as a visible evaluation-order choice,
; the same way classic cdecl's own right-to-left argument evaluation is
; a side effect of its stack layout, not an accident.
compile_call_args:
    push rbx
    mov rbx, rdi
    cmp rbx, IMM_NIL
    je .base
    push rbx
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call compile_call_args                 ; recurse first: evaluates
                                            ; every later arg before this
                                            ; one
    pop rbx
    push rax                                 ; save the rest's count
    mov rdi, rbx
    call car
    mov rdi, rax
    call compile_form                          ; this arg -> target rax
    mov dil, REG_RAX
    call emit_push_reg                            ; push it (topmost, since
                                                   ; evaluated/pushed last)
    pop rax
    inc rax
    jmp .out
.base:
    xor rax, rax
.out:
    pop rbx
    ret

; compile_record_new(rdi = args list [brand-form, field-form1...N]) —
; RECORD-NEW, the reference's own runtime constructor for a StructObj
; (HDR_RECORD, tags.inc): every DEFRECORD/DEFVARIANT-generated
; constructor compiles through this (`(defun ,ctor ,argnames
; (record-new ',ctor ,@argnames))`). Field count is a compile-time
; constant here (the args list's own length), so this unrolls
; entirely at compile time — no target-level loop — the same
; technique compile_lambda's own free-variable-copying loop uses, one
; level simpler since there is no re-derivation step needed.
;
; Evaluation order: fields are compiled via compile_call_args (right-
; to-left, landing on the target stack with field0 topmost — its own
; existing, already-tested ordering, reused unchanged), then the
; brand is compiled and pushed on top of that — so after both steps
; the target stack (top to bottom) holds brand, field0, field1, ...,
; fieldN-1, exactly the pop order the allocation code below wants.
compile_record_new:
    push rbx
    push r12
    push r13
    mov rbx, rdi                  ; args list: (brand-form field-form...)

    mov rdi, rbx
    call cdr
    mov rdi, rax
    call compile_call_args          ; pushes field0..fieldN-1 (field0
                                     ; topmost); rax = nfields
    mov r12, rax                      ; nfields (compile-time constant
                                       ; from here on)

    mov rdi, rbx
    call car
    mov rdi, rax
    call compile_form                   ; brand -> target rax
    mov dil, REG_RAX
    call emit_push_reg                    ; push brand (now topmost)

    ; --- allocate: 24 + nfields*8 bytes ---
    mov eax, r12d
    imul eax, eax, 8
    add eax, 24
    mov rsi, rax
    mov dil, REG_RDI
    call emit_mov_reg_imm64                 ; target: rdi = alloc size
    lea rax, [rel data_alloc]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg                        ; target: call rax ->
                                               ; rax = raw addr
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr                            ; target: rbx = raw
                                                 ; addr (store base)

    mov rsi, HDR_RECORD
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov edx, 0
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_store_based                         ; [rbx+0] = HDR_RECORD

    mov dil, REG_RAX
    call emit_pop_reg                               ; target: rax =
                                                     ; brand (popped)
    mov edx, 8
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_store_based                             ; [rbx+8] = brand

    mov rsi, r12
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov edx, 16
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_store_based                             ; [rbx+16] = nfields

    xor r13, r13                    ; host-side (compile-time) unroll
                                     ; index, not a target register
.floop:
    cmp r13, r12
    jae .fdone
    mov dil, REG_RAX
    call emit_pop_reg                 ; target: rax = field[r13]
                                       ; (popped in forward order,
                                       ; since field0 was topmost)
    mov eax, r13d
    imul eax, eax, 8
    add eax, 24
    mov edx, eax
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_store_based               ; [rbx+24+i*8] = rax
    inc r13
    jmp .floop
.fdone:

    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_mov_rr
    mov edi, TAG_HEAPOBJ
    call emit_or_rax_imm32              ; target: rax = tagged record —
                                         ; final result

    ; The closure's captured slots now hold heap->heap references —
    ; count them. One emitted hostcall per closure *creation* (not per
    ; call), and rc_register returns its argument, so the tagged
    ; closure is still in rax afterwards and this splices in without
    ; disturbing the sequence around it.
    mov dil, REG_RDI
    mov sil, REG_RAX
    call emit_mov_rr
    lea rsi, [rel rc_register]
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg

    pop r13
    pop r12
    pop rbx
    ret

; emit_check_callable() — target: rax holds a tagged value about to be
; treated as a closure and called. Verifies it is actually a
; HDR_CLOSURE (or HDR_OPERATIVE — a $VAU/DEFVAU operative shares the
; exact same [8]=code-ptr layout and is equally callable; a call
; site's own different treatment, baking QUOTE'd operands instead of
; evaluating them, already happened at compile time before this check
; ever runs, so from here on an operative is invoked exactly like an
; ordinary closure) heapobj; if neither, calls fail_wrong_type (native_errors.asm)
; instead of letting the caller's own subsequent `and rax,~TAG_MASK` +
; dereference run on whatever address an unbound global (IMM_NIL) or
; other non-closure value happens to produce — previously a near-NULL
; dereference (a segfault), since IMM_NIL's tag bits mask to a null
; pointer. KERNEL.md Part VIII lists calling a non-callable value among
; the native-failure classes a host must signal for; this now goes
; through the same real CATCH/HANDLER-CASE/ERRORSET-signaling machinery
; CAR/CDR's own wrong-type check already uses (native_throw), not the
; separate hard exit(1) an earlier version of this routine had —
; catchable by an enclosing HANDLER-CASE/ERRORSET, and an uncaught one
; traps (int3) the same way any other unmatched THROW does.
;
; On success, leaves target rax holding exactly the tagged value it was
; given: every register used as scratch (rdi, rbx) is restored, so a
; caller can insert one `call emit_check_callable` right after loading
; a value it's about to call, with no other change to its own logic.
; Does not touch rsi/rdx/rcx, which both call sites that use this have
; live forwarded-argument values in.
emit_check_callable:
    mov dil, REG_RDI
    mov sil, REG_RAX
    call emit_mov_rr                       ; rdi = rax (save tagged value)

    mov edi, TAG_MASK
    call emit_and_rax_imm32                ; rax &= TAG_MASK
    mov edi, TAG_HEAPOBJ
    call emit_sub_rax_imm32                ; rax -= TAG_HEAPOBJ (ZF iff a match)
    call emit_jne                          ; -> fail
    push rax                                 ; [tag_fail_site]

    mov dil, REG_RAX
    mov sil, REG_RDI
    call emit_mov_rr                          ; rax = rdi (tagged value again)
    mov edi, 0xFFFFFFFC
    call emit_and_rax_imm32                   ; rax = raw pointer
    mov dil, REG_RBX
    mov sil, REG_RAX
    mov edx, 0
    call emit_load_based                        ; rbx = header word
    mov rsi, HDR_CLOSURE
    mov dil, REG_RAX
    call emit_mov_reg_imm64                       ; rax = HDR_CLOSURE
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_cmp_rr                                ; cmp rbx, rax
    call emit_je                                      ; -> restore (ordinary closure)
    push rax                                            ; [hdr_ok1_site, tag_fail_site]

    mov rsi, HDR_OPERATIVE
    mov dil, REG_RAX
    call emit_mov_reg_imm64                               ; rax = HDR_OPERATIVE
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_cmp_rr                                        ; cmp rbx, rax
    call emit_jne                                             ; -> fail (neither)
    push rax                                                    ; [hdr_fail_site, hdr_ok1_site, tag_fail_site]

    ; both the HDR_CLOSURE early-je above and this HDR_OPERATIVE match
    ; falling through converge here — restore *must* run on both paths
    ; (the je above must not land past it, straight into "success" with
    ; rax still holding the HDR_CLOSURE comparison constant instead of
    ; the original tagged pointer — an earlier draft of this patch had
    ; exactly that bug), so hdr_ok1_site is patched right here rather
    ; than deferred to the shared "success" site below.
    call codegen_here                                           ; restore:
    mov rdi, [rsp+8]                                              ; hdr_ok1_site
    mov rsi, rax
    call patch_rel32                                                ; hdr_ok1_site -> restore

    mov dil, REG_RAX
    mov sil, REG_RDI
    call emit_mov_rr                                      ; rax = rdi (restore tagged value)
    call emit_jmp32                                         ; -> success
    push rax                                                  ; [ok_site, hdr_fail_site, hdr_ok1_site, tag_fail_site]

    call codegen_here                                           ; fail:
    push rax                                                      ; [fail_addr, ok_site, hdr_fail_site, hdr_ok1_site, tag_fail_site]
    ; patch_rel32 clobbers rax internally (lea rax,[rdi+4]), so
    ; fail_addr must be reloaded from memory for the second call
    ; rather than trusted to survive in a register across the first.
    mov rdi, [rsp+16]
    mov rsi, [rsp]
    call patch_rel32                                              ; hdr_fail_site -> fail
    mov rdi, [rsp+32]
    mov rsi, [rsp]
    call patch_rel32                                                ; tag_fail_site -> fail
    add rsp, 8                                                        ; discard fail_addr
                                                                       ; [ok_site, hdr_fail_site, hdr_ok1_site, tag_fail_site]

    ; fail: target rdi still holds the original tagged culprit value —
    ; the very first instruction this routine emitted was "rdi = rax"
    ; and nothing on either failing path (a bare rax compare/subtract)
    ; ever touches rdi again — so calling fail_wrong_type here (the same
    ; real CATCH/HANDLER-CASE/ERRORSET-signaling routine CAR/CDR already
    ; use, native_errors.asm) needs only two more immediate loads before
    ; the call: target rsi/rdx = the fixed message text this v0 uses,
    ; matching car_err_msg/cdr_err_msg's own fixed-message scope
    ; (reader.asm) rather than the reference's interpolated text.
    mov rsi, not_callable_err_msg
    mov dil, REG_RSI
    call emit_mov_reg_imm64
    mov rsi, not_callable_err_msg_len
    mov dil, REG_RDX
    call emit_mov_reg_imm64

    lea rax, [rel fail_wrong_type]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg                                                ; never returns

    call codegen_here                                                   ; success:
    mov rdi, [rsp]
    mov rsi, rax
    call patch_rel32                                                      ; ok_site -> success
    add rsp, 32                                                             ; discard [ok_site, hdr_fail_site, hdr_ok1_site, tag_fail_site]
    ret

; emit_ic_trampoline(rdi=cell_addr, rsi=mode, rdx=field_addr) -> rax =
; trampoline_entry (a target code address). rdx is meaningful only when
; mode=1; mode=0 callers may pass anything (compile_call's mode=0 call
; site passes junk in rdx, matching its own "unused" status there).
;
; Originally factored out of compile_call's named-global-call path
; (docs/spec-tco-capture-gc.md section 2.6 step 2) with no behavior
; change; step 4 then added the mode=1 variant below. Emits, at the
; current codegen position: a forward jmp (so ordinary fallthrough
; control flow at runtime skips the trampoline body — the same "jmp
; over co-located data/code" trick compile_lambda's own thunk header
; uses), the trampoline body itself (resolve cell_addr's current
; value, patch the ORIGINAL call site's rel32 operand to target the
; resolved code directly, then transfer control there), and patches
; the forward jmp to land just past the body. The caller still owns
; emitting the call site itself and patching it to target the returned
; trampoline_entry — this only builds the trampoline, once,
; immediately after the jmp-over site.
;
; mode=0: an ordinary, non-tail call site, always reached via `call` —
; the trampoline body discovers the call site's own rel32 field
; address at RUNTIME by reading the return address off the stack (the
; return address sitting on the stack at trampoline entry is always
; exactly four bytes past that field, since this trampoline is only
; ever reached via a `call` at that exact site).
;
; mode=1: a tail call site, reached via `jmp` instead of `call` — the
; "return address" sitting on the stack at entry belongs to some
; unrelated, already-in-progress call higher up, not to this call
; site, so deriving field_addr from it would silently corrupt that
; unrelated call site's own patch target (a delayed, wrong-answer bug
; surfacing only on that other call site's NEXT invocation — the
; failure mode this whole mode split exists to avoid). mode=1 instead
; bakes rdx (the call site's own field address, already known at
; compile time — the caller emitted the jmp instruction and got this
; back from emit_jmp32 before ever calling this routine) as an
; immediate, the same technique emit_install_catch_frame already uses
; for catch-frame resume addresses.
emit_ic_trampoline:
    push r12
    push r13
    push r14
    push r15
    mov r14, rdi                              ; cell_addr
    mov r13, rsi                              ; mode
    mov r15, rdx                              ; field_addr (mode=1 only)

    call emit_jmp32
    mov r12, rax                              ; jmp_over_site

    call codegen_here
    push rax                                    ; [trampoline_entry]

    ; rax (nargs, set by the caller just before this call) must survive
    ; the trampoline's own heavy use of rax as scratch, the same way
    ; arg0/closure below must survive being repurposed as patch_rel32's
    ; arguments — save it first, restore it last.
    mov dil, REG_RAX
    call emit_push_reg                              ; save nargs

    ; mode 0 (an ordinary, non-tail call site, always reached via
    ; `call`): the call site's own field_addr is discovered at RUNTIME
    ; from the return address already sitting on the stack. mode 1 (a
    ; tail call site, reached via `jmp` — the stack holds some
    ; unrelated caller's return address instead) instead uses
    ; field_addr as a compile-time-known immediate, baked in by
    ; whoever built this trampoline (the same technique
    ; emit_install_catch_frame already uses for resume addresses).
    cmp r13, 1
    je .baked_field_addr
    mov dil, REG_RAX
    mov esi, 8                                        ; return address is now
    call emit_load_rsp_disp8                            ; one slot deeper, under
                                                         ; the nargs we just saved
    mov edi, 4
    call emit_sub_rax_imm32                            ; rax = field_addr
    jmp .have_field_addr
.baked_field_addr:
    mov dil, REG_RAX
    mov rsi, r15
    call emit_mov_reg_imm64                            ; rax = field_addr (baked)
.have_field_addr:

    mov dil, REG_RAX
    call emit_push_reg                                    ; save field_addr

    mov dil, REG_RAX
    mov rsi, r14
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    mov sil, REG_RAX
    call emit_load_local_zero_disp                            ; rax = tagged closure (global's value)

    call emit_check_callable                                   ; die cleanly if it isn't one

    mov dil, REG_RAX
    call emit_push_reg                                          ; save tagged closure

    mov edi, 0xFFFFFFFC
    call emit_and_rax_imm32
    mov dil, REG_RBX
    mov sil, REG_RAX
    mov edx, 8
    call emit_load_based                                          ; rbx = code_ptr

    ; rsi holds arg0 (the callee's real first argument) and is about to
    ; be repurposed as patch_rel32's target argument — save it first, or
    ; the callee ends up receiving a code pointer where it expected its
    ; own parameter. (Same reasoning already covered the closure value,
    ; saved/restored via rdi below.) rdx/rcx (arg1/arg2) are untouched by
    ; patch_rel32 and need no such protection.
    mov dil, REG_RSI
    call emit_push_reg                                                ; save arg0

    mov dil, REG_RSI
    mov sil, REG_RBX
    call emit_mov_rr                                                ; rsi = target (patch_rel32 arg2)
    mov dil, REG_RDI
    mov esi, 16                                                       ; field_addr, now one push deeper
    call emit_load_rsp_disp8                                          ; rdi = field_addr (unpopped)

    ; same reach limit as the data_alloc call in compile_lambda — an
    ; absolute indirect call through a free target register (rax; the
    ; args for patch_rel32 already sit in rdi/rsi and must not move).
    lea rax, [rel patch_rel32]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg

    mov dil, REG_RSI
    call emit_pop_reg                                                   ; rsi = arg0 (restored)
    mov dil, REG_RDI
    call emit_pop_reg                                                   ; rdi = tagged closure (restored)
    mov dil, REG_RAX
    call emit_pop_reg                                                     ; discard field_addr
    mov dil, REG_RAX
    call emit_pop_reg                                                     ; rax = nargs (restored,
                                                                           ; overwriting the just-
                                                                           ; discarded field_addr)

    mov dil, REG_RBX
    call emit_jmp_reg                                                       ; tail-jump into the resolved callee

    call codegen_here
    mov rdi, r12
    mov rsi, rax
    call patch_rel32                                                          ; jmp-over -> here

    pop rax                                     ; trampoline_entry (return value)
    pop r15
    pop r14
    pop r13
    pop r12
    ret

; compile_call(rdi=operator form, rsi=args list, rdx=tail)
; A general application (f arg...). If f is a symbol that is not locally
; bound (i.e. a genuine global), the call site is compiled through a
; per-site, self-patching inline-cache trampoline: the first invocation
; resolves the symbol's current value, rewrites the *original* call
; site's rel32 in place to target the resolved code directly, and only
; then jumps there — every later call from that exact site is a plain
; direct call, no indirection, no re-resolution. Anything else (a local
; variable holding a closure, a literal LAMBDA) goes through one indirect
; call via the closure's stored code pointer.
; rdx=tail (docs/spec-tco-capture-gc.md section 2.6): when this call is
; itself in tail position, current_lambda_depth is nonzero (never
; tail-jump while compiling a top-level thunk), and nargs<=3 (v0
; scope), BOTH paths below emit a genuine tail call — `leave` +
; `emit_jmp_reg` for the indirect path, a `jmp`-based call site with a
; mode=1 baked-address trampoline for the named-global path — reusing
; this function's own frame instead of growing the native stack.
; Anything not meeting all three conditions takes the unchanged
; ordinary call/indirect-call path.
global compile_call
compile_call:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov rbx, rdi                    ; operator form
    mov r12, rsi                      ; args
    mov r15, rdx                      ; tail flag, live through both paths
                                       ; below (docs/spec-tco-capture-
                                       ; gc.md section 2.6 steps 3-4)

    mov rax, rbx
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .indirect_path
    mov rax, rbx
    UNTAG_PTR rax
    cmp qword [rax], HDR_SYMBOL
    jne .indirect_path
    mov rdi, rbx
    mov rsi, [current_scope]
    call frame_lookup
    cmp rax, FRAME_NOT_FOUND
    jne .indirect_path                 ; locally bound — not a global call

    ; --- named global call: inline-cached, self-patching ---
    mov rdi, r12
    call compile_call_args
    mov r13, rax                          ; nargs

    mov rax, rbx
    UNTAG_PTR rax
    add rax, 16
    mov r14, rax                            ; cell_addr

    ; compile_call_args pushes right-to-left, so arg0 ends up topmost —
    ; pop ascending (arg0 first) to match. Anything past the 3rd stays
    ; on the stack, already positioned exactly where the callee's
    ; stack-passed params expect it (build_param_frame).
    cmp r13, 1
    jb .n_after_a0
    mov dil, REG_RSI
    call emit_pop_reg
.n_after_a0:
    cmp r13, 2
    jb .n_after_a1
    mov dil, REG_RDX
    call emit_pop_reg
.n_after_a1:
    cmp r13, 3
    jb .n_after_a2
    mov dil, REG_RCX
    call emit_pop_reg
.n_after_a2:

    ; rax = actual arg count, for a &REST-taking callee to know how many
    ; stack-passed args past its fixed params actually exist (see
    ; compile_lambda). Every call sets this, whether or not the callee
    ; happens to want it — a callee that doesn't just ignores it.
    mov rsi, r13
    mov dil, REG_RAX
    call emit_mov_reg_imm64

    ; --- named tail call (docs/spec-tco-capture-gc.md section 2.6 step
    ; 4): same three compile-time conditions as .indirect_path's own
    ; tail check (this call is itself tail, current_lambda_depth is
    ; nonzero, nargs<=3), but the call SITE itself must become a `jmp`
    ; here rather than a `call` — a `call`'s own return address would
    ; grow the native stack on every iteration exactly like the
    ; un-fixed indirect path used to. That in turn means the
    ; trampoline's usual trick of finding its own patch site by reading
    ; the return address off the stack no longer works (a `jmp`-entered
    ; trampoline finds some unrelated caller's return address there
    ; instead — emit_ic_trampoline's own mode=1 comment has the full
    ; danger), so this path uses mode=1: emit the `jmp` first (getting
    ; its field_addr back, at compile time, from emit_jmp32 itself,
    ; before the trampoline that needs to bake it even exists), then
    ; build a mode=1 trampoline around that known field_addr.
    cmp r15, 1
    jne .n_ordinary_call
    cmp qword [current_lambda_depth], 0
    je .n_ordinary_call
    cmp r13, 3
    ja .n_ordinary_call

    ; Tear down THIS function's own frame before transferring control —
    ; exactly like .indirect_path's own emit_leave, and for the same
    ; reason: args are already popped into their final registers above,
    ; so nothing below this function's entry-time rbp is needed again.
    ; Forgetting this (an earlier draft of this patch did) leaves the
    ; callee building a new frame ON TOP of this one instead of reusing
    ; it — the call stack still grows on every iteration exactly as
    ; before this feature, and once the callee's own `leave`/`ret` runs
    ; it returns through a return address this function's own `call`
    ; site never actually set up as its caller expected, corrupting the
    ; whole chain (the segfaults this exact mistake produced, caught by
    ; tests/cases/016_rest_params.asm's F, whose body's only form is a
    ; tail call to a different named function, before this comment was
    ; written).
    call emit_leave

    call emit_jmp32
    mov r13, rax                                ; field_addr (nargs, r13's
                                                 ; old value, is never
                                                 ; needed again on this
                                                 ; path — no 4+ cleanup
                                                 ; is possible when
                                                 ; nargs<=3 is already
                                                 ; required above)
    mov rdi, r14                                  ; cell_addr
    mov rsi, 1                                      ; mode 1
    mov rdx, r13                                      ; field_addr
    call emit_ic_trampoline
    mov rdi, r13                                        ; field_addr
    mov rsi, rax                                          ; trampoline_entry
    call patch_rel32                                        ; jmp site -> trampoline
    jmp .out

.n_ordinary_call:
    mov rdi, r14                              ; cell_addr
    mov rsi, 0                                  ; mode 0
    mov rdx, 0                                    ; unused at mode 0
    call emit_ic_trampoline
    mov r12, rax                                    ; trampoline_entry

    call emit_call32
    mov rdi, rax
    mov rsi, r12                                                                ; trampoline_entry
    call patch_rel32                                                              ; call site -> trampoline

    ; Anything past the 3rd argument was left on the *target* stack by
    ; compile_call_args, positioned for the callee's own stack-passed
    ; params (build_param_frame) — the callee's own `leave`/`ret` only
    ; unwinds what it pushed *after* its own `push rbp`, never these
    ; caller-pushed extra args sitting below the return address. Without
    ; this cleanup they stay on the stack after the call returns,
    ; corrupting anything the *enclosing* expression pushed for its own
    ; safekeeping around this call (compile_binop's own lhs, a
    ; compile_binary_hostcall's arg1, ...) — a real, previously-latent
    ; bug: `(CONS 'X (F a b c d))` for any 4+-arg F silently returned
    ; garbage instead of X as its car. nargs is a compile-time constant
    ; here (this call site's own syntactic argument count), so the
    ; cleanup amount is too.
    cmp r13, 3
    jbe .named_no_cleanup
    mov rax, r13
    sub rax, 3
    imul eax, eax, -8
    mov edi, eax
    call emit_sub_rsp_imm32
.named_no_cleanup:
    jmp .out

.indirect_path:
    ; Args first, *then* the operator — fully right-to-left, operator
    ; included, not just "before the args" as an earlier version had it.
    ; That earlier order pushed the closure value *underneath* any
    ; excess (beyond 3) stack-passed args, so popping only 3 args before
    ; reaching for the closure grabbed an excess arg instead whenever
    ; nargs > 3. Evaluating the operator last puts the closure on top,
    ; poppable before the excess args — which must stay untouched,
    ; positioned exactly where the callee's stack-passed params expect
    ; them — are ever reached.
    mov rdi, r12
    call compile_call_args
    mov r12, rax                                ; nargs (r12 reused: the
                                                 ; args list itself is no
                                                 ; longer needed; rbx still
                                                 ; holds the operator form)

    mov rdi, rbx
    call compile_form                             ; operator -> target rax
    call emit_check_callable                        ; die cleanly if it isn't one
    mov dil, REG_RAX
    call emit_push_reg                               ; push closure (now on
                                                      ; top, above every arg)
    mov dil, REG_RDI
    call emit_pop_reg                                  ; rdi = closure ptr

    cmp r12, 1
    jb .i_after_a0
    mov dil, REG_RSI
    call emit_pop_reg
.i_after_a0:
    cmp r12, 2
    jb .i_after_a1
    mov dil, REG_RDX
    call emit_pop_reg
.i_after_a1:
    cmp r12, 3
    jb .i_after_a2
    mov dil, REG_RCX
    call emit_pop_reg
.i_after_a2:

    mov dil, REG_RAX
    mov sil, REG_RDI
    call emit_mov_rr
    mov edi, 0xFFFFFFFC
    call emit_and_rax_imm32
    mov dil, REG_RBX
    mov sil, REG_RAX
    mov edx, 8
    call emit_load_based
    ; rax (used as scratch just above) is free again now that code_ptr
    ; is safely in rbx — set it to the actual arg count (see the named
    ; path's identical comment) right before the call itself.
    mov rsi, r12
    mov dil, REG_RAX
    call emit_mov_reg_imm64

    ; --- indirect tail call (docs/spec-tco-capture-gc.md section 2.6
    ; step 3): reuse this function's own frame instead of growing the
    ; native stack, when it's actually safe to. Three compile-time
    ; conditions, all required: (1) r15 (this call's own tail flag, the
    ; rdx compile_call was given) is 1 — the call's VALUE must be the
    ; enclosing function's own return value with nothing left to do
    ; afterward; (2) [current_lambda_depth] is nonzero — the defensive
    ; belt-and-suspenders guard: never emit a tail jump while compiling
    ; a top-level thunk, regardless of any bug upstream in the tail_ctx
    ; plumbing; (3) nargs<=3 — v0 scope, no stack-passed args to worry
    ; about, so `leave` alone (no manual stack cleanup at all) is
    ; exactly correct: it resets rsp to this function's own entry-time
    ; rbp, discarding every LET/LET* slot this call might be nested
    ; under, which is correct because this genuinely is the last thing
    ; the function will ever do. rax/rsi/rdx/rcx (nargs, arg0-2) and
    ; rbx (code_ptr) are all already in their final places above and
    ; survive `leave` untouched (it only touches rsp/rbp) — a bare
    ; `emit_jmp_reg` after it is the entire difference from the
    ; ordinary call path just below.
    cmp r15, 1
    jne .i_ordinary_call
    cmp qword [current_lambda_depth], 0
    je .i_ordinary_call
    cmp r12, 3
    ja .i_ordinary_call
    call emit_leave
    mov dil, REG_RBX
    call emit_jmp_reg
    jmp .out                              ; nothing after a tail jump is
                                           ; ever reached at runtime

.i_ordinary_call:
    mov dil, REG_RBX
    call emit_call_reg

    ; Same stack cleanup as the named-global path above, and for the
    ; same reason: anything past the 3rd argument was left on the
    ; target stack by compile_call_args for the callee's own
    ; stack-passed params, and nothing unwinds it after the call
    ; returns without this.
    cmp r12, 3
    jbe .i_no_cleanup
    mov rax, r12
    sub rax, 3
    imul eax, eax, -8
    mov edi, eax
    call emit_sub_rsp_imm32
.i_no_cleanup:

.out:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_catch(rdi = (CATCH tag body) form)
; Installs a frame (inline — no separate function, exactly like IF) and
; falls straight through into body: the frame's own resume target is
; recorded as "wherever body starts", so a normal, non-thrown return
; needs no jump at all, only bookkeeping before and after.
compile_catch:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi
    call cadr
    mov r12, rax                      ; tag form
    mov rdi, rbx
    call caddr
    mov r13, rax                        ; body form

    mov rdi, r12
    call compile_form                     ; tag -> target rax
    mov dil, REG_RAX
    call emit_push_reg                       ; target: push tag

    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_load_mem64                       ; target: rax = top
    mov edi, 32
    call emit_imul_rax_imm32                        ; target: rax = top*32
    lea rax, [rel catch_stack]
    mov edi, eax
    call emit_add_rax_imm32                           ; target: rax = frame_addr
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr                                    ; target: rbx = frame_addr (kept
                                                         ; in rbx, not rcx: emit_store_mem64
                                                         ; below uses rcx as its own scratch
                                                         ; and would otherwise clobber it)

    mov dil, REG_RAX
    call emit_pop_reg                                     ; target: rax = tag (restored)
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 0
    call emit_store_based                                   ; frame[0] = tag

    mov dil, REG_RAX
    mov sil, REG_RBP
    call emit_mov_rr
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 8
    call emit_store_based                                     ; frame[8] = saved rbp

    mov dil, REG_RAX
    mov sil, REG_RSP
    call emit_mov_rr
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 16
    call emit_store_based                                       ; frame[16] = saved rsp

    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_load_mem64                                          ; target: rax = top (again;
                                                                   ; cheaper to reload than to
                                                                   ; keep yet another register live)
    mov edi, 1
    call emit_add_rax_imm32
    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_store_mem64                                           ; catch_stack_top = top + 1

    ; frame[24] = resume target. This is NOT "wherever body starts" — a
    ; caught throw must resume where a normal return from body would
    ; continue (the epilogue just below, which pops this frame), never
    ; back at the top of body, or a caught throw would simply re-run
    ; body and re-throw forever. So the placeholder is patched with the
    ; address *after* body compiles, not before — the same deferred-
    ; patch idea as compile_if's forward branches, just later.
    call codegen_here
    mov r14, rax
    add r14, 2                              ; &imm64 operand (always 2
                                             ; bytes into the instruction
                                             ; — REX.W + opcode — a fixed
                                             ; fact about emit_mov_reg_imm64's
                                             ; own encoding)
    mov dil, REG_RAX
    mov rsi, 0
    call emit_mov_reg_imm64                    ; placeholder: rax = 0
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 24
    call emit_store_based                        ; frame[24] = rax (patched below)

    mov rdi, r13
    call compile_form                          ; body — its own result ends
                                                ; up in rax when it falls
                                                ; through normally

    call codegen_here                              ; true resume target:
    mov rdi, r14                                      ; the epilogue below,
    mov rsi, rax                                        ; reached either by
    call patch_imm64                                      ; falling through
                                                             ; or by a throw
                                                             ; landing here
                                                             ; with its value
                                                             ; already in rax
    mov dil, REG_RAX
    call emit_push_reg                            ; save result across the
                                                   ; pop-the-frame bookkeeping
    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_load_mem64
    mov edi, 1
    call emit_sub_rax_imm32
    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_store_mem64                            ; catch_stack_top -= 1
                                                       ; (a throw landing here
                                                       ; already pre-added 1
                                                       ; to compensate — see
                                                       ; compile_throw)
    mov dil, REG_RAX
    call emit_pop_reg                                  ; rax = result (restored)

    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_throw(rdi = (THROW tag value) form)
; Compiles tag and value, then emits a call into native_throw
; (native_errors.asm) with rdi=tag, rsi=value — the exact same catch-
; stack search/restore/jump this used to hand-encode inline as target
; machine code, now written once as an ordinary host routine and
; reused from both here and every native failure (CAR/CDR's wrong-type
; check, calling a non-callable value) that needs to signal into the
; same CATCH/HANDLER-CASE/ERRORSET machinery. Folding THROW itself
; into that one shared implementation, rather than keeping two
; hand-encoded copies of the same search loop in sync, is also what
; makes UNWIND-PROTECT's cleanup-on-any-passing-throw semantics
; (KERNEL.md Part VII) implementable at all: native_throw's walk is
; the one and only place that needs to notice an unwind-protect marker
; frame on its way past — see "KERNEL.md conformance" below.
extern native_throw
compile_throw:
    push rbx
    push r12
    push r13
    mov rbx, rdi
    call cadr
    mov r12, rax                    ; tag form
    mov rdi, rbx
    call caddr
    mov r13, rax                      ; value form

    mov rdi, r12
    call compile_form                    ; tag -> target rax
    mov dil, REG_RAX
    call emit_push_reg                      ; target: push tag

    mov rdi, r13
    call compile_form                          ; value -> target rax
    mov dil, REG_RSI
    mov sil, REG_RAX
    call emit_mov_rr                              ; target: rsi = value
    mov dil, REG_RDI
    call emit_pop_reg                                ; target: rdi = tag (restored)

    lea rax, [rel native_throw]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg                                 ; never returns

    pop r13
    pop r12
    pop rbx
    ret

; --- the condition system (KERNEL.md Part VIII) ---
;
; A condition value is a two-field heapobj (HDR_CONDITION,
; conditions.asm). ERROR signals one by THROWing it to a single
; shared internal tag (handler_case_tag, conditions.asm) that every
; HANDLER-CASE/ERRORSET installs its catch frame with — this is Part
; XII axis 3's own explicit license to derive a special form from a
; smaller primitive set: CATCH/THROW's existing "nearest matching tag
; wins" search is already exactly the dynamic-extent unwind a
; condition system needs, so one shared tag plus the machinery below
; is the whole of it, no separate signaling primitive required.

extern make_string
extern make_error
extern is_condition
extern error_message_tagged
extern error_data_tagged
extern error_of
extern handler_case_tag

; emit_pop_catch_frame() — target: catch_stack_top -= 1, preserving
; rax. Correct whether reached by a protected form completing normally
; or by a throw landing here (a throw always pre-adds 1 to
; catch_stack_top before jumping in — see compile_throw/compile_error).
emit_pop_catch_frame:
    mov dil, REG_RAX
    call emit_push_reg
    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_load_mem64
    mov edi, 1
    call emit_sub_rax_imm32
    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_store_mem64
    mov dil, REG_RAX
    jmp emit_pop_reg

; emit_install_catch_frame(rdi = host-known tag value) -> rax =
; address of the resume-target placeholder's imm64 operand (patch it
; with patch_imm64 once the real resume label is known — same
; deferred-patch idea compile_catch itself uses, just factored out so
; HANDLER-CASE and ERRORSET don't each re-derive it with a baked
; rather than compiled tag).
emit_install_catch_frame:
    push rbx
    push r12
    mov r12, rdi                      ; tag

    mov rsi, r12
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_push_reg                   ; push tag

    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_load_mem64
    mov edi, 32
    call emit_imul_rax_imm32
    lea rax, [rel catch_stack]
    mov edi, eax
    call emit_add_rax_imm32
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr                        ; rbx = frame_addr

    mov dil, REG_RAX
    call emit_pop_reg                          ; rax = tag
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 0
    call emit_store_based                        ; frame[0] = tag

    mov dil, REG_RAX
    mov sil, REG_RBP
    call emit_mov_rr
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 8
    call emit_store_based                           ; frame[8] = rbp

    mov dil, REG_RAX
    mov sil, REG_RSP
    call emit_mov_rr
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 16
    call emit_store_based                             ; frame[16] = rsp

    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_load_mem64
    mov edi, 1
    call emit_add_rax_imm32
    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_store_mem64                               ; top += 1

    call codegen_here
    mov r12, rax
    add r12, 2
    mov dil, REG_RAX
    mov rsi, 0
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 24
    call emit_store_based                                 ; frame[24] = placeholder

    mov rax, r12
    pop r12
    pop rbx
    ret

; emit_throw_baked(rdi = host-known tag value) — throws whatever is
; currently in target rax to the nearest enclosing CATCH/HANDLER-CASE
; installed with this exact tag. Unlike compile_throw, the tag is a
; compile-time constant (not a form to compile) and the value is
; already sitting in target rax (built by a host function call, not
; evaluated fresh) — this is what ERROR's signaling needs. Emits a
; call into native_throw (native_errors.asm), the same shared
; implementation compile_throw itself now uses, rather than a second
; hand-encoded copy of the search loop — so ERROR's own THROW-to-
; handler_case_tag() also correctly fires any UNWIND-PROTECT cleanup
; it passes on the way to its handler (see native_throw's own comment).
emit_throw_baked:
    push rbx
    mov rbx, rdi                  ; save tag — dil is rdi's own low byte,
                                   ; so the emit_mov_rr call just below
                                   ; would otherwise destroy it in place
    mov dil, REG_RSI
    mov sil, REG_RAX
    call emit_mov_rr                       ; target: rsi = value

    mov rsi, rbx
    mov dil, REG_RDI
    call emit_mov_reg_imm64                  ; target: rdi = tag

    lea rax, [rel native_throw]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg                          ; never returns
    pop rbx
    ret

; compile_error(rdi = the full (ERROR ...) form, 0/1/2 args)
compile_error:
    push rbx
    mov rbx, rdi
    call cdr
    cmp rax, IMM_NIL
    jne .has_args

    ; (ERROR) — bake a fixed "Error"/NIL condition once, at compile
    ; time, exactly like any other self-evaluating literal.
    lea rdi, [rel default_error_msg]
    mov rsi, 5
    call make_string
    mov rdi, rax
    mov rsi, IMM_NIL
    call make_error
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    jmp .throw_it

.has_args:
    mov rbx, rax                    ; args list
    mov rdi, rbx
    call cdr
    cmp rax, IMM_NIL
    jne .two_args

    ; (ERROR c) — runtime dispatch (error_of): re-signal c unchanged if
    ; it's already a condition, else wrap it as the message.
    mov rdi, rbx
    call car
    lea rsi, [rel error_of]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .throw_it

.two_args:
    ; (ERROR message data)
    mov rdi, rbx
    call car
    push rax
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call car
    mov rsi, rax
    pop rdi
    lea rdx, [rel make_error]
    call compile_binary_hostcall

.throw_it:
    call handler_case_tag
    mov rdi, rax
    call emit_throw_baked
    pop rbx
    ret

; compile_handler_case(rdi = the full
;   (HANDLER-CASE protected (head (var) handler-body...)) form)
; Exactly two operands: the protected form, and one clause. `head` is
; never inspected (any symbol works, `error` by convention); its
; second element, if non-empty, names the variable the caught
; condition is bound to. Catches *unconditionally* — there is no
; typed clause, matching KERNEL.md Part VIII exactly.
compile_handler_case:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi
    call cadr
    mov r12, rax                       ; protected form
    mov rdi, rbx
    call caddr
    mov r13, rax                         ; clause = (head (var) handler-body...)

    call handler_case_tag
    mov rdi, rax
    call emit_install_catch_frame
    mov r14, rax                            ; resume-target placeholder addr

    mov rdi, r12
    call compile_form                          ; protected -> target rax

    call emit_pop_catch_frame
    call emit_jmp32                              ; -> DONE (patched below)
    push rax                                       ; [done_site]

    call codegen_here                              ; HANDLER label
    mov rdi, r14
    mov rsi, rax
    call patch_imm64

    call emit_pop_catch_frame                        ; rax = condition value

    mov rdi, r13
    call cadr
    mov rbx, rax                                       ; (var) or NIL
    cmp rbx, IMM_NIL
    je .bind_none

    mov rdi, rbx
    call car
    mov rbx, rax                                         ; var symbol

    mov edi, 8
    call emit_sub_rsp_imm32

    mov rax, [current_frame_depth]
    inc rax
    imul rax, rax, -8
    mov esi, eax
    mov dil, REG_RAX
    call emit_store_local                                    ; var's slot = condition

    mov rax, [current_frame_depth]
    inc rax
    imul rax, rax, -8
    mov rsi, rax
    mov rdi, rbx
    call cons                                                    ; (var . disp)
    mov rdi, rax
    mov rsi, [current_scope]
    call cons                                                      ; new_scope
    mov rbx, rax

    mov rax, [current_scope]
    push rax                                                          ; [old_scope, done_site]
    mov [current_scope], rbx
    mov rax, [current_frame_depth]
    push rax                                                            ; [old_frame_depth, old_scope, done_site]
    inc qword [current_frame_depth]

    mov rdi, r13
    call cdr
    mov rdi, rax
    call cdr
    mov rdi, rax
    xor esi, esi                                                           ; never tail: a catch-stack
                                                                            ; frame would point into a
                                                                            ; discarded frame (spec sec. 2)
    call compile_progn                                                     ; handler-body -> rax

    pop rax
    mov [current_frame_depth], rax
    pop rax
    mov [current_scope], rax

    mov dil, REG_RSP
    mov esi, 8
    call emit_add_reg_imm32
    jmp .handler_done

.bind_none:
    mov rdi, r13
    call cdr
    mov rdi, rax
    call cdr
    mov rdi, rax
    xor esi, esi                          ; never tail, same as the bound path
    call compile_progn

.handler_done:
    call codegen_here                                                          ; DONE
    pop rdi
    mov rsi, rax
    call patch_rel32

    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_errorset(rdi = the full (ERRORSET form [ignored]) form)
; Matches KERNEL.md's own ERRORSET exactly, now that EVAL exists:
; `form` is compiled and run to get a *value* (the idiom is
; `(errorset '(car 5))`, so this is ordinarily a QUOTE), and that value
; is itself then run as code via eval_form — the second step ERRORSET's
; own spec requires, previously missing (see README roadmap; EVAL is
; new). Catches any ERROR signaled while either step runs: returns a
; one-element list (value) on success (so a successful NIL return is
; distinguishable from failure), or NIL if caught.
compile_errorset:
    push rbx
    push r12
    mov rbx, rdi
    call cadr
    mov r12, rax                    ; protected form

    call handler_case_tag
    mov rdi, rax
    call emit_install_catch_frame
    mov rbx, rax                       ; resume-target placeholder addr

    mov rdi, r12
    call compile_form                     ; protected -> target rax = the
                                           ; value to run as code (usually
                                           ; a quoted form)
    mov dil, REG_RDI
    mov sil, REG_RAX
    call emit_mov_rr                        ; target: rdi = that value
    lea rax, [rel eval_form]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64                   ; target: rax = &eval_form
    mov dil, REG_RAX
    call emit_call_reg                          ; target: call eval_form ->
                                                 ; rax = its result

    call emit_pop_catch_frame
    mov dil, REG_RDI
    mov sil, REG_RAX
    call emit_mov_rr
    mov rsi, IMM_NIL
    mov dil, REG_RSI
    call emit_mov_reg_imm64
    lea rax, [rel cons]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg                       ; rax = (value)

    call emit_jmp32
    push rax                                   ; [done_site]

    call codegen_here
    mov rdi, rbx
    mov rsi, rax
    call patch_imm64

    call emit_pop_catch_frame                    ; rax = condition (discarded)
    mov rsi, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64                        ; rax = NIL (failure)

    call codegen_here
    pop rdi
    mov rsi, rax
    call patch_rel32

    pop r12
    pop rbx
    ret

; compile_unwind_protect(rdi = (UNWIND-PROTECT body-form cleanup...)
; form) — KERNEL.md Part VII: body-form is evaluated, then every
; cleanup form runs *unconditionally* — after a normal return, an
; error, or any other non-local exit passing through — and the body's
; own outcome (its value, or the propagating throw/error) is what this
; form ultimately delivers. Unlike BLOCK/HANDLER-CASE, which only need
; to react to a throw that targets *them specifically*, this needs to
; react to *any* throw merely passing through on its way somewhere
; else — which is exactly why compile_throw/emit_throw_baked were
; first refactored to funnel every THROW/ERROR through one shared host
; routine, native_throw (native_errors.asm): that is now the one place
; a passing throw's search can notice this form's own "marker" frame
; on the catch stack and fire its cleanup right there, before
; continuing to search for the real target. See native_throw's own
; comment for the marker-frame mechanics.
;
; The marker frame reuses the ordinary 32-byte catch_stack slot shape,
; but not its ordinary meaning: frame[0] = the shared
; unwind_protect_marker_tag() (never a real CATCH/THROW target — see
; conditions.asm's handler_case_tag for the identical one-shared-tag
; precedent) and frame[8] = the cleanup closure itself, in place of
; where an ordinary frame keeps its saved rbp (frame[16]/frame[24] are
; unused, since a marker is never jumped to directly). The cleanup
; forms compile as an ordinary zero-parameter LAMBDA — a real closure
; over the enclosing lexical scope, built from a synthesized
; (LAMBDA () cleanup...) AST via the same CONS/intern_symbol host
; calls the reader itself uses, then handed to compile_lambda exactly
; like any other LAMBDA form — so free variables in cleanup forms
; resolve normally, with no new capture mechanism needed.
;
; v0 scope, narrower than the spec on purpose: "an error raised by a
; cleanup form is discarded" is NOT yet true here — a cleanup form
; that itself signals an uncaught condition propagates as an ordinary
; new native_throw search, which can end up superseding whichever
; throw was already being processed, rather than being silently
; swallowed so the original throw continues. Implementing the
; spec's exact discard behavior needs the cleanup invocation itself
; wrapped in a synthetic innermost HANDLER-CASE-shaped catch (the same
; handler_case_tag() ERROR always throws to) — a real next step, not
; attempted here to keep this change's blast radius to the primary,
; spec-critical guarantee: cleanup runs on every exit path, full stop.
extern unwind_protect_marker_tag
extern invoke_thunk
compile_unwind_protect:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi
    call cdr
    mov r12, rax                     ; (body-form cleanup...)
    mov rdi, r12
    call car
    mov r13, rax                       ; body-form
    mov rdi, r12
    call cdr
    mov r12, rax                         ; cleanup-forms list (0 or more)

    ; Synthesize (LAMBDA () cleanup-forms...) and compile it as an
    ; ordinary LAMBDA — target rax ends up holding the fresh closure.
    ; Cons cells are immutable here, so this builds outside-in: the
    ; empty param list consed onto the cleanup-forms list first, then
    ; the LAMBDA symbol consed onto that.
    mov rdi, IMM_NIL
    mov rsi, r12
    call cons                                        ; (() . cleanup-forms)
    mov r12, rax
    mov rdi, kw_lambda
    mov rsi, 6
    call intern_symbol
    mov rdi, rax
    mov rsi, r12
    call cons                                          ; (LAMBDA () cleanup...)
    mov rdi, rax
    call compile_lambda                                  ; target rax = closure
    mov dil, REG_RAX
    call emit_push_reg                                     ; target: push closure

    ; Install the marker frame: frame[0]=marker_tag frame[8]=closure
    ; frame[16]=0 frame[24]=0; catch_stack_top += 1.
    call unwind_protect_marker_tag
    mov r14, rax                                             ; marker tag (host)

    mov dil, REG_RAX
    mov rsi, r14
    call emit_mov_reg_imm64                                    ; target rax = marker_tag
    mov dil, REG_RAX
    call emit_push_reg                                           ; push it (over closure)

    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_load_mem64                                          ; target rax = top
    mov edi, 32
    call emit_imul_rax_imm32
    lea rax, [rel catch_stack]
    mov edi, eax
    call emit_add_rax_imm32                                         ; target rax = frame_addr
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr                                                  ; target rbx = frame_addr

    mov dil, REG_RAX
    call emit_pop_reg                                                   ; target rax = marker_tag
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 0
    call emit_store_based                                                 ; frame[0] = marker_tag

    mov dil, REG_RAX
    mov esi, 0
    call emit_load_rsp_disp8                                                ; target rax = closure (still on stack)
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 8
    call emit_store_based                                                     ; frame[8] = closure
    mov dil, REG_RAX
    call emit_pop_reg                                                           ; discard closure copy (balance stack)

    mov dil, REG_RAX
    mov rsi, 0
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 16
    call emit_store_based                                                         ; frame[16] = 0
    mov dil, REG_RAX
    mov sil, REG_RBX
    mov edx, 24
    call emit_store_based                                                           ; frame[24] = 0

    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_load_mem64
    mov edi, 1
    call emit_add_rax_imm32
    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_store_mem64                                                           ; top += 1

    ; --- compile body-form ---
    mov rdi, r13
    call compile_form                                                     ; target rax = body result

    ; --- normal-completion epilogue: fire cleanup, pop the marker ---
    mov dil, REG_RAX
    call emit_push_reg                                                      ; save body result

    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_load_mem64
    mov edi, 1
    call emit_sub_rax_imm32
    mov edi, 32
    call emit_imul_rax_imm32
    lea rax, [rel catch_stack]
    mov edi, eax
    call emit_add_rax_imm32                                                  ; target rax = frame_addr (top-1)
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr                                                           ; target rbx = frame_addr

    mov dil, REG_RDI
    mov sil, REG_RBX
    mov edx, 8
    call emit_load_based                                                         ; target rdi = closure

    lea rax, [rel invoke_thunk]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg                                                             ; call invoke_thunk(rdi) ->
                                                                                    ; rax = result (discarded)

    call emit_pop_catch_frame                                                        ; catch_stack_top -= 1

    mov dil, REG_RAX
    call emit_pop_reg                                                                  ; target rax = body result
                                                                                        ; (restored)
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_block(rdi = (BLOCK name body...) form)
; `name` is an *unevaluated* symbol — a compile-time-known tag, not a
; form to compile — so this reuses the exact same catch-frame-install/
; pop machinery HANDLER-CASE does, just with `name` itself as the tag
; instead of the shared handler-case tag, and no variable binding on
; the way out: RETURN-FROM's thrown value is already the correct
; result. BLOCK/RETURN-FROM being expressible this way at all is
; KERNEL.md Part XII axis 3's explicit license to derive a special
; form from CATCH/THROW rather than add a new primitive.
compile_block:
    push rbx
    push r12
    push r13
    mov rbx, rdi
    call cadr
    mov r12, rax                      ; name (used directly as tag)
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call cdr
    mov rbx, rax                        ; body forms

    mov rdi, r12
    call emit_install_catch_frame
    mov r13, rax                          ; resume-target placeholder addr

    mov rdi, rbx
    xor esi, esi                             ; never tail: same catch-frame
                                              ; reasoning as HANDLER-CASE
    call compile_progn                       ; body -> target rax

    call emit_pop_catch_frame
    call emit_jmp32
    push rax                                   ; [done_site]

    call codegen_here                            ; RETURN-FROM lands here
    mov rdi, r13
    mov rsi, rax
    call patch_imm64

    call emit_pop_catch_frame                       ; rax = returned value
                                                     ; (already the correct
                                                     ; overall result — no
                                                     ; binding needed, unlike
                                                     ; HANDLER-CASE)

    call codegen_here
    pop rdi
    mov rsi, rax
    call patch_rel32

    pop r13
    pop r12
    pop rbx
    ret

; compile_return_from(rdi = (RETURN-FROM name [value]) form)
; `name` is unevaluated, matched by EQ against the nearest enclosing
; BLOCK's own name — an unknown name is a hard failure at the THROW
; site (the trap emit_throw_baked already falls back to when nothing
; matches), same v0 posture as an unmatched CATCH/THROW.
compile_return_from:
    push rbx
    push r12
    mov rbx, rdi
    call cadr
    mov r12, rax                    ; name

    mov rdi, rbx
    call cdr
    mov rdi, rax
    call cdr
    cmp rax, IMM_NIL
    jne .has_value
    mov rsi, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    jmp .throw_it
.has_value:
    mov rdi, rax
    call car
    mov rdi, rax
    call compile_form
.throw_it:
    mov rdi, r12
    call emit_throw_baked
    pop r12
    pop rbx
    ret

; --- PROG/GO/RETURN (KERNEL.md Part VII) ---
;
; "(prog (vars...) item...) binds each var to NIL, then executes the
; items in order, treating a bare symbol item as a label. (go label)
; jumps to a label in the innermost dynamically enclosing PROG ...
; (return value) exits the innermost PROG with value; falling off the
; end yields NIL." v0 scope, narrower than the spec on purpose: GO/
; RETURN here are *lexical*, not dynamic-extent — they only resolve
; when textually within the same PROG's own body (including nested
; inside ordinary IF/WHEN/LET expressions there), not when reached via
; a call into a separate function. A fully dynamic-extent GO would
; need the same catch-stack-marker machinery UNWIND-PROTECT uses, but
; re-installed on *every* pass through a label — which would grow the
; catch stack without bound on an ordinary GO-based loop, unlike
; UNWIND-PROTECT's own one-shot marker. This lexical v0 covers the
; overwhelmingly common real usage (an imperative loop with labels,
; every GO/RETURN written directly inside the PROG that owns them)
; without that cost, and is honestly narrower where it diverges.
;
; compile_prog reserves a small host-side (compile-time only, not
; target-runtime) scratch buffer on its own machine stack frame:
; labels_seen (symbol -> address, filled in as each label is reached),
; pending_gos (site -> label symbol, one per GO — resolved in one pass
; once every label in the body has been seen), and pending_returns
; (site only — every RETURN converges on the same exit point, patched
; once that is known). Fixed 32-entry capacity each; PROG bodies
; needing more silently degrade (a v0 bound, same spirit as
; catch_stack's own fixed 256 frames) rather than growing dynamically.
%define PROG_LABELS_CAP 32
%define PROG_GOS_CAP 32
%define PROG_RETURNS_CAP 32
%define PROG_LABELS_OFF 0
%define PROG_GOS_OFF 512
%define PROG_RETURNS_OFF 1024
%define PROG_LABELS_COUNT_OFF 1280
%define PROG_GOS_COUNT_OFF 1288
%define PROG_RETURNS_COUNT_OFF 1296
%define PROG_CTX_SIZE 1304

; compile_prog(rdi = the full (PROG (vars...) item...) form)
compile_prog:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov rbx, rdi
    call cadr
    mov r12, rax                     ; vars list
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call cdr
    mov r13, rax                       ; items list

    mov rdi, r12
    call list_length
    push rax                              ; [k]

    mov eax, [rsp]
    imul eax, eax, 8
    mov edi, eax
    call emit_sub_rsp_imm32                  ; reserve var slots (rbp-
                                              ; relative — independent
                                              ; of this bookkeeping's
                                              ; own rsp-relative stack)

    mov rdi, r12
    mov rsi, [current_frame_depth]
    call build_frame_from_list
    mov r14, rax                               ; new_frame

    mov rbx, r12
.var_init_loop:
    cmp rbx, IMM_NIL
    je .vars_done
    mov rdi, rbx
    call car
    mov rdi, rax
    mov rsi, r14
    call frame_lookup
    mov esi, eax
    mov rax, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_store_local
    mov rdi, rbx
    call cdr
    mov rbx, rax
    jmp .var_init_loop
.vars_done:
    mov rdi, r14
    mov rsi, [current_scope]
    call append_lists
    mov r14, rax                                 ; new_scope

    mov rax, [current_scope]
    push rax                                       ; [old_scope, k]
    mov [current_scope], r14

    mov rax, [current_frame_depth]
    push rax                                         ; [old_frame_depth, old_scope, k]
    mov rcx, [rsp+16]                                  ; k
    add rax, rcx
    mov [current_frame_depth], rax

    sub rsp, PROG_CTX_SIZE                               ; [buffer, old_frame_depth, old_scope, k]
    mov rax, rsp                                           ; buffer address
    mov qword [rax+PROG_LABELS_COUNT_OFF], 0
    mov qword [rax+PROG_GOS_COUNT_OFF], 0
    mov qword [rax+PROG_RETURNS_COUNT_OFF], 0

    mov rcx, [current_prog_ctx]
    push rcx                                                 ; [old_prog_ctx, buffer, old_frame_depth, old_scope, k]
    mov [current_prog_ctx], rax

    ; --- walk items ---
    mov rbx, r13                    ; items cursor
.item_loop:
    cmp rbx, IMM_NIL
    je .items_done
    mov rdi, rbx
    call car
    mov r12, rax                      ; item
    mov rdi, rbx
    call cdr
    mov rbx, rax                        ; advance cursor

    cmp r12, IMM_NIL
    je .ordinary_item
    mov rax, r12
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .ordinary_item
    mov rax, r12
    UNTAG_PTR rax
    cmp qword [rax], HDR_SYMBOL
    jne .ordinary_item

    ; label item: record {symbol, this address} in labels_seen.
    call codegen_here
    mov r13, rax
    mov rax, [current_prog_ctx]
    mov rcx, [rax+PROG_LABELS_COUNT_OFF]
    cmp rcx, PROG_LABELS_CAP
    jae .item_loop
    mov rdx, rcx
    shl rdx, 4
    mov [rax+PROG_LABELS_OFF+rdx], r12
    mov [rax+PROG_LABELS_OFF+rdx+8], r13
    inc rcx
    mov [rax+PROG_LABELS_COUNT_OFF], rcx
    jmp .item_loop

.ordinary_item:
    mov rdi, r12
    call compile_form                     ; value discarded — items
                                           ; are not tail positions
    jmp .item_loop
.items_done:

    ; --- resolve every pending GO against the now-complete label table ---
    mov rbx, [current_prog_ctx]
    xor r12, r12                    ; scan index over pending_gos
.resolve_gos_loop:
    mov rcx, [rbx+PROG_GOS_COUNT_OFF]
    cmp r12, rcx
    jae .resolve_gos_done
    mov rdx, r12
    shl rdx, 4
    mov r13, [rbx+PROG_GOS_OFF+rdx]           ; site
    mov r14, [rbx+PROG_GOS_OFF+rdx+8]           ; target label symbol
    xor r15, r15                                  ; scan index over labels_seen
.find_label_loop:
    mov rax, [rbx+PROG_LABELS_COUNT_OFF]
    cmp r15, rax
    jae .next_go                                    ; unknown label: v0
                                                     ; leaves this GO's
                                                     ; displacement at
                                                     ; its 0 placeholder
                                                     ; (falls through to
                                                     ; the next
                                                     ; instruction)
                                                     ; rather than
                                                     ; trapping — see
                                                     ; README
    mov rdx, r15
    shl rdx, 4
    cmp qword [rbx+PROG_LABELS_OFF+rdx], r14
    je .label_found
    inc r15
    jmp .find_label_loop
.label_found:
    mov rdx, r15
    shl rdx, 4
    mov rsi, [rbx+PROG_LABELS_OFF+rdx+8]
    mov rdi, r13
    call patch_rel32
.next_go:
    inc r12
    jmp .resolve_gos_loop
.resolve_gos_done:

    ; --- falling off the end: rax = NIL; every RETURN converges here ---
    mov rsi, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    call codegen_here
    mov r12, rax                       ; exit address

    mov rbx, [current_prog_ctx]
    xor r13, r13
.patch_returns_loop:
    mov rcx, [rbx+PROG_RETURNS_COUNT_OFF]
    cmp r13, rcx
    jae .patch_returns_done
    mov rdx, r13
    shl rdx, 3
    mov rdi, [rbx+PROG_RETURNS_OFF+rdx]
    mov rsi, r12
    call patch_rel32
    inc r13
    jmp .patch_returns_loop
.patch_returns_done:

    ; --- teardown ---
    pop rax
    mov [current_prog_ctx], rax
    add rsp, PROG_CTX_SIZE
    pop rax
    mov [current_frame_depth], rax
    pop rax
    mov [current_scope], rax
    pop rax                                ; k
    imul eax, eax, 8
    mov esi, eax
    mov dil, REG_RSP
    call emit_add_reg_imm32                   ; release var slots

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_go(rdi = the full (GO label) form) — label is unevaluated.
; Emits an unconditional jump, recorded in the innermost active PROG's
; pending_gos for later resolution once every label in that PROG's own
; body has been seen (compile_prog above). Outside any PROG (v0's
; lexical scoping can't find an enclosing one), this traps.
compile_go:
    push rbx
    push r12
    mov rbx, rdi
    mov r12, [current_prog_ctx]
    test r12, r12
    jz .no_prog

    mov rdi, rbx
    call cadr
    mov rbx, rax                  ; label symbol

    ; Same reasoning as WHILE's back edge: a PROG/GO loop is this
    ; kernel's other unbounded, call-free allocation site. The label a
    ; GO targets is resolved after the fact (it may be forward or
    ; backward), so the safe point is emitted for both — a forward GO
    ; pays three compares once.
    call emit_safepoint

    call emit_jmp32
    mov rcx, rax                    ; site

    mov rdx, [r12+PROG_GOS_COUNT_OFF]
    cmp rdx, PROG_GOS_CAP
    jae .out
    mov rsi, rdx
    shl rsi, 4
    mov [r12+PROG_GOS_OFF+rsi], rcx
    mov [r12+PROG_GOS_OFF+rsi+8], rbx
    inc rdx
    mov [r12+PROG_GOS_COUNT_OFF], rdx
    jmp .out
.no_prog:
    mov rdi, 0xCC
    call emit8                      ; GO outside any (lexically
                                     ; visible) PROG — a v0 trap
.out:
    ; control never reaches past the jmp/trap above, but compile_form
    ; callers expect target rax to hold something regardless.
    mov rsi, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    pop r12
    pop rbx
    ret

; compile_return(rdi = the full (RETURN [value]) form) — value defaults
; to NIL. Compiles value (if any) into target rax, then emits an
; unconditional jump recorded in the innermost active PROG's
; pending_returns, resolved once that PROG knows its own exit address
; (every RETURN and the fall-off-the-end case converge on the same
; point). Outside any PROG, this traps, same as GO above.
compile_return:
    push rbx
    push r12
    mov rbx, rdi
    mov r12, [current_prog_ctx]
    test r12, r12
    jz .no_prog

    mov rdi, rbx
    call cdr
    cmp rax, IMM_NIL
    je .no_value
    mov rdi, rax
    call car
    mov rdi, rax
    call compile_form                  ; value -> target rax
    jmp .have_value
.no_value:
    mov rsi, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64
.have_value:
    call emit_jmp32
    mov rcx, rax                          ; site

    mov rdx, [r12+PROG_RETURNS_COUNT_OFF]
    cmp rdx, PROG_RETURNS_CAP
    jae .out
    mov rsi, rdx
    shl rsi, 3
    mov [r12+PROG_RETURNS_OFF+rsi], rcx
    inc rdx
    mov [r12+PROG_RETURNS_COUNT_OFF], rdx
    jmp .out
.no_prog:
    mov rdi, 0xCC
    call emit8                      ; RETURN outside any PROG — a v0 trap
.out:
    pop r12
    pop rbx
    ret

; compile_while(rdi = (WHILE test body...) form) -> always NIL.
; test is re-evaluated before each pass (a forward branch, patched
; once the loop's overall end is known); the backward jump back to the
; top is the mirror image — its target is already known, so it needs
; no patching at all, just like every other backward branch this
; compiler has emitted (the &REST rest-list loop, notably).
compile_while:
    push rbx
    push r12
    mov rbx, rdi
    call cadr
    mov r12, rax                  ; test form
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call cdr
    mov rbx, rax                    ; body forms

    call codegen_here
    push rax                          ; [loop_start]

    mov rdi, r12
    call compile_form                     ; test -> rax
    mov rsi, IMM_NIL
    call emit_cmp_rax_imm64
    call emit_je                             ; -> done_site
    push rax                                   ; [done_site, loop_start]

    mov rdi, rbx
    xor esi, esi                                  ; never tail (spec sec. 2)
    call compile_progn                            ; body -> rax (discarded)

    ; Loop back-edge safe point: a WHILE body that allocates but calls
    ; no function would otherwise never reach one, and would grow the
    ; heap without bound exactly as it does today (gc.asm).
    call emit_safepoint

    call emit_jmp32
    mov rdi, rax
    mov rsi, [rsp+8]                                ; loop_start
    call patch_rel32

    call codegen_here
    pop rdi                                            ; done_site
    mov rsi, rax
    call patch_rel32
    pop rax                                               ; discard loop_start

    mov rsi, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64

    pop r12
    pop rbx
    ret

section .rodata
default_error_msg: db "Error"

section .text

; compile_form(rdi = tagged sexpr)
; Emits code, inline into whatever function is currently open, that
; leaves the form's value in rax at runtime. Purely recursive descent —
; there is no separate IR and no bytecode.
global compile_form
compile_form:
    push rbx
    push r12
    push r13
    push r14
    push r15
    ; Consume-on-entry: whatever the caller armed [tail_ctx] with applies
    ; only to THIS form, not to anything compile_form calls transitively
    ; (e.g. compile_call's own argument sub-forms) — so snapshot it into
    ; r15 and zero the global immediately. Any tail-transparent helper
    ; below (compile_progn/if/cond/and/or/let/let*/compile_call) that
    ; wants to forward tail-ness to one of ITS OWN subforms re-arms
    ; [tail_ctx] from its own saved copy right before compiling that one
    ; subform (docs/spec-tco-capture-gc.md section 2).
    mov r15, [tail_ctx]
    mov qword [tail_ctx], 0
    mov rbx, rdi
    call is_cons
    test rax, rax
    jnz .dispatch_list

    ; --- atom: fixnum/NIL literal, or a symbol reference ---
    mov rax, rbx
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .literal
    mov rax, rbx
    UNTAG_PTR rax
    cmp qword [rax], HDR_SYMBOL
    jne .literal
    ; A keyword — a symbol whose own name starts with ":", e.g. :FOO —
    ; self-evaluates, exactly like the reference implementation's own
    ; evaluator (core.rs's check_bindable comment: "keywords are
    ; self-evaluating"). Unconditional and unaffected by any lexical
    ; binding: nothing in this kernel's own compiled code should ever
    ; be able to shadow a keyword's identity, matching Common Lisp's
    ; own keyword package semantics (every :FOO is interned once,
    ; always bound to itself). This is the piece &KEY parameter lists
    ; (lib/prelude.lisp's $EXTENDED-LAMBDA) actually depend on: a call
    ; site writing (F :D 5) needs :D to reach the callee as the tagged
    ; keyword symbol itself, not as an evaluated (and, before this,
    ; always-unbound) variable reference.
    cmp qword [rax+8], 0                    ; name_len
    je .not_keyword
    cmp byte [rax+48], ':'                    ; first name byte
    je .literal
.not_keyword:
    ; local or captured-free reference?
    mov rdi, rbx
    mov rsi, [current_scope]
    call frame_lookup
    cmp rax, FRAME_NOT_FOUND
    je .global_ref
    mov esi, eax
    mov dil, REG_RAX
    call emit_load_local
    jmp .out
.global_ref:
    mov rax, rbx
    UNTAG_PTR rax
    mov rdi, REG_RAX
    lea rsi, [rax+16]                      ; global variable reference
    call emit_load_mem64
    jmp .out
.literal:
    mov rdi, REG_RAX
    mov rsi, rbx
    call emit_mov_reg_imm64
    jmp .out

.dispatch_list:
    mov rdi, rbx
    call car
    mov r12, rax                            ; head
    mov rdi, rbx
    call cdr
    mov r13, rax                             ; rest (args)

    mov rdi, r12
    mov rsi, kw_quote
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_quote
    mov rdi, r13
    call car
    mov rdi, REG_RAX
    mov rsi, rax
    call emit_mov_reg_imm64                    ; bake the datum as-is
    jmp .out

.not_quote:
    mov rdi, r12
    mov rsi, kw_jit_optimize
    mov rdx, 12
    call sym_is
    test rax, rax
    jz .not_jit_optimize
    ; (JIT-OPTIMIZE name) — a real special form in the Rust reference
    ; (jit.rs), taking its symbol operand UNevaluated and attempting HM
    ; inference + native-membrane compilation for an interpreted
    ; closure that was, until that call, tree-walked. This host has no
    ; tree-walking tier at all — `compile_lambda` already turns every
    ; LAMBDA into real native code the moment it is read (see "Why this
    ; exists" above) — so there is no distinct "optimize this" step to
    ; perform: every function here already *is* what JIT-OPTIMIZE would
    ; produce there. A documented no-op returning its own (unevaluated)
    ; operand, exactly like QUOTE above, is the honest behavior for this
    ; architecture, not a stub standing in for missing work — and it is
    ; what makes `lib/00-core.lisp`'s own `defun` macro (`$defun-auto-
    ; compile`, which calls `(eval (list 'jit-optimize name))` after
    ; every definition) loadable at all: every other reference stdlib
    ; file depends on `defun`.
    mov rdi, r13
    call car
    mov rdi, REG_RAX
    mov rsi, rax
    call emit_mov_reg_imm64
    jmp .out

.not_jit_optimize:
    mov rdi, r12
    mov rsi, kw_function
    mov rdx, 8
    call sym_is
    test rax, rax
    jz .not_function
    ; (FUNCTION x) / #'x — x is *not* evaluated as a nested application:
    ; compiling it directly, the same way any other operand position
    ; would, already gives exactly the right answer for both spellings
    ; the spec requires — a bare symbol compiles as the ordinary
    ; local-or-global variable read compile_form's own atom case above
    ; already does (identical to referencing the symbol without
    ; FUNCTION at all, since this kernel keeps no separate function
    ; namespace), and #'(LAMBDA ...) compiles as an ordinary LAMBDA.
    ; v0 does not yet check "is it actually callable, else an error"
    ; (Part VI) — same scope as every other place this kernel doesn't
    ; check a value's type before using it (see README).
    mov rdi, r13
    call car
    mov rdi, rax
    call compile_form
    jmp .out

.not_function:
    mov rdi, r12
    mov rsi, kw_gensym
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_gensym
    ; (GENSYM) — no operand to compile, matching CLEAR-ALL-FLAGS's own
    ; nullary shape; the fresh symbol is entirely a runtime effect of
    ; calling the gensym host routine (symtab.asm), never a compile-time
    ; computation, since two separate (GENSYM) calls at the same call
    ; site (e.g. inside a loop or recursive function) must each return a
    ; genuinely distinct symbol.
    lea rsi, [rel gensym]
    call compile_nullary_hostcall
    jmp .out

.not_gensym:
    ; --- heap/collector observability (docs/spec-tco-capture-gc.md 3.5)
    ; Nullary hostcalls, GENSYM's exact shape. These are the only
    ; program-visible surface the collector has: everything else about
    ; it must be invisible to a running program.
    mov rdi, r12
    mov rsi, kw_heap_bytes_used
    mov rdx, 15
    call sym_is
    test rax, rax
    jz .not_heap_bytes_used
    lea rsi, [rel heap_bytes_used]
    call compile_nullary_hostcall
    jmp .out
.not_heap_bytes_used:
    mov rdi, r12
    mov rsi, kw_heap_bytes_live
    mov rdx, 15
    call sym_is
    test rax, rax
    jz .not_heap_bytes_live
    lea rsi, [rel heap_bytes_live]
    call compile_nullary_hostcall
    jmp .out
.not_heap_bytes_live:
    mov rdi, r12
    mov rsi, kw_gc_verify
    mov rdx, 9
    call sym_is
    test rax, rax
    jz .not_gc_verify
    lea rsi, [rel gc_verify]
    call compile_nullary_hostcall
    jmp .out
.not_gc_verify:
    mov rdi, r12
    mov rsi, kw_gc_collect
    mov rdx, 10
    call sym_is
    test rax, rax
    jz .not_gc_collect
    lea rsi, [rel rc_collect]
    call compile_nullary_hostcall
    jmp .out
.not_gc_collect:
    mov rdi, r12
    mov rsi, kw_refcount
    mov rdx, 8
    call sym_is
    test rax, rax
    jz .not_refcount
    mov rdi, r13
    call car
    mov rdi, rax
    lea rsi, [rel rc_refcount]
    call compile_unary_hostcall
    jmp .out
.not_refcount:
    mov rdi, r12
    mov rsi, kw_symbol_plist
    mov rdx, 12
    call sym_is
    test rax, rax
    jz .not_symbol_plist
    ; (SYMBOL-PLIST sym) — one operand, an ordinary unary hostcall
    ; reading the symbol's plist slot (symtab.asm). GETP/PUTP
    ; themselves are prelude library code over this and
    ; SET-SYMBOL-PLIST! below, per KERNEL.md Part XII axis 3.
    mov rdi, r13
    call car
    mov rdi, rax
    lea rsi, [rel symbol_plist]
    call compile_unary_hostcall
    jmp .out

.not_symbol_plist:
    mov rdi, r12
    mov rsi, kw_set_symbol_plist
    mov rdx, 17
    call sym_is
    test rax, rax
    jz .not_set_symbol_plist
    ; (SET-SYMBOL-PLIST! sym new-plist) — overwrites the symbol's
    ; plist slot outright; PUTP always passes a freshly CONSed pair
    ; onto the front of the existing plist, never any other value.
    mov rdi, r13
    call car                            ; sym form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; new-plist form
    mov rsi, rax
    pop rdi
    lea rdx, [rel set_symbol_plist]
    call compile_binary_hostcall
    jmp .out

.not_set_symbol_plist:
    mov rdi, r12
    mov rsi, kw_set
    mov rdx, 3
    call sym_is
    test rax, rax
    jz .not_set
    ; (SET sym-form val-form) — Lisp 1.5's SET (KERNEL.md/the
    ; reference's own environment.rs builtin): unlike DEFINE/SETQ,
    ; whose target is a literal name known at compile time, SET
    ; evaluates its first operand to find out WHICH symbol to assign
    ; at runtime (lib/29-protocols.lisp's DEFPROTOCOL rebinds a
    ; dynamically-named protocol symbol this way) — an ordinary
    ; evaluated-both-operands hostcall over the new set_symbol_value
    ; primitive (symtab.asm), same shape as SET-SYMBOL-PLIST! just
    ; above.
    mov rdi, r13
    call car                            ; sym form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; val form
    mov rsi, rax
    pop rdi
    lea rdx, [rel set_symbol_value]
    call compile_binary_hostcall
    jmp .out

.not_set:
    ; PORT-* (ports.asm) — lib/31-ports.lisp's synchronous binary ports
    ; over real files, in-memory byte buffers, and stdin/stdout/
    ; stderr. Every one below is an ordinary unary/binary/nullary
    ; hostcall over a genuine Rust-level builtin in the reference
    ; (evaluator/builtins_ports.rs), same idiom as SYMBOL-PLIST/
    ; SET-SYMBOL-PLIST!/RECORD-BRAND above.
    mov rdi, r12
    mov rsi, kw_port_open_input_file
    mov rdx, 21
    call sym_is
    test rax, rax
    jz .not_port_open_input_file
    mov rdi, r13
    call car
    lea rsi, [rel port_open_input_file_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_open_input_file:
    mov rdi, r12
    mov rsi, kw_port_open_output_file
    mov rdx, 22
    call sym_is
    test rax, rax
    jz .not_port_open_output_file
    mov rdi, r13
    call car
    lea rsi, [rel port_open_output_file_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_open_output_file:
    mov rdi, r12
    mov rsi, kw_port_open_append_file
    mov rdx, 22
    call sym_is
    test rax, rax
    jz .not_port_open_append_file
    mov rdi, r13
    call car
    lea rsi, [rel port_open_append_file_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_open_append_file:
    mov rdi, r12
    mov rsi, kw_port_open_input_bytes
    mov rdx, 22
    call sym_is
    test rax, rax
    jz .not_port_open_input_bytes
    mov rdi, r13
    call car
    lea rsi, [rel port_open_input_bytes_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_open_input_bytes:
    mov rdi, r12
    mov rsi, kw_port_open_output_bytes
    mov rdx, 23
    call sym_is
    test rax, rax
    jz .not_port_open_output_bytes
    lea rsi, [rel port_open_output_bytes_tagged]
    call compile_nullary_hostcall
    jmp .out

.not_port_open_output_bytes:
    mov rdi, r12
    mov rsi, kw_port_output_contents
    mov rdx, 21
    call sym_is
    test rax, rax
    jz .not_port_output_contents
    mov rdi, r13
    call car
    lea rsi, [rel port_output_contents_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_output_contents:
    mov rdi, r12
    mov rsi, kw_port_stdin
    mov rdx, 11
    call sym_is
    test rax, rax
    jz .not_port_stdin
    lea rsi, [rel port_stdin_tagged]
    call compile_nullary_hostcall
    jmp .out

.not_port_stdin:
    mov rdi, r12
    mov rsi, kw_port_stdout
    mov rdx, 12
    call sym_is
    test rax, rax
    jz .not_port_stdout
    lea rsi, [rel port_stdout_tagged]
    call compile_nullary_hostcall
    jmp .out

.not_port_stdout:
    mov rdi, r12
    mov rsi, kw_port_stderr
    mov rdx, 12
    call sym_is
    test rax, rax
    jz .not_port_stderr
    lea rsi, [rel port_stderr_tagged]
    call compile_nullary_hostcall
    jmp .out

.not_port_stderr:
    mov rdi, r12
    mov rsi, kw_port_read_byte
    mov rdx, 15
    call sym_is
    test rax, rax
    jz .not_port_read_byte
    mov rdi, r13
    call car
    lea rsi, [rel port_read_byte_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_read_byte:
    mov rdi, r12
    mov rsi, kw_port_read_bytes
    mov rdx, 16
    call sym_is
    test rax, rax
    jz .not_port_read_bytes
    mov rdi, r13
    call car
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car
    mov rsi, rax
    pop rdi
    lea rdx, [rel port_read_bytes_tagged]
    call compile_binary_hostcall
    jmp .out

.not_port_read_bytes:
    mov rdi, r12
    mov rsi, kw_port_write_byte
    mov rdx, 16
    call sym_is
    test rax, rax
    jz .not_port_write_byte
    mov rdi, r13
    call car
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car
    mov rsi, rax
    pop rdi
    lea rdx, [rel port_write_byte_tagged]
    call compile_binary_hostcall
    jmp .out

.not_port_write_byte:
    mov rdi, r12
    mov rsi, kw_port_write_bytes
    mov rdx, 17
    call sym_is
    test rax, rax
    jz .not_port_write_bytes
    mov rdi, r13
    call car
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car
    mov rsi, rax
    pop rdi
    lea rdx, [rel port_write_bytes_tagged]
    call compile_binary_hostcall
    jmp .out

.not_port_write_bytes:
    mov rdi, r12
    mov rsi, kw_port_flush
    mov rdx, 11
    call sym_is
    test rax, rax
    jz .not_port_flush
    mov rdi, r13
    call car
    lea rsi, [rel port_flush_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_flush:
    mov rdi, r12
    mov rsi, kw_port_close
    mov rdx, 11
    call sym_is
    test rax, rax
    jz .not_port_close
    mov rdi, r13
    call car
    lea rsi, [rel port_close_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_close:
    mov rdi, r12
    mov rsi, kw_port_open_p
    mov rdx, 12
    call sym_is
    test rax, rax
    jz .not_port_open_p
    mov rdi, r13
    call car
    lea rsi, [rel port_open_p_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_open_p:
    mov rdi, r12
    mov rsi, kw_port_input_p
    mov rdx, 13
    call sym_is
    test rax, rax
    jz .not_port_input_p
    mov rdi, r13
    call car
    lea rsi, [rel port_input_p_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_input_p:
    mov rdi, r12
    mov rsi, kw_port_output_p
    mov rdx, 14
    call sym_is
    test rax, rax
    jz .not_port_output_p
    mov rdi, r13
    call car
    lea rsi, [rel port_output_p_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_output_p:
    mov rdi, r12
    mov rsi, kw_port_seekable_p
    mov rdx, 16
    call sym_is
    test rax, rax
    jz .not_port_seekable_p
    mov rdi, r13
    call car
    lea rsi, [rel port_seekable_p_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_seekable_p:
    mov rdi, r12
    mov rsi, kw_port_position
    mov rdx, 14
    call sym_is
    test rax, rax
    jz .not_port_position
    mov rdi, r13
    call car
    lea rsi, [rel port_position_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_position:
    mov rdi, r12
    mov rsi, kw_port_seek
    mov rdx, 10
    call sym_is
    test rax, rax
    jz .not_port_seek
    mov rdi, r13
    call car
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car
    mov rsi, rax
    pop rdi
    lea rdx, [rel port_seek_tagged]
    call compile_binary_hostcall
    jmp .out

.not_port_seek:
    mov rdi, r12
    mov rsi, kw_port_p
    mov rdx, 7
    call sym_is
    test rax, rax
    jz .not_port_p
    mov rdi, r13
    call car
    lea rsi, [rel port_p_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_p:
    mov rdi, r12
    mov rsi, kw_port_name
    mov rdx, 10
    call sym_is
    test rax, rax
    jz .not_port_name
    mov rdi, r13
    call car
    lea rsi, [rel port_name_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_name:
    mov rdi, r12
    mov rsi, kw_port_kind
    mov rdx, 10
    call sym_is
    test rax, rax
    jz .not_port_kind
    mov rdi, r13
    call car
    lea rsi, [rel port_kind_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_port_kind:
    mov rdi, r12
    mov rsi, kw_apply
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_apply
    ; (APPLY fn args-list) — invoke_macro (above) already does exactly
    ; this job for macro expansion: given a closure and a raw list, it
    ; collects the list's own elements as argument *values* (car/cdr
    ; traversal, no compile_form involved) into the same rsi/rdx/rcx +
    ; stack layout an ordinary compiled call site would produce, then
    ; calls the closure with the right nargs. That's every bit of what
    ; APPLY needs too — the only difference from macro expansion is
    ; that here args-list holds already-*evaluated* values instead of
    ; unevaluated operand forms, which invoke_macro never distinguishes
    ; (it never compiles or evaluates anything itself either way). No
    ; new kernel mechanism needed, just this compile_binary_hostcall
    ; wiring — fn and args-list are ordinary operand *expressions* here
    ; (unlike a macro call site's raw syntax), so both compile normally.
    mov rdi, r13
    call car                            ; fn form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; args-list form
    mov rsi, rax
    pop rdi
    lea rdx, [rel invoke_macro]
    call compile_binary_hostcall
    jmp .out

.not_apply:
    mov rdi, r12
    mov rsi, kw_if
    mov rdx, 2
    call sym_is
    test rax, rax
    jz .not_if
    mov rdi, rbx
    mov rsi, r15
    call compile_if
    jmp .out

.not_if:
    mov rdi, r12
    mov rsi, kw_define
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_define
    mov rdi, rbx
    call compile_define
    jmp .out

.not_define:
    mov rdi, r12
    mov rsi, kw_defdynamic
    mov rdx, 10
    call sym_is
    test rax, rax
    jz .not_defdynamic
    mov rdi, rbx
    call compile_defdynamic
    jmp .out

.not_defdynamic:
    mov rdi, r12
    mov rsi, kw_vau
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_vau
    mov rdi, rbx
    call compile_vau
    jmp .out

.not_vau:
    mov rdi, r12
    mov rsi, kw_defexpr
    mov rdx, 7
    call sym_is
    test rax, rax
    jz .not_defexpr
    mov rdi, rbx
    call compile_defexpr
    jmp .out

.not_defexpr:
    mov rdi, r12
    mov rsi, kw_lambda
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_lambda
    mov rdi, rbx
    call compile_lambda
    jmp .out

.not_lambda:
    mov rdi, r12
    mov rsi, kw_defmacro
    mov rdx, 8
    call sym_is
    test rax, rax
    jz .not_defmacro
    mov rdi, rbx
    call compile_defmacro
    jmp .out

.not_defmacro:
    mov rdi, r12
    mov rsi, kw_catch
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_catch
    mov rdi, rbx
    call compile_catch
    jmp .out

.not_catch:
    mov rdi, r12
    mov rsi, kw_throw
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_throw
    mov rdi, rbx
    call compile_throw
    jmp .out

.not_throw:
    mov rdi, r12
    mov rsi, kw_cons
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_cons
    mov rdi, r13
    call car
    mov r14, rax                       ; arg1 form
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car
    mov rsi, rax                         ; arg2 form
    mov rdi, r14
    lea rdx, [rel cons]
    call compile_binary_hostcall
    jmp .out

.not_cons:
    mov rdi, r12
    mov rsi, kw_car
    mov rdx, 3
    call sym_is
    test rax, rax
    jz .not_car
    mov rdi, r13
    call car
    mov rdi, rax
    lea rsi, [rel car]
    call compile_unary_hostcall
    jmp .out

.not_car:
    mov rdi, r12
    mov rsi, kw_cdr
    mov rdx, 3
    call sym_is
    test rax, rax
    jz .not_cdr
    mov rdi, r13
    call car
    mov rdi, rax
    lea rsi, [rel cdr]
    call compile_unary_hostcall
    jmp .out

.not_cdr:
    mov rdi, r12
    mov rsi, kw_eqp
    mov rdx, 2
    call sym_is
    test rax, rax
    jz .not_eqp
    mov rdi, r13
    call car
    mov r14, rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car
    mov rsi, rax
    mov rdi, r14
    call compile_eq
    jmp .out

.not_eqp:
    mov rdi, r12
    mov rsi, kw_atom
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_atom
    mov rdi, r13
    call car
    mov rdi, rax
    call compile_atom
    jmp .out

.not_atom:
    mov rdi, r12
    mov rsi, kw_nullp
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_nullp
    mov rdi, r13
    call car
    mov rdi, rax
    call compile_nullp
    jmp .out

.not_nullp:
    mov rdi, r12
    mov rsi, kw_print
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_print
    mov rdi, r13
    call car
    mov rdi, rax
    call compile_print
    jmp .out

.not_print:
    mov rdi, r12
    mov rsi, kw_newline
    mov rdx, 7
    call sym_is
    test rax, rax
    jz .not_newline
    call compile_newline_form
    jmp .out

.not_newline:
    mov rdi, r12
    mov rsi, kw_string_length
    mov rdx, 13
    call sym_is
    test rax, rax
    jz .not_string_length
    mov rdi, r13
    call car
    lea rsi, [rel string_length_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_string_length:
    mov rdi, r12
    mov rsi, kw_string_length_star
    mov rdx, 14
    call sym_is
    test rax, rax
    jz .not_string_length_star
    ; (STRING-LENGTH* s) — the reference's own actual name for this
    ; primitive (environment.rs registers only "STRING-LENGTH*", never
    ; a bare "STRING-LENGTH"); lib/14-strings.lisp's own STRING-INDEX-OF
    ; calls it under this exact spelling. Same host routine STRING-LENGTH
    ; already uses — this kernel just also answers to the reference's
    ; own name, matching v0's existing STRING-LENGTH alias rather than
    ; replacing it (README's own "v0 limits" tracks naming gaps like
    ; this honestly rather than silently renaming established surface).
    mov rdi, r13
    call car
    lea rsi, [rel string_length_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_string_length_star:
    mov rdi, r12
    mov rsi, kw_string_ref
    mov rdx, 10
    call sym_is
    test rax, rax
    jz .not_string_ref
    mov rdi, r13
    call car                            ; string form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; index form
    mov rsi, rax
    pop rdi
    lea rdx, [rel string_ref_tagged]
    call compile_binary_hostcall
    jmp .out

.not_string_ref:
    mov rdi, r12
    mov rsi, kw_string_append
    mov rdx, 13
    call sym_is
    test rax, rax
    jz .not_string_append
    mov rdi, r13
    call car                            ; string A form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; string B form
    mov rsi, rax
    pop rdi
    lea rdx, [rel string_append]
    call compile_binary_hostcall
    jmp .out

.not_string_append:
    mov rdi, r12
    mov rsi, kw_substring
    mov rdx, 9
    call sym_is
    test rax, rax
    jz .not_substring
    mov rdi, r13
    call car                            ; string form
    push rax
    mov rdi, r13
    call cdr
    mov rbx, rax                          ; (start-form end-form)
    mov rdi, rbx
    call car                                ; start form
    push rax
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call car                                  ; end form
    mov rdx, rax
    pop rsi                                     ; start form
    pop rdi                                       ; string form
    lea rcx, [rel substring]
    call compile_ternary_hostcall
    jmp .out

.not_substring:
    mov rdi, r12
    mov rsi, kw_read_from_string
    mov rdx, 16
    call sym_is
    test rax, rax
    jz .not_read_from_string
    mov rdi, r13
    call car
    lea rsi, [rel read_from_string_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_read_from_string:
    mov rdi, r12
    mov rsi, kw_princ_to_string
    mov rdx, 15
    call sym_is
    test rax, rax
    jz .not_princ_to_string
    mov rdi, r13
    call car
    lea rsi, [rel princ_to_string]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_princ_to_string:
    mov rdi, r12
    mov rsi, kw_eval
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_eval
    mov rdi, r13
    call car
    lea rsi, [rel eval_form]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_eval:
    mov rdi, r12
    mov rsi, kw_fd_open
    mov rdx, 7
    call sym_is
    test rax, rax
    jz .not_fd_open
    mov rdi, r13
    call car                            ; path form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; mode form
    mov rsi, rax
    pop rdi
    lea rdx, [rel file_open]
    call compile_binary_hostcall
    jmp .out

.not_fd_open:
    mov rdi, r12
    mov rsi, kw_fd_close
    mov rdx, 8
    call sym_is
    test rax, rax
    jz .not_fd_close
    mov rdi, r13
    call car
    lea rsi, [rel file_close]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_fd_close:
    mov rdi, r12
    mov rsi, kw_fd_write
    mov rdx, 8
    call sym_is
    test rax, rax
    jz .not_fd_write
    mov rdi, r13
    call car                            ; fd form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; string form
    mov rsi, rax
    pop rdi
    lea rdx, [rel file_write]
    call compile_binary_hostcall
    jmp .out

.not_fd_write:
    mov rdi, r12
    mov rsi, kw_fd_read
    mov rdx, 7
    call sym_is
    test rax, rax
    jz .not_fd_read
    mov rdi, r13
    call car                            ; fd form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; max-len form
    mov rsi, rax
    pop rdi
    lea rdx, [rel file_read]
    call compile_binary_hostcall
    jmp .out

.not_fd_read:
    mov rdi, r12
    mov rsi, kw_float
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_float
    mov rdi, r13
    call car
    lea rsi, [rel float_of_fixnum]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_float:
    mov rdi, r12
    mov rsi, kw_fadd
    mov rdx, 2
    call sym_is
    test rax, rax
    jz .not_fadd
    lea rdx, [rel float_add]
    jmp .do_float_binop
.not_fadd:
    mov rdi, r12
    mov rsi, kw_fsub
    mov rdx, 2
    call sym_is
    test rax, rax
    jz .not_fsub
    lea rdx, [rel float_sub]
    jmp .do_float_binop
.not_fsub:
    mov rdi, r12
    mov rsi, kw_fmul
    mov rdx, 2
    call sym_is
    test rax, rax
    jz .not_fmul
    lea rdx, [rel float_mul]
    jmp .do_float_binop
.not_fmul:
    mov rdi, r12
    mov rsi, kw_fdiv
    mov rdx, 2
    call sym_is
    test rax, rax
    jz .not_fdiv
    lea rdx, [rel float_div]
    jmp .do_float_binop
.not_fdiv:
    mov rdi, r12
    mov rsi, kw_flt
    mov rdx, 2
    call sym_is
    test rax, rax
    jz .not_flt
    lea rdx, [rel float_lt]
    jmp .do_float_binop
.not_flt:
    jmp .not_float_binop

.do_float_binop:
    mov r14, rdx                          ; host fn addr, across the two `car`s below
    mov rdi, r13
    call car                                ; arg1 form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                                  ; arg2 form
    mov rsi, rax
    pop rdi
    mov rdx, r14
    call compile_binary_hostcall
    jmp .out

.not_float_binop:
    mov rdi, r12
    mov rsi, kw_make_array
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_make_array
    mov rdi, r13
    call car
    lea rsi, [rel make_array]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_make_array:
    mov rdi, r12
    mov rsi, kw_typed_array
    mov rdx, 11
    call sym_is
    test rax, rax
    jz .not_typed_array
    mov rdi, r13
    call car
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; elem-type form
    mov rsi, rax
    pop rdi
    lea rdx, [rel make_typed_array]
    call compile_binary_hostcall
    jmp .out

.not_typed_array:
    mov rdi, r12
    mov rsi, kw_array_length
    mov rdx, 13
    call sym_is
    test rax, rax
    jz .not_array_length
    mov rdi, r13
    call car
    lea rsi, [rel array_length_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_array_length:
    mov rdi, r12
    mov rsi, kw_hash_code
    mov rdx, 9
    call sym_is
    test rax, rax
    jz .not_hash_code
    mov rdi, r13
    call car
    lea rsi, [rel hash_code_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_hash_code:
    mov rdi, r12
    mov rsi, kw_make_char
    mov rdx, 9
    call sym_is
    test rax, rax
    jz .not_make_char
    mov rdi, r13
    call car
    lea rsi, [rel make_char_from_fixnum]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_make_char:
    mov rdi, r12
    mov rsi, kw_char_code
    mov rdx, 9
    call sym_is
    test rax, rax
    jz .not_char_code
    mov rdi, r13
    call car
    lea rsi, [rel char_code_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_char_code:
    mov rdi, r12
    mov rsi, kw_stringp
    mov rdx, 7
    call sym_is
    test rax, rax
    jz .not_stringp
    ; (STRINGP x) — a new kernel primitive (compile_unary_hostcall over
    ; strings.asm's stringp_tagged, itself a thin T/NIL wrapper around
    ; the existing internal is_string check `print_value`/`lisp_eq`
    ; already use). Needed by lib/00-core.lisp's own DEFUN macro,
    ; unmodified, which checks `(stringp (car body))` on every
    ; expansion to peel off an optional leading docstring.
    mov rdi, r13
    call car
    lea rsi, [rel stringp_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_stringp:
    mov rdi, r12
    mov rsi, kw_symbolp
    mov rdx, 7
    call sym_is
    test rax, rax
    jz .not_symbolp
    ; (SYMBOLP x) — a genuine Rust-level builtin in the reference
    ; (environment.rs), missing here until now; lib/06-require.lisp's
    ; own `$require-canonical-name` needs it.
    mov rdi, r13
    call car
    lea rsi, [rel symbolp_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_symbolp:
    mov rdi, r12
    mov rsi, kw_module_source_lookup
    mov rdx, 21
    call sym_is
    test rax, rax
    jz .not_module_source_lookup
    ; ($MODULE-SOURCE-LOOKUP name-string) -- the embedded half of
    ; REQUIRE's module resolution (modules.asm); a genuine Rust-level
    ; builtin in the reference (environment.rs), needed by
    ; lib/06-require.lisp's own $require-resolve.
    mov rdi, r13
    call car
    lea rsi, [rel module_source_lookup_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_module_source_lookup:
    mov rdi, r12
    mov rsi, kw_eval_module_source
    mov rdx, 19
    call sym_is
    test rax, rax
    jz .not_eval_module_source
    ; ($EVAL-MODULE-SOURCE name-string source-string) — the other half
    ; of REQUIRE's module loading (modules.asm), a genuine Rust-level
    ; builtin in the reference: parses and evaluates every top-level
    ; form in source-string, exactly the way file_runner.asm's own
    ; run_buffer does for a real file.
    mov rdi, r13
    call car                            ; name form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; source form
    mov rsi, rax
    pop rdi
    lea rdx, [rel eval_module_source_tagged]
    call compile_binary_hostcall
    jmp .out

.not_eval_module_source:
    mov rdi, r12
    mov rsi, kw_intern
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_intern
    ; (INTERN x) — a genuine Rust-level builtin (environment.rs),
    ; needed by lib/27-modules.lisp's own $MODULE-QUALIFY.
    mov rdi, r13
    call car
    lea rsi, [rel intern_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_intern:
    mov rdi, r12
    mov rsi, kw_record_new
    mov rdx, 10
    call sym_is
    test rax, rax
    jz .not_record_new
    ; (RECORD-NEW brand-form field-form...) — see compile_record_new's
    ; own comment.
    mov rdi, r13
    call compile_record_new
    jmp .out

.not_record_new:
    mov rdi, r12
    mov rsi, kw_record_brand
    mov rdx, 12
    call sym_is
    test rax, rax
    jz .not_record_brand
    mov rdi, r13
    call car
    lea rsi, [rel record_brand_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_record_brand:
    mov rdi, r12
    mov rsi, kw_record_fields
    mov rdx, 13
    call sym_is
    test rax, rax
    jz .not_record_fields
    mov rdi, r13
    call car
    lea rsi, [rel record_fields_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_record_fields:
    mov rdi, r12
    mov rsi, kw_closure_nfree
    mov rdx, 13
    call sym_is
    test rax, rax
    jz .not_closure_nfree
    ; (CLOSURE-NFREE f) — how many free variables the closure f actually
    ; captured, straight out of its own HDR_CLOSURE [24] field
    ; (closure_nfree_tagged); NIL for a non-closure. A debug/
    ; introspection primitive, not a KERNEL.md form: it exists so the
    ; capture analysis (analyze_lambda_captures, above) can be tested
    ; for what it *doesn't* capture. An over-captured slot is never
    ; read, so without this every shadowing bug and every shadowing fix
    ; alike prints the same right answer.
    mov rdi, r13
    call car
    lea rsi, [rel closure_nfree_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_closure_nfree:
    mov rdi, r12
    mov rsi, kw_boundp
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_boundp
    ; (BOUNDP sym) — sym is an ordinary evaluated operand (the reference
    ; caller side always passes an explicit `(quote name)`, never bare
    ; unevaluated syntax, unlike DEF's NAME); a new kernel primitive
    ; (compile_unary_hostcall over symtab.asm's boundp_tagged) needed by
    ; lib/00-core.lisp's own DEFUN macro to guard its optional
    ; call-graph bookkeeping globals.
    mov rdi, r13
    call car
    lea rsi, [rel boundp_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_boundp:
    mov rdi, r12
    mov rsi, kw_code_char
    mov rdx, 9
    call sym_is
    test rax, rax
    jz .not_code_char
    mov rdi, r13
    call car
    lea rsi, [rel code_char_string]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_code_char:
    mov rdi, r12
    mov rsi, kw_random
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_random
    mov rdi, r13
    call car
    lea rsi, [rel random_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_random:
    mov rdi, r12
    mov rsi, kw_random_seed
    mov rdx, 12
    call sym_is
    test rax, rax
    jz .not_random_seed
    mov rdi, r13
    call car
    lea rsi, [rel random_seed_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_random_seed:
    mov rdi, r12
    mov rsi, kw_lognot
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_lognot
    mov rdi, r13
    call car
    lea rsi, [rel lognot_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_lognot:
    mov rdi, r12
    mov rsi, kw_logand
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_logand
    ; LOGAND/LOGIOR/LOGXOR (KERNEL.md Part XI): the reference's own
    ; builtins take any number of operands (a variadic fold); this
    ; dispatch used to just take car/cadr of the operand list and
    ; silently ignore anything past the second (same bug shape MAX/MIN
    ; had, lib/prelude.lisp, fixed there as a Lisp-level fold — these
    ; can't be, being real compiler special forms). More than two
    ; operands now folds host-side into a nested 2-operand AST
    ; (`(LOGAND (LOGAND a b) c)`, fold_binop_ast below) and recompiles
    ; that in this call's place, terminating at the exact-2-operand
    ; base case below, unchanged.
    mov rdi, r13
    call cdr
    mov rdi, rax
    call cdr
    cmp rax, IMM_NIL
    jne .logand_fold
    mov rdi, r13
    call car
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car
    mov rsi, rax
    pop rdi
    lea rdx, [rel logand_tagged]
    call compile_binary_hostcall
    jmp .out
.logand_fold:
    mov rdi, r12
    mov rsi, r13
    call fold_binop_ast
    mov rdi, rax
    call compile_form
    jmp .out

.not_logand:
    mov rdi, r12
    mov rsi, kw_logior
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_logior
    mov rdi, r13
    call cdr
    mov rdi, rax
    call cdr
    cmp rax, IMM_NIL
    jne .logior_fold
    mov rdi, r13
    call car
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car
    mov rsi, rax
    pop rdi
    lea rdx, [rel logior_tagged]
    call compile_binary_hostcall
    jmp .out
.logior_fold:
    mov rdi, r12
    mov rsi, r13
    call fold_binop_ast
    mov rdi, rax
    call compile_form
    jmp .out

.not_logior:
    mov rdi, r12
    mov rsi, kw_logxor
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_logxor
    mov rdi, r13
    call cdr
    mov rdi, rax
    call cdr
    cmp rax, IMM_NIL
    jne .logxor_fold
    mov rdi, r13
    call car
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car
    mov rsi, rax
    pop rdi
    lea rdx, [rel logxor_tagged]
    call compile_binary_hostcall
    jmp .out
.logxor_fold:
    mov rdi, r12
    mov rsi, r13
    call fold_binop_ast
    mov rdi, rax
    call compile_form
    jmp .out

.not_logxor:
    mov rdi, r12
    mov rsi, kw_ash
    mov rdx, 3
    call sym_is
    test rax, rax
    jz .not_ash
    mov rdi, r13
    call car
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car
    mov rsi, rax
    pop rdi
    lea rdx, [rel ash_tagged]
    call compile_binary_hostcall
    jmp .out

.not_ash:
    mov rdi, r12
    mov rsi, kw_feature_enabled_p
    mov rdx, 17
    call sym_is
    test rax, rax
    jz .not_feature_enabled_p
    mov rdi, r13
    call car
    lea rsi, [rel feature_enabled_p]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_feature_enabled_p:
    mov rdi, r12
    mov rsi, kw_capability_mask_allows_p
    mov rdx, 24
    call sym_is
    test rax, rax
    jz .not_capability_mask_allows_p
    mov rdi, r13
    call car
    lea rsi, [rel capability_mask_allows_p]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_capability_mask_allows_p:
    mov rdi, r12
    mov rsi, kw_push_capability_mask
    mov rdx, 21
    call sym_is
    test rax, rax
    jz .not_push_capability_mask
    mov rdi, r13
    call car
    lea rsi, [rel push_capability_mask]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_push_capability_mask:
    mov rdi, r12
    mov rsi, kw_pop_capability_mask
    mov rdx, 20
    call sym_is
    test rax, rax
    jz .not_pop_capability_mask
    lea rsi, [rel pop_capability_mask]
    call compile_nullary_hostcall
    jmp .out

.not_pop_capability_mask:
    mov rdi, r12
    mov rsi, kw_array_ref
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_array_ref
    mov rdi, r13
    call car                            ; array form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; index form
    mov rsi, rax
    pop rdi
    lea rdx, [rel array_ref]
    call compile_binary_hostcall
    jmp .out

.not_array_ref:
    mov rdi, r12
    mov rsi, kw_mod
    mov rdx, 3
    call sym_is
    test rax, rax
    jz .not_mod
    mov rdi, r13
    call car                            ; a form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; b form
    mov rsi, rax
    pop rdi
    lea rdx, [rel mod_tagged]
    call compile_binary_hostcall
    jmp .out

.not_mod:
    mov rdi, r12
    mov rsi, kw_remainder
    mov rdx, 9
    call sym_is
    test rax, rax
    jz .not_remainder
    mov rdi, r13
    call car                            ; a form
    push rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                              ; b form
    mov rsi, rax
    pop rdi
    lea rdx, [rel remainder_tagged]
    call compile_binary_hostcall
    jmp .out

.not_remainder:
    mov rdi, r12
    mov rsi, kw_array_set
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_array_set
    mov rdi, r13
    call car                            ; array form
    push rax
    mov rdi, r13
    call cdr
    mov rbx, rax                          ; (index-form val-form)
    mov rdi, rbx
    call car                                ; index form
    push rax
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call car                                  ; val form
    mov rdx, rax
    pop rsi                                     ; index form
    pop rdi                                       ; array form
    lea rcx, [rel array_set]
    call compile_ternary_hostcall
    jmp .out

.not_array_set:
    mov rdi, r12
    mov rsi, kw_flag_set_p
    mov rdx, 10
    call sym_is
    test rax, rax
    jz .not_flag_set_p
    mov rdi, r13
    call car
    lea rsi, [rel flag_set_p]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_flag_set_p:
    mov rdi, r12
    mov rsi, kw_clear_flag
    mov rdx, 10
    call sym_is
    test rax, rax
    jz .not_clear_flag
    mov rdi, r13
    call car
    lea rsi, [rel clear_flag]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_clear_flag:
    mov rdi, r12
    mov rsi, kw_clear_all_flags
    mov rdx, 15
    call sym_is
    test rax, rax
    jz .not_clear_all_flags
    lea rsi, [rel clear_all_flags]
    call compile_nullary_hostcall
    jmp .out

.not_clear_all_flags:
    mov rdi, r12
    mov rsi, kw_progn
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_progn
    mov rdi, rbx
    call cdr
    mov rdi, rax
    mov rsi, r15
    call compile_progn
    jmp .out

.not_progn:
    mov rdi, r12
    mov rsi, kw_cond
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_cond
    mov rdi, rbx
    mov rsi, r15
    call compile_cond
    jmp .out

.not_cond:
    mov rdi, r12
    mov rsi, kw_and
    mov rdx, 3
    call sym_is
    test rax, rax
    jz .not_and
    mov rdi, rbx
    mov rsi, r15
    call compile_and
    jmp .out

.not_and:
    mov rdi, r12
    mov rsi, kw_or
    mov rdx, 2
    call sym_is
    test rax, rax
    jz .not_or
    mov rdi, rbx
    mov rsi, r15
    call compile_or
    jmp .out

.not_or:
    mov rdi, r12
    mov rsi, kw_let_star
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_let_star
    mov rdi, rbx
    mov rsi, r15
    call compile_let_star
    jmp .out

.not_let_star:
    mov rdi, r12
    mov rsi, kw_let
    mov rdx, 3
    call sym_is
    test rax, rax
    jz .not_let
    mov rdi, rbx
    mov rsi, r15
    call compile_let
    jmp .out

.not_let:
    mov rdi, r12
    mov rsi, kw_setq
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_setq
    mov rdi, rbx
    call compile_setq
    jmp .out

.not_setq:
    mov rdi, r12
    mov rsi, kw_handler_case
    mov rdx, 12
    call sym_is
    test rax, rax
    jz .not_handler_case
    mov rdi, rbx
    call compile_handler_case
    jmp .out

.not_handler_case:
    mov rdi, r12
    mov rsi, kw_errorset
    mov rdx, 8
    call sym_is
    test rax, rax
    jz .not_errorset
    mov rdi, rbx
    call compile_errorset
    jmp .out

.not_errorset:
    mov rdi, r12
    mov rsi, kw_unwind_protect
    mov rdx, 14
    call sym_is
    test rax, rax
    jz .not_unwind_protect
    mov rdi, rbx
    call compile_unwind_protect
    jmp .out

.not_unwind_protect:
    mov rdi, r12
    mov rsi, kw_error
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_error
    mov rdi, rbx
    call compile_error
    jmp .out

.not_error:
    mov rdi, r12
    mov rsi, kw_error_p
    mov rdx, 7
    call sym_is
    test rax, rax
    jz .not_error_p
    mov rdi, r13
    call car
    lea rsi, [rel is_condition]
    mov rdi, rax
    call compile_unary_hostcall           ; target rax = raw 0/1
    call bool_from_al                        ; -> IMM_NIL/IMM_TRUE (AL
                                              ; already holds that 0/1,
                                              ; no fresh set* needed)
    jmp .out

.not_error_p:
    mov rdi, r12
    mov rsi, kw_error_message
    mov rdx, 13
    call sym_is
    test rax, rax
    jz .not_error_message
    mov rdi, r13
    call car
    lea rsi, [rel error_message_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_error_message:
    mov rdi, r12
    mov rsi, kw_error_data
    mov rdx, 10
    call sym_is
    test rax, rax
    jz .not_error_data
    mov rdi, r13
    call car
    lea rsi, [rel error_data_tagged]
    mov rdi, rax
    call compile_unary_hostcall
    jmp .out

.not_error_data:
    mov rdi, r12
    mov rsi, kw_block
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_block
    mov rdi, rbx
    call compile_block
    jmp .out

.not_block:
    mov rdi, r12
    mov rsi, kw_return_from
    mov rdx, 11
    call sym_is
    test rax, rax
    jz .not_return_from
    mov rdi, rbx
    call compile_return_from
    jmp .out

.not_return_from:
    mov rdi, r12
    mov rsi, kw_prog
    mov rdx, 4
    call sym_is
    test rax, rax
    jz .not_prog
    mov rdi, rbx
    call compile_prog
    jmp .out

.not_prog:
    mov rdi, r12
    mov rsi, kw_go
    mov rdx, 2
    call sym_is
    test rax, rax
    jz .not_go
    mov rdi, rbx
    call compile_go
    jmp .out

.not_go:
    mov rdi, r12
    mov rsi, kw_return
    mov rdx, 6
    call sym_is
    test rax, rax
    jz .not_return
    mov rdi, rbx
    call compile_return
    jmp .out

.not_return:
    mov rdi, r12
    mov rsi, kw_while
    mov rdx, 5
    call sym_is
    test rax, rax
    jz .not_while
    mov rdi, rbx
    call compile_while
    jmp .out

.not_while:
    mov rdi, r12
    mov rsi, kw_add
    mov rdx, 1
    call sym_is
    test rax, rax
    jz .try_sub
    mov r14b, '+'
    jmp .do_binop
.try_sub:
    mov rdi, r12
    mov rsi, kw_sub
    mov rdx, 1
    call sym_is
    test rax, rax
    jz .try_mul
    mov r14b, '-'
    jmp .do_binop
.try_mul:
    mov rdi, r12
    mov rsi, kw_mul
    mov rdx, 1
    call sym_is
    test rax, rax
    jz .try_lt
    mov r14b, '*'
    jmp .do_binop
.try_lt:
    mov rdi, r12
    mov rsi, kw_lt
    mov rdx, 1
    call sym_is
    test rax, rax
    jz .try_eq
    mov r14b, '<'
    jmp .do_binop
.try_eq:
    mov rdi, r12
    mov rsi, kw_eq
    mov rdx, 1
    call sym_is
    test rax, rax
    jz .unsupported
    mov r14b, '='
    jmp .do_binop

.do_binop:
    mov rdi, r13
    call car                                 ; lhs form
    mov r12, rax
    mov rdi, r13
    call cdr
    mov rdi, rax
    call car                                  ; rhs form
    mov rsi, rax
    mov rdi, r12
    mov dl, r14b
    call compile_binop
    jmp .out

.unsupported:
    ; Is the head a symbol registered as a macro? Expansion is a
    ; compile-time (host-time) computation: the transformer is invoked
    ; right now, synchronously, from host code — not emitted into the
    ; target program — with the raw, unevaluated argument forms, and
    ; whatever it returns is recursively compiled in its place.
    mov rax, r12
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .not_macro_call
    mov rax, r12
    UNTAG_PTR rax
    cmp qword [rax], HDR_SYMBOL
    jne .not_macro_call
    mov rax, [rax+24]                    ; macro slot
    cmp rax, IMM_NIL
    je .not_macro_call
    mov r14, rax                            ; macro closure (tagged)
    mov rdi, rbx                               ; call form (memo key)
    mov rsi, r14
    mov rdx, r13
    call macroexpand_once                        ; rax = expansion
    mov rdi, rax
    call compile_form                           ; recompile in its place
    jmp .out

.not_macro_call:
    ; Is the head symbol's global VALUE currently an Operative
    ; ($VAU/DEFVAU — KERNEL.md's vau combiner)? If so this call must
    ; NOT evaluate its operands: an operative's whole point is
    ; receiving the raw, unevaluated argument forms as an ordinary
    ; list (exactly like a macro's raw args) plus a placeholder
    ; "caller's environment" value (compile_vau's own comment explains
    ; why a fixed placeholder, not a real one, is this kernel's honest
    ; v0 answer). Building `(name (QUOTE raw-args) (QUOTE sentinel))`
    ; and delegating to compile_call, rather than hand-rolling a second
    ; calling convention, reuses its entire self-patching inline-cache
    ; machinery unchanged — an operative call site is just an ordinary
    ; 2-argument call whose two arguments happen to be QUOTE'd data
    ; instead of expressions to evaluate. A frame_lookup guard first
    ; (matching compile_call's own "locally bound — not a global call"
    ; check) keeps a local variable or parameter that happens to share
    ; an operative's name from being misread as one.
    mov rax, r12
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .not_operative_call
    mov rax, r12
    UNTAG_PTR rax
    cmp qword [rax], HDR_SYMBOL
    jne .not_operative_call
    mov rdi, r12
    mov rsi, [current_scope]
    call frame_lookup
    cmp rax, FRAME_NOT_FOUND
    jne .not_operative_call            ; locally bound — never an operative
    mov rax, r12
    UNTAG_PTR rax
    mov rax, [rax+16]                     ; value slot
    cmp rax, IMM_UNBOUND
    je .not_operative_call
    mov rdx, rax
    and rdx, TAG_MASK
    cmp rdx, TAG_HEAPOBJ
    jne .not_operative_call
    mov rdx, rax
    UNTAG_PTR rdx
    cmp qword [rdx], HDR_OPERATIVE
    jne .not_operative_call

    mov rdi, kw_quote
    mov rsi, 5
    call intern_symbol
    mov rdi, rax
    mov rsi, r13                          ; raw args (unevaluated)
    call build_list2
    push rax                                ; [q1 = (QUOTE raw-args)]

    mov rdi, kw_quote
    mov rsi, 5
    call intern_symbol
    push rax                                  ; [QUOTE_sym, q1]
    call global_environment_sentinel
    mov rsi, rax
    pop rdi                                     ; QUOTE_sym
    call build_list2                              ; q2 = (QUOTE sentinel)
    mov rsi, rax
    pop rdi                                         ; q1
    call build_list2                                  ; (q1 q2)
    mov r13, rax                                        ; new args list —
                                                         ; the original
                                                         ; raw args are
                                                         ; already baked
                                                         ; into q1, so
                                                         ; overwriting r13
                                                         ; here is safe

    mov rdi, r12
    mov rsi, r13
    mov rdx, r15
    call compile_call
    jmp .out

.not_operative_call:
    ; not a recognized special form, macro, or operative — an ordinary
    ; general application.
    mov rdi, r12
    mov rsi, r13
    mov rdx, r15
    call compile_call

.out:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_thunk(rdi = tagged sexpr) -> rax = pointer to a fresh native
; 0-arg function (any incoming register content is ignored) that
; evaluates the form and returns its tagged value in rax.
;
; Emits a leading `jmp` over its own body before compiling anything,
; the same guard compile_lambda's own nested-function emission already
; uses (a lambda body is itself emitted inline into whatever function
; is currently open, jumped over so the enclosing function's own
; straight-line code never falls into it). compile_thunk needs the
; identical guard for a reason compile_lambda doesn't have to worry
; about: compile_thunk can be invoked *re-entrantly*, in the middle of
; another, still-open compile_thunk's own emission — EVAL, exposed to
; *compiled* Lamedh code via eval_form below, can be called from
; within a macro transformer's own body (invoke_macro runs the
; transformer as ordinary already-compiled target code; the
; reference's own lib/02-cxr.lisp does exactly this: `defcxr`'s
; `(eval operations)` evaluates one of its own macro parameters at
; expansion time, while compile_form is still mid-way through
; compiling the *outer* form that triggered the macro expansion).
; Without the jmp-over guard, the nested thunk's own prologue/epilogue
; get spliced directly into the still-open outer thunk's own
; instruction stream at whatever offset codegen_here happened to be —
; the outer thunk would then run straight into the nested thunk's own
; `leave`/`ret` mid-body, popping the *outer* frame's saved rbp as a
; return address (0 at the top level, since boot.asm never sets one) —
; a real, previously-uncaught bug, not a hypothetical one: this is
; exactly what made `lib/02-cxr.lisp` (and thus `lib/08-vau.lisp`,
; whose own `$if` needs CADR/CADDR from it) segfault before this fix.
;
; Also always compiles against a clean top-level scope
; (current_scope=NIL, current_frame_depth=0, current_prog_ctx=0),
; saved and restored around compile_form exactly like compile_lambda's
; own body already does for its nested scope — never whatever ambient
; compile-time state happens to be active when this is called. For a
; plain top-level EVAL call these are already NIL/0/0, so this changes
; nothing observable there, but for the same re-entrant EVAL-inside-a-
; macro-transformer case described above, it stops a *silent* second
; bug once the jmp-over splicing fix above is in place: without this,
; a nested compile_thunk call would inherit whatever current_scope/
; current_frame_depth the *outer*, still-active compilation happened
; to be using, corrupting the outer compilation's own frame bookkeeping
; once it later resumed. KERNEL.md's own EVAL is one-argument,
; evaluating in "the global environment" — an ambient non-global scope
; leaking in was never correct to begin with, re-entrant or not.
global compile_thunk
compile_thunk:
    push rbx
    mov rbx, rdi

    ; Everything the compiler allocates while compiling — macro
    ; expansions, scope lists, synthetic forms, the &REST param split —
    ; is born pinned: a form under construction is referenced only from
    ; host registers and host stack frames, and v0 accepts leaking that
    ; compile-time garbage exactly as today rather than making the
    ; compiler itself collector-safe (docs/spec-tco-capture-gc.md 3.4,
    ; 3.7 item 7). eval_form therefore compiles pinned and runs
    ; unpinned.
    call rc_pin_enter

    cmp qword [compile_nesting_depth], 0
    jne .no_memo_clear
    call clear_macroexpand_memo
    call clear_capture_memo
.no_memo_clear:
    inc qword [compile_nesting_depth]

    mov rax, [current_scope]
    push rax                          ; [old_scope]
    mov rax, [current_frame_depth]
    push rax                            ; [old_frame_depth, old_scope]
    mov rax, [current_prog_ctx]
    push rax                              ; [old_prog_ctx, old_frame_depth, old_scope]
    mov qword [current_scope], IMM_NIL
    mov qword [current_frame_depth], 0
    mov qword [current_prog_ctx], 0

    call emit_jmp32
    push rax                     ; [jmp_over_site, old_prog_ctx, old_frame_depth, old_scope]

    call codegen_here
    push rax                     ; [entry_addr, jmp_over_site, old_prog_ctx, old_frame_depth, old_scope]
                                  ; — entry address, returned below
    call emit_push_rbp_frame
    mov rdi, rbx
    call compile_form
    call emit_leave
    call emit_ret

    call codegen_here
    mov rsi, rax
    mov rdi, [rsp+8]              ; jmp_over_site
    call patch_rel32                ; jmp_over_site -> just past this thunk

    pop rax                          ; entry_addr
    add rsp, 8                         ; discard jmp_over_site
                                        ; [old_prog_ctx, old_frame_depth, old_scope]

    pop rcx
    mov [current_prog_ctx], rcx
    pop rcx
    mov [current_frame_depth], rcx
    pop rcx
    mov [current_scope], rcx

    dec qword [compile_nesting_depth]
    call rc_pin_leave

    pop rbx
    ret

; eval_form(rdi = tagged sexpr) -> rax = the form's value, evaluated in
; the global environment (KERNEL.md Part XI's EVAL, one-argument form —
; there is no environment-as-value in this kernel to pass a second
; argument for; see README). This is compile_thunk plus the one step
; compile_thunk's own callers (file_runner.asm, every tests/cases/
; lamedh_main) already do by hand: compile the form into a fresh native
; function, then call it. Exposing this to *compiled* Lamedh code as
; (EVAL form) is what makes reflection possible from within a running
; program, not just from the host driver — the same "the compiler is
; just more compiled code" idea DEFMACRO's own invoke_macro already
; rests on (see README's "kernel surface" section), one level up.
global eval_form
eval_form:
    push rbx
    mov rbx, rdi
    call compile_thunk           ; rax = fresh native 0-arg function
    ; Every compiled thunk is invoked this way, everywhere else in this
    ; project (file_runner.asm's run_buffer, every tests/cases/*.asm
    ; lamedh_main): the extra push keeps rsp 16-byte aligned at the
    ; callee's entry, which floats.asm's host routines rely on for
    ; aligned SSE moves — a bare `call rax` here shifts alignment by 8
    ; and can fault (SIGBUS) the moment the freshly compiled code calls
    ; into one of them.
    push rax
    call qword [rsp]
    add rsp, 8
    pop rbx
    ret
