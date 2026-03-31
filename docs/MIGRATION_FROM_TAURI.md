# Migration from Tauri — telegram-drive-web

This document tracks the migration from the original Tauri desktop app to the current headless web architecture.

## Current status

The functional migration is complete: runtime behavior is provided by `server/` (Rust + Actix-Web) and `web/` (React + Vite), with deployment assets under `deploy/`.

Remaining work is operational hardening and release/process improvements, not Tauri-to-web functional parity.

## Migration map

### Backend (Rust)

| Original file | New location | Status |
|---|---|---|
| `app/src-tauri/src/lib.rs` | `server/src/main.rs` + `server/src/http/mod.rs` | Migrated |
| `app/src-tauri/src/models.rs` | `server/src/domain/models.rs` | Migrated |
| `app/src-tauri/src/bandwidth.rs` | `server/src/services/bandwidth.rs` | Migrated |
| `app/src-tauri/src/server.rs` | `server/src/services/streaming.rs` + `server/src/http/routes/media.rs` | Migrated |
| `app/src-tauri/src/commands/auth.rs` | `server/src/services/telegram_auth.rs` + `server/src/http/routes/telegram_auth.rs` | Migrated |
| `app/src-tauri/src/commands/fs.rs` | `server/src/services/telegram_files.rs` + `server/src/http/routes/files.rs` | Migrated |
| `app/src-tauri/src/commands/preview.rs` | `server/src/services/previews.rs` + `server/src/http/routes/media.rs` | Migrated |
| `app/src-tauri/src/commands/network.rs` | `server/src/http/routes/health.rs` | Migrated |
| `app/src-tauri/src/commands/utils.rs` | `server/src/errors.rs` + `server/src/services/helpers.rs` | Migrated |
| `app/src-tauri/Cargo.toml` | `server/Cargo.toml` | Migrated (Tauri removed) |

### Frontend (React)

| Original file | New location | Status |
|---|---|---|
| `app/src/` | `web/src/` | Migrated |
| `app/package.json` | `web/package.json` | Migrated |
| `app/vite.config.ts` | `web/vite.config.ts` | Migrated |

## Validation checklist

- [x] Frontend uses `fetch('/api/...')` instead of Tauri IPC
- [x] Backend API served by Actix-Web
- [x] CI validates `server` and `web`
- [x] Legacy Tauri publish workflows removed

## Next milestones (post-migration)

- [ ] Additional security hardening and observability
- [ ] Deployment UX refinements (LXC/Coolify)
- [ ] Optional release automation strategy for web/server artifacts
