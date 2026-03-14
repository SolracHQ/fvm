# FVM Assembly: Language Reference

Fantasy Assembly (`.fa`) is the assembly language for the FVM. Goal is simplicity: easy to read,
easy to parse, easy to execute. Each opcode is one byte and each register operand is one byte;
wasteful but keeps the toolchain trivial.

---

## Instruction format

```
MNEMONIC
MNEMONIC TARGET
MNEMONIC TARGET, SOURCE
```

Comments start with `#` and continue to the end of the line.

---

## Registers

16 general-purpose registers plus `sp` (stack pointer) and `cr` (context register). Each
general-purpose register has three views that share the same underlying storage:

- `rw0`..`rw15`: full 32-bit register
- `rh0`..`rh15`: low 16 bits of the corresponding `rw` register
- `rb0`..`rb15`: low 8 bits of the corresponding `rh` register

Writing a narrower view never touches bits outside that view. To promote a narrower value to a
wider register use `ZEXT` or `SEXT`.

```
MOV  rw0, 0xDEADBEEF   # rw0 = 0xDEADBEEF
MOV  rh0, 0x1234       # rw0 = 0xDEAD1234, upper 16 untouched
MOV  rb0, 0xFF         # rw0 = 0xDEAD12FF, upper 24 untouched
```

The width of an operation is determined by the register view used, not the mnemonic. Mixing
views of different widths in a single instruction is an assembler error; use `ZEXT` or `SEXT`
to promote first.

`sp` is a separate register, not part of the `rw`/`rh`/`rb` file. It is a normal writable
register: you can read it into a general register, do arithmetic on it, and write it back
directly.

`cr` holds the active address space context identifier. Readable in any mode, writable only
from kernel mode. Writing it in user mode raises interrupt 6.

---

## Immediates

Plain decimal, `0x` hex, `0o` octal: `42`, `0xFF`, `0o17`.

Character literals: `'A'` expands to the ASCII value of the character. Supported escape
sequences: `\n`, `\t`, `\0`, `\\`, `\'`.

Width is determined by the destination register view: 32-bit for `rw`, 16-bit for `rh`, 8-bit
for `rb`. The assembler enforces the range and rejects values that do not fit.

---

## Sections

A source file can contain up to three sections. The default section when no directive is present
is `.code`.

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
next 4 kb boundary after the previous region, following the device table and fault info region
at the start of the address space. The stack is always mapped to `0xFFC00000..0xFFFFFFFF`.
See the VM reference for the full layout.

Labels defined in `.rodata` or `.data` resolve to their loaded address and are usable as
`imm32` operands in `.code`.

---

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

---

## Instruction set

### NOP

Does nothing, advances IP by one byte.

```
NOP
```

---

### HALT

Stops execution.

```
HALT
```

---

### PUSH

Decrements SP then writes the value to the stack.

```
PUSH rw     # SP -= 4, writes 32-bit value
PUSH rh     # SP -= 2, writes 16-bit value
PUSH rb     # SP -= 1, writes byte
```

---

### POP

Reads from the stack then increments SP. Writing a narrower view leaves upper bits of the
register untouched.

```
POP  rw     # reads 32-bit value, SP += 4
POP  rh     # reads 16-bit value into low 16, SP += 2
POP  rb     # reads byte into low 8, SP += 1
```

---

### MOV

Copies a value. Both operands must use the same register view.

```
MOV  rw, imm32
MOV  rw, label      # loads the address of label as imm32
MOV  rw, rw
MOV  rw, sp
MOV  sp, rw
MOV  rh, imm16
MOV  rh, rh
MOV  rb, imm8
MOV  rb, 'A'        # char literal immediate
MOV  rb, rb
```

To move a narrower view into a wider register use `ZEXT` or `SEXT` first.

---

### ZEXT

Zero-extends a narrower view into a wider register. Bits above the source width are set to zero.

```
ZEXT rw, rh     # zero-extend 16-bit into 32-bit
ZEXT rw, rb     # zero-extend 8-bit into 32-bit
ZEXT rh, rb     # zero-extend 8-bit into 16-bit
```

Source and destination can be different registers.

---

### SEXT

Sign-extends a narrower view into a wider register. The high bit of the source is replicated
into all bits above it in the destination.

```
SEXT rw, rh     # sign-extend 16-bit into 32-bit
SEXT rw, rb     # sign-extend 8-bit into 32-bit
SEXT rh, rb     # sign-extend 8-bit into 16-bit
```

