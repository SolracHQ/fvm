# Privilege and interrupts

The interrupt vector table is stored inside `Vm`, not in bus memory. `LOAD` and
`STORE` cannot reach it.

```rust
ivt:               [Address; 256],
pending_interrupt: Option<u8>,
```

`SIE`, `DPL`, `TUR`, `TKR`, `MMAP`, `MUNMAP`, `MPROTECT`, direct writes to
`cr`, and writes to `mr` are privileged. Executing any of them when
`active == 1` raises interrupt 6.

## Process launch sequence

The canonical sequence to launch a user process from the kernel:

```
MOV  mr, rw_pid        # target the new context
MMAP ...               # map sections and stack for the new context
MOV  mr, 0             # restore mapping target to kernel context
TKR  sp, rw_stack_top  # set user sp
TKR  ip, rw_entry      # set user ip
TKR  cr, rw_pid        # set user cr; kernel cr unchanged
DPL                    # activates user file, user cr takes effect immediately
```

After `DPL` the kernel is no longer running. The next path back is through an
interrupt.

## Interrupt vectors

The IVT has 256 entries indexed by a `u8`.

| Index | Name | Raised by |
|-------|------|-----------|
| 0 | reserved | reserved for future reset or shutdown flow |
| 1 | bus fault | unmapped address, permission violation, or device fault |
| 2 | invalid opcode | opcode byte outside the defined set |
| 3..5 | reserved | - |
| 6 | privilege fault | privileged operation attempted in user mode |
| 7..14 | reserved | - |
| 15 | software interrupt | conventional syscall slot |
| 16..255 | unreserved | available for hardware device interrupts |

Reservation ranges within `16..255` for specific hardware device classes will be
defined when device support is implemented.

## Interrupt dispatch

`raise_interrupt(vm, index)` is the only entry point. It always switches to and
runs in the kernel file.

Dispatch sequence:

1. If `pending_interrupt` is `Some(1)` and the incoming index is also `1`, this
   is a double fault: halt the VM immediately.
2. Set `pending_interrupt = Some(index)`.
3. Record whether the interrupted file was user (`came_from_user = active == 1`).
4. Switch to the kernel file (`active = 0`).
5. Push onto the kernel stack in this order:
   - the 16 general-purpose registers from the interrupted file, each as u32 (64 bytes)
   - the interrupted `ip` as u32 (4 bytes)
   - the interrupted `cr` as u32 (4 bytes)
   - the interrupted `flags` as u8 (1 byte)
   - `came_from_user` as u8 (1 byte)
6. Look up `ivt[index]`. If the address is `0`, halt the VM.
7. Set kernel `ip` to the handler address and clear `pending_interrupt`.

The interrupted `ip` pushed in step 5 is chosen by the caller before
`raise_interrupt` runs:

- fetch-time faults push `ip + 1`
- execute-time faults push `ip + instruction.size`
- `INT` pushes the address of the instruction following `INT`

Total frame size pushed onto the kernel stack: 74 bytes.

## IRET

`IRET` unwinds exactly one interrupt frame from the kernel stack:

1. Pop `came_from_user` (1 byte), `flags` (1 byte), `cr` (4 bytes), `ip`
   (4 bytes), and the 16 general-purpose registers (64 bytes) from kernel `sp`
   in reverse push order.
2. If `came_from_user == 1`, restore the popped state into the user file and set
   `active = 1`. Otherwise restore into the kernel file and stay in kernel mode.

`IRET` with nothing on the kernel stack raises interrupt 6.

## Double fault

If a bus fault (interrupt 1) arrives while `pending_interrupt` is already
`Some(1)` the VM halts immediately with no further interrupt delivery. This
condition indicates the kernel stack has overflowed or the interrupt handler
itself is faulting.
