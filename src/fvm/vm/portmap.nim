## Port device construction from CLI map specs.
##
## A map spec has the form  port:path:mode:repr
## where all fields after port are optional.

import std/strutils
import std/tables

import ../types/core
import ../types/errors
import ./ports

export ports

type
  MapRepr* = enum
    reprRaw
    reprHex
    reprDec

  MapMode* = enum
    modeIn
    modeOut
    modeBoth

  MapSpec* = object
    port*: Byte
    path*: string
    mode*: MapMode
    repr*: MapRepr

proc parseMapSpec*(raw: string): FvmResult[MapSpec] =
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
  if path == "" or path == "stdin":
    return stdin.ok
  try:
    open(path, fmRead).ok
  except IOError as e:
    e.msg.err

proc resolveOutStream(path: string): FvmResult[File] =
  if path == "" or path == "stdout":
    return stdout.ok
  if path == "stderr":
    return stderr.ok
  try:
    open(path, fmWrite).ok
  except IOError as e:
    e.msg.err

proc makeReadProc(stream: File, repr: MapRepr): proc(): FvmResult[Byte] =
  case repr
  of reprRaw:
    proc(): FvmResult[Byte] =
      var buf: array[1, char]
      if stream.readChars(buf) == 0:
        return "EOF on port read".err
      Byte(buf[0]).ok
  of reprHex:
    proc(): FvmResult[Byte] =
      try:
        let line = stream.readLine().strip()
        if line.len == 0:
          return "EOF on port read".err
        let s =
          if line.startsWith("0x") or line.startsWith("0X"):
            line[2 ..^ 1]
          else:
            line
        Byte(fromHex[int](s)).ok
      except EOFError:
        "EOF on port read".err
      except ValueError as e:
        ("Bad hex input on port read: " & e.msg).err
  of reprDec:
    proc(): FvmResult[Byte] =
      try:
        let line = stream.readLine().strip()
        if line.len == 0:
          return "EOF on port read".err
        let val = parseInt(line)
        if val < 0 or val > 255:
          return ("Dec input out of byte range: " & line).err
        Byte(val).ok
      except EOFError:
        "EOF on port read".err
      except ValueError as e:
        ("Bad decimal input on port read: " & e.msg).err

proc makeWriteProc(stream: File, repr: MapRepr): proc(v: Byte): FvmResult[void] =
  case repr
  of reprRaw:
    proc(v: Byte): FvmResult[void] =
      try:
        stream.write(char(v))
        stream.flushFile()
        ok()
      except IOError as e:
        e.msg.err
  of reprHex:
    proc(v: Byte): FvmResult[void] =
      try:
        stream.write("0x" & toHex(int(v), 2) & "\n")
        stream.flushFile()
        ok()
      except IOError as e:
        e.msg.err
  of reprDec:
    proc(v: Byte): FvmResult[void] =
      try:
        stream.write($int(v) & "\n")
        stream.flushFile()
        ok()
      except IOError as e:
        e.msg.err

proc buildDevices*(specs: seq[MapSpec]): FvmResult[Table[Byte, PortDevice]] =
  # Later specs for the same port and mode overwrite earlier ones.
  var reads: Table[Byte, proc(): FvmResult[Byte]]
  var writes: Table[Byte, proc(v: Byte): FvmResult[void]]

  for spec in specs:
    if spec.mode in {modeIn, modeBoth}:
      reads[spec.port] = makeReadProc(?resolveInStream(spec.path), spec.repr)
    if spec.mode in {modeOut, modeBoth}:
      writes[spec.port] = makeWriteProc(?resolveOutStream(spec.path), spec.repr)

  var devices: Table[Byte, PortDevice]
  var allPorts: seq[Byte]
  for k in reads.keys:
    allPorts.add(k)
  for k in writes.keys:
    if k notin reads:
      allPorts.add(k)

  for port in allPorts:
    let readProc =
      if port in reads:
        reads[port]
      else:
        proc(): FvmResult[Byte] =
          Byte(0).ok
    let writeProc =
      if port in writes:
        writes[port]
      else:
        proc(v: Byte): FvmResult[void] =
          ok()
    devices[port] =
      PortDevice(label: "mapped-port-" & $port, read: readProc, write: writeProc)

  devices.ok

proc applyMaps*(ports: var Ports, maps: seq[string]): FvmResult[void] =
  var specs: seq[MapSpec]
  for raw in maps:
    specs.add(?parseMapSpec(raw))
  let devices = ?buildDevices(specs)
  for port, device in devices:
    ?ports.registerPort(port, device)
  ok()
