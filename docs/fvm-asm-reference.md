# FVM Assembly: Language Reference

Fantasy Assembly (`.fa`) is the assembly language for the FVM. Goal is simplicity: easy to read, easy to parse, easy to execute. Each opcode is one byte and each register operand is one byte; wasteful but keeps the toolchain trivial.

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

16 general-purpose 16-bit registers, `r0` through `r15`, plus `sp` (stack pointer).

`sp` is a normal writable register. You can read it into a general register, do arithmetic on it, write it back directly. No restrictions.

Individual byte lanes are accessed with a suffix:

- `r0l`: low byte of r0 (bits 7:0)
- `r0h`: high byte of r0 (bits 15:8)

When a byte-lane operand is used the operation is 8-bit and the other byte of the register is unaffected. The width is determined by the operand, not the mnemonic. Mixing full-width and byte-lane operands in a single instruction is an assembler error; use `ZEXT` or `SEXT` to promote a byte lane first.

---

## Immediates

Plain decimal, `0x` hex, `0o` octal: `42`, `0xFF`, `0o17`.

Character literals: `'A'` expands to the ASCII value of the character. Supported escape sequences: `\n`, `\t`, `\0`, `\\`, `\'`.

Width is determined by the destination operand: 16-bit for full registers, 8-bit for byte lanes. The assembler enforces the range and rejects values that do not fit.

---

## Sections

A source file can contain up to three sections. The default section when no directive is present is `.code`.

```
.rodata
    greeting: db "Hello", 0
    table:    dw 0x1234, 0x5678

.code
main:
    MOV r0, greeting
    HALT

.data
    counter: dw 0
```

Section layout at runtime:

```
0x0000 .. rodataEnd-1       ROM  (Read)            .rodata
rodataEnd .. codeEnd-1      ROM  (Read, Execute)   .code
codeEnd .. dataEnd-1        RAM  (Read, Write)     .data
dataEnd .. 0xEFFF           unmapped
0xF000 .. 0xFFFF            RAM  (Read, Write)     stack
```

Labels defined in `.rodata` or `.data` resolve to their loaded address and are usable as `imm16` operands in `.code`.

---

## Data directives

Used inside `.rodata` or `.data` sections to emit raw bytes.

```
db 0x41, 0x42, 0x43     # emit bytes
db "Hello", 0           # string shorthand, null terminated
dw 0x1234, 0x5678       # emit 16-bit big-endian words
```

