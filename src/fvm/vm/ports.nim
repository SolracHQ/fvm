## I/O Port subsystemsupports the IN and OUT instructions.
##
## Each of the 256 ports can have an independent device attached.
## Unregistered ports return a bus error when accessed.
##
## Usage:
##   var dev = PortDevice(
##     read:  proc(): Byte = 42'u8,
##     write: proc(v: Byte) = echo "out: ", v,
##   )
##   vm.registerPort(0, dev)  # attach to port 0

import ./types

export types

proc registerPort*(ports: var Ports, port: Byte, device: PortDevice) =
  if ports.mapped[int(port)]:
    raise newPortRegistrationError(
      "Port " & $port & " is already registered as '" & ports.devices[int(port)].label &
      "'"
    )
  ports.devices[int(port)] = device
  ports.mapped[int(port)] = true

proc portIn*(ports: Ports, port: Byte): Byte =
  if not ports.mapped[int(port)]:
    raise newPortAccessError("IN on unregistered port " & $port)
  ports.devices[int(port)].read()

proc portOut*(ports: Ports, port: Byte, value: Byte) =
  if not ports.mapped[int(port)]:
    raise newPortAccessError("OUT on unregistered port " & $port)
  ports.devices[int(port)].write(value)
