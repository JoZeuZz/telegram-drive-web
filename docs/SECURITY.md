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
- Sessions use **HttpOnly, SameSite=Strict** cookies, with `COOKIE_SECURE=true` recommended behind TLS
- Session state is stored using cookie-based sessions (`CookieSessionStore`), not in SQLite
- Session TTL is controlled via `SESSION_TTL_HOURS` (default: 8h)
- No JWT — stateful sessions are simpler and revocable

### Telegram auth
- Telegram API credentials (api_id, api_hash) are stored server-side in `.env`
- The Telegram session file is stored on disk, never sent to the browser
- During login setup, `api_hash` is kept in browser `sessionStorage` (tab-scoped), not long-lived local storage
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

## Secret hygiene and documentation policy

- Never commit real values for `SESSION_SECRET`, `ADMIN_PASSWORD`, `TELEGRAM_API_HASH`, tunnel credentials, or API keys.
- Never commit private infrastructure details (private IPs, internal hostnames, VPN topology, node inventory).
- Use placeholders in docs and examples (`example.com`, `<your-secret>`, `***REDACTED***`).
- Store runtime secrets only in deployment secret managers (Coolify environment UI, Cloudflare Zero Trust, system secret stores).
- If any secret is exposed in git history, rotate it immediately and replace all affected credentials.

## Split topology notes (Coolify + Cloudflared)

- Keep Cloudflared as edge ingress and Coolify/Traefik as application ingress.
- Preserve the public `Host` header from Cloudflared to Coolify so host-based routing resolves the correct app.
- Keep `CORS_ALLOWED_ORIGIN` equal to the final public URL served to users.