---

### ADD

Adds source to destination, result in destination. Source can be a register or immediate.

Flags: C set on unsigned overflow, N set if high bit of result is set.

```
ADD rw, rw
ADD rw, imm32
ADD rh, rh
ADD rh, imm16
ADD rb, rb
ADD rb, imm8
```

---

### SUB

Subtracts source from destination, result in destination. Source can be a register or immediate.

Flags: C set on unsigned underflow (borrow), N set if high bit of result is set.

```
SUB rw, rw
SUB rw, imm32
SUB rh, rh
SUB rh, imm16
SUB rb, rb
SUB rb, imm8
```

---

### AND

Bitwise AND, result in destination. Flags: N.

```
AND rw, rw
AND rw, imm32
AND rh, rh
AND rh, imm16
AND rb, rb
AND rb, imm8
```

---

### OR

Bitwise OR, result in destination. Flags: N.

```
OR  rw, rw
OR  rw, imm32
OR  rh, rh
OR  rh, imm16
OR  rb, rb
OR  rb, imm8
```

---

### XOR

Bitwise XOR, result in destination. Flags: N. `XOR rw, rw` is the canonical way to zero a
register.

```
XOR rw, rw
XOR rw, imm32
XOR rh, rh
XOR rh, imm16
XOR rb, rb
XOR rb, imm8
```

---

### NOT

Bitwise NOT in place. Flags: N.

```
NOT rw
NOT rh
NOT rb
```

---

### SHL

Shifts destination left by `n` bits. Zeros fill from the right. If `n` >= width of the
destination view, the result is zero.

Flags: Z set if result is zero, N set if high bit of result is set, C set to the last bit
shifted out of the top.

```
SHL rw, rb
SHL rw, imm8
SHL rh, rb
SHL rh, imm8
SHL rb, rb
SHL rb, imm8
```

---

### SHR

Shifts destination right by `n` bits logically. Zeros fill from the left. If `n` >= width of
the destination view, the result is zero.

Flags: Z set if result is zero, N set if high bit of result is set, C set to the last bit
shifted out of the bottom.

```
SHR rw, rb
SHR rw, imm8
SHR rh, rb
SHR rh, imm8
SHR rb, rb
SHR rb, imm8
```

---

### SAR

Shifts destination right by `n` bits arithmetically. The sign bit is replicated into vacated
bits from the left. If `n` >= width of the destination view, the result is all sign bits
(0x00 or 0xFF / 0xFFFF / 0xFFFFFFFF).

Flags: Z set if result is zero, N set if high bit of result is set, C set to the last bit
shifted out of the bottom.

```
SAR rw, rb
SAR rw, imm8
SAR rh, rb
SAR rh, imm8
SAR rb, rb
SAR rb, imm8
```

---

### ROL

Rotates destination left by `n` bits. Bits shifted out of the top wrap into the bottom.

Flags: Z set if result is zero, N set if high bit of result is set, C unaffected.

```
ROL rw, rb
ROL rw, imm8
ROL rh, rb
ROL rh, imm8
ROL rb, rb
ROL rb, imm8
```

---

### ROR

Rotates destination right by `n` bits. Bits shifted out of the bottom wrap into the top.

Flags: Z set if result is zero, N set if high bit of result is set, C unaffected.

```
ROR rw, rb
ROR rw, imm8
ROR rh, rb
ROR rh, imm8
ROR rb, rb
ROR rb, imm8
```

---

### CMP

Computes `dst - src` and sets flags, discards the result. Source can be a register or immediate.
Use before conditional jumps.

Flags: Z set if equal, C set if src > dst (unsigned), N set if high bit of result is set.

```
CMP rw, rw
CMP rw, imm32
CMP rh, rh
CMP rh, imm16
CMP rb, rb
CMP rb, imm8
```

---

### IN

Reads from an I/O port into a register. Width is determined by the register view.

```
IN rw, port
IN rh, port
IN rb, port
```

---

### OUT

Writes a register value to an I/O port. Width is determined by the register view.

```
OUT port, rw
OUT port, rh
OUT port, rb
```

---

### LOAD

Loads a value from the address held in `addr` into `dst`. Width is determined by `dst`. `addr`
must be an `rw` register since addresses are 32-bit.

```
LOAD rw, rw     # load 32-bit value at address in src
LOAD rh, rw     # load 16-bit value at address in src into low 16
LOAD rb, rw     # load byte at address in src into low 8
```

