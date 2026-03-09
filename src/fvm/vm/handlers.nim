## Instruction handler procs and the InstructionDef dispatch table.

import std/logging
import std/strformat
import std/strutils

import ./types
import ./ports
import ../core/types
import ../core/constants
import ../core/registers
import ../core/flags
import ../utils

export types

proc getInstructionDef*(opcode: OpCode): InstructionDef

proc raiseInterrupt*(vm: var Vm, index: int): FvmResult[void]

# Shared helpers

proc raiseInterrupt*(vm: var Vm, index: int): FvmResult[void] =
  if index < 0 or index >= IvtEntryCount:
    return ("Interrupt index out of range: " & $index).err

  if vm.inInterrupt:
    debug "Dropping nested interrupt " & $index
    return ok()

  vm.ictx = InterruptContext(
    regs: vm.regs,
    ip: vm.ip,
    sp: vm.sp,
    flags: vm.flags,
    privileged: vm.privileged,
  )
  vm.inInterrupt = true
  vm.privileged = true

  let target = vm.ivt[index]
  if target == 0'u16:
    debug "Unhandled interrupt " & $index & ", halting"
    vm.halted = true
    return ok()

  debug "Interrupt " & $index & " -> 0x" & toHex(int(target), 4)
  vm.ip = target
  ok()

proc requirePrivileged(vm: Vm, instruction: string): FvmResult[void] =
  if not vm.privileged:
    return ("Privileged instruction: " & instruction).err
  ok()

proc interruptIndex(value: Word): FvmResult[int] =
  if value >= Word(IvtEntryCount):
    return ("Invalid interrupt vector index: " & $value).err
  int(value).ok

proc decodeReg(vm: Vm, enc: RegEncoding): Word =
  if enc.isSp:
    vm.sp
  elif enc.isHigh:
    vm.regs[enc.index] shr 8
  elif enc.isLow:
    vm.regs[enc.index] and ByteMask
  else:
    vm.regs[enc.index]

