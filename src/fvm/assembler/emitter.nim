import ./resolver
import ./mapper
import ./parser
import ../core/types
import ../core/constants
import ../errors
import ../format/fvmobject

# Instruction encoding
#
# Layouts by arg count:
#   0 args:              opcode
#   1 arg, reg:          opcode enc
#   1 arg, imm16:        opcode hi lo
#   2 args, reg+reg:     opcode enc enc
#   2 args, reg+imm8:    opcode enc imm8
#   2 args, reg+imm16:   opcode enc hi lo
#   2 args, imm8+reg:    opcode imm8 enc       (OUT port, reg)

proc emitByte(buf: var seq[Byte], b: Byte) =
  buf.add(b)

proc emitWord(buf: var seq[Byte], w: uint16) =
  buf.add(Byte((w shr 8) and ByteMask))
  buf.add(Byte(w and ByteMask))

proc encodeInstruction(
    node: FlatNode, codeRelocs: var seq[uint16], codeBase: uint16
): seq[Byte] =
  var buf: seq[Byte]
  let op = Byte(ord(node.opcode))
  let instrCodeOffset = node.nodeAddr - codeBase

  case node.args.len
  of 0:
    emitByte(buf, op)
  of 1:
    let a = node.args[0]
    case a.kind
    of faNone:
      raise newAssemblyEmitError(
        "Missing operand in encoded instruction" & " at line " & $node.line & ":" &
        $node.col
      , node.line, node.col)
    of faReg:
      emitByte(buf, op)
      emitByte(buf, Byte(a.enc))
    of faImm8:
      emitByte(buf, op)
      emitByte(buf, a.imm8)
    of faImm16:
      emitByte(buf, op)
      if node.relocate:
        codeRelocs.add(instrCodeOffset + 1)
      emitWord(buf, a.imm16)
  of 2:
    let a0 = node.args[0]
    let a1 = node.args[1]
    emitByte(buf, op)
    case a0.kind
    of faNone:
      raise newAssemblyEmitError(
        "Missing first operand in encoded instruction" & " at line " & $node.line & ":" &
        $node.col
      , node.line, node.col)
    of faImm8:
      # OUT port, reg: opcode imm8 enc
      if a1.kind != faReg:
        raise newAssemblyEmitError(
          "OUT encoding requires a register second operand" & " at line " & $node.line &
          ":" & $node.col
        , node.line, node.col)
      emitByte(buf, a0.imm8)
      emitByte(buf, Byte(a1.enc))
    of faReg:
      case a1.kind
      of faNone:
        raise newAssemblyEmitError(
          "Missing second operand in encoded instruction" & " at line " & $node.line &
          ":" & $node.col
        , node.line, node.col)
      of faReg:
        emitByte(buf, Byte(a0.enc))
        emitByte(buf, Byte(a1.enc))
      of faImm8:
        emitByte(buf, Byte(a0.enc))
        emitByte(buf, a1.imm8)
      of faImm16:
        emitByte(buf, Byte(a0.enc))
        if node.relocate:
          codeRelocs.add(instrCodeOffset + 2)
        emitWord(buf, a1.imm16)
    of faImm16:
      raise newAssemblyEmitError(
        "Unexpected imm16 in first operand position" & " at line " & $node.line & ":" &
        $node.col
      , node.line, node.col)
  else:
    raise newAssemblyEmitError(
      "Instruction with " & $node.args.len & " arguments has no encoding" & " at line " &
      $node.line & ":" & $node.col
    , node.line, node.col)

  buf

# Section assembly

proc assembleSections(
    nodes: seq[FlatNode], codeBase: uint16
): tuple[rodata, code, data: seq[Byte], relocations: seq[uint16]] =
  var rodata, code, data: seq[Byte]
  var codeRelocs: seq[uint16]

  for node in nodes:
    case node.kind
    of fnInstruction:
      let encoded = encodeInstruction(node, codeRelocs, codeBase)
      code.add(encoded)
    of fnBytes:
      case node.section
      of secRoData:
        rodata.add(node.bytes)
      of secCode:
        code.add(node.bytes)
      of secData:
        data.add(node.bytes)

  (rodata, code, data, codeRelocs)

proc emit*(program: ResolvedProgram, sizes: SectionSizes): FvmObject =
  let codeBase = sizes.rodata
  let (rodata, code, data, relocations) = assembleSections(program.nodes, codeBase)
  FvmObject(
    version: FvmVersion,
    entryPoint: program.entryPoint,
    rodata: rodata,
    code: code,
    data: data,
    relocations: relocations,
  )
