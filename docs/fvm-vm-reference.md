# FVM: Virtual Machine Reference

What the machine actually is right now: registers, memory model, the bus, and the execution loop. For planned extensions see `fvm-vm-design.md`.

---

## Registers

16 general-purpose 16-bit registers, `r0` through `r15`, plus `sp` (stack pointer), also 16-bit. All 17 are first-class operands in MOV, ADD, SUB, and the other ALU instructions.

### Register encoding

Each operand is one byte:

```
bits 7:6  lane field
            00 = full 16-bit register
            01 = sp  (0x40; the otherwise-invalid state)
            10 = low byte lane
            11 = high byte lane
bits 5:4  reserved
bits 3:0  register index 0-15 (ignored for sp)
```

Examples: `r5 = 0x05`, `r5l = 0x85`, `r5h = 0xC5`, `sp = 0x40`.

Three flag bits updated by arithmetic and comparison instructions:

| Flag | Name | Set when |
|------|------|----------|
| Z | zero | result == 0 |
| C | carry | unsigned overflow (ADD) or borrow (SUB/CMP) |
| N | negative | high bit of result is set |

---

## Memory and the bus

Flat 64 KB address space. All reads and writes go through the bus, which owns the backing byte array and a list of mapped regions. Each region has a base address, a size, a label, and a permissions set.

Permissions:

| Constant | Set | Used for |
|----------|-----|----------|
| `PermRom` | `{Read}` | `.rodata` |
| `PermCode` | `{Read, Execute}` | `.code` |
| `PermRam` | `{Read, Write}` | `.data`, stack |

The bus checks every access against the region list. Writing to a region without `Write` permission returns an error. Fetching an opcode from a region without `Execute` permission returns an error. Accessing an unmapped address returns an error. The VM treats any bus error as a fault and halts.

Opcode fetches use `bus.fetch8` which enforces `Execute` permission. Data reads and writes use `bus.read8`/`bus.read16`/`bus.write8`/`bus.write16` which enforce `Read`/`Write`.

Device regions also carry read/write callbacks. When a device region is hit the callbacks fire instead of touching the backing array. This is how peripherals (UART, screen, timers) will be wired in.

### Memory layout

```
0x0000 .. rodataEnd-1       ROM  {Read}            .rodata
rodataEnd .. codeEnd-1      ROM  {Read, Execute}   .code
codeEnd .. dataEnd-1        RAM  {Read, Write}     .data
dataEnd .. 0xEFFF           unmapped
0xF000 .. 0xFFFF            RAM  {Read, Write}     stack
```

Stack starts at `0xFFFF` and grows downward. SP is initialized to `0xFFFF` on VM creation. If `dataEnd > 0xF000` the loader returns an error.

---

## Binary format (v2)

What the assembler produces. Big-endian throughout. Header is exactly 13 bytes.

```
offset  size  description
------  ----  -----------
0       4     magic: 0x46 0x56 0x4D 0x21  ("FVM!")
4       1     version: 2
5       2     entry point address
7       2     .rodata byte count
9       2     .code byte count
11      2     .data byte count
13      N     .rodata bytes
13+N    M     .code bytes
13+N+M  P     .data bytes
```

`initRom` deserializes the object, maps the three sections as the correct region types using `writeRangeDirect` (bypasses permission checks during initialization), and sets IP to the entry point.

---

## Execution loop

`step` calls `bus.fetch8(ip)` to read one opcode byte, decodes it, looks up the handler in the dispatch table, and calls it. The handler is responsible for advancing IP. `run` calls `step` in a loop until `vm.halted` is true or a step returns an error.

`fetch8` enforces `Execute` permission, so fetching from `.rodata` or `.data` produces a bus error and halts the VM.

---

## Jump opcodes

All five conditional variants come in two encodings. The assembler selects based on the operand type; the VM does not auto-detect.

| Opcode | Bytes | Condition | Behavior when false |
|--------|-------|-----------|---------------------|
| `Jmp` | `op hi lo` | always | - |
| `JmpReg` | `op enc` | always | - |
| `Jz` | `op hi lo` | Z set | `ip += 3` |
| `JzReg` | `op enc` | Z set | `ip += 2` |
| `Jnz` | `op hi lo` | Z clear | `ip += 3` |
| `JnzReg` | `op enc` | Z clear | `ip += 2` |
| `Jc` | `op hi lo` | C set | `ip += 3` |
| `JcReg` | `op enc` | C set | `ip += 2` |
| `Jn` | `op hi lo` | N set | `ip += 3` |
| `JnReg` | `op enc` | N set | `ip += 2` |

The immediate form reads `hi` and `lo` as a big-endian `u16` and overwrites `ip`. The register form reads the full 16-bit value of the register named by `enc`; `sp` (`enc = 0x40`) is a valid operand.

---

## Memory opcodes

| Opcode | Bytes | Effect |
|--------|-------|--------|
| `Load` | `op dst_enc addr_enc` | load word or byte from address in `addr` into `dst`; width from `dst` encoding |
| `Store` | `op addr_enc src_enc` | store word or byte from `src` to address in `addr`; width from `src` encoding |

`addr` must always be a full 16-bit register. `dst`/`src` can be full or byte-lane; the lane encoding determines whether one or two bytes are transferred.

---

## Subroutine opcodes

| Opcode | Bytes | Effect |
|--------|-------|--------|
| `Call` | `op hi lo` | push `ip + 3` as `u16`, set `ip = (hi << 8) | lo` |
| `CallReg` | `op enc` | push `ip + 2` as `u16`, set `ip = regs[enc]` |
| `Ret` | `op` | pop `u16` from stack, set `ip` to it |

The push uses the same decrement-then-write mechanism as `PUSH`: `sp -= 2`, then write 16-bit big-endian. The pop mirrors `POP`: read 16-bit big-endian, then `sp += 2`. The return address is the byte immediately after the `Call` instruction, so execution resumes correctly after `Ret`.