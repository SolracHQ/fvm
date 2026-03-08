## CLI-side map parsing and device wiring.

import std/strutils
import std/tables

import ../core/types
import ../vm/ports
import ../vm/devices/zero
import ../vm/devices/hexfile

type
  MapRepr* = enum ## CLI wire format used by a mapped port.
    reprRaw
    reprHex
    reprDec

  MapMode* = enum ## Which directions are enabled for a mapped port.
    modeIn
    modeOut
    modeBoth

  MapSpec* = object ## Parsed `--map` specification.
    port*: Byte
    path*: string
    mode*: MapMode
    repr*: MapRepr

proc parseMapSpec*(raw: string): FvmResult[MapSpec] =
  ## Parses `port:path:mode:repr` into a structured map specification.
  let parts = raw.split(':')
  if parts.len < 1 or parts.len > 4:
    return ("Invalid --map spec, expected port:path:mode:repr, got: " & raw).err

  let portVal =
    try:
      parseInt(parts[0])
    except ValueError:
      -1
  if portVal < 0 or portVal > 255:
    return ("Invalid port in --map spec: " & parts[0]).err

  let path =
    if parts.len >= 2:
      parts[1]
    else:
      ""

  let mode =
    if parts.len >= 3 and parts[2].len > 0:
      case parts[2]
      of "in":
        modeIn
      of "out":
        modeOut
      of "both":
        modeBoth
      else:
        return ("Invalid mode in --map spec: " & parts[2]).err
    else:
      modeBoth

  let repr =
    if parts.len >= 4 and parts[3].len > 0:
      case parts[3]
      of "raw":
        reprRaw
      of "hex":
        reprHex
      of "dec":
        reprDec
      else:
        return ("Invalid repr in --map spec: " & parts[3]).err
    else:
      reprRaw

  MapSpec(port: Byte(portVal), path: path, mode: mode, repr: repr).ok

proc resolveInStream(path: string): FvmResult[File] =
  ## Opens an input stream for the requested path or stdin alias.
  if path == "" or path == "stdin":
    return stdin.ok
  try:
    open(path, fmRead).ok
  except IOError as e:
    e.msg.err

proc resolveOutStream(path: string): FvmResult[File] =
  ## Opens an output stream for the requested path or stdio alias.
  if path == "" or path == "stdout":
    return stdout.ok
  if path == "stderr":
    return stderr.ok
  try:
    open(path, fmWrite).ok
  except IOError as e:
    e.msg.err

proc rawRead(stream: File): PortRead =
  ## Creates a raw byte reader backed by a stream.
  proc(): FvmResult[Byte] =
    var buf: array[1, char]
    if stream.readChars(buf) == 0:
      return "EOF on port read".err
    Byte(buf[0]).ok

proc rawWrite(stream: File): PortWrite =
  ## Creates a raw byte writer backed by a stream.
  proc(value: Byte): FvmResult[void] =
    try:
      stream.write(char(value))
      stream.flushFile()
      ok()
    except IOError as e:
      e.msg.err

proc decRead(stream: File): PortRead =
  ## Creates a decimal text reader backed by a stream.
  proc(): FvmResult[Byte] =
    try:
      let line = stream.readLine().strip()
      if line.len == 0:
        return "EOF on port read".err
      let value = parseInt(line)
      if value < 0 or value > 255:
        return ("Dec input out of byte range: " & line).err
      Byte(value).ok
    except EOFError:
      "EOF on port read".err
    except ValueError as e:
      ("Bad decimal input on port read: " & e.msg).err

proc decWrite(stream: File): PortWrite =
  ## Creates a decimal text writer backed by a stream.
  proc(value: Byte): FvmResult[void] =
    try:
      stream.write($int(value) & "\n")
      stream.flushFile()
      ok()
    except IOError as e:
      e.msg.err

proc inputProc(spec: MapSpec): FvmResult[PortRead] =
  ## Builds the input side of a port mapping from one map specification.
  let stream = ?resolveInStream(spec.path)
  case spec.repr
  of reprRaw:
    rawRead(stream).ok
  of reprHex:
    hexFileRead(stream).ok
  of reprDec:
    decRead(stream).ok

proc outputProc(spec: MapSpec): FvmResult[PortWrite] =
  ## Builds the output side of a port mapping from one map specification.
  let stream = ?resolveOutStream(spec.path)
  case spec.repr
  of reprRaw:
    rawWrite(stream).ok
  of reprHex:
    hexFileWrite(stream).ok
  of reprDec:
    decWrite(stream).ok

proc buildDevices*(specs: seq[MapSpec]): FvmResult[Table[Byte, PortDevice]] =
  ## Merges CLI map specifications into concrete port devices.
  var reads: Table[Byte, PortRead]
  var writes: Table[Byte, PortWrite]

  for spec in specs:
    if spec.mode in {modeIn, modeBoth}:
      reads[spec.port] = ?inputProc(spec)
    if spec.mode in {modeOut, modeBoth}:
      writes[spec.port] = ?outputProc(spec)

  var devices: Table[Byte, PortDevice]
  var allPorts: seq[Byte]
  for port in reads.keys:
    allPorts.add(port)
  for port in writes.keys:
    if port notin reads:
      allPorts.add(port)

  for port in allPorts:
    let readProc =
      if port in reads:
        reads[port]
      else:
        ZeroDevice.read
    let writeProc =
      if port in writes:
        writes[port]
      else:
        ZeroDevice.write
    devices[port] =
      PortDevice(label: "mapped-port-" & $port, read: readProc, write: writeProc)

  devices.ok

proc applyMaps*(ports: var Ports, maps: seq[string]): FvmResult[void] =
  ## Parses and applies all CLI port mappings to a VM instance.
  var specs: seq[MapSpec]
  for raw in maps:
    specs.add(?parseMapSpec(raw))
  let devices = ?buildDevices(specs)
  for port, device in devices:
    ?ports.registerPort(port, device)
  ok()
