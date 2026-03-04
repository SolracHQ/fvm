import ../types/core

import std/strutils

type
  TokenKind* = enum
    tkIdent       # [a-zA-Z_][a-zA-Z0-9_]*
    tkDot         # .
    tkColon       # :
    tkComma       # ,
    tkNewline     # significant: terminates an instruction
    tkNumber      # any numeric literal, stored as raw string for now
    tkString      # "..." content (escape-processed)
    tkChar        # 'x' ascii character literal
    tkEof

  Token* = object
    line*: uint16
    col*: uint16
    case kind*: TokenKind
    of tkIdent:
      ident*: string
    of tkNumber:
      number*: uint16
    of tkString:
      str*: seq[Byte]
    of tkChar:
      ch*: Byte
    else: discard

  Lexer* = object
    input*: string
    pos*: int
    line*: uint16
    col*: uint16

proc newLexer*(input: string): Lexer =
  Lexer(input: input, pos: 0, line: 1, col: 1)


# lexer lookup and movement utilities

proc peek(self: Lexer, offset: int = 0): char =
  if self.pos + offset >= self.input.len:
    '\0'
  else:
    self.input[self.pos + offset]

proc check(self: Lexer, expected: char, offset: int = 0): bool =
  self.peek(offset) == expected

proc advance(self: var Lexer): char =
  result = self.peek()
  if result == '\n':
    self.line += 1
    self.col = 1
  else:
    self.col += 1
  self.pos += 1

proc skipWhitespace(self: var Lexer) =
  while true:
    let ch = self.peek()
    if ch in {' ', '\t', '\r'}:
      discard self.advance()
    elif ch == '#':
      while self.peek() != '\n' and self.peek() != '\0':
        discard self.advance()
    else:
      break

# Numeric utilities for lexing numbers in various formats
template isHexDigit(ch: char): bool =
  ch in {'0'..'9', 'a'..'f', 'A'..'F'}

template isDigit(ch: char): bool =
  ch in {'0'..'9'}

template isOctDigit(ch: char): bool =
  ch in {'0'..'7'}

proc parseAnCheck(s: string, parser: proc(s:string): int, baseName: string): FvmResult[uint16] =
  try:
    let val = parser(s)
    if val < 0 or val > 0xFFFF:
      return ($baseName & " number out of range (must fit in 16 bits): " & s).err
    return uint16(val).ok
  except ValueError as e:
    return ("Invalid " & baseName & " number: " & e.msg).err

proc lexHexNumber(self: var Lexer): FvmResult[uint16] =
  ## Lex a hexadecimal number, starting after "0x" or "0X"
  var number = ""
  while isHexDigit(self.peek()):
    number.add(self.advance())
  if number.len == 0:
    return "Expected hex digits after 0x".err
  parseAnCheck(number, fromHex[int], "Hex")

proc lexDecNumber(self: var Lexer): FvmResult[uint16] =
  ## Lex a decimal number
  var number = ""
  while isDigit(self.peek()):
    number.add(self.advance())
  if number.len == 0:
    return "Expected decimal digits".err
  parseAnCheck(number, parseInt, "Decimal")

proc lexOctNumber(self: var Lexer): FvmResult[uint16] =
  ## Lex an octal number, starting after "0o" or "0O"
  var number = ""
  while isOctDigit(self.peek()):
    number.add(self.advance())
  if number.len == 0:
    return "Expected octal digits after 0o".err
  parseAnCheck(number, fromOct[int], "Octal")

# Identifier utilities
template isIdentStart(ch: char): bool =
  ch in {'a'..'z', 'A'..'Z', '_'}

template isIdentPart(ch: char): bool =
  isIdentStart(ch) or isDigit(ch)

proc lexIdentifier(self: var Lexer): string =
  var ident = ""
  while isIdentPart(self.peek()):
    ident.add(self.advance())
  ident

proc escapeChar(self: var Lexer): FvmResult[Byte] =
  ## Utility to handle next char after a backslash in string/char literals
  let ch = self.advance()
  case ch
  of 'n': Byte(ord '\n').ok
  of 't': Byte(ord '\t').ok
  of 'r': Byte(ord '\r').ok
  of '\\': Byte(ord '\\').ok
  of '"': Byte(ord '"').ok
  of '\'': Byte(ord '\'').ok
  of '0': Byte(0).ok
  else: ("Invalid escape sequence: \\" & $ch).err

proc lexString(self: var Lexer): FvmResult[seq[Byte]] =
  ## Lex a string literal, starting after the opening quote
  var str = newSeq[Byte]()
  while true:
    let ch = self.peek()
    if ch == '\0':
      return "Unterminated string literal".err
    elif ch == '"':
      discard self.advance() # consume closing quote
      break
    elif ch == '\\':
      discard self.advance() # consume backslash
      let esc = ?self.escapeChar()
      str.add(esc)
    else:
      str.add(Byte(ord self.advance()))
  result = str.ok

proc lexChar(self: var Lexer): FvmResult[Byte] =
  ## Lex a character literal, starting after the opening single quote
  let ch = self.peek()
  if ch == '\0':
    return "Unterminated character literal".err
  elif ch == '\\':
    discard self.advance() # consume backslash
    result = ok ?self.escapeChar()
  else:
    result = ok Byte ord self.advance()
  if self.peek() != '\'':
    return "Expected closing single quote for character literal".err
  discard self.advance() # consume closing quote

proc lexNumber(self: var Lexer): FvmResult[uint16] =
  ## Lex a number, which can be in hex (0x), octal (0o), or decimal
  if self.check('0'):
    if self.check('x', 1) or self.check('X', 1):
      discard self.advance() # consume '0'
      discard self.advance() # consume 'x' or 'X'
      return self.lexHexNumber()
    elif self.check('o', 1) or self.check('O', 1):
      discard self.advance() # consume '0'
      discard self.advance() # consume 'o' or 'O'
      return self.lexOctNumber()
  return self.lexDecNumber()

proc nextToken(self: var Lexer): FvmResult[Token] =
  self.skipWhitespace()
  let startLine = self.line
  let startCol = self.col
  case self.peek()
  of '\0':
    Token(line: startLine, col: startCol, kind: tkEof).ok
  of '.':
    discard self.advance()
    Token(line: startLine, col: startCol, kind: tkDot).ok
  of ':':
    discard self.advance()
    Token(line: startLine, col: startCol, kind: tkColon).ok
  of ',':
    discard self.advance()
    Token(line: startLine, col: startCol, kind: tkComma).ok
  of '\n':
    discard self.advance()
    Token(line: startLine, col: startCol, kind: tkNewline).ok
  of '"':
    discard self.advance() # consume opening quote
    Token(line: startLine, col: startCol, kind: tkString, str: ?self.lexString()).ok
  of '\'':
    discard self.advance() # consume opening single quote
    Token(line: startLine, col: startCol, kind: tkChar, ch: ?self.lexChar()).ok
  of '0'..'9':
    Token(line: startLine, col: startCol, kind: tkNumber, number: ?self.lexNumber()).ok
  of 'a'..'z', 'A'..'Z', '_':
    let ident = self.lexIdentifier()
    Token(line: startLine, col: startCol, kind: tkIdent, ident: ident).ok
  else:
    let ch = self.advance()
    ("Unexpected character: " & $ch).err

proc tokenize*(self: var Lexer): FvmResult[seq[Token]] =
  var tokens: seq[Token]
  while true:
    let tok = ?self.nextToken()
    tokens.add(tok)
    if tok.kind == tkEof:
      break
  tokens.ok