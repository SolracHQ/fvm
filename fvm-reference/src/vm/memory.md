# Memory and the bus

32-bit address space. All memory access goes through the bus. There is no flat
backing array; the bus owns a list of physical regions each backed by either a
RAM device or a memory-mapped device. Unmapped virtual addresses always fault.

## Physical layout

At VM startup the host assembles a contiguous physical address space from the
enabled devices. RAM is placed first at physical address `0x00000000`, then each
additional device is placed immediately after the previous one. The result is a
linear physical layout fixed at startup and never modified at runtime.

RAM size is configured at startup. It is not required to be 4 GB or any fixed
size.

The first pages of RAM hold the loader info region, written once by the VM
before the kernel runs and mapped read-only in context 0. It is the single
source of truth for boot-time topology: what physical memory and port devices
exist, and which pages the loader already consumed.

## Virtual address translation

The bus maintains a page table per context in host Rust memory, not accessible
to guest code. Each entry covers one 4 kb page and records the backing device,
the offset within that device, and the permissions for that mapping.

On every access the bus computes `page = virt_addr >> 12`, looks up the entry
for `(current_cr, page)`, checks permissions, then dispatches to the device at
`device_offset = entry.device_page_base + (virt_addr & 0xFFF)`.

`MMAP`, `MUNMAP`, and `MPROTECT` consult `mr` instead of `cr` when selecting
the target context. All other memory accesses use `cr`.

The address and count operands of `MMAP`, `MUNMAP`, and `MPROTECT` are page
numbers and page counts, not byte addresses. The bus multiplies them by 4096
internally. A page number of 1 refers to the page starting at byte address
`0x1000`.

## Permissions

| Bit | Name | Effect |
|-----|------|--------|
| 0 | Read | allows data reads |
| 1 | Write | allows data writes |
| 2 | Execute | allows instruction fetch |

Common combinations:

| Name | Bits | Used for |
|------|------|----------|
| ROM | `001` | `.rodata`, loader info |
| Code | `101` | `.code` |
| RAM | `011` | `.data`, stack |

Violating any permission raises interrupt 1.

## Default virtual layout in context 0

The loader builds this layout at startup. All addresses are determined at load
time; nothing is hardcoded. Each region starts at the next 4 kb boundary after
the previous region. Sections with size 0 are skipped entirely.

```
page 0        loader info region   ROM   ceil(loader_info_size / 4096) pages
next page     .rodata              ROM   ceil(rodata_size / 4096) pages
next page     .code                Code  ceil(code_size / 4096) pages
next page     .data                RAM   ceil(data_size / 4096) pages
...           unmapped
0xFFC00000 .. 0xFFFFFFFF           RAM   stack, mapped to last 4 MB of RAM
```

The stack region is always mapped regardless of RAM size. It maps the last 4 MB
of physical RAM to the fixed virtual range `0xFFC00000..0xFFFFFFFF`. `sp` in
the kernel file is initialized to `0xFFFFFFFF` on VM creation.

If the end of `.data` would overlap `0xFFC00000` the loader returns an error.

## Loader info region

Starts at physical page 0 and is mapped read-only at virtual page 0 in context
0. The loader writes it once before the kernel runs. Size is rounded up to the
nearest page boundary after all fields are serialized.

All multi-byte fields are big-endian. A `Vec` in the layout below means a
4-byte count followed by that many entries.

```
enum KernelMappingKind : u8 {
    LoaderInfo = 0,
    Rodata     = 1,
    Code       = 2,
    Data       = 3,
    Stack      = 4,
}

struct MemoryRegion {
    id:         [u8; 8],   // 8-byte ASCII device id, unused bytes zero-padded
    base_page:  u32,
    page_count: u32,
}

struct PortDevice {
    id:   [u8; 8],         // 8-byte ASCII device id, unused bytes zero-padded
    port: u32,
}

struct KernelMapping {
    kind:       KernelMappingKind,  // 1 byte
    base_page:  u32,
    page_count: u32,               // 0 if the section was empty and not mapped
}

struct LoaderInfo {
    memory_regions:  Vec<MemoryRegion>,
    port_devices:    Vec<PortDevice>,
    kernel_mappings: Vec<KernelMapping>,
}
```

`memory_regions` lists every physical RAM and memory-mapped device region in
physical address order. The kernel uses this to enumerate what backing memory
exists.

`port_devices` lists every port-mapped device with its port number. There is no
separate discovery mechanism for port devices; this is the only place the kernel
learns about them.

`kernel_mappings` lists every page range the loader reserved, in layout order.
The kernel reads this at startup and marks all listed pages as occupied before
doing any allocation. Entries with `page_count = 0` were not loaded and hold no
physical pages.

## Bus device trait

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

Multi-byte reads and writes are atomic at the device level. Devices with wide
registers override the relevant methods. RAM provides default implementations
that compose bytes in big-endian order.

`VmResult` may carry an `Interrupt` variant. A device raising an interrupt
returns `Err(VmError::Interrupt(n))`. The bus propagates this to the execution
loop which delivers it through the normal interrupt dispatch path.
