import ./parser
import ./mapper
import ../core/types
import ../core/constants
import ../core/registers

import std/tables

# Types

type
  FlatNodeKind* = enum
    fnInstruction
    fnBytes

  FlatNode* = object
    line*: uint16
    col*: uint16
    nodeAddr*: uint16
    section*: SectionKind
    case kind*: FlatNodeKind
    of fnInstruction:
      opcode*: OpCode
      args*: seq[FlatArg]
      relocate*: bool
    of fnBytes:
      bytes*: seq[Byte]

  ResolvedProgram* = object
    nodes*: seq[FlatNode]
    entryPoint*: uint16
    relocations*: seq[uint16]

# Section base address

proc sectionBase(sizes: SectionSizes, section: SectionKind): uint16 =
  case section
  of secRoData:
    0'u16
  of secCode:
    sizes.rodata
  of secData:
    sizes.rodata + sizes.code

# Opcode dispatch
#
# Key format: "MNEMONIC:X:Y" where X/Y are R (reg), 8 (imm8), A (imm16/addr).

proc opcodeKey(mnemonic: string, args: seq[FlatArg]): string =
  result = mnemonic
  for a in args:
    result.add(':')
    case a.kind
    of faNone:
      result.add('_')
    of faReg:
      result.add('R')
    of faImm8:
      result.add('8')
    of faImm16:
      result.add('A')

const opcodeTable = block:
  var t = initTable[string, OpCode]()
  t["NOP"] = OpCode.Nop
  t["HALT"] = OpCode.Halt
  t["RET"] = OpCode.Ret
  t["PUSH:R"] = OpCode.Push
  t["POP:R"] = OpCode.Pop
  t["NOT:R"] = OpCode.Not
  t["MOV:R:R"] = OpCode.MovRegReg
  t["MOV:R:8"] = OpCode.MovRegImm
  t["MOV:R:A"] = OpCode.MovRegImm
  t["ZEXT:R:R"] = OpCode.ZeroExtend
  t["SEXT:R:R"] = OpCode.SignExtend
  t["ADD:R:R"] = OpCode.Add
  t["ADD:R:8"] = OpCode.AddImm
  t["ADD:R:A"] = OpCode.AddImm
  t["SUB:R:R"] = OpCode.Sub
  t["SUB:R:8"] = OpCode.SubImm
  t["SUB:R:A"] = OpCode.SubImm
  t["AND:R:R"] = OpCode.And
  t["OR:R:R"] = OpCode.Or
  t["XOR:R:R"] = OpCode.Xor
  t["CMP:R:R"] = OpCode.Cmp
  t["CMP:R:8"] = OpCode.CmpImm
  t["CMP:R:A"] = OpCode.CmpImm
  t["JMP:R"] = OpCode.JmpReg
  t["JMP:A"] = OpCode.Jmp
  t["JZ:R"] = OpCode.JzReg
  t["JZ:A"] = OpCode.Jz
  t["JNZ:R"] = OpCode.JnzReg
  t["JNZ:A"] = OpCode.Jnz
  t["JC:R"] = OpCode.JcReg
  t["JC:A"] = OpCode.Jc
  t["JN:R"] = OpCode.JnReg
  t["JN:A"] = OpCode.Jn
  t["CALL:R"] = OpCode.CallReg
  t["CALL:A"] = OpCode.Call
  t["IN:R:8"] = OpCode.In
  t["OUT:8:R"] = OpCode.Out
  t["LOAD:R:R"] = OpCode.Load
  t["STORE:R:R"] = OpCode.Store
  t

# Argument resolution

proc resolveArg(
    arg: Arg, srcMap: SourceMap, currentGlobal: string
): FvmResult[FlatArg] =
  case arg.kind
  of akReg:
    FlatArg(kind: faReg, enc: arg.reg.enc).ok
  of akImm:
    if arg.imm.value > 0xFF:
      FlatArg(kind: faImm16, imm16: arg.imm.value).ok
    else:
      FlatArg(kind: faImm8, imm8: Byte(arg.imm.value)).ok
  of akLabelRef:
    let name = qualifyLabel(arg.lbl.raw, currentGlobal)
    if name notin srcMap.labels:
      return (
        "Undefined label: " & arg.lbl.raw & " at line " & $arg.line & ":" & $arg.col
      ).err
    # Labels in the map carry absolute VM addresses (see mapper note).
    FlatArg(kind: faImm16, imm16: srcMap.labels[name]).ok

