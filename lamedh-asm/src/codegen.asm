; codegen.asm — raw machine-code emission into the RWX code heap, plus the
; one patching primitive (patch_rel32) that both compile-time backpatching
; (an IF's forward branch) and runtime self-modification (an inline cache
; rewriting a call site after it has already executed) are built from.
;
; There is no assembler-within-the-assembler here: the compiler in
; compiler.asm emits raw opcode bytes it has chosen itself. This is
; deliberate — hand-encoding means every instruction the compiler emits
; is exactly the one intended, with no intermediate mnemonic layer to
; second-guess register allocation or encoding choice.

extern code_alloc
extern code_heap_cur

section .text

; codegen_here() -> rax = current code-heap write cursor (the address the
; next emitted byte will land at). Used to remember branch-target and
; patch-site addresses during single-pass compilation.
global codegen_here
codegen_here:
    mov rax, [code_heap_cur]
    ret

; emit8(dil = byte)
global emit8
emit8:
    mov rax, [code_heap_cur]
    mov [rax], dil
    inc qword [code_heap_cur]
    ret

; emit32(edi = dword, little-endian)
global emit32
emit32:
    mov rax, [code_heap_cur]
    mov [rax], edi
    add qword [code_heap_cur], 4
    ret

; emit64(rdi = qword, little-endian)
global emit64
emit64:
    mov rax, [code_heap_cur]
    mov [rax], rdi
    add qword [code_heap_cur], 8
    ret

; --- target instruction emitters -------------------------------------
;
; A small, fixed set of hand-encoded x86-64 instructions, each over the
; 8 base GPRs only (rax=0 rcx=1 rdx=2 rbx=3 rsp=4 rbp=5 rsi=6 rdi=7 — the
; standard encoding, no REX.B/R needed since every field fits in 3 bits).
; The compiler proper (compiler.asm) calls these; nothing here knows
; anything about Lisp forms.

; emit_mov_reg_imm64(dil = dst reg code, rsi = imm64)
global emit_mov_reg_imm64
emit_mov_reg_imm64:
    push rbx
    mov bl, dil
    mov rdi, 0x48
    call emit8
    movzx rdi, bl
    add rdi, 0xB8
    call emit8
    mov rdi, rsi
    call emit64
    pop rbx
    ret

; emit_rr(dil = opcode byte, sil = dst reg, dl = src reg) -> "OP dst,src"
; for the r/m64,r64 instruction family (MOV/ADD/SUB/CMP all share this
; ModRM shape: mod=11, reg=src, rm=dst).
emit_rr:
    push rbx
    push r12
    mov bl, dil                    ; opcode
    mov r12b, sil                   ; dst
    ; dl (src) is untouched by emit8 (which only clobbers rax) and so
    ; survives to the modrm computation below without being saved.
    mov rdi, 0x48
    call emit8                       ; REX.W
    movzx rdi, bl
    call emit8                        ; opcode
    mov al, 0xC0
    mov cl, dl                          ; src
    shl cl, 3
    or al, cl
    or al, r12b                          ; | dst
    movzx rdi, al
    call emit8                            ; modrm
    pop r12
    pop rbx
    ret

; emit_mov_rr(dil=dst, sil=src)
global emit_mov_rr
emit_mov_rr:
    mov dl, sil
    mov sil, dil
    mov dil, 0x89
    jmp emit_rr

; emit_add_rr(dil=dst, sil=src)
global emit_add_rr
emit_add_rr:
    mov dl, sil
    mov sil, dil
    mov dil, 0x01
    jmp emit_rr

; emit_sub_rr(dil=dst, sil=src)
global emit_sub_rr
emit_sub_rr:
    mov dl, sil
    mov sil, dil
    mov dil, 0x29
    jmp emit_rr

; emit_cmp_rr(dil=dst, sil=src)
global emit_cmp_rr
emit_cmp_rr:
    mov dl, sil
    mov sil, dil
    mov dil, 0x39
    jmp emit_rr

; emit_imul_rr(dil=dst, sil=src) — IMUL r64,r/m64: 0F AF /r, reg=dst rm=src
global emit_imul_rr
emit_imul_rr:
    push rbx
    mov bl, dil                    ; dst
    mov rdi, 0x48
    call emit8
    mov rdi, 0x0F
    call emit8
    mov rdi, 0xAF
    call emit8
    mov al, 0xC0
    mov cl, bl
    shl cl, 3
    or al, cl
    or al, sil
    movzx rdi, al
    call emit8
    pop rbx
    ret

