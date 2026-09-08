# lamedh-asm — freestanding x86-64 build. No libc, no Rust, no Cargo.
#
# Excluded from the Cargo workspace deliberately (there is no Cargo.toml
# here at all, so cargo never sees this directory).

AS      := nasm
ASFLAGS := -f elf64 -g -F dwarf -w+all -Isrc/
LD      := ld
BUILD   := build

CORE := src/heap.asm src/print.asm src/reader.asm src/symtab.asm \
        src/strings.asm src/floats.asm src/fileio.asm src/arrays.asm \
        src/codegen.asm src/compiler.asm

.PHONY: test clean

test: $(BUILD)/.stamp
	@tests/run.sh

$(BUILD)/.stamp:
	@mkdir -p $(BUILD)
	@touch $@

clean:
	rm -rf $(BUILD)
