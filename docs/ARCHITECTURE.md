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
3. **SQLite for persistence** — App state, user sessions, and metadata cache live in a single SQLite database.
4. **Telegram session on disk** — `grammers` SQLite session file is stored in `DATA_DIR/`.
5. **Cookie-based auth** — HttpOnly, Secure, SameSite=Strict cookies for session management.
6. **Backend upload queue** — Uploads are queued and processed server-side, not in the browser.
7. **Streaming through the server** — Media is streamed from Telegram through the Actix server to the browser.

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
