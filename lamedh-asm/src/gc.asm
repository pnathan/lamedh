; gc.asm — the data heap's granule side table, free lists, and (from
; step 2 on) its deferred reference-counting collector.
; docs/spec-tco-capture-gc.md section 3.
;
; Why a side table rather than a header word: a cons cell is a bare
; 16-byte [car|cdr] pair with no header at all (tags.inc), so there is
; nowhere in the object to put a count without either widening every
; cons to 32 bytes or stealing bits from a value word. Instead, because
; the data heap is one contiguous mmap with 16-byte allocation
; granularity, granule = (raw_addr - data_heap_base) >> 4 is a dense
; index into a flat array of 8-byte entries:
;
;   [0..3]  u32 count       — heap->heap references to this object
;   [4]     u8  flags       — HEAD / PINNED / IN_ZCT / MARK / RAW
;   [5..7]  u24 ngranules   — allocation length, in 16-byte granules
;
; Only an object's *first* granule carries a meaningful entry (HEAD).
; ngranules living in the entry is what makes both a linear heap walk
; (GC-VERIFY) and freeing without a per-HDR_* size switch cheap.
;
; The table is 256 MiB / 16 * 8 = 128 MiB of *virtual* space, mapped
; once at startup and committed lazily by the kernel exactly like the
; heap it shadows — the same "an anonymous reservation costs nothing
; until touched" argument boot.asm already makes for the arenas.
;
; Nothing about any object's layout, tag, address or identity changes:
; every existing [raw+8]-style access in every other file stays valid,
; and an address is still a stable EQ identity and HASH-CODE.

%include "src/syscalls.inc"
%include "src/tags.inc"
%define ZCT_TRIGGER      65536            ; used by rc_collect above the safe-point section, so defined up here

; --- granule entry flag bits (byte 4 of an entry) ---
%define GF_HEAD    1      ; this granule begins a live allocation
%define GF_PINNED  2      ; never reclaim (symbols, baked literals, compile-time)
%define GF_IN_ZCT  4      ; already queued on the zero-count table
%define GF_MARK    8      ; found by the conservative root scan this cycle
%define GF_RAW     16     ; not a Lisp object: explicit data_free, never counted
%define GF_CONS    32     ; a bare 16-byte [car|cdr] cell: no header word to read

; ZCT capacity, in entries. Overflow is not a correctness problem (it
; sets zct_overflowed, and the collector's linear sweep picks up what
; the table could not hold) — only a performance one.
%ifndef ZCT_CAPACITY
%define ZCT_CAPACITY (1024 * 1024)
%endif
%define PIN_STACK_CAPACITY 65536

extern data_heap_base
extern data_heap_cur
extern data_heap_end

section .bss
align 8
global rc_table_base
global rc_pin_depth
global rc_bytes_live
global stack_base
rc_table_base:          resq 1
rc_pin_depth:           resq 1   ; > 0: allocations are born PINNED (compile-time)
rc_bytes_live:          resq 1   ; bytes in live (allocated, not freed) blocks
rc_bytes_since_collect: resq 1
stack_base:             resq 1   ; target-stack root-scan limit (boot.asm)

; free_lists[n] (1 <= n <= 8) threads freed runs of exactly n granules;
; free_list_large threads everything bigger. A freed run's own first
; word is the next link — LISP 1.5's free-storage list trick, no
; separate free-block header anywhere.
free_lists:      resq 9
free_list_large: resq 1

; The zero-count table: objects whose count has reached (or started at)
; zero and which are therefore *candidates* for reclamation — not
; garbage, because an uncounted reference from the stack or a register
; may still be live. Deutsch-Bobrow's whole point: pay for the stack
; scan once per collection instead of paying for an inc/dec on every
; local store.
global zct_count
global zct_overflowed
zct:            resq ZCT_CAPACITY
zct_count:      resq 1
zct_overflowed: resq 1

; rc_pin_deep's explicit worklist — an iterative walk, never host
; recursion (a deep literal must not be able to blow the host stack;
; and cdr-chaining below keeps this bounded by list *nesting* depth,
; not list length).
pin_stack:      resq PIN_STACK_CAPACITY
pin_stack_top:  resq 1
rc_verify_table: resq 1

section .rodata
heap_exhausted_msg: db "lamedh: data heap exhausted", 10
heap_exhausted_len: equ $ - heap_exhausted_msg
double_free_msg: db "lamedh: free list corrupt (double free)", 10
double_free_len: equ $ - double_free_msg

section .text

; rc_init(rdi = data heap size in bytes) — called by heap_init_all once
; the data heap itself is mapped. One entry per 16-byte granule: the
; table is exactly half the heap's size.
global rc_init
rc_init:
    shr rdi, 1
    mov rsi, rdi
    xor edi, edi
    mov edx, PROT_READ | PROT_WRITE
    mov r10d, MAP_PRIVATE | MAP_ANONYMOUS
    mov r8d, -1
    xor r9d, r9d
    mov eax, SYS_mmap
    syscall
    mov [rc_table_base], rax
    ret

; ENTRY_OF dst, raw — dst = address of raw's granule entry.
; Clobbers dst only.
%macro ENTRY_OF 2
    mov %1, %2
    sub %1, [data_heap_base]
    shr %1, 4
    lea %1, [%1*8]
    add %1, [rc_table_base]
