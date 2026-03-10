import ./types
import ./constants
import ../errors

export types, constants

proc newRegEncoding*(
    index: int, lane: bool = false, high: bool = false, isSp: bool = false
): RegEncoding =
  ## Builds a register encoding from the logical register selection bits.
  if isSp:
    if lane or high:
      raise newRegisterEncodingError("SP encoding cannot specify lane or high byte")
    return SpEncoding

  if index < 0 or index >= GeneralRegisterCount:
    raise newRegisterEncodingError("Register index out of range: " & $index)

  var encoding = RegEncoding(Byte(index) and RegIndexMask)
  if lane:
    encoding = RegEncoding(Byte(encoding) or RegLaneBit)
  if high:
    encoding = RegEncoding(Byte(encoding) or RegHighBit)
  encoding

proc `==`*(a, b: RegEncoding): bool {.borrow.}

proc isSp*(r: RegEncoding): bool =
  ## Returns true when the encoding designates the stack pointer.
  (Byte(r) and (RegLaneBit or RegHighBit)) == RegHighBit

proc isLane*(r: RegEncoding): bool =
  ## Returns true when the encoding selects a byte lane instead of a word.
  (Byte(r) and RegLaneBit) != 0

proc isHigh*(r: RegEncoding): bool =
  ## Returns true when the encoding selects the high byte lane.
  (Byte(r) and (RegLaneBit or RegHighBit)) == (RegLaneBit or RegHighBit)

proc isLow*(r: RegEncoding): bool =
  ## Returns true when the encoding selects the low byte lane.
  r.isLane and not r.isHigh

proc isWord*(r: RegEncoding): bool =
  ## Returns true when the encoding refers to the full-width register.
  not r.isLane and not r.isSp

proc index*(r: RegEncoding): int =
  ## Extracts the logical register index from the encoding.
  int(Byte(r) and RegIndexMask)

proc laneTag*(r: RegEncoding): Byte =
  ## Returns the lane bits used to compare operand widths.
  if r.isSp:
    0'u8
  else:
    Byte(r) and (RegLaneBit or RegHighBit)
