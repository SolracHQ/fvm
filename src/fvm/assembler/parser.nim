## Fantasy Assembly Parser
##
## Converts a flat sequence of AsmTokens into a sequence of Instructions.
## Mnemonic dispatch uses a Table[string, ParseInstructionProc] keyed by upper-case mnemonic.

import std/tables
import std/logging
import std/strutils

import ../types/core
import ../types/opcodes
import ../types/errors
import ./lexer
import ./instructions

export instructions, lexer ## Re-export so callers get Instruction and AsmToken types

# Section tracking

type Section* = enum
  secCode ## default when no directive has been seen
  secRodata
  secData

# Assembler cursor

type
  Assembler* = object
    tokens*: seq[AsmToken]
    current*: int
    labelMap*: Table[string, Address]
    currentGlobal*: string ## tracks last global label for local label expansion
    currentSection*: Section ## active section for directives

  ParseInstructionProc* =
    proc(assembler: var Assembler, line: int): FvmResult[Instruction]

proc hasMore(a: Assembler): bool =
  a.current < a.tokens.len

proc peek(a: Assembler): AsmToken =
  a.tokens[a.current]

proc advance(a: var Assembler): AsmToken =
  result = a.tokens[a.current]
  inc a.current

# Generic parse helpers

proc expect(
    assembler: var Assembler, kinds: set[TokenKind], msg: string
): FvmResult[AsmToken] =
  if not assembler.hasMore:
    return msg.err
  let tok = assembler.advance()
  if tok.kind notin kinds:
    return msg.err
  tok.ok

proc consumeEol(
    assembler: var Assembler, mnemonic: string, line: int
): FvmResult[void] =
  ## Consumes the mandatory TkEol at the end of a statement.
  discard
    ?assembler.expect(
      {TkEol},
      "Expected end of line after " & mnemonic & " at line " & $line &
        ", got extra token",
    )
  ok()

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

# Label helpers

proc expandLabel(name: string, currentGlobal: string): string =
  if name.len > 0 and name[0] == '.':
    currentGlobal & name
  else:
    name

proc resolveLabel(assembler: Assembler, name: string, line: int): FvmResult[Address] =
  let expanded = expandLabel(name, assembler.currentGlobal)
  if expanded notin assembler.labelMap:
    return ("Undefined label '" & expanded & "' at line " & $line).err
  assembler.labelMap[expanded].ok

# Instruction-specific parsers

proc parseNopInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseNoOperandInstruction(assembler, line, OpCode.Nop, "NOP")

proc parseHaltInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseNoOperandInstruction(assembler, line, OpCode.Halt, "HALT")

proc parsePushInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseOneRegisterInstruction(assembler, line, OpCode.Push, "PUSH")

proc parsePopInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseOneRegisterInstruction(assembler, line, OpCode.Pop, "POP")

proc parseMovInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  let dstTok =
    ?assembler.expect(
      {TkRegister}, "Expected destination register for MOV at line " & $line
    )
  discard ?assembler.expect({TkComma}, "Expected ',' in MOV at line " & $line)
  let srcTok =
    ?assembler.expect(
      {TkImmediate, TkRegister, TkMnemonic},
      "Expected register, immediate, or label for MOV source at line " & $line,
    )
  ?consumeEol(assembler, "MOV", line)

  case srcTok.kind
  of TkImmediate:
    let dstEnc = dstTok.regEncoding
    if dstEnc.isLane:
      # Byte-lane destination: 8-bit immediate
      let imm = srcTok.immValue
      if imm < 0 or imm > 0xFF:
        return
          ("Immediate out of range (0..255) for byte-lane MOV at line " & $line).err
      return
        Instruction(
          line: line, opcode: OpCode.MovRegImm, operands: @[Byte(dstEnc), Byte(imm)]
        ).ok
    else:
      # Full-width destination: 16-bit immediate (big-endian)
      let imm = srcTok.immValue
      if imm < 0 or imm > 0xFFFF:
        return ("Immediate out of range (0..65535) for MOV at line " & $line).err
      let imm16 = Word(imm)
      return
        Instruction(
          line: line,
          opcode: OpCode.MovRegImm,
          operands:
            @[Byte(dstEnc), Byte((imm16 shr 8) and ByteMask), Byte(imm16 and ByteMask)],
        ).ok
  of TkMnemonic:
    # Label reference: resolves to a 16-bit address; always full-width.
    let dstEnc = dstTok.regEncoding
    if dstEnc.isLane:
      return
        ("Cannot MOV a label address into a byte-lane register at line " & $line).err
    let lblAddr = ?assembler.resolveLabel(srcTok.mnemonic, line)
    return
      Instruction(
        line: line,
        opcode: OpCode.MovRegImm,
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
        "MOV lane mismatch at line " & $line & ": " & dstTok.regSource & " vs " &
        srcTok.regSource
      ).err
    return
      Instruction(
        line: line, opcode: OpCode.MovRegReg, operands: @[Byte(dstEnc), Byte(srcEnc)]
      ).ok

