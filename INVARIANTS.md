# Mirage Invariants

## Path Normalization
All file paths normalized before Magellan queries.

## No Silent First-Match
Ambiguous lookups return explicit errors, never first-match.

## Explicit Ambiguity Handling
All lookups return: `Unique`, `Ambiguous`, or `NotFound`.

## Adapter-Only Backend Access
All Magellan access goes through `MagellanAdapter`.

## Numeric ID Resolution
Names resolved to IDs before graph operations.

## Backend Detection
Database format detected by extension and magic bytes.

## No SQLite-Only Shortcuts
All backends support same operations via trait abstraction.

## Release Invariants

1. Zero production-code warnings
2. All tests pass
3. Docs match code
4. Backend parity maintained
