# Assembler

## Immediates

Plain decimal, `0x` hex, `0o` octal: `42`, `0xFF`, `0o17`.

Character literals: `'A'` expands to the ASCII value of the character. Supported escape
sequences: `\n`, `\t`, `\0`, `\\`, `\'`.

Width is determined by the destination register view: 32-bit for `rw`, 16-bit for `rh`, 8-bit
for `rb`. The assembler enforces the range and rejects values that do not fit.

## Sections

A source file can contain up to three sections. The default section when no directive is
present is `.code`.

```
.rodata
    greeting: db "Hello", 0
    table:    dw 0x12345678, 0xDEADBEEF

.code
main:
    MOV rw0, greeting
    HALT

.data
    counter: dw 0
```

Section layout at runtime is determined by the loader. Each non-empty section is placed at the
next 4 kb boundary after the previous region, following the fault info region at the start of
the address space. The stack is always mapped to `0xFFC00000..0xFFFFFFFF`. See the VM
reference for the full layout.

Labels defined in `.rodata` or `.data` resolve to their loaded address and are usable as
`imm32` operands in `.code`.

## Data directives

Used inside `.rodata` or `.data` sections to emit raw bytes.

```
db 0x41, 0x42, 0x43         # emit bytes
db "Hello", 0               # string shorthand, null must be explicit
dh 0x1234, 0x5678           # emit 16-bit big-endian half-words
dw 0x12345678, 0xDEADBEEF   # emit 32-bit big-endian words
dw some_label               # emit a label address as a 32-bit word
```

`db` accepts any mix of integer literals and quoted strings in a single directive. A string is
expanded to its ASCII bytes with no implicit null terminator; add a trailing `, 0` if needed.

`dw` accepts label names as operands. The assembler emits a relocation entry for each one so
the loader can patch the address to the actual loaded location.

## Labels

A label marks the byte address of the next instruction or data item. Two kinds:

- Global: bare identifier followed by `:`. Visible everywhere in the file.
- Local: dot-prefixed identifier followed by `:`. Scoped to the preceding global label. The
  assembler expands `.name` to `global.name` internally, so `.loop` under `multiply:` becomes
  `multiply.loop` and does not conflict with `.loop` under `divide:`.

```
multiply:
    MOV rw0, 0
.loop:
    ADD rw0, rw1
    SUB rw2, 1
    JNZ .loop
    HALT

divide:
.loop:          # divide.loop, no conflict with multiply.loop
    HALT
```

Identifier rules:

```
global:  [a-zA-Z_][a-zA-Z0-9_]* ':'
local:   '.' [a-zA-Z_][a-zA-Z0-9_]* ':'
```

Labels are valid as `imm32` operands anywhere a 32-bit immediate is accepted. `MOV rw0, some_label`
loads the address of `some_label` into `rw0`.

## Preprocessor

The preprocessor runs before the assembler sees anything. It handles file inclusion,
conditional compilation, and text substitution. All preprocessor directives start with `#`
and occupy their own line.

### #include

```
#include "path/to/file.fa"
```

Inserts the contents of the given file at the point of the directive. The path is relative to
the including file. Circular includes are detected and cause an error.

Use include guards to make files safe to include more than once:

```
#ifndef MY_DEFS_FA
#define MY_DEFS_FA

# ... your definitions ...

#endif
```

### #define and #undef

```
#define NAME
#define NAME value
#undef NAME
```

`#define NAME` with no value just marks `NAME` as defined, useful for `#ifdef` guards.
`#define NAME value` replaces every occurrence of `NAME` in subsequent source lines with
`value` before the assembler tokenizes them.

```
#define PAGE_SIZE 4096
#define STACK_TOP 0xFFFFFFFF

MOV rw0, PAGE_SIZE   # assembler sees: MOV rw0, 4096
```

### #ifdef / #ifndef / #else / #endif

```
#ifdef NAME
    # included if NAME is defined
#else
    # included if NAME is not defined
#endif

#ifndef NAME
    # included if NAME is not defined
#endif
```

Conditionals can be nested. Each `#ifdef` or `#ifndef` needs a matching `#endif`.

```
#define DEBUG

#ifdef DEBUG
    OUT 0, rb0      # log register value
#endif
```
