import path from 'path'
import { execSync, spawn, type ChildProcess } from 'child_process'

const ROOT = path.resolve(__dirname, '..', '..')

let appProcess: ChildProcess | null = null

export function startApp(): void {
  const binaryPath = path.join(
    ROOT,
    'src-tauri',
    'target',
    'debug',
    'squirecli.exe',
  )
  appProcess = spawn(binaryPath, [], {
    cwd: ROOT,
    stdio: 'ignore',
    env: { ...process.env },
  })
  console.log(`[tauri] Started app (pid: ${appProcess.pid})`)
}

export function stopApp(): void {
  if (appProcess) {
    appProcess.kill()
    appProcess = null
    console.log('[tauri] Stopped app')
  }
}

export function buildApp(): void {
  console.log('[tauri] Building app...')
  execSync('cargo build -p squirecli_lib', {
    cwd: path.join(ROOT, 'src-tauri'),
    stdio: 'inherit',
  })
  console.log('[tauri] Build complete')
}

export function isAppRunning(): boolean {
  return appProcess !== null && appProcess.exitCode === null
}
