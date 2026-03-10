## VM lifecycle tests
##
## These tests go through the public API:
##   newVm -> initRom (with raw FvmObject) -> step / run -> assert register state
##
## No assembly source is used here; bytecode is constructed directly to keep
## the VM tests independent from the assembler subsystem.

import unittest
import fvm/errors
import fvm/assembler/assembler
import fvm/vm/vm
import fvm/vm/ports
import fvm/format/fvmobject as fmtobject
import fvm/core/constants

template get(value: untyped): untyped =
  block:
    when compiles(
      block:
        let tmp = value
        tmp
    ):
      let tmp = value
      tmp
    else:
      value

proc makeObj(code: seq[Byte]): FvmObject =
  FvmObject(version: FvmVersion, entryPoint: 0'u16, code: code)

proc freshVm(code: seq[Byte]): Vm =
  var vm = newVm()
  vm.initRom(makeObj(code))
  vm

proc freshVm(obj: FvmObject): Vm =
  var vm = newVm()
  vm.initRom(obj)
  vm

proc bytes(s: string): seq[Byte] =
  for ch in s:
    result.add(Byte(ord(ch)))

suite "newVm":
  test "creates a halted=false VM":
    let vm = newVm()
    check not vm.halted

  test "all registers initialised to 0":
    let vm = newVm()
    for r in vm.regs:
      check r == 0

  test "ip starts at 0":
    let vm = newVm()
    check vm.ip == 0

suite "initRom":
  test "loads bytecode at entry point 0":
    var vm = newVm()
    let obj = makeObj(@[Byte(ord(OpCode.Nop)), Byte(ord(OpCode.Halt))])
    vm.initRom(obj)
    check vm.bus.mem[0] == Byte(ord(OpCode.Nop))
    check vm.bus.mem[1] == Byte(ord(OpCode.Halt))

  test "sets ip to entryPoint":
    var vm = newVm()
    let obj = FvmObject(
      version: FvmVersion, entryPoint: 0x0010'u16, code: @[Byte(ord(OpCode.Halt))]
    )
    vm.initRom(obj)
    check vm.ip == 0x0010'u16

  test "program too large returns error":
    var vm = newVm()
    let bigCode = newSeq[Byte](VmMemorySize + 1)
    let obj = makeObj(bigCode)
    expect VmLayoutError:
      vm.initRom(obj)

suite "step - NOP":
  test "NOP advances ip by 1":
    var vm = freshVm(@[Byte(ord(OpCode.Nop)), Byte(ord(OpCode.Halt))])
    vm.step()
    check vm.ip == 1

suite "step - HALT":
  test "HALT sets halted flag":
    var vm = freshVm(@[Byte(ord(OpCode.Halt))])
    vm.step()
    check vm.halted

suite "step - MOV / PUSH / POP":
  test "MOV r0, 42 sets r0":
    var vm =
      freshVm(@[Byte(ord(OpCode.MovRegImm)), 0'u8, 0'u8, 42'u8, Byte(ord(OpCode.Halt))])
    vm.step()
    check vm.regs[0] == 42

  test "PUSH r0 then POP r1 copies value":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0'u8,
        0'u8,
        0xAB'u8, # MOV r0, 0xAB
        Byte(ord(OpCode.Push)),
        0'u8, # PUSH r0
        Byte(ord(OpCode.Pop)),
        1'u8, # POP  r1
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[1] == 0xAB

  test "MOV r0, r1 copies register":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        1'u8,
        0'u8,
        0x55'u8, # MOV r1, 0x55
        Byte(ord(OpCode.MovRegReg)),
        0'u8,
        1'u8, # MOV r0, r1
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 0x55

  test "MOV r0, 0x1234 stores 16-bit value":
    var vm = freshVm(
      @[Byte(ord(OpCode.MovRegImm)), 0'u8, 0x12'u8, 0x34'u8, Byte(ord(OpCode.Halt))]
    )
    vm.run()
    check vm.regs[0] == 0x1234

  test "MOV r0l writes low byte only leaving high byte intact":
    # RegLaneBit = 0x80, so enc for r0l = 0x80
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0'u8,
        0x12'u8,
        0x34'u8, # MOV r0, 0x1234
        Byte(ord(OpCode.MovRegImm)),
        0x80'u8,
        0xAB'u8, # MOV r0l, 0xAB
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 0x12AB

suite "run":
  test "run stops on HALT":
    var vm = freshVm(@[Byte(ord(OpCode.Nop)), Byte(ord(OpCode.Halt))])
    vm.run()
    check vm.halted

  test "invalid opcode raises interrupt and halts when unhandled":
    var vm = freshVm(@[0xFF'u8])
    vm.run()
    check vm.halted
    check vm.ip == 1

# enc byte constants used throughout arithmetic tests
# r0 = 0x00, r1 = 0x01, r0l = 0x80, r1l = 0x81

suite "step - ADD":
  test "ADD r0, r1 full-width":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x05'u8, # MOV r0, 5
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0x03'u8, # MOV r1, 3
        Byte(ord(OpCode.Add)),
        0x00'u8,
        0x01'u8, # ADD r0, r1
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 8
    check not vm.flags.carry
    check not vm.flags.zero
    check not vm.flags.negative

  test "ADD carry on word overflow":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0xFF'u8,
        0xFF'u8, # MOV r0, 0xFFFF
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0x01'u8, # MOV r1, 1
        Byte(ord(OpCode.Add)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 0x0000
    check vm.flags.carry
    check vm.flags.zero

  test "ADD byte-lane carry on overflow":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x80'u8,
        0xFF'u8, # MOV r0l, 0xFF
        Byte(ord(OpCode.MovRegImm)),
        0x81'u8,
        0x01'u8, # MOV r1l, 0x01
        Byte(ord(OpCode.Add)),
        0x80'u8,
        0x81'u8, # ADD r0l, r1l
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check (vm.regs[0] and 0xFF'u16) == 0x00
    check vm.flags.carry
    check vm.flags.zero

  test "ADD sets zero flag":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x00'u8,
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0x00'u8,
        Byte(ord(OpCode.Add)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.flags.zero

suite "step - SUB":
  test "SUB r0, r1 basic":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x0A'u8, # MOV r0, 10
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0x03'u8, # MOV r1, 3
        Byte(ord(OpCode.Sub)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 7
    check not vm.flags.carry

  test "SUB borrow sets carry":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x03'u8, # MOV r0, 3
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0x05'u8, # MOV r1, 5
        Byte(ord(OpCode.Sub)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 0xFFFE'u16
    check vm.flags.carry

  test "SUB equal values sets zero flag":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x07'u8,
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0x07'u8,
        Byte(ord(OpCode.Sub)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 0
    check vm.flags.zero
    check not vm.flags.carry

suite "step - AND/OR/XOR/NOT":
  test "AND r0, r1":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0xFF'u8, # r0 = 0x00FF
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x0F'u8,
        0x0F'u8, # r1 = 0x0F0F
        Byte(ord(OpCode.And)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 0x000F'u16

  test "OR r0, r1":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0xF0'u8,
        0x00'u8, # r0 = 0xF000
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0x0F'u8, # r1 = 0x000F
        Byte(ord(OpCode.Or)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 0xF00F'u16

  test "XOR r0 with itself zeroes register":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x12'u8,
        0x34'u8,
        Byte(ord(OpCode.Xor)),
        0x00'u8,
        0x00'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 0
    check vm.flags.zero

  test "NOT r0":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0xFF'u8,
        0x00'u8, # r0 = 0xFF00
        Byte(ord(OpCode.Not)),
        0x00'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 0x00FF'u16

  test "NOT byte-lane":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x80'u8,
        0x0F'u8, # MOV r0l, 0x0F
        Byte(ord(OpCode.Not)),
        0x80'u8, # NOT r0l
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check (vm.regs[0] and 0xFF'u16) == 0xF0'u16

suite "step - CMP":
  test "CMP equal sets zero, clears carry":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x0A'u8,
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0x0A'u8,
        Byte(ord(OpCode.Cmp)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.flags.zero
    check not vm.flags.carry
    check vm.regs[0] == 0x000A'u16 # not modified

  test "CMP src > dst sets carry":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x03'u8,
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0x05'u8,
        Byte(ord(OpCode.Cmp)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.flags.carry
    check not vm.flags.zero
    check vm.regs[0] == 0x0003'u16 # unchanged

suite "step - OUT":
  test "OUT byte sends single byte to port":
    var captured: seq[Byte]
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x80'u8,
        0xAB'u8, # MOV r0l, 0xAB
        Byte(ord(OpCode.Out)),
        0x00'u8,
        0x80'u8, # OUT 0, r0l
        Byte(ord(OpCode.Halt)),
      ]
    )

    vm.ports.registerPort(
      0,
      PortDevice(
        label: "capture",
        read: proc(): Byte =
          Byte(0),
        write: proc(v: Byte): void =
          captured.add(v),
      ),
    )

    vm.run()
    check captured == @[0xAB'u8]

  test "OUT word sends hi then lo byte":
    var captured: seq[Byte]
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x12'u8,
        0x34'u8, # MOV r0, 0x1234
        Byte(ord(OpCode.Out)),
        0x00'u8,
        0x00'u8, # OUT 0, r0
        Byte(ord(OpCode.Halt)),
      ]
    )

    vm.ports.registerPort(
      0,
      PortDevice(
        label: "capture",
        read: proc(): Byte =
          Byte(0),
        write: proc(v: Byte): void =
          captured.add(v),
      ),
    )

    vm.run()
    check captured == @[0x12'u8, 0x34'u8]

suite "step - IN":
  test "IN byte lane reads from port":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.In)),
        0x80'u8,
        0x00'u8, # IN r0l, 0
        Byte(ord(OpCode.Halt)),
      ]
    )

    vm.ports.registerPort(
      0,
      PortDevice(
        label: "provide",
        read: proc(): Byte =
          Byte(0xCD),
        write: proc(v: Byte): void =
          discard,
      ),
    )

    vm.run()
    check (vm.regs[0] and 0xFF'u16) == 0xCD'u16

  test "IN word assembles hi then lo from port":
    var counter = 0
    var vm = freshVm(
      @[
        Byte(ord(OpCode.In)),
        0x00'u8,
        0x00'u8, # IN r0, 0
        Byte(ord(OpCode.Halt)),
      ]
    )

    vm.ports.registerPort(
      0,
      PortDevice(
        label: "provide",
        read: proc(): Byte =
          inc counter
          (if counter == 1: Byte(0xAB) else: Byte(0xCD)),
        write: proc(v: Byte): void =
          discard,
      ),
    )

    vm.run()
    check vm.regs[0] == 0xABCD'u16

suite "step - JMP":
  test "JMP immediate skips instructions":
    # layout:
    #   [0] JMP offset 7          3 bytes  -> jumps to code[7]
    #   [3] MOV r0, 1           4 bytes  -> skipped
    #   [7] HALT
    var vm = freshVm(
      @[
        Byte(ord(OpCode.Jmp)),
        0x00'u8,
        0x27'u8,
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[0] == 0

  test "JNZ taken (zero=false) skips fall-through":
    # layout:
    #   [0]  MOV r0, 1       4 bytes
    #   [4]  CMP r0, r1      3 bytes  -> zero=false (1 - 0 = 1)
    #   [7]  JNZ code[14]     3 bytes  -> jumps to code[14]
    #   [10] MOV r2, 1       4 bytes  -> skipped
    #   [14] HALT
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Cmp)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Jnz)),
        0x00'u8,
        0x2E'u8,
        Byte(ord(OpCode.MovRegImm)),
        0x02'u8,
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[2] == 0

  test "JNZ not taken (zero=true) continues":
    # layout:
    #   [0] CMP r0, r1       3 bytes  -> zero=true (0 - 0 = 0)
    #   [3] JNZ 0x000B       3 bytes  -> not taken
    #   [6] MOV r2, 0xFF     4 bytes  -> executes
    #   [10] HALT
    var vm = freshVm(
      @[
        Byte(ord(OpCode.Cmp)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Jnz)),
        0x00'u8,
        0x0B'u8,
        Byte(ord(OpCode.MovRegImm)),
        0x02'u8,
        0x00'u8,
        0xFF'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[2] == 0xFF

  test "JZ taken (zero=true) skips fall-through":
    # layout:
    #   [0] CMP r0, r0       3 bytes  -> zero=true
    #   [3] JZ code[10]       3 bytes  -> jumps to code[10]
    #   [6] MOV r1, 1        4 bytes  -> skipped
    #   [10] HALT
    var vm = freshVm(
      @[
        Byte(ord(OpCode.Cmp)),
        0x00'u8,
        0x00'u8,
        Byte(ord(OpCode.Jz)),
        0x00'u8,
        0x2A'u8,
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[1] == 0

  test "JZ not taken (zero=false) continues":
    # layout:
    #   [0]  MOV r0, 1       4 bytes
    #   [4]  CMP r0, r1      3 bytes  -> zero=false
    #   [7]  JZ 0x000F       3 bytes  -> not taken
    #   [10] MOV r2, 0xAB    4 bytes  -> executes
    #   [14] HALT
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Cmp)),
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Jz)),
        0x00'u8,
        0x0F'u8,
        Byte(ord(OpCode.MovRegImm)),
        0x02'u8,
        0x00'u8,
        0xAB'u8,
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[2] == 0xAB

  test "JmpReg (indirect) jumps to address in register":
    # layout:
    #   [0]  MOV r0, code[11]  4 bytes  -> r0 = absolute VM address of code[11]
    #   [4]  JmpReg r0         2 bytes  -> jump to r0
    #   [6]  MOV r1, 1         4 bytes  -> skipped (offsets 6-9)
    #   [10] HALT                       -> skipped
    #   [11] HALT
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x2B'u8,
        Byte(ord(OpCode.JmpReg)),
        0x00'u8,
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0x01'u8,
        Byte(ord(OpCode.Halt)),
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.regs[1] == 0

suite "step - CALL / RET":
  test "CALL pushes return address and jumps, RET returns":
    # layout:
    #   [0]  CALL code[8]    3 bytes  -> retAddr=code[3], jumps to code[8]
    #   [3]  MOV r1, 0xFF    4 bytes  -> executes after RET
    #   [7]  HALT
    #   [8]  MOV r0l, 0xAB   3 bytes  -> subroutine body
    #   [11] RET              1 byte   -> returns to code[3]
    var vm = freshVm(
      @[
        Byte(ord(OpCode.Call)),
        0x00'u8,
        0x08'u8,
        Byte(ord(OpCode.MovRegImm)),
        0x01'u8,
        0x00'u8,
        0xFF'u8,
        Byte(ord(OpCode.Halt)),
        Byte(ord(OpCode.MovRegImm)),
        0x80'u8,
        0xAB'u8,
        Byte(ord(OpCode.Ret)),
      ]
    )
    vm.run()
    check (vm.regs[0] and 0xFF'u16) == 0xAB
    check vm.regs[1] == 0xFF

  test "nested CALL / RET restores correct return addresses":
    # outer calls inner at offset 14; inner increments r0 and returns;
    # outer then increments r0 again and returns to base caller.
    # layout:
    #   [0]  CALL 0x0007     3 bytes  -> calls outer, retAddr=3
    #   [3]  MOV r2, 1       4 bytes  -> executes when outer returns (but HALT follows)
    # wait, simpler: just 2-level depth
    #   [0]  CALL outer      3 bytes  -> retAddr=3
    #   [3]  HALT
    #   outer at [4]:
    #   [4]  CALL inner      3 bytes  -> retAddr=7
    #   [7]  MOV r0l, 0xBB   3 bytes  -> runs after inner returns
    #   [10] RET              1 byte   -> returns to 3
    #   inner at [11]:
    #   [11] MOV r0l, 0xAA   3 bytes
    #   [14] RET              1 byte   -> returns to 7
    var vm = freshVm(
      @[
        Byte(ord(OpCode.Call)),
        0x00'u8,
        0x04'u8, # CALL outer (code offset 4)
        Byte(ord(OpCode.Halt)),
        Byte(ord(OpCode.Call)),
        0x00'u8,
        0x0B'u8, # CALL inner (code offset 11)
        Byte(ord(OpCode.MovRegImm)),
        0x80'u8,
        0xBB'u8, # MOV r0l, 0xBB
        Byte(ord(OpCode.Ret)),
        Byte(ord(OpCode.MovRegImm)),
        0x80'u8,
        0xAA'u8, # MOV r0l, 0xAA
        Byte(ord(OpCode.Ret)),
      ]
    )
    vm.run()
    # inner set r0l=0xAA, outer then set r0l=0xBB
    check (vm.regs[0] and 0xFF'u16) == 0xBB

suite "step - SP register":
  test "MOV r0, sp reads stack pointer":
    # sp starts at StackBase (0xFFFF); MovRegReg with src=SpEncoding (0x40)
    var vm =
      freshVm(@[Byte(ord(OpCode.MovRegReg)), 0x00'u8, 0x40'u8, Byte(ord(OpCode.Halt))])
    vm.run()
    check vm.regs[0] == StackBase

  test "MOV sp, r0 writes stack pointer":
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x12'u8,
        0x34'u8, # MOV r0, 0x1234
        Byte(ord(OpCode.MovRegReg)),
        0x40'u8,
        0x00'u8, # MOV sp, r0
        Byte(ord(OpCode.Halt)),
      ]
    )
    vm.run()
    check vm.sp == 0x1234'u16

suite "interrupts":
  test "lane registers are valid interrupt indexes for SIE and INT":
    var captured: seq[Byte]
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x80'u8,
        0x0F'u8, # MOV r0l, 15
        Byte(ord(OpCode.SieRegImm)),
        0x80'u8,
        0x00'u8,
        0x0A'u8, # SIE r0l, handler
        Byte(ord(OpCode.IntReg)),
        0x80'u8, # INT r0l
        Byte(ord(OpCode.Halt)),
        Byte(ord(OpCode.MovRegImm)),
        0x81'u8,
        0x4C'u8, # MOV r1l, 'L'
        Byte(ord(OpCode.Out)),
        0x00'u8,
        0x81'u8, # OUT 0, r1l
        Byte(ord(OpCode.Iret)),
      ]
    )
    vm.ports.registerPort(
      0,
      PortDevice(
        label: "capture",
        read: proc(): Byte =
          Byte(0),
        write: proc(v: Byte): void =
          captured.add(v),
      ),
    )

    vm.run()
    check captured == @[Byte('L')]
    check vm.halted

  test "SIE installs a handler and INT immediate resumes after IRET":
    var captured: seq[Byte]
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x0F'u8, # MOV r0, 15
        Byte(ord(OpCode.SieRegImm)),
        0x00'u8,
        0x00'u8,
        0x0B'u8, # SIE r0, handler
        Byte(ord(OpCode.IntImm)),
        0x0F'u8, # INT 15
        Byte(ord(OpCode.Halt)),
        Byte(ord(OpCode.MovRegImm)),
        0x81'u8,
        0x53'u8, # MOV r1l, 'S'
        Byte(ord(OpCode.Out)),
        0x00'u8,
        0x81'u8, # OUT 0, r1l
        Byte(ord(OpCode.Iret)),
      ]
    )
    vm.ports.registerPort(
      0,
      PortDevice(
        label: "capture",
        read: proc(): Byte =
          Byte(0),
        write: proc(v: Byte): void =
          captured.add(v),
      ),
    )

    vm.run()
    check captured == @[Byte('S')]
    check vm.halted
    check not vm.inInterrupt

  test "invalid opcode enters installed handler and resumes at next byte":
    var captured: seq[Byte]
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x02'u8, # MOV r0, 2
        Byte(ord(OpCode.SieRegImm)),
        0x00'u8,
        0x00'u8,
        0x0A'u8, # handler at byte 10
        0xFF'u8, # invalid opcode
        Byte(ord(OpCode.Halt)),
        Byte(ord(OpCode.MovRegImm)),
        0x80'u8,
        0x49'u8, # MOV r0l, 'I'
        Byte(ord(OpCode.Out)),
        0x00'u8,
        0x80'u8, # OUT 0, r0l
        Byte(ord(OpCode.Iret)),
      ]
    )
    vm.ports.registerPort(
      0,
      PortDevice(
        label: "capture",
        read: proc(): Byte =
          Byte(0),
        write: proc(v: Byte): void =
          captured.add(v),
      ),
    )

    vm.run()
    check captured == @[Byte('I')]
    check vm.halted

  test "DPL drops to user mode and user SIE raises privilege fault":
    var captured: seq[Byte]
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x06'u8, # MOV r0, 6
        Byte(ord(OpCode.SieRegImm)),
        0x00'u8,
        0x00'u8,
        0x0E'u8, # SIE r0, handler
        Byte(ord(OpCode.Dpl)),
        Byte(ord(OpCode.SieRegImm)),
        0x00'u8,
        0x00'u8,
        0x0F'u8, # user-mode SIE faults
        Byte(ord(OpCode.Halt)),
        Byte(ord(OpCode.MovRegImm)),
        0x81'u8,
        0x50'u8, # MOV r1l, 'P'
        Byte(ord(OpCode.Out)),
        0x00'u8,
        0x81'u8, # OUT 0, r1l
        Byte(ord(OpCode.Iret)),
      ]
    )
    vm.ports.registerPort(
      0,
      PortDevice(
        label: "capture",
        read: proc(): Byte =
          Byte(0),
        write: proc(v: Byte): void =
          captured.add(v),
      ),
    )

    vm.run()
    check captured == @[Byte('P')]
    check not vm.privileged

  test "nested interrupts are dropped inside handlers":
    var captured: seq[Byte]
    var vm = freshVm(
      @[
        Byte(ord(OpCode.MovRegImm)),
        0x00'u8,
        0x00'u8,
        0x02'u8, # MOV r0, 2
        Byte(ord(OpCode.SieRegImm)),
        0x00'u8,
        0x00'u8,
        0x0A'u8, # invalid-opcode handler
        0xFF'u8, # trigger interrupt 2
        Byte(ord(OpCode.Halt)),
        0xFF'u8, # nested invalid opcode should be dropped
        Byte(ord(OpCode.MovRegImm)),
        0x81'u8,
        0x4E'u8, # MOV r1l, 'N'
        Byte(ord(OpCode.Out)),
        0x00'u8,
        0x81'u8, # OUT 0, r1l
        Byte(ord(OpCode.Iret)),
      ]
    )
    vm.ports.registerPort(
      0,
      PortDevice(
        label: "capture",
        read: proc(): Byte =
          Byte(0),
        write: proc(v: Byte): void =
          captured.add(v),
      ),
    )

    vm.run()
    check captured == @[Byte('N')]
    check vm.halted

  test "interrupt example assembles and runs":
    var captured: seq[Byte]
    var vm = freshVm(assembleFile("examples/interrupts.fa").get())
    vm.ports.registerPort(
      0,
      PortDevice(
        label: "capture",
        read: proc(): Byte =
          Byte(0),
        write: proc(v: Byte): void =
          captured.add(v),
      ),
    )

    vm.run()
    check captured ==
      bytes(
        "software interrupt ok\n" & "bus fault: unmapped read below stack\n" &
          "stack underflow: POP on empty stack\n"
      )
