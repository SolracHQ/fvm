## Instruction parser dispatch.
##
## Owns the Assembler cursor type and every parse<Name>Instruction proc.
## Keyed by upper-case mnemonic in parserTable.

import std/tables

import ../../types/core
import ../../types/opcodes
import ../../types/errors
import ../lexer
import ../instructions
import ./passes

export instructions

# Assembler cursor

type
  Assembler* = object
    tokens*: seq[AsmToken]
    current*: int
    labelMap*: Table[string, Address]
    currentGlobal*: string
    currentSection*: Section

  ParseInstructionProc* =
    proc(assembler: var Assembler, line: int): FvmResult[Instruction]

proc hasMore*(a: Assembler): bool =
  a.current < a.tokens.len

proc peek*(a: Assembler): AsmToken =
  a.tokens[a.current]

proc advance*(a: var Assembler): AsmToken =
  result = a.tokens[a.current]
  inc a.current

# Generic parse helpers

proc expect*(
    assembler: var Assembler, kinds: set[TokenKind], msg: string
): FvmResult[AsmToken] =
  if not assembler.hasMore:
    return msg.err
  let tok = assembler.advance()
  if tok.kind notin kinds:
    return msg.err
  tok.ok

proc consumeEol*(
    assembler: var Assembler, mnemonic: string, line: int
): FvmResult[void] =
  discard
    ?assembler.expect(
      {TkEol},
      "Expected end of line after " & mnemonic & " at line " & $line &
        ", got extra token",
    )
  ok()

proc expandLabel*(name: string, currentGlobal: string): string =
  if name.len > 0 and name[0] == '.':
    currentGlobal & name
  else:
    name

proc resolveLabel*(assembler: Assembler, name: string, line: int): FvmResult[Address] =
  let expanded = expandLabel(name, assembler.currentGlobal)
  if expanded notin assembler.labelMap:
    return ("Undefined label '" & expanded & "' at line " & $line).err
  assembler.labelMap[expanded].ok

# Shared parser building blocks

proc parseNoOperandInstruction(
    assembler: var Assembler, line: int, opcode: OpCode, mnemonic: string
): FvmResult[Instruction] =
  var extraCount = 0
  while assembler.hasMore and assembler.peek().kind != TkEol:
    let tok = assembler.advance()
    case tok.kind
    of TkRegister, TkImmediate:
      inc extraCount
    of TkComma:
      return ("Unexpected ',' after " & mnemonic & " at line " & $tok.line).err
    else:
      return ("Unexpected token after " & mnemonic & " at line " & $tok.line).err
  ?consumeEol(assembler, mnemonic, line)
  if extraCount > 0:
    return (mnemonic & " takes no operands (line " & $line & ")").err
  Instruction(line: line, opcode: opcode, operands: @[]).ok

proc parseOneRegisterInstruction(
    assembler: var Assembler, line: int, opcode: OpCode, mnemonic: string
): FvmResult[Instruction] =
  let regTok =
    ?assembler.expect(
      {TkRegister}, "Expected register for " & mnemonic & " at line " & $line
    )
  ?consumeEol(assembler, mnemonic, line)
  Instruction(line: line, opcode: opcode, operands: @[Byte(regTok.regEncoding)]).ok

proc parseBinaryRegInstruction(
    assembler: var Assembler, line: int, opcode: OpCode, mnemonic: string
): FvmResult[Instruction] =
  let dstTok =
    ?assembler.expect(
      {TkRegister},
      "Expected destination register for " & mnemonic & " at line " & $line,
    )
  discard
    ?assembler.expect({TkComma}, "Expected ',' in " & mnemonic & " at line " & $line)
  let srcTok =
    ?assembler.expect(
      {TkRegister}, "Expected source register for " & mnemonic & " at line " & $line
    )
  ?consumeEol(assembler, mnemonic, line)
  let dstEnc = dstTok.regEncoding
  let srcEnc = srcTok.regEncoding
  if dstEnc.laneTag != srcEnc.laneTag:
    return (
      mnemonic & " lane mismatch at line " & $line & ": " & dstTok.regSource & " vs " &
      srcTok.regSource
    ).err
  Instruction(line: line, opcode: opcode, operands: @[Byte(dstEnc), Byte(srcEnc)]).ok

