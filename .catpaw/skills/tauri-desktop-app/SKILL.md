---
name: tauri-desktop-app
description: Tauri v2 desktop app engineering for EchoMind. Use when working on IPC command security, capability-scoped permissions, code signing and notarization, auto-update pipelines, native OS integration (tray, global shortcuts, deep links), frontend build (esbuild IIFE bundling), or memory/binary footprint optimization. Covers the untrusted renderer boundary, typed IPC contract validation, and Tauri-specific security patterns.
---

# Tauri Desktop App Engineer

Adapted from [agency-agents](https://github.com/msitarzewski/agency-agents) Desktop App Engineer, tailored for EchoMind's Tauri v2 + Rust architecture.

## EchoMind Tauri Architecture

| Layer | Technology |
|---|---|
| Frontend | Vanilla JS ES modules (78 files) -> esbuild IIFE bundle -> single `ui/index.html` |
| Shell | Tauri v2 (`crates/tauri-app`) |
| Backend | Rust workspace (7 crates, hexagonal architecture) |
| IPC | 150+ commands via `generate_handler!` in `lib.rs::run()`, 16 pro-gated |
| State | `AppState` (storage, data_dir, llm_config, is_pro, lazy embedder, abort_tokens, llm_mode, model_manager, etc.) |

## Critical Rules

1. **The renderer is a browser tab with delusions.** Treat all webview content as untrusted. No `nodeIntegration` equivalent. Every IPC command validates inputs on the Rust side.
2. **IPC is a public API surface.** Every Tauri command validates its inputs, checks authorization (Pro gating), and exposes the narrowest verb possible. Example: `import_files(paths)` validates extensions + checks quota, never exposes generic filesystem.
3. **Never ship unsigned.** Signing infrastructure is release-blocking, built first.
4. **The updater is the most critical code.** Signed update manifests, staged rollouts (1% -> 10% -> 100%), health checks, tested rollback path.
5. **Remote content never gets privileges.** No remote URLs loaded into privileged windows.
6. **Respect each platform's conventions.** macOS menu bar, Windows/Linux tray differences, Cmd vs Ctrl shortcuts.
7. **Measure the footprint.** Cold start, idle memory, binary size — CI-enforced budgets. EchoMind has `crates/tauri-app/tests/bundle_size.rs`.
8. **Offline is first-class.** EchoMind works fully offline (local RAG, local LLM via mistral.rs). `REQ-NFR-021`.

## EchoMind IPC Security Patterns

### Command Validation Pattern

Every IPC command in `crates/tauri-app/src/commands/` follows:

```rust
#[tauri::command]
pub async fn import_files(
    paths: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DocStatusPayload>, String> {
    // 1. Validate extensions (ALLOWED_EXTENSIONS whitelist)
    // 2. Check Pro quota (free tier: 50 files max, PDF disabled)
    // 3. Check is_pro for gated formats
    // 4. Delegate to ImportService
    import_inner(&paths, &state).await
        .map_err(|e| e.to_string())
}
```

### Pro Feature Gating Pattern

```rust
#[tauri::command]
#[cfg(feature = "pro")]
pub async fn pro_only_command(...) -> Result<..., String> {
    // Pro commands are compile-time gated
}
```

Free tier runtime check: `state.is_pro` read from settings table.

### Abort/Cancel Pattern

```rust
// Each conversation gets a CancellationToken
state.abort_chat(conversation_id); // -> CancellationToken.cancel()
// forward_stream() loop breaks on select! biased branch
// Partial content is preserved and persisted
```

### API Key Masking

```rust
// get_settings returns api_key_masked (**** + last 4 chars), never plaintext
fn mask_api_key(key: &str) -> String { ... }
```

## Capability Configuration

`crates/tauri-app/capabilities/default.json` defines the frontend's permissions. Keep it minimal:

- Core: default permissions
- Dialog: save/open file dialogs
- FS: scoped to `$APPDATA/exports/*`
- Shell: only specific open-url patterns
- Notification: for background task completion

## Frontend Build Pipeline

```bash
# Development (external refs for browser dev)
node scripts/build-ui.mjs --dev

# Production (esbuild bundles IIFE + inlines CSS into single file)
node scripts/build-ui.mjs
# Output: ui/index.html (single file, works under file://)
```

**NEVER hand-edit `ui/index.html`** — it is a build artifact. Edit `ui/src/*.js` / `ui/styles/*.css` / `ui/locales/*.json` and rebuild.

### TypeScript Checking

```bash
npx tsc --noEmit  # tsconfig.json at root, checkJs: true
# types/*.d.ts: tauri.d.ts, vendor.d.ts, dom-ext.d.ts
```

### Mermaid Custom Build (C2a)

```bash
node scripts/build-mermaid.mjs
# esbuild bundles mermaid -> ui/vendor/mermaid-custom.min.js (IIFE + minify + tree-shaking)
```

## Security Layer

### SecurityManager State Machine

```
Unencrypted -> encrypt_database(password) -> EncryptedUnlocked
EncryptedUnlocked -> lock_app() / auto_lock_timeout -> Locked
Locked -> unlock(password) -> EncryptedUnlocked
```

- `crates/core/src/security.rs` — SecurityManager
- `crates/core/src/encryption.rs` — Argon2id KDF (m=19456KB, t=2, p=1) + PBKDF2 fallback + SecureString (zeroize)
- `crates/core/src/privacy.rs` — PII detector (8 types) + AuditLogger trait (hash-chain)
- `crates/infra/src/sqlite_storage.rs` — SQLCipher AES-256-CBC transparent encryption

### Brute-Force Protection

5 failed unlock attempts -> exponential backoff (30s -> 60s -> 120s -> ...).

### Clipboard Auto-Clear

Sensitive clipboard content auto-cleared after configurable timeout (default 30s).

## Footprint Budget

| Metric | Budget | Enforcement |
|---|---|---|
| Binary size | Track growth, no silent >5% increase per release | `bundle_size.rs` test |
| Cold start | < 2s on reference machine | Manual profiling |
| Idle memory | < 150MB (Tauri uses system webview) | Manual profiling |
| Background CPU | ~0% when idle | No timers keeping machine awake (except AutoDream idle tidying) |

## Tauri vs Electron (For Reference)

| Concern | EchoMind (Tauri) | Electron |
|---|---|---|
| Installer size | ~10-15MB | ~80-150MB |
| Idle memory | Low (shared webview) | Higher (bundled Chromium) |
| Privileged language | Rust (memory safety) | Node.js |
| Rendering consistency | Varies with OS webview | Identical everywhere |

## Key Files

- `crates/tauri-app/src/lib.rs` — Tauri command setup + `run()` (150+ IPC commands)
- `crates/tauri-app/src/state.rs` — AppState (all runtime state)
- `crates/tauri-app/src/commands/` — 14 functional domain submodules
- `crates/tauri-app/src/stores/` — Store pattern state modules
- `crates/tauri-app/capabilities/default.json` — Frontend permission scope
- `crates/tauri-app/tauri.conf.json` — Tauri configuration
- `scripts/build-ui.mjs` — Frontend esbuild bundling script
- `crates/tauri-app/tests/bundle_size.rs` — Binary size CI enforcement
