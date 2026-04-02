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
- Sessions use **HttpOnly, SameSite=Strict** cookies; `COOKIE_SECURE=true` is required in production
- Session state is stored using cookie-based sessions (`CookieSessionStore`), not in SQLite
- Session TTL is controlled via `SESSION_TTL_HOURS` (default: 8h)
- No JWT — stateful sessions are simpler and revocable
- Environment policy:
  - `APP_ENV=development` allows local defaults but logs warnings for weak settings
  - `APP_ENV=production` enforces fail-fast validation at startup for weak/unsafe values
  - `SESSION_SECRET` must be explicitly set in production

### Telegram auth
- Telegram API credentials (api_id, api_hash) are stored server-side in `.env`
- The Telegram session file is stored on disk, never sent to the browser
- During login setup, `api_hash` is kept in browser `sessionStorage` (tab-scoped), not long-lived local storage
- The Telegram auth flow (phone → code → 2FA) happens over the app's authenticated API
- Account tier (Free/Premium) is cached server-side after Telegram auth and refreshed on connection checks
- If tier detection is stale/unavailable and fallback is enabled, the server applies Free-tier limits conservatively

## Transport security

- All traffic should go through a reverse proxy with TLS (nginx/caddy)
- CORS is restricted to the frontend origin
- No sensitive data in URL query parameters
- In production, `CORS_ALLOWED_ORIGIN` must be an HTTPS public origin

## Data protection

- SQLite database file permissions: `0600`
- `DATA_DIR` should be on a dedicated volume
- Secrets (SESSION_SECRET, Telegram credentials) are in `.env`, never committed to git
- Cache files (previews, thumbnails) are stored with restricted permissions

## Rate limiting

- Login endpoint: limited to prevent brute force
- Telegram-proxied endpoints: respect FLOOD_WAIT errors from Telegram
- Upload/download: configurable concurrency limits
- Current limiter is in-memory and single-instance scoped (not distributed)
- `TRUST_PROXY_HEADERS` controls whether `X-Forwarded-For` is trusted for limiter key extraction

## Input validation

- All user input is validated at the HTTP layer before reaching services
- File uploads are checked against effective server-side limits (tier-aware when dynamic limits are enabled)
- Request payload size is capped by backend payload configuration and reverse proxy limits
- Upload mode `as_photo=true` can prioritize Telegram photo UX for images, which does not guarantee exact filename preservation
- Upload mode `as_photo=false` sends document/file media and preserves filename/extension metadata
- ZIP and archive uploads are treated as opaque files (no internal archive inspection)
- Path traversal is prevented in all file operations

## Deployment hardening

- systemd service runs as dedicated unprivileged user
- `NoNewPrivileges=true`, `ProtectSystem=strict`, `ProtectHome=true`
- Container should have no unnecessary capabilities
- Reverse proxy enforces upload size limits
- Production must set:
  - `APP_ENV=production`
  - `COOKIE_SECURE=true`
  - strong `SESSION_SECRET` and `ADMIN_PASSWORD`

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
