## I'm not a big fan of utils modules but I really dont know where else to put this
import std/macros

template constArray*[Index, T](name: untyped, body: untyped): untyped =
  ## Declares a compile-time constant array indexed by `Index` with type `T`.
  ##
  ## Nim does not support `[index: value]` syntax for array literals, so
  ## initializing arrays by named index requires a helper. This template
  ## wraps the standard `block` pattern into a more concise form.
  ##
  ## Example:
  ## ```nim
  ## constArray[OpCode, HandlerProc](handlers):
  ##   result[OpCode.NOP] = handleNop
  ##   result[OpCode.ADD] = handleAdd
  ## ```
  ##
  ## The resulting `handlers` is a true `const`, not a `let`, so it lives
  ## in static memory and is available at compile time.
  proc initArray(): array[Index, T] =
    body

  const name = initArray()