proc parseExtendInstruction(
    assembler: var Assembler, line: int, opcode: OpCode, mnemonic: string
): FvmResult[Instruction] =
  let dstTok =
    ?assembler.expect(
      {TkRegister},
      "Expected destination register for " & mnemonic & " at line " & $line,
    )
  discard
    ?assembler.expect({TkComma}, "Expected ',' in " & mnemonic & " at line " & $line)
  let srcTok =
    ?assembler.expect(
      {TkRegister}, "Expected source register for " & mnemonic & " at line " & $line
    )
  ?consumeEol(assembler, mnemonic, line)
  let dstEnc = dstTok.regEncoding
  let srcEnc = srcTok.regEncoding
  if dstEnc.isLane:
    return (
      "Destination register cannot be byte-lane for " & mnemonic & " at line " & $line
    ).err
  if srcEnc.isWord:
    return
      ("Source register must be byte-lane for " & mnemonic & " at line " & $line).err
  Instruction(line: line, opcode: opcode, operands: @[Byte(dstEnc), Byte(srcEnc)]).ok

proc parseBinaryRegOrImmInstruction(
    assembler: var Assembler, line: int, immOp: OpCode, regOp: OpCode, mnemonic: string
): FvmResult[Instruction] =
  ## Generalized parser for instructions that accept either register or immediate as source.
  ## Used by MOV, ADDI, SUBI, CMPI.
  let dstTok =
    ?assembler.expect(
      {TkRegister},
      "Expected destination register for " & mnemonic & " at line " & $line,
    )
  discard
    ?assembler.expect({TkComma}, "Expected ',' in " & mnemonic & " at line " & $line)
  let srcTok =
    ?assembler.expect(
      {TkImmediate, TkRegister, TkMnemonic},
      "Expected register, immediate, or label for " & mnemonic & " source at line " &
        $line,
    )
  ?consumeEol(assembler, mnemonic, line)

  case srcTok.kind
  of TkImmediate:
    let dstEnc = dstTok.regEncoding
    if dstEnc.isLane:
      let imm = srcTok.immValue
      if imm < 0 or imm > 0xFF:
        return (
          "Immediate out of range (0..255) for byte-lane " & mnemonic & " at line " &
          $line
        ).err
      return
        Instruction(line: line, opcode: immOp, operands: @[Byte(dstEnc), Byte(imm)]).ok
    else:
      let imm = srcTok.immValue
      if imm < 0 or imm > 0xFFFF:
        return (
          "Immediate out of range (0..65535) for " & mnemonic & " at line " & $line
        ).err
      let imm16 = Word(imm)
      return
        Instruction(
          line: line,
          opcode: immOp,
          operands:
            @[Byte(dstEnc), Byte((imm16 shr 8) and ByteMask), Byte(imm16 and ByteMask)],
        ).ok
  of TkMnemonic:
    let dstEnc = dstTok.regEncoding
    if dstEnc.isLane:
      return (
        "Cannot use label with byte-lane register for " & mnemonic & " at line " & $line
      ).err
    let lblAddr = ?assembler.resolveLabel(srcTok.mnemonic, line)
    return
      Instruction(
        line: line,
        opcode: immOp,
        operands:
          @[
            Byte(dstEnc), Byte((lblAddr shr 8) and ByteMask), Byte(lblAddr and ByteMask)
          ],
      ).ok
  else: # TkRegister
    let dstEnc = dstTok.regEncoding
    let srcEnc = srcTok.regEncoding
    if dstEnc.laneTag != srcEnc.laneTag:
      return (
        mnemonic & " lane mismatch at line " & $line & ": " & dstTok.regSource & " vs " &
        srcTok.regSource
      ).err
    return
      Instruction(line: line, opcode: regOp, operands: @[Byte(dstEnc), Byte(srcEnc)]).ok

