## Vm object definitionlives here so both handlers.nim and core.nim can
## import it without a circular dependency.

import ./bus
import ./ports
import ../types/core
import ../types/flags

export bus, ports, core, flags, errors

type Vm* = object
  bus*: Bus
  regs*: array[GeneralRegisterCount, Word]
  ip*: Address
  sp*: Address
  flags*: Flags
  halted*: bool
  ports*: Ports
