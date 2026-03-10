## Logging configuration.
##
## The console handler is NOT installed on import, call `initLogger()` to
## set it up.  This lets test code import this module without any side-effects.

import std/logging
import std/strutils

import errors

export logging ## re-export so importers can use `debug`, `info`, etc.

var consoleLogger: ConsoleLogger ## nil until initLogger() is called

proc parseDebugLevel*(value: string): Level =
  let normalized = value.strip().toLowerAscii()
  case normalized
  of "lvldebug", "debug":
    lvlDebug
  of "lvlinfo", "info":
    lvlInfo
  of "lvlnotice", "notice":
    lvlNotice
  of "lvlwarn", "warn", "warning":
    lvlWarn
  of "lvlerror", "error":
    lvlError
  of "lvlfatal", "fatal":
    lvlFatal
  of "lvlnone", "none":
    lvlNone
  else:
    raise newLoggerError("Invalid debug level '" & value & "'")

proc setDebugLevel*(level: Level) =
  setLogFilter(level)

proc initLogger*(level: string) =
  ## Installs the console log handler.  Call exactly once at program startup
  ## (i.e. from the CLI entry point).  Idempotent: a second call is a no-op.
  if consoleLogger == nil:
    consoleLogger = newConsoleLogger()
    addHandler(consoleLogger)
    setDebugLevel(parseDebugLevel(level))
