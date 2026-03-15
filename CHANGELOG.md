# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-03-14
 
### Changed
 
- Replaced the placeholder fault info region with a loader info region. The loader now writes
  a structured boot-time record covering: the physical page ranges of all kernel sections
  (discovery table, loader info, rodata, code, data, stack), all physical memory regions with
  device ids, and all port-mapped devices with their port numbers and device ids. The kernel
  reads this at startup to know exactly which physical pages are reserved before doing any
  allocation. No binary format version change; no existing assembly breaks.
 
---

## [0.1.0] - 2026-03-14

This release marks the complete rewrite of FVM from Nim to Rust. The Nim implementation worked excellently, but the LSP broke due to cyclic dependencies and project size, making refactoring, autocomplete, and especially "go to definition" unreliable. Since the entire codebase was being rewritten anyway, the opportunity was taken to improve the bus and memory model, which had been more experimental in the original design. The architecture and instruction set remain recognizably the same, but now benefit from Rust's type system, proper LSP support, and thoughtful redesign of the memory subsystem.

### Added

- Rust implementation: Complete rewrite from Nim, providing better performance, type safety, and error handling
- Virtual memory: Page-based virtual address translation with per-context page tables
- Dual register files: Separate kernel and user mode register files, switching atomically via `DPL`
- Configurable memory: VM memory size is now configurable at startup (default 16 MiB) instead of fixed 64 KB
- Memory mapping operations: New instructions `MMAP`, `MUNMAP`, and `MPROTECT` for managing virtual address spaces
- Privileged register: New `mr` (mapping register) for specifying target context in memory mapping operations
- Fault info region: Reserved memory region for kernel to inspect fault details on interrupt delivery
- Device configuration: Config-based device setup with support for multiple device types (DecimalIo, HexIo, RawIo)
- Human-readable memory sizes: Config parser supports sizes like `"128mb"`, `"2gb"`, `"512kb"` in addition to raw bytes
- RON config support: Config files support both JSON and RON formats; script uses `rq` for parsing
- 32-bit registers: Upgraded from 16-bit to 32-bit general-purpose registers with multi-width views (`rw`, `rh`, `rb`)
- Context register (`cr`): New register for virtual address space identification
- Instruction pointer register (`ip`): Made explicit as a readable register (kernel-writable)
- Extended shift support: Full suite of shift operations `SHL`, `SHR`, `SAR`, `ROL`, `ROR`
- Memory management instructions: `TUR`, `TKR` for privileged register access
- Comprehensive test suite: Unit tests for assembler, VM core, and integration tests

### Changed

- Memory model: Redesigned from a simple flat 64 KB backing array to a flexible, permission-checked memory bus with virtual address translation. This provides a solid foundation for OS experiments without fundamental rearchitecting.
- Address space: Enlarged from 64 KB to configurable (default 16 MiB) with support for multiple device regions and proper page-level isolation.
- Register width: Upgraded from 16-bit to 32-bit general-purpose registers with multi-width views (`rw`, `rh`, `rb`) for better arithmetic range and data handling.
- Privilege model: Separated kernel and user mode into distinct register files that switch atomically, replacing the simpler global privilege flag.
- Device attachment: Devices now declared in a config file at startup rather than wired via runtime CLI flags; cleaner abstraction for future device types.
- Binary format: Updated `.fo` object file format to include relocation entries and support larger address spaces; assembler now emits proper relocations for data section labels.
- Opcode encoding: Updated to support new privilege instructions and register encodings.
- CLI: Switched from separate ad-hoc port mapping to declarative device configuration in RON or JSON.
- Build system: Switched from Nim tools to Cargo workspace structure for better tooling and faster compilation feedback.
- Architecture documentation: References now point to Rust implementation with better integration into the language docs.

### Fixed

- Tooling support: Rust's LSP ecosystem is mature and reliable at scale; no recurrence of the Nim LSP issues
- Type safety: Rust's type system prevents entire categories of memory safety mistakes; no unsafe code outside necessary FFI bindings
- Undefined behavior: Arithmetic operations have well-defined semantics; no unspecified overflow behavior
- Error handling: All operations return `Result` types for explicit error propagation and better stack traces

### Removed

- Nim codebase: Replaced entirely with Rust implementation; Nim version archived for reference
- 16-bit registers: Replaced with 32-bit registers with multi-width views for better arithmetic range
- Fixed 64 KB memory: Replaced with configurable memory model supporting multiple device regions
- Simple port mapping CLI: Replaced with declarative device configuration system

### Migration Guide

The Nim implementation (0.0.0) was solid and well-designed. If you need to use pre-0.1.0 object files:

1. **Register naming changed**: `r0` → `rw0`, `r0l` → `rb0`, `r0h` → `rh0`
2. **Memory configuration**: Moved from hardcoded 64 KB to config file (supports human-readable sizes like `"128mb"`)
3. **I/O device setup**: Replace old `--map` CLI flags with device declarations in RON config
4. **Object files**: Old `.fo` format is incompatible; reassemble all `.fa` sources with the new Rust assembler
5. **Build tooling**: `cargo build` / `just build` replaces Nim build steps

The instruction set is essentially the same, but with new instructions for virtual memory management (`MMAP`, `MUNMAP`, `MPROTECT`) and privileged register access (`TUR`, `TKR`).

### Known Limitations

- Fault info region layout is provisional (version 0 status); may change to include more details in future releases
- Interrupt vector table is VM-resident and not user-accessible; kernel must provide managed access abstraction
- Step debugger UI not yet implemented; debugging requires reading registers via instruction output
- Virtual filesystem not implemented
- No block device support

---

## [0.0.0] - Nim implementation (archived)

The original FVM implementation in Nim. Fully functional and well-designed; replaced by 0.1.0 due to Rust tooling maturity (LSP, refactoring support, "go to definition") rather than implementation quality.

### 0.0.0 Features

- 16-bit general-purpose registers with byte-level access
- 64 KB fixed memory with read/write/execute permissions per region
- Complete instruction set: arithmetic, logic, control flow, memory access, I/O, interrupts
- 256 interrupt vectors with privileged interrupt handling
- 256 port-mapped I/O addresses with configurable device attachment
- Fantasy assembly language: labels, local scopes, directives, comments
- Single-pass assembler producing compact bytecode

The Nim codebase is preserved in version control history for reference.