proc parseJumpInstruction(
    assembler: var Assembler, line: int, immOp: OpCode, regOp: OpCode, mnemonic: string
): FvmResult[Instruction] =
  let tok =
    ?assembler.expect(
      {TkRegister, TkImmediate, TkMnemonic},
      "Expected register, address, or label for " & mnemonic & " at line " & $line,
    )
  ?consumeEol(assembler, mnemonic, line)
  case tok.kind
  of TkRegister:
    return Instruction(line: line, opcode: regOp, operands: @[Byte(tok.regEncoding)]).ok
  of TkImmediate:
    let immAddr = tok.immValue
    if immAddr < 0 or immAddr > 0xFFFF:
      return ("Address out of range for " & mnemonic & " at line " & $line).err
    let a = Word(immAddr)
    return
      Instruction(
        line: line, opcode: immOp, operands: @[Byte(a shr 8), Byte(a and 0xFF)]
      ).ok
  of TkMnemonic:
    let lblAddr = ?assembler.resolveLabel(tok.mnemonic, line)
    return
      Instruction(
        line: line,
        opcode: immOp,
        operands: @[Byte(lblAddr shr 8), Byte(lblAddr and 0xFF)],
      ).ok
  else:
    return ("Unexpected token for " & mnemonic & " at line " & $line).err

# Instruction parsers

proc parseNopInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseNoOperandInstruction(assembler, line, OpCode.Nop, "NOP")

proc parseHaltInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseNoOperandInstruction(assembler, line, OpCode.Halt, "HALT")

proc parsePushInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseOneRegisterInstruction(assembler, line, OpCode.Push, "PUSH")

proc parsePopInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseOneRegisterInstruction(assembler, line, OpCode.Pop, "POP")

proc parseNotInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseOneRegisterInstruction(assembler, line, OpCode.Not, "NOT")

proc parseAddInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegOrImmInstruction(assembler, line, OpCode.AddImm, OpCode.Add, "ADD")

proc parseSubInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegOrImmInstruction(assembler, line, OpCode.SubImm, OpCode.Sub, "SUB")

proc parseAndInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegInstruction(assembler, line, OpCode.And, "AND")

proc parseOrInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegInstruction(assembler, line, OpCode.Or, "OR")

proc parseXorInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegInstruction(assembler, line, OpCode.Xor, "XOR")

proc parseCmpInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegOrImmInstruction(assembler, line, OpCode.CmpImm, OpCode.Cmp, "CMP")

proc parseZeroExtendInstruction(
    assembler: var Assembler, line: int
): FvmResult[Instruction] =
  parseExtendInstruction(assembler, line, OpCode.ZeroExtend, "ZEXT")

proc parseSignExtendInstruction(
    assembler: var Assembler, line: int
): FvmResult[Instruction] =
  parseExtendInstruction(assembler, line, OpCode.SignExtend, "SEXT")

proc parseJmpInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseJumpInstruction(assembler, line, OpCode.Jmp, OpCode.JmpReg, "JMP")

proc parseJzInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseJumpInstruction(assembler, line, OpCode.Jz, OpCode.JzReg, "JZ")

proc parseJnzInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseJumpInstruction(assembler, line, OpCode.Jnz, OpCode.JnzReg, "JNZ")

proc parseJcInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseJumpInstruction(assembler, line, OpCode.Jc, OpCode.JcReg, "JC")

proc parseJnInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseJumpInstruction(assembler, line, OpCode.Jn, OpCode.JnReg, "JN")

proc parseCallInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseJumpInstruction(assembler, line, OpCode.Call, OpCode.CallReg, "CALL")

proc parseRetInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseNoOperandInstruction(assembler, line, OpCode.Ret, "RET")

proc parseMovInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegOrImmInstruction(
    assembler, line, OpCode.MovRegImm, OpCode.MovRegReg, "MOV"
  )