proc parseZeroExtendInstruction(
    assembler: var Assembler, line: int
): FvmResult[Instruction] =
  let dstTok =
    ?assembler.expect(
      {TkRegister}, "Expected destination register for ZEXT at line " & $line
    )
  discard ?assembler.expect({TkComma}, "Expected ',' in ZEXT at line " & $line)
  let srcTok =
    ?assembler.expect(
      {TkRegister}, "Expected source register for ZEXT at line " & $line
    )
  ?consumeEol(assembler, "ZEXT", line)
  let dstEnc = dstTok.regEncoding
  let srcEnc = srcTok.regEncoding
  if dstEnc.isLane:
    return ("Destination register cannot be byte-lane for ZEXT at line " & $line).err
  if srcEnc.isWord:
    return ("Source register must be byte-lane for ZEXT at line " & $line).err

  Instruction(
    line: line, opcode: OpCode.ZeroExtend, operands: @[Byte(dstEnc), Byte(srcEnc)]
  ).ok

proc parseSignExtendInstruction(
    assembler: var Assembler, line: int
): FvmResult[Instruction] =
  let dstTok =
    ?assembler.expect(
      {TkRegister}, "Expected destination register for SEXT at line " & $line
    )
  discard ?assembler.expect({TkComma}, "Expected ',' in SEXT at line " & $line)
  let srcTok =
    ?assembler.expect(
      {TkRegister}, "Expected source register for SEXT at line " & $line
    )
  ?consumeEol(assembler, "SEXT", line)
  let dstEnc = dstTok.regEncoding
  let srcEnc = srcTok.regEncoding
  if dstEnc.isLane:
    return ("Destination register cannot be byte-lane for SEXT at line " & $line).err
  if srcEnc.isWord:
    return ("Source register must be byte-lane for SEXT at line " & $line).err

  Instruction(
    line: line, opcode: OpCode.SignExtend, operands: @[Byte(dstEnc), Byte(srcEnc)]
  ).ok

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

proc parseAddInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegInstruction(assembler, line, OpCode.Add, "ADD")

proc parseSubInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegInstruction(assembler, line, OpCode.Sub, "SUB")

proc parseAndInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegInstruction(assembler, line, OpCode.And, "AND")

proc parseOrInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegInstruction(assembler, line, OpCode.Or, "OR")

proc parseXorInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegInstruction(assembler, line, OpCode.Xor, "XOR")

proc parseNotInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseOneRegisterInstruction(assembler, line, OpCode.Not, "NOT")

proc parseCmpInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseBinaryRegInstruction(assembler, line, OpCode.Cmp, "CMP")

proc parseInInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  ## IN dst, port  --  encodes as [In, enc, port]
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
  ## OUT port, src  --  encodes as [Out, port, enc]
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

proc parseRetInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  parseNoOperandInstruction(assembler, line, OpCode.Ret, "RET")

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

# Memory access instruction parsers

