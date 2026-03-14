#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: scripts/asm-exec.sh '<ron-or-config-path>'" >&2
  exit 1
fi

config_input="$1"

if [[ -f "$config_input" ]]; then
  config_ron="$(cat "$config_input")"
else
  config_ron="$config_input"
fi

if ! command -v rq >/dev/null 2>&1; then
  echo "rq is required for asm-exec" >&2
  exit 1
fi

rom_path="$(printf '%s\n' "$config_ron" | rq 'rom')"

if [[ -z "$rom_path" || "$rom_path" == "null" ]]; then
  echo "config must provide a non-empty .rom path" >&2
  exit 1
fi

example_name="$(basename "$rom_path" .fo)"
source_path="examples/${example_name}.fa"

if [[ ! -f "$source_path" ]]; then
  echo "example source not found: $source_path" >&2
  exit 1
fi

mkdir -p "$(dirname "$rom_path")"

cargo run -p fvm-assembler -- "$source_path" --output "$rom_path"
cargo run -p fvm-vm -- --config "$config_ron"