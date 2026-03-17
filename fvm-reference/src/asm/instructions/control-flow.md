# Control flow

## JMP

Unconditional jump. Register form enables indirect jumps and must use an `rw` register.

```
JMP label       # opcode + addr32
JMP rw          # opcode + enc  (indirect)
JMP 0x12345678  # opcode + addr32 literal
```

## JZ

Jump if zero flag is set (last result was zero, or operands were equal).

```
JZ  label
JZ  rw
```

## JNZ

Jump if zero flag is clear.

```
JNZ label
JNZ rw
```

## JC

Jump if carry flag is set (unsigned overflow or borrow).

```
JC  label
JC  rw
```

## JN

Jump if negative flag is set (high bit of last result was 1).

```
JN  label
JN  rw
```

## JO

Jump if overflow flag is set (signed arithmetic overflow occurred).

```
JO  label
JO  rw
```

## JNO

Jump if overflow flag is clear (no signed arithmetic overflow).

```
JNO label
JNO rw
```

## CALL

Pushes the return address onto the stack then jumps. The return address is `IP + 5` for the
immediate form and `IP + 2` for the register form.

```
CALL label      # opcode + addr32
CALL rw         # opcode + enc  (indirect)
```

## RET

Pops a 32-bit address off the stack and jumps to it. Paired with `CALL`.

```
RET
```

## NOP

Does nothing, advances IP by one byte.

```
NOP
```

## HALT

Stops execution.

```
HALT
```
