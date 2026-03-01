# FVM: Virtual Machine Design Notes

Planned changes to the VM. Roughly in implementation order.

---

## Interrupt vector table

Fixed at address `0x0000`, immediately before `.rodata`. Once IVT is implemented, `.rodata` will start at `0x0020` instead of `0x0000` and the assembler will shift section addresses accordingly. 16 entries, 2 bytes each (one u16 address per entry), total 32 bytes. Each entry holds the address of the handler for that interrupt number. The VM jumps to that address when the interrupt fires, after pushing IP onto the stack. The handler returns with RET (same as a subroutine).

Interrupt numbers:

| Index | Name | Fires when |
|-------|------|------------|
| 0 | reset | VM starts or is reset |
| 1 | bus fault | access to unmapped or protected memory |
| 2 | invalid opcode | fetch returns an unrecognized byte |
| 3 | stack overflow | SP would wrap below the stack region |
| 4 | stack underflow | POP on empty stack |
| 5-14 | reserved | |
| 15 | software INT | explicit INT instruction from code |

Hardware interrupts (1-4) fire instead of halting. If the IVT entry for the fault is zero the VM halts anyway, so unhandled faults still stop execution cleanly.

`INT n` is a new instruction that triggers interrupt n directly from code. This is the mechanism for syscalls once privilege levels exist.

The assembler will provide a syntax for populating the IVT, probably something like:

```
.ivt
    [1] fault_handler
    [15] syscall_handler
```

Entries not listed are left as zero (unhandled, VM halts on that interrupt).

---

## Privilege levels

MMAP and anything that modifies region permissions only makes sense if normal programs cannot call it freely, otherwise there is no point having protections at all.

One status bit: current privilege level, 0 = kernel, 1 = user. Privileged instructions trap with a bus fault (or a dedicated privilege fault interrupt) when called from user mode. The interrupt handlers run in kernel mode. Returning from an interrupt with IRET (or just RET, TBD) drops back to whatever mode was active before.

The switch from user to kernel only happens through an interrupt, never by the program escalating itself.

---

## MMAP

Once privilege levels exist, MMAP lets kernel code create, resize, and change the permissions of memory regions at runtime. Useful for programs that need more stack, dynamic allocation, or want to map device memory.

Rough idea for the instruction:

```
MMAP r0, r1, r2   # base in r0, length in r1, permission flags in r2
```

Permission flags in r2 would use the existing `Permission` enum values (`Read`, `Write`, `Execute`). Returns an error code somewhere (flags register or a dedicated result register, TBD).

This is far enough out that the exact encoding is not worth fixing yet.