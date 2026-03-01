## Instruction handler procs and the InstructionDef dispatch table.

import std/logging
import std/strformat
import std/strutils

import ./vmstate
import ../types/opcodes
import ../utils

export vmstate, opcodes

# Types

type
  HandlerProc* = proc(vm: var Vm): FvmResult[void]

  InstructionDef* = object
    mnemonic*: string
    handler*: HandlerProc

# Shared helpers

proc checkBounds(vm: Vm, needed: int): FvmResult[void] =
  result = ok()
  if int(vm.ip) + needed > VmMemorySize:
    result = "Instruction out of bounds".err

proc checkReg(enc: RegEncoding): FvmResult[void] =
  if enc.isSp:
    return ok()
  if enc.index >= GeneralRegisterCount:
    return ("Register index out of range: r" & $enc.index).err
  ok()

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

proc readRegEnc(vm: Vm, offset: int): FvmResult[RegEncoding] =
  let enc = RegEncoding(?vm.bus.read8(Address(int(vm.ip) + offset)))
  ?checkReg(enc)
  enc.ok

proc readUnaryReg(vm: Vm): FvmResult[RegEncoding] =
  ?checkBounds(vm, 2)
  readRegEnc(vm, 1)

proc readBinaryRegs(vm: Vm): FvmResult[tuple[dst, src: RegEncoding]] =
  ?checkBounds(vm, 3)
  let dstEnc = ?readRegEnc(vm, 1)
  let srcEnc = ?readRegEnc(vm, 2)
  (dst: dstEnc, src: srcEnc).ok

proc setArithFlags(vm: var Vm, value: Word, carry: bool, isLane: bool) =
  let highBit = if isLane: 7'u16 else: 15'u16
  vm.flags.zero = value == 0
  vm.flags.negative = (value shr highBit) != 0
  vm.flags.carry = carry

# Control

proc handleNop(vm: var Vm): FvmResult[void] {.defaultOk.} =
  debug "NOP"
  vm.ip += 1

proc handleHalt(vm: var Vm): FvmResult[void] {.defaultOk.} =
  debug "HALT"
  vm.halted = true

# Stack

proc handlePush(vm: var Vm): FvmResult[void] {.defaultOk.} =
  ?checkBounds(vm, 2)
  let enc = RegEncoding(?vm.bus.read8(Address(int(vm.ip) + 1)))
  ?checkReg(enc)
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
  vm.ip += 2

proc handlePop(vm: var Vm): FvmResult[void] {.defaultOk.} =
  ?checkBounds(vm, 2)
  let enc = RegEncoding(?vm.bus.read8(Address(int(vm.ip) + 1)))
  ?checkReg(enc)
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
  vm.ip += 2

# Data movement

proc handleMovRegImm(vm: var Vm): FvmResult[void] {.defaultOk.} =
  ?checkBounds(vm, 2)
  let dstEnc = RegEncoding(?vm.bus.read8(Address(int(vm.ip) + 1)))
  ?checkReg(dstEnc)
  if dstEnc.isLane:
    ?checkBounds(vm, 3)
    let imm = ?vm.bus.read8(Address(int(vm.ip) + 2))
    writeReg(vm, dstEnc, Word(imm))
    debug fmt"MOV r{dstEnc.index}(lane), 0x{imm:02X}"
    vm.ip += 3
  else:
    ?checkBounds(vm, 4)
    let imm = ?vm.bus.read16(Address(int(vm.ip) + 2))
    writeReg(vm, dstEnc, imm)
    debug fmt"MOV r{dstEnc.index}, 0x{imm:04X}"
    vm.ip += 4

