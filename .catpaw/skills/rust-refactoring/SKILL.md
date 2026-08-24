---
name: rust-refactoring
description: Behavior-aware Rust refactoring for the EchoMind workspace. Use when refactoring Rust code, renaming symbols, restructuring modules, removing duplication, hardening panics, fixing Clippy failures, or migrating feature-gated code across crates. Covers crate boundaries, trait ownership, async fn in trait (Edition 2024), workspace deny lints, and hexagonal architecture dependency direction.
---

# Rust Refactoring Specialist

Adapted from [agency-agents](https://github.com/msitarzewski/agency-agents) Rust Refactoring Specialist, tailored for EchoMind's Rust + Tauri v2 hexagonal architecture.

## EchoMind-Specific Constraints

### Workspace Lints (Iron Rule 5)

`Cargo.toml` `[workspace.lints.clippy]` denies: `unwrap_used`, `expect_used`, `panic`, `unreachable`, `todo`, `unimplemented`. Production code must return `Err`; test code uses `#![allow(...)]` exemptions.

### Edition 2024

- Native `async fn` in trait — **no `#[async_trait]` macro**
- `#![allow(async_fn_in_trait)]` is a deliberate exemption (Send + Sync supertrait constrains all implementors)
- Requires Rust toolchain >= 1.85

### Dependency Direction (Strictly Inward)

```
crates/models -> crates/prompt -> crates/core -> crates/infra -> crates/tauri-app
                                     ^
                              crates/compact
```

`crates/core` must NEVER depend on Tauri, reqwest, rusqlite, or any concrete implementation.

### Pro Feature Gating

`#[cfg(feature = "pro")]` gates optional functionality. Tests use `#[cfg(all(test, feature = "pro"))]`.

## Critical Rules

1. **No arbitrary refactor limit.** Semantic coherence defines the boundary, not file count.
2. **No unrelated churn.** Every changed line must belong to the requested transformation.
3. **No silent public breakage.** Obtain authorization before changing externally reachable APIs, IPC commands, CLI, config, features, wire formats, serialization, or persistence contracts.
4. **No half-migrations.** Update definitions, references, tests, docs, module declarations, macros, build scripts, and string-based paths together. Update `docs/api/` in the same PR (Iron Rule 7).
5. **No unsafe shortcuts.** Never introduce `unsafe` to bypass ownership/borrowing/lifetime constraints (Iron Rule 5: `forbid(unsafe_code)` in core+infra).
6. **No test manipulation.** Never weaken, skip, or rewrite tests merely to accept changed behavior. TDD: Red-Green-Refactor (Iron Rule 2).
7. **No silent data loss.** Never replace an error with empty/default/sentinel unless the contract explicitly requires it.
8. **No speculative abstractions.** Do not add traits, generics, macros, dependencies, or design patterns merely to look idiomatic (SOLID principles).
9. **No unsupported claims.** Claim speedups only after comparable measurement.
10. **No destructive Git operations.** Never discard user work, force-checkout, reset, clean, or publish without explicit authorization.
11. **No secret exposure.** Never print, copy, commit, or alter credentials discovered during inspection.

## Refactoring Opportunity Inventory Format

Every audit finding:

```markdown
### RUST-007 — Ownership — Avoid repeated path allocation
- **Location**: `crates/config/src/loader.rs`, `load_workspace`
- **Evidence**: All four callers already retain a borrowed `&Path`, but the function accepts `PathBuf`.
- **End state**: Accept `&Path`; update all callers and tests.
- **Coupled changes**: `loader.rs`, `workspace.rs`, integration fixtures.
- **API/behavior impact**: Internal signature only; filesystem and error behavior unchanged.
- **Risk/value**: Low risk, medium value.
- **Verification**: Targeted loader tests, workspace check, Clippy, diff review.
```

## Workflow

1. **Interpret** — Classify as audit, implementation, explanation, or plan. Establish scope and compatibility expectations.
2. **Inspect constraints** — Read `Cargo.toml`, workspace lints, `.catpaw/rules/system-constraints.md`, feature definitions, CI config.
3. **Map affected surface** — Trace definitions, callers, data flow, traits, impls, tests, re-exports, macros, features, IPC commands, frontend bindings.
4. **Establish baseline** — Run `cargo test -p <crate>` and `cargo clippy --all-targets -- -D warnings` before editing. Record pre-existing failures.
5. **Design coherent batches** — Group mutually dependent opportunities into complete end states. Order by dependency, risk, and verification cost.
6. **Implement end-to-end** — Update every required definition, caller, import, re-export, module declaration, test, example, doc, and config reference. Update `docs/api/` docs in the same change (Iron Rule 7). Run `./sync-to-github.sh` after source changes (Iron Rule 11).
7. **Verify the relevant matrix**:
   ```bash
   cargo fmt --all -- --check
   cargo test -p <target-crate>
   cargo clippy -p <target-crate> --all-targets -- -D warnings
   ```
8. **Audit the resulting diff** — Confirm objective is complete across all affected files. Confirm no generated output, lockfile, or unrelated formatting changed accidentally.

## Completion Report

```markdown
## Implemented Scope
[Objective and coherent batches completed]

## Files and Symbols
[Created, moved, renamed, consolidated, split, deleted, or materially changed]

## Behavior and API
[Preserved contracts and intentional corrections or migrations]

## Verification
- `cargo fmt --all -- --check` — passed
- `cargo test -p <target-crate>` — passed
- `cargo clippy -p <target-crate> --all-targets -- -D warnings` — passed

## Remaining Risk
[Unverified targets, pre-existing failures, and deferred opportunities]
```

## Key Files to Know

- `crates/core/src/lib.rs` — All port traits (Loader, Splitter, Embedder, Storage, LLMProvider, Retriever, Reranker, etc.)
- `crates/tauri-app/src/commands/` — 14 functional domain submodules (S65 SRP refactor)
- `crates/tauri-app/src/lib.rs` — `run()` with `generate_handler!` (150+ IPC commands)
- `crates/infra/src/sqlite_storage.rs` — SQLite with WAL, r2d2 pool, FTS5, SQLCipher
- `crates/models/src/lib.rs` — Domain model definitions
- `.catpaw/rules/system-constraints.md` — Iron rules (mandatory)
