# System

All instructions on this page are privileged unless noted. Executing them in user mode raises
interrupt 6.

## SIE

Sets one entry in the interrupt vector table. The first operand is an `rb` register holding
the vector index. The second operand is the handler address, either as a label, immediate, or
`rw` register.

```
SIE rb, label
SIE rb, 0x12345678
SIE rb, rw
```

- Index must be `0..255`.
- Address `0` clears the handler.

## INT

Raises a software interrupt. Not privileged; available in user mode.

```
INT 15          # immediate form, u8 literal
INT rb          # register form
```

The immediate form encodes a `u8` vector index directly. The register form reads an `rb`
register; using a wider view is an invalid opcode. Index 15 is the conventional syscall slot.

## IRET

Returns from an interrupt handler. Unwinds one interrupt frame from the kernel stack,
restoring the general-purpose registers, `ip`, `flags`, and the active register file to
whatever they were before the interrupt was raised.

```
IRET
```

## DPL

Activates the user register file. One-way: the only path back to kernel mode is through an
interrupt. The kernel is responsible for setting up the user file completely before calling
`DPL`, including `sp` and `ip` via `TKR`.

```
DPL
```

## TUR

Transfers a value from a user register into a kernel register without switching mode. Both
operands must be `rw`. `ip` and `cr` are valid as sources to read the user instruction pointer
and address space context.

```
TUR rw_dst, rw_src   # kern_file[dst] = user_file[src]
TUR rw_dst, ip       # kern_file[dst] = user_file.ip
TUR rw_dst, cr       # kern_file[dst] = user_file.cr
```

Reading from an uninitialised user file returns zero.

## TKR

Transfers a value from a kernel register into a user register without switching mode. Both
operands must be `rw`. `ip` and `cr` are valid as destinations to set the user entry point and
address space before `DPL`.

```
TKR rw_dst, rw_src   # user_file[dst] = kern_file[src]
TKR ip, rw_src       # user_file.ip   = kern_file[src]
TKR cr, rw_src       # user_file.cr   = kern_file[src]
```
