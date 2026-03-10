## CLI entry point: assembler pipeline and VM lifecycle.

import std/os

import ./assembler/assembler
import ./cli/portmap
import ./errors
import ./format/fvmobject
import ./vm/vm
import ./vm/debug as vmdbg
import ./logger

# Shared helpers

proc setupVm(obj: FvmObject, maps: seq[string]): Vm =
  var vm = newVm()
  vm.ports.applyMaps(maps)
  vm.initRom(obj)
  vm

proc execVm(vm: var Vm, step: bool): int =
  try:
    if step:
      while not vm.halted:
        info formatIp(vm) & "  " & formatRegisters(vm) & "  " & formatFlags(vm)
        vm.step()
    else:
      vm.run()
  except FvmError as e:
    error "VM error: " & e.msg
    return 1
  0

proc loadAndRun(
    obj: FvmObject, maps: seq[string], debugLevel: string, step: bool
): int =
  var vm: Vm
  try:
    vm = setupVm(obj, maps)
  except FvmError as e:
    error e.msg
    return 1
  execVm(vm, step)

# Commands

proc assemble*(source: string, output = "", debugLevel = "lvlInfo"): int =
  ## Assembles a .fa source file into a .fo object file.
  try:
    initLogger(debugLevel)
    let obj = assembleFile(source)
    let outPath =
      if output.len > 0:
        output
      else:
        source.changeFileExt("fo")
    writeFile(outPath, cast[string](obj.serialize()))
  except FvmError as e:
    error e.msg
    return 1
  except IOError as e:
    error "Write error: " & e.msg
    return 1
  0

proc run*(
    objectFile: string, map: seq[string] = @[], debugLevel = "lvlInfo", step = false
): int =
  ## Runs a .fo object file.
  let obj =
    try:
      initLogger(debugLevel)
      deserialize(cast[seq[Byte]](readFile(objectFile)))
    except FvmError as e:
      error e.msg
      return 1
    except IOError as e:
      error "Read error: " & e.msg
      return 1
  loadAndRun(obj, map, debugLevel, step)

proc runAsm*(
    source: string, map: seq[string] = @[], debugLevel = "lvlError", step = false
): int =
  ## Assembles a .fa source file and runs it without writing an object file.
  let obj =
    try:
      initLogger(debugLevel)
      assembleFile(source)
    except FvmError as e:
      error e.msg
      return 1
  loadAndRun(obj, map, debugLevel, step)
