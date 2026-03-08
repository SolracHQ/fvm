## VM runtime type declarations.

import ./bus
import ../core/types as coretypes
import ../core/constants as coreconst
import ../core/flags as coreflags

export bus, coretypes, coreconst, coreflags, errors

type
  PortRead* = proc(): FvmResult[Byte] {.closure.} ## Reads one byte from a port device.
  PortWrite* = proc(value: Byte): FvmResult[void] {.closure.}
    ## Writes one byte to a port device.

  PortDevice* = object ## Bound read/write behavior for one I/O port.
    read*: PortRead ## Read callback for IN instructions.
    write*: PortWrite ## Write callback for OUT instructions.
    label*: string ## Human-readable device name.

  Ports* = object ## Port registry for the running VM.
    devices*: array[MaxPortCount, PortDevice] ## Device bound to each port slot.
    mapped*: array[MaxPortCount, bool] ## Whether a port slot is active.

  Vm* = object ## Mutable runtime state for one VM instance.
    bus*: Bus ## Backing memory bus and mapped regions.
    regs*: array[GeneralRegisterCount, Word] ## General-purpose registers.
    ip*: Address ## Instruction pointer.
    sp*: Address ## Stack pointer.
    flags*: Flags ## Arithmetic and comparison flags.
    halted*: bool ## Halt latch set by HALT.
    ports*: Ports ## Registered I/O ports.

  HandlerProc* = proc(vm: var Vm, insn: DecodedInstruction): FvmResult[void]
    ## Executes one decoded instruction.

  InstructionDef* = object ## Executor dispatch entry for one opcode.
    mnemonic*: string ## Human-readable mnemonic.
    handler*: HandlerProc ## Execute-stage procedure for the opcode.

  DecoderProc* = proc(vm: Vm, opcode: OpCode): FvmResult[DecodedInstruction]
    ## Decodes operands for one opcode from the current IP.
