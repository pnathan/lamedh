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
extern fail_not_callable

%define FRAME_NOT_FOUND 0x7FFFFFFF

section .rodata
kw_quote:  db "QUOTE"
kw_function: db "FUNCTION"
kw_if:     db "IF"
kw_define: db "DEFINE"
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
kw_error_p:       db "ERROR-P"
kw_error_message: db "ERROR-MESSAGE"
kw_error_data:    db "ERROR-DATA"
kw_block:       db "BLOCK"
kw_return_from: db "RETURN-FROM"
kw_while:       db "WHILE"
kw_string_length: db "STRING-LENGTH"
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
kw_array_ref:    db "FETCH"
kw_array_set:    db "STORE"
kw_array_length: db "ARRAY-LENGTH*"
kw_hash_code:    db "HASH-CODE"
kw_mod:          db "MOD"
kw_remainder:    db "REMAINDER"
kw_flag_set_p:      db "FLAG-SET-P"
kw_clear_flag:      db "CLEAR-FLAG"
kw_clear_all_flags: db "CLEAR-ALL-FLAGS"
kw_rest:   db "&REST"

section .data
align 8
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
rest_sym_scratch: resq 1

; current_frame_depth: how many rbp-relative local slots are already
; considered reserved in the *current* function's own frame (params +
; frees + REST slot, plus whatever LET/LET* nesting is currently
; active) — the same "how deep am I" count compile_lambda already
; computes for its own prologue, just kept live across LET/LET* so
; they know where their own slots start. Saved/restored around a
; nested LAMBDA's or LET's body exactly like current_scope is.
current_frame_depth: resq 1

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

; compile_eq(rdi=arg1 form, rsi=arg2 form) — raw 64-bit equality, valid
; across every tag (pointer or immediate) uniformly.
compile_eq:
    push rbx
    mov rbx, rsi
    call compile_form
    mov dil, REG_RAX
    call emit_push_reg
    mov rdi, rbx
    call compile_form
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr
    mov dil, REG_RAX
    call emit_pop_reg
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_cmp_rr
    call emit_sete_al
    call bool_from_al
    pop rbx
    ret

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
    mov dil, REG_RAX
    call emit_push_reg               ; push lhs

    mov rdi, r12
    call compile_form                 ; rhs -> rax
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

; compile_progn(rdi = forms list) — emits code evaluating each form in
; order; target rax holds the last one's value, or NIL for an empty
; list (KERNEL.md Part VI: "(progn) is NIL"). This is what a multi-
; form LAMBDA/LET/LET* body compiles through — compile_form's callers
; already preserve rbx/r12/r13/r14 across a nested compile_form call,
; so a plain host-side loop over the list is enough; no special
; backpatching is needed since nothing branches here.
compile_progn:
    push rbx
    mov rbx, rdi
    cmp rbx, IMM_NIL
    jne .have
    mov rsi, IMM_NIL
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    jmp .out
.have:
.loop:
    mov rdi, rbx
    call car
    mov rdi, rax
    call compile_form
    mov rdi, rbx
    call cdr
    mov rbx, rax
    cmp rbx, IMM_NIL
    jne .loop
.out:
    pop rbx
    ret

; compile_cond(rdi = the full (COND clause...) form)
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
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_and(rdi = the full (AND form...) form)
; (AND) -> T. Otherwise forms are evaluated left to right; the first
; NIL short-circuits the rest with NIL as the result; if none is NIL,
; the last form's value is the result. Same end-jump-list-then-patch
; technique as compile_cond, one jump per short-circuiting form.
compile_and:
    push rbx
    push r12
    push r13
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
    call car
    mov rdi, rax
    call compile_form                    ; form -> target rax
    mov rdi, rbx
    call cdr
    mov rbx, rax
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
    pop r13
    pop r12
    pop rbx
    ret

; compile_or(rdi = the full (OR form...) form)
; (OR) -> NIL. Otherwise forms are evaluated left to right; the first
; non-NIL short-circuits the rest with that value as the result; if
; every form is NIL, the result is NIL (the last form's own NIL value,
; already correct with no extra work). Mirrors compile_and exactly,
; short-circuiting on "not NIL" (emit_jne) instead of "is NIL".
compile_or:
    push rbx
    push r12
    push r13
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
    call car
    mov rdi, rax
    call compile_form
    mov rdi, rbx
    call cdr
    mov rbx, rax
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

; compile_let(rdi = the full (LET ((name init)...) body...) form)
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
    mov rbx, rdi
    call cadr
    mov r12, rax                        ; bindings list (original head)
    mov rdi, rbx
    call cdr
    mov rdi, rax
    call cdr
    mov r13, rax                          ; body forms list

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

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_let_star(rdi = the full (LET* ((name init)...) body...) form)
;
; Sequential binding: each init sees every earlier binding of the same
; LET* (but not later ones). Structurally identical to compile_let
; except each binding's frame entry is installed into current_scope
; immediately after its own init is stored, one at a time, instead of
; building the whole frame up front and installing it only once every
; init has run.
compile_let_star:
    push rbx
    push r12
    push r13
    push r14
    push r15
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

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_if(rdi = the full (IF test then else) form)
;
; Backpatched forward branches: the je/jmp targets aren't known until the
; then/else branches have themselves been compiled (their length depends
; on what's inside them), so each is emitted first against a placeholder
; rel32 and fixed up afterward with the same patch_rel32 primitive the
; runtime inline cache uses on already-executed code — compile-time
; backpatching and runtime self-modification are the same mechanism.
compile_if:
    push rbx
    push r12
    push r13
    push r14
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

    mov rdi, r13
    call compile_form                              ; then branch
    call emit_jmp32                                  ; rax = jmp rel32 field addr
    mov r12, rax

    call codegen_here
    mov rdi, r14
    mov rsi, rax
    call patch_rel32                                    ; je -> else branch start

    mov rdi, rbx
    call compile_form                                     ; else branch

    call codegen_here
    mov rdi, r12
    mov rsi, rax
    call patch_rel32                                        ; jmp -> end

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
    mov dil, REG_RAX
    call emit_store_mem64
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
    mov dil, REG_RAX
    call emit_store_mem64
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

    mov rdi, r13
    mov rsi, [current_scope]
    mov rdx, IMM_NIL
    call scan_free_vars
    mov r12, rax                                  ; free_syms

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

    mov rdi, r13
    call compile_progn

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

    ; re-derive free_syms (same deterministic result) to copy each
    ; free var's *current* value, from the enclosing scope, into the
    ; closure's captured array.
    mov rdi, r13
    mov rsi, [current_scope]
    mov rdx, IMM_NIL
    call scan_free_vars
    mov r12, rax                                        ; free_syms cursor
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
    mov rdi, rbx
    call cadddr                          ; body -> rax
    mov rdi, rax
    mov rsi, IMM_NIL
    call cons                              ; (body . nil)
    mov rdi, [rsp]                           ; params
    mov rsi, rax
    call cons                                  ; (params body)
    add rsp, 8                                   ; [ ]

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
    mov dil, REG_RAX
    call emit_store_mem64                                      ; target: symbol.macro = closure

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

; emit_check_callable() — target: rax holds a tagged value about to be
; treated as a closure and called. Verifies it is actually a
; HDR_CLOSURE heapobj; if not, calls fail_not_callable (native_errors.asm)
; instead of letting the caller's own subsequent `and rax,~TAG_MASK` +
; dereference run on whatever address an unbound global (IMM_NIL) or
; other non-closure value happens to produce — previously a near-NULL
; dereference (a segfault), since IMM_NIL's tag bits mask to a null
; pointer. KERNEL.md Part VIII lists calling a non-callable value among
; the native-failure classes a host must signal for; this is the v0
; stand-in (a deterministic exit, not yet a HANDLER-CASE-catchable
; condition — see native_errors.asm).
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
    call emit_jne                                     ; -> fail
    push rax                                            ; [hdr_fail_site, tag_fail_site]

    mov dil, REG_RAX
    mov sil, REG_RDI
    call emit_mov_rr                                      ; rax = rdi (restore tagged value)
    call emit_jmp32                                         ; -> success
    push rax                                                  ; [ok_site, hdr_fail_site, tag_fail_site]

    call codegen_here                                           ; fail:
    push rax                                                      ; [fail_addr, ok_site, hdr_fail_site, tag_fail_site]
    ; patch_rel32 clobbers rax internally (lea rax,[rdi+4]), so
    ; fail_addr must be reloaded from memory for the second call
    ; rather than trusted to survive in a register across the first.
    mov rdi, [rsp+16]
    mov rsi, [rsp]
    call patch_rel32                                              ; hdr_fail_site -> fail
    mov rdi, [rsp+24]
    mov rsi, [rsp]
    call patch_rel32                                                ; tag_fail_site -> fail
    add rsp, 8                                                        ; discard fail_addr

    lea rax, [rel fail_not_callable]
    mov rsi, rax
    mov dil, REG_RAX
    call emit_mov_reg_imm64
    mov dil, REG_RAX
    call emit_call_reg                                                ; never returns

    call codegen_here                                                   ; success:
    mov rdi, [rsp]
    mov rsi, rax
    call patch_rel32                                                      ; ok_site -> success
    add rsp, 24
    ret

; compile_call(rdi=operator form, rsi=args list)
; A general application (f arg...). If f is a symbol that is not locally
; bound (i.e. a genuine global), the call site is compiled through a
; per-site, self-patching inline-cache trampoline: the first invocation
; resolves the symbol's current value, rewrites the *original* call
; site's rel32 in place to target the resolved code directly, and only
; then jumps there — every later call from that exact site is a plain
; direct call, no indirection, no re-resolution. Anything else (a local
; variable holding a closure, a literal LAMBDA) goes through one indirect
; call via the closure's stored code pointer.
global compile_call
compile_call:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi                    ; operator form
    mov r12, rsi                      ; args

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

    mov dil, REG_RAX
    mov esi, 8                                        ; return address is now
    call emit_load_rsp_disp8                            ; one slot deeper, under
                                                         ; the nargs we just saved
    mov edi, 4
    call emit_sub_rax_imm32                            ; rax = field_addr

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

    call emit_call32
    mov rdi, rax
    pop rsi                                                                     ; trampoline_entry
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
; Searches the catch stack from the top for an EQ tag match, then
; restores rbp/rsp to that frame's saved state and jumps to its resume
; point with the thrown value in rax — an ordinary longjmp. Trapping
; (int3) on no match is the v0 failure mode; see README.
compile_throw:
    push rbx
    push r12
    push r13
    push r14
    mov rbx, rdi
    call cadr
    mov r12, rax                    ; tag form
    mov rdi, rbx
    call caddr
    mov r13, rax                      ; value form

    ; The tag must survive the value form's own compilation, and no
    ; register survives that: compile_binop parks its RHS in rbx, and
    ; every call path parks the callee's code pointer there too. So the
    ; tag goes on the machine stack (like compile_binop's own lhs) and
    ; is reloaded into rbx only once the value is computed and pushed.
    mov rdi, r12
    call compile_form                    ; tag -> target rax
    mov dil, REG_RAX
    call emit_push_reg                      ; target: push tag

    mov rdi, r13
    call compile_form                          ; value -> target rax
    mov dil, REG_RAX
    call emit_push_reg                            ; target: push value

    mov dil, REG_RBX
    mov esi, 8
    call emit_load_rsp_disp8                        ; target: rbx = tag (under value)

    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_load_mem64                            ; target: rax = top
    mov dil, REG_RSI
    mov sil, REG_RAX
    call emit_mov_rr                                   ; target: rsi = remaining count

    call codegen_here
    mov r12, rax                                          ; loop_start

    mov dil, REG_RAX
    mov sil, REG_RSI
    call emit_mov_rr
    mov rsi, 0
    call emit_cmp_rax_imm64
    call emit_je                                            ; rax = unmatched_site (deferred)
    mov r13, rax

    mov dil, REG_RAX
    mov sil, REG_RSI
    call emit_mov_rr
    mov edi, 1
    call emit_sub_rax_imm32
    mov dil, REG_RSI
    mov sil, REG_RAX
    call emit_mov_rr                                          ; target: rsi = index

    mov dil, REG_RAX
    mov sil, REG_RSI
    call emit_mov_rr
    mov edi, 32
    call emit_imul_rax_imm32
    lea rax, [rel catch_stack]
    mov edi, eax
    call emit_add_rax_imm32                                     ; target: rax = frame_addr

    mov dil, REG_RDX
    mov sil, REG_RAX
    mov edx, 0
    call emit_load_based                                          ; target: rdx = frame[0] (tag)
    mov dil, REG_RDX
    mov sil, REG_RBX
    call emit_cmp_rr
    call emit_jne                                                   ; rax = continue_site (deferred)
    mov r14, rax

    ; --- match: rax still = frame_addr ---
    ; catch_stack_top is stored *before* any frame field is loaded into
    ; rcx: emit_store_mem64 uses rcx as its own scratch (unless the
    ; source register itself is rcx), and would otherwise clobber the
    ; saved-rsp value the very next few instructions depend on.
    ;
    ; The stored count is index+1, not index: resume target now lands in
    ; CATCH's shared epilogue (see compile_catch), which always does its
    ; own "-1" on the way out — landing there with the count pre-bumped
    ; by one is what makes that shared decrement land on the correct
    ; final count either way (a normal return or a caught throw).
    ; rax must not move here — it still holds frame_addr, needed by the
    ; loads just below — so the +1 goes straight into rsi, not through
    ; the usual rax shuttle.
    mov dil, REG_RSI
    mov esi, 1
    call emit_add_reg_imm32                                                     ; rsi += 1
    lea rsi, [rel catch_stack_top]
    mov dil, REG_RSI
    call emit_store_mem64                                                     ; catch_stack_top = index + 1

    mov dil, REG_RCX
    mov sil, REG_RAX
    mov edx, 16
    call emit_load_based                                              ; rcx = saved rsp
    mov dil, REG_RDX
    mov sil, REG_RAX
    mov edx, 8
    call emit_load_based                                                ; rdx = saved rbp
    mov dil, REG_RBX
    mov sil, REG_RAX
    mov edx, 24
    call emit_load_based                                                  ; rbx = resume target
    mov dil, REG_RDI
    mov esi, 0
    call emit_load_rsp_disp8                                                ; rdi = thrown value (unpopped)

    mov dil, REG_RSP
    mov sil, REG_RCX
    call emit_mov_rr                                                            ; rsp = saved rsp
    mov dil, REG_RBP
    mov sil, REG_RDX
    call emit_mov_rr                                                              ; rbp = saved rbp
    mov dil, REG_RAX
    mov sil, REG_RDI
    call emit_mov_rr                                                                ; rax = thrown value
    mov dil, REG_RBX
    call emit_jmp_reg                                                                 ; -> resume target

    call codegen_here
    mov rdi, r14
    mov rsi, rax
    call patch_rel32                       ; continue_site -> here

    call emit_jmp32
    mov rdi, rax
    mov rsi, r12
    call patch_rel32                       ; jmp back to loop_start

    call codegen_here
    mov rdi, r13
    mov rsi, rax
    call patch_rel32                       ; unmatched_site -> here
    mov rdi, 0xCC
    call emit8                             ; no matching CATCH — trap

    pop r14
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
; evaluated fresh) — this is what ERROR's signaling needs.
emit_throw_baked:
    push rbx
    push r12
    push r13
    push r14
    mov r12, rdi                  ; tag

    mov dil, REG_RAX
    call emit_push_reg               ; push value (already computed)

    mov rsi, r12
    mov dil, REG_RAX
    call emit_mov_reg_imm64             ; rax = tag
    mov dil, REG_RBX
    mov sil, REG_RAX
    call emit_mov_rr                       ; rbx = tag

    lea rsi, [rel catch_stack_top]
    mov dil, REG_RAX
    call emit_load_mem64
    mov dil, REG_RSI
    mov sil, REG_RAX
    call emit_mov_rr                          ; rsi = remaining count

    call codegen_here
    mov r13, rax                                 ; loop_start

    mov dil, REG_RAX
    mov sil, REG_RSI
    call emit_mov_rr
    mov rsi, 0
    call emit_cmp_rax_imm64
    call emit_je
    mov r14, rax                                    ; unmatched_site

    mov dil, REG_RAX
    mov sil, REG_RSI
    call emit_mov_rr
    mov edi, 1
    call emit_sub_rax_imm32
    mov dil, REG_RSI
    mov sil, REG_RAX
    call emit_mov_rr                                    ; rsi = index

    mov dil, REG_RAX
    mov sil, REG_RSI
    call emit_mov_rr
    mov edi, 32
    call emit_imul_rax_imm32
    lea rax, [rel catch_stack]
    mov edi, eax
    call emit_add_rax_imm32                               ; rax = frame_addr

    mov dil, REG_RDX
    mov sil, REG_RAX
    mov edx, 0
    call emit_load_based                                     ; rdx = frame[0] tag
    mov dil, REG_RDX
    mov sil, REG_RBX
    call emit_cmp_rr
    call emit_jne
    push rax                                                    ; [continue_site]

    mov dil, REG_RSI
    mov esi, 1
    call emit_add_reg_imm32
    lea rsi, [rel catch_stack_top]
    mov dil, REG_RSI
    call emit_store_mem64                                          ; top = index+1

    mov dil, REG_RCX
    mov sil, REG_RAX
    mov edx, 16
    call emit_load_based                                             ; rcx = saved rsp
    mov dil, REG_RDX
    mov sil, REG_RAX
    mov edx, 8
    call emit_load_based                                               ; rdx = saved rbp
    mov dil, REG_RBX
    mov sil, REG_RAX
    mov edx, 24
    call emit_load_based                                                 ; rbx = resume target
    mov dil, REG_RDI
    mov esi, 0
    call emit_load_rsp_disp8                                               ; rdi = thrown value (unpopped)

    mov dil, REG_RSP
    mov sil, REG_RCX
    call emit_mov_rr
    mov dil, REG_RBP
    mov sil, REG_RDX
    call emit_mov_rr
    mov dil, REG_RAX
    mov sil, REG_RDI
    call emit_mov_rr
    mov dil, REG_RBX
    call emit_jmp_reg

    call codegen_here
    pop rdi                                                                   ; continue_site
    mov rsi, rax
    call patch_rel32

    call emit_jmp32
    mov rdi, rax
    mov rsi, r13
    call patch_rel32

    call codegen_here
    mov rdi, r14
    mov rsi, rax
    call patch_rel32
    mov rdi, 0xCC
    call emit8                             ; no matching CATCH — trap

    pop r14
    pop r13
    pop r12
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
    call compile_progn                            ; body -> rax (discarded)

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
    mov rsi, kw_if
    mov rdx, 2
    call sym_is
    test rax, rax
    jz .not_if
    mov rdi, rbx
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
    mov rbx, rax                            ; macro closure (tagged)
    mov rdi, rbx
    mov rsi, r13
    call invoke_macro                         ; rax = expansion
    mov rdi, rax
    call compile_form                           ; recompile in its place
    jmp .out

.not_macro_call:
    ; not a recognized special form or macro — a general application.
    mov rdi, r12
    mov rsi, r13
    call compile_call

.out:
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; compile_thunk(rdi = tagged sexpr) -> rax = pointer to a fresh native
; 0-arg function (any incoming register content is ignored) that
; evaluates the form and returns its tagged value in rax.
global compile_thunk
compile_thunk:
    push rbx
    mov rbx, rdi
    call codegen_here
    push rax                     ; function entry address, returned below
    call emit_push_rbp_frame
    mov rdi, rbx
    call compile_form
    call emit_leave
    call emit_ret
    pop rax
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
    call rax                       ; -> rax = its result
    pop rbx
    ret
