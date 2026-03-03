## CPU flags set by comparison and arithmetic instructions.
## Required for CMP, conditional jumps, and overflow detection.

type 
  Flag* = enum
    Zero ## Set when the result is zero
    Carry ## Set on unsigned overflow / borrow
    Negative ## Set when the result's high bit is 1

  Flags* = set[Flag] ## In nim set of enums are implemented as bitsets, so this is a compact representation.

proc clearFlags*(flags: var Flags) =
  flags = {}

proc setBooleanFlag*(flags: var Flags, flag: Flag, condition: bool) =
  if condition:
    flags.incl(flag)
  else:
    flags.excl(flag)

template zero*(flags: Flags): bool =
  Flag.Zero in flags

template carry*(flags: Flags): bool =
  Flag.Carry in flags

template negative*(flags: Flags): bool =
  Flag.Negative in flags