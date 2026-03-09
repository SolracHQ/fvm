# FVM: Virtual Machine Reference

What the machine actually is right now: registers, memory model, privilege,
interrupt delivery, and the execution loop.

---

## Registers

16 general-purpose 16-bit registers, `r0` through `r15`, plus `sp` (stack
pointer), also 16-bit. All 17 are first-class operands in MOV, ADD, SUB, and
the other ALU instructions.

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

Three flag bits are updated by arithmetic and comparison instructions:

| Flag | Name | Set when |
|------|------|----------|
| Z | zero | result == 0 |
| C | carry | unsigned overflow (ADD) or borrow (SUB/CMP) |
| N | negative | high bit of result is set |

---

## Memory and the bus

Flat 64 KB address space. All reads and writes go through the bus, which owns
the backing byte array and a list of mapped regions. Each region has a base
address, a size, a label, and a permissions set.

Permissions:

| Constant | Set | Used for |
|----------|-----|----------|
| `PermRom` | `{Read}` | `.rodata` |
| `PermCode` | `{Read, Execute}` | `.code` |
| `PermRam` | `{Read, Write}` | `.data`, stack |

The bus checks every access against the region list. Writing to a region
without `Write` permission returns an error. Fetching an opcode from a region
without `Execute` permission returns an error. Accessing an unmapped address
returns an error.

Opcode fetches use `bus.fetch8` which enforces `Execute` permission. Data reads
and writes use `bus.read8`/`bus.read16`/`bus.write8`/`bus.write16` which enforce
`Read`/`Write`.

### Memory layout

```
0x0000 .. rodataEnd-1       ROM  {Read}            .rodata
rodataEnd .. codeEnd-1      ROM  {Read, Execute}   .code
codeEnd .. dataEnd-1        RAM  {Read, Write}     .data
dataEnd .. 0xEFFF           unmapped
0xF000 .. 0xFFFF            RAM  {Read, Write}     stack
```

Stack starts at `0xFFFF` and grows downward. SP is initialized to `0xFFFF` on
VM creation. If `dataEnd > 0xF000` the loader returns an error.

---

## Privilege and interrupt state

The interrupt vector table is stored inside `Vm`, not in bus memory. Regular
`LOAD` and `STORE` cannot reach it.

`Vm` carries these interrupt-related fields:

```nim
ivt:         array[IvtEntryCount, Address]
ictx:        InterruptContext
inInterrupt: bool
privileged:  bool
```

`InterruptContext` stores a complete snapshot of the interrupted machine state:

```nim
InterruptContext = object
  regs:       array[GeneralRegisterCount, Word]
  ip:         Address
  sp:         Address
  flags:      Flags
  privileged: bool
```

`inInterrupt` prevents nesting. If an interrupt fires while a handler is
already active, the VM drops the new interrupt and continues from the next
instruction instead of overwriting `ictx`.

`privileged = true` means kernel mode. `privileged = false` means user mode.
`SIE` and `DPL` are privileged instructions. Executing them in user mode raises
interrupt 6.

### Interrupt numbers

`IvtEntryCount` is 16.

| Index | Name | Current use |
|-------|------|-------------|
| 0 | reserved | reserved for future reset/shutdown flow |
| 1 | bus fault | unmapped or permission-denied memory access |
| 2 | invalid opcode | opcode byte outside the defined enum |
| 3 | stack overflow | push/call would move SP below valid stack space |
| 4 | stack underflow | pop/ret would read above stack ceiling |
| 5 | reserved | unused |
| 6 | privilege fault | privileged instruction executed in user mode |
| 7-14 | reserved | unused |
| 15 | software interrupt | conventional syscall/soft interrupt slot |

`INT n` currently accepts any vector index `0..15`, but index 15 is the
conventional software-interrupt slot.

### Interrupt dispatch

`raiseInterrupt(vm, index)` is the only entry point used by the execution loop
and instruction handlers.

Dispatch sequence:

1. If `inInterrupt` is true, drop the interrupt and return.
2. Copy `regs`, `ip`, `sp`, `flags`, and `privileged` into `ictx`.
3. Set `inInterrupt = true` and `privileged = true`.
4. Look up `ivt[index]`. If the address is 0, halt the VM.
5. Set `vm.ip` to the handler address and continue execution.

Resume IP is chosen by the caller before `raiseInterrupt()` runs:

- fetch-time faults save `ip + 1`
- execute-time faults save `ip + insn.size`
- `INT` saves the address after the `INT` instruction

`IRET` restores `regs`, `ip`, `sp`, `flags`, and `privileged`, then clears
`inInterrupt`.

---

## Binary format (v2)

Big-endian throughout. Header is exactly 15 bytes.

```
offset  size  description
------  ----  -----------
0       4     magic: 0x46 0x56 0x4D 0x21  ("FVM!")
4       1     version: 2
5       2     entry point address
7       2     .rodata byte count
9       2     .code byte count
11      2     .data byte count
13      2     relocation count
15      N     .rodata bytes
15+N    M     .code bytes
15+N+M  P     .data bytes
...     2*K   relocation offsets into .code
```

Relocations identify 16-bit addresses inside `.code` that must be adjusted by
the loader's section-base shift. With the IVT now outside memory, the current
loader shift is zero, but the relocation table remains part of the format.

`initRom` deserializes the object, maps the three sections with the correct
permissions using `writeRangeDirect`, and sets IP to the entry point.

---

## Execution loop

`step` performs fetch, decode, and execute. The handler is responsible for
advancing IP only when it changes control flow; otherwise `execute` increments
by the decoded instruction size.

Fault handling in `step` is interrupt-aware:

- bus fetch/read/write faults raise interrupt 1
- invalid opcode bytes raise interrupt 2
- stack overflow and underflow raise interrupts 3 and 4
- privileged-instruction misuse raises interrupt 6

If the corresponding IVT entry is zero, the VM halts. Otherwise the handler
runs in privileged mode and returns with `IRET`.

---

## Interrupt and privilege opcodes

| Opcode | Bytes | Effect |
|--------|-------|--------|
| `SIE` | `op idx_enc hi lo` | set `ivt[idx] = imm16` |
| `SIE` | `op idx_enc addr_enc` | set `ivt[idx] = regs[addr]` |
| `INT` | `op imm8` | raise interrupt `imm8` |
| `INT` | `op enc` | raise interrupt `regs[enc]` |
| `IRET` | `op` | restore `ictx` and leave interrupt mode |
| `DPL` | `op` | drop from privileged to user mode |

Rules:

- `SIE` and `DPL` are privileged.
- `IRET` outside an active handler raises interrupt 6.
- `SIE` requires a full-width index register. Index must be `0..15`.
- `INT` accepts any installed vector index `0..15`.

---

## Jump, memory, and subroutine opcodes

The previously documented jump, memory, and subroutine instructions are
unchanged except for absolute addresses no longer being shifted by a memory-
mapped IVT region.
