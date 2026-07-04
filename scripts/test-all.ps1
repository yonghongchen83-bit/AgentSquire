<#
.SYNOPSIS
    Run ALL Rust tests (lib + integration tests + examples/doc-tests).
    Does NOT accept arbitrary CLI args.
.NOTES
    Slower than scripts/test.ps1 — only use when integration tests are needed.
#>

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$CargoManifest = Join-Path (Join-Path $ProjectRoot "src-tauri") "Cargo.toml"

Write-Host "=== Test: ALL Rust tests ===" -ForegroundColor Cyan

cargo test --manifest-path $CargoManifest

if ($LASTEXITCODE -ne 0) {
    Write-Host "TESTS FAILED (exit code $LASTEXITCODE)" -ForegroundColor Red
    exit $LASTEXITCODE
}

Write-Host "All tests passed." -ForegroundColor Green
