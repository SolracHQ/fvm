## I'm not a big fan of utils modules but I really dont know where else to put this
import results
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

macro defaultOk*(procDef: untyped): untyped =
  ## A macro to simplify the definition of procedures that always return `Ok`.
  ##
  ## This is useful for stubbing out functions during development or when
  ## the implementation is trivial. It eliminates boilerplate by automatically
  ## wrapping the procedure body in a `result = Ok()` statement.
  ##
  ## Example:
  ## ```nim
  ## proc doNothing(): Result[void] {.defaultOk.} =
  ##   # No implementation needed, it always returns Ok
  ## ```
  result = procDef
  let oldBody = result.body
  result.body = quote:
    result = ok()
    `oldBody`
