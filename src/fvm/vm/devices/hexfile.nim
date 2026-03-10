## Hex-encoded file-backed port device helpers.

import std/strutils

import ../../core/types
import ../../errors
import ../ports
import ./zero

proc hexFileRead*(stream: File): PortRead =
  ## Creates a port reader that consumes one hexadecimal byte per line.
  proc(): Byte =
    try:
      let line = stream.readLine().strip()
      if line.len == 0:
        raise newPortEofError()
      let digits =
        if line.startsWith("0x") or line.startsWith("0X"):
          line[2 ..^ 1]
        else:
          line
      Byte(fromHex[int](digits))
    except EOFError:
      raise newPortEofError()
    except ValueError as e:
      raise newPortValueError("Bad hex input on port read: " & e.msg)

proc hexFileWrite*(stream: File): PortWrite =
  ## Creates a port writer that emits one hexadecimal byte per line.
  proc(value: Byte) =
    try:
      stream.write("0x" & toHex(int(value), 2) & "\n")
      stream.flushFile()
    except IOError as e:
      raise newPortIoError(e.msg)

proc hexFileReadDevice*(stream: File, label = "hex-file-read"): PortDevice =
  ## Creates a read-focused device backed by a hex-encoded input file.
  PortDevice(read: hexFileRead(stream), write: zeroWrite, label: label)

proc hexFileWriteDevice*(stream: File, label = "hex-file-write"): PortDevice =
  ## Creates a write-focused device backed by a hex-encoded output file.
  PortDevice(read: zeroRead, write: hexFileWrite(stream), label: label)

proc hexFileDevice*(input: File, output: File, label = "hex-file-device"): PortDevice =
  ## Creates a read/write device backed by hex-encoded input and output files.
  PortDevice(read: hexFileRead(input), write: hexFileWrite(output), label: label)
