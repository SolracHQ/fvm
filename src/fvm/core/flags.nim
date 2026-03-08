import ./types

export types

proc clearFlags*(flags: var Flags) =
  ## Clears all CPU flags.
  flags = {}

proc setBooleanFlag*(flags: var Flags, flag: Flag, condition: bool) =
  ## Sets or clears a flag based on the provided condition.
  if condition:
    flags.incl(flag)
  else:
    flags.excl(flag)

template zero*(flags: Flags): bool =
  ## Returns true when the zero flag is set.
  Flag.Zero in flags

template carry*(flags: Flags): bool =
  ## Returns true when the carry flag is set.
  Flag.Carry in flags

template negative*(flags: Flags): bool =
  ## Returns true when the negative flag is set.
  Flag.Negative in flags
