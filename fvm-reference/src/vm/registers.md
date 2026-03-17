# Registers

The VM maintains two complete register files: one for kernel mode and one for
user mode. Only one file is active at a time. All instructions read and write
the active file. The inactive file is preserved untouched until the mode
switches.

Each file contains:

- 16 general-purpose 32-bit registers with three views sharing the same storage:
  - `rw0`..`rw15`: full 32-bit view
  - `rh0`..`rh15`: low 16 bits of the corresponding `rw` register
  - `rb0`..`rb15`: low 8 bits of the corresponding `rh` register
- `sp`: stack pointer, 32-bit, separate from the general-purpose file
- `ip`: instruction pointer, 32-bit, not directly writable by general instructions
- `cr`: address space context identifier, 32-bit
- `flags`: Z, C, N flag bits

`cr` switches with the file on a mode change. When `DPL` activates the user
file, the bus immediately begins translating through the user file's `cr`. This
means the kernel sets the user CR via `TKR cr, rw` before `DPL` without ever
changing its own CR. Writing `cr` directly in user mode raises interrupt 6; the
user file's `cr` is only writable by the kernel via `TKR`.

`mr` (mapping register) is a single privileged 32-bit register not part of
either file. It holds the target context for `MMAP`, `MUNMAP`, and `MPROTECT`.
Readable and writable only from kernel mode. Initialized to 0 on VM start.

Writing a narrower view of a general-purpose register never touches bits outside
that view. Use `ZEXT` or `SEXT` to promote a value into a wider view.

The internal layout:

```rust
struct RegisterFile {
    regs:  [u32; 16],
    ip:    u32,
    sp:    u32,
    cr:    u32,
    flags: Flags,
}

files:             [RegisterFile; 2],  // 0 = kernel, 1 = user
active:            usize,              // index into files
mr:                u32,
```

`files[0]` is the kernel file. `files[1]` is the user file. `active` is the
only thing that changes on a mode switch.

## Register encoding

Each operand is one byte:

```
bits 7:5  view
            001 = rw  (32-bit general purpose)
            010 = rh  (low 16-bit view)
            011 = rb  (low 8-bit view)
            100 = sp  (stack pointer, from active file)
            101 = cr  (context register, write is privileged)
            110 = ip  (instruction pointer, privileged via TKR/TUR only)
            111 = mr  (mapping register, privileged)
bit  4    reserved
bits 3:0  register index 0-15 (ignored for sp, cr, ip, and mr)
```

Examples: `rw5 = 0x25`, `rh5 = 0x45`, `rb5 = 0x65`, `sp = 0x80`, `cr = 0xA0`,
`ip = 0xC0`, `mr = 0xE0`.

## Flags

Three flag bits updated by arithmetic and comparison instructions:

| Flag | Name | Set when |
|------|------|----------|
| Z | zero | result == 0 |
| C | carry | unsigned overflow (ADD) or borrow (SUB/CMP) |
| N | negative | high bit of result is set |
