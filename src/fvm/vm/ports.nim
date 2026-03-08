## I/O Port subsystemsupports the IN and OUT instructions.
##
## Each of the 256 ports can have an independent device attached.
## Unregistered ports return a bus error when accessed.
##
## Usage:
##   var dev = PortDevice(
##     read:  proc(): FvmResult[Byte]         = (42'u8).ok,
##     write: proc(v: Byte): FvmResult[void]  = echo "out: ", v; ok(),
##   )
##   ?vm.registerPort(0, dev)  # attach to port 0

import ./types

export types

proc registerPort*(ports: var Ports, port: Byte, device: PortDevice): FvmResult[void] =
  if ports.mapped[int(port)]:
    return (
      "Port " & $port & " is already registered as '" & ports.devices[int(port)].label &
      "'"
    ).err
  ports.devices[int(port)] = device
  ports.mapped[int(port)] = true
  ok()

proc portIn*(ports: Ports, port: Byte): FvmResult[Byte] =
  if not ports.mapped[int(port)]:
    return ("IN on unregistered port " & $port).err
  ports.devices[int(port)].read()

proc portOut*(ports: Ports, port: Byte, value: Byte): FvmResult[void] =
  if not ports.mapped[int(port)]:
    return ("OUT on unregistered port " & $port).err
  ports.devices[int(port)].write(value)
