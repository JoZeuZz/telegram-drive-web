# Code Style & Conventions

## TypeScript/React (web)
- Functional components only (no class components)
- Custom hooks under `web/src/hooks/`
- Shared API client in `web/src/lib/api.ts` (typed `fetch` wrappers)
- React Query for server state and cache invalidation
- TailwindCSS utility classes for styling
- `sonner` for toast notifications
- `framer-motion` for animations
- `lucide-react` for icons
- camelCase for variables/functions, PascalCase for components/types
- TypeScript strict mode enabled

## Rust (server)
- Async/await with tokio runtime
- Actix-Web for HTTP API and middleware
- `Arc<Mutex<...>>` / `RwLock` for shared mutable state
- `serde` derives for DTO serialization
- `grammers` for Telegram MTProto operations
- Error mapping centralized through `AppError`
- Cookie-based session auth with CSRF header checks

## File Organization
- Frontend components: `web/src/components/` + `web/src/components/dashboard/`
- Frontend hooks: `web/src/hooks/`
- Frontend API/types: `web/src/lib/api.ts`, `web/src/types.ts`
- Backend routes: `server/src/http/routes/`
- Backend middleware: `server/src/http/middleware/`
- Backend services: `server/src/services/`
- Backend storage helpers: `server/src/storage/`
