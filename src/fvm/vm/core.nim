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
  ## load correctly: the code region starts at 0x0000 and the rest of the
  ## rule still applies.
  let rodataBase = 0x0000'u16
  let codeEnd = obj.rodata.len + obj.code.len
  let dataEnd = codeEnd + obj.data.len

  if dataEnd > int(StackRegionBase):
    return (
      "Program sections exceed available address space: data ends at 0x" &
      toHex(dataEnd, 4) & " but stack begins at 0x" & toHex(int(StackRegionBase), 4)
    ).err

  let codeBase = Address(obj.rodata.len)
  let dataBase = Address(codeEnd)
  # Write bytes into backing memory before mapping regions so writes bypass
  # the permission checks that will be installed below.
  if obj.rodata.len > 0:
    vm.bus.writeRangeDirect(rodataBase, obj.rodata)
  if obj.code.len > 0:
    vm.bus.writeRangeDirect(codeBase, obj.code)
  if obj.data.len > 0:
    vm.bus.writeRangeDirect(dataBase, obj.data)

  # Map sections as the appropriate region types.
  if obj.rodata.len > 0:
    ?vm.bus.mapRegion(romRegion(rodataBase, uint32(obj.rodata.len), "rodata"))
  if obj.code.len > 0:
    ?vm.bus.mapRegion(codeRegion(codeBase, uint32(obj.code.len), "code"))
  if obj.data.len > 0:
    ?vm.bus.mapRegion(ramRegion(dataBase, uint32(obj.data.len), "data"))
  ?vm.bus.mapRegion(ramRegion(StackRegionBase, StackRegionSize, "stack"))

  vm.ip = obj.entryPoint
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
