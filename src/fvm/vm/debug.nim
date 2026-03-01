## VM debug utilities: formatting helpers for registers, stack, and flags.

import std/logging
import std/strformat
import std/strutils

import ./core

export core, logging

# Register formatting

proc formatRegisters*(vm: Vm): string =
  ## Returns a one-line summary of all 16 general-purpose registers.
  var parts: seq[string]
  for i in 0 ..< GeneralRegisterCount:
    parts.add(fmt"r{i}=0x{vm.regs[i]:04X}")
  parts.join("  ")

proc formatFlags*(vm: Vm): string =
  ## Returns a compact flag-register display.
  fmt"Z={int(vm.flags.zero)} C={int(vm.flags.carry)} N={int(vm.flags.negative)} V={int(vm.flags.overflow)}"

# Stack window

proc formatStackWindow*(vm: Vm): string =
  ## Returns a formatted dump of the live stack entries (sp .. StackBase).
  var entries: seq[string]
  for address in int(vm.sp) .. int(StackBase):
    entries.add(fmt"0x{address:04X}:0x{vm.bus.mem[address]:02X}")
  entries.join(" ")

proc debugStackWindow*(vm: Vm) =
  debug fmt"stack[0x{vm.sp:04X}..0x{StackBase:04X}] = [{formatStackWindow(vm)}]"

# Instruction pointer

proc formatIp*(vm: Vm): string =
  fmt"ip=0x{vm.ip:04X}  sp=0x{vm.sp:04X}"
