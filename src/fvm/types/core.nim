## Core primitive type aliases and VM-wide constants.

import results

export results

type

  FvmResult*[T] = Result[T, string]

  Byte* = uint8
    ## The basic unit of memory and I/O in the FVM. Instructions are byte-aligned and may be 1-3 bytes long.
  Word* = uint16
    ## This VM is 16 bits addresses so to keep convention I will call it word
  Address* = Word
    ## Memory addresses are 16 bits, addressing a flat 64 KB address space.
  RegEncoding* = distinct Byte
    ## Register operand encoding: one byte per register operand, with bits to indicate lane and high/low byte access.

  Permission* = enum
    Read ## region may be read by data loads and opcode fetch
    Write ## region may be written by data stores
    Execute ## region may supply opcodes to the fetch stage

  Permissions* = set[Permission]

const
  VmMemorySize* = 0x1_0000 ## Flat address space: 64 KB
  StackBase* = 0xFFFF'u16 ## Stack grows downward from here
  StackRegionBase* = 0xF000'u16 ## Stack region starts here
  StackRegionSize* = 0x1000'u32 ## 4 KB stack region
  IvtBase* = 0x0000'u16.Address ## Interrupt vector table base address
  IvtEntries* = 16 ## Number of IVT entries
  IvtSize* = IvtEntries.Address * 2 ## 16 entries × 2 bytes each = 32 bytes
  GeneralRegisterCount* = 16
  MaxPortCount* = 256 ## Number of I/O ports for IN/OUT
  ByteMask* = 0xFF'u16 ## Low-byte extraction mask
  FvmHeaderSize* = 15
    ## magic(4) + version(1) + entry(2) + rodataLen(2) + codeLen(2) + dataLen(2) + relocCount(2)

  # Common permission sets
  PermRom*: Permissions = {Read} ## read-only: .rodata
  PermCode*: Permissions = {Read, Execute} ## executable code segment
  PermRam*: Permissions = {Read, Write} ## read/write data or stack

  # Register operand encoding (one byte per operand)
  #   bits 7:6  lane field  00=full16  10=low byte  11=high byte
  #              01 is otherwise invalid; we use it to encode SP
  #   bits 5:4  reserved
  #   bits 3:0  register index 0-15 (ignored for SP)
  RegLaneBit* = 0b1000_0000'u8 ## Set when a byte-lane access is requested
  RegHighBit* = 0b0100_0000'u8 ## Selects high byte (only meaningful with RegLaneBit)
  RegIndexMask* = 0b0000_1111'u8 ## Isolates the register index from an encoding byte
  SpEncoding* = RegEncoding(0b0100_0000'u8)
    ## bits[7:6] = 01, the otherwise-invalid state

# RegEncoding predicates

proc newRegEncoding*(index: int, lane: bool = false, high: bool = false, isSp: bool = false): FvmResult[RegEncoding] =
  if isSp:
    if lane or high:
      return "SP encoding cannot specify lane or high byte".err
    else:
      return SpEncoding.ok
  else:
    if index < 0 or index >= GeneralRegisterCount:
      return ("Register index out of range: " & $index).err
    var encoding = RegEncoding(Byte(index) and RegIndexMask)
    if lane:
      encoding = RegEncoding(Byte(encoding) or RegLaneBit)
    if high:
      encoding = RegEncoding(Byte(encoding) or RegHighBit)
    return encoding.ok

proc `==`*(a, b: RegEncoding): bool {.borrow.}

proc isSp*(r: RegEncoding): bool =
  (Byte(r) and (RegLaneBit or RegHighBit)) == RegHighBit

proc isLane*(r: RegEncoding): bool =
  (Byte(r) and RegLaneBit) != 0

proc isHigh*(r: RegEncoding): bool =
  (Byte(r) and (RegLaneBit or RegHighBit)) == (RegLaneBit or RegHighBit)

proc isLow*(r: RegEncoding): bool =
  r.isLane and not r.isHigh

proc isWord*(r: RegEncoding): bool =
  not r.isLane and not r.isSp

proc index*(r: RegEncoding): int =
  int(Byte(r) and RegIndexMask)

proc laneTag*(r: RegEncoding): Byte =
  if r.isSp:
    0'u8
  else:
    Byte(r) and (RegLaneBit or RegHighBit)
