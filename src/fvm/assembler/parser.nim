## Fantasy Assembly Parser
##
## Converts a flat sequence of AsmTokens into a ParseOutput.

import std/logging
import std/strutils
import std/tables

import ../types/core
import ../types/errors
import ./lexer
import ./parser/passes
import ./parser/dispatch

export dispatch, lexer

# Data directive parsers
#
# These live here rather than in dispatch because they produce raw bytes
# into output.rodata / output.data, not Instruction values.

proc parseDbDirective(assembler: var Assembler, line: int): FvmResult[seq[Byte]] =
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

# Public API

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

    # Data directives
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

    # Instructions
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
