; syscalls.inc — raw x86-64 Linux syscall numbers used by lamedh-asm.
; No libc: every host interaction is a bare `syscall` instruction.

%define SYS_read         0
%define SYS_write         1
%define SYS_open          2
%define SYS_close         3
%define SYS_lseek         8

%define SEEK_SET          0
%define SEEK_CUR          1
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
%define STDERR            2

%define O_RDONLY          0
%define O_WRONLY          1
%define O_CREAT          64
%define O_TRUNC         512
%define O_APPEND       1024
