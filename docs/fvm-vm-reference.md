# FVM: Virtual Machine Reference

What the machine actually is: registers, memory model, privilege, interrupt
delivery, and the execution loop.

---

## Registers

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

`cr` switches with the file on a mode change. When `DPL` activates the user file, the bus
immediately begins translating through the user file's `cr`. This means the kernel sets the
user CR via `TKR cr, rw` before `DPL` without ever changing its own CR. Writing `cr` directly
in user mode raises interrupt 6; the user file's `cr` is only writable by the kernel via `TKR`.

`mr` (mapping register) is a single privileged 32-bit register not part of either file. It
holds the target context for `MMAP`, `MUNMAP`, and `MPROTECT`. Readable and writable only from
kernel mode. Initialized to 0 on VM start.

Writing a narrower view of a general-purpose register never touches bits outside that view. Use
`ZEXT` or `SEXT` to promote a value into a wider view.

### Register encoding

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

### Flags

Three flag bits updated by arithmetic and comparison instructions:

| Flag | Name | Set when |
|------|------|----------|
| Z | zero | result == 0 |
| C | carry | unsigned overflow (ADD) or borrow (SUB/CMP) |
| N | negative | high bit of result is set |

---

## Memory and the bus

32-bit address space. All memory access goes through the bus. There is no flat backing array;
the bus owns a list of physical regions each backed by either a RAM device or a memory-mapped
device. Unmapped virtual addresses always fault.

### Physical layout

At VM startup the host assembles a contiguous physical address space from the enabled devices.
RAM is placed first at physical address `0x00000000`, then each additional device is placed
immediately after the previous one. The result is a linear physical layout fixed at startup and
never modified at runtime.

RAM size is configured by the user at startup. It is not required to be 4 GB or any fixed size.

The first bytes of RAM hold a discovery table so the kernel can learn what devices are present
and where. The table is written by the VM at startup before the kernel runs.

Discovery table layout (big-endian):

```
offset  size  field
0       4     number of entries
per entry (24 bytes each):
  0     4     physical base address
  4     4     size in bytes
  8     8     device id (8-byte ASCII, unused bytes zero-padded)
  16    1     default permissions (RWX bits, see permissions table)
  17    7     reserved
```

### Virtual address translation

The bus maintains a page table per context in host Rust memory, not accessible to guest code.
Each entry covers one 4 kb page and records the backing device, the offset within that device,
and the permissions for that mapping.

On every access the bus computes `page = virt_addr >> 12`, looks up the entry for
`(current_cr, page)`, checks permissions, then dispatches to the device at
`device_offset = entry.device_page_base + (virt_addr & 0xFFF)`.

`MMAP`, `MUNMAP`, and `MPROTECT` consult `mr` instead of `cr` when selecting the target
context. All other memory accesses use `cr`.

The address and count operands of `MMAP`, `MUNMAP`, and `MPROTECT` are page numbers and page
counts, not byte addresses. The bus multiplies them by 4096 internally. A page number of 1
refers to the page starting at byte address 0x1000.

### Permissions

| Bit | Name | Effect |
|-----|------|--------|
| 0 | Read | allows data reads |
| 1 | Write | allows data writes |
| 2 | Execute | allows instruction fetch |

Common combinations:

| Name | Bits | Used for |
|------|------|----------|
| ROM | `001` | `.rodata`, device table |
| Code | `101` | `.code` |
| RAM | `011` | `.data`, stack |

Violating any permission raises interrupt 1.

### Default virtual layout in context 0

The loader builds this layout at startup. All addresses are determined at load time; nothing is hardcoded. Each region starts at the next 4 kb boundary after the previous region. Sections with size 0 are skipped entirely.

```
page 0        device table         ROM   ceil((4 + entries*24) / 4096) pages
next page     fault info region    ROM   reserved, layout pending (see below)
next page     .rodata              ROM   ceil(rodata_size / 4096) pages
next page     .code                Code  ceil(code_size / 4096) pages
next page     .data                RAM   ceil(data_size / 4096) pages
...           unmapped
0xFFC00000 .. 0xFFFFFFFF           RAM   stack, mapped to last 4 MB of RAM
```

The stack region is always mapped regardless of RAM size. It maps the last 4 MB of physical RAM
to the fixed virtual range `0xFFC00000..0xFFFFFFFF`. `sp` in the kernel file is initialised to
`0xFFFFFFFF` on VM creation.

