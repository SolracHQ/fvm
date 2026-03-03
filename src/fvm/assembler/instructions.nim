## Assembler instruction representation.
##
## An Instruction stores the resolved OpCode directly eliminating the
## redundant InstructionKind enum that was a parallel copy of OpCode.
## The `operands` seq holds the raw bytes that follow the opcode byte in the
## emitted code.

import ../types/core
import ../types/opcodes

type Instruction* = object
  line*: int ## Source line number (for error reporting)
  opcode*: OpCode
  operands*: seq[Byte]
  addressOperandOffsets*: seq[int]
    ## Byte offsets in operands that contain 16-bit addresses

type ParseOutput* = object
  ## Combined output of the parser: three sections plus the resolved
  ## instruction list for the code section.
  rodata*: seq[Byte] ## .rodata section bytes
  instructions*: seq[Instruction] ## .code section, not yet emitted
  data*: seq[Byte] ## .data section bytes
