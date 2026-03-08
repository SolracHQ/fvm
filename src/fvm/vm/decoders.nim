import ./types
import ../core/types as coretypes
import ../core/constants
import ../core/registers
import ../utils

export types, coretypes

proc checkBounds(vm: Vm, needed: int): FvmResult[void] =
  if int(vm.ip) + needed > VmMemorySize:
    return "Instruction out of bounds".err
  ok()

proc checkReg(enc: RegEncoding): FvmResult[void] =
  if enc.isSp:
    return ok()
  if enc.index >= GeneralRegisterCount:
    return ("Register index out of range: r" & $enc.index).err
  ok()

proc readReg(vm: Vm, offset: int): FvmResult[FlatArg] =
  let enc = RegEncoding(?vm.bus.read8(Address(int(vm.ip) + offset)))
  ?checkReg(enc)
  FlatArg(kind: faReg, enc: enc).ok

proc readImm8(vm: Vm, offset: int): FvmResult[FlatArg] =
  FlatArg(kind: faImm8, imm8: ?vm.bus.read8(Address(int(vm.ip) + offset))).ok

proc readImm16(vm: Vm, offset: int): FvmResult[FlatArg] =
  FlatArg(kind: faImm16, imm16: ?vm.bus.read16(Address(int(vm.ip) + offset))).ok

proc decodeNoArgs(vm: Vm, opcode: OpCode): FvmResult[DecodedInstruction] =
  ?checkBounds(vm, 1)
  DecodedInstruction(opcode: opcode, argCount: 0, size: 1).ok

proc decodeUnaryReg(vm: Vm, opcode: OpCode): FvmResult[DecodedInstruction] =
  ?checkBounds(vm, 2)
  var insn = DecodedInstruction(opcode: opcode, argCount: 1, size: 2)
  insn.args[0] = ?readReg(vm, 1)
  insn.ok

proc decodeUnaryAddr(vm: Vm, opcode: OpCode): FvmResult[DecodedInstruction] =
  ?checkBounds(vm, 3)
  var insn = DecodedInstruction(opcode: opcode, argCount: 1, size: 3)
  insn.args[0] = ?readImm16(vm, 1)
  insn.ok

proc decodeBinaryRegs(vm: Vm, opcode: OpCode): FvmResult[DecodedInstruction] =
  ?checkBounds(vm, 3)
  var insn = DecodedInstruction(opcode: opcode, argCount: 2, size: 3)
  insn.args[0] = ?readReg(vm, 1)
  insn.args[1] = ?readReg(vm, 2)
  insn.ok

proc decodeRegImm(vm: Vm, opcode: OpCode): FvmResult[DecodedInstruction] =
  ?checkBounds(vm, 2)
  var insn = DecodedInstruction(opcode: opcode, argCount: 2)
  insn.args[0] = ?readReg(vm, 1)
  if insn.args[0].enc.isLane:
    ?checkBounds(vm, 3)
    insn.args[1] = ?readImm8(vm, 2)
    insn.size = 3
  else:
    ?checkBounds(vm, 4)
    insn.args[1] = ?readImm16(vm, 2)
    insn.size = 4
  insn.ok

proc decodePortReg(vm: Vm, opcode: OpCode): FvmResult[DecodedInstruction] =
  ?checkBounds(vm, 3)
  var insn = DecodedInstruction(opcode: opcode, argCount: 2, size: 3)
  insn.args[0] = ?readImm8(vm, 1)
  insn.args[1] = ?readReg(vm, 2)
  insn.ok

proc decodeRegPort(vm: Vm, opcode: OpCode): FvmResult[DecodedInstruction] =
  ?checkBounds(vm, 3)
  var insn = DecodedInstruction(opcode: opcode, argCount: 2, size: 3)
  insn.args[0] = ?readReg(vm, 1)
  insn.args[1] = ?readImm8(vm, 2)
  insn.ok

constArray[OpCode, DecoderProc](decoders):
  result[OpCode.Nop] = decodeNoArgs
  result[OpCode.Halt] = decodeNoArgs
  result[OpCode.Ret] = decodeNoArgs

  result[OpCode.Push] = decodeUnaryReg
  result[OpCode.Pop] = decodeUnaryReg
  result[OpCode.Not] = decodeUnaryReg
  result[OpCode.JmpReg] = decodeUnaryReg
  result[OpCode.JzReg] = decodeUnaryReg
  result[OpCode.JnzReg] = decodeUnaryReg
  result[OpCode.JcReg] = decodeUnaryReg
  result[OpCode.JnReg] = decodeUnaryReg
  result[OpCode.CallReg] = decodeUnaryReg

  result[OpCode.Jmp] = decodeUnaryAddr
  result[OpCode.Jz] = decodeUnaryAddr
  result[OpCode.Jnz] = decodeUnaryAddr
  result[OpCode.Jc] = decodeUnaryAddr
  result[OpCode.Jn] = decodeUnaryAddr
  result[OpCode.Call] = decodeUnaryAddr

  result[OpCode.MovRegReg] = decodeBinaryRegs
  result[OpCode.ZeroExtend] = decodeBinaryRegs
  result[OpCode.SignExtend] = decodeBinaryRegs
  result[OpCode.Add] = decodeBinaryRegs
  result[OpCode.Sub] = decodeBinaryRegs
  result[OpCode.And] = decodeBinaryRegs
  result[OpCode.Or] = decodeBinaryRegs
  result[OpCode.Xor] = decodeBinaryRegs
  result[OpCode.Cmp] = decodeBinaryRegs
  result[OpCode.Load] = decodeBinaryRegs
  result[OpCode.Store] = decodeBinaryRegs

  result[OpCode.MovRegImm] = decodeRegImm
  result[OpCode.AddImm] = decodeRegImm
  result[OpCode.SubImm] = decodeRegImm
  result[OpCode.CmpImm] = decodeRegImm

  result[OpCode.In] = decodeRegPort
  result[OpCode.Out] = decodePortReg

proc decode*(vm: Vm, opcode: OpCode): FvmResult[DecodedInstruction] =
  let decoder = decoders[opcode]
  if decoder == nil:
    return ("No decoder for opcode 0x" & $ord(opcode)).err
  decoder(vm, opcode)
