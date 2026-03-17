# FVM Assembly

Fantasy Assembly (`.fa`) is the assembly language for the FVM. Goal is simplicity: easy to
read, easy to parse, easy to execute. Each opcode is one byte and each register operand is one
byte; wasteful but keeps the toolchain trivial.

## Instruction format

```
MNEMONIC
MNEMONIC TARGET
MNEMONIC TARGET, SOURCE
```

Comments start with `#` and continue to the end of the line.
