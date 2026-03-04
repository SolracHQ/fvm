import unittest
import fvm/assembler/lexer
import fvm/assembler/parser
import fvm/types/core

# Lexer tests

suite "lexer":
  test "empty input gives only tkEof":
    var lexer = newLexer("")
    let tokens = lexer.tokenize().get()
    require tokens.len == 1
    check tokens[0].kind == tkEof

  test "comment line produces only tkNewline and tkEof":
    var lexer = newLexer("# just a comment\n")
    let tokens = lexer.tokenize().get()
    require tokens.len == 2
    check tokens[0].kind == tkNewline
    check tokens[1].kind == tkEof

  test "blank line produces tkNewline":
    var lexer = newLexer("\n")
    let tokens = lexer.tokenize().get()
    require tokens.len == 2
    check tokens[0].kind == tkNewline

  test "identifier":
    var lexer = newLexer("NOP")
    let tokens = lexer.tokenize().get()
    require tokens.len == 2
    check tokens[0].kind == tkIdent
    check tokens[0].ident == "NOP"

  test "dot token":
    var lexer = newLexer(".")
    let tokens = lexer.tokenize().get()
    require tokens.len == 2
    check tokens[0].kind == tkDot

  test "colon token":
    var lexer = newLexer(":")
    let tokens = lexer.tokenize().get()
    require tokens.len == 2
    check tokens[0].kind == tkColon

  test "comma token":
    var lexer = newLexer(",")
    let tokens = lexer.tokenize().get()
    require tokens.len == 2
    check tokens[0].kind == tkComma

  test "decimal number":
    var lexer = newLexer("42")
    let tokens = lexer.tokenize().get()
    require tokens.len == 2
    check tokens[0].kind == tkNumber
    check tokens[0].number == 42

  test "hex number":
    var lexer = newLexer("0xFF")
    let tokens = lexer.tokenize().get()
    require tokens.len == 2
    check tokens[0].number == 0xFF

  test "octal number":
    var lexer = newLexer("0o10")
    let tokens = lexer.tokenize().get()
    require tokens.len == 2
    check tokens[0].number == 8

  test "number out of 16-bit range is an error":
    var lexer = newLexer("0x10000")
    let res = lexer.tokenize()
    check res.isErr

  test "string literal":
    var lexer = newLexer("\"hi\"")
    let tokens = lexer.tokenize().get()
    require tokens.len == 2
    check tokens[0].kind == tkString
    check tokens[0].str == @[Byte('h'), Byte('i')]

  test "string escape sequences":
    var lexer = newLexer("\"\\n\\t\\0\"")
    let tokens = lexer.tokenize().get()
    check tokens[0].str == @[Byte('\n'), Byte('\t'), Byte(0)]

  test "unterminated string is an error":
    var lexer = newLexer("\"oops")
    let res = lexer.tokenize()
    check res.isErr

  test "char literal":
    var lexer = newLexer("'A'")
    let tokens = lexer.tokenize().get()
    require tokens.len == 2
    check tokens[0].kind == tkChar
    check tokens[0].ch == Byte('A')

  test "char escape":
    var lexer = newLexer("'\\n'")
    let tokens = lexer.tokenize().get()
    check tokens[0].ch == Byte('\n')

  test "unexpected character is an error":
    var lexer = newLexer("@")
    let res = lexer.tokenize()
    check res.isErr

  test "inline comment is consumed before newline":
    var lexer = newLexer("NOP # do nothing\n")
    let tokens = lexer.tokenize().get()
    # NOP tkIdent, tkNewline, tkEof
    require tokens.len == 3
    check tokens[0].kind == tkIdent
    check tokens[1].kind == tkNewline

  test "line and column tracking":
    var lexer = newLexer("NOP\nHALT")
    let tokens = lexer.tokenize().get()
    check tokens[0].line == 1
    check tokens[2].line == 2

# Parser tests

