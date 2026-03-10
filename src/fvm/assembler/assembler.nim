import ./lexer
import ./parser
import ./mapper
import ./resolver
import ./emitter
import ../core/types
import ../errors
import ../format/fvmobject

import std/logging

proc assembleSource*(source: string): FvmObject =
  var lexer = newLexer(source)
  let tokens = lexer.tokenize()
  var parser = newParser(tokens)
  let nodes = parser.parse()
  let srcMap = map(nodes)
  let program = resolve(nodes, srcMap)
  emit(program, srcMap.sizes)

proc assembleFile*(path: string, entryPoint: Address = 0'u16): FvmObject =
  ## Reads a `.fa` source file from disk and assembles it.
  debug "Assembling file: " & path
  let source =
    try:
      readFile(path)
    except CatchableError as e:
      raise newAssemblyIoError("Failed to read assembly file '" & path & "': " & e.msg)
  assembleSource(source)
