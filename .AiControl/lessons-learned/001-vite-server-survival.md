# Lesson 001: Vite Dev Server Dies During WDIO Tests

## Problem

When running WDIO tests, `browser.url('http://localhost:5173/')` returns `net::ERR_CONNECTION_REFUSED`. The app renders a blank/error page. Tests fail with "unable to connect" or "invalid session id".

## Symptoms

```
unknown error: net::ERR_CONNECTION_REFUSED
  (Session info: MicrosoftEdge=149.0.4022.98) when running "url" with method "POST"
```

Or tests time out waiting for elements:

```
Error: waitUntil condition failed with the following reason: WebDriverError: invalid session id
```

## Root Cause

The Vite dev server was started via the shell tool (e.g. `pnpm dev` inside a `Start-Process` call). When the shell tool's timeout expires, the entire process tree is killed — including Vite.

**Why `Start-Process` didn't work:**

```powershell
# ❌ FAILS: pnpm is resolved through PATH, Start-Process with UseShellExecute=$false
# doesn't see the PATH
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "pnpm"  # "The system cannot find the file specified"
$psi.Arguments = "dev"
$psi.UseShellExecute = $false  # No PATH lookup

# ❌ FAILS: Even with UseShellExecute=$true (default), the shell tool's
# timeout eventually kills the process tree
Start-Process -WindowStyle Hidden -FilePath "pnpm" -ArgumentList "dev"
```

## Fix

Launch Vite using `cmd.exe /c` to decouple it from the shell tool's process tree:

```powershell
Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList @"
/c cd /d D:\work\MyAgent && npx vite --port 5173
"@
```

This works because:
- `cmd.exe` is a Windows system binary found via absolute PATH
- `cmd /c` creates a new process tree independent of the shell tool
- The shell tool's timeout doesn't propagate kill signals across `cmd.exe` boundaries

## Prevention

1. **Use a startup script**: Create `e2e/start-test-env.ps1` that launches Vite + tauri-driver + MSEdgeDriver in a single command.
2. **Verify before running**: Always check `Invoke-WebRequest -Uri http://localhost:5173/` returns 200 before launching WDIO.
3. **Document the command**: Keep the exact `cmd.exe /c` invocation in the test skill's prerequisites.

## Verification

```powershell
# Check Vite is alive
try { $r = Invoke-WebRequest -Uri "http://localhost:5173/" -UseBasicParsing -TimeoutSec 5; $r.StatusCode } catch { "DOWN" }
# Should return 200
```
