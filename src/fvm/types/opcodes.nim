## Instruction set enumeration the single source of truth for the ISA.
## Add new opcodes here; handlers and the assembler parser table are the only
## other places that need updating.

type OpCode* {.pure.} = enum
  # Control
  Nop ## No operation
  Halt ## Stop execution

  # Stack  (register operand uses the encoding byte from types/core)
  Push ## PUSH enc
  Pop ## POP  enc

  # Data movement
  MovRegImm ## MOV  enc, imm16  (imm8 when RegLaneBit set in enc)
  MovRegReg ## MOV  enc, enc
  ZeroExtend ## ZEXT enc, enc  (zero-extend low byte to full register)
  SignExtend ## SEXT enc, enc  (sign-extend low byte to full register)

  # Arithmetic (reserved)
  Add ## ADD  enc, enc
  Sub ## SUB  enc, enc

  # Bitwise (reserved)
  And ## AND  enc, enc
  Or ## OR   enc, enc
  Xor ## XOR  enc, enc
  Not ## NOT  enc

  # Comparison / Branches (reserved)
  Cmp ## CMP  enc, enc  (sets flags, no result stored)
  Jmp ## JMP  addr16
  JmpReg ## JMP  enc
  Jz ## JZ   addr16  (jump if zero flag set)
  JzReg ## JZ   enc
  Jnz ## JNZ  addr16
  JnzReg ## JNZ  enc
  Jc ## JC   addr16  (jump if carry flag set)
  JcReg ## JC   enc
  Jn ## JN   addr16  (jump if negative flag set)
  JnReg ## JN   enc

  # Subroutines
  Call ## CALL addr16  (push ip+3, jump)
  CallReg ## CALL enc     (push ip+2, jump)
  Ret ## RET          (pop addr, jump)

  # Peripherals (reserved)
  In ## IN   enc, port8  (read byte from I/O port)
  Out ## OUT  port8, enc  (write byte to I/O port)

  # Memory access
  Load ## LOAD  dst_enc, addr_enc  (byte or word depending on dst encoding)
  Store ## STORE addr_enc, src_enc  (byte or word depending on src encoding)