proc parseLoadInstruction(assembler: var Assembler, line: int): FvmResult[Instruction] =
  ## LOAD dst, addr  --  dst encoding determines byte vs word access
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
  if addrTok.regEncoding.isLane or addrTok.regEncoding.isSp:
    return ("LOAD address register must be full-width at line " & $line).err

  Instruction(
    line: line,
    opcode: OpCode.Load,
    operands: @[Byte(dstTok.regEncoding), Byte(addrTok.regEncoding)],
  ).ok

proc parseStoreInstruction(
    assembler: var Assembler, line: int
): FvmResult[Instruction] =
  ## STORE addr, src  --  src encoding determines byte vs word access
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
  if addrTok.regEncoding.isLane or addrTok.regEncoding.isSp:
    return ("STORE address register must be full-width at line " & $line).err

  Instruction(
    line: line,
    opcode: OpCode.Store,
    operands: @[Byte(addrTok.regEncoding), Byte(srcTok.regEncoding)],
  ).ok

# Dispatch table
#
# Keyed by upper-case mnemonic string. Add an entry here for every new instruction.

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

let parserTable = buildParserTable()

# Pass 1: label map builder

proc instructionSizeAt(tokens: seq[AsmToken], pos: int, mnemonic: string): int =
  let upper = mnemonic.toUpperAscii()
  case upper
  of "NOP", "HALT", "RET":
    return 1
  of "PUSH", "POP", "NOT":
    return 2
  of "ADD", "SUB", "AND", "OR", "XOR", "CMP", "ZEXT", "SEXT", "IN", "OUT", "LOAD",
      "STORE":
    return 3
  of "MOV":
    var i = pos
    while i < tokens.len and tokens[i].kind == TkEol:
      inc i
    if i >= tokens.len:
      return 3
    let dstTok = tokens[i]
    i += 1
    while i < tokens.len and tokens[i].kind in {TkComma, TkEol}:
      inc i
    if i >= tokens.len:
      return 3
    let srcTok = tokens[i]
    if srcTok.kind == TkMnemonic:
      # Label source: always 4 bytes (word immediate)
      return 4
    elif srcTok.kind == TkImmediate and dstTok.kind == TkRegister and
        dstTok.regEncoding.isLane:
      return 3
    elif srcTok.kind == TkImmediate:
      return 4
    else:
      return 3
  of "JMP", "JZ", "JNZ", "JC", "JN", "CALL":
    var i = pos
    while i < tokens.len and tokens[i].kind == TkEol:
      inc i
    if i >= tokens.len:
      return 3
    return if tokens[i].kind == TkRegister: 2 else: 3
  of ".RODATA", ".CODE", ".DATA":
    return 0 # section directives emit no bytes
  else:
    return 1

proc dbDataSizeAt(tokens: seq[AsmToken], pos: int): int =
  ## Counts the byte count that a DB directive at `pos` would emit.
  var i = pos
  var count = 0
  while i < tokens.len and tokens[i].kind != TkEol:
    case tokens[i].kind
    of TkImmediate:
      count += 1
    of TkStringLit:
      count += tokens[i].strValue.len
    of TkComma:
      discard
    else:
      break
    inc i
  count

proc dwDataSizeAt(tokens: seq[AsmToken], pos: int): int =
  ## Counts the byte count that a DW directive at `pos` would emit (2 per item).
  var i = pos
  var count = 0
  while i < tokens.len and tokens[i].kind != TkEol:
    case tokens[i].kind
    of TkImmediate, TkMnemonic:
      count += 2
    of TkComma:
      discard
    else:
      break
    inc i
  count

type SectionSizes = object
  rodataTotal: int
  codeTotal: int
  dataTotal: int

