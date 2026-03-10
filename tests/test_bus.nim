## MMIO Bus tests

import unittest
import fvm/errors
import fvm/vm/bus
import fvm/core/types
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

suite "Bus - region mapping":
  test "single RAM region: read/write round-trip":
    var bus = newBus()
    bus.mapRegion(ramRegion(0'u16, 256, "ram"))
    bus.write8(0'u16, 0xAB'u8)
    check bus.read8(0'u16) == 0xAB'u8

  test "read at unmapped address returns error":
    var bus = newBus()
    bus.mapRegion(ramRegion(0x1000'u16, 256, "ram"))
    expect BusFaultError:
      discard bus.read8(0x0000'u16)

  test "write at unmapped address returns error":
    var bus = newBus()
    bus.mapRegion(ramRegion(0x1000'u16, 256, "ram"))
    expect BusFaultError:
      bus.write8(0x0000'u16, 0xFF'u8)

  test "write to ROM region returns error":
    var bus = newBus()
    bus.mapRegion(romRegion(0'u16, 256, "rom"))
    expect BusFaultError:
      bus.write8(0'u16, 0x42'u8)

  test "overlapping regions return error on second map":
    var bus = newBus()
    bus.mapRegion(ramRegion(0'u16, 256, "a"))
    expect VmLayoutError:
      bus.mapRegion(ramRegion(128'u16, 256, "b"))

  test "adjacent non-overlapping regions are both accessible":
    var bus = newBus()
    bus.mapRegion(ramRegion(0'u16, 256, "lo"))
    bus.mapRegion(ramRegion(256'u16, 256, "hi"))
    bus.write8(255'u16, 0x11'u8)
    bus.write8(256'u16, 0x22'u8)
    check bus.read8(255'u16) == 0x11'u8
    check bus.read8(256'u16) == 0x22'u8

suite "Bus - 16-bit access":
  test "read16 big-endian":
    var bus = newBus()
    bus.mapRegion(ramRegion(0'u16, 256, "ram"))
    bus.write8(0'u16, 0x12'u8)
    bus.write8(1'u16, 0x34'u8)
    check bus.read16(0'u16) == 0x1234'u16

  test "write16 big-endian":
    var bus = newBus()
    bus.mapRegion(ramRegion(0'u16, 256, "ram"))
    bus.write16(0'u16, 0xABCD'u16)
    check bus.read8(0'u16) == 0xAB'u8
    check bus.read8(1'u16) == 0xCD'u8

suite "Bus - device region":
  test "device read is called for mapped address":
    var bus = newBus()
    var called = false
    let devRead: DeviceRead = proc(a: Address): Byte =
      called = true
      0x99'u8
    let devWrite: DeviceWrite = proc(a: Address, v: Byte) =
      discard
    bus.mapRegion(deviceRegion(0x8000'u16, 16, "dev", devRead, devWrite))
    let val = bus.read8(0x8000'u16)
    check called
    check val == 0x99'u8
