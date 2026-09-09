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
        src/conditions.asm src/overflow.asm src/native_errors.asm \
        src/codegen.asm src/compiler.asm

.PHONY: test clean lamedhc

test: $(BUILD)/.stamp
	@tests/run.sh

# lamedhc: the real (non-test) driver — `lamedhc path/to/program.lisp`
# or `producer | lamedhc` reads a whole source text and runs every
# top-level form in it (file_runner.asm), the concrete first step
# toward running ../examples/*/main.lisp (README Roadmap).
lamedhc: $(BUILD)/.stamp
	@objs=""; \
	for f in src/boot.asm $(CORE) src/file_runner.asm; do \
		obj=$(BUILD)/$$(basename $${f%.asm}).o; \
		$(AS) $(ASFLAGS) $$f -o $$obj || exit 1; \
		objs="$$objs $$obj"; \
	done; \
	$(LD) -static -nostdlib -o $(BUILD)/lamedhc $$objs

$(BUILD)/.stamp:
	@mkdir -p $(BUILD)
	@touch $@

clean:
	rm -rf $(BUILD)
