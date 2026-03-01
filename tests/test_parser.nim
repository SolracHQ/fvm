## Assembler parser tests

import unittest
import results
import fvm/assembler/lexer
import fvm/assembler/parser
import fvm/types/opcodes
import fvm/types/core

proc parse(source: string): seq[Instruction] =
  let tokens = tokenizeAssembly(source).get()
  parseTokens(tokens).get().instructions

suite "parseTokens":
  test "NOP produces opcode Nop with no operands":
    let instrs = parse("NOP\n")
    require instrs.len == 1
    check instrs[0].opcode == OpCode.Nop
    check instrs[0].operands.len == 0

  test "HALT produces opcode Halt":
    let instrs = parse("HALT\n")
    require instrs.len == 1
    check instrs[0].opcode == OpCode.Halt

  test "PUSH r0 produces Push opcode with encoding byte 0":
    let instrs = parse("PUSH r0\n")
    require instrs.len == 1
    check instrs[0].opcode == OpCode.Push
    check instrs[0].operands == @[0'u8]

  test "PUSH r0l produces Push with low-lane encoding":
    let instrs = parse("PUSH r0l\n")
    require instrs.len == 1
    check instrs[0].opcode == OpCode.Push
    check instrs[0].operands == @[RegLaneBit]

  test "POP r3 produces Pop opcode with encoding byte 3":
    let instrs = parse("POP r3\n")
    check instrs[0].opcode == OpCode.Pop
    check instrs[0].operands == @[3'u8]

  test "MOV r0, 42 produces MovRegImm with 16-bit big-endian immediate":
    let instrs = parse("MOV r0, 42\n")
    check instrs[0].opcode == OpCode.MovRegImm
    check instrs[0].operands == @[0'u8, 0'u8, 42'u8]

  test "MOV r0l, 42 produces MovRegImm with 8-bit immediate":
    let instrs = parse("MOV r0l, 42\n")
    check instrs[0].opcode == OpCode.MovRegImm
    check instrs[0].operands == @[RegLaneBit, 42'u8]

  test "MOV r1, r2 produces MovRegReg":
    let instrs = parse("MOV r1, r2\n")
    check instrs[0].opcode == OpCode.MovRegReg
    check instrs[0].operands == @[1'u8, 2'u8]

  test "MOV r0l, r1l produces MovRegReg (both low lane)":
    let instrs = parse("MOV r0l, r1l\n")
    check instrs[0].opcode == OpCode.MovRegReg
    check instrs[0].operands == @[RegLaneBit or 0'u8, RegLaneBit or 1'u8]

  test "MOV r0, 0x1234 produces MovRegImm with big-endian operands":
    let instrs = parse("MOV r0, 0x1234\n")
    check instrs[0].opcode == OpCode.MovRegImm
    check instrs[0].operands == @[0'u8, 0x12'u8, 0x34'u8]

  test "MOV lane mismatch returns error":
    let tokens = tokenizeAssembly("MOV r0, r1l\n").get()
    let res = parseTokens(tokens)
    check res.isErr

  test "MOV immediate out of 8-bit range for byte-lane returns error":
    let tokens = tokenizeAssembly("MOV r0l, 256\n").get()
    let res = parseTokens(tokens)
    check res.isErr

  test "MOV immediate out of 16-bit range returns error":
    let tokens = tokenizeAssembly("MOV r0, 65536\n").get()
    let res = parseTokens(tokens)
    check res.isErr

  test "unknown mnemonic returns error":
    let tokens = tokenizeAssembly("FOOBAR\n").get()
    let res = parseTokens(tokens)
    check res.isErr

  test "multiple instructions parsed in order":
    let instrs = parse("NOP\nHALT\n")
    require instrs.len == 2
    check instrs[0].opcode == OpCode.Nop
    check instrs[1].opcode == OpCode.Halt

