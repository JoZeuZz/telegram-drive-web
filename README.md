# telegram-drive-web

A headless web server that turns your Telegram account into a personal cloud drive. Access your files through a browser, manage folders backed by Telegram channels, stream media, and upload/download — all self-hosted behind your VPN.

> Fork of [Telegram-Drive](https://github.com/caamer20/Telegram-Drive) by Cameron Amer. Converted from Tauri desktop app to a deployable web server.

## Features

- **Telegram as storage** — files are stored as messages in Saved Messages and private channels
- **Web interface** — React SPA accessible from any browser
- **Media streaming** — stream video and audio directly without downloading
- **Folder management** — create, sync, and organize folders (Telegram channels)
- **Search** — global search across all folders
- **Self-hosted** — runs on your own server, no third-party services
- **VPN-only access** — designed for homelab behind WireGuard/Tailscale

## Architecture

```
server/     Rust backend — Actix-Web API + Telegram integration
web/        React frontend — Vite SPA
deploy/     systemd, nginx, caddy, docker configs
docs/       Architecture, security, API, deployment guides
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full design.

## Quick start (development)

### Prerequisites
- Rust (stable)
- Node.js 18+
- Telegram API credentials from [my.telegram.org](https://my.telegram.org)

### Backend
```bash
cd server
cp .env.example .env
# Edit .env with your settings
cargo run
```

### Frontend
```bash
cd web
npm install
npm run dev
```

The frontend dev server runs on `http://localhost:3000` and proxies API calls to `http://localhost:8080`.

## Deployment

See [docs/DEPLOYMENT_LXC.md](docs/DEPLOYMENT_LXC.md) for production deployment on Proxmox LXC.

## Security

This app is designed for **single-user homelab deployment behind a VPN**. See [docs/SECURITY.md](docs/SECURITY.md).

## Migration status

This project is being incrementally migrated from the Tauri desktop app. See [docs/MIGRATION_FROM_TAURI.md](docs/MIGRATION_FROM_TAURI.md) for current status.

## License

MIT — see [LICENSE](LICENSE) and [NOTICE](NOTICE) for attribution.
