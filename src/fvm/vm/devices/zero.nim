## Zero-valued utility device.

import ../../core/types
import ../ports

proc zeroRead*(): Byte = ## Always reads zero.
  Byte(0)

proc zeroWrite*(value: Byte) = ## Ignores all output.
  discard

const ZeroDevice*: PortDevice = PortDevice( ## Stateless sink/source device.
  read: zeroRead, write: zeroWrite, label: "zero-device"
)
