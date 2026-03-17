# Build the Rust workspace
build:
  cargo build --workspace

# Run the assembler with provided arguments
asm *args:
  cargo run -p fvm-assembler -- {{args}}

# Assemble an example under examples/ by stem, keeping outputs under target/examples/
asm-example example:
  if [ ! -f examples/{{example}}.fa ] && [ -f examples/$(printf '%s' {{example}} | sed 's#^include_example/error_#include_errors/error_#').fa ]; then echo "example moved: examples/{{example}}.fa -> examples/$(printf '%s' {{example}} | sed 's#^include_example/error_#include_errors/error_#').fa" >&2; exit 1; fi
  if [ ! -f examples/{{example}}.fa ]; then echo "example source not found: examples/{{example}}.fa" >&2; exit 1; fi
  mkdir -p target/examples/$(dirname {{example}})
  cargo run -p fvm-assembler -- examples/{{example}}.fa --output target/examples/{{example}}.fo

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

# Build the mdbook reference and copy to /docs
doc:
  rm -rf docs
  cd fvm-reference && mdbook build
  mv fvm-reference/book docs

# Show available commands
help:
  @echo "Available commands:"
  @echo "  just build       - Build the Rust workspace"
  @echo "  just asm <args>  - Run the Rust assembler via cargo run"
  @echo "  just asm-example <name> - Assemble examples/<name>.fa into target/examples/<name>.fo"
  @echo "  just asm-exec <json> - Assemble the example named by .rom and run it with a JSON VM config"
  @echo "  just run <args>  - Run the Rust VM via cargo run"
  @echo "  just test <args> - Run Rust tests via cargo test"
  @echo "  just clean       - Remove cargo artifacts"
  @echo "  just doc         - Build mdbook reference and copy to /docs"
  @echo "  just help        - Show this message"