## FVM VM lifecycle and fetch/decode/execute engine.

import std/logging
import std/strutils

import ../format/fvmobject as fmtobject
import ../core/constants
import ./decoders
import ./handlers

export decoders, handlers, fmtobject

proc newVm*(): FvmResult[Vm] =
  ## Creates a VM with an empty memory bus. Call `initRom` before executing.
  Vm(bus: newBus(), ip: 0'u16, sp: StackBase, halted: false).ok

proc applyRelocations(
    mem: var seq[Byte], codeBase: Address, baseShift: uint16, relocations: seq[uint16]
): FvmResult[void] =
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
  let rodataBase = Address(IvtBase + IvtSize)
  let codeBase = Address(uint16(rodataBase) + uint16(obj.rodata.len))
  let dataBase = Address(uint16(codeBase) + uint16(obj.code.len))
  let dataEnd = int(dataBase) + obj.data.len

  if obj.rodata.len + obj.code.len + obj.data.len + IvtSize.int > int(StackRegionBase):
    return (
      "Program sections exceed available address space: data ends at 0x" &
      toHex(dataEnd, 4) & " but stack begins at 0x" & toHex(int(StackRegionBase), 4)
    ).err

  if obj.rodata.len > 0:
    vm.bus.writeRangeDirect(rodataBase, obj.rodata)
  if obj.code.len > 0:
    vm.bus.writeRangeDirect(codeBase, obj.code)
  if obj.data.len > 0:
    vm.bus.writeRangeDirect(dataBase, obj.data)

  ?applyRelocations(vm.bus.mem, codeBase, IvtSize, obj.relocations)

  ?vm.bus.mapRegion(ramRegion(IvtBase, uint32(IvtSize), "ivt"))
  if obj.rodata.len > 0:
    ?vm.bus.mapRegion(romRegion(rodataBase, uint32(obj.rodata.len), "rodata"))
  if obj.code.len > 0:
    ?vm.bus.mapRegion(codeRegion(codeBase, uint32(obj.code.len), "code"))
  if obj.data.len > 0:
    ?vm.bus.mapRegion(ramRegion(dataBase, uint32(obj.data.len), "data"))
  ?vm.bus.mapRegion(ramRegion(StackRegionBase, StackRegionSize, "stack"))

  debug "Entry point: 0x" & toHex(int(obj.entryPoint), 4)
  vm.ip = Address(uint16(obj.entryPoint) + IvtSize)
  ok()

proc parseOpCode*(code: Byte): FvmResult[OpCode] =
  try:
    OpCode(code).ok
  except RangeDefect:
    ("Invalid opcode byte: 0x" & toHex(int(code), 2)).err

proc fetch*(vm: Vm): FvmResult[OpCode] =
  if int(vm.ip) >= VmMemorySize:
    return "Instruction pointer out of bounds".err
  parseOpCode(?vm.bus.fetch8(vm.ip))

proc step*(vm: var Vm): FvmResult[void] =
  let opcode = ?vm.fetch()
  let insn = ?vm.decode(opcode)
  vm.execute(insn)

proc run*(vm: var Vm): FvmResult[void] =
  while not vm.halted:
    ?vm.step()
  ok()