; emit_sar_imm8(dil=dst, sil=imm8) — C1 /7 ib
global emit_sar_imm8
emit_sar_imm8:
    push rbx
    push r12
    mov bl, sil                    ; imm8
    mov r12b, dil                   ; dst — saved before emit8 clobbers dil
    mov rdi, 0x48
    call emit8
    mov rdi, 0xC1
    call emit8
    mov al, 0xF8                     ; mod11 reg=111(/7) rm=000 (rax) base
    or al, r12b
    movzx rdi, al
    call emit8
    movzx rdi, bl
    call emit8
    pop r12
    pop rbx
    ret

; emit_push_reg(dil=reg)
global emit_push_reg
emit_push_reg:
    movzx rdi, dil
    add rdi, 0x50
    jmp emit8

; emit_pop_reg(dil=reg)
global emit_pop_reg
emit_pop_reg:
    movzx rdi, dil
    add rdi, 0x58
    jmp emit8

; emit_load_local(dil=dst reg, esi=disp32) — MOV dst,[rbp+disp32]
global emit_load_local
emit_load_local:
    push rbx
    mov bl, dil
    mov rdi, 0x48
    call emit8
    mov rdi, 0x8B
    call emit8
    mov al, 0x80
    mov cl, bl
    shl cl, 3
    or al, cl
    or al, 5                        ; rm = 101 (rbp), mod=10 -> disp32 follows
    movzx rdi, al
    call emit8
    mov edi, esi
    call emit32
    pop rbx
    ret

; emit_store_local(dil=src reg, esi=disp32) — MOV [rbp+disp32],src
global emit_store_local
emit_store_local:
    push rbx
    mov bl, dil
    mov rdi, 0x48
    call emit8
    mov rdi, 0x89
    call emit8
    mov al, 0x80
    mov cl, bl
    shl cl, 3
    or al, cl
    or al, 5
    movzx rdi, al
    call emit8
    mov edi, esi
    call emit32
    pop rbx
    ret

; emit_sete_al() / emit_setl_al() — fixed 3-byte sequences
global emit_sete_al
emit_sete_al:
    mov rdi, 0x0F
    call emit8
    mov rdi, 0x94
    call emit8
    mov rdi, 0xC0
    jmp emit8

global emit_setl_al
emit_setl_al:
    mov rdi, 0x0F
    call emit8
    mov rdi, 0x9C
    call emit8
    mov rdi, 0xC0
    jmp emit8

global emit_setne_al
emit_setne_al:
    mov rdi, 0x0F
    call emit8
    mov rdi, 0x95
    call emit8
    mov rdi, 0xC0
    jmp emit8

; emit_movzx_eax_al() — 0F B6 C0
global emit_movzx_eax_al
emit_movzx_eax_al:
    mov rdi, 0x0F
    call emit8
    mov rdi, 0xB6
    call emit8
    mov rdi, 0xC0
    jmp emit8

; emit_add_rax_imm32(edi=imm32) — 05 id
global emit_add_rax_imm32
emit_add_rax_imm32:
    push rbx
    mov ebx, edi
    mov rdi, 0x48
    call emit8
    mov rdi, 0x05
    call emit8
    mov edi, ebx
    call emit32
    pop rbx
    ret

; emit_imul_rax_imm32(edi=imm32) — 69 /0 id (IMUL rax,rax,imm32)
global emit_imul_rax_imm32
emit_imul_rax_imm32:
    push rbx
    mov ebx, edi
    mov rdi, 0x48
    call emit8
    mov rdi, 0x69
    call emit8
    mov rdi, 0xC0
    call emit8
    mov edi, ebx
    call emit32
    pop rbx
    ret

; emit_ret() — C3
global emit_ret
emit_ret:
    mov rdi, 0xC3
    jmp emit8

; emit_push_rbp_frame() — 55 (push rbp); 48 89 E5 (mov rbp,rsp)
global emit_push_rbp_frame
emit_push_rbp_frame:
    mov rdi, 0x55
    call emit8
    mov rdi, 0x48
    call emit8
    mov rdi, 0x89
    call emit8
    mov rdi, 0xE5
    jmp emit8

; emit_leave() — 48 89 EC (mov rsp,rbp); 5D (pop rbp)
global emit_leave
emit_leave:
    mov rdi, 0x48
    call emit8
    mov rdi, 0x89
    call emit8
    mov rdi, 0xEC
    call emit8
    mov rdi, 0x5D
    jmp emit8

; emit_cmp_rax_imm64(rsi=imm64) — load imm64 into rcx (reg 1), then CMP rax,rcx
global emit_cmp_rax_imm64
emit_cmp_rax_imm64:
    push rdi
    mov dil, 1                      ; rcx
    call emit_mov_reg_imm64
    pop rdi
    mov dil, 0                       ; dst=rax
    mov sil, 1                       ; src=rcx
    jmp emit_cmp_rr

