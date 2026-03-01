## Unified result type used throughout the entire codebase.
## Replaces the five per-module Result aliases (LexerResult, AssemblerResult,
## VmResult, LoggerResult, OpCodeResult) with one canonical alias so errors
## can cross subsystem boundaries without re-wrapping.

import results

export results

type FvmResult*[T] = Result[T, string]
