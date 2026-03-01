## MMIO Bus tests

import unittest
import fvm/vm/bus
import fvm/types/core

suite "Bus - region mapping":
  test "single RAM region: read/write round-trip":
    var bus = newBus()
    bus.mapRegion(ramRegion(0'u16, 256, "ram")).get()
    bus.write8(0'u16, 0xAB'u8).get()
    check bus.read8(0'u16).get() == 0xAB'u8

  test "read at unmapped address returns error":
    var bus = newBus()
    bus.mapRegion(ramRegion(0x1000'u16, 256, "ram")).get()
    let res = bus.read8(0x0000'u16)
    check res.isErr

  test "write at unmapped address returns error":
    var bus = newBus()
    bus.mapRegion(ramRegion(0x1000'u16, 256, "ram")).get()
    let res = bus.write8(0x0000'u16, 0xFF'u8)
    check res.isErr

  test "write to ROM region returns error":
    var bus = newBus()
    bus.mapRegion(romRegion(0'u16, 256, "rom")).get()
    let res = bus.write8(0'u16, 0x42'u8)
    check res.isErr

  test "overlapping regions return error on second map":
    var bus = newBus()
    bus.mapRegion(ramRegion(0'u16, 256, "a")).get()
    let res = bus.mapRegion(ramRegion(128'u16, 256, "b"))
    check res.isErr

  test "adjacent non-overlapping regions are both accessible":
    var bus = newBus()
    bus.mapRegion(ramRegion(0'u16,   256, "lo")).get()
    bus.mapRegion(ramRegion(256'u16, 256, "hi")).get()
    bus.write8(255'u16, 0x11'u8).get()
    bus.write8(256'u16, 0x22'u8).get()
    check bus.read8(255'u16).get() == 0x11'u8
    check bus.read8(256'u16).get() == 0x22'u8

suite "Bus - 16-bit access":
  test "read16 big-endian":
    var bus = newBus()
    bus.mapRegion(ramRegion(0'u16, 256, "ram")).get()
    bus.write8(0'u16, 0x12'u8).get()
    bus.write8(1'u16, 0x34'u8).get()
    check bus.read16(0'u16).get() == 0x1234'u16

  test "write16 big-endian":
    var bus = newBus()
    bus.mapRegion(ramRegion(0'u16, 256, "ram")).get()
    bus.write16(0'u16, 0xABCD'u16).get()
    check bus.read8(0'u16).get() == 0xAB'u8
    check bus.read8(1'u16).get() == 0xCD'u8

suite "Bus - device region":
  test "device read is called for mapped address":
    var bus = newBus()
    var called = false
    let devRead: DeviceRead  = proc(a: Address): FvmResult[Byte] =
      called = true
      (0x99'u8).ok
    let devWrite: DeviceWrite = proc(a: Address, v: Byte): FvmResult[void] = ok()
    bus.mapRegion(deviceRegion(0x8000'u16, 16, "dev", devRead, devWrite)).get()
    let val = bus.read8(0x8000'u16).get()
    check called
    check val == 0x99'u8
