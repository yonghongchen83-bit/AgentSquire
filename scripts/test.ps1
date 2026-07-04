<#
.SYNOPSIS
    Run Rust unit/integration tests (lib scope). Does NOT accept arbitrary CLI args.
    For specific test filtering, modify the script or ask the user.
.NOTES
    This script is the ONLY authorized way to run Rust tests.
    Do NOT run cargo test directly.
#>

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$CargoManifest = Join-Path (Join-Path $ProjectRoot "src-tauri") "Cargo.toml"

Write-Host "=== Test: Rust unit + integration tests ===" -ForegroundColor Cyan

cargo test --manifest-path $CargoManifest --lib

if ($LASTEXITCODE -ne 0) {
    Write-Host "TESTS FAILED (exit code $LASTEXITCODE)" -ForegroundColor Red
    exit $LASTEXITCODE
}

Write-Host "All tests passed." -ForegroundColor Green
