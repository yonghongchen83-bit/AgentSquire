<#
.SYNOPSIS
    Build the Rust backend (debug profile). Does NOT accept arbitrary CLI args.
    Uses the pre-configured cargo build command matching what tauri dev uses.
    Incremental: relies on existing build cache. If you need a full rebuild,
    run scripts/clean.ps1 first (and only after user confirmation).
.NOTES
    This script is the ONLY authorized way to build Rust code.
    Do NOT run cargo build directly — it bypasses the locked config.
#>

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$CargoManifest = Join-Path (Join-Path $ProjectRoot "src-tauri") "Cargo.toml"

Write-Host "=== Build: Rust backend (debug) ===" -ForegroundColor Cyan

cargo build -vv --manifest-path $CargoManifest --no-default-features

if ($LASTEXITCODE -ne 0) {
    Write-Host "BUILD FAILED (exit code $LASTEXITCODE)" -ForegroundColor Red
    exit $LASTEXITCODE
}

Write-Host "Build succeeded." -ForegroundColor Green
