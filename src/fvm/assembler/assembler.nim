import ./lexer
import ./parser
import ./mapper
import ./resolver
import ./emitter
import ../core/types
import ../format/fvmobject

import std/logging

proc assembleSource*(source: string): FvmResult[FvmObject] =
  var lexer = newLexer(source)
  let tokens = ?lexer.tokenize()
  var parser = newParser(tokens)
  let nodes = ?parser.parse()
  let srcMap = ?map(nodes)
  let program = ?resolve(nodes, srcMap)
  return emit(program, srcMap.sizes)

proc assembleFile*(path: string, entryPoint: Address = 0'u16): FvmResult[FvmObject] =
  ## Reads a `.fa` source file from disk and assembles it.
  debug "Assembling file: " & path
  let source =
    try:
      readFile(path)
    except CatchableError as e:
      return ("Failed to read assembly file '" & path & "': " & e.msg).err
  assembleSource(source)
