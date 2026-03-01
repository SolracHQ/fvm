## Fantasy Assembly Lexer
##
## Converts raw source text into a flat sequence of AsmTokens.

import std/strutils

import ../types/core
import ../types/errors

export errors ## re-export so importers can use FvmResult and results procs directly

type
  TokenKind* = enum
    TkMnemonic
    TkRegister
    TkImmediate
    TkStringLit ## string literal in double quotes, used by db
    TkComma
    TkEol
    TkLabel ## label definition: identifier followed by ':'

  AsmToken* = object
    line*: int
    column*: int
    case kind*: TokenKind
    of TkMnemonic:
      mnemonic*: string
    of TkRegister:
      regEncoding*: RegEncoding
        ## full operand byte: lane bits (7:6) + reserved (5:4) + index (3:0)
      regSource*: string ## original text; used in error messages
    of TkImmediate:
      immValue*: int
    of TkStringLit:
      strValue*: string
    of TkLabel:
      labelName*: string ## bare name without the colon; local labels start with '.'
    of TkComma, TkEol:
      discard

  Lexer* = object
    source*: string
    current*: int
    line*: int
    column*: int

# Lexer state management

proc isAtEnd(lexer: Lexer): bool =
  lexer.current >= lexer.source.len

proc peek(lexer: Lexer): char =
  if lexer.isAtEnd:
    '\0'
  else:
    lexer.source[lexer.current]

proc peekNext(lexer: Lexer): char =
  if lexer.current + 1 >= lexer.source.len:
    '\0'
  else:
    lexer.source[lexer.current + 1]

proc advance(lexer: var Lexer): char =
  result = lexer.peek()
  if result == '\n':
    inc lexer.line
    lexer.column = 0
  else:
    inc lexer.column
  inc lexer.current

proc skipWhitespace(lexer: var Lexer) =
  while not lexer.isAtEnd:
    let ch = lexer.peek()
    case ch
    of ' ', '\t', '\r':
      discard lexer.advance()
    else:
      break

proc skipComment(lexer: var Lexer) =
  ## Skips a comment starting with '#' until end of line.
  if lexer.peek() == '#':
    while not lexer.isAtEnd and lexer.peek() != '\n':
      discard lexer.advance()

# Token parsing

proc parseIdentifier(lexer: var Lexer): string =
  ## Reads an identifier or keyword (starts with letter or underscore).
  var ident = ""
  while not lexer.isAtEnd and lexer.peek() in {'a' .. 'z', 'A' .. 'Z', '0' .. '9', '_'}:
    ident.add(lexer.advance())
  ident

proc parseNumber(lexer: var Lexer): FvmResult[int] =
  ## Parses decimal, hex (0x), or octal (0o) immediate.
  var numStr = ""

  # Check for hex or octal prefix
  if lexer.peek() == '0' and not lexer.isAtEnd:
    numStr.add(lexer.advance())
    if not lexer.isAtEnd:
      let next = lexer.peek()
      if next == 'x' or next == 'X':
        numStr.add(lexer.advance())
        # Read hex digits
        while not lexer.isAtEnd and lexer.peek() in {'0' .. '9', 'a' .. 'f', 'A' .. 'F'}:
          numStr.add(lexer.advance())
        if numStr.len < 3:
          return ("Invalid hex immediate: " & numStr).err
        try:
          return parseHexInt(numStr[2 .. ^1]).ok
        except ValueError:
          return ("Invalid hex immediate: " & numStr).err
      elif next == 'o' or next == 'O':
        numStr.add(lexer.advance())
        # Read octal digits
        while not lexer.isAtEnd and lexer.peek() in {'0' .. '7'}:
          numStr.add(lexer.advance())
        if numStr.len < 3:
          return ("Invalid octal immediate: " & numStr).err
        try:
          return parseOctInt(numStr[2 .. ^1]).ok
        except ValueError:
          return ("Invalid octal immediate: " & numStr).err

  # Parse remaining decimal digits
  while not lexer.isAtEnd and lexer.peek() in {'0' .. '9'}:
    numStr.add(lexer.advance())

  if numStr.len == 0:
    return ("Empty immediate").err

  try:
    return parseInt(numStr).ok
  except ValueError:
    ("Invalid immediate: " & numStr).err

proc parseRegisterToken(regText: string, line: int, column: int): FvmResult[AsmToken] =
  let lower = regText.toLowerAscii()

  if lower.len >= 2 and lower[0] == 'r':
    var indexText = lower[1 .. ^1]
    var laneBits = 0'u8

    # Strip byte-lane suffix: l = low byte, h = high byte
    if indexText.endsWith("l"):
      laneBits = RegLaneBit
      indexText = indexText[0 .. ^2]
    elif indexText.endsWith("h"):
      laneBits = RegLaneBit or RegHighBit
      indexText = indexText[0 .. ^2]

    try:
      let idx = parseInt(indexText)
      if idx < 0 or idx >= GeneralRegisterCount:
        return ("Register out of range: " & regText).err
      return
        AsmToken(
          line: line,
          column: column,
          kind: TkRegister,
          regEncoding: RegEncoding(laneBits or Byte(idx)),
          regSource: lower,
        ).ok
    except ValueError:
      discard

  ("Invalid register: " & regText).err

proc parseImmediateToken(immValue: int, line: int, column: int): AsmToken =
  AsmToken(line: line, column: column, kind: TkImmediate, immValue: immValue)

# Main tokenization

