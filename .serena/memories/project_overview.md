# Telegram Drive Web - Project Overview

## Purpose
Self-hosted web application that uses a Telegram account as personal cloud storage.
Files are stored in Saved Messages and private Telegram channels (folders), managed via browser UI.

## Tech Stack
- **Frontend**: React 19, TypeScript 5.8, Vite 7, TailwindCSS 4, React Query 5
- **Backend**: Rust 2021, Actix-Web 4, grammers (Telegram MTProto)
- **Storage**: SQLite session (grammers), app admin metadata in `data/admin.json`, filesystem cache in `CACHE_DIR`
- **Deploy**: Docker (Coolify), systemd/LXC, nginx/caddy, optional Cloudflared edge

## Repository Layout
- `web/` — React SPA and API client (`fetch('/api/...')`)
- `server/` — Rust API server, Telegram integration, middleware, background jobs
- `deploy/` — docker/nginx/caddy/systemd/cloudflared templates
- `docs/` — architecture, API, security, deployment guides

## Backend Architecture (`server/src`)
- `main.rs` — runtime bootstrap, config loading, session middleware, route wiring
- `config.rs` — env contract, environment policy (`APP_ENV`), runtime security validation
- `http/routes/` — REST endpoints (`health`, `app_auth`, `telegram_auth`, `files`, `folders`, `media`, `search`, `uploads`, `admin`, `metrics`)
- `http/middleware/` — auth, csrf, audit, request-id, logging, rate limiting
- `services/` — Telegram business logic, streaming, previews, upload queue, bandwidth
- `storage/` — persistent files/cache helpers and Telegram session helpers
- `jobs/` — reconnect and cleanup background tasks

## Frontend Architecture (`web/src`)
- `lib/api.ts` — typed HTTP client with cookie auth + CSRF header
- `App.tsx` — auth gate and app shell
- `components/` + `components/dashboard/` — auth flow and file explorer UI
- `hooks/` — connection, file operations, uploads/downloads, keyboard/network behavior

## Runtime Model
- Cookie-based app auth (`td_session`) + CSRF header for mutating requests.
- Telegram auth and file operations proxied by backend.
- Media streaming and previews served by backend API.
- In-memory rate limiting per IP; trust of proxy headers controlled by `TRUST_PROXY_HEADERS`.
