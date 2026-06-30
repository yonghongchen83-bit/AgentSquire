# Decisions

## D-001 Refactor Strategy
Adopt incremental refactor batches with strict verification after each batch.

## D-002 Priority Rule
Fix API and event contract mismatches before structural decomposition.

## D-003 Backend Modularization
Split command responsibilities into submodules and keep command registration centralized.

## D-004 Frontend Decomposition
Split large UI/store files by concern (catalog/config/tab UI/theme utilities and stream/parsing/state side effects).

## D-005 Behavior Preservation
No product behavior changes are allowed unless a mismatch or defect is directly fixed and covered by validation.

## D-006 Validation Gate
Each batch must pass lint and project build checks before moving to next batch.

