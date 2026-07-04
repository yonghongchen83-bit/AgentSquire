<#
.SYNOPSIS
    Clean Rust build artifacts (cargo clean) to force a full rebuild.
    
    WARNING: This triggers a ~30-minute full recompilation of all dependencies.
    Only run this when absolutely necessary (e.g., corrupted build cache,
    Cargo.toml changes that won't incrementally resolve, switching toolchains).
    
    This script REQUIRES interactive confirmation before proceeding.
.NOTES
    NEVER run this automatically. Always ask the user for explicit permission.
    The user must type 'yes' to proceed.
#>

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$CargoManifest = Join-Path (Join-Path $ProjectRoot "src-tauri") "Cargo.toml"

Write-Host "=============================================================" -ForegroundColor Red
Write-Host "  WARNING: Full Rust clean + rebuild" -ForegroundColor Red
Write-Host "  This will DELETE all build artifacts." -ForegroundColor Red
Write-Host "  Next build will take ~30 minutes." -ForegroundColor Red
Write-Host "=============================================================" -ForegroundColor Red
Write-Host ""
Write-Host "Target: $CargoManifest" -ForegroundColor Gray
Write-Host ""

$confirmation = Read-Host "Type 'yes' to clean, anything else to cancel"
if ($confirmation -ne "yes") {
    Write-Host "Clean cancelled." -ForegroundColor Yellow
    exit 0
}

Write-Host "Cleaning Rust build artifacts..." -ForegroundColor Cyan
cargo clean --manifest-path $CargoManifest

if ($LASTEXITCODE -ne 0) {
    Write-Host "CLEAN FAILED (exit code $LASTEXITCODE)" -ForegroundColor Red
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "Clean complete. Next build will recompile everything (~30 min)." -ForegroundColor Yellow
