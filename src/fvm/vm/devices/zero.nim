## Zero-valued utility device.

import ../../core/types
import ../ports

proc zeroRead*(): FvmResult[Byte] = ## Always reads zero.
  Byte(0).ok

proc zeroWrite*(value: Byte): FvmResult[void] = ## Ignores all output.
  ok()

const ZeroDevice*: PortDevice = PortDevice( ## Stateless sink/source device.
  read: zeroRead, write: zeroWrite, label: "zero-device"
)
