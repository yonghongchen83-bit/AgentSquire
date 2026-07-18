# Decisions — Architecture Review

| # | Decision | Rationale |
|---|----------|-----------|
| D1 | Current implementation has 10 distinct token_type values, not "7+" | Grepped every `token_type = "..."` assignment across the codebase. Types: `concept`, `referential`, `system_referential`, `tool`, `skill`, `workflow`, `user`, `decision`, `assumption`, `todo`. |
| D2 | New spec collapses these to 3 (source, referential, concept) + relationships | Spec §2: token type ≠ role. Workflows/tools/skills/etc. become graph-assigned roles (via relationship triplets), not hardcoded type fields. |
| D3 | Backward compatibility NOT required — store data can be wiped | Stakeholder directive. Eliminates all migration/schema-versioning concerns. This is the single biggest risk reducer for the token model redesign. |
| D4 | All tokens prefixed with turn number in the backend | Stakeholder directive. Duplicate-free by construction. Currently only applies to auto-chunked tokens (USR_T{turn}_{NNN}); will need to be extended to all token types. |
| D5 | Pivot to new spec architecture now recommended (Option 1) | The wipe-on-deploy + turn-number-prefix constraints reverse the earlier recommendation. The token model redesign becomes a clean schema swap rather than an in-place migration; turn-number prefixing aligns naturally with source-token-based storage. |
