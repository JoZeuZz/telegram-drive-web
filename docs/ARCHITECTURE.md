# Architecture — telegram-drive-web

## Overview

telegram-drive-web is a headless web application that uses Telegram as a cloud storage backend. It replaces the original Tauri desktop app with a server-side Rust process and a browser-based React frontend.

## High-level diagram

```
┌──────────────┐       HTTP        ┌──────────────────┐       MTProto       ┌───────────┐
│  Browser     │ ◄───────────────► │  Actix-Web       │ ◄────────────────► │  Telegram  │
│  (React SPA) │   /api/*          │  Server (Rust)   │   grammers          │  Servers   │
└──────────────┘                   └──────────────────┘                     └───────────┘
                                          │
                                          ▼
                                   ┌──────────────┐
                                   │  SQLite       │
                                   │  + File Cache │
                                   └──────────────┘
```

## Components

### server/ — Rust backend

| Layer | Responsibility |
|-------|---------------|
| `http/routes/` | HTTP request handlers, input validation, response formatting |
| `http/middleware/` | Auth guard, rate limiting, request ID, logging |
| `services/` | Business logic: Telegram auth, file operations, streaming, bandwidth |
| `domain/` | Data models and DTOs — no I/O, no framework dependencies |
| `storage/` | SQLite persistence, Telegram session management, file cache |
| `jobs/` | Background tasks: cache cleanup, Telegram reconnection |

### web/ — React frontend

| Layer | Responsibility |
|-------|---------------|
| `lib/` | API client (`fetch`-based), auth helpers |
| `features/` | Feature modules: auth, files, folders, media, settings |
| `components/` | Shared UI components |

### deploy/

Infrastructure configs for systemd, nginx, caddy, and docker.

## Key design decisions

1. **Single server process** — Actix-Web serves both the API and (in production) the static frontend files.
2. **No Tauri** — All desktop dependencies are removed. Communication is over HTTP, not IPC.
3. **SQLite for persistence** — Telegram session storage uses SQLite (`grammers`), while app auth session state is cookie-based.
4. **Telegram session on disk** — `grammers` SQLite session file is stored in `DATA_DIR/`.
5. **Cookie-based auth** — HttpOnly, Secure, SameSite=Strict cookies for session management.
6. **Backend upload queue** — Uploads are queued and processed server-side, not in the browser.
7. **Streaming through the server** — Media is streamed from Telegram through the Actix server to the browser.
8. **Virtual folder hierarchy** — Subfolders are modeled as independent Telegram channels with parent metadata managed by the app (`parent_id`), instead of relying on Telegram-native nested folders.
9. **Unified media cache** — Previews and thumbnails are stored under the same cache root (`CACHE_DIR/media`) to simplify cleanup and avoid key collisions.

## Data flow examples

### File upload
```
Browser → POST /api/files/upload (multipart) → Server saves to temp
→ Server queues upload → grammers uploads to Telegram
→ Server returns file metadata
```

### Media streaming
```
Browser → GET /api/media/stream/:folderId/:messageId
→ Server authenticates request
→ Server streams chunks from Telegram via grammers
→ Chunks forwarded to browser as HTTP streaming response
```

## Security model

See [SECURITY.md](SECURITY.md) for the full security design.

## Deployment topologies

### Standard (single platform)

- Reverse proxy and app stack run in the same platform (LXC or Coolify-managed Docker).
- Public domain terminates TLS at the platform edge.

### Split edge topology (Cloudflared + Coolify)

```
┌─────────────┐      HTTPS      ┌──────────────────┐      Tunnel      ┌─────────────────────┐
│   Browser   │ ─────────────► │ Cloudflare Edge  │ ───────────────► │ Cloudflared Agent   │
└─────────────┘                 └──────────────────┘                  └─────────┬───────────┘
                                                                                │
                                                                                │ HTTP/HTTPS (private network)
                                                                                ▼
                                                                      ┌─────────────────────┐
                                                                      │ Coolify / Traefik   │
                                                                      │ Host-based routing  │
                                                                      └─────────┬───────────┘
                                                                                ▼
                                                                      ┌─────────────────────┐
                                                                      │ telegram-drive-web  │
                                                                      │ web + server        │
                                                                      └─────────────────────┘
```

Key rule: Cloudflared must forward the public hostname to Coolify (host header), and backend CORS must match that same public URL.
