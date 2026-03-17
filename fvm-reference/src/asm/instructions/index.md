# Instructions

Quick reference. Click any mnemonic for the full description.

## Data movement

| Mnemonic | Operands | Description |
|----------|----------|-------------|
| [MOV](./data-movement.md#mov) | `dst, src` | Copy a value between registers or load an immediate |
| [PUSH](./data-movement.md#push) | `reg` | Decrement SP, write register to stack |
| [POP](./data-movement.md#pop) | `reg` | Read from stack, increment SP |
| [ZEXT](./data-movement.md#zext) | `dst, src` | Zero-extend a narrower view into a wider register |
| [SEXT](./data-movement.md#sext) | `dst, src` | Sign-extend a narrower view into a wider register |

## Arithmetic

| Mnemonic | Operands | Description |
|----------|----------|-------------|
| [ADD](./arithmetic.md#add) | `dst, src` | Add, result in dst. Flags: C, N |
| [SUB](./arithmetic.md#sub) | `dst, src` | Subtract, result in dst. Flags: C, N |
| [CMP](./arithmetic.md#cmp) | `dst, src` | Subtract and set flags, discard result. Flags: Z, C, N |
| [MUL](./arithmetic.md#mul) | `dst, src` | Unsigned multiply, result in dst. Flags: C, O, N |
| [SMUL](./arithmetic.md#smul) | `dst, src` | Signed multiply, result in dst. Flags: C, O, N |
| [DIV](./arithmetic.md#div) | `dst, src` | Unsigned divide, result in dst. Flags: N |
| [SDIV](./arithmetic.md#sdiv) | `dst, src` | Signed divide, result in dst. Flags: N |
| [MOD](./arithmetic.md#mod) | `dst, src` | Unsigned modulo, result in dst. Flags: N |
| [SMOD](./arithmetic.md#smod) | `dst, src` | Signed modulo, result in dst. Flags: N |

## Bitwise

| Mnemonic | Operands | Description |
|----------|----------|-------------|
| [AND](./bitwise.md#and) | `dst, src` | Bitwise AND. Flags: N |
| [OR](./bitwise.md#or) | `dst, src` | Bitwise OR. Flags: N |
| [XOR](./bitwise.md#xor) | `dst, src` | Bitwise XOR. Flags: N |
| [NOT](./bitwise.md#not) | `dst` | Bitwise NOT in place. Flags: N |

## Shifts and rotations

| Mnemonic | Operands | Description |
|----------|----------|-------------|
| [SHL](./shifts.md#shl) | `dst, n` | Shift left logical. Flags: Z, N, C |
| [SHR](./shifts.md#shr) | `dst, n` | Shift right logical. Flags: Z, N, C |
| [SAR](./shifts.md#sar) | `dst, n` | Shift right arithmetic. Flags: Z, N, C |
| [ROL](./shifts.md#rol) | `dst, n` | Rotate left. Flags: Z, N |
| [ROR](./shifts.md#ror) | `dst, n` | Rotate right. Flags: Z, N |

## Control flow

| Mnemonic | Operands | Description |
|----------|----------|-------------|
| [JMP](./control-flow.md#jmp) | `target` | Unconditional jump |
| [JZ](./control-flow.md#jz) | `target` | Jump if zero flag set |
| [JNZ](./control-flow.md#jnz) | `target` | Jump if zero flag clear |
| [JC](./control-flow.md#jc) | `target` | Jump if carry flag set |
| [JN](./control-flow.md#jn) | `target` | Jump if negative flag set |
| [JO](./control-flow.md#jo) | `target` | Jump if overflow flag set |
| [JNO](./control-flow.md#jno) | `target` | Jump if overflow flag clear |
| [CALL](./control-flow.md#call) | `target` | Push return address, jump to target |
| [RET](./control-flow.md#ret) | | Pop return address, jump to it |
| [CALL](./control-flow.md#call) | `target` | Push return address, jump |
| [RET](./control-flow.md#ret) | - | Pop return address, jump |
| [NOP](./control-flow.md#nop) | - | No operation |
| [HALT](./control-flow.md#halt) | - | Stop execution |

## I/O

| Mnemonic | Operands | Description |
|----------|----------|-------------|
| [IN](./io.md#in) | `dst, port` | Read from I/O port into register |
| [OUT](./io.md#out) | `port, src` | Write register to I/O port |

## Memory

| Mnemonic | Operands | Description |
|----------|----------|-------------|
| [LOAD](./memory.md#load) | `dst, addr` | Load from address in register |
| [STORE](./memory.md#store) | `addr, src` | Store to address in register |
| [MMAP](./memory.md#mmap) | `virt, phys, count` | Map physical pages into a context. Privileged |
| [MUNMAP](./memory.md#munmap) | `virt, count` | Unmap pages from a context. Privileged |
| [MPROTECT](./memory.md#mprotect) | `virt, count, perms` | Set page permissions. Privileged |

## System

| Mnemonic | Operands | Description |
|----------|----------|-------------|
| [SIE](./system.md#sie) | `rb, handler` | Set interrupt vector entry. Privileged |
| [INT](./system.md#int) | `index` | Raise software interrupt |
| [IRET](./system.md#iret) | - | Return from interrupt handler. Privileged |
| [DPL](./system.md#dpl) | - | Drop to user mode. Privileged |
| [TUR](./system.md#tur) | `dst, src` | Transfer user register to kernel register. Privileged |
| [TKR](./system.md#tkr) | `dst, src` | Transfer kernel register to user register. Privileged |
