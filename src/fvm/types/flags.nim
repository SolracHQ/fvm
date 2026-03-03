## CPU flags set by comparison and arithmetic instructions.
## Required for CMP, conditional jumps, and overflow detection.

type Flags* = object
  zero*: bool ## Set when the result is zero
  carry*: bool ## Set on unsigned overflow / borrow
  negative*: bool ## Set when the result's high bit is 1

proc clearFlags*(flags: var Flags) =
  flags.zero = false
  flags.carry = false
  flags.negative = false
