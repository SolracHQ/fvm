import ./parser
import ../types/core

import std/tables

# Types

type
  SectionSizes* = object
    rodata*: uint16
    code*: uint16
    data*: uint16

  SourceMap* = object
    # All label values are absolute VM addresses, not section-relative offsets.
    labels*: Table[string, uint16]
    sizes*: SectionSizes

# Label qualification

proc qualifyLabel*(raw: string, currentGlobal: string): string =
  if raw.len > 0 and raw[0] == '.':
    currentGlobal & raw
  else:
    raw

# Instruction size estimation
#
# Label references always produce a 16-bit immediate so they count as
# hasImm16. Numeric literals wider than 8 bits count as hasImm16 too.

proc hasImm16Arg*(args: seq[Arg]): bool =
  # Label references always resolve to a 16-bit address.
  for a in args:
    if a.kind == akLabelRef: return true
  # A numeric immediate wider than 8 bits must be imm16.
  for a in args:
    if a.kind == akImm and a.imm.value > 0xFF: return true
  # A full-width register destination forces the immediate to 16 bits because
  # the VM reads immediate width from the destination lane bit, not the value.
  if args.len == 2 and args[0].kind == akReg and not args[0].reg.enc.isLane:
    if args[1].kind == akImm:
      return true
  false

type SizeKey* = object
  mnemonic*: string
  argCount*: int
  hasImm16*: bool

const instrSizes* = block:
  var t = initTable[SizeKey, int]()
  for m in ["NOP", "HALT", "RET"]:
    t[SizeKey(mnemonic: m, argCount: 0, hasImm16: false)] = 1
  for m in ["PUSH", "POP", "NOT"]:
    t[SizeKey(mnemonic: m, argCount: 1, hasImm16: false)] = 2
  for m in ["JMP", "JZ", "JNZ", "JC", "JN", "CALL"]:
    t[SizeKey(mnemonic: m, argCount: 1, hasImm16: false)] = 2
    t[SizeKey(mnemonic: m, argCount: 1, hasImm16: true)]  = 3
  for m in ["MOV", "ZEXT", "SEXT", "ADD", "SUB", "AND", "OR", "XOR",
            "CMP", "LOAD", "STORE", "IN", "OUT"]:
    t[SizeKey(mnemonic: m, argCount: 2, hasImm16: false)] = 3
    t[SizeKey(mnemonic: m, argCount: 2, hasImm16: true)]  = 4
  t

proc estimateSize*(mnemonic: string, args: seq[Arg]): FvmResult[int] =
  let key = SizeKey(mnemonic: mnemonic, argCount: args.len, hasImm16: hasImm16Arg(args))
  if key in instrSizes:
    return instrSizes[key].ok
  ("Unknown instruction or operand combination: " & mnemonic &
   " with " & $args.len & " argument(s)").err

proc dbByteCount(items: seq[DbItem]): uint16 =
  for item in items:
    result += (if item.isStr: uint16(item.bytes.len) else: 1)

# Map entry point

proc map*(nodes: seq[Node]): FvmResult[SourceMap] =
  # Pass 1: accumulate section sizes only, no label recording.
  var rodataSize, codeSize, dataSize: uint16
  var section = secCode

  for node in nodes:
    case node.kind
    of nkSection:
      section = node.section

    of nkLabel:
      discard

    of nkInstruction:
      if section != secCode:
        return ("Instruction outside .code section at line " &
                $node.line & ":" & $node.col).err
      codeSize += uint16(?estimateSize(node.mnemonic, node.args))

    of nkDb:
      case section
      of secRoData: rodataSize += dbByteCount(node.dbItems)
      of secData:   dataSize   += dbByteCount(node.dbItems)
      of secCode:
        return ("db directive in .code section at line " &
                $node.line & ":" & $node.col).err

    of nkDw:
      let byteCount = uint16(node.dwItems.len * 2)
      case section
      of secRoData: rodataSize += byteCount
      of secData:   dataSize   += byteCount
      of secCode:
        return ("dw directive in .code section at line " &
                $node.line & ":" & $node.col).err

  # Section bases derived from pass 1 sizes.
  let rodataBase: uint16 = 0
  let codeBase            = rodataSize
  let dataBase            = rodataSize + codeSize

  # Pass 2: record absolute label addresses using the now-known section bases.
  var srcMap = SourceMap(
    sizes: SectionSizes(rodata: rodataSize, code: codeSize, data: dataSize)
  )
  var currentGlobal = ""
  var rodataOff, codeOff, dataOff: uint16
  section = secCode

  for node in nodes:
    case node.kind
    of nkSection:
      section = node.section

    of nkLabel:
      let name = qualifyLabel(node.name, currentGlobal)
      if node.name[0] != '.':
        currentGlobal = node.name
      let absAddr = case section
        of secRoData: rodataBase + rodataOff
        of secCode:   codeBase   + codeOff
        of secData:   dataBase   + dataOff
      if name in srcMap.labels:
        return ("Duplicate label: " & name &
                " at line " & $node.line & ":" & $node.col).err
      srcMap.labels[name] = absAddr

    of nkInstruction:
      codeOff += uint16(?estimateSize(node.mnemonic, node.args))

    of nkDb:
      let byteCount = dbByteCount(node.dbItems)
      case section
      of secRoData: rodataOff += byteCount
      of secData:   dataOff   += byteCount
      of secCode:   discard

    of nkDw:
      let byteCount = uint16(node.dwItems.len * 2)
      case section
      of secRoData: rodataOff += byteCount
      of secData:   dataOff   += byteCount
      of secCode:   discard

  srcMap.ok