%endmacro

; alloc_granules(rdi = ngranules >= 1, rsi = extra flag bits) -> rax raw
;
; Exact-fit free list first, bump second. Exact fit (rather than the
; spec's first-fit-with-split for the large class) is deliberate: every
; large allocation this kernel makes is a repeat of a fixed size (the
; 64 KB PRINC-TO-STRING capture buffer, a port's read scratch), so an
; exact-fit list recycles them perfectly with no splitting, no
; coalescing, and no possibility of a partially-consumed run confusing
; the linear heap walk GC-VERIFY depends on.
;
; Internal; clobbers rax, rcx, rdi, rsi, r8, r9, r10.
alloc_granules:
    cmp rdi, 8
    ja .large
    mov rax, [free_lists + rdi*8]
    test rax, rax
    jz .bump
    mov rcx, [rax]                       ; the run's own first word = next link
    mov [free_lists + rdi*8], rcx
    jmp .have
.large:
    lea r9, [rel free_list_large]        ; r9 = address of the link pointing at rax
    mov rax, [r9]
.walk:
    test rax, rax
    jz .bump
    ENTRY_OF r10, rax
    mov r8d, [r10+4]
    shr r8d, 8                            ; r8 = this run's ngranules
    cmp r8, rdi
    je .unlink
    mov r9, rax
    mov rax, [rax]
    jmp .walk
.unlink:
    mov rcx, [rax]
    mov [r9], rcx
    jmp .have
.bump:
    mov rax, [data_heap_cur]
    mov rcx, rdi
    shl rcx, 4
    add rcx, rax
    cmp rcx, [data_heap_end]
    ja .exhausted
    mov [data_heap_cur], rcx
.have:
    ENTRY_OF r10, rax
    ; A free-list pop must never hand back a block that is still marked
    ; live: that would mean a double free threaded the same run twice.
    test byte [r10+4], GF_HEAD
    jnz .double_free
    mov dword [r10], 0                     ; count = 0
    mov r8, rdi
    shl r8, 8                                ; ngranules in bits 8..31
    or r8, GF_HEAD
    or r8, rsi
    test rsi, GF_RAW
    jnz .no_pin
    cmp qword [rc_pin_depth], 0
    je .no_pin
    or r8, GF_PINNED
.no_pin:
    mov [r10+4], r8d
    mov rcx, rdi
    shl rcx, 4
    add [rc_bytes_live], rcx
    add [rc_bytes_since_collect], rcx
    ; A fresh object has count 0 — no *heap* reference to it exists yet,
    ; only the register its allocator is about to return it in — so it
    ; is born on the zero-count table. Raw buffers (freed explicitly)
    ; and pinned objects (never freed) are not.
    test r8, GF_RAW | GF_PINNED
    jnz .no_zct
    mov rcx, [zct_count]
    cmp rcx, ZCT_CAPACITY
    jae .zct_full
    mov [zct + rcx*8], rax
    inc qword [zct_count]
    or byte [r10+4], GF_IN_ZCT
    ret
.zct_full:
    mov qword [zct_overflowed], 1
.no_zct:
    ret
.double_free:
    mov edi, STDERR
    lea rsi, [rel double_free_msg]
    mov edx, double_free_len
    mov eax, SYS_write
    syscall
    mov edi, 1
    mov eax, SYS_exit
    syscall
.exhausted:
    mov edi, STDERR
    lea rsi, [rel heap_exhausted_msg]
    mov edx, heap_exhausted_len
    mov eax, SYS_write
    syscall
    mov edi, 1
    mov eax, SYS_exit
    syscall

; data_alloc(rdi = size in bytes) -> rax = raw pointer, 16-byte aligned.
;
; Callers rely on this clobbering nothing but rax and rdi — reader.asm's
; `cons` holds the cdr in rsi across it, floats.asm's make_float holds
; the double in xmm0 — so everything else is saved here rather than
; audited at 20 call sites.
global data_alloc
data_alloc:
    push rsi
    push rcx
    push r8
    push r9
    push r10
    add rdi, 15
    shr rdi, 4
    jnz .go
    mov edi, 1                            ; a zero-byte request still gets a granule
.go:
    xor esi, esi
    call alloc_granules
    pop r10
    pop r9
    pop r8
    pop rcx
    pop rsi
    ret

; data_alloc_cons() -> rax = raw 16-byte cell, flagged GF_CONS.
;
; A cons is the one heap object with no header word, so the linear heap
; walk (rc_free's child enumeration, GC-VERIFY) cannot recognize it by
; reading [raw+0] — that word is the car. The flag in its granule entry
; is what tells the walker "two tagged slots, no header". reader.asm's
; `cons` is the only caller.
global data_alloc_cons
data_alloc_cons:
    push rsi
    push rcx
    push r8
    push r9
    push r10
    mov edi, 1
    mov esi, GF_CONS
    call alloc_granules
    pop r10
    pop r9
    pop r8
    pop rcx
    pop rsi
    ret

; data_alloc_raw(rdi = size) -> rax = raw pointer to a scratch buffer
; that is NOT a Lisp object: never counted, never scanned, never on the
; ZCT, freed explicitly by data_free. PRINC-TO-STRING's capture buffer,
; file_read's and the port layer's read/write scratch. Their lifetimes
; are exactly stack-shaped, so explicit freeing is both simpler and
; tighter than anything the collector could do for them.
global data_alloc_raw
data_alloc_raw:
    push rsi
    push rcx
    push r8
    push r9
    push r10
    add rdi, 15
    shr rdi, 4
    jnz .go
    mov edi, 1
.go:
    mov esi, GF_RAW
    call alloc_granules
    pop r10
    pop r9
    pop r8
    pop rcx
    pop rsi
    ret

; data_free(rdi = raw pointer previously returned by data_alloc_raw or
; data_alloc) — threads the run onto its size class's free list. A
; pointer whose granule is not a live HEAD is ignored rather than
; corrupting a free list (a double free must not be able to thread the
; same run twice; alloc_granules asserts the other half of this).
; Clobbers nothing.
global data_free
data_free:
    push rax
    push rcx
    push rdx
    push r8
    push r9
    push r10
    ENTRY_OF r10, rdi
    mov r8d, [r10+4]
    test r8b, GF_HEAD
    jz .out
    shr r8d, 8                             ; r8 = ngranules
    mov dword [r10], 0                     ; count = 0
    mov r9d, r8d
    shl r9d, 8                              ; flags byte cleared, length kept
    mov [r10+4], r9d
    mov rcx, r8
    shl rcx, 4
    sub [rc_bytes_live], rcx
    cmp r8, 8
    ja .large
    mov rax, [free_lists + r8*8]
    mov [rdi], rax
    mov [free_lists + r8*8], rdi
    jmp .out
.large:
    mov rax, [free_list_large]
    mov [rdi], rax
    mov [free_list_large], rdi
.out:
    pop r10
    pop r9
    pop r8
    pop rdx
    pop rcx
    pop rax
    ret

; --- observability primitives (compiled as nullary hostcalls) --------

; (HEAP-BYTES-USED) -> bump-pointer high-water mark, in bytes. Does not
; retreat when memory is reclaimed — freed granules are reused from the
; free lists, so a program whose live set is bounded stops advancing
; this number, which is exactly the observable the spec's reclamation
; test asserts on.
global heap_bytes_used
heap_bytes_used:
    mov rax, [data_heap_cur]
    sub rax, [data_heap_base]
    TO_FIXNUM rax
    ret

; (HEAP-BYTES-LIVE) -> bytes currently in allocated, not-yet-freed
; blocks (every allocation, counted or raw).
global heap_bytes_live
heap_bytes_live:
    mov rax, [rc_bytes_live]
    TO_FIXNUM rax
    ret

; ====================================================================
; Deferred reference counting (docs/spec-tco-capture-gc.md 3.3)
;
; A count records heap->heap references ONLY: a tagged pointer stored
; in a cons cell, a closure's captured slot, an array/record/condition/
; port slot, or a symbol's value/macro/plist cell. References from the
; target stack, the host stack, registers and catch frames are NOT
; counted — they are found by a conservative scan at a safe point
; instead. That asymmetry is the entire point: this compiler spills
; every local to a [rbp+disp] slot and pushes every intermediate, so
; counting stack references would mean an inc/dec pair around nearly
; every store the compiler emits, plus decrementing every slot of every
; frame on exit — including on native_throw's longjmp, which cannot
; visit the frames it skips.
;
; Consequence: a count of zero does not mean garbage. It means
; *candidate*, which is what the ZCT holds.
; ====================================================================

; RC_RESOLVE — rdi(tagged) -> r10 = entry address, or jump to %1 if this
; is not a countable heap pointer (immediate/fixnum, out of arena, not
; an allocation head, or pinned). Clobbers rax, r10.
%macro RC_RESOLVE 1
    mov rax, rdi
    and eax, TAG_MASK
    dec eax
    cmp eax, 1                           ; tags 01 (cons) and 10 (heapobj) only
    ja %1
    mov rax, rdi
    UNTAG_PTR rax
    cmp rax, [data_heap_base]
    jb %1
    cmp rax, [data_heap_cur]
    jae %1
    ENTRY_OF r10, rax
    mov r10d, dword [r10+4]
    test r10b, GF_HEAD
    jz %1
    test r10b, GF_PINNED
    jnz %1
    ENTRY_OF r10, rax
%endmacro

; rc_inc(rdi = tagged value) — clobbers nothing (flags aside).
global rc_inc
rc_inc:
    push rax
    push r10
    RC_RESOLVE .out
    inc dword [r10]
    jz .saturate                          ; wrapped past 2^32 — pin instead
.out:
    pop r10
    pop rax
    ret
.saturate:
    mov dword [r10], 0x7FFFFFFF
    or byte [r10+4], GF_PINNED
    jmp .out

; rc_dec(rdi = tagged value) — clobbers nothing (flags aside). On a
; drop to zero the object is queued on the ZCT, never freed here:
; freeing on decrement is what makes naive RC's worst case a recursive
; walk of a million-cell list at an arbitrary point in a program.
global rc_dec
rc_dec:
    push rax
    push rcx
    push r10
    RC_RESOLVE .out
    mov eax, dword [r10]
    test eax, eax
    jz .out                               ; already zero: already a candidate
    dec eax
    mov dword [r10], eax
    test eax, eax
    jnz .out
    test byte [r10+4], GF_IN_ZCT
    jnz .out
    mov rcx, [zct_count]
    cmp rcx, ZCT_CAPACITY
    jae .full
    mov rax, rdi
    UNTAG_PTR rax
    mov [zct + rcx*8], rax
    inc qword [zct_count]
    or byte [r10+4], GF_IN_ZCT
    jmp .out
.full:
    mov qword [zct_overflowed], 1
.out:
    pop r10
    pop rcx
    pop rax
    ret

; rc_walk_children(rdi = raw addr, rsi = flags byte, rdx = callback)
;
; Calls callback(rdi = address of a slot holding a tagged word) once per
; heap slot of the object. The single authority on "what are this
; object's children" — rc_register, rc_free and GC-VERIFY all go
; through it, so a new heapobj kind is taught to all three at once
; (docs/spec-tco-capture-gc.md 3.7 item 5).
;
; Deliberately NOT enumerated: a symbol's next-in-bucket link and name
; bytes, a port's fd/flags/mem_pos, a closure's code pointer/nargs/
; nfree, a typed array's raw slots, a string's bytes, a float's double.
; The enumerated set must match exactly what the inc/dec sites maintain
; — GC-VERIFY compares the two, so any divergence is a test failure,
; not a silent corruption.
; Callback must preserve rbx, rbp, r12-r15.
global rc_walk_children
rc_walk_children:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov r14, rdx                          ; callback
    mov rbx, rdi                          ; raw addr
    xor r13, r13                          ; contiguous run length
    xor r12, r12                          ; contiguous run base
    test sil, GF_RAW
    jnz .run
    test sil, GF_CONS
    jnz .cons
    mov rax, [rbx]                        ; header word
    cmp rax, HDR_CLOSURE
    je .closure
    cmp rax, HDR_OPERATIVE
    je .closure
    cmp rax, HDR_ARRAY
    je .array
    cmp rax, HDR_RECORD
    je .record
    cmp rax, HDR_CONDITION
    je .condition
    cmp rax, HDR_SYMBOL
    je .three_slots
    cmp rax, HDR_PORT
    je .three_slots
    jmp .run                              ; string / float / typed array: no children
.cons:
    mov r12, rbx
    mov r13, 2
    jmp .run
.closure:
    lea r12, [rbx+32]
    mov r13, [rbx+24]                     ; nfree
    jmp .run
.array:
    lea r12, [rbx+16]
    mov r13, [rbx+8]                      ; len
    jmp .run
.record:
    lea rdi, [rbx+8]                      ; brand symbol
    call r14
    lea r12, [rbx+24]
    mov r13, [rbx+16]                     ; nfields
    jmp .run
.condition:
    lea r12, [rbx+8]
    mov r13, 2                            ; message, data
    jmp .run
.three_slots:
    ; symbol: value/macro/plist. port: kind/name/mem_buf. Same offsets.
    lea rdi, [rbx+16]
    call r14
    lea rdi, [rbx+24]
    call r14
    lea rdi, [rbx+40]
    call r14
    jmp .run
.run:
    xor r15, r15
.loop:
    cmp r15, r13
    jae .done
    lea rdi, [r12+r15*8]
    call r14
    inc r15
    jmp .loop
.done:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; callbacks for rc_walk_children
cb_inc:
    mov rdi, [rdi]
    jmp rc_inc
cb_dec:
    mov rdi, [rdi]
    jmp rc_dec

; rc_register(rdi = tagged object) -> rax = rdi
;
; "This object now exists and its slots are filled": count every heap
; child it holds. The creation-site counterpart of rc_store_slot.
; Returns its argument so an emitted construction sequence can call it
; through rax and carry straight on (compiler.asm).
global rc_register
rc_register:
    push rbx
    push rdi
    mov rbx, rdi
    mov rax, rdi
    and eax, TAG_MASK
    dec eax
    cmp eax, 1
    ja .out
    mov rdi, rbx
    UNTAG_PTR rdi
    cmp rdi, [data_heap_base]
    jb .out
    cmp rdi, [data_heap_cur]
    jae .out
    ENTRY_OF rax, rdi
    movzx esi, byte [rax+4]
    test sil, GF_HEAD
    jz .out
    lea rdx, [rel cb_inc]
    call rc_walk_children
.out:
    pop rdi
    pop rbx
    mov rax, rdi
    ret

; rc_store_slot(rdi = address of a heap slot, rsi = new tagged value)
;
; The mutation-site primitive. Increment before decrementing, so that
; (STORE a i (FETCH a i)) — new and old the same object, count 1 —
; cannot transiently drop to zero and queue a live object.
global rc_store_slot
rc_store_slot:
    push rbx
    push r12
    mov rbx, rdi
    mov r12, [rdi]                        ; old value
    mov [rbx], rsi
    mov rdi, rsi
    call rc_inc
    mov rdi, r12
    call rc_dec
    pop r12
    pop rbx
    ret

; rc_store_cell(rdi = address of a symbol's value/macro/plist cell,
;               rsi = new tagged value) -> rax = rsi
;
; rc_store_slot with the emitted-call shape compile_define/compile_setq
; need: returns the stored value, since that is what the form itself
; evaluates to.
global rc_store_cell
rc_store_cell:
    push rsi
    call rc_store_slot
    pop rax
    ret

; --- pinning ---------------------------------------------------------

; rc_pin_deep(rdi = tagged value) -> rax = rdi
;
; Mark this object and everything reachable from it immortal. Called
; wherever a heap value is baked into emitted code as an immediate: at
; that moment a live reference to it exists that no scan will ever see
; (it is an operand byte inside the code heap, not a word on any stack),
; so it must never be reclaimed. This is what keeps a runtime
; READ-FROM-STRING result safe once EVAL bakes it.
global rc_pin_deep
rc_pin_deep:
    push rbx
    push r12
    mov r12, rdi
    mov qword [pin_stack_top], 0
    call .push
.drain:
    mov rax, [pin_stack_top]
    test rax, rax
    jz .done
    dec rax
    mov [pin_stack_top], rax
    mov rbx, [pin_stack + rax*8]          ; raw address of a pinned object
    ENTRY_OF rax, rbx
    movzx esi, byte [rax+4]
    mov rdi, rbx
    lea rdx, [rel cb_pin]
    call rc_walk_children
    jmp .drain
.done:
    mov rax, r12
    pop r12
    pop rbx
    ret
; .push(rdi = tagged): pin it if countable-and-unpinned, and queue its
; children for the same treatment.
.push:
    push rax
    push r10
    mov rax, rdi
    and eax, TAG_MASK
    dec eax
    cmp eax, 1
    ja .push_out
    mov rax, rdi
    UNTAG_PTR rax
    cmp rax, [data_heap_base]
    jb .push_out
    cmp rax, [data_heap_cur]
    jae .push_out
    ENTRY_OF r10, rax
    test byte [r10+4], GF_HEAD
    jz .push_out
    test byte [r10+4], GF_PINNED
    jnz .push_out                          ; already immortal: stops cycles too
    or byte [r10+4], GF_PINNED
    mov r10, [pin_stack_top]
    cmp r10, PIN_STACK_CAPACITY
    jae .push_out                          ; pathologically deep literal: the
                                            ; object itself is pinned, its
                                            ; children stay counted (safe: they
                                            ; are still referenced from it, and
                                            ; it is never freed)
    mov [pin_stack + r10*8], rax
    inc qword [pin_stack_top]
.push_out:
    pop r10
    pop rax
    ret
cb_pin:
    mov rdi, [rdi]
    jmp rc_pin_deep.push

; rc_pin(rdi = raw address) — pin one object, shallowly. Used for
; symbols, which are interned, address-baked into emitted code, and
; hold the global value cells: never reclaimable, and their *children*
; (a symbol's value) very much are.
global rc_pin
rc_pin:
    push rax
    ENTRY_OF rax, rdi
    or byte [rax+4], GF_PINNED
    pop rax
    ret

; rc_pin_enter() / rc_pin_leave() — bracket a compile. Everything the
; compiler itself allocates while compiling (macro expansions, scope
; lists, synthetic forms) is born pinned: a form still being compiled
; is referenced only from host registers and host stack frames, and v0
; accepts leaking it exactly as today rather than making the compiler
; itself GC-safe.
global rc_pin_enter
rc_pin_enter:
    inc qword [rc_pin_depth]
    ret
global rc_pin_leave
rc_pin_leave:
    cmp qword [rc_pin_depth], 0
    je .out
    dec qword [rc_pin_depth]
.out:
    ret

; (REFCOUNT x) -> the object's current heap->heap reference count, or
; -1 for a value that is not a countable heap object (an immediate, a
; fixnum, or a pinned object such as a symbol or a baked literal).
global rc_refcount
rc_refcount:
    push r10
    RC_RESOLVE .none
    mov eax, dword [r10]
    TO_FIXNUM rax
    pop r10
    ret
.none:
    mov rax, -1
    TO_FIXNUM rax
    pop r10
    ret

; ====================================================================
; (GC-VERIFY) — a full, independent recount.
;
; Walk the whole heap linearly (which the ngranules field in each entry
; makes possible: every granule in [base, cur) belongs to exactly one
; run whose head entry records its length), recompute every unpinned
; object's count from scratch by enumerating every heap slot of every
; live object, and compare with the side table. Print the first
; mismatch and return NIL, else T.
;
; This is the tool that turns "did I instrument every creation and
; mutation site?" from a code-reading exercise into a test: a missed
; rc_inc anywhere shows up here as a count mismatch at the end of a
; stdlib load, rather than as a corrupted list three programs later.
; ====================================================================

extern write_buf
extern print_fixnum

section .rodata
verify_msg1: db "GC-VERIFY mismatch: granule "
verify_msg1_len: equ $ - verify_msg1
verify_msg2: db " header "
verify_msg2_len: equ $ - verify_msg2
verify_msg3: db " expected "
verify_msg3_len: equ $ - verify_msg3
verify_msg4: db " actual "
verify_msg4_len: equ $ - verify_msg4
verify_msg5: db 10
verify_msg5_len: equ $ - verify_msg5
verify_corrupt: db "GC-VERIFY: heap walk lost (zero-length run)", 10
verify_corrupt_len: equ $ - verify_corrupt

section .text

cb_verify_tally:
    push rax
    push rdi
    push r10
    mov rdi, [rdi]
    mov rax, rdi
    and eax, TAG_MASK
    dec eax
    cmp eax, 1
    ja .out
    mov rax, rdi
    UNTAG_PTR rax
    cmp rax, [data_heap_base]
    jb .out
    cmp rax, [data_heap_cur]
    jae .out
    ENTRY_OF r10, rax
    test byte [r10+4], GF_HEAD
    jz .out
    sub rax, [data_heap_base]
    shr rax, 4
    add rax, rax
    add rax, rax                            ; granule * 4
    add rax, [rc_verify_table]
    inc dword [rax]
.out:
    pop r10
    pop rdi
    pop rax
    ret

global gc_verify
gc_verify:
    push rbx
    push r12
    push r13
    push r14
    push r15
    ; A fresh anonymous mapping per call: guaranteed zero-filled, so
    ; there is no stale tally from a previous verification to clear.
    mov rsi, [data_heap_end]
    sub rsi, [data_heap_base]
    shr rsi, 2                              ; 4 bytes per 16-byte granule
    xor edi, edi
    mov edx, PROT_READ | PROT_WRITE
    mov r10d, MAP_PRIVATE | MAP_ANONYMOUS
    mov r8d, -1
    xor r9d, r9d
    mov eax, SYS_mmap
    syscall
    mov [rc_verify_table], rax

    ; --- pass 1: tally every heap->heap reference ---
    mov rbx, [data_heap_base]
.walk1:
    cmp rbx, [data_heap_cur]
    jae .walk1_done
    ENTRY_OF r12, rbx
    mov r13d, [r12+4]
    mov r14d, r13d
    shr r14d, 8
    test r14d, r14d
    jz .corrupt
    test r13b, GF_HEAD
    jz .next1
    test r13b, GF_RAW
    jnz .next1
    mov rdi, rbx
    movzx esi, r13b
    lea rdx, [rel cb_verify_tally]
    call rc_walk_children
.next1:
    ENTRY_OF r12, rbx
    mov r14d, [r12+4]
    shr r14d, 8
    shl r14, 4
    add rbx, r14
    jmp .walk1
.walk1_done:

    ; --- pass 2: compare ---
    mov rbx, [data_heap_base]
.walk2:
    cmp rbx, [data_heap_cur]
    jae .ok
    ENTRY_OF r12, rbx
    mov r13d, [r12+4]
    test r13b, GF_HEAD
    jz .next2
    test r13b, GF_RAW
    jnz .next2
    test r13b, GF_PINNED
    jnz .next2                              ; pinned objects are not counted
    mov r14, rbx
    sub r14, [data_heap_base]
    shr r14, 4
    lea r14, [r14*4]
    add r14, [rc_verify_table]
    mov r14d, [r14]                          ; expected
    mov r15d, [r12]                            ; actual
    cmp r14d, r15d
    jne .mismatch
.next2:
    ENTRY_OF r12, rbx
    mov r14d, [r12+4]
    shr r14d, 8
    shl r14, 4
    add rbx, r14
    jmp .walk2

.ok:
    call .unmap
    mov rax, IMM_TRUE
    jmp .out

.mismatch:
    push r14
    push r15
    lea rsi, [rel verify_msg1]
    mov edx, verify_msg1_len
    call write_buf
    mov rdi, rbx
    sub rdi, [data_heap_base]
    shr rdi, 4                                ; granule index, a small number
    TO_FIXNUM rdi
    call print_fixnum
    lea rsi, [rel verify_msg2]
    mov edx, verify_msg2_len
    call write_buf
    mov rdi, [rbx]
    TO_FIXNUM rdi
    call print_fixnum
    lea rsi, [rel verify_msg3]
    mov edx, verify_msg3_len
    call write_buf
    pop r15
    pop r14
    mov rdi, r14
    TO_FIXNUM rdi
    call print_fixnum
    lea rsi, [rel verify_msg4]
    mov edx, verify_msg4_len
    call write_buf
    mov rdi, r15
    TO_FIXNUM rdi
    call print_fixnum
    lea rsi, [rel verify_msg5]
    mov edx, verify_msg5_len
    call write_buf
    call .unmap
    mov rax, IMM_NIL
    jmp .out

.corrupt:
    lea rsi, [rel verify_corrupt]
    mov edx, verify_corrupt_len
    call write_buf
    call .unmap
    mov rax, IMM_NIL
.out:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

.unmap:
    push rax
    push rdi
    push rsi
    mov rdi, [rc_verify_table]
    mov rsi, [data_heap_end]
    sub rsi, [data_heap_base]
    shr rsi, 2
    mov eax, SYS_munmap
    syscall
    mov qword [rc_verify_table], 0
    pop rsi
    pop rdi
    pop rax
    ret

; ====================================================================
; Collection (docs/spec-tco-capture-gc.md 3.5)
;
; A collection is: (1) conservatively scan the roots — the target stack
; (which is the host stack: compiled code and the compiler run on the
; same one), the catch stack, the compiler's own scratch cells, and all
; 16 GPRs as pushed below — marking every allocation head they appear
; to point at; (2) drain the ZCT, freeing every member that is
; unmarked, unpinned and still at count zero, and re-queueing the rest;
; (3) unmark.
;
; Step (1) is what makes step (2) sound: a count of zero only means "no
; *heap* reference", and the scan is what accounts for the stack and
; register references that were deliberately never counted.
; ====================================================================

extern catch_stack
extern catch_stack_top
extern macroexpand_memo
extern macroexpand_memo_end
extern capture_memo
extern capture_memo_end
extern current_scope
extern rest_sym_scratch

; scan_roots_range(rdi = start, rsi = end, rdx = 1 to set MARK / 0 to clear)
; Conservative: any word whose tag bits are 01/10 and whose address
; lands on an allocation head inside the arena is treated as a
; reference. False positives keep dead objects alive for one cycle;
; there are no false negatives, which is the direction that matters.
scan_roots_range:
    push rbx
.loop:
    cmp rdi, rsi
    jae .done
    mov rbx, [rdi]
    mov rax, rbx
    and eax, TAG_MASK
    dec eax
    cmp eax, 1
    ja .next
    mov rax, rbx
    UNTAG_PTR rax
    cmp rax, [data_heap_base]
    jb .next
    cmp rax, [data_heap_cur]
    jae .next
    ENTRY_OF r10, rax
    test byte [r10+4], GF_HEAD
    jz .next
    test rdx, rdx
    jz .clear
    or byte [r10+4], GF_MARK
    jmp .next
.clear:
    and byte [r10+4], ~GF_MARK
.next:
    add rdi, 8
    jmp .loop
.done:
    pop rbx
    ret

; scan_all_roots(rdi = the stack scan's low bound, rsi = 1 mark / 0 clear)
scan_all_roots:
    push rbx
    push r12
    mov r12, rsi                          ; mark/clear
    mov rbx, rdi                          ; stack low bound
    mov rsi, [stack_base]
    mov rdx, r12
    call scan_roots_range
    ; the catch stack: a frame's tag word may be any Lisp value, and an
    ; UNWIND-PROTECT marker frame holds its cleanup closure there
    lea rdi, [rel catch_stack]
    mov rax, [catch_stack_top]
    shl rax, 5                             ; 32 bytes per frame
    lea rsi, [rdi+rax]
    mov rdx, r12
    call scan_roots_range
    ; compile-time scratch cells that can hold a tagged value: the
    ; lexical scope list and the &REST parameter symbol. (Every other
    ; host .bss cell audited in 3.4 holds a raw pointer, a fixnum, or a
    ; pinned symbol.)
    lea rdi, [rel current_scope]
    lea rsi, [rdi+8]
    mov rdx, r12
    call scan_roots_range
    lea rdi, [rel rest_sym_scratch]
    lea rsi, [rdi+8]
    mov rdx, r12
    call scan_roots_range
    ; The macro-expansion and capture memo tables. These are cleared at
    ; the start of every top-level compile and only ever read during
    ; one, so a stale entry could not be *used* after a collection —
    ; but they are scanned anyway rather than relying on that argument
    ; holding for every future caller of compile_thunk.
    lea rdi, [rel macroexpand_memo]
    lea rsi, [rel macroexpand_memo_end]
    mov rdx, r12
    call scan_roots_range
    lea rdi, [rel capture_memo]
    lea rsi, [rel capture_memo_end]
    mov rdx, r12
    call scan_roots_range
    pop r12
    pop rbx
    ret

; rc_free(rdi = raw address of an allocation head) — release one object:
; decrement everything it points at (which may put those on the ZCT, to
; be handled by the drain loop, never by recursion — freeing a
; million-cell list must not recurse a million deep on the host stack),
; then thread its granule run onto the matching free list.
rc_free:
    push rbx
    push r12
    mov rbx, rdi
    ENTRY_OF r12, rbx
    mov eax, [r12+4]
    test al, GF_HEAD
    jz .out                               ; not live: refuse to free twice
    ; children first: threading the run onto a free list overwrites
    ; word 0, which for a cons *is* the car.
    movzx esi, al
    mov rdi, rbx
    lea rdx, [rel cb_dec]
    call rc_walk_children
    mov eax, [r12+4]
    shr eax, 8                             ; ngranules
    mov dword [r12], 0                      ; count = 0
    mov ecx, eax
    shl ecx, 8                               ; flags cleared, length kept
    mov [r12+4], ecx
    mov rcx, rax
    shl rcx, 4
    sub [rc_bytes_live], rcx
    cmp rax, 8
    ja .large
    mov rcx, [free_lists + rax*8]
    mov [rbx], rcx
    mov [free_lists + rax*8], rbx
    jmp .out
.large:
    mov rcx, [free_list_large]
    mov [rbx], rcx
    mov [free_list_large], rbx
.out:
    pop r12
    pop rbx
    ret

; zct_drain() — the collection proper. Read cursor r12, write cursor
; r13 over the same array: survivors compact toward the front, and the
; decrements rc_free performs append new candidates at the end, which
; the same loop then reaches. r13 <= r12 <= zct_count always, so an
; append can never overwrite an unread entry.
zct_drain:
    push rbx
    push r12
    push r13
    push r14
    xor r12, r12
    xor r13, r13
.loop:
    cmp r12, [zct_count]
    jae .done
    mov rbx, [zct + r12*8]
    inc r12
    ENTRY_OF r14, rbx
    mov eax, [r14+4]
    test al, GF_HEAD
    jz .drop                               ; already freed this cycle
    test al, GF_PINNED
    jnz .drop
    cmp dword [r14], 0
    jne .drop                              ; resurrected by a heap store
    test al, GF_MARK
    jnz .keep                              ; a root still points at it
    mov rdi, rbx
    call rc_free
    jmp .loop
.keep:
    mov [zct + r13*8], rbx
    inc r13
    jmp .loop
.drop:
    and byte [r14+4], ~GF_IN_ZCT
    jmp .loop
.done:
    mov [zct_count], r13
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; rc_collect() -> rax = IMM_TRUE. Every GPR is pushed first so that a
; tagged pointer held only in a callee-saved register of some host
; routine up the call chain (invoke_macro's r12, say) is covered by the
; stack scan without that routine knowing anything about the collector.
global rc_collect
rc_collect:
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    mov rbx, rsp                          ; low bound of the root scan

    mov rdi, rbx
    mov rsi, 1
    call scan_all_roots
    call zct_drain
    ; A ZCT overflow means some candidates were never recorded. The
    ; linear sweep is what turns that from a permanent leak into a
    ; merely slow collection: walk every allocation head in the arena
    ; and free the ones the drain would have freed had it seen them.
    cmp qword [zct_overflowed], 0
    je .no_sweep
    call rc_sweep
    call zct_drain                        ; the sweep's own decrements
    mov qword [zct_overflowed], 0
.no_sweep:
    mov rdi, rbx
    xor rsi, rsi
    call scan_all_roots
    ; Adapt the ZCT trigger to what this collection could not free (see
    ; zct_trigger's own comment): max(ZCT_TRIGGER, 2 * retained).
    mov rax, [zct_count]
    shl rax, 1
    cmp rax, ZCT_TRIGGER
    jae .trigger_ok
    mov rax, ZCT_TRIGGER
.trigger_ok:
    mov [zct_trigger], rax
    mov qword [rc_bytes_since_collect], 0

    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    mov rax, IMM_TRUE
    ret

; ====================================================================
; Safe points (docs/spec-tco-capture-gc.md 3.5)
;
; The compiler emits a call to rc_safepoint at every compiled
; function's entry, immediately after the prologue has extracted the
; free variables and built any &REST list — i.e. at the one place where
; every value in play is a *tagged* word in a frame slot, an argument
; register or on the stack, and no host routine is holding a raw
; untagged address or an interior pointer mid-construction. That
; property, not the ZCT, is what makes the conservative scan sound;
; it is also why data_alloc must never collect.
; ====================================================================

%define BYTES_TRIGGER    (16 * 1024 * 1024)

section .bss
global gc_force
gc_force: resq 1
section .data
; zct_trigger — the ZCT-size trigger, adaptive rather than the fixed
; ZCT_TRIGGER it started as. An entry the drain KEEPS (a zero-count
; object some root still points at — every cons a deep non-tail
; recursion is holding in a frame slot, say) stays in the ZCT after the
; collection. With a fixed trigger, once more than ZCT_TRIGGER such
; entries were retained every safe point — every function call — ran a
; full collection, each one a conservative scan of the whole native
; stack: 100,000 frames deep, that was ~60 s for a program that runs in
; 20 ms at 50,000 (the cliff was the retained count crossing 65,536).
; rc_collect now raises the trigger to twice whatever the drain kept,
; so the next collection is earned by real new garbage, not by the
; same live set being re-scanned.
zct_trigger: dq ZCT_TRIGGER
section .text

section .text

; rc_safepoint() — clobbers nothing. Three compares on the fast path.
global rc_safepoint
rc_safepoint:
    cmp qword [gc_force], 0
    jne .go
    push rax
    mov rax, [zct_count]
    cmp rax, [zct_trigger]
    pop rax                               ; flags survive the pop
    jae .go
    cmp qword [rc_bytes_since_collect], BYTES_TRIGGER
    jae .go
    ret
.go:
    mov qword [gc_force], 0
    push rax
    call rc_collect
    pop rax
    ret

; rc_sweep() — the ZCT-overflow fallback. Walks the arena run by run
; (ngranules in each head entry is what makes this possible without a
; per-header size switch) and frees every object the drain loop's own
; predicate accepts. Only ever reached after an overflow.
rc_sweep:
    push rbx
    push r12
    mov rbx, [data_heap_base]
.loop:
    cmp rbx, [data_heap_cur]
    jae .done
    ENTRY_OF r12, rbx
    mov eax, [r12+4]
    shr eax, 8
    test eax, eax
    jz .done                              ; a zero-length run: the walk
                                           ; has lost the thread; stop
                                           ; rather than run off the end
    push rax                                ; ngranules of THIS run
    mov eax, [r12+4]
    test al, GF_HEAD
    jz .next
    test al, GF_RAW | GF_PINNED | GF_MARK
    jnz .next
    cmp dword [r12], 0
    jne .next
    mov rdi, rbx
    call rc_free
.next:
    pop rax
    shl rax, 4
    add rbx, rax
    jmp .loop
.done:
    pop r12
    pop rbx
    ret
