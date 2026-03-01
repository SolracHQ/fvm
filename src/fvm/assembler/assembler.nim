## Assembler public facade
##
## Composes the three assembler pipeline stages into a single easy entry point:
##   tokenizeAssembly -> parseTokens -> emitBytecode -> FvmObject
##
## The assembler produces an FvmObject (with the FVM header) rather than a raw
## byte sequence.  The VM's `initRom` consumes an FvmObject directly.

import std/logging

import ../types/core
import ../types/errors
import ../format/fvmobject
import ./lexer
import ./parser
import ./emitter

export fvmobject ## re-export so importers see FvmObject without extra imports

proc assembleSource*(
    source: string, entryPoint: Address = 0'u16
): FvmResult[FvmObject] =
  ## Assembles a string of Fantasy Assembly source code.
  ## When entryPoint is zero (the default) the initial IP is set to the start
  ## of the .code section, which follows any .rodata bytes.
  let tokens = (?tokenizeAssembly(source))
  let output = (?parseTokens(tokens))
  let code = (?emitBytecode(output.instructions))
  let ep =
    if entryPoint == 0:
      Address(output.rodata.len)
    else:
      entryPoint

  FvmObject(
    version: FvmVersion,
    entryPoint: ep,
    rodata: output.rodata,
    code: code,
    data: output.data,
  ).ok

proc assembleFile*(path: string, entryPoint: Address = 0'u16): FvmResult[FvmObject] =
  ## Reads a `.fa` source file from disk and assembles it.
  debug "Assembling file: " & path
  let source =
    try:
      readFile(path)
    except CatchableError as e:
      return ("Failed to read assembly file '" & path & "': " & e.msg).err
  assembleSource(source, entryPoint)
