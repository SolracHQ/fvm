# Build the project and move binary to ./bin/fvm
build:
  nimble build && mkdir -p bin && mv fvm bin/fvm

# Build and run with provided arguments
fvm *args: build
  ./bin/fvm {{args}}

# Build and run assembler with provided arguments
asm *args: build
  ./bin/fvm run-asm {{args}}

# Run tests
test:
  nimble test

# Run with provided arguments (run subcommand)
run *args: build
  ./bin/fvm run {{args}}

# Clean build artifacts and test binaries
clean:
  rm -rf bin
  find tests -type f ! -name "*.nim*" -delete

# Show available commands
help:
  @echo "Available commands:"
  @echo "  just build       - Build project to ./bin/fvm"
  @echo "  just fvm <args>  - Build and run with args"
  @echo "  just asm <args>  - Build and run assembler with args"
  @echo "  just test        - Run tests"
  @echo "  just run <args>  - Build and run with run subcommand"
  @echo "  just clean       - Clean build artifacts and test binaries"
  @echo "  just help        - Show this message"