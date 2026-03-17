# Bitwise

## AND

Bitwise AND, result in destination. Source can be a register or immediate. Flags: N.

```
AND rw, rw
AND rw, imm32
AND rh, rh
AND rh, imm16
AND rb, rb
AND rb, imm8
```

## OR

Bitwise OR, result in destination. Source can be a register or immediate. Flags: N.

```
OR  rw, rw
OR  rw, imm32
OR  rh, rh
OR  rh, imm16
OR  rb, rb
OR  rb, imm8
```

## XOR

Bitwise XOR, result in destination. Source can be a register or immediate. Flags: N.
`XOR rw, rw` is the canonical way to zero a register.

```
XOR rw, rw
XOR rw, imm32
XOR rh, rh
XOR rh, imm16
XOR rb, rb
XOR rb, imm8
```

## NOT

Bitwise NOT in place. Flags: N.

```
NOT rw
NOT rh
NOT rb
```
