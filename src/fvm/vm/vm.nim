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
  Vm(
    bus: newBus(),
    ip: 0'u16,
    sp: StackBase,
    privileged: true,
    halted: false,
  ).ok

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
  let rodataBase = Address(0'u16)
  let codeBase = Address(uint16(rodataBase) + uint16(obj.rodata.len))
  let dataBase = Address(uint16(codeBase) + uint16(obj.code.len))
  let dataEnd = int(dataBase) + obj.data.len

  if obj.rodata.len + obj.code.len + obj.data.len > int(StackRegionBase):
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

  ?applyRelocations(vm.bus.mem, codeBase, 0'u16, obj.relocations)

  if obj.rodata.len > 0:
    ?vm.bus.mapRegion(romRegion(rodataBase, uint32(obj.rodata.len), "rodata"))
  if obj.code.len > 0:
    ?vm.bus.mapRegion(codeRegion(codeBase, uint32(obj.code.len), "code"))
  if obj.data.len > 0:
    ?vm.bus.mapRegion(ramRegion(dataBase, uint32(obj.data.len), "data"))
  ?vm.bus.mapRegion(ramRegion(StackRegionBase, StackRegionSize, "stack"))

  debug "Entry point: 0x" & toHex(int(obj.entryPoint), 4)
  vm.ip = obj.entryPoint
  vm.sp = StackBase
  vm.flags = {}
  vm.inInterrupt = false
  vm.privileged = true
  vm.ivt = default(array[IvtEntryCount, Address])
  vm.ictx = default(InterruptContext)
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

proc classifyRuntimeFault(message: string): int =
  if message.startsWith("Bus "):
    InterruptBusFault
  elif message.startsWith("Stack overflow"):
    InterruptStackOverflow
  elif message.startsWith("Stack underflow"):
    InterruptStackUnderflow
  elif message.startsWith("Privileged instruction") or
      message.startsWith("IRET outside interrupt handler") or
      message.startsWith("Invalid interrupt vector index"):
    InterruptPrivilegeFault
  else:
    -1

proc step*(vm: var Vm): FvmResult[void] =
  let fetchByte = vm.bus.fetch8(vm.ip)
  if fetchByte.isErr:
    let fault = classifyRuntimeFault(fetchByte.error)
    if fault >= 0:
      vm.ip += 1
      ?vm.raiseInterrupt(fault)
      return ok()
    return fetchByte.error.err

  let opcode = parseOpCode(fetchByte.get())
  if opcode.isErr:
    vm.ip += 1
    ?vm.raiseInterrupt(InterruptInvalidOpcode)
    return ok()

  let insn = vm.decode(opcode.get())
  if insn.isErr:
    let fault = classifyRuntimeFault(insn.error)
    if fault >= 0:
      vm.ip += 1
      ?vm.raiseInterrupt(fault)
      return ok()
    return insn.error.err

  let decoded = insn.get()
  let startIp = vm.ip
  let executed = vm.execute(decoded)
  if executed.isErr:
    let fault = classifyRuntimeFault(executed.error)
    if fault >= 0:
      vm.ip = startIp + Address(decoded.size)
      ?vm.raiseInterrupt(fault)
      return ok()
    return executed.error.err
  ok()

proc run*(vm: var Vm): FvmResult[void] =
  while not vm.halted:
    ?vm.step()
  ok()