proc computeSectionSizes(tokens: seq[AsmToken]): FvmResult[SectionSizes] =
  ## Sub-pass 1: walks tokens to compute the byte size of each section.
  var sec = secCode
  var sizes = SectionSizes()
  var i = 0
  while i < tokens.len:
    let tok = tokens[i]
    case tok.kind
    of TkLabel, TkComma, TkEol:
      inc i
    of TkMnemonic:
      let upper = tok.mnemonic.toUpperAscii()
      case upper
      of ".RODATA":
        sec = secRodata
        inc i
      of ".CODE":
        sec = secCode
        inc i
      of ".DATA":
        sec = secData
        inc i
      of "DB":
        let bytes = dbDataSizeAt(tokens, i + 1)
        case sec
        of secRodata:
          sizes.rodataTotal += bytes
        of secData:
          sizes.dataTotal += bytes
        of secCode:
          return ("DB directive not allowed in .code section at line " & $tok.line).err
        inc i
        while i < tokens.len and tokens[i].kind != TkEol:
          inc i
      of "DW":
        let bytes = dwDataSizeAt(tokens, i + 1)
        case sec
        of secRodata:
          sizes.rodataTotal += bytes
        of secData:
          sizes.dataTotal += bytes
        of secCode:
          return ("DW directive not allowed in .code section at line " & $tok.line).err
        inc i
        while i < tokens.len and tokens[i].kind != TkEol:
          inc i
      else:
        if sec == secCode:
          sizes.codeTotal += instructionSizeAt(tokens, i + 1, upper)
        inc i
        while i < tokens.len and tokens[i].kind != TkEol:
          inc i
    else:
      inc i
  sizes.ok

proc buildLabelMap(
    tokens: seq[AsmToken], sizes: SectionSizes
): FvmResult[Table[string, Address]] =
  ## Sub-pass 2: walks tokens assigning absolute addresses to labels.
  ## .rodata starts at 0x0000; .code starts at rodataTotal;
  ## .data starts at rodataTotal + codeTotal.
  var labelMap: Table[string, Address]
  let rodataBase = 0
  let codeBase = sizes.rodataTotal
  let dataBase = sizes.rodataTotal + sizes.codeTotal
  var offsets: array[Section, int]
  var sec = secCode
  var currentGlobal = ""
  var i = 0

  while i < tokens.len:
    let tok = tokens[i]
    case tok.kind
    of TkLabel:
      let labelAddr =
        case sec
        of secRodata:
          rodataBase + offsets[secRodata]
        of secCode:
          codeBase + offsets[secCode]
        of secData:
          dataBase + offsets[secData]
      let name = tok.labelName
      if name.len > 0 and name[0] == '.':
        let expanded = currentGlobal & name
        if expanded in labelMap:
          return ("Duplicate label '" & expanded & "' at line " & $tok.line).err
        labelMap[expanded] = Address(labelAddr)
      else:
        if name in labelMap:
          return ("Duplicate label '" & name & "' at line " & $tok.line).err
        labelMap[name] = Address(labelAddr)
        currentGlobal = name
      inc i
    of TkMnemonic:
      let upper = tok.mnemonic.toUpperAscii()
      case upper
      of ".RODATA":
        sec = secRodata
        inc i
      of ".CODE":
        sec = secCode
        inc i
      of ".DATA":
        sec = secData
        inc i
      of "DB":
        let bytes = dbDataSizeAt(tokens, i + 1)
        case sec
        of secRodata:
          offsets[secRodata] += bytes
        of secData:
          offsets[secData] += bytes
        of secCode:
          discard
        inc i
        while i < tokens.len and tokens[i].kind != TkEol:
          inc i
      of "DW":
        let bytes = dwDataSizeAt(tokens, i + 1)
        case sec
        of secRodata:
          offsets[secRodata] += bytes
        of secData:
          offsets[secData] += bytes
        of secCode:
          discard
        inc i
        while i < tokens.len and tokens[i].kind != TkEol:
          inc i
      else:
        if sec == secCode:
          offsets[secCode] += instructionSizeAt(tokens, i + 1, upper)
        inc i
        while i < tokens.len and tokens[i].kind != TkEol:
          inc i
    of TkEol:
      inc i
    else:
      inc i

  labelMap.ok

# Public API

proc parseDbDirective(assembler: var Assembler, line: int): FvmResult[seq[Byte]] =
  ## Parses a DB operand list: comma-separated immediates (0-255) and strings.
  var bytes: seq[Byte]
  while assembler.hasMore and assembler.peek().kind != TkEol:
    let tok = assembler.advance()
    case tok.kind
    of TkImmediate:
      if tok.immValue < 0 or tok.immValue > 0xFF:
        return ("DB value out of range (0..255) at line " & $line).err
      bytes.add(Byte(tok.immValue))
    of TkStringLit:
      for ch in tok.strValue:
        bytes.add(Byte(ch))
    of TkComma:
      discard
    else:
      return ("Unexpected token in DB at line " & $line).err
  ?consumeEol(assembler, "DB", line)
  bytes.ok