proc writeReg(vm: var Vm, enc: RegEncoding, value: Word) =
  if enc.isSp:
    vm.sp = value
  elif enc.isHigh:
    vm.regs[enc.index] =
      (vm.regs[enc.index] and ByteMask) or ((value and ByteMask) shl 8)
  elif enc.isLow:
    vm.regs[enc.index] = (vm.regs[enc.index] and 0xFF00'u16) or (value and ByteMask)
  else:
    vm.regs[enc.index] = value

proc setArithFlags(vm: var Vm, value: Word, carry: bool, isLane: bool) =
  let highBit = if isLane: 7'u16 else: 15'u16
  vm.flags.setBooleanFlag(Zero, value == 0)
  vm.flags.setBooleanFlag(Negative, (value shr highBit) != 0)
  vm.flags.setBooleanFlag(Carry, carry)

# Arithmetic cores
#
# Pure arithmetic used by both the reg-reg and reg-imm handler variants.
# Keeping the operation in one place ensures flag behavior stays consistent
# regardless of where the second operand comes from.

type ArithResult = tuple[value: Word, carry: bool]

proc addCore(a, b: Word, isLane: bool): ArithResult {.inline.} =
  let mask =
    if isLane:
      uint32(ByteMask)
    else:
      0xFFFF'u32
  let raw = uint32(a) + uint32(b)
  (Word(raw and mask), raw > mask)

proc subCore(a, b: Word, isLane: bool): ArithResult {.inline.} =
  let mask =
    if isLane:
      uint32(ByteMask)
    else:
      0xFFFF'u32
  let carry = uint32(b) > uint32(a)
  (Word((uint32(a) - uint32(b)) and mask), carry)

proc expectReg(arg: FlatArg, name: string): FvmResult[RegEncoding] =
  if arg.kind != faReg:
    return ("Expected register operand for " & name).err
  arg.enc.ok

proc expectImm(arg: FlatArg, name: string): FvmResult[Word] =
  case arg.kind
  of faImm8:
    Word(arg.imm8).ok
  of faImm16:
    arg.imm16.ok
  else:
    ("Expected immediate operand for " & name).err

proc expectImm8(arg: FlatArg, name: string): FvmResult[Byte] =
  if arg.kind != faImm8:
    return ("Expected 8-bit immediate operand for " & name).err
  arg.imm8.ok

proc expectImm16(arg: FlatArg, name: string): FvmResult[Word] =
  if arg.kind != faImm16:
    return ("Expected 16-bit immediate operand for " & name).err
  arg.imm16.ok

# Control

proc handleNop(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  debug "NOP"

proc handleHalt(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  debug "HALT"
  vm.halted = true

proc handleIret(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  if not vm.inInterrupt:
    return "IRET outside interrupt handler".err
  vm.regs = vm.ictx.regs
  vm.ip = vm.ictx.ip
  vm.sp = vm.ictx.sp
  vm.flags = vm.ictx.flags
  vm.privileged = vm.ictx.privileged
  vm.inInterrupt = false
  debug fmt"IRET -> 0x{vm.ip:04X}"

proc handleDpl(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  ?requirePrivileged(vm, "DPL")
  vm.privileged = false
  debug "DPL -> user mode"

# Stack

proc handlePush(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let enc = ?expectReg(insn.args[0], "PUSH")
  let value = decodeReg(vm, enc)
  if enc.isLane:
    if vm.sp == 0'u16:
      return "Stack overflow on PUSH".err
    vm.sp -= 1
    ?vm.bus.write8(vm.sp, Byte(value))
    debug fmt"PUSH r{enc.index}(lane) = 0x{value:02X}"
  else:
    if vm.sp <= 1'u16:
      return "Stack overflow on PUSH".err
    vm.sp -= 2
    ?vm.bus.write16(vm.sp, value)
    debug fmt"PUSH r{enc.index} = 0x{value:04X}"

proc handlePop(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let enc = ?expectReg(insn.args[0], "POP")
  if enc.isLane:
    if vm.sp == StackBase:
      return "Stack underflow on POP".err
    let value = ?vm.bus.read8(vm.sp)
    vm.sp += 1
    writeReg(vm, enc, Word(value))
    debug fmt"POP r{enc.index}(lane) = 0x{value:02X}"
  else:
    if int(vm.sp) + 1 >= int(StackBase):
      return "Stack underflow on POP".err
    let value = ?vm.bus.read16(vm.sp)
    vm.sp += 2
    writeReg(vm, enc, value)
    debug fmt"POP r{enc.index} = 0x{value:04X}"

# Data movement

proc handleMovRegImm(
    vm: var Vm, insn: DecodedInstruction
): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "MOV")
  let imm = ?expectImm(insn.args[1], "MOV")
  if dstEnc.isLane:
    writeReg(vm, dstEnc, imm)
    debug fmt"MOV r{dstEnc.index}(lane), 0x{Byte(imm):02X}"
  else:
    writeReg(vm, dstEnc, imm)
    debug fmt"MOV r{dstEnc.index}, 0x{imm:04X}"

proc handleMovRegReg(
    vm: var Vm, insn: DecodedInstruction
): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "MOV")
  let srcEnc = ?expectReg(insn.args[1], "MOV")
  let value = decodeReg(vm, srcEnc)
  writeReg(vm, dstEnc, value)
  debug fmt"MOV r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"

proc handleZextRegReg(
    vm: var Vm, insn: DecodedInstruction
): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "ZEXT")
  let srcEnc = ?expectReg(insn.args[1], "ZEXT")
  if not srcEnc.isLane:
    return "Source for ZEXT must be a byte-lane register".err
  let value = decodeReg(vm, srcEnc)
  writeReg(vm, dstEnc, Word(value))
  debug fmt"ZEXT r{dstEnc.index}, r{srcEnc.index}(lane) = 0x{value:02X}"

proc handleSextRegReg(
    vm: var Vm, insn: DecodedInstruction
): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "SEXT")
  let srcEnc = ?expectReg(insn.args[1], "SEXT")
  if not srcEnc.isLane:
    return "Source for SEXT must be a byte-lane register".err
  let value = cast[int8](decodeReg(vm, srcEnc).uint8).Word
  writeReg(vm, dstEnc, value)
  debug fmt"SEXT r{dstEnc.index}, r{srcEnc.index}(lane) = 0x{value:02X}"

# Arithmetic

proc handleAdd(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "ADD")
  let srcEnc = ?expectReg(insn.args[1], "ADD")
  if dstEnc.laneTag != srcEnc.laneTag:
    return "ADD operand width mismatch".err
  let (value, carry) =
    addCore(decodeReg(vm, dstEnc), decodeReg(vm, srcEnc), dstEnc.isLane)
  setArithFlags(vm, value, carry, dstEnc.isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"ADD r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"

proc handleAddImm(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "ADDI")
  let imm = ?expectImm(insn.args[1], "ADDI")
  let (value, carry) = addCore(decodeReg(vm, dstEnc), imm, dstEnc.isLane)
  setArithFlags(vm, value, carry, dstEnc.isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"ADDI r{dstEnc.index}, 0x{imm:X} = 0x{value:04X}"

proc handleSub(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "SUB")
  let srcEnc = ?expectReg(insn.args[1], "SUB")
  if dstEnc.laneTag != srcEnc.laneTag:
    return "SUB operand width mismatch".err
  let (value, carry) =
    subCore(decodeReg(vm, dstEnc), decodeReg(vm, srcEnc), dstEnc.isLane)
  setArithFlags(vm, value, carry, dstEnc.isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"SUB r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"

proc handleSubImm(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "SUBI")
  let imm = ?expectImm(insn.args[1], "SUBI")
  let (value, carry) = subCore(decodeReg(vm, dstEnc), imm, dstEnc.isLane)
  setArithFlags(vm, value, carry, dstEnc.isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"SUBI r{dstEnc.index}, 0x{imm:X} = 0x{value:04X}"

proc handleAnd(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "AND")
  let srcEnc = ?expectReg(insn.args[1], "AND")
  if dstEnc.laneTag != srcEnc.laneTag:
    return "AND operand width mismatch".err
  let value = decodeReg(vm, dstEnc) and decodeReg(vm, srcEnc)
  setArithFlags(vm, value, false, dstEnc.isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"AND r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"

proc handleOr(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "OR")
  let srcEnc = ?expectReg(insn.args[1], "OR")
  if dstEnc.laneTag != srcEnc.laneTag:
    return "OR operand width mismatch".err
  let value = decodeReg(vm, dstEnc) or decodeReg(vm, srcEnc)
  setArithFlags(vm, value, false, dstEnc.isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"OR r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"

proc handleXor(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "XOR")
  let srcEnc = ?expectReg(insn.args[1], "XOR")
  if dstEnc.laneTag != srcEnc.laneTag:
    return "XOR operand width mismatch".err
  let value = decodeReg(vm, dstEnc) xor decodeReg(vm, srcEnc)
  setArithFlags(vm, value, false, dstEnc.isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"XOR r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"

proc handleNot(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let enc = ?expectReg(insn.args[0], "NOT")
  let isLane = enc.isLane
  let mask = if isLane: ByteMask else: 0xFFFF'u16
  let value = (not decodeReg(vm, enc)) and mask
  setArithFlags(vm, value, false, isLane)
  writeReg(vm, enc, value)
  debug fmt"NOT r{enc.index} = 0x{value:04X}"

proc handleCmp(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "CMP")
  let srcEnc = ?expectReg(insn.args[1], "CMP")
  if dstEnc.laneTag != srcEnc.laneTag:
    return "CMP operand width mismatch".err
  let (value, carry) =
    subCore(decodeReg(vm, dstEnc), decodeReg(vm, srcEnc), dstEnc.isLane)
  setArithFlags(vm, value, carry, dstEnc.isLane)
  debug fmt"CMP r{dstEnc.index}, r{srcEnc.index} flags Z={vm.flags.zero} N={vm.flags.negative} C={vm.flags.carry}"

proc handleCmpImm(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "CMPI")
  let imm = ?expectImm(insn.args[1], "CMPI")
  let (value, carry) = subCore(decodeReg(vm, dstEnc), imm, dstEnc.isLane)
  setArithFlags(vm, value, carry, dstEnc.isLane)
  debug fmt"CMPI r{dstEnc.index}, 0x{imm:X} flags Z={vm.flags.zero} N={vm.flags.negative} C={vm.flags.carry}"

# Jumps and subroutines

proc pushWord(vm: var Vm, value: Word): FvmResult[void] =
  if vm.sp <= 1'u16:
    return "Stack overflow on CALL".err
  vm.sp -= 2
  ?vm.bus.write16(vm.sp, value)
  ok()

proc popWord(vm: var Vm): FvmResult[Word] =
  if int(vm.sp) + 1 >= int(StackBase):
    return "Stack underflow on RET".err
  let value = ?vm.bus.read16(vm.sp)
  vm.sp += 2
  value.ok

proc handleJmp(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let target = Address(?expectImm16(insn.args[0], "JMP"))
  debug fmt"JMP 0x{target:04X}"
  vm.ip = target

proc handleJmpReg(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let enc = ?expectReg(insn.args[0], "JMP")
  let target = decodeReg(vm, enc)
  debug fmt"JMP r{enc.index} -> 0x{target:04X}"
  vm.ip = target

template conditionalJump(vm: var Vm, condition: bool, target: Address, name: string) =
  if condition:
    debug name & fmt" taken -> 0x{target:04X}"
    vm.ip = target
  else:
    debug name & " not taken"

proc handleJz(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let target = Address(?expectImm16(insn.args[0], "JZ"))
  conditionalJump(vm, vm.flags.zero, target, "JZ")

proc handleJzReg(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let enc = ?expectReg(insn.args[0], "JZ")
  let target = decodeReg(vm, enc)
  conditionalJump(vm, vm.flags.zero, target, "JZ")

proc handleJnz(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let target = Address(?expectImm16(insn.args[0], "JNZ"))
  conditionalJump(vm, not vm.flags.zero, target, "JNZ")

proc handleJnzReg(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let enc = ?expectReg(insn.args[0], "JNZ")
  let target = decodeReg(vm, enc)
  conditionalJump(vm, not vm.flags.zero, target, "JNZ")

proc handleJc(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let target = Address(?expectImm16(insn.args[0], "JC"))
  conditionalJump(vm, vm.flags.carry, target, "JC")

proc handleJcReg(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let enc = ?expectReg(insn.args[0], "JC")
  let target = decodeReg(vm, enc)
  conditionalJump(vm, vm.flags.carry, target, "JC")

proc handleJn(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let target = Address(?expectImm16(insn.args[0], "JN"))
  conditionalJump(vm, vm.flags.negative, target, "JN")

proc handleJnReg(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let enc = ?expectReg(insn.args[0], "JN")
  let target = decodeReg(vm, enc)
  conditionalJump(vm, vm.flags.negative, target, "JN")

proc handleCall(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let target = Address(?expectImm16(insn.args[0], "CALL"))
  let retAddr = vm.ip + Address(insn.size)
  ?pushWord(vm, retAddr)
  debug fmt"CALL 0x{target:04X} (ret=0x{retAddr:04X})"
  vm.ip = target

proc handleCallReg(
    vm: var Vm, insn: DecodedInstruction
): FvmResult[void] {.defaultOk.} =
  let enc = ?expectReg(insn.args[0], "CALL")
  let target = decodeReg(vm, enc)
  let retAddr = vm.ip + Address(insn.size)
  ?pushWord(vm, retAddr)
  debug fmt"CALL r{enc.index} -> 0x{target:04X} (ret=0x{retAddr:04X})"
  vm.ip = target

proc handleRet(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let target = ?popWord(vm)
  debug fmt"RET -> 0x{target:04X}"
  vm.ip = target

proc handleSieRegImm(
    vm: var Vm, insn: DecodedInstruction
): FvmResult[void] {.defaultOk.} =
  ?requirePrivileged(vm, "SIE")
  let indexEnc = ?expectReg(insn.args[0], "SIE")
  let index = ?interruptIndex(decodeReg(vm, indexEnc))
  let target = Address(?expectImm16(insn.args[1], "SIE"))
  vm.ivt[index] = target
  debug fmt"SIE r{indexEnc.index}, 0x{target:04X}"

proc handleSieRegReg(
    vm: var Vm, insn: DecodedInstruction
): FvmResult[void] {.defaultOk.} =
  ?requirePrivileged(vm, "SIE")
  let indexEnc = ?expectReg(insn.args[0], "SIE")
  let targetEnc = ?expectReg(insn.args[1], "SIE")
  let index = ?interruptIndex(decodeReg(vm, indexEnc))
  let target = Address(decodeReg(vm, targetEnc))
  vm.ivt[index] = target
  debug fmt"SIE r{indexEnc.index}, r{targetEnc.index} -> 0x{target:04X}"

proc handleIntImm(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let index = ?interruptIndex(Word(?expectImm8(insn.args[0], "INT")))
  let resumeIp = vm.ip + Address(insn.size)
  vm.ip = resumeIp
  ?vm.raiseInterrupt(index)
  debug fmt"INT 0x{index:02X}"

proc handleIntReg(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let enc = ?expectReg(insn.args[0], "INT")
  let index = ?interruptIndex(decodeReg(vm, enc))
  let resumeIp = vm.ip + Address(insn.size)
  vm.ip = resumeIp
  ?vm.raiseInterrupt(index)
  debug fmt"INT r{enc.index} -> 0x{index:02X}"

proc handleOut(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let port = ?expectImm8(insn.args[0], "OUT")
  let enc = ?expectReg(insn.args[1], "OUT")
  let value = decodeReg(vm, enc)
  if enc.isLane:
    ?vm.ports.portOut(port, Byte(value))
    debug fmt"OUT port={port} r{enc.index}(lane) = 0x{value:02X}"
  else:
    ?vm.ports.portOut(port, Byte((value shr 8) and ByteMask))
    ?vm.ports.portOut(port, Byte(value and ByteMask))
    debug fmt"OUT port={port} r{enc.index} = 0x{value:04X}"

proc handleIn(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let enc = ?expectReg(insn.args[0], "IN")
  let port = ?expectImm8(insn.args[1], "IN")
  if enc.isLane:
    let value = ?vm.ports.portIn(port)
    writeReg(vm, enc, Word(value))
    debug fmt"IN r{enc.index}(lane) = 0x{value:02X} from port={port}"
  else:
    let hi = ?vm.ports.portIn(port)
    let lo = ?vm.ports.portIn(port)
    let value = (Word(hi) shl 8) or Word(lo)
    writeReg(vm, enc, value)
    debug fmt"IN r{enc.index} = 0x{value:04X} from port={port}"

# Memory access

proc handleLoad(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let dstEnc = ?expectReg(insn.args[0], "LOAD")
  let addrEnc = ?expectReg(insn.args[1], "LOAD")
  if addrEnc.isLane:
    return "LOAD address register must be full-width".err
  let address = Address(decodeReg(vm, addrEnc))
  if dstEnc.isLane:
    let value = ?vm.bus.read8(address)
    writeReg(vm, dstEnc, Word(value))
    debug fmt"LOAD r{dstEnc.index}(lane) = 0x{value:02X} from 0x{address:04X}"
  else:
    let value = ?vm.bus.read16(address)
    writeReg(vm, dstEnc, value)
    debug fmt"LOAD r{dstEnc.index} = 0x{value:04X} from 0x{address:04X}"

proc handleStore(vm: var Vm, insn: DecodedInstruction): FvmResult[void] {.defaultOk.} =
  let addrEnc = ?expectReg(insn.args[0], "STORE")
  let srcEnc = ?expectReg(insn.args[1], "STORE")
  if addrEnc.isLane:
    return "STORE address register must be full-width".err
  let address = Address(decodeReg(vm, addrEnc))
  if srcEnc.isLane:
    let value = Byte(decodeReg(vm, srcEnc))
    ?vm.bus.write8(address, value)
    debug fmt"STORE 0x{address:04X} = 0x{value:02X} (byte)"
  else:
    let value = decodeReg(vm, srcEnc)
    ?vm.bus.write16(address, value)
    debug fmt"STORE 0x{address:04X} = 0x{value:04X} (word)"

# Reserved / unimplemented placeholders

proc handleUnimplemented(vm: var Vm, insn: DecodedInstruction): FvmResult[void] =
  ("Unimplemented opcode 0x" & toHex(int(vm.bus.mem[int(vm.ip)]), 2)).err

proc execute*(vm: var Vm, insn: DecodedInstruction): FvmResult[void] =
  let def = getInstructionDef(insn.opcode)
  if def.handler == nil:
    return ("No handler for opcode 0x" & toHex(ord(insn.opcode), 2)).err

  let startIp = vm.ip
  ?def.handler(vm, insn)
  if vm.ip == startIp:
    vm.ip += Address(insn.size)
  ok()

# Instruction definition table

constArray[OpCode, InstructionDef](instructions):
  result[OpCode.Nop] = InstructionDef(mnemonic: "NOP", handler: handleNop)
  result[OpCode.Halt] = InstructionDef(mnemonic: "HALT", handler: handleHalt)
  result[OpCode.Iret] = InstructionDef(mnemonic: "IRET", handler: handleIret)
  result[OpCode.Dpl] = InstructionDef(mnemonic: "DPL", handler: handleDpl)
  result[OpCode.Push] = InstructionDef(mnemonic: "PUSH", handler: handlePush)
  result[OpCode.Pop] = InstructionDef(mnemonic: "POP", handler: handlePop)
  result[OpCode.MovRegImm] = InstructionDef(mnemonic: "MOV", handler: handleMovRegImm)
  result[OpCode.MovRegReg] = InstructionDef(mnemonic: "MOV", handler: handleMovRegReg)
  result[OpCode.ZeroExtend] =
    InstructionDef(mnemonic: "ZEXT", handler: handleZextRegReg)
  result[OpCode.SignExtend] =
    InstructionDef(mnemonic: "SEXT", handler: handleSextRegReg)
  result[OpCode.Add] = InstructionDef(mnemonic: "ADD", handler: handleAdd)
  result[OpCode.Sub] = InstructionDef(mnemonic: "SUB", handler: handleSub)
  result[OpCode.AddImm] = InstructionDef(mnemonic: "ADDI", handler: handleAddImm)
  result[OpCode.SubImm] = InstructionDef(mnemonic: "SUBI", handler: handleSubImm)
  result[OpCode.And] = InstructionDef(mnemonic: "AND", handler: handleAnd)
  result[OpCode.Or] = InstructionDef(mnemonic: "OR", handler: handleOr)
  result[OpCode.Xor] = InstructionDef(mnemonic: "XOR", handler: handleXor)
  result[OpCode.Not] = InstructionDef(mnemonic: "NOT", handler: handleNot)
  result[OpCode.Cmp] = InstructionDef(mnemonic: "CMP", handler: handleCmp)
  result[OpCode.CmpImm] = InstructionDef(mnemonic: "CMPI", handler: handleCmpImm)
  result[OpCode.Jmp] = InstructionDef(mnemonic: "JMP", handler: handleJmp)
  result[OpCode.JmpReg] = InstructionDef(mnemonic: "JMP", handler: handleJmpReg)
  result[OpCode.Jz] = InstructionDef(mnemonic: "JZ", handler: handleJz)
  result[OpCode.JzReg] = InstructionDef(mnemonic: "JZ", handler: handleJzReg)
  result[OpCode.Jnz] = InstructionDef(mnemonic: "JNZ", handler: handleJnz)
  result[OpCode.JnzReg] = InstructionDef(mnemonic: "JNZ", handler: handleJnzReg)
  result[OpCode.Jc] = InstructionDef(mnemonic: "JC", handler: handleJc)
  result[OpCode.JcReg] = InstructionDef(mnemonic: "JC", handler: handleJcReg)
  result[OpCode.Jn] = InstructionDef(mnemonic: "JN", handler: handleJn)
  result[OpCode.JnReg] = InstructionDef(mnemonic: "JN", handler: handleJnReg)
  result[OpCode.Call] = InstructionDef(mnemonic: "CALL", handler: handleCall)
  result[OpCode.CallReg] = InstructionDef(mnemonic: "CALL", handler: handleCallReg)
  result[OpCode.Ret] = InstructionDef(mnemonic: "RET", handler: handleRet)
  result[OpCode.SieRegImm] = InstructionDef(mnemonic: "SIE", handler: handleSieRegImm)
  result[OpCode.SieRegReg] = InstructionDef(mnemonic: "SIE", handler: handleSieRegReg)
  result[OpCode.IntImm] = InstructionDef(mnemonic: "INT", handler: handleIntImm)
  result[OpCode.IntReg] = InstructionDef(mnemonic: "INT", handler: handleIntReg)
  result[OpCode.In] = InstructionDef(mnemonic: "IN", handler: handleIn)
  result[OpCode.Out] = InstructionDef(mnemonic: "OUT", handler: handleOut)
  result[OpCode.Load] = InstructionDef(mnemonic: "LOAD", handler: handleLoad)
  result[OpCode.Store] = InstructionDef(mnemonic: "STORE", handler: handleStore)

proc getInstructionDef*(opcode: OpCode): InstructionDef =
  instructions[opcode]
