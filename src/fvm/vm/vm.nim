## FVM VM lifecycle and fetch/decode/execute engine.

import std/logging
import std/strutils

import ../format/fvmobject as fmtobject
import ../core/constants
import ../errors
import ./decoders
import ./handlers

export decoders, handlers, fmtobject

proc newVm*(): Vm =
  ## Creates a VM with an empty memory bus. Call `initRom` before executing.
  Vm(
    bus: newBus(),
    ip: 0'u16,
    sp: StackBase,
    privileged: true,
    halted: false,
  )

proc applyRelocations(
    mem: var seq[Byte], codeBase: Address, baseShift: uint16, relocations: seq[uint16]
) =
  for reloc in relocations:
    let off = int(codeBase) + int(reloc)
    if off + 1 >= mem.len:
      raise newRelocationError("Relocation offset out of range: " & $reloc)
    let original = (uint16(mem[off]) shl 8) or uint16(mem[off + 1])
    let patched = original + baseShift
    mem[off] = Byte(patched shr 8)
    mem[off + 1] = Byte(patched and 0xFF)

proc initRom*(vm: var Vm, obj: FvmObject) =
  let rodataBase = Address(0'u16)
  let codeBase = Address(uint16(rodataBase) + uint16(obj.rodata.len))
  let dataBase = Address(uint16(codeBase) + uint16(obj.code.len))
  let dataEnd = int(dataBase) + obj.data.len

  if obj.rodata.len + obj.code.len + obj.data.len > int(StackRegionBase):
    raise newVmLayoutError(
      "Program sections exceed available address space: data ends at 0x" &
      toHex(dataEnd, 4) & " but stack begins at 0x" & toHex(int(StackRegionBase), 4)
    )

  if obj.rodata.len > 0:
    vm.bus.writeRangeDirect(rodataBase, obj.rodata)
  if obj.code.len > 0:
    vm.bus.writeRangeDirect(codeBase, obj.code)
  if obj.data.len > 0:
    vm.bus.writeRangeDirect(dataBase, obj.data)

  applyRelocations(vm.bus.mem, codeBase, 0'u16, obj.relocations)

  if obj.rodata.len > 0:
    vm.bus.mapRegion(romRegion(rodataBase, uint32(obj.rodata.len), "rodata"))
  if obj.code.len > 0:
    vm.bus.mapRegion(codeRegion(codeBase, uint32(obj.code.len), "code"))
  if obj.data.len > 0:
    vm.bus.mapRegion(ramRegion(dataBase, uint32(obj.data.len), "data"))
  vm.bus.mapRegion(ramRegion(StackRegionBase, StackRegionSize, "stack"))

  debug "Entry point: 0x" & toHex(int(obj.entryPoint), 4)
  vm.ip = obj.entryPoint
  vm.sp = StackBase
  vm.flags = {}
  vm.inInterrupt = false
  vm.privileged = true
  vm.ivt = default(array[IvtEntryCount, Address])
  vm.ictx = default(InterruptContext)

proc parseOpCode*(code: Byte): OpCode =
  try:
    OpCode(code)
  except RangeDefect:
    raise newInvalidOpcodeError(code)

proc fetch*(vm: Vm): OpCode =
  if int(vm.ip) >= VmMemorySize:
    raise newInstructionPointerError()
  parseOpCode(vm.bus.fetch8(vm.ip))

proc raiseFaultInterrupt(vm: var Vm, fault: ref VmFault, nextIp: Address) =
  vm.ip = nextIp
  if fault of BusFaultError:
    vm.raiseInterrupt(InterruptBusFault)
  elif fault of errors.StackOverflowError:
    vm.raiseInterrupt(InterruptStackOverflow)
  elif fault of errors.StackUnderflowError:
    vm.raiseInterrupt(InterruptStackUnderflow)
  elif fault of PrivilegeFaultError:
    vm.raiseInterrupt(InterruptPrivilegeFault)
  elif fault of InvalidOpcodeError:
    vm.raiseInterrupt(InterruptInvalidOpcode)
  else:
    raise fault

proc step*(vm: var Vm) =
  let opcode =
    try:
      vm.fetch()
    except VmFault as e:
      vm.raiseFaultInterrupt(e, vm.ip + 1)
      return

  let decoded =
    try:
      vm.decode(opcode)
    except VmFault as e:
      vm.raiseFaultInterrupt(e, vm.ip + 1)
      return

  let startIp = vm.ip
  try:
    vm.execute(decoded)
  except VmFault as e:
    vm.raiseFaultInterrupt(e, startIp + Address(decoded.size))

proc run*(vm: var Vm) =
  while not vm.halted:
    vm.step()
