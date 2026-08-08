# Contributing to EchoMind

## Development Setup

### Prerequisites

- **Rust** ≥ 1.85 (Edition 2024) — [install](https://rustup.rs/)
- **Node.js** ≥ 18 (for frontend build and E2E tests)

### Build

```bash
git clone https://github.com/lisering/EchoMind.git
cd EchoMind
cargo build
cargo tauri dev          # dev mode
cargo build --features pro  # Pro features
```

### Test

```bash
cargo test                              # all tests
cargo clippy --all-targets -- -D warnings  # lint (zero warnings)
cargo fmt --check                       # format check
cargo audit && cargo deny check         # supply chain
npx tsc --noEmit                        # frontend type check
```

## Code Standards

| Rule | Enforcement |
|------|-------------|
| No `unwrap()` / `expect()` / `panic!()` | Clippy deny |
| No `unsafe` | `forbid(unsafe_code)` |
| All `reqwest::Client::builder()` must use `.no_proxy()` | Code review |
| DB operations via `spawn_blocking` | Code review |
| Public types must have `///` doc comments | Code review |

## AI Code Review

Every PR is automatically reviewed by:
1. **CodeRabbit** — free for public repos, install at https://github.com/apps/coderabbitai
2. **Claude Code Action** — requires `ANTHROPIC_API_KEY` secret

Mention `@coderabbitai` or `@claude` in PR comments to interact.

## Pull Request Process

1. Fork → feature branch
2. Write tests first (TDD)
3. `cargo clippy -- -D warnings` + `cargo fmt --check` + `cargo test`
4. Commit with conventional commits (`feat:`, `fix:`, `refactor:`, etc.)
5. Open PR → wait for AI review → address issues → merge
