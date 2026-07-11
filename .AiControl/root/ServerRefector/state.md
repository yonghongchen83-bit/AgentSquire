# State — ServerRefector

## Status
🟡 Planning phase — Node initialized, architecture analysis pending.

## Overview
This node handles server-side refactoring to decouple the engine from Tauri's AppState.
Two core abstractions to be designed and implemented:
1. **ModelInstance** — encapsulate provider+model+endpoint+options+apiKey into a single value
2. **RuntimeContext** — encapsulate all runtime dependencies (workspace, session, config) so engines run independently of AppState

## Progress
- [ ] ModelInstance design
- [ ] RuntimeContext design
- [ ] Implementation
- [ ] Migrate existing code
- [ ] Update tests

