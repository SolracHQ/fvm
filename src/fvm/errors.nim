import std/strutils

import ./core/types as coretypes

export coretypes

type
  FvmError* = object of CatchableError

  VmError* = object of FvmError
  VmFault* = object of VmError
  BusFaultError* = object of VmFault
  StackOverflowError* = object of VmFault
  StackUnderflowError* = object of VmFault
  PrivilegeFaultError* = object of VmFault
  InvalidOpcodeError* = object of VmFault

  VmLayoutError* = object of VmError
  RelocationError* = object of VmError
  InstructionPointerError* = object of VmError
  DecoderError* = object of VmError
  InstructionBoundsError* = object of DecoderError
  MissingDecoderError* = object of VmError
  MissingHandlerError* = object of VmError
  UnimplementedOpcodeError* = object of VmError
  RegisterEncodingError* = object of VmError
  OperandError* = object of VmError
  OperandKindError* = object of OperandError
  OperandWidthError* = object of OperandError
  AddressRegisterError* = object of OperandError
  InterruptIndexError* = object of PrivilegeFaultError
  InterruptStateError* = object of PrivilegeFaultError
  PortError* = object of VmError
  PortRegistrationError* = object of PortError
  PortAccessError* = object of PortError
  PortIoError* = object of PortError
  PortEofError* = object of PortIoError
  PortValueError* = object of PortIoError

  AssemblyError* = object of FvmError
  AssemblyLexError* = object of AssemblyError
  AssemblyParseError* = object of AssemblyError
  AssemblyMapError* = object of AssemblyError
  AssemblyResolveError* = object of AssemblyError
  AssemblyEmitError* = object of AssemblyError
  AssemblyIoError* = object of AssemblyError

  ObjectFormatError* = object of FvmError
  LoggerError* = object of FvmError
  CliError* = object of FvmError

proc newError[T](errorType: typedesc[T], message: string): ref T {.inline.} =
  (ref T)(msg: message)

proc withLocation(message: string, line, col: uint16): string =
  message & " at line " & $line & ":" & $col

proc newBusFaultError*(message: string): ref BusFaultError {.inline.} =
  newError(BusFaultError, message)

proc newStackOverflowError*(message: string): ref StackOverflowError {.inline.} =
  newError(StackOverflowError, message)

proc newStackUnderflowError*(message: string): ref StackUnderflowError {.inline.} =
  newError(StackUnderflowError, message)

proc newPrivilegeFaultError*(message: string): ref PrivilegeFaultError {.inline.} =
  newError(PrivilegeFaultError, message)

proc newInvalidOpcodeError*(opcode: Byte): ref InvalidOpcodeError =
  newError(InvalidOpcodeError, "Invalid opcode byte: 0x" & toHex(int(opcode), 2))

proc newVmLayoutError*(message: string): ref VmLayoutError {.inline.} =
  newError(VmLayoutError, message)

proc newRelocationError*(message: string): ref RelocationError {.inline.} =
  newError(RelocationError, message)

proc newInstructionPointerError*(message = "Instruction pointer out of bounds"): ref InstructionPointerError {.inline.} =
  newError(InstructionPointerError, message)

proc newInstructionBoundsError*(message = "Instruction out of bounds"): ref InstructionBoundsError {.inline.} =
  newError(InstructionBoundsError, message)

proc newMissingDecoderError*(opcode: OpCode): ref MissingDecoderError =
  newError(MissingDecoderError, "No decoder for opcode 0x" & toHex(ord(opcode), 2))

proc newMissingHandlerError*(opcode: OpCode): ref MissingHandlerError =
  newError(MissingHandlerError, "No handler for opcode 0x" & toHex(ord(opcode), 2))

proc newUnimplementedOpcodeError*(opcode: Byte): ref UnimplementedOpcodeError =
  newError(UnimplementedOpcodeError, "Unimplemented opcode 0x" & toHex(int(opcode), 2))

proc newRegisterEncodingError*(message: string): ref RegisterEncodingError {.inline.} =
  newError(RegisterEncodingError, message)

proc newOperandKindError*(message: string): ref OperandKindError {.inline.} =
  newError(OperandKindError, message)

proc newOperandWidthError*(message: string): ref OperandWidthError {.inline.} =
  newError(OperandWidthError, message)

proc newAddressRegisterError*(message: string): ref AddressRegisterError {.inline.} =
  newError(AddressRegisterError, message)

proc newInterruptIndexError*(value: SomeInteger): ref InterruptIndexError =
  newError(InterruptIndexError, "Invalid interrupt vector index: " & $value)

proc newInterruptRaiseIndexError*(index: int): ref InterruptIndexError =
  newError(InterruptIndexError, "Interrupt index out of range: " & $index)

proc newInterruptStateError*(message = "IRET outside interrupt handler"): ref InterruptStateError {.inline.} =
  newError(InterruptStateError, message)

proc newPortRegistrationError*(message: string): ref PortRegistrationError {.inline.} =
  newError(PortRegistrationError, message)

proc newPortAccessError*(message: string): ref PortAccessError {.inline.} =
  newError(PortAccessError, message)

proc newPortIoError*(message: string): ref PortIoError {.inline.} =
  newError(PortIoError, message)

proc newPortEofError*(message = "EOF on port read"): ref PortEofError {.inline.} =
  newError(PortEofError, message)

proc newPortValueError*(message: string): ref PortValueError {.inline.} =
  newError(PortValueError, message)

proc newAssemblyLexError*(message: string, line, col: uint16): ref AssemblyLexError =
  newError(AssemblyLexError, withLocation(message, line, col))

proc newAssemblyParseError*(message: string, line, col: uint16): ref AssemblyParseError =
  newError(AssemblyParseError, withLocation(message, line, col))

proc newAssemblyMapError*(message: string, line, col: uint16): ref AssemblyMapError =
  newError(AssemblyMapError, withLocation(message, line, col))

proc newAssemblyResolveError*(message: string, line, col: uint16): ref AssemblyResolveError =
  newError(AssemblyResolveError, withLocation(message, line, col))

proc newAssemblyEmitError*(message: string, line, col: uint16): ref AssemblyEmitError =
  newError(AssemblyEmitError, withLocation(message, line, col))

proc newAssemblyIoError*(message: string): ref AssemblyIoError {.inline.} =
  newError(AssemblyIoError, message)

proc newObjectFormatError*(message: string): ref ObjectFormatError {.inline.} =
  newError(ObjectFormatError, message)

proc newLoggerError*(message: string): ref LoggerError {.inline.} =
  newError(LoggerError, message)

proc newCliError*(message: string): ref CliError {.inline.} =
  newError(CliError, message)