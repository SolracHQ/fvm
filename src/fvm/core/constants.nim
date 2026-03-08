import ./types

export types

const
  VmMemorySize* = 0x1_0000 ## Flat address space size: 64 KB.
  StackBase* = 0xFFFF'u16 ## Stack grows downward from the top of memory.
  StackRegionBase* = 0xF000'u16 ## Base address of the reserved stack region.
  StackRegionSize* = 0x1000'u32 ## Stack region size in bytes.
  IvtBase* = 0x0000'u16.Address ## Base address of the interrupt vector table.
  IvtEntries* = 16 ## Number of interrupt vector entries.
  IvtSize* = IvtEntries.Address * 2 ## IVT byte size: 16 entries × 2 bytes.
  GeneralRegisterCount* = 16 ## Number of general-purpose registers.
  MaxInstructionArgs* = 2'u8 ## Maximum operand count supported by the ISA.
  MaxPortCount* = 256 ## Number of addressable I/O ports.
  ByteMask* = 0xFF'u16 ## Mask for extracting the low byte of a word.
  FvmHeaderSize* = 15 ## Fixed-size object file header length in bytes.

  PermRom*: Permissions = {Read} ## Permission set for read-only memory.
  PermCode*: Permissions = {Read, Execute} ## Permission set for executable code.
  PermRam*: Permissions = {Read, Write} ## Permission set for writable memory.

  RegLaneBit* = 0b1000_0000'u8 ## Bit set when accessing a byte lane.
  RegHighBit* = 0b0100_0000'u8 ## Bit selecting the high byte lane.
  RegIndexMask* = 0b0000_1111'u8 ## Mask that isolates the register index bits.
  SpEncoding* = RegEncoding(0b0100_0000'u8) ## Reserved encoding used for SP.
