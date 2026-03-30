# Security Design — telegram-drive-web

## Threat model

This application is designed for **single-user, homelab deployment behind a VPN**. It is NOT intended to be exposed directly to the public internet.

### Assumptions
- Network access is restricted by VPN (e.g., WireGuard, Tailscale)
- The server runs in an unprivileged LXC container or similar isolated environment
- Only one Telegram account is active at a time (v1)

## Authentication

### App-level auth
- The app has its own login system independent of Telegram
- On first run, a bootstrap flow creates the admin user
- Passwords are hashed with **argon2** (not bcrypt, not plain SHA)
- Sessions use **HttpOnly, Secure, SameSite=Strict** cookies
- Session tokens are stored server-side in SQLite
- No JWT — stateful sessions are simpler and revocable

### Telegram auth
- Telegram API credentials (api_id, api_hash) are stored server-side in `.env`
- The Telegram session file is stored on disk, never sent to the browser
- The Telegram auth flow (phone → code → 2FA) happens over the app's authenticated API

## Transport security

- All traffic should go through a reverse proxy with TLS (nginx/caddy)
- CORS is restricted to the frontend origin
- No sensitive data in URL query parameters

## Data protection

- SQLite database file permissions: `0600`
- `DATA_DIR` should be on a dedicated volume
- Secrets (SESSION_SECRET, Telegram credentials) are in `.env`, never committed to git
- Cache files (previews, thumbnails) are stored with restricted permissions

## Rate limiting

- Login endpoint: limited to prevent brute force
- Telegram-proxied endpoints: respect FLOOD_WAIT errors from Telegram
- Upload/download: configurable concurrency limits

## Input validation

- All user input is validated at the HTTP layer before reaching services
- File uploads are checked for size limits
- Path traversal is prevented in all file operations

## Deployment hardening

- systemd service runs as dedicated unprivileged user
- `NoNewPrivileges=true`, `ProtectSystem=strict`, `ProtectHome=true`
- Container should have no unnecessary capabilities
- Reverse proxy enforces upload size limits
