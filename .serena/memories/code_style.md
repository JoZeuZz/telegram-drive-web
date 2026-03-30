# Code Style & Conventions

## TypeScript/React
- Functional components only (no class components)
- Custom hooks pattern: `use[Feature].ts` in `hooks/` directory
- Context pattern: separate `context/` and `contexts/` folders for providers
- TailwindCSS utility classes for styling (no CSS modules)
- `@tanstack/react-query` for server state (files fetching)
- `@tauri-apps/plugin-store` for persistent local state (JSON files)
- `sonner` for toast notifications
- `framer-motion` for animations
- `lucide-react` for icons
- camelCase for functions/variables, PascalCase for components/interfaces
- `invoke()` from `@tauri-apps/api/core` for all Tauri command calls
- TypeScript strict mode enabled
- ESNext module system

## Rust
- Async/await with tokio runtime
- `Arc<Mutex<>>` for shared state 
- `#[tauri::command]` for IPC handlers
- `serde` derive for serialization
- `grammers` library for Telegram MTProto
- Error handling: `map_error()` utility that handles FLOOD_WAIT specially
- Mock mode: many commands return mock data when client is None
- Actix-Web for HTTP streaming server (separate thread with own runtime)
- Session stored as SQLite via `grammers_session::SqliteSession`
- Token-based auth for streaming server (random hex token per session)

## File Organization
- Components: `src/components/` (top-level) and `src/components/dashboard/` (dashboard sub-components)
- Hooks: `src/hooks/`
- Types: `src/types.ts` (single file)
- Utils: `src/utils.ts` (single file)
- Rust commands: `src-tauri/src/commands/` (split by domain: auth, fs, preview, network, streaming, utils)
