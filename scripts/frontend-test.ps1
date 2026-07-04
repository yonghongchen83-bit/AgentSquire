<#
.SYNOPSIS
    Run frontend unit tests (Vitest). Does NOT accept arbitrary CLI args.
.NOTES
    Fast — no Rust compilation needed.
#>

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot

Write-Host "=== Test: Frontend (Vitest) ===" -ForegroundColor Cyan

Push-Location $ProjectRoot
try {
    npm test
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FRONTEND TESTS FAILED (exit code $LASTEXITCODE)" -ForegroundColor Red
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}

Write-Host "All frontend tests passed." -ForegroundColor Green