; emit_je(rel32 placeholder) -> rax = address of the rel32 field.
; 0F 84 <rel32>
global emit_je
emit_je:
    mov rdi, 0x0F
    call emit8
    mov rdi, 0x84
    call emit8
    call codegen_here
    push rax
    mov rdi, 0
    call emit32
    pop rax
    ret

; emit_jne(rel32 placeholder) -> rax = address of the rel32 field.
; 0F 85 <rel32>
global emit_jne
emit_jne:
    mov rdi, 0x0F
    call emit8
    mov rdi, 0x85
    call emit8
    call codegen_here
    push rax
    mov rdi, 0
    call emit32
    pop rax
    ret

; emit_jmp32() -> rax = address of the rel32 field. E9 <rel32>
global emit_jmp32
emit_jmp32:
    mov rdi, 0xE9
    call emit8
    call codegen_here
    push rax
    mov rdi, 0
    call emit32
    pop rax
    ret

; emit_call32() -> rax = address of the rel32 field (i.e. site+1, since the
; opcode byte precedes it) for later patch_rel32. E8 <rel32>
global emit_call32
emit_call32:
    mov rdi, 0xE8
    call emit8
    call codegen_here
    push rax
    mov rdi, 0
    call emit32
    pop rax
    ret

; emit_call_reg(dil=reg) -> indirect call through a register. FF /2
; modrm = mod11 reg=010 rm=reg
global emit_call_reg
emit_call_reg:
    push rbx
    mov bl, dil
    mov rdi, 0xFF
    call emit8
    mov al, 0xD0                     ; mod11 reg=010(/2) rm=000, i.e. 0xD0 base
    or al, bl
    movzx rdi, al
    call emit8
    pop rbx
    ret

; emit_load_mem64(dil=dst reg, rsi=abs addr) — MOV dst,[abs] via two
; instructions: load the absolute address as an immediate, then deref.
; The data heap never relocates (one fixed mmap for process lifetime),
; so baking an absolute address as a code immediate is sound.
global emit_load_mem64
emit_load_mem64:
    push rbx
    mov bl, dil
    call emit_mov_reg_imm64            ; dst <- abs addr  (dil,rsi already args)
    movzx rdi, bl
    mov sil, bl
    call emit_load_local_zero_disp
    pop rbx
    ret

; emit_load_local_zero_disp(dil=reg, sil=reg) — MOV reg,[reg] with disp32=0,
; i.e. reg <- *reg (dst and src are the same register: address in, value out).
global emit_load_local_zero_disp
emit_load_local_zero_disp:
    push rbx
    mov bl, dil
    mov rdi, 0x48
    call emit8
    mov rdi, 0x8B
    call emit8
    mov al, 0x80                       ; mod10, rm=bl (address reg), disp32=0
    mov cl, bl
    shl cl, 3
    or al, cl
    or al, bl
    movzx rdi, al
    call emit8
    xor edi, edi
    call emit32                         ; disp32 = 0
    pop rbx
    ret

; emit_store_mem64(rsi=abs addr, dil=src reg) — MOV [abs],src via a scratch
; address register that is NOT the same register as src (rcx, unless src
; is itself rcx, in which case rdx).
global emit_store_mem64
emit_store_mem64:
    push rbx
    push r12
    push r13
    mov bl, dil                          ; src reg
    mov r12, rsi                          ; abs addr
    mov r13b, 1                            ; scratch = rcx by default
    cmp bl, 1
    jne .scratch_ok
    mov r13b, 2                             ; src is rcx -> use rdx instead
.scratch_ok:
    mov dil, r13b
    mov rsi, r12
    call emit_mov_reg_imm64                 ; scratch <- imm64(abs addr)

    mov rdi, 0x48
    call emit8                                ; REX.W
    mov rdi, 0x89
    call emit8                                 ; MOV r/m64,r64
    mov al, 0x00                                ; mod00 [reg], no SIB/disp needed:
    mov cl, bl                                   ; scratch is rcx/rdx, never rsp/rbp
    shl cl, 3
    or al, cl
    or al, r13b
    movzx rdi, al
    call emit8                                    ; modrm: [scratch] <- src
    pop r13
    pop r12
    pop rbx
    ret

; emit_load_based(dil=dst reg, sil=base reg, edx=disp32) — MOV dst,[base+disp32]
global emit_load_based
emit_load_based:
    push rbx
    push r12
    mov bl, dil
    mov r12b, sil
    mov rdi, 0x48
    call emit8
    mov rdi, 0x8B
    call emit8
    mov al, 0x80
    mov cl, bl
    shl cl, 3
    or al, cl
    or al, r12b
    movzx rdi, al
    call emit8
    mov edi, edx
    call emit32
    pop r12
    pop rbx
    ret

