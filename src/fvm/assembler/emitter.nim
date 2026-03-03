## Bytecode emitter converts a sequence of Instructions into raw bytes.
##
## The emitter is the last stage of the assembler pipeline:
##   source -> (lexer) -> tokens -> (parser) -> instructions -> (emitter) -> code bytes
##
## The result is returned as a plain `seq[Byte]`; it is up to the assembler
## facade (assembler.nim) to wrap it in an FvmObject with the proper header.

import ../types/core
import ../types/errors
import ./instructions

type EmitResult* = object
  code*: seq[Byte]
  relocations*: seq[uint16]

proc emitBytecodeWithRelocations*(
    instructions: openArray[Instruction]
): FvmResult[EmitResult] =
  var code: seq[Byte]
  var relocations: seq[uint16]

  for instr in instructions:
    let instrStart = code.len
    code.add(Byte(ord(instr.opcode)))
    for operandByte in instr.operands:
      code.add(operandByte)

    # Record relocations for address operands
    for addrOffset in instr.addressOperandOffsets:
      # addrOffset is relative to the start of operands
      # The actual offset in code is 1 (opcode) + addrOffset
      let codeOffset = instrStart + 1 + addrOffset
      relocations.add(uint16(codeOffset))

  EmitResult(code: code, relocations: relocations).ok
