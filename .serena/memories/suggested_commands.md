# Suggested Commands

## Development
```bash
# Terminal 1: backend
cd server
cp ../.env.example .env   # first run only
cargo run

# Terminal 2: frontend
cd web
npm install               # first run only
npm run dev
```

## Build & Test
```bash
# Frontend
cd web
npm run build

# Backend
cd server
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test --release
```

## Docker / Compose (local validation)
```bash
cd /home/telegram/Telegram-Drive
cp .env.example .env.coolify
# edit values in .env.coolify

docker compose --env-file .env.coolify -f deploy/docker/docker-compose.coolify.yml config
```

## Repository Hygiene
```bash
git status
git log --oneline -n 20
grep -RIn '@tauri-apps' web/src web/package.json web/package-lock.json
```

## Key Paths
- Frontend source: `web/src/`
- Backend source: `server/src/`
- Frontend config: `web/package.json`, `web/vite.config.ts`
- Backend config: `server/Cargo.toml`, `server/src/config.rs`
- CI workflow: `.github/workflows/ci.yml`
- Env contract: `.env.example`
