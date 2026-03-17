# Shifts and rotations

The shift amount `n` is always either an `rb` register or an `imm8`. The destination width is
determined by the register view used.

## SHL

Shifts destination left by `n` bits. Zeros fill from the right. If `n` >= width of the
destination view, the result is zero.

Flags: Z set if result is zero, N set if high bit of result is set, C set to the last bit
shifted out of the top.

```
SHL rw, rb
SHL rw, imm8
SHL rh, rb
SHL rh, imm8
SHL rb, rb
SHL rb, imm8
```

## SHR

Shifts destination right by `n` bits logically. Zeros fill from the left. If `n` >= width of
the destination view, the result is zero.

Flags: Z set if result is zero, N set if high bit of result is set, C set to the last bit
shifted out of the bottom.

```
SHR rw, rb
SHR rw, imm8
SHR rh, rb
SHR rh, imm8
SHR rb, rb
SHR rb, imm8
```

## SAR

Shifts destination right by `n` bits arithmetically. The sign bit is replicated into vacated
bits from the left. If `n` >= width of the destination view, the result is all sign bits
(`0x00` or `0xFF` / `0xFFFF` / `0xFFFFFFFF`).

Flags: Z set if result is zero, N set if high bit of result is set, C set to the last bit
shifted out of the bottom.

```
SAR rw, rb
SAR rw, imm8
SAR rh, rb
SAR rh, imm8
SAR rb, rb
SAR rb, imm8
```

## ROL

Rotates destination left by `n` bits. Bits shifted out of the top wrap into the bottom.

Flags: Z set if result is zero, N set if high bit of result is set, C unaffected.

```
ROL rw, rb
ROL rw, imm8
ROL rh, rb
ROL rh, imm8
ROL rb, rb
ROL rb, imm8
```

## ROR

Rotates destination right by `n` bits. Bits shifted out of the bottom wrap into the top.

Flags: Z set if result is zero, N set if high bit of result is set, C unaffected.

```
ROR rw, rb
ROR rw, imm8
ROR rh, rb
ROR rh, imm8
ROR rb, rb
ROR rb, imm8
```