Encoding: `op dst_enc addr_enc` (3 bytes).

---

### STORE

Stores a value from `src` to the address held in `addr`. Width is determined by `src`. `addr`
must be an `rw` register since addresses are 32-bit.

```
STORE rw, rw    # store 32-bit value to address in addr
STORE rw, rh    # store low 16 of src to address in addr
STORE rw, rb    # store low 8 of src to address in addr
```

Encoding: `op addr_enc src_enc` (3 bytes).

---

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

---

### JMP

Unconditional jump. Register form enables indirect jumps; must use an `rw` register.

```
JMP label       # opcode + addr32
JMP rw          # opcode + enc  (indirect)
JMP 0x12345678  # opcode + addr32 literal
```

---

### JZ

Jump if zero flag is set (last result was zero / operands were equal).

```
JZ  label
JZ  rw
```

---

### JNZ

Jump if zero flag is clear.

```
JNZ label
JNZ rw
```

---

### JC

Jump if carry flag is set (unsigned overflow or borrow).

```
JC  label
JC  rw
```

---

### JN

Jump if negative flag is set (high bit of last result was 1).

```
JN  label
JN  rw
```

---

### CALL

Pushes the return address onto the stack then jumps. The return address is `IP + 5` for the
immediate form and `IP + 2` for the register form.

```
CALL label      # opcode + addr32
CALL rw         # opcode + enc  (indirect)
```

---

### RET

Pops a 32-bit address off the stack and jumps to it. Paired with `CALL`.

```
RET
```

---

### SIE

Sets one entry in the interrupt vector table. The first operand is an `rb` register holding
the vector index. The second operand is the handler address, either as a label, immediate, or
`rw` register.

```
SIE rb, label
SIE rb, 0x12345678
SIE rb, rw
```

- Privileged. Executing in user mode raises interrupt 6.
- Index must be `0..255`.
- Address `0` clears the handler.

---

### INT

Raises a software interrupt.

```
INT 15          # immediate form, u8 literal
INT rb          # register form
```

The immediate form encodes a `u8` vector index directly. The register form reads an `rb`
register; using a wider view is an invalid opcode. Index 15 is the conventional syscall slot.

---

### IRET

Returns from an interrupt handler. Unwinds one interrupt frame from the kernel stack,
restoring the general-purpose registers, `ip`, `flags`, and the active register file to
whatever they were before the interrupt was raised.

```
IRET
```

---

### DPL

Activates the user register file. Privileged and one-way: the only path back to kernel mode
is through an interrupt. The kernel is responsible for setting up the user file completely
before calling `DPL`.

```
DPL
```

---

### TUR

Transfers a value from a user register into a kernel register without switching mode.
Privileged. Both operands must be `rw`.

```
TUR rw_dst, rw_src   # kern_file[dst] = user_file[src]
```

Reading from an uninitialised user file returns zero.

---

### TKR

Transfers a value from a kernel register into a user register without switching mode.
Privileged. Both operands must be `rw`.

```
TKR rw_dst, rw_src   # user_file[dst] = kern_file[src]
```

---

### MMAP

Maps a range of physical pages into the current address space context. Privileged.

```
MMAP rw, rw, rw     # virt_addr, phys_addr, size (all rw)
MMAP rw, rw, imm32  # size as immediate
```

- `virt_addr` and `phys_addr` must be `rw` registers.
- `size` may be an `rw` register or an `imm32`.
- Size is rounded up to the nearest 4 kb page boundary.
- Each covered page is mapped independently. Overlapping an existing mapping
  replaces it.

---

### MUNMAP

Unmaps a range of pages from the current address space context. Privileged.

```
MUNMAP rw, rw       # virt_addr, size (both rw)
MUNMAP rw, imm32    # size as immediate
```

- `virt_addr` must be an `rw` register.
- `size` may be an `rw` register or an `imm32`.
- Size is rounded up to the nearest 4 kb page boundary.
- Unmapping a page that is not mapped raises interrupt 1.

---

### MPROTECT

Sets the permission bits on a single page. Privileged.

```
MPROTECT rw, rw, rb     # virt_addr in rw, size in rw or imm32, permission bits in rb
MPROTECT rw, imm32, rb  # virt_addr in rw, size in rw or imm32, permission bits in rb
```

Permission bits: bit 0 = Read, bit 1 = Write, bit 2 = Execute.