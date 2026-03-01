## Lexer tests

import unittest
import std/sequtils
import fvm/assembler/lexer
import fvm/types/core

suite "tokenizeAssembly":
  test "blank lines and comments are skipped":
    let tokens = tokenizeAssembly("# comment\n\n# another\n").get()
    check tokens.len == 3 # 3 EOL tokens for the blank lines and comment lines

  test "single mnemonic generates mnemonic + eol":
    let tokens = tokenizeAssembly("NOP\n").get()
    require tokens.len == 2
    check tokens[0].kind == TkMnemonic
    check tokens[0].mnemonic == "NOP"
    check tokens[1].kind == TkEol

  test "plain register r0":
    let tokens = tokenizeAssembly("PUSH r0\n").get()
    require tokens.len == 3
    check tokens[1].kind == TkRegister
    check tokens[1].regEncoding.index == 0
    check tokens[1].regEncoding.isWord

  test "plain register r15":
    let tokens = tokenizeAssembly("PUSH r15\n").get()
    require tokens.len == 3
    check tokens[1].regEncoding.index == 15
    check tokens[1].regEncoding.isWord

  test "low byte-lane r0l":
    let tokens = tokenizeAssembly("PUSH r0l\n").get()
    require tokens.len == 3
    check tokens[1].regEncoding.index == 0
    check tokens[1].regEncoding.isLow

  test "high byte-lane r0h":
    let tokens = tokenizeAssembly("PUSH r0h\n").get()
    require tokens.len == 3
    check tokens[1].regEncoding.index == 0
    check tokens[1].regEncoding.isHigh

  test "low byte-lane r15l":
    let tokens = tokenizeAssembly("PUSH r15l\n").get()
    require tokens.len == 3
    check tokens[1].regEncoding.index == 15
    check tokens[1].regEncoding.isLow

  test "high byte-lane r15h":
    let tokens = tokenizeAssembly("PUSH r15h\n").get()
    require tokens.len == 3
    check tokens[1].regEncoding.index == 15
    check tokens[1].regEncoding.isHigh

  test "decimal immediate":
    let tokens = tokenizeAssembly("MOV r0, 42\n").get()
    let imm = tokens.filterIt(it.kind == TkImmediate)
    require imm.len == 1
    check imm[0].immValue == 42

  test "hex immediate":
    let tokens = tokenizeAssembly("MOV r0, 0xFF\n").get()
    let imm = tokens.filterIt(it.kind == TkImmediate)
    check imm[0].immValue == 0xFF

  test "octal immediate":
    let tokens = tokenizeAssembly("MOV r0, 0o10\n").get()
    let imm = tokens.filterIt(it.kind == TkImmediate)
    check imm[0].immValue == 8

  test "comma produces TkComma":
    let tokens = tokenizeAssembly("MOV r0, 1\n").get()
    check tokens.anyIt(it.kind == TkComma)

  test "unknown register returns error":
    let res = tokenizeAssembly("PUSH r99\n")
    check res.isErr
