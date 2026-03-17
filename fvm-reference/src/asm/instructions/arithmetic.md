# Arithmetic

## ADD

Adds source to destination, result in destination. Source can be a register or immediate.

Flags: C set on unsigned overflow, N set if high bit of result is set.

```
ADD rw, rw
ADD rw, imm32
ADD rh, rh
ADD rh, imm16
ADD rb, rb
ADD rb, imm8
```

## SUB

Subtracts source from destination, result in destination. Source can be a register or
immediate.

Flags: C set on unsigned underflow (borrow), N set if high bit of result is set.

```
SUB rw, rw
SUB rw, imm32
SUB rh, rh
SUB rh, imm16
SUB rb, rb
SUB rb, imm8
```

## CMP

Computes `dst - src` and sets flags, discards the result. Source can be a register or
immediate. Use before conditional jumps.

Flags: Z set if equal, C set if src > dst (unsigned), N set if high bit of result is set.

```
CMP rw, rw
CMP rw, imm32
CMP rh, rh
CMP rh, imm16
CMP rb, rb
CMP rb, imm8
```

## MUL

Multiplies source by destination as unsigned integers, result in destination. Source can be a register or immediate.

Flags: C set on unsigned overflow, O set on overflow, N set if high bit of result is set.

```
MUL rw, rw
MUL rw, imm32
MUL rh, rh
MUL rh, imm16
MUL rb, rb
MUL rb, imm8
```

## SMUL

Multiplies source by destination as signed integers, result in destination. Source can be a register or immediate.

Flags: C set on overflow, O set if signed overflow occurred, N set if high bit of result is set.

```
SMUL rw, rw
SMUL rw, imm32
SMUL rh, rh
SMUL rh, imm16
SMUL rb, rb
SMUL rb, imm8
```

## DIV

Divides destination by source as unsigned integers, result in destination. Source can be a register or immediate.
Raises a fault if source is zero.

Flags: N set if high bit of result is set.

```
DIV rw, rw
DIV rw, imm32
DIV rh, rh
DIV rh, imm16
DIV rb, rb
DIV rb, imm8
```

## SDIV

Divides destination by source as signed integers, result in destination. Source can be a register or immediate.
Raises a fault if source is zero.

Flags: N set if high bit of result is set.

```
SDIV rw, rw
SDIV rw, imm32
SDIV rh, rh
SDIV rh, imm16
SDIV rb, rb
SDIV rb, imm8
```

## MOD

Computes the remainder of destination divided by source as unsigned integers, result in destination.
Source can be a register or immediate. Raises a fault if source is zero.

Flags: N set if high bit of result is set.

```
MOD rw, rw
MOD rw, imm32
MOD rh, rh
MOD rh, imm16
MOD rb, rb
MOD rb, imm8
```

## SMOD

Computes the remainder of destination divided by source as signed integers, result in destination.
Source can be a register or immediate. Raises a fault if source is zero.

Flags: N set if high bit of result is set.

```
SMOD rw, rw
SMOD rw, imm32
SMOD rh, rh
SMOD rh, imm16
SMOD rb, rb
SMOD rb, imm8
```
