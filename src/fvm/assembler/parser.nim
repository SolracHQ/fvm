import ./lexer
import ../types/core

import std/tables

# Types

type
  SectionKind* = enum
    secCode
    secRoData
    secData

  RegArg* = object
    enc*: RegEncoding

  ImmArg* = object
    value*: uint16

  LabelRef* = object
    raw*: string

  ArgKind* = enum
    akReg
    akImm
    akLabelRef

  Arg* = object
    line*: uint16
    col*: uint16
    case kind*: ArgKind
    of akReg:      reg*: RegArg
    of akImm:      imm*: ImmArg
    of akLabelRef: lbl*: LabelRef

  DbItem* = object
    case isStr*: bool
    of true:  bytes*: seq[Byte]
    of false: value*: Byte

  NodeKind* = enum
    nkSection
    nkLabel
    nkInstruction
    nkDb
    nkDw

  Node* = object
    line*: uint16
    col*: uint16
    case kind*: NodeKind
    of nkSection:
      section*: SectionKind
    of nkLabel:
      name*: string
    of nkInstruction:
      mnemonic*: string
      args*: seq[Arg]
    of nkDb:
      dbItems*: seq[DbItem]
    of nkDw:
      dwItems*: seq[uint16]

  Parser* = object
    tokens*: seq[Token]
    pos*: int

# Register name table

const registerNames = block:
  var t = initTable[string, RegEncoding]()
  for i in 0..15:
    let base = "r" & $i
    t[base]       = RegEncoding(Byte(i))
    t[base & "l"] = RegEncoding(RegLaneBit or Byte(i))
    t[base & "h"] = RegEncoding(RegLaneBit or RegHighBit or Byte(i))
  t["sp"] = SpEncoding
  t

proc isRegister(ident: string): bool =
  ident in registerNames

proc parseRegister(ident: string): FvmResult[RegEncoding] =
  if ident in registerNames:
    registerNames[ident].ok
  else:
    ("Unknown register: " & ident).err

# Parser cursor utilities

proc newParser*(tokens: seq[Token]): Parser =
  Parser(tokens: tokens, pos: 0)

proc peek(self: Parser, offset: int = 0): Token =
  let idx = self.pos + offset
  if idx >= self.tokens.len:
    Token(kind: tkEof)
  else:
    self.tokens[idx]

proc check(self: Parser, kind: TokenKind, offset: int = 0): bool =
  self.peek(offset).kind == kind

proc advance(self: var Parser): Token =
  result = self.peek()
  if result.kind != tkEof:
    self.pos += 1

proc expect(self: var Parser, kind: TokenKind): FvmResult[Token] =
  let tok = self.peek()
  if tok.kind != kind:
    return ("Expected " & $kind & " but got " & $tok.kind &
            " at line " & $tok.line & ":" & $tok.col).err
  discard self.advance()
  tok.ok

proc skipNewlines(self: var Parser) =
  while self.check(tkNewline):
    discard self.advance()

proc atEnd(self: Parser): bool =
  self.check(tkEof)

# Argument parsing

proc parseArg(self: var Parser): FvmResult[Arg] =
  let tok = self.peek()
  case tok.kind
  of tkIdent:
    discard self.advance()
    if isRegister(tok.ident):
      let enc = ?parseRegister(tok.ident)
      Arg(line: tok.line, col: tok.col, kind: akReg, reg: RegArg(enc: enc)).ok
    else:
      Arg(line: tok.line, col: tok.col, kind: akLabelRef,
          lbl: LabelRef(raw: tok.ident)).ok
  of tkDot:
    discard self.advance()
    let ident = ?self.expect(tkIdent)
    let raw = "." & ident.ident
    Arg(line: tok.line, col: tok.col, kind: akLabelRef,
        lbl: LabelRef(raw: raw)).ok
  of tkNumber:
    discard self.advance()
    Arg(line: tok.line, col: tok.col, kind: akImm,
        imm: ImmArg(value: tok.number)).ok
  of tkChar:
    discard self.advance()
    Arg(line: tok.line, col: tok.col, kind: akImm,
        imm: ImmArg(value: uint16(tok.ch))).ok
  else:
    ("Unexpected token in argument position: " & $tok.kind &
     " at line " & $tok.line & ":" & $tok.col).err

proc parseArgs(self: var Parser): FvmResult[seq[Arg]] =
  var args: seq[Arg]
  if self.check(tkNewline) or self.check(tkEof):
    return args.ok
  args.add(?self.parseArg())
  while self.check(tkComma):
    discard self.advance()
    args.add(?self.parseArg())
  args.ok

# Data directive parsing

