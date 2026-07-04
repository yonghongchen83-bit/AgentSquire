<#
.SYNOPSIS
    Launch the full app (Tauri dev mode): starts Vite dev server + builds Rust
    backend + opens the desktop window. Does NOT accept arbitrary CLI args.
.NOTES
    This script is the ONLY authorized way to run the app in dev mode.
    Do NOT run 'npm run tauri dev' or 'cargo build' directly.
    Uses existing build cache. Run scripts/clean.ps1 first if you need a full rebuild.
#>

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot

Write-Host "=== Run: Tauri dev (full app) ===" -ForegroundColor Cyan
Write-Host "NOTE: The first compilation after a clean takes ~30 minutes." -ForegroundColor Yellow
Write-Host "      Use existing cache unless you really need a clean build." -ForegroundColor Yellow
Write-Host ""

Push-Location $ProjectRoot
try {
    npm run tauri dev
    if ($LASTEXITCODE -ne 0) {
        Write-Host "APP EXIT CODE: $LASTEXITCODE" -ForegroundColor Yellow
    }
} finally {
    Pop-Location
}