proc parseDwDirective(assembler: var Assembler, line: int): FvmResult[seq[Byte]] =
  ## Parses a DW operand list: comma-separated 16-bit immediates or labels.
  var bytes: seq[Byte]
  while assembler.hasMore and assembler.peek().kind != TkEol:
    let tok = assembler.advance()
    case tok.kind
    of TkImmediate:
      if tok.immValue < 0 or tok.immValue > 0xFFFF:
        return ("DW value out of range (0..65535) at line " & $line).err
      let w = Word(tok.immValue)
      bytes.add(Byte(w shr 8))
      bytes.add(Byte(w and ByteMask))
    of TkMnemonic:
      let labelAddr = ?assembler.resolveLabel(tok.mnemonic, line)
      bytes.add(Byte(labelAddr shr 8))
      bytes.add(Byte(labelAddr and ByteMask))
    of TkComma:
      discard
    else:
      return ("Unexpected token in DW at line " & $line).err
  ?consumeEol(assembler, "DW", line)
  bytes.ok

proc parseTokens*(tokens: openArray[AsmToken]): FvmResult[ParseOutput] =
  let sizes = ?computeSectionSizes(@tokens)
  let lmap = ?buildLabelMap(@tokens, sizes)
  var assembler = Assembler(
    tokens: @tokens,
    current: 0,
    labelMap: lmap,
    currentGlobal: "",
    currentSection: secCode,
  )
  var output = ParseOutput()

  while assembler.hasMore:
    let tok = assembler.advance()
    if tok.kind == TkEol:
      continue
    if tok.kind == TkLabel:
      if tok.labelName.len > 0 and tok.labelName[0] != '.':
        assembler.currentGlobal = tok.labelName
      continue
    if tok.kind != TkMnemonic:
      return ("Unexpected token kind at line " & $tok.line).err

    let mnemonic = tok.mnemonic.toUpperAscii()

    # Section directives
    case mnemonic
    of ".RODATA":
      assembler.currentSection = secRodata
      ?consumeEol(assembler, ".rodata", tok.line)
      continue
    of ".CODE":
      assembler.currentSection = secCode
      ?consumeEol(assembler, ".code", tok.line)
      continue
    of ".DATA":
      assembler.currentSection = secData
      ?consumeEol(assembler, ".data", tok.line)
      continue
    else:
      discard

    # Data directives: only valid outside .code
    case mnemonic
    of "DB":
      if assembler.currentSection == secCode:
        return ("DB directive not allowed in .code section at line " & $tok.line).err
      let bytes = ?parseDbDirective(assembler, tok.line)
      case assembler.currentSection
      of secRodata:
        output.rodata.add(bytes)
      of secData:
        output.data.add(bytes)
      of secCode:
        discard
      continue
    of "DW":
      if assembler.currentSection == secCode:
        return ("DW directive not allowed in .code section at line " & $tok.line).err
      let bytes = ?parseDwDirective(assembler, tok.line)
      case assembler.currentSection
      of secRodata:
        output.rodata.add(bytes)
      of secData:
        output.data.add(bytes)
      of secCode:
        discard
      continue
    else:
      discard

    # Instructions: only valid in .code
    if assembler.currentSection != secCode:
      return (
        "Instruction '" & mnemonic & "' not allowed outside .code section at line " &
        $tok.line
      ).err

    let parser = parserTable.getOrDefault(mnemonic, nil)
    if parser == nil:
      return ("Unknown mnemonic '" & tok.mnemonic & "' at line " & $tok.line).err

    debug "Parsing mnemonic: " & mnemonic & " at line " & $tok.line
    let instrResult = parser(assembler, tok.line)
    if instrResult.isErr:
      return instrResult.error.err
    output.instructions.add(instrResult.get())

  output.ok
