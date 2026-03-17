# Loader

The loader runs once at startup and sets up context 0 before the kernel starts.

Steps:

1. Serialise the loader info region into the first pages of RAM and map them
   read-only in context 0.
2. Compute the virtual base address for each non-empty section by starting after
   the loader info region and advancing by `ceil(section_size / 4096) * 4096`
   for each section in order.
3. Walk the relocation table. Each entry `(section: u8, offset: u32)` identifies
   a 4-byte slot within that section. Patch the slot by replacing the
   assembler-assumed address with the actual loaded address:
   `patched = actual_base[section] + (slot_value - assumed_base[section])`.
4. Map each non-empty section into context 0 with the correct permissions.
5. Map the stack: last 4 MB of physical RAM to `0xFFC00000..0xFFFFFFFF` with
   RAM permissions.
6. Set kernel `ip` to the patched entry point address.

Section indices used in relocation entries:

| Value | Section |
|-------|---------|
| 0 | `.rodata` |
| 1 | `.code` |
| 2 | `.data` |
