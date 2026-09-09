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

%define FRAME_NOT_FOUND 0x7FFFFFFF

section .rodata
kw_quote:  db "QUOTE"
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
kw_string_length: db "STRING-LENGTH"
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
kw_make_array:   db "MAKE-ARRAY"
kw_array_ref:    db "ARRAY-REF"
kw_array_set:    db "ARRAY-SET"
kw_array_length: db "ARRAY-LENGTH"
kw_hash_code:    db "HASH-CODE"
kw_mod:          db "MOD"
kw_remainder:    db "REMAINDER"
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
    jmp .done
.not_add:
    cmp bl, '-'
    jne .not_sub
    mov dil, REG_RAX
    mov sil, REG_RBX
    call emit_sub_rr
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
    ; actually used — only the first 3 (register-spilled) consume one;
    ; params beyond that live on the caller's stack and use none. r15
    ; already holds `entry` (needed later, for the closure's code
    ; pointer), so min(nparams,3) is recomputed into scratch rax each
    ; time it's needed rather than cached in a register.
    mov rax, r14
    cmp rax, 3
    jbe .nregparams_ok1
    mov rax, 3
.nregparams_ok1:
    cmp qword [rest_sym_scratch], IMM_NIL
    je .no_rest_bump1
    inc rax                                                ; &REST consumes one more local slot
.no_rest_bump1:
    mov rsi, rax
    mov rdi, r12
    call build_frame_from_list                        ; free_frame

    ; If this lambda has a &REST param, give it its own (symbol . disp)
    ; frame entry — disp is always -32 here: the restriction that &REST
    ; is only supported when nfixed>=3 (see split_rest_params call site
    ; and the README) means the register-spilled slots always fill all
    ; 3 of -8,-16,-24, so the REST slot always lands at -32.
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
    mov rax, r14
    cmp rax, 3
    jbe .nregparams_ok2
    mov rax, 3
.nregparams_ok2:
    cmp qword [rest_sym_scratch], IMM_NIL
    je .no_rest_bump2
    inc rax
.no_rest_bump2:
    add rax, rbx
    mov [lambda_frame_depth_scratch], rax     ; stash base_index for
                                               ; current_frame_depth,
                                               ; installed right before
                                               ; the body compiles below
    imul eax, eax, 8
    mov edi, eax
    call emit_sub_rsp_imm32

    ; spill up to 3 incoming params (rsi,rdx,rcx) into their slots
    cmp r14, 1
    jb .no_p0
    mov dil, REG_RSI
    mov esi, -8
    call emit_store_local
.no_p0:
    cmp r14, 2
    jb .no_p1
    mov dil, REG_RDX
    mov esi, -16
    call emit_store_local
.no_p1:
    cmp r14, 3
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
    mov eax, r14d                                 ; nparams, clamped to the
    cmp eax, 3                                      ; number of *local slots*
    jbe .minok                                        ; params actually use
    mov eax, 3                                          ; (see build_param_frame)
.minok:
    cmp qword [rest_sym_scratch], IMM_NIL
    je .no_rest_bump3
    inc eax                                              ; free vars start one slot later
.no_rest_bump3:
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
    ; Restriction: only supported when nfixed>=3, so every rest argument
    ; is stack-resident (see split_rest_params / README). Walks from the
    ; last actual argument down to nfixed, consing each onto an
    ; accumulator, so the final list is in left-to-right order.
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
    sub rsi, 3                                                ; nfixed-3, a compile-time constant
    call emit_cmp_rax_imm64                                     ; target: cmp rax, (nfixed-3) (clobbers rcx)
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

; invoke_closure_host(rdi=tagged closure, rsi=arg0, rdx=arg1, rcx=arg2)
; -> rax = result. Calls an *already-compiled* Lamedh closure directly
; from host code, synchronously, right now — not by emitting target
; instructions. This works because a compiled closure and the compiler
; itself are both just x86-64 machine code in the same process; there is
; no barrier between "host" and "target" beyond which code is calling
; which. It is the entire mechanism a macro transformer needs: the
; transformer is an ordinary compiled closure, and expanding a macro
; call means invoking it now instead of emitting a call to it.
global invoke_closure_host
invoke_closure_host:
    push rbx
    mov rbx, rdi
    UNTAG_PTR rbx
    mov rax, [rbx+8]              ; code_ptr
    call rax                        ; rdi/rsi/rdx/rcx already match the
                                     ; closure calling convention exactly
    pop rbx
    ret

; raw_args_to_regs(rdi=args list) -> sets rsi,rdx,rcx (host registers)
; from up to 3 elements of the list, taken as-is — car'd, never compiled
; or evaluated. This is how a macro transformer receives its arguments:
; unevaluated syntax, not values.
raw_args_to_regs:
    push rbx
    mov rbx, rdi
    xor rsi, rsi
    xor rdx, rdx
    xor rcx, rcx
    cmp rbx, IMM_NIL
    je .out
    mov rdi, rbx
    call car
    mov rsi, rax
    mov rdi, rbx
    call cdr
    mov rbx, rax
    cmp rbx, IMM_NIL
    je .out
    mov rdi, rbx
    call car
    mov rdx, rax
    mov rdi, rbx
    call cdr
    mov rbx, rax
    cmp rbx, IMM_NIL
    je .out
    mov rdi, rbx
    call car
    mov rcx, rax
.out:
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
    mov rdx, 10
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
    mov rdx, 12
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
    mov rdx, 9
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
    mov rdx, 9
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
    mov rdi, r13
    call raw_args_to_regs                     ; -> rsi,rdx,rcx (raw forms)
    mov rdi, rbx
    call invoke_closure_host                     ; rax = expansion
    mov rdi, rax
    call compile_form                              ; recompile in its place
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
