$ErrorActionPreference = "Stop"

Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "  SemanticSearchAgent Quality Gate Check " -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan

Write-Host "`n[1/5] Checking code formatting..." -ForegroundColor Yellow
npm run format:check

Write-Host "`n[2/5] Running TypeScript typecheck..." -ForegroundColor Yellow
npm run typecheck

Write-Host "`n[3/5] Running Rust test suite..." -ForegroundColor Yellow
cargo test

Write-Host "`n[4/5] Checking WebAssembly target compilation..." -ForegroundColor Yellow
cargo check --target wasm32-unknown-unknown

Write-Host "`n[5/5] Running TypeScript contract & E2E tests..." -ForegroundColor Yellow
npm run test:ts

Write-Host "`n=========================================" -ForegroundColor Green
Write-Host "  All Quality Gates Passed Successfully! " -ForegroundColor Green
Write-Host "=========================================" -ForegroundColor Green