proc parseDbItems(self: var Parser): FvmResult[seq[DbItem]] =
  var items: seq[DbItem]
  while true:
    let tok = self.peek()
    case tok.kind
    of tkString:
      discard self.advance()
      items.add(DbItem(isStr: true, bytes: tok.str))
    of tkNumber:
      discard self.advance()
      if tok.number > 0xFF:
        return ("db value out of byte range: " & $tok.number &
                " at line " & $tok.line & ":" & $tok.col).err
      items.add(DbItem(isStr: false, value: Byte(tok.number)))
    of tkChar:
      discard self.advance()
      items.add(DbItem(isStr: false, value: tok.ch))
    else:
      return ("Unexpected token in db directive: " & $tok.kind &
              " at line " & $tok.line & ":" & $tok.col).err
    if not self.check(tkComma):
      break
    discard self.advance()
  items.ok

proc parseDwItems(self: var Parser): FvmResult[seq[uint16]] =
  var items: seq[uint16]
  while true:
    let tok = self.peek()
    case tok.kind
    of tkNumber:
      discard self.advance()
      items.add(tok.number)
    of tkChar:
      discard self.advance()
      items.add(uint16(tok.ch))
    else:
      return ("Unexpected token in dw directive: " & $tok.kind &
              " at line " & $tok.line & ":" & $tok.col).err
    if not self.check(tkComma):
      break
    discard self.advance()
  items.ok

# Section directive parsing

proc parseSectionDirective(self: var Parser, dotLine: uint16, dotCol: uint16): FvmResult[Node] =
  let ident = ?self.expect(tkIdent)
  case ident.ident
  of "code":
    Node(line: dotLine, col: dotCol, kind: nkSection, section: secCode).ok
  of "rodata":
    Node(line: dotLine, col: dotCol, kind: nkSection, section: secRoData).ok
  of "data":
    Node(line: dotLine, col: dotCol, kind: nkSection, section: secData).ok
  else:
    ("Unknown section directive: ." & ident.ident &
     " at line " & $ident.line & ":" & $ident.col).err

# Top-level line parsing

proc parseLineContent(self: var Parser, nodes: var seq[Node]): FvmResult[void] =
  let tok = self.peek()
  case tok.kind
  of tkIdent:
    discard self.advance()
    case tok.ident
    of "db":
      let items = ?self.parseDbItems()
      nodes.add(Node(line: tok.line, col: tok.col, kind: nkDb, dbItems: items))
    of "dw":
      let items = ?self.parseDwItems()
      nodes.add(Node(line: tok.line, col: tok.col, kind: nkDw, dwItems: items))
    else:
      let args = ?self.parseArgs()
      nodes.add(Node(line: tok.line, col: tok.col, kind: nkInstruction,
                     mnemonic: tok.ident, args: args))
  else:
    return ("Expected instruction or directive at line " & $tok.line &
            ":" & $tok.col & ", got " & $tok.kind).err
  ok()

proc parseLine(self: var Parser): FvmResult[seq[Node]] =
  var nodes: seq[Node]
  let tok = self.peek()

  case tok.kind
  of tkNewline, tkEof:
    discard self.advance()
    return nodes.ok

  of tkDot:
    discard self.advance()
    if self.check(tkIdent) and self.check(tkColon, 1):
      let ident = self.advance()
      discard self.advance() # consume ':'
      nodes.add(Node(line: tok.line, col: tok.col, kind: nkLabel,
                     name: "." & ident.ident))
      # local label may be followed by content on the same line: .loop: INSTR
      if not self.check(tkNewline) and not self.check(tkEof):
        ?self.parseLineContent(nodes)
    else:
      nodes.add(?self.parseSectionDirective(tok.line, tok.col))

  of tkIdent:
    discard self.advance()
    if self.check(tkColon):
      discard self.advance() # consume ':'
      nodes.add(Node(line: tok.line, col: tok.col, kind: nkLabel, name: tok.ident))
      # global label may be followed by content on the same line: msg: db ...
      if not self.check(tkNewline) and not self.check(tkEof):
        ?self.parseLineContent(nodes)
    else:
      case tok.ident
      of "db":
        let items = ?self.parseDbItems()
        nodes.add(Node(line: tok.line, col: tok.col, kind: nkDb, dbItems: items))
      of "dw":
        let items = ?self.parseDwItems()
        nodes.add(Node(line: tok.line, col: tok.col, kind: nkDw, dwItems: items))
      else:
        let args = ?self.parseArgs()
        nodes.add(Node(line: tok.line, col: tok.col, kind: nkInstruction,
                       mnemonic: tok.ident, args: args))

  else:
    let ch = self.advance()
    return ("Unexpected token at line " & $ch.line & ":" & $ch.col &
            ": " & $ch.kind).err

  let eol = self.peek()
  if eol.kind != tkNewline and eol.kind != tkEof:
    return ("Expected newline after statement at line " & $eol.line &
            ":" & $eol.col & ", got " & $eol.kind).err
  if eol.kind == tkNewline:
    discard self.advance()

  nodes.ok

# Entry point

proc parse*(self: var Parser): FvmResult[seq[Node]] =
  var nodes: seq[Node]
  while not self.atEnd():
    self.skipNewlines()
    if self.atEnd():
      break
    let lineNodes = ?self.parseLine()
    nodes.add(lineNodes)
  nodes.ok