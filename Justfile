# Build the Rust workspace
build:
  cargo build --workspace

# Run the assembler with provided arguments
asm *args:
  cargo run -p fvm-assembler -- {{args}}

# Assemble the example referenced by `.rom` in a JSON config, then execute it.
asm-exec config:
  bash scripts/asm-exec.sh '{{config}}'

# Run the VM with provided arguments
run *args:
  cargo run -p fvm-vm -- {{args}}

# Run all Rust tests
test *args:
  cargo test {{args}}

# Clean Rust build artifacts
clean:
  cargo clean

# Show available commands
help:
  @echo "Available commands:"
  @echo "  just build       - Build the Rust workspace"
  @echo "  just asm <args>  - Run the Rust assembler via cargo run"
  @echo "  just asm-exec <json> - Assemble the example named by .rom and run it with a JSON VM config"
  @echo "  just run <args>  - Run the Rust VM via cargo run"
  @echo "  just test <args> - Run Rust tests via cargo test"
  @echo "  just clean       - Remove cargo artifacts"
  @echo "  just help        - Show this message"