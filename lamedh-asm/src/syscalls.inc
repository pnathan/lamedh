; syscalls.inc — raw x86-64 Linux syscall numbers used by lamedh-asm.
; No libc: every host interaction is a bare `syscall` instruction.

%define SYS_read         0
%define SYS_write         1
%define SYS_mmap          9
%define SYS_mprotect     10
%define SYS_exit         60

%define PROT_READ         1
%define PROT_WRITE        2
%define PROT_EXEC         4

%define MAP_PRIVATE       2
%define MAP_ANONYMOUS  0x20

%define STDIN             0
%define STDOUT            1