proc resolveInstruction(
    node: Node,
    nodeAddr: uint16,
    section: SectionKind,
    srcMap: SourceMap,
    currentGlobal: string,
    relocations: var seq[uint16],
): FvmResult[FlatNode] =
  var flatArgs: seq[FlatArg]
  var hasReloc = false

  for arg in node.args:
    let flat = ?resolveArg(arg, srcMap, currentGlobal)
    if arg.kind == akLabelRef:
      hasReloc = true
      if section == secCode:
        relocations.add(nodeAddr)
    flatArgs.add(flat)

  # The VM determines immediate width from the destination register's lane bit,
  # not from a separate opcode. A full-width register destination always causes
  # the handler to read 2 bytes for the immediate, so faImm8 must be promoted
  # to faImm16 when arg0 is a full-width register and arg1 is a numeric immediate.
  if flatArgs.len == 2 and flatArgs[0].kind == faReg and not flatArgs[0].enc.isLane and
      flatArgs[1].kind == faImm8:
    flatArgs[1] = FlatArg(kind: faImm16, imm16: uint16(flatArgs[1].imm8))

  let key = opcodeKey(node.mnemonic, flatArgs)
  if key notin opcodeTable:
    return (
      "No opcode for " & node.mnemonic & " with operands " & key & " at line " &
      $node.line & ":" & $node.col
    ).err

  FlatNode(
    line: node.line,
    col: node.col,
    nodeAddr: nodeAddr,
    section: section,
    kind: fnInstruction,
    opcode: opcodeTable[key],
    args: flatArgs,
    relocate: hasReloc,
  ).ok

proc flattenDataNode(node: Node, nodeAddr: uint16, section: SectionKind): FlatNode =
  var bytes: seq[Byte]
  case node.kind
  of nkDb:
    for item in node.dbItems:
      if item.isStr:
        bytes.add(item.bytes)
      else:
        bytes.add(item.value)
  of nkDw:
    for w in node.dwItems:
      bytes.add(Byte((w shr 8) and ByteMask))
      bytes.add(Byte(w and ByteMask))
  else:
    discard
  FlatNode(
    line: node.line,
    col: node.col,
    nodeAddr: nodeAddr,
    section: section,
    kind: fnBytes,
    bytes: bytes,
  )

# Resolve entry point

proc resolve*(nodes: seq[Node], srcMap: SourceMap): FvmResult[ResolvedProgram] =
  var program: ResolvedProgram
  var section = secCode
  var currentGlobal = ""
  var rodataOff, codeOff, dataOff: uint16

  for node in nodes:
    case node.kind
    of nkSection:
      section = node.section
    of nkLabel:
      if node.name[0] != '.':
        currentGlobal = node.name
    of nkInstruction:
      let base = sectionBase(srcMap.sizes, section)
      let nodeAddr = base + codeOff
      let flat = ?resolveInstruction(
        node, nodeAddr, section, srcMap, currentGlobal, program.relocations
      )
      codeOff += uint16(?estimateSize(node.mnemonic, node.args))
      program.nodes.add(flat)
    of nkDb, nkDw:
      let base = sectionBase(srcMap.sizes, section)
      let sectionOff =
        case section
        of secRoData: rodataOff
        of secData: dataOff
        of secCode: 0'u16
      let flat = flattenDataNode(node, base + sectionOff, section)
      let byteLen = uint16(flat.bytes.len)
      case section
      of secRoData:
        rodataOff += byteLen
      of secData:
        dataOff += byteLen
      of secCode:
        discard
      program.nodes.add(flat)

  let codeBase = sectionBase(srcMap.sizes, secCode)
  program.entryPoint =
    if "main" in srcMap.labels:
      srcMap.labels["main"]
    else:
      codeBase

  program.ok
