# Deployment on Proxmox LXC — telegram-drive-web

Production deployment guide for running telegram-drive-web in an **unprivileged LXC container** on Proxmox VE, accessed through a VPN or reverse proxy.

If you run Cloudflared in a separate node and route traffic through Coolify/Traefik, use the split deployment guide: [DEPLOYMENT_SPLIT_CLOUDFLARED.md](DEPLOYMENT_SPLIT_CLOUDFLARED.md).

## Prerequisites

| Requirement | Minimum |
|---|---|
| Proxmox VE | 7.x or 8.x |
| CT Template | Debian 12 (Bookworm) or Ubuntu 24.04 |
| vCPU | 1 |
| RAM | 512 MB (1 GB recommended) |
| Disk | 4 GB root + data volume |
| Network | VPN (WireGuard/Tailscale) or internal VLAN |
| Telegram | API ID & Hash from <https://my.telegram.org> |

## 1. Create the LXC container

From the Proxmox shell or UI:

```bash
# Download template (if not already present)
pveam download local debian-12-standard_12.7-1_amd64.tar.zst

# Create unprivileged container
pct create 200 local:vztmpl/debian-12-standard_12.7-1_amd64.tar.zst \
  --hostname telegram-drive \
  --memory 1024 \
  --cores 1 \
  --rootfs local-lvm:4 \
  --net0 name=eth0,bridge=vmbr0,ip=dhcp \
  --unprivileged 1 \
  --features nesting=1 \
  --start 1

# Attach
pct enter 200
```

> **Nesting** is required for systemd inside the container.

## 2. Install system dependencies

```bash
apt-get update && apt-get upgrade -y
apt-get install -y \
  curl wget git build-essential pkg-config libssl-dev \
  nginx        # or caddy, see below

# Install Node.js 20 LTS
curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
apt-get install -y nodejs

# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

## 3. Create service user

```bash
useradd --system --create-home --home-dir /opt/telegram-drive-web \
  --shell /usr/sbin/nologin telegram-drive
```

## 4. Build from source

```bash
cd /tmp
git clone https://github.com/your-org/Telegram-Drive.git
cd Telegram-Drive

# Build server (release mode)
cd server
cargo build --release
cp target/release/telegram-drive-server /opt/telegram-drive-web/
cd ..

# Build frontend
cd web
npm ci
npm run build
mkdir -p /opt/telegram-drive-web/web
cp -r dist /opt/telegram-drive-web/web/
cd ..
```

## 5. Configure environment

```bash
cp .env.example /opt/telegram-drive-web/.env
chmod 600 /opt/telegram-drive-web/.env
```

Edit `/opt/telegram-drive-web/.env`:

```dotenv
HOST=127.0.0.1
PORT=8080
FRONTEND_PORT=80
CORS_ALLOWED_ORIGIN=https://telegram-drive.example.com
DATA_DIR=/opt/telegram-drive-web/data
CACHE_DIR=/opt/telegram-drive-web/data/cache
RUST_LOG=info
LOG_FORMAT=json
COOKIE_SECURE=true
SESSION_TTL_HOURS=8

# REQUIRED — generate with: openssl rand -hex 32
SESSION_SECRET=<your-secret>

# REQUIRED — pick a strong password
ADMIN_PASSWORD=<your-password>

# REQUIRED — from https://my.telegram.org
TELEGRAM_API_ID=<your-api-id>
TELEGRAM_API_HASH=<your-api-hash>
```

Set ownership:

```bash
mkdir -p /opt/telegram-drive-web/data/cache
chown -R telegram-drive:telegram-drive /opt/telegram-drive-web
```

## 6. Install systemd service

```bash
cp deploy/systemd/telegram-drive-web.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now telegram-drive-web

# Check status
systemctl status telegram-drive-web
journalctl -u telegram-drive -f
```

## 7. Configure reverse proxy

### Option A: Nginx

```bash
cp deploy/nginx/telegram-drive-web.conf /etc/nginx/sites-available/telegram-drive
ln -s /etc/nginx/sites-available/telegram-drive /etc/nginx/sites-enabled/
rm -f /etc/nginx/sites-enabled/default

# Edit server_name to match your domain / LAN hostname
nano /etc/nginx/sites-available/telegram-drive

nginx -t && systemctl reload nginx
```

### Option B: Caddy

```bash
apt-get install -y caddy
cp deploy/caddy/Caddyfile /etc/caddy/Caddyfile

# Edit the domain
nano /etc/caddy/Caddyfile

systemctl reload caddy
```

## 8. Verify

```bash
# Health check
curl -s http://127.0.0.1:8080/api/health | python3 -m json.tool

# From outside the container (replace IP)
curl -s http://<container-ip>/api/version
```

You should see:

```json
{
  "status": "ok",
  "telegram_connected": false,
  "uptime_secs": 5,
  "cache_bytes": 0
}
```

Open `http://<container-ip>` in a browser to access the web UI. Log in with your `ADMIN_PASSWORD`, then connect your Telegram account.

## Volume layout

```
/opt/telegram-drive-web/
├── telegram-drive-server          # Server binary
├── .env                           # Configuration (mode 600)
├── web/dist/                      # Frontend static files
└── data/                          # Persistent data
    ├── admin.json                 # Hashed admin password
    ├── telegram.session           # Telegram session (SQLite)
    ├── cache/                     # Preview/thumbnail cache (auto-cleaned)
    └── bandwidth.json             # Bandwidth stats
```

## Updating

```bash
cd /tmp/Telegram-Drive && git pull

# Rebuild server
cd server && cargo build --release
systemctl stop telegram-drive-web
cp target/release/telegram-drive-server /opt/telegram-drive-web/
cd ..

# Rebuild frontend
cd web && npm ci && npm run build
cp -r dist /opt/telegram-drive-web/web/
cd ..

chown -R telegram-drive:telegram-drive /opt/telegram-drive-web
systemctl start telegram-drive-web
```

## Monitoring

```bash
# Live logs (structured JSON)
journalctl -u telegram-drive -f -o cat

# Health with jq
watch -n 30 'curl -s http://127.0.0.1:8080/api/health | jq .'

# Disk usage
du -sh /opt/telegram-drive-web/data/cache
```

## Troubleshooting

| Symptom | Fix |
|---|---|
| `ENOSPC` or disk full | Cache cleanup runs every 30 min, but you can reduce `MAX_CACHE_BYTES` or manually `rm -rf data/cache/*` |
| Telegram disconnects | The reconnect job retries every 60 s. Check `journalctl -u telegram-drive --since "5 min ago"` |
| 429 Too Many Requests | Rate limiter is active. Wait 60 seconds or check the login rate limit config |
| Can't bind port 8080 | Another process may be using it. Use `ss -tlnp \| grep 8080` to check |
| Session cookie not set | Ensure your reverse proxy forwards `Cookie` and `Set-Cookie` headers correctly |