; emit_store_based(dil=src reg, sil=base reg, edx=disp32) — MOV [base+disp32],src
global emit_store_based
emit_store_based:
    push rbx
    push r12
    mov bl, dil
    mov r12b, sil
    mov rdi, 0x48
    call emit8
    mov rdi, 0x89
    call emit8
    mov al, 0x80
    mov cl, bl
    shl cl, 3
    or al, cl
    or al, r12b
    movzx rdi, al
    call emit8
    mov edi, edx
    call emit32
    pop r12
    pop rbx
    ret

; emit_or_rax_imm32(edi=imm32) — 48 0D id
global emit_or_rax_imm32
emit_or_rax_imm32:
    push rbx
    mov ebx, edi
    mov rdi, 0x48
    call emit8
    mov rdi, 0x0D
    call emit8
    mov edi, ebx
    call emit32
    pop rbx
    ret

; emit_and_rax_imm32(edi=imm32) — 48 25 id
global emit_and_rax_imm32
emit_and_rax_imm32:
    push rbx
    mov ebx, edi
    mov rdi, 0x48
    call emit8
    mov rdi, 0x25
    call emit8
    mov edi, ebx
    call emit32
    pop rbx
    ret

; emit_sub_rax_imm32(edi=imm32) — 48 2D id
global emit_sub_rax_imm32
emit_sub_rax_imm32:
    push rbx
    mov ebx, edi
    mov rdi, 0x48
    call emit8
    mov rdi, 0x2D
    call emit8
    mov edi, ebx
    call emit32
    pop rbx
    ret

; emit_add_reg_imm32(dil=reg, esi=imm32) — 48 81 /0 id, general register
global emit_add_reg_imm32
emit_add_reg_imm32:
    push rbx
    push r12
    mov bl, dil
    mov r12d, esi
    mov rdi, 0x48
    call emit8
    mov rdi, 0x81
    call emit8
    mov al, 0xC0                     ; mod11, reg=000(/0), rm=reg
    or al, bl
    movzx rdi, al
    call emit8
    mov edi, r12d
    call emit32
    pop r12
    pop rbx
    ret

; emit_sub_rsp_imm32(edi=imm32) — 48 81 /5 id
global emit_sub_rsp_imm32
emit_sub_rsp_imm32:
    push rbx
    mov ebx, edi
    mov rdi, 0x48
    call emit8
    mov rdi, 0x81
    call emit8
    mov rdi, 0xEC                  ; mod11 reg=101(/5) rm=100(rsp)
    call emit8
    mov edi, ebx
    call emit32
    pop rbx
    ret

; emit_load_rsp_disp8(dil=dst reg, sil=disp8) — MOV dst,[rsp+disp8]. RSP as a
; base always needs a SIB byte; used to read the return address pushed by
; the call that entered an inline-cache trampoline, without disturbing the
; stack (no pop), however many bytes the trampoline has itself pushed since.
global emit_load_rsp_disp8
emit_load_rsp_disp8:
    push rbx
    push r12
    mov bl, dil
    mov r12b, sil
    mov rdi, 0x48
    call emit8
    mov rdi, 0x8B
    call emit8
    mov al, 0x44                     ; mod01, rm=100 (SIB follows)
    mov cl, bl
    shl cl, 3
    or al, cl
    movzx rdi, al
    call emit8
    mov rdi, 0x24                      ; SIB: scale00 index100(none) base100(rsp)
    call emit8
    movzx rdi, r12b
    call emit8                            ; disp8
    pop r12
    pop rbx
    ret

; emit_jmp_reg(dil=reg) — indirect tail-jump through a register. FF /4
global emit_jmp_reg
emit_jmp_reg:
    push rbx
    mov bl, dil
    mov rdi, 0xFF
    call emit8
    mov al, 0xE0                       ; mod11 reg=100(/4) rm=000, base
    or al, bl
    movzx rdi, al
    call emit8
    pop rbx
    ret

; patch_rel32(rdi = address of the rel32 field to rewrite,
;             rsi = absolute target address)
;
; Stores (target - (field_addr + 4)) as the 32-bit displacement at
; field_addr — the standard x86 rel32 encoding, whether the instruction
; being patched is a `call`, a `jmp`, or a `jcc`. This is the single
; mechanism inline caching, backpatched forward branches, and on-stack
; respecialization all reduce to.
global patch_rel32
patch_rel32:
    lea rax, [rdi+4]
    sub rsi, rax                 ; rsi = target - (field+4)
    mov [rdi], esi
    ret

; patch_imm64(rdi = address of a 64-bit immediate field, rsi = value)
; Overwrites an already-emitted `mov reg, imm64` operand in place with a
; value that wasn't known yet when the instruction was emitted (e.g. a
; forward "resume here" address) — the absolute-value counterpart to
; patch_rel32's relative displacements.
global patch_imm64
patch_imm64:
    mov [rdi], rsi
    ret
