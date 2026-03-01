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

proc emitBytecode*(instructions: openArray[Instruction]): FvmResult[seq[Byte]] =
  var code: seq[Byte]
  for instr in instructions:
    code.add(Byte(ord(instr.opcode)))
    code.add(instr.operands)
  code.ok
