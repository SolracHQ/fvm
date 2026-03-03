## FVM core: VM lifecycle and execution engine.

import std/strutils

import ../format/fvmobject as fmtobject
import ./handlers

export handlers, fmtobject

# Lifecycle

proc newVm*(): FvmResult[Vm] =
  ## Creates a VM with an empty memory bus.  Call `initRom` to map sections
  ## and load program data before executing.
  Vm(bus: newBus(), ip: 0'u16, sp: StackBase, halted: false).ok

proc applyRelocations(
    mem: var seq[Byte], codeBase: Address, baseShift: uint16, relocations: seq[uint16]
): FvmResult[void] =
  ## Patches assembler-zero-relative addresses stored in .code by adding
  ## baseShift (the offset from VM address 0 to the assembler's address 0,
  ## i.e. IvtSize). codeBase locates each relocation entry in memory.
  for reloc in relocations:
    let off = int(codeBase) + int(reloc)
    if off + 1 >= mem.len:
      return ("Relocation offset out of range: " & $reloc).err
    let original = (uint16(mem[off]) shl 8) or uint16(mem[off + 1])
    let patched = original + baseShift
    mem[off] = Byte(patched shr 8)
    mem[off + 1] = Byte(patched and 0xFF)
  ok()

proc initRom*(vm: var Vm, obj: FvmObject): FvmResult[void] =
  ## Maps memory sections from an FvmObject and initialises the instruction
  ## pointer.  Three sections are supported:
  ##
  ##   .rodata  read-only constants, at 0x0000
  ##   .code    executable bytecode, immediately after .rodata
  ##   .data    mutable initialised data, immediately after .code
  ##   stack    always at StackRegionBase (0xF000)
  ##
  ## Programs that contain only a .code section (no rodata/data fields set)
  ## load correctly: the code region starts immediately after the IVT.
  ##
  ## Memory layout:
  ##   0x0000  IVT     32 bytes (16 entries × 2 bytes), writable RAM
  ##   0x0020  .rodata read-only constants (may be empty)
  ##   0x0020+ .code   executable bytecode
  ##          + .data   mutable initialised data
  ##   0xF000  stack   4 KB
  ##
  ## Embedded 16-bit addresses in .code are assembler-zero-relative and are
  ## shifted by IvtSize at load time via the relocation table.
  let rodataBase = Address(IvtBase + IvtSize)
  let codeBase = Address(uint16(rodataBase) + uint16(obj.rodata.len))
  let dataBase = Address(uint16(codeBase) + uint16(obj.code.len))
  let dataEnd = int(dataBase) + obj.data.len

  if obj.rodata.len + obj.code.len + obj.data.len + IvtSize.int > int(StackRegionBase):
    return (
      "Program sections exceed available address space: data ends at 0x" &
      toHex(dataEnd, 4) & " but stack begins at 0x" & toHex(int(StackRegionBase), 4)
    ).err

  # Write bytes into backing memory before mapping regions so writes bypass
  # the permission checks that will be installed below.
  if obj.rodata.len > 0:
    vm.bus.writeRangeDirect(rodataBase, obj.rodata)
  if obj.code.len > 0:
    vm.bus.writeRangeDirect(codeBase, obj.code)
  if obj.data.len > 0:
    vm.bus.writeRangeDirect(dataBase, obj.data)

  # Shift all assembler-zero-relative addresses by IvtSize.
  ?applyRelocations(vm.bus.mem, codeBase, IvtSize, obj.relocations)

  # Map the IVT as writable RAM so kernel code can install handlers.
  ?vm.bus.mapRegion(ramRegion(IvtBase, uint32(IvtSize), "ivt"))
  # Map sections as the appropriate region types.
  if obj.rodata.len > 0:
    ?vm.bus.mapRegion(romRegion(rodataBase, uint32(obj.rodata.len), "rodata"))
  if obj.code.len > 0:
    ?vm.bus.mapRegion(codeRegion(codeBase, uint32(obj.code.len), "code"))
  if obj.data.len > 0:
    ?vm.bus.mapRegion(ramRegion(dataBase, uint32(obj.data.len), "data"))
  ?vm.bus.mapRegion(ramRegion(StackRegionBase, StackRegionSize, "stack"))

  # Entry point is assembler-zero-relative; shift it by IvtSize.
  vm.ip = Address(uint16(obj.entryPoint) + IvtSize)
  ok()

# Opcode dispatch

proc parseOpCode*(code: Byte): FvmResult[OpCode] =
  try:
    OpCode(code).ok
  except RangeDefect:
    ("Invalid opcode byte: 0x" & toHex(int(code), 2)).err

# Execution

proc step*(vm: var Vm): FvmResult[void] =
  ## Fetches the opcode at the current IP and executes one instruction.
  ## The handler is responsible for advancing vm.ip.
  if int(vm.ip) >= VmMemorySize:
    return "Instruction pointer out of bounds".err

  let codeByte = (?vm.bus.fetch8(vm.ip))
  let opcode = (?parseOpCode(codeByte))
  let def = getInstructionDef(opcode)

  if def.handler == nil:
    return ("No handler for opcode 0x" & toHex(int(codeByte), 2)).err

  ?def.handler(vm)
  ok()

proc run*(vm: var Vm): FvmResult[void] =
  ## Runs the VM until it halts or an error occurs.
  while not vm.halted:
    ?vm.step()
  ok()
