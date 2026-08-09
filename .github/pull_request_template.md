## Description

<!-- Brief description of what this PR does -->

## Type of Change

- [ ] feat: New feature
- [ ] fix: Bug fix
- [ ] refactor: Code refactoring
- [ ] perf: Performance improvement
- [ ] docs: Documentation
- [ ] test: Tests
- [ ] chore: Build/CI/tooling

## Checklist

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes (zero warnings)
- [ ] `cargo test` passes (Free)
- [ ] `cargo test --features pro` passes (Pro)
- [ ] `cargo audit` + `cargo deny check` pass
- [ ] No `unwrap()` / `expect()` / `panic!()` in production code
- [ ] No `unsafe` code (`forbid(unsafe_code)`)
- [ ] Public types have `///` doc comments
- [ ] New features have tests (TDD)

## Test Summary

<!-- How did you test this? What tests were added/modified? -->
