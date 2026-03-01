## Two-pass layout analysis for the assembler.
##
## Pass 1 (computeSectionSizes) walks tokens to total the byte size of each
## section so that absolute base addresses can be derived before any labels are
## assigned.
##
## Pass 2 (buildLabelMap) walks tokens a second time and assigns each label its
## final absolute address using the base addresses from pass 1.

import std/tables
import std/strutils

import ../../types/core
import ../../types/errors
import ../lexer

# Instruction size estimation
#
# These routines mirror the encoding decisions in dispatch.nim without
# requiring a label map, which is the whole reason they exist as a
# separate pre-pass.

proc dbDataSizeAt*(tokens: seq[AsmToken], pos: int): int =
  var i = pos
  while i < tokens.len and tokens[i].kind != TkEol:
    case tokens[i].kind
    of TkImmediate:
      inc result
    of TkStringLit:
      result += tokens[i].strValue.len
    of TkComma:
      discard
    else:
      break
    inc i

proc dwDataSizeAt*(tokens: seq[AsmToken], pos: int): int =
  var i = pos
  while i < tokens.len and tokens[i].kind != TkEol:
    case tokens[i].kind
    of TkImmediate, TkMnemonic:
      result += 2
    of TkComma:
      discard
    else:
      break
    inc i

proc instructionSizeAt*(tokens: seq[AsmToken], pos: int, mnemonic: string): int =
  let upper = mnemonic.toUpperAscii()
  case upper
  of "NOP", "HALT", "RET":
    return 1
  of "PUSH", "POP", "NOT":
    return 2
  of "AND", "OR", "XOR", "ZEXT", "SEXT", "IN", "OUT", "LOAD", "STORE":
    return 3
  of "MOV":
    # Size depends on dst and src: reg/reg = 3, byte-lane imm = 3, word imm or label = 4.
    var i = pos
    while i < tokens.len and tokens[i].kind == TkEol:
      inc i
    if i >= tokens.len:
      return 3
    let dstTok = tokens[i]
    inc i
    while i < tokens.len and tokens[i].kind in {TkComma, TkEol}:
      inc i
    if i >= tokens.len:
      return 3
    let srcTok = tokens[i]
    if srcTok.kind == TkMnemonic:
      return 4
    elif srcTok.kind == TkImmediate and dstTok.kind == TkRegister and
        dstTok.regEncoding.isLane:
      return 3
    elif srcTok.kind == TkImmediate:
      return 4
    else:
      return 3
  of "ADD", "SUB", "CMP":
    # Size depends on src: register = 3, byte-lane imm = 3, word imm = 4.
    var i = pos
    while i < tokens.len and tokens[i].kind == TkEol:
      inc i
    if i >= tokens.len:
      return 3
    let dstTok = tokens[i]
    inc i
    while i < tokens.len and tokens[i].kind in {TkComma, TkEol}:
      inc i
    if i >= tokens.len:
      return 3
    let srcTok = tokens[i]
    if srcTok.kind == TkRegister:
      return 3
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
    return 0
  else:
    return 1

# Section tracking

type Section* = enum
  secCode
  secRodata
  secData

type SectionSizes* = object
  rodataTotal*: int
  codeTotal*: int
  dataTotal*: int

proc computeSectionSizes*(tokens: seq[AsmToken]): FvmResult[SectionSizes] =
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
        if sec == secCode:
          return ("DB directive not allowed in .code section at line " & $tok.line).err
        let bytes = dbDataSizeAt(tokens, i + 1)
        case sec
        of secRodata:
          sizes.rodataTotal += bytes
        of secData:
          sizes.dataTotal += bytes
        of secCode:
          discard
        inc i
        while i < tokens.len and tokens[i].kind != TkEol:
          inc i
      of "DW":
        if sec == secCode:
          return ("DW directive not allowed in .code section at line " & $tok.line).err
        let bytes = dwDataSizeAt(tokens, i + 1)
        case sec
        of secRodata:
          sizes.rodataTotal += bytes
        of secData:
          sizes.dataTotal += bytes
        of secCode:
          discard
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

proc buildLabelMap*(
    tokens: seq[AsmToken], sizes: SectionSizes
): FvmResult[Table[string, Address]] =
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
