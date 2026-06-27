# Env

## Tauri v2 E2E Testing Setup

### Installed Components
| Component | Version | Location |
|-----------|---------|----------|
| tauri-driver | 2.0.6 | `cargo install` → `~/.cargo/bin/tauri-driver.exe` |
| @wdio/cli | 9.29.0 | `node_modules/.bin/wdio` |
| @wdio/local-runner | 9.29.0 | npm |
| @wdio/mocha-framework | 9.29.0 | npm |
| @wdio/spec-reporter | 9.29.0 | npm |
| @wdio/globals | 9.29.0 | npm |

### File Structure
```
e2e/
├── wdio.conf.ts          # WDIO configuration → tauri-driver:4444
├── tsconfig.json         # TypeScript config for E2E specs
├── helpers/
│   └── tauri.ts          # App build/start/stop helpers
└── specs/
    └── app.test.ts       # Smoke tests

src-tauri/tests/
└── integration_test.rs   # Rust integration tests
```

### Available Scripts
| Script | Command | Description |
|--------|---------|-------------|
| `npm test` | `vitest run` | Frontend unit tests |
| `npm run test:e2e` | `wdio run ./e2e/wdio.conf.ts` | E2E tests (app + tauri-driver must be running) |
| `npm run test:e2e:dev` | `tauri-driver & wdio run ./e2e/wdio.conf.ts` | Start tauri-driver + run E2E |
| `npm run test:rust` | `cargo test -p squirecli_lib` | Rust integration tests |

### How to Run Full E2E Test
```powershell
# Terminal 1: Start tauri-driver
tauri-driver

# Terminal 2: Start the Tauri app
cd src-tauri && cargo build -p squirecli_lib
./target/debug/squirecli.exe

# Terminal 3: Run E2E tests
npm run test:e2e
```
