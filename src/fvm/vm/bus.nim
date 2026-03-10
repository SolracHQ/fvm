## MMIO Memory Bus
##
## The Bus owns the flat 64 KB backing array and a list of MemRegion descriptors
## that partition (or overlay) that address space.  All VM memory access both
## code fetch and data read/write goes through the Bus API.
##
## Peripheral devices (screen, UART, timers, ...) are added by registering a
## MemRegion whose `deviceRead`/`deviceWrite` procs intercept I/O for their
## address range.  Regions backed only by the flat array leave those procs nil.
##
## Permission model:
##   writable = false -> write returns an error (ROM / code segment)
##   writable = true  -> write proceeds (RAM / device registers)
##
## Device model:
##   hasDevice = false -> read/write go directly to mem[address]
##   hasDevice = true  -> read/write delegated to the device procs
##                        (the backing mem array is ignored for that region)

import std/strutils

import ../core/types as coretypes
import ../core/constants as coreconst
import ../errors

export coretypes, coreconst ## Re-export shared bus-visible core declarations
export errors

type
  DeviceRead* = proc(address: Address): Byte {.closure.}
  DeviceWrite* = proc(address: Address, value: Byte) {.closure.}

  MemRegion* = object
    base*: Address
    size*: uint32 ## uint32 so we can express a full 64 KB region
    label*: string
    permissions*: Permissions
    hasDevice*: bool
    deviceRead*: DeviceRead
    deviceWrite*: DeviceWrite

  Bus* = object
    mem*: seq[Byte] ## heap-allocated; initialized to VmMemorySize bytes
    regions*: seq[MemRegion]

proc newBus*(): Bus =
  ## Creates a Bus with its full 64 KB backing store zeroed.
  Bus(mem: newSeq[Byte](VmMemorySize))

# Region registration

proc overlaps(a, b: MemRegion): bool =
  let aEnd = uint32(a.base) + a.size
  let bEnd = uint32(b.base) + b.size
  not (aEnd <= uint32(b.base) or bEnd <= uint32(a.base))

proc mapRegion*(bus: var Bus, region: MemRegion) =
  ## Registers a memory region on the bus.
  ## Returns an error if the new region overlaps an existing one.
  for existing in bus.regions:
    if overlaps(existing, region):
      raise newVmLayoutError(
        "Bus region '" & region.label & "' at 0x" & toHex(int(region.base), 4) &
        " overlaps existing region '" & existing.label & "'"
      )
  bus.regions.add(region)

proc findRegion(bus: Bus, address: Address): int =
  ## Returns the index of the region that contains `address`, or -1.
  for i, r in bus.regions:
    if address >= r.base and uint32(address) < uint32(r.base) + r.size:
      return i
  -1

# Plain RAM/ROM region helpers

proc ramRegion*(base: Address, size: uint32, label = "ram"): MemRegion =
  MemRegion(
    base: base, size: size, label: label, permissions: PermRam, hasDevice: false
  )

proc romRegion*(base: Address, size: uint32, label = "rom"): MemRegion =
  MemRegion(
    base: base, size: size, label: label, permissions: PermRom, hasDevice: false
  )

proc codeRegion*(base: Address, size: uint32, label = "code"): MemRegion =
  MemRegion(
    base: base, size: size, label: label, permissions: PermCode, hasDevice: false
  )

proc deviceRegion*(
    base: Address, size: uint32, label: string, read: DeviceRead, write: DeviceWrite
): MemRegion =
  MemRegion(
    base: base,
    size: size,
    label: label,
    permissions: PermRam,
    hasDevice: true,
    deviceRead: read,
    deviceWrite: write,
  )

# Read / Write API

proc read8*(bus: Bus, address: Address): Byte =
  let idx = findRegion(bus, address)
  if idx < 0:
    raise newBusFaultError("Bus read at unmapped address 0x" & toHex(int(address), 4))
  let region = bus.regions[idx]
  if Read notin region.permissions:
    raise newBusFaultError(
      "Bus read from non-readable region '" & region.label & "' at 0x" &
      toHex(int(address), 4)
    )
  if region.hasDevice:
    return region.deviceRead(address)
  bus.mem[int(address)]

proc write8*(bus: var Bus, address: Address, value: Byte) =
  let idx = findRegion(bus, address)
  if idx < 0:
    raise newBusFaultError("Bus write at unmapped address 0x" & toHex(int(address), 4))
  let region = bus.regions[idx]
  if Write notin region.permissions:
    raise newBusFaultError(
      "Bus write to read-only region '" & region.label & "' at 0x" &
      toHex(int(address), 4)
    )
  if region.hasDevice:
    region.deviceWrite(address, value)
    return
  bus.mem[int(address)] = value

proc fetch8*(bus: Bus, address: Address): Byte =
  ## Opcode fetch: requires the Execute permission in addition to Read.
  let idx = findRegion(bus, address)
  if idx < 0:
    raise newBusFaultError("Bus fetch at unmapped address 0x" & toHex(int(address), 4))
  let region = bus.regions[idx]
  if Execute notin region.permissions:
    raise newBusFaultError(
      "Bus fetch from non-executable region '" & region.label & "' at 0x" &
      toHex(int(address), 4)
    )
  if region.hasDevice:
    return region.deviceRead(address)
  bus.mem[int(address)]

proc read16*(bus: Bus, address: Address): Word =
  ## Big-endian 16-bit read.
  let hi = bus.read8(address)
  let lo = bus.read8(Address(uint32(address) + 1))
  (Word(hi) shl 8) or Word(lo)

proc write16*(bus: var Bus, address: Address, value: Word) =
  ## Big-endian 16-bit write.
  bus.write8(address, Byte((value shr 8) and ByteMask))
  bus.write8(Address(uint32(address) + 1), Byte(value and ByteMask))

proc writeRange*(bus: var Bus, base: Address, data: openArray[Byte]) =
  ## Bulk-write a byte sequence starting at `base`.  Goes through `write8` so
  ## permission checks apply; use `writeRangeDirect` for loader initialization.
  for i, b in data:
    bus.write8(Address(uint32(base) + uint32(i)), b)

proc writeRangeDirect*(bus: var Bus, base: Address, data: openArray[Byte]) =
  ## Writes bytes directly into the backing array without permission checks.
  ## Use only during VM initialization before regions are made live.
  for i, b in data:
    bus.mem[int(base) + i] = b
