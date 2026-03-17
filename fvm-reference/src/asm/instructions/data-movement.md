# Data movement

## MOV

Copies a value. Both operands must use the same register view.

```
MOV  rw, imm32
MOV  rw, label      # loads the address of label as imm32
MOV  rw, rw
MOV  rw, sp
MOV  sp, rw
MOV  rw, cr
MOV  cr, rw         # privileged, writes kernel cr only
MOV  rw, mr
MOV  mr, rw         # privileged
MOV  rh, imm16
MOV  rh, rh
MOV  rb, imm8
MOV  rb, 'A'        # char literal immediate
MOV  rb, rb
```

`ip` is not a valid MOV operand. Use `TKR ip, rw` to set the user instruction pointer from
kernel mode.

To move a narrower view into a wider register use `ZEXT` or `SEXT` first.

## PUSH

Decrements SP then writes the value to the stack. Width is determined by the register view.

```
PUSH rw     # SP -= 4, writes 32-bit value
PUSH rh     # SP -= 2, writes 16-bit value
PUSH rb     # SP -= 1, writes byte
```

## POP

Reads from the stack then increments SP. Writing a narrower view leaves upper bits of the
register untouched.

```
POP  rw     # reads 32-bit value, SP += 4
POP  rh     # reads 16-bit value into low 16, SP += 2
POP  rb     # reads byte into low 8, SP += 1
```

## ZEXT

Zero-extends a narrower view into a wider register. Bits above the source width are set to
zero. Source and destination can be different registers.

```
ZEXT rw, rh     # zero-extend 16-bit into 32-bit
ZEXT rw, rb     # zero-extend 8-bit into 32-bit
ZEXT rh, rb     # zero-extend 8-bit into 16-bit
```

## SEXT

Sign-extends a narrower view into a wider register. The high bit of the source is replicated
into all bits above it in the destination. Source and destination can be different registers.

```
SEXT rw, rh     # sign-extend 16-bit into 32-bit
SEXT rw, rb     # sign-extend 8-bit into 32-bit
SEXT rh, rb     # sign-extend 8-bit into 16-bit
```
