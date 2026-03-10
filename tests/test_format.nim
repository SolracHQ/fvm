## FVM object format tests

import unittest
import fvm/errors
import fvm/format/fvmobject

template get(value: untyped): untyped =
  value

suite "FvmObject serialize / deserialize":
  test "round-trip: serialize then deserialize":
    let original =
      FvmObject(version: FvmVersion, entryPoint: 0x0010'u16, code: @[0x00'u8, 0x01'u8])
    let bytes = original.serialize()
    let restored = deserialize(bytes).get()
    check restored.version == original.version
    check restored.entryPoint == original.entryPoint
    check restored.code == original.code

  test "magic bytes are 'FVM!'":
    let obj = FvmObject(version: FvmVersion, entryPoint: 0'u16, code: @[])
    let bytes = obj.serialize()
    check bytes[0] == 0x46'u8 # 'F'
    check bytes[1] == 0x56'u8 # 'V'
    check bytes[2] == 0x4D'u8 # 'M'
    check bytes[3] == 0x21'u8 # '!'

  test "wrong magic returns error":
    var bytes = FvmObject(version: FvmVersion, entryPoint: 0'u16, code: @[]).serialize()
    bytes[0] = 0x00'u8
    expect ObjectFormatError:
      discard deserialize(bytes)

  test "unsupported version returns error":
    var bytes = FvmObject(version: FvmVersion, entryPoint: 0'u16, code: @[]).serialize()
    bytes[4] = 99'u8
    expect ObjectFormatError:
      discard deserialize(bytes)

  test "truncated data returns error":
    let bytes = @[0x46'u8, 0x56, 0x4D, 0x21] # only magic, no header body
    expect ObjectFormatError:
      discard deserialize(bytes)

  test "header declares more code bytes than present returns error":
    var bytes =
      FvmObject(version: FvmVersion, entryPoint: 0'u16, code: @[0x00'u8]).serialize()
    # inflate the declared code length to 100
    bytes[7] = 0x00'u8
    bytes[8] = 100'u8
    expect ObjectFormatError:
      discard deserialize(bytes)

  test "empty code section is valid":
    let obj = FvmObject(version: FvmVersion, entryPoint: 0'u16, code: @[])
    let bytes = obj.serialize()
    let restored = deserialize(bytes)
    check restored.code.len == 0
