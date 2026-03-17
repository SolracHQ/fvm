# Overview

The FVM is a simple 32-bit virtual machine. One opcode is one byte, one
register operand is one byte; wasteful but keeps everything easy to reason
about.

The machine has two privilege levels: kernel and user. Only one is active at a
time. The only way into the kernel is through an interrupt. The only way out is
`DPL`.

## Execution loop

`step` performs fetch, decode, and execute for one instruction using the active
register file.

IP advancement: control-flow instructions update `ip` themselves. All other
instructions have `ip` incremented by `instruction.size` after execute returns.

Faults raised during `step`:

| Cause | Interrupt |
|-------|-----------|
| bus fault on fetch, read, or write | 1 |
| unrecognised opcode byte | 2 |
| privileged instruction in user mode | 6 |

If the IVT entry for the raised interrupt is `0` the VM halts immediately.
Otherwise the handler runs in the kernel file and returns via `IRET`.