`db` accepts any mix of integer literals and quoted strings in a single directive. A string is expanded to its ASCII bytes, no null terminator is added automatically so add a trailing `, 0` if you need one.

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
PUSH r      # SP -= 2, writes 16-bit value
PUSH rl     # SP -= 1, writes low byte
PUSH rh     # SP -= 1, writes high byte
```

---

### POP

Reads from the stack then increments SP. When popping into a byte lane the other byte is unaffected.

```
POP  r      # reads 16-bit value, SP += 2
POP  rl     # reads into low byte, SP += 1
POP  rh     # reads into high byte, SP += 1
```

---

### MOV

Copies a value. Both operands must be the same width.

```
MOV  r,  imm16
MOV  r,  label      # loads the address of label as a 16-bit immediate
MOV  r,  r
MOV  r,  sp
MOV  sp, r
MOV  rl, imm8
MOV  rl, 'A'        # char literal immediate
MOV  rl, rl
MOV  rh, imm8
MOV  rh, rh
```

To move from a byte lane into a full register, use `ZEXT` or `SEXT` first.

---

### ZEXT

Zero-extends a byte lane into a full register. High byte of destination is set to zero.

```
ZEXT r, rl
ZEXT r, rh
```

Source and destination can be different registers.

---

### SEXT

Sign-extends a byte lane into a full register. Bit 7 of the source byte is replicated into the entire high byte of the destination.

```
SEXT r, rl
SEXT r, rh
```

---

### ADD

Adds source to destination, result in destination. Source can be a register or immediate.

Flags: C set on unsigned overflow, N set if high bit of result is set.

```
ADD r,  r
ADD rl, rl
ADD rh, rh
ADD r,  imm16
ADD rl, imm8
ADD rh, imm8
```

---

### SUB

Subtracts source from destination, result in destination. Source can be a register or immediate.

Flags: C set on unsigned underflow (borrow), N set if high bit of result is set.

```
SUB r,  r
SUB rl, rl
SUB rh, rh
SUB r,  imm16
SUB rl, imm8
SUB rh, imm8
```

---

### AND

Bitwise AND, result in destination. Flags: N.

```
AND  r,  r
AND  rl, rl
AND  rh, rh
```

---

### OR

Bitwise OR, result in destination. Flags: N.

```
OR   r,  r
OR   rl, rl
OR   rh, rh
```

---

### XOR

Bitwise XOR, result in destination. Flags: N. `XOR r, r` is the canonical way to zero a register.

```
XOR  r,  r
XOR  rl, rl
XOR  rh, rh
```

---

### NOT

Bitwise NOT in place. Flags: N.

```
NOT  r
NOT  rl
NOT  rh
```

---

### CMP

Computes `dst - src` and sets flags, discards the result. Source can be a register or immediate. Use before conditional jumps.

Flags: Z set if equal, C set if src > dst (unsigned), N set if high bit of result is set.

```
CMP r,  r
CMP rl, rl
CMP rh, rh
CMP r,  imm16
CMP rl, imm8
CMP rh, imm8
```

---

### IN

Reads from an I/O port. Full register reads two bytes big-endian, byte lane reads one byte.

```
IN r,  port
IN rl, port
IN rh, port
```

---

### OUT

Writes to an I/O port. Full register writes two bytes big-endian, byte lane writes one byte.

```
OUT port, r
OUT port, rl
OUT port, rh
```

---

### LOAD

Loads a value from the address held in `addr` into `dst`. Width is determined by `dst`: a full register reads a 16-bit big-endian word, a byte lane reads one byte. `addr` must be a full register.

```
LOAD r,  r      # load 16-bit word at address in src into dst
LOAD rl, r      # load byte at address in src into low lane of dst
LOAD rh, r      # load byte at address in src into high lane of dst
```

Encoding: `op dst_enc addr_enc` (3 bytes).

---

### STORE

Stores a value from `src` to the address held in `addr`. Width is determined by `src`: a full register writes a 16-bit big-endian word, a byte lane writes one byte. `addr` must be a full register.

```
STORE r,  r     # store 16-bit word from src to address in addr
STORE r,  rl    # store low byte of src to address in addr
STORE r,  rh    # store high byte of src to address in addr
```

Encoding: `op addr_enc src_enc` (3 bytes).

---

### Labels

A label marks the byte address of the next instruction. Two kinds:

- Global: bare identifier followed by `:`. Visible everywhere in the file.
- Local: dot-prefixed identifier followed by `:`. Scoped to the preceding global label. The assembler expands `.name` to `global.name` internally, so `.loop` under `multiply:` becomes `multiply.loop` and does not conflict with `.loop` under `divide:`.

```
multiply:
    MOV r0, 0
.loop:
    ADD r0, r1
    SUB r2, 1
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

Labels are valid as `imm16` operands anywhere a 16-bit immediate is accepted. `MOV r0, some_label` loads the address of `some_label` into `r0`.

---

### JMP

Unconditional jump. Register form enables indirect jumps.

```
JMP label       # opcode + addr16
JMP r           # opcode + enc  (indirect)
JMP 0x1234      # opcode + addr16 literal
```

---

### JZ

Jump if zero flag is set (last result was zero / operands were equal).

```
JZ  label
JZ  r
```

---

### JNZ

Jump if zero flag is clear.

```
JNZ label
JNZ r
```

---

### JC

Jump if carry flag is set (unsigned overflow or borrow).

```
JC  label
JC  r
```

---

### JN

Jump if negative flag is set (high bit of last result was 1).

```
JN  label
JN  r
```

---

### CALL

Pushes the return address onto the stack then jumps. The return address is `IP + 3` for the immediate form and `IP + 2` for the register form.

```
CALL label      # opcode + addr16
CALL r          # opcode + enc  (indirect)
```

---

### RET

Pops a 16-bit address off the stack and jumps to it. Paired with `CALL`.

```
RET
```