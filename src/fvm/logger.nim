## Logging configuration.
##
## The console handler is NOT installed on import, call `initLogger()` to
## set it up.  This lets test code import this module without any side-effects.

import std/logging
import std/strutils

import core/types

export logging ## re-export so importers can use `debug`, `info`, etc.

var consoleLogger: ConsoleLogger ## nil until initLogger() is called

proc parseDebugLevel*(value: string): FvmResult[Level] =
  let normalized = value.strip().toLowerAscii()
  case normalized
  of "lvldebug", "debug":
    lvlDebug.ok
  of "lvlinfo", "info":
    lvlInfo.ok
  of "lvlnotice", "notice":
    lvlNotice.ok
  of "lvlwarn", "warn", "warning":
    lvlWarn.ok
  of "lvlerror", "error":
    lvlError.ok
  of "lvlfatal", "fatal":
    lvlFatal.ok
  of "lvlnone", "none":
    lvlNone.ok
  else:
    ("Invalid debug level '" & value & "'").err

proc setDebugLevel*(level: Level) =
  setLogFilter(level)

proc initLogger*(level: string): FvmResult[void] =
  ## Installs the console log handler.  Call exactly once at program startup
  ## (i.e. from the CLI entry point).  Idempotent: a second call is a no-op.
  if consoleLogger == nil:
    consoleLogger = newConsoleLogger()
    addHandler(consoleLogger)
    let levelResult = parseDebugLevel(level)
    if levelResult.isErr:
      return ("Invalid debug level: " & levelResult.error).err
    setDebugLevel(levelResult.get())
  ok()
