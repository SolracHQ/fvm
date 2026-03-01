import cligen
import fvm/cli

when isMainModule:
  dispatchMulti(
    [assemble, cmdName = "assemble"],
    [run, cmdName = "run"],
    [runAsm, cmdName = "run-asm"],
  )
