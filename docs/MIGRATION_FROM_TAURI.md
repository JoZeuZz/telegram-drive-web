# Migration from Tauri — telegram-drive-web

This document tracks the migration from the original Tauri desktop app to the headless web server architecture.

## Status: Phase 0 — Foundation

Phase 0 establishes the monorepo structure without functional migration.

## Migration map

### Backend (Rust)

| Original file | New location | Status |
|---|---|---|
| `app/src-tauri/src/lib.rs` | `server/src/main.rs` + `server/src/http/mod.rs` | Skeleton created |
| `app/src-tauri/src/models.rs` | `server/src/domain/models.rs` | Migrated |
| `app/src-tauri/src/bandwidth.rs` | `server/src/services/bandwidth.rs` | Pending (Phase 1) |
| `app/src-tauri/src/server.rs` | `server/src/services/streaming.rs` + `server/src/http/routes/media.rs` | Pending (Phase 3) |
| `app/src-tauri/src/commands/auth.rs` | `server/src/services/telegram_auth.rs` + `server/src/http/routes/telegram_auth.rs` | Pending (Phase 1-2) |
| `app/src-tauri/src/commands/fs.rs` | `server/src/services/telegram_files.rs` + `server/src/http/routes/files.rs` | Pending (Phase 3) |
| `app/src-tauri/src/commands/preview.rs` | `server/src/services/previews.rs` + `server/src/http/routes/media.rs` | Pending (Phase 3) |
| `app/src-tauri/src/commands/network.rs` | `server/src/http/routes/health.rs` | Skeleton created |
| `app/src-tauri/src/commands/utils.rs` | `server/src/errors.rs` | Migrated (error mapping) |
| `app/src-tauri/Cargo.toml` | `server/Cargo.toml` | Created (no Tauri deps) |

### Frontend (React)

| Original file | New location | Status |
|---|---|---|
| `app/src/` | `web/src/` | Copied (still has Tauri imports) |
| `app/package.json` | `web/package.json` | Created (Tauri deps removed) |
| `app/vite.config.ts` | `web/vite.config.ts` | Updated (proxy to API server) |

### Tauri-specific dependencies to remove from frontend (Phase 4)

| Package | Replacement |
|---|---|
| `@tauri-apps/api` (invoke) | `fetch('/api/...')` |
| `@tauri-apps/plugin-store` | Server-side SQLite via API |
| `@tauri-apps/plugin-dialog` | `<input type="file">` / download links |
| `@tauri-apps/plugin-updater` | Remove entirely |
| `@tauri-apps/plugin-process` | Remove entirely |

## Phase completion checklist

- [x] Phase 0: Monorepo structure, build configs, docs
- [ ] Phase 1: Extract reusable Rust domain (no Tauri deps)
- [ ] Phase 2: HTTP API (auth, health, Telegram auth)
- [ ] Phase 3: File operations API
- [ ] Phase 4: Frontend migration (invoke → fetch)
- [ ] Phase 5: Hardening
- [ ] Phase 6: LXC deployment
- [ ] Phase 7: Open source release