proc tokenizeAssembly*(source: string): FvmResult[seq[AsmToken]] =
  var lexer = Lexer(source: source, current: 0, line: 1, column: 0)
  var tokens: seq[AsmToken]

  while not lexer.isAtEnd:
    lexer.skipWhitespace()

    let startLine = lexer.line
    let startColumn = lexer.column

    if lexer.isAtEnd:
      break

    let ch = lexer.peek()

    # Handle end of line (implicit token)
    if ch == '\n':
      discard lexer.advance()
      tokens.add(AsmToken(line: startLine, column: startColumn, kind: TkEol))
      continue

    # Handle comments (skip to end of line)
    if ch == '#':
      lexer.skipComment()
      continue

    # Handle comma
    if ch == ',':
      discard lexer.advance()
      tokens.add(AsmToken(line: startLine, column: startColumn, kind: TkComma))
      continue

    # Handle number (immediate)
    if ch in {'0' .. '9'}:
      let immResult = lexer.parseNumber()
      if immResult.isErr:
        return immResult.error.err
      tokens.add(parseImmediateToken(immResult.get(), startLine, startColumn))
      continue

    # Handle character literal: 'X' or escape sequences '\n', '\t', '\0', '\''
    if ch == '\'':
      discard lexer.advance()
      if lexer.isAtEnd:
        return ("Unterminated character literal at line " & $startLine).err
      let inner = lexer.advance()
      let charVal =
        if inner == '\\':
          if lexer.isAtEnd:
            return
              ("Unterminated escape in character literal at line " & $startLine).err
          let esc = lexer.advance()
          case esc
          of 'n':
            int('\n')
          of 't':
            int('\t')
          of '0':
            0
          of '\\':
            int('\\')
          of '\'':
            int('\'')
          else:
            return
              ("Unknown escape sequence '\\" & $esc & "' at line " & $startLine).err
        else:
          int(inner)
      if lexer.isAtEnd or lexer.peek() != '\'':
        return
          ("Expected closing \"'\" for character literal at line " & $startLine).err
      discard lexer.advance()
      tokens.add(parseImmediateToken(charVal, startLine, startColumn))
      continue

    # Handle string literal: "..."
    if ch == '"':
      discard lexer.advance()
      var str = ""
      while not lexer.isAtEnd and lexer.peek() != '"':
        let sc = lexer.advance()
        if sc == '\n':
          return ("Unterminated string literal at line " & $startLine).err
        if sc == '\\':
          if lexer.isAtEnd:
            return ("Unterminated escape in string literal at line " & $startLine).err
          let esc = lexer.advance()
          case esc
          of 'n':
            str.add('\n')
          of 't':
            str.add('\t')
          of '0':
            str.add('\0')
          of '\\':
            str.add('\\')
          of '"':
            str.add('"')
          else:
            return (
              "Unknown escape '\\" & $esc & "' in string literal at line " & $startLine
            ).err
        else:
          str.add(sc)
      if lexer.isAtEnd:
        return ("Unterminated string literal at line " & $startLine).err
      discard lexer.advance() # closing '"'
      tokens.add(
        AsmToken(line: startLine, column: startColumn, kind: TkStringLit, strValue: str)
      )
      continue

    # Handle dot-prefixed local label definition or local label reference
    if ch == '.':
      discard lexer.advance()
      let ident = lexer.parseIdentifier()
      if ident.len == 0:
        return ("Empty identifier after '.' at line " & $startLine).err
      let name = "." & ident
      if lexer.peek() == ':':
        discard lexer.advance()
        tokens.add(
          AsmToken(line: startLine, column: startColumn, kind: TkLabel, labelName: name)
        )
      else:
        # Local label reference used as a jump operand
        tokens.add(
          AsmToken(
            line: startLine, column: startColumn, kind: TkMnemonic, mnemonic: name
          )
        )
      continue

    # Handle identifier (mnemonic, register, or label definition)
    if ch in {'a' .. 'z', 'A' .. 'Z', '_'}:
      let ident = lexer.parseIdentifier()
      let lower = ident.toLowerAscii()

      # Label definition: identifier followed by ':'
      if lexer.peek() == ':':
        discard lexer.advance()
        tokens.add(
          AsmToken(
            line: startLine, column: startColumn, kind: TkLabel, labelName: ident
          )
        )
        continue

      # SP register
      if lower == "sp":
        tokens.add(
          AsmToken(
            line: startLine,
            column: startColumn,
            kind: TkRegister,
            regEncoding: SpEncoding,
            regSource: "sp",
          )
        )
        continue

      # Try to parse as register: must be 'r' followed immediately by a digit
      if lower.len >= 2 and lower[0] == 'r' and lower[1] in {'0' .. '9'}:
        let regResult = parseRegisterToken(ident, startLine, startColumn)
        if regResult.isErr:
          return regResult.error.err
        tokens.add(regResult.get())
      else:
        # Treat as mnemonic
        tokens.add(
          AsmToken(
            line: startLine, column: startColumn, kind: TkMnemonic, mnemonic: ident
          )
        )
      continue

    # Unknown character
    return (
      "Unexpected character '" & ch & "' at line " & $startLine & ", column " &
      $startColumn
    ).err

  # Add final EOL if not already at end with EOL
  if tokens.len == 0 or tokens[^1].kind != TkEol:
    tokens.add(AsmToken(line: lexer.line, column: lexer.column, kind: TkEol))

  tokens.ok
