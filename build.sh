#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
RUST_DIR="$ROOT/rust"
BIN_DIR="$ROOT/addons/gdbridge/bin"

echo "[1/3] Building Rust workspace..."
cd "$RUST_DIR"
cargo build

echo "[2/3] Copying library to Godot addons..."
mkdir -p "$BIN_DIR"
case "$(uname -s)" in
    Linux*)  cp -f "$RUST_DIR/target/debug/libgdbridge.so" "$BIN_DIR/libgdbridge.so" ;;
    Darwin*) cp -f "$RUST_DIR/target/debug/libgdbridge.dylib" "$BIN_DIR/libgdbridge.dylib" ;;
    *)       cp -f "$RUST_DIR/target/debug/gdbridge.dll" "$BIN_DIR/gdbridge.dll" ;;
esac

echo "[3/3] Done."
echo ""
echo "  Open client/project.godot in Godot to run."
