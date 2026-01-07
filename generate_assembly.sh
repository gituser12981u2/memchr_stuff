#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

OUTPUT_DIR="./assembly_outputs"
mkdir -p "$OUTPUT_DIR"

if ! command -v cargo >/dev/null 2>&1; then
	echo "error: cargo not found on PATH" >&2
	exit 127
fi



cargo-asm --lib "memchr_stuff::memchr_new::memchr" > "$OUTPUT_DIR/memchr_new.asm.txt"
cargo-asm --lib "memchr_stuff::memchr_new::memrchr" > "$OUTPUT_DIR/memrchr_new.asm.txt"
cargo-asm --lib "memchr_stuff::memchr_old::memchr" > "$OUTPUT_DIR/memchr_old.asm.txt"
cargo-asm --lib "memchr_stuff::memchr_old::memrchr" > "$OUTPUT_DIR/memrchr_old.asm.txt"