If the end of `.data` would overlap `0xFFC00000` the loader returns an error.

### Fault info region

A reserved page in context 0 following the device table, accessible only from kernel mode. The VM writes fault information here on every interrupt entry before jumping to the handler.

The exact byte layout is not yet finalised. Fields planned:

- fault kind discriminant
- which register file was active at the time of the fault
- CR value from the interrupted file
- faulting virtual address if applicable
- resolved physical address if applicable
- device id if the fault originated in a device
- instruction pointer at the time of the fault

Finalising this layout will increment the binary format version.

### Bus device trait

All backing storage including RAM implements `MemoryMappedDevice`:

```rust
trait MemoryMappedDevice {
    fn id(&self) -> [u8; 8];
    fn size(&self) -> u32;

    fn read_byte(&self, offset: u32) -> VmResult<u8>;
    fn read_half(&self, offset: u32) -> VmResult<u16>;
    fn read_word(&self, offset: u32) -> VmResult<u32>;

    fn write_byte(&self, offset: u32, value: u8) -> VmResult<()>;
    fn write_half(&self, offset: u32, value: u16) -> VmResult<()>;
    fn write_word(&self, offset: u32, value: u32) -> VmResult<()>;
}
```

Multi-byte reads and writes are atomic at the device level. Devices with wide registers
override the relevant methods. RAM provides default implementations that compose bytes in
big-endian order.

`VmResult` may carry an `Interrupt` variant. A device raising an interrupt returns
`Err(VmError::Interrupt(n))`. The bus propagates this to the execution loop which delivers it
through the normal interrupt dispatch path.

---

## Privilege and interrupt state

The interrupt vector table is stored inside `Vm`, not in bus memory. `LOAD` and `STORE` cannot
reach it.

`Vm` carries these fields:

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
ivt:               [Address; 256],
pending_interrupt: Option<u8>,
```

`files[0]` is the kernel file. `files[1]` is the user file. `active` is the only thing that
changes on a mode switch. All instructions operate on `files[active]`.

`SIE`, `DPL`, `TUR`, `TKR`, `MMAP`, `MUNMAP`, `MPROTECT`, direct writes to `cr`, and writes
to `mr` are privileged. Executing any of them when `active == 1` raises interrupt 6.

### Process launch sequence

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

After `DPL` the kernel is no longer running. The next path back is through an interrupt.

### Interrupt vectors

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

Reservation ranges within `16..255` for specific hardware device classes will be defined when
device support is implemented.

### Interrupt dispatch

`raise_interrupt(vm, index)` is the only entry point. It always switches to and runs in the
kernel file.

Dispatch sequence:

1. Write fault information to the fault info region.
2. If `pending_interrupt` is `Some(1)` and the incoming index is also `1`, this is a double
   fault: halt the VM immediately.
3. Set `pending_interrupt = Some(index)`.
4. Record whether the interrupted file was user (`came_from_user = active == 1`).
5. Switch to the kernel file (`active = 0`).
6. Push onto the kernel stack in this order (all values big-endian):
   - the 16 general-purpose registers from the interrupted file (64 bytes)
   - the interrupted `ip` (4 bytes)
   - the interrupted `cr` (4 bytes)
   - the interrupted `flags` (4 bytes)
   - `came_from_user` as a single byte (1 byte)
7. Look up `ivt[index]`. If the address is `0`, halt the VM.
8. Set kernel `ip` to the handler address and clear `pending_interrupt`.

The interrupted `ip` pushed in step 6 is chosen by the caller before `raise_interrupt` runs:

- fetch-time faults push `ip + 1`
- execute-time faults push `ip + instruction.size`
- `INT` pushes the address of the instruction following `INT`

Total frame size pushed onto the kernel stack: 77 bytes.

### IRET

`IRET` unwinds exactly one interrupt frame from the kernel stack:

1. Pop `came_from_user` (1 byte), `flags` (4 bytes), `cr` (4 bytes), `ip` (4 bytes), and the
   16 general-purpose registers (64 bytes) from kernel `sp` in reverse push order.
2. If `came_from_user == 1`, restore the popped state into the user file and set `active = 1`.
   Otherwise restore into the kernel file and stay in kernel mode.

`IRET` with nothing on the kernel stack raises interrupt 6.

### Double fault

If a bus fault (interrupt 1) arrives while `pending_interrupt` is already `Some(1)` the VM
halts immediately with no further interrupt delivery. This condition indicates the kernel stack
has overflowed or the interrupt handler itself is faulting.

---

## Loader and address patching

The loader performs the following steps at startup:

1. Build the physical layout: write the device discovery table into the first bytes of RAM.
2. Compute the virtual base address for each non-empty section by starting after the fixed
   regions (device table, fault info) and advancing by `ceil(section_size / 4096) * 4096` for
   each section in order.
3. Walk the relocation table. Each entry `(section: u8, offset: u32)` identifies a 4-byte slot
   within that section. Patch the slot by replacing the assembler-assumed address with the
   actual loaded address:
   `patched = actual_base[section] + (slot_value - assumed_base[section])`.
4. Map each non-empty section into context 0 with the correct permissions.
5. Map the stack: last 4 MB of physical RAM to `0xFFC00000..0xFFFFFFFF` with RAM permissions.
6. Set kernel `ip` to the patched entry point address.

Section indices used in relocation entries:

| Value | Section |
|-------|---------|
| 0 | `.rodata` |
| 1 | `.code` |
| 2 | `.data` |

---

## Binary format (v3)

Big-endian throughout. All length and count fields are 4 bytes.

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

Relocations identify 32-bit address slots that must be patched by the loader when mapping
sections to their actual virtual base addresses. Each relocation names the section the slot
belongs to and its byte offset within that section.

---

## Execution loop

`step` performs fetch, decode, and execute for one instruction using the active register file.

IP advancement: control-flow instructions update `ip` themselves. All other instructions have
`ip` incremented by `instruction.size` after execute returns.

Faults raised during `step`:

| Cause | Interrupt |
|-------|-----------|
| bus fault on fetch, read, or write | 1 |
| unrecognised opcode byte | 2 |
| privileged instruction in user mode | 6 |

If the IVT entry for the raised interrupt is `0` the VM halts immediately. Otherwise the
handler runs in the kernel file and returns via `IRET`.

---

## Privileged and interrupt opcodes

| Mnemonic | Encoding | Effect |
|----------|----------|--------|
| `SIE rb, label` | `op rb_enc imm32[4]` | `ivt[rb] = imm32` |
| `SIE rb, rw` | `op rb_enc rw_enc` | `ivt[rb] = rw` |
| `INT imm8` | `op imm8` | raise interrupt `imm8` |
| `INT rb` | `op rb_enc` | raise interrupt `rb` |
| `IRET` | `op` | unwind one interrupt frame from the kernel stack |
| `DPL` | `op` | activate user file, one-way until next interrupt |
| `TUR rw, rw\|ip\|cr` | `op rw_enc rw_enc` | copy user register into kernel register; `ip` and `cr` valid as source |
| `TKR rw\|ip\|cr, rw` | `op rw_enc rw_enc` | copy kernel register into user register; `ip` and `cr` valid as destination |
| `MMAP rw, rw, rw\|imm32` | `op rw_enc rw_enc rw_enc\|imm32` | map physical pages into context `mr`; operands are page numbers and page count |
| `MUNMAP rw, rw\|imm32` | `op rw_enc rw_enc\|imm32` | unmap pages from context `mr`; operands are page number and page count |
| `MPROTECT rw, rw\|imm32, rb` | `op rw_enc rw_enc\|imm32 rb_enc` | set permission bits in context `mr`; operands are page number and page count |

Rules:

- `SIE`, `DPL`, `TUR`, `TKR`, `MMAP`, `MUNMAP`, `MPROTECT`, writes to `cr`, and writes to
  `mr` are privileged. Executing any of them in user mode raises interrupt 6.
- `IRET` with nothing on the kernel stack raises interrupt 6.
- `SIE` index operand must be `rb`.
- `INT` immediate form accepts a `u8` literal. Register form requires `rb`. Using a wider
  register view with `INT` is an invalid opcode.
- `TUR` and `TKR` general-purpose operands must be `rw`. `ip` and `cr` are additionally valid
  as the source of `TUR` and the destination of `TKR`.
- `MMAP`, `MUNMAP`, and `MPROTECT` address operands are page numbers, not byte addresses. The
  size operand is a page count. The size operand also accepts `imm32`.
- `MPROTECT` permission operand must be `rb`.
- `MMAP`, `MUNMAP`, and `MPROTECT` target the context in `mr`, not `cr`.