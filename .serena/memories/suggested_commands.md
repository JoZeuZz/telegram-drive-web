# Suggested Commands

## Development
```bash
cd app
npm install              # Install frontend dependencies
npm run tauri dev        # Run in development mode (starts both Vite + Tauri)
npm run tauri build      # Build production app
npm run dev              # Start only the Vite dev server (for frontend-only)
npm run build            # Build only the frontend (tsc + vite build)
```

## Rust Backend
```bash
cd app/src-tauri
cargo build              # Build Rust backend only
cargo check              # Type-check Rust code
```

## System Utilities
```bash
git status / git log --oneline   # Git management
ls, cd, grep, find              # Standard Linux tools
```

## Project Paths
- Frontend source: `app/src/`
- Backend source: `app/src-tauri/src/`
- Config: `app/package.json`, `app/src-tauri/Cargo.toml`, `app/src-tauri/tauri.conf.json`
- Vite config: `app/vite.config.ts`
