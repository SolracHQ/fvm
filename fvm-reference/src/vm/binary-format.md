# Binary format

Version 3. Big-endian throughout. All length and count fields are 4 bytes.

```
offset      size  description
----------  ----  -----------
0           4     magic: 0x46 0x56 0x4D 0x21  ("FVM!")
4           1     version: 3
5           4     entry point address
9           4     .rodata byte count  (N)
13          4     .code byte count    (M)
17          4     .data byte count    (P)
21          4     relocation count    (K)
25          N     .rodata bytes
25+N        M     .code bytes
25+N+M      P     .data bytes
25+N+M+P    5*K   relocations: (section: u8, offset: u32) each, big-endian
```

Header before section data is 25 bytes.

Relocations identify 32-bit address slots that must be patched by the loader
when mapping sections to their actual virtual base addresses. Each relocation
names the section the slot belongs to and its byte offset within that section.

## Privileged instruction encodings

For reference when writing or verifying kernel code.

| Mnemonic | Encoding |
|----------|----------|
| `SIE rb, label` | `op rb_enc imm32[4]` |
| `SIE rb, rw` | `op rb_enc rw_enc` |
| `INT imm8` | `op imm8` |
| `INT rb` | `op rb_enc` |
| `IRET` | `op` |
| `DPL` | `op` |
| `TUR rw, rw\|ip\|cr` | `op rw_enc rw_enc` |
| `TKR rw\|ip\|cr, rw` | `op rw_enc rw_enc` |
| `MMAP rw, rw, rw\|imm32` | `op rw_enc rw_enc rw_enc\|imm32` |
| `MUNMAP rw, rw\|imm32` | `op rw_enc rw_enc\|imm32` |
| `MPROTECT rw, rw\|imm32, rb` | `op rw_enc rw_enc\|imm32 rb_enc` |

Rules that are not obvious from the encoding:

- `SIE`, `DPL`, `TUR`, `TKR`, `MMAP`, `MUNMAP`, `MPROTECT`, writes to `cr`,
  and writes to `mr` are privileged. Executing any of them in user mode raises
  interrupt 6.
- `IRET` with nothing on the kernel stack raises interrupt 6.
- `INT` immediate form accepts a `u8` literal. Register form requires `rb`.
  Using a wider register view with `INT` is an invalid opcode.
- `TUR` and `TKR` general-purpose operands must be `rw`. `ip` and `cr` are
  additionally valid as the source of `TUR` and the destination of `TKR`.
- `MMAP`, `MUNMAP`, and `MPROTECT` address operands are page numbers, not byte
  addresses. The size operand is a page count and also accepts `imm32`.
- `MPROTECT` permission operand must be `rb`.
- `MMAP`, `MUNMAP`, and `MPROTECT` target the context in `mr`, not `cr`.
