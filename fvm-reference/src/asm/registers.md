# Registers

16 general-purpose registers plus `sp` (stack pointer), `cr` (context register), `ip`
(instruction pointer), and `mr` (mapping register). Each general-purpose register has three
views that share the same underlying storage:

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

## sp

Separate register, not part of the `rw`/`rh`/`rb` file. Normal writable register: you can
read it into a general register, do arithmetic on it, and write it back directly.

## cr

Holds the active address space context identifier used for virtual address translation. Part
of each register file and switches with it on a mode change. Readable in any mode, but writing
it directly in user mode raises interrupt 6. The kernel sets the user CR before `DPL` via
`TKR cr, rw`.

## ip

Holds the current instruction pointer. Readable in any mode via `TUR`, writable only from
kernel mode via `TKR`. Not directly writable by general instructions. The kernel uses
`TKR ip, rw` to set the user entry point before `DPL`.

## mr

Holds the target context for memory mapping operations. `MMAP`, `MUNMAP`, and `MPROTECT` all
consult `mr` instead of `cr` when selecting which context to modify. Readable and writable
only from kernel mode. Initialized to 0 on VM start. Write it with `MOV mr, rw` before
issuing mapping instructions for a new context, then restore it to 0 when done.

```
MOV  mr, rw0    # target mappings at context in rw0
MMAP ...        # modifies context mr, not cr
MOV  mr, 0      # restore
```