proc handleMovRegReg(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let (dstEnc, srcEnc) = ?readBinaryRegs(vm)
  let value = decodeReg(vm, srcEnc)
  writeReg(vm, dstEnc, value)
  debug fmt"MOV r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"
  vm.ip += 3

proc handleZextRegReg(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let (dstEnc, srcEnc) = ?readBinaryRegs(vm)
  if not srcEnc.isLane:
    return "Source for ZEXT must be a byte-lane register".err
  let value = decodeReg(vm, srcEnc)
  writeReg(vm, dstEnc, Word(value))
  debug fmt"ZEXT r{dstEnc.index}, r{srcEnc.index}(lane) = 0x{value:02X}"
  vm.ip += 3

proc handleSextRegReg(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let (dstEnc, srcEnc) = ?readBinaryRegs(vm)
  if not srcEnc.isLane:
    return "Source for SEXT must be a byte-lane register".err
  let value = cast[int8](decodeReg(vm, srcEnc).uint8).Word
  writeReg(vm, dstEnc, value)
  debug fmt"SEXT r{dstEnc.index}, r{srcEnc.index}(lane) = 0x{value:02X}"
  vm.ip += 3

# Arithmetic

proc handleAdd(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let (dstEnc, srcEnc) = ?readBinaryRegs(vm)
  if dstEnc.laneTag != srcEnc.laneTag:
    return "ADD operand width mismatch".err
  let isLane = dstEnc.isLane
  let mask =
    if isLane:
      uint32(ByteMask)
    else:
      0xFFFF'u32
  let raw = uint32(decodeReg(vm, dstEnc)) + uint32(decodeReg(vm, srcEnc))
  let carry = raw > mask
  let value = Word(raw and mask)
  setArithFlags(vm, value, carry, isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"ADD r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"
  vm.ip += 3

proc handleSub(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let (dstEnc, srcEnc) = ?readBinaryRegs(vm)
  if dstEnc.laneTag != srcEnc.laneTag:
    return "SUB operand width mismatch".err
  let isLane = dstEnc.isLane
  let mask =
    if isLane:
      uint32(ByteMask)
    else:
      0xFFFF'u32
  let dstVal = uint32(decodeReg(vm, dstEnc))
  let srcVal = uint32(decodeReg(vm, srcEnc))
  let carry = srcVal > dstVal
  let value = Word((dstVal - srcVal) and mask)
  setArithFlags(vm, value, carry, isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"SUB r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"
  vm.ip += 3

proc handleAnd(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let (dstEnc, srcEnc) = ?readBinaryRegs(vm)
  if dstEnc.laneTag != srcEnc.laneTag:
    return "AND operand width mismatch".err
  let value = decodeReg(vm, dstEnc) and decodeReg(vm, srcEnc)
  setArithFlags(vm, value, false, dstEnc.isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"AND r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"
  vm.ip += 3

proc handleOr(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let (dstEnc, srcEnc) = ?readBinaryRegs(vm)
  if dstEnc.laneTag != srcEnc.laneTag:
    return "OR operand width mismatch".err
  let value = decodeReg(vm, dstEnc) or decodeReg(vm, srcEnc)
  setArithFlags(vm, value, false, dstEnc.isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"OR r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"
  vm.ip += 3

proc handleXor(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let (dstEnc, srcEnc) = ?readBinaryRegs(vm)
  if dstEnc.laneTag != srcEnc.laneTag:
    return "XOR operand width mismatch".err
  let value = decodeReg(vm, dstEnc) xor decodeReg(vm, srcEnc)
  setArithFlags(vm, value, false, dstEnc.isLane)
  writeReg(vm, dstEnc, value)
  debug fmt"XOR r{dstEnc.index}, r{srcEnc.index} = 0x{value:04X}"
  vm.ip += 3

proc handleNot(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let enc = ?readUnaryReg(vm)
  let isLane = enc.isLane
  let mask = if isLane: ByteMask else: 0xFFFF'u16
  let value = (not decodeReg(vm, enc)) and mask
  setArithFlags(vm, value, false, isLane)
  writeReg(vm, enc, value)
  debug fmt"NOT r{enc.index} = 0x{value:04X}"
  vm.ip += 2

proc handleCmp(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let (dstEnc, srcEnc) = ?readBinaryRegs(vm)
  if dstEnc.laneTag != srcEnc.laneTag:
    return "CMP operand width mismatch".err
  let isLane = dstEnc.isLane
  let mask =
    if isLane:
      uint32(ByteMask)
    else:
      0xFFFF'u32
  let dstVal = uint32(decodeReg(vm, dstEnc))
  let srcVal = uint32(decodeReg(vm, srcEnc))
  let carry = srcVal > dstVal
  let value = Word((dstVal - srcVal) and mask)
  setArithFlags(vm, value, carry, isLane)
  debug fmt"CMP r{dstEnc.index}, r{srcEnc.index} flags Z={vm.flags.zero} N={vm.flags.negative} C={vm.flags.carry}"
  vm.ip += 3

# Jumps and subroutines

proc readAddr16(vm: Vm, offset: int): FvmResult[Address] =
  let hi = ?vm.bus.read8(Address(int(vm.ip) + offset))
  let lo = ?vm.bus.read8(Address(int(vm.ip) + offset + 1))
  Address((Word(hi) shl 8) or Word(lo)).ok

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

proc handleJmp(vm: var Vm): FvmResult[void] {.defaultOk.} =
  ?checkBounds(vm, 3)
  let target = ?readAddr16(vm, 1)
  debug fmt"JMP 0x{target:04X}"
  vm.ip = target

proc handleJmpReg(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let enc = ?readUnaryReg(vm)
  let target = decodeReg(vm, enc)
  debug fmt"JMP r{enc.index} -> 0x{target:04X}"
  vm.ip = target

template conditionalJump(
    vm: var Vm, condition: bool, immSize: int, target: Address, name: string
) =
  if condition:
    debug name & fmt" taken -> 0x{target:04X}"
    vm.ip = target
  else:
    debug name & " not taken"
    vm.ip += Address(immSize)

proc handleJz(vm: var Vm): FvmResult[void] {.defaultOk.} =
  ?checkBounds(vm, 3)
  let target = ?readAddr16(vm, 1)
  conditionalJump(vm, vm.flags.zero, 3, target, "JZ")

proc handleJzReg(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let enc = ?readUnaryReg(vm)
  let target = decodeReg(vm, enc)
  conditionalJump(vm, vm.flags.zero, 2, target, "JZ")

proc handleJnz(vm: var Vm): FvmResult[void] {.defaultOk.} =
  ?checkBounds(vm, 3)
  let target = ?readAddr16(vm, 1)
  conditionalJump(vm, not vm.flags.zero, 3, target, "JNZ")

proc handleJnzReg(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let enc = ?readUnaryReg(vm)
  let target = decodeReg(vm, enc)
  conditionalJump(vm, not vm.flags.zero, 2, target, "JNZ")

proc handleJc(vm: var Vm): FvmResult[void] {.defaultOk.} =
  ?checkBounds(vm, 3)
  let target = ?readAddr16(vm, 1)
  conditionalJump(vm, vm.flags.carry, 3, target, "JC")

proc handleJcReg(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let enc = ?readUnaryReg(vm)
  let target = decodeReg(vm, enc)
  conditionalJump(vm, vm.flags.carry, 2, target, "JC")

proc handleJn(vm: var Vm): FvmResult[void] {.defaultOk.} =
  ?checkBounds(vm, 3)
  let target = ?readAddr16(vm, 1)
  conditionalJump(vm, vm.flags.negative, 3, target, "JN")

proc handleJnReg(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let enc = ?readUnaryReg(vm)
  let target = decodeReg(vm, enc)
  conditionalJump(vm, vm.flags.negative, 2, target, "JN")

proc handleCall(vm: var Vm): FvmResult[void] {.defaultOk.} =
  ?checkBounds(vm, 3)
  let target = ?readAddr16(vm, 1)
  let retAddr = vm.ip + 3
  ?pushWord(vm, retAddr)
  debug fmt"CALL 0x{target:04X} (ret=0x{retAddr:04X})"
  vm.ip = target

proc handleCallReg(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let enc = ?readUnaryReg(vm)
  let target = decodeReg(vm, enc)
  let retAddr = vm.ip + 2
  ?pushWord(vm, retAddr)
  debug fmt"CALL r{enc.index} -> 0x{target:04X} (ret=0x{retAddr:04X})"
  vm.ip = target

proc handleRet(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let target = ?popWord(vm)
  debug fmt"RET -> 0x{target:04X}"
  vm.ip = target

proc handleOut(vm: var Vm): FvmResult[void] {.defaultOk.} =
  ## OUT port8, enc  --  encoding: [Out, port, enc]
  ?checkBounds(vm, 3)
  let port = ?vm.bus.read8(Address(int(vm.ip) + 1))
  let enc = ?readRegEnc(vm, 2)
  let value = decodeReg(vm, enc)
  if enc.isLane:
    ?vm.ports.portOut(port, Byte(value))
    debug fmt"OUT port={port} r{enc.index}(lane) = 0x{value:02X}"
  else:
    ?vm.ports.portOut(port, Byte((value shr 8) and ByteMask))
    ?vm.ports.portOut(port, Byte(value and ByteMask))
    debug fmt"OUT port={port} r{enc.index} = 0x{value:04X}"
  vm.ip += 3

proc handleIn(vm: var Vm): FvmResult[void] {.defaultOk.} =
  ## IN enc, port8  --  encoding: [In, enc, port]
  ?checkBounds(vm, 3)
  let enc = ?readRegEnc(vm, 1)
  let port = ?vm.bus.read8(Address(int(vm.ip) + 2))
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
  vm.ip += 3

# Memory access

proc handleLoad(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let regs = ?readBinaryRegs(vm)
  let dstEnc = regs.dst
  let addrEnc = regs.src
  if addrEnc.isLane or addrEnc.isSp:
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
  vm.ip += 3

proc handleStore(vm: var Vm): FvmResult[void] {.defaultOk.} =
  let regs = ?readBinaryRegs(vm)
  let addrEnc = regs.dst
  let srcEnc = regs.src
  if addrEnc.isLane or addrEnc.isSp:
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
  vm.ip += 3

# Reserved / unimplemented placeholders

proc handleUnimplemented(vm: var Vm): FvmResult[void] =
  ("Unimplemented opcode 0x" & toHex(int(vm.bus.mem[int(vm.ip)]), 2)).err

# Instruction definition table

constArray[OpCode, InstructionDef](instructions):
  result[OpCode.Nop] = InstructionDef(mnemonic: "NOP", handler: handleNop)
  result[OpCode.Halt] = InstructionDef(mnemonic: "HALT", handler: handleHalt)
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
  result[OpCode.And] = InstructionDef(mnemonic: "AND", handler: handleAnd)
  result[OpCode.Or] = InstructionDef(mnemonic: "OR", handler: handleOr)
  result[OpCode.Xor] = InstructionDef(mnemonic: "XOR", handler: handleXor)
  result[OpCode.Not] = InstructionDef(mnemonic: "NOT", handler: handleNot)
  result[OpCode.Cmp] = InstructionDef(mnemonic: "CMP", handler: handleCmp)
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
  result[OpCode.In] = InstructionDef(mnemonic: "IN", handler: handleIn)
  result[OpCode.Out] = InstructionDef(mnemonic: "OUT", handler: handleOut)
  result[OpCode.Load] = InstructionDef(mnemonic: "LOAD", handler: handleLoad)
  result[OpCode.Store] = InstructionDef(mnemonic: "STORE", handler: handleStore)

proc getInstructionDef*(opcode: OpCode): InstructionDef =
  ## Returns the InstructionDef for a given opcode.  Used by core.step.
  instructions[opcode]