proc parseInInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  let dstTok =
    ?assembler.expect({TkRegister}, "Expected register for IN at line " & $line)
  discard ?assembler.expect({TkComma}, "Expected ',' in IN at line " & $line)
  let portTok =
    ?assembler.expect({TkImmediate}, "Expected port number for IN at line " & $line)
  ?consumeEol(assembler, "IN", line)
  let port = portTok.immValue
  if port < 0 or port > 0xFF:
    return ("Port number out of range (0..255) for IN at line " & $line).err

  Instruction(
    line: line, opcode: OpCode.In, operands: @[Byte(dstTok.regEncoding), Byte(port)]
  ).ok

proc parseOutInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  # Operand order is reversed vs IN: OUT port, src
  let portTok =
    ?assembler.expect({TkImmediate}, "Expected port number for OUT at line " & $line)
  discard ?assembler.expect({TkComma}, "Expected ',' in OUT at line " & $line)
  let srcTok =
    ?assembler.expect({TkRegister}, "Expected register for OUT at line " & $line)
  ?consumeEol(assembler, "OUT", line)
  let port = portTok.immValue
  if port < 0 or port > 0xFF:
    return ("Port number out of range (0..255) for OUT at line " & $line).err

  Instruction(
    line: line, opcode: OpCode.Out, operands: @[Byte(port), Byte(srcTok.regEncoding)]
  ).ok

proc parseLoadInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  let dstTok =
    ?assembler.expect(
      {TkRegister}, "Expected destination register for LOAD at line " & $line
    )
  discard ?assembler.expect({TkComma}, "Expected ',' in LOAD at line " & $line)
  let addrTok =
    ?assembler.expect(
      {TkRegister}, "Expected address register for LOAD at line " & $line
    )
  ?consumeEol(assembler, "LOAD", line)
  if addrTok.regEncoding.isLane:
    return ("LOAD address register must be full-width at line " & $line).err

  Instruction(
    line: line,
    opcode: OpCode.Load,
    operands: @[Byte(dstTok.regEncoding), Byte(addrTok.regEncoding)],
  ).ok

proc parseStoreInstruction(
    assembler: var Assembler, line: int
): FvmResult[Instruction] =
  let addrTok =
    ?assembler.expect(
      {TkRegister}, "Expected address register for STORE at line " & $line
    )
  discard ?assembler.expect({TkComma}, "Expected ',' in STORE at line " & $line)
  let srcTok =
    ?assembler.expect(
      {TkRegister}, "Expected source register for STORE at line " & $line
    )
  ?consumeEol(assembler, "STORE", line)
  if addrTok.regEncoding.isLane:
    return ("STORE address register must be full-width at line " & $line).err

  Instruction(
    line: line,
    opcode: OpCode.Store,
    operands: @[Byte(addrTok.regEncoding), Byte(srcTok.regEncoding)],
  ).ok

# Dispatch table

proc buildParserTable(): Table[string, ParseInstructionProc] =
  result["NOP"] = parseNopInstruction
  result["HALT"] = parseHaltInstruction
  result["PUSH"] = parsePushInstruction
  result["POP"] = parsePopInstruction
  result["MOV"] = parseMovInstruction
  result["ZEXT"] = parseZeroExtendInstruction
  result["SEXT"] = parseSignExtendInstruction
  result["ADD"] = parseAddInstruction
  result["SUB"] = parseSubInstruction
  result["AND"] = parseAndInstruction
  result["OR"] = parseOrInstruction
  result["XOR"] = parseXorInstruction
  result["NOT"] = parseNotInstruction
  result["CMP"] = parseCmpInstruction
  result["IN"] = parseInInstruction
  result["OUT"] = parseOutInstruction
  result["JMP"] = parseJmpInstruction
  result["JZ"] = parseJzInstruction
  result["JNZ"] = parseJnzInstruction
  result["JC"] = parseJcInstruction
  result["JN"] = parseJnInstruction
  result["CALL"] = parseCallInstruction
  result["RET"] = parseRetInstruction
  result["LOAD"] = parseLoadInstruction
  result["STORE"] = parseStoreInstruction

let parserTable* = buildParserTable()
