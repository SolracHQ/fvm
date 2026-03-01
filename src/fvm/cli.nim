## CLI entry point: assembler pipeline and VM lifecycle.

import std/os

import ./assembler/assembler
import ./format/fvmobject
import ./vm/core
import ./vm/debug as vmdbg
import ./vm/portmap
import ./logger

# Shared helpers

proc setupVm(obj: FvmObject, maps: seq[string]): FvmResult[Vm] =
  var vm = ?newVm()
  ?vm.ports.applyMaps(maps)
  ?vm.initRom(obj)
  vm.ok

proc execVm(vm: var Vm, step: bool): int =
  if step:
    while not vm.halted:
      info formatIp(vm) & "  " & formatRegisters(vm) & "  " & formatFlags(vm)
      let res = vm.step()
      if res.isErr:
        error "VM error: " & res.error
        return 1
  else:
    let res = vm.run()
    if res.isErr:
      error "VM error: " & res.error
      return 1
  0

proc loadAndRun(
    obj: FvmObject, maps: seq[string], debugLevel: string, step: bool
): int =
  var vm = block:
    let r = setupVm(obj, maps)
    if r.isErr:
      error r.error
      return 1
    r.get()
  execVm(vm, step)

# Commands

proc assemble*(source: string, output = "", debugLevel = "lvlInfo"): int =
  ## Assembles a .fa source file into a .fo object file.
  let loggerResult = initLogger(debugLevel)
  if loggerResult.isErr:
    error loggerResult.error
    return 1
  let res = assembleFile(source)
  if res.isErr:
    error "Assembler error: " & res.error
    return 1
  let outPath =
    if output.len > 0:
      output
    else:
      source.changeFileExt("fo")
  try:
    writeFile(outPath, cast[string](res.get().serialize()))
  except IOError as e:
    error "Write error: " & e.msg
    return 1
  0

proc run*(
    objectFile: string, map: seq[string] = @[], debugLevel = "lvlInfo", step = false
): int =
  ## Runs a .fo object file.
  let loggerResult = initLogger(debugLevel)
  if loggerResult.isErr:
    error loggerResult.error
    return 1
  let bytes =
    try:
      cast[seq[Byte]](readFile(objectFile))
    except IOError as e:
      error "Read error: " & e.msg
      return 1
  let obj = block:
    let r = deserialize(bytes)
    if r.isErr:
      error "Object error: " & r.error
      return 1
    r.get()
  loadAndRun(obj, map, debugLevel, step)

proc runAsm*(
    source: string, map: seq[string] = @[], debugLevel = "lvlError", step = false
): int =
  ## Assembles a .fa source file and runs it without writing an object file.
  let loggerResult = initLogger(debugLevel)
  if loggerResult.isErr:
    error loggerResult.error
    return 1
  let obj = block:
    let r = assembleFile(source)
    if r.isErr:
      error "Assembler error: " & r.error
      return 1
    r.get()
  loadAndRun(obj, map, debugLevel, step)
