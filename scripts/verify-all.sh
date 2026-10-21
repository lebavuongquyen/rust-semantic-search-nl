#!/usr/bin/env bash
set -euo pipefail

echo "========================================="
echo "  SemanticSearchAgent Quality Gate Check "
echo "========================================="

echo "[1/5] Checking code formatting..."
npm run format:check

echo "[2/5] Running TypeScript typecheck..."
npm run typecheck

echo "[3/5] Running Rust test suite..."
cargo test

echo "[4/5] Checking WebAssembly target compilation..."
cargo check --target wasm32-unknown-unknown

echo "[5/5] Running TypeScript contract & E2E tests..."
npm run test:ts

echo "========================================="
echo "  All Quality Gates Passed Successfully! "
echo "========================================="