suite "parser: labels":
  test "global label":
    var lexer = newLexer("main:\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 1
    check nodes[0].kind == nkLabel
    check nodes[0].name == "main"

  test "local label":
    var lexer = newLexer(".loop:\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 1
    check nodes[0].kind == nkLabel
    check nodes[0].name == ".loop"

  test "label and instruction on same line":
    var lexer = newLexer("main: NOP\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 2
    check nodes[0].kind == nkLabel
    check nodes[1].kind == nkInstruction
    check nodes[1].mnemonic == "NOP"

  test "label and db directive on same line":
    var lexer = newLexer("msg: db 0x41\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 2
    check nodes[0].kind == nkLabel
    check nodes[1].kind == nkDb

suite "parser: section directives":
  test ".code section":
    var lexer = newLexer(".code\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 1
    check nodes[0].kind == nkSection
    check nodes[0].section == secCode

  test ".rodata section":
    var lexer = newLexer(".rodata\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 1
    check nodes[0].section == secRoData

  test ".data section":
    var lexer = newLexer(".data\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 1
    check nodes[0].section == secData

  test "unknown section is an error":
    var lexer = newLexer(".bss\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let res = parser.parse()
    check res.isErr

suite "parser: instructions":
  test "bare instruction no args":
    var lexer = newLexer("NOP\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 1
    check nodes[0].kind == nkInstruction
    check nodes[0].mnemonic == "NOP"
    check nodes[0].args.len == 0

  test "instruction with register arg":
    var lexer = newLexer("PUSH r0\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes[0].args.len == 1
    check nodes[0].args[0].kind == akReg
    check nodes[0].args[0].reg.enc.index == 0
    check nodes[0].args[0].reg.enc.isWord

  test "instruction with byte-lane register":
    var lexer = newLexer("PUSH r3l\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    check nodes[0].args[0].reg.enc.index == 3
    check nodes[0].args[0].reg.enc.isLow

  test "instruction with sp register":
    var lexer = newLexer("MOV r0, sp\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    check nodes[0].args[1].reg.enc.isSp

  test "instruction with numeric immediate":
    var lexer = newLexer("MOV r0, 42\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    let imm = nodes[0].args[1]
    check imm.kind == akImm
    check imm.imm.value == 42

  test "instruction with char immediate":
    var lexer = newLexer("MOV r0l, 'A'\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    let imm = nodes[0].args[1]
    check imm.kind == akImm
    check imm.imm.value == uint16(ord('A'))

  test "instruction with global label ref":
    var lexer = newLexer("JMP target\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    let lbl = nodes[0].args[0]
    check lbl.kind == akLabelRef
    check lbl.lbl.raw == "target"

  test "instruction with local label ref":
    var lexer = newLexer("JZ .done\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    let lbl = nodes[0].args[0]
    check lbl.kind == akLabelRef
    check lbl.lbl.raw == ".done"

  test "two-arg instruction":
    var lexer = newLexer("MOV r1, r2\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes[0].args.len == 2
    check nodes[0].args[0].reg.enc.index == 1
    check nodes[0].args[1].reg.enc.index == 2

suite "parser: data directives":
  test "db with single byte":
    var lexer = newLexer("db 0x41\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 1
    check nodes[0].kind == nkDb
    require nodes[0].dbItems.len == 1
    check not nodes[0].dbItems[0].isStr
    check nodes[0].dbItems[0].value == 0x41

  test "db with string":
    var lexer = newLexer("db \"hi\"\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    check nodes[0].dbItems[0].isStr
    check nodes[0].dbItems[0].bytes == @[Byte('h'), Byte('i')]

  test "db with string and null terminator":
    var lexer = newLexer("db \"hi\", 0\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes[0].dbItems.len == 2
    check nodes[0].dbItems[1].value == 0

  test "db value out of byte range is an error":
    var lexer = newLexer("db 0x100\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let res = parser.parse()
    check res.isErr

  test "dw with single word":
    var lexer = newLexer("dw 0x1234\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 1
    check nodes[0].kind == nkDw
    check nodes[0].dwItems == @[uint16(0x1234)]

  test "dw with multiple words":
    var lexer = newLexer("dw 1, 2, 3\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    check nodes[0].dwItems == @[uint16(1), uint16(2), uint16(3)]

suite "parser: extra newlines and blank lines":
  test "multiple blank lines between instructions":
    var lexer = newLexer("NOP\n\n\nHALT\n")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 2
    check nodes[0].mnemonic == "NOP"
    check nodes[1].mnemonic == "HALT"

  test "trailing newline is not required":
    var lexer = newLexer("NOP")
    let tokens = lexer.tokenize().get()
    var parser = newParser(tokens)
    let nodes = parser.parse().get()
    require nodes.len == 1
    check nodes[0].mnemonic == "NOP"