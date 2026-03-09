# FVM: Virtual Machine Design Notes

Planned VM work that is still ahead of the current implementation.

---

## Future reset / shutdown interrupt flow

Vector 0 is intentionally left unused today. One possible next step is to use
it for a VM-level reset or shutdown hook so guest code can run cleanup logic
before the machine stops.

The exact trigger source is still open. It may come from a host-side reset /
power button concept rather than from initial VM startup.

---

## Process context register and address spaces

The next privilege-related building block is a process context register,
settable only through a privileged instruction, that identifies the active
address space. Once the bus consults it, this becomes the basis for per-process
memory maps.

The register itself is easy to add. Bus-level translation and region lookup by
context are the real work and are still future work.

---

## MMAP

Once multiple address spaces exist, MMAP can let kernel code create, resize,
and change memory mappings at runtime.

Rough direction:

```
MMAP r0, r1, r2   # base in r0, length in r1, permission flags in r2
```

Permission flags would reuse the existing `Permission` enum values.

---

## Nested interrupts

The current VM keeps only one `InterruptContext` and drops nested interrupts.
That is deliberate for simplicity. A future nested-interrupt stack would allow
handlers to be interrupted cleanly when there is a concrete need for it.

---

## Device-raised interrupts

Device callbacks do not raise interrupts yet. The intended direction is for bus
devices and port devices to be able to signal hardware events through the same
`raiseInterrupt()` path the core already uses.
