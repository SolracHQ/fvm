## FVM binary object format produced by the assembler, consumed by the VM.
##
## Layout (big-endian), version 2:
##   Offset  Size  Description
##   ------  ----  ------------------------------------------
##    0       4    Magic bytes: 0x46 0x56 0x4D 0x21  ("FVM!")
##    4       1    Format version (currently 2)
##    5       2    Entry-point address (initial IP)
##    7       2    .rodata section byte count
##    9       2    .code section byte count
##   11       2    .data section byte count
##   13       N    .rodata bytes
##   13+N     M    .code bytes
##   13+N+M   P    .data bytes
##
## Total fixed header size = 13 bytes (FvmHeaderSize constant in core/constants).

import ../core/types as coretypes
import ../core/constants as coreconst

export
  coretypes, coreconst ## re-export so importers can use core object primitives directly

const
  FvmMagic*: array[4, Byte] = [0x46'u8, 0x56, 0x4D, 0x21] ## "FVM!"
  FvmVersion*: Byte = 2

type FvmObject* = object
  version*: Byte ## Format version read from the header
  entryPoint*: Address ## Initial value of the instruction pointer
  rodata*: seq[Byte] ## Read-only constants and strings
  code*: seq[Byte] ## Executable bytecode
  data*: seq[Byte] ## Mutable initialized data
  relocations*: seq[uint16] ## Offsets into .code section that hold 16-bit addresses

# Serialization

proc serialize*(obj: FvmObject): seq[Byte] =
  ## Encodes an FvmObject into a flat byte sequence suitable for writing to disk.
  let rodataLen = uint16(obj.rodata.len)
  let codeLen = uint16(obj.code.len)
  let dataLen = uint16(obj.data.len)
  let relocCount = uint16(obj.relocations.len)
  result = @[
    FvmMagic[0],
    FvmMagic[1],
    FvmMagic[2],
    FvmMagic[3],
    FvmVersion,
    Byte((obj.entryPoint shr 8) and ByteMask),
    Byte(obj.entryPoint and ByteMask),
    Byte((rodataLen shr 8) and ByteMask),
    Byte(rodataLen and ByteMask),
    Byte((codeLen shr 8) and ByteMask),
    Byte(codeLen and ByteMask),
    Byte((dataLen shr 8) and ByteMask),
    Byte(dataLen and ByteMask),
    Byte((relocCount shr 8) and ByteMask),
    Byte(relocCount and ByteMask),
  ]
  result.add(obj.rodata)
  result.add(obj.code)
  result.add(obj.data)
  # Append relocations as 2-byte big-endian values
  for reloc in obj.relocations:
    result.add(Byte((reloc shr 8) and ByteMask))
    result.add(Byte(reloc and ByteMask))

# Deserialization

proc deserialize*(data: openArray[Byte]): FvmResult[FvmObject] =
  ## Validates and decodes a raw byte sequence into an FvmObject.
  if data.len < FvmHeaderSize:
    return (
      "FVM object too short: expected at least " & $FvmHeaderSize & " bytes, got " &
      $data.len
    ).err

  if data[0] != FvmMagic[0] or data[1] != FvmMagic[1] or data[2] != FvmMagic[2] or
      data[3] != FvmMagic[3]:
    return "Invalid FVM magic bytes".err

  let version = data[4]
  if version != FvmVersion:
    return
      ("Unsupported FVM version: " & $version & " (expected " & $FvmVersion & ")").err

  let entryPoint = Address((uint16(data[5]) shl 8) or uint16(data[6]))
  let rodataLen = int((uint16(data[7]) shl 8) or uint16(data[8]))
  let codeLen = int((uint16(data[9]) shl 8) or uint16(data[10]))
  let dataLen = int((uint16(data[11]) shl 8) or uint16(data[12]))
  let relocCount = int((uint16(data[13]) shl 8) or uint16(data[14]))
  let totalPayload = rodataLen + codeLen + dataLen

  if data.len < FvmHeaderSize + totalPayload + (relocCount * 2):
    return (
      "FVM object truncated: header declares " & $totalPayload & " payload bytes and " &
      $relocCount & " relocations but insufficient data present"
    ).err

  let rodataStart = FvmHeaderSize
  let codeStart = rodataStart + rodataLen
  let dataStart = codeStart + codeLen
  let relocStart = dataStart + dataLen

  var relocations: seq[uint16]
  for i in 0 ..< relocCount:
    let reloffset = relocStart + (i * 2)
    let reloc = uint16((uint16(data[reloffset]) shl 8) or uint16(data[reloffset + 1]))
    relocations.add(reloc)

  FvmObject(
    version: version,
    entryPoint: entryPoint,
    rodata: @(data[rodataStart ..< codeStart]),
    code: @(data[codeStart ..< dataStart]),
    data: @(data[dataStart ..< relocStart]),
    relocations: relocations,
  ).ok
