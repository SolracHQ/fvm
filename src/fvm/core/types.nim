type
  Byte* = uint8 ## The basic unit of memory and I/O in the FVM.
  Word* = uint16 ## The VM is 16-bit, so a word is also the address width.
  Address* = Word ## Memory addresses are 16-bit in the flat 64 KB space.
  RegEncoding* = distinct Byte
    ## Register operand encoding, including lane selection bits.

  Permission* = enum
    Read
    Write
    Execute

  Permissions* = set[Permission]

  Flag* = enum
    Zero
    Carry
    Negative

  Flags* = set[Flag]

  OpCode* {.pure.} = enum
    Nop
    Halt
    Push
    Pop
    MovRegImm
    MovRegReg
    ZeroExtend
    SignExtend
    Add
    Sub
    AddImm
    SubImm
    And
    Or
    Xor
    Not
    Cmp
    CmpImm
    Jmp
    JmpReg
    Jz
    JzReg
    Jnz
    JnzReg
    Jc
    JcReg
    Jn
    JnReg
    Call
    CallReg
    Ret
    In
    Out
    Load
    Store
    SieRegImm
    SieRegReg
    Iret
    IntImm
    IntReg
    Dpl

  FlatArgKind* = enum
    faNone
    faReg
    faImm8
    faImm16

  FlatArg* = object
    case kind*: FlatArgKind
    of faNone:
      discard
    of faReg:
      enc*: RegEncoding
    of faImm8:
      imm8*: Byte
    of faImm16:
      imm16*: Word

  DecodedInstruction* = object
    opcode*: OpCode
    args*: array[2, FlatArg]
    argCount*: uint8
    size*: uint8
