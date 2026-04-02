# Changelog

## [Unreleased]

### Security

- Backend session cookie policy hardened to `SameSite=Strict` with configurable `COOKIE_SECURE` for TLS deployments.
- Added configurable session TTL via `SESSION_TTL_HOURS` (default 8h) and propagated it to cookie max-age.
- Replaced custom session key derivation with SHA-512-based derivation from `SESSION_SECRET`.
- Frontend now keeps `api_hash` in `sessionStorage` (tab-scoped) instead of `localStorage`.

### Tests

- Extended backend integration tests to assert session cookie security attributes (`HttpOnly`, `SameSite=Strict`).
- Added backend integration coverage for `/api/account-info` and dynamic limit fields in `/api/metrics`.

### Docs

- Updated session security docs to reflect cookie-session storage behavior and TTL configuration.
- Updated deployment docs to use the single root `.env.example` contract across local, Coolify, and LXC workflows.
- Added split deployment documentation for external Cloudflared tunnel in front of Coolify, using generic placeholders and no infra-specific data.
- Documented tier-aware limit responses (`/api/bandwidth`, `/api/metrics`) and the new `/api/account-info` endpoint.
- Documented new feature-flagged forums/community endpoints (`/api/forums` and `/api/forums/{forum_id}/topics`).
- Documented topic-aware file move payload for `/api/files/move` including structured subfolder destinations.

### Deployment

- Fixed Docker runtime env names to match backend config contract (`HOST`, `PORT`, `DATA_DIR`, `CACHE_DIR`).
- Fixed Docker build context path mismatch in compose.
- Added deploy-ready Coolify stack assets (`docker-compose.coolify.yml`, `Dockerfile.web`, nginx reverse-proxy config).
- Added configurable `CORS_ALLOWED_ORIGIN` for production domains.
- Added backend container entrypoint to create/chown mounted data dirs before dropping privileges, preventing first-boot volume permission failures.
- Added Cloudflared deployment templates for split edge routing (Cloudflare tunnel to Coolify proxy).
- Consolidated environment examples into a single root `.env.example` as the canonical cross-deployment contract.
- Removed duplicate env templates under `server/` and `deploy/` to keep one canonical env contract for local, Coolify, and LXC workflows.

### Changed

- Unified previews and thumbnails under `CACHE_DIR/media` with folder-aware cache keys to prevent cross-folder collisions and simplify cleanup behavior.
- Added centralized TanStack Query client defaults (`staleTime`, `gcTime`, retry policy, no window-focus refetch by default).
- Made bandwidth polling visibility-aware to avoid background refetch traffic.
- Switched upload queue item IDs to `crypto.randomUUID()` with timestamp fallback.
- Added Vite `manualChunks` split for React/Query/Motion vendors to reduce main bundle size.
- Improved multi-selection UX with Shift+click and Shift+Arrow range extension based on visible ordering.
- Added progressive delete feedback (in-progress state, counters, action disabling, and toasts) for single and bulk deletes.
- Added hierarchical folder UX and API behavior for `parent_id`, subtree-aware sync state, and cascade delete reporting (`deleted_count`).
- Folder hierarchy sync now resolves parent metadata primarily from channel title (`[TD|s=1|p=...]`), with legacy `about` fallback using bounded retry/backoff and FLOOD_WAIT early cutoff.
- Added lazy migration for legacy folder titles during sync and exposed sync integrity summary in `/api/folders/sync` (`resolved_by_title`, `resolved_by_about`, `orphans`, `migrated`) for frontend visibility.
- Added dynamic account-tier limit foundations: cached Telegram profile (with premium flag), effective limit computation, and conservative fallback to Free tier when account detection is stale/unavailable.
- Updated upload/download and preview bandwidth checks to use effective per-tier limits.
- Added protected `/api/account-info` endpoint and enriched `/api/bandwidth` and `/api/metrics` with tier/limit context.
- Added account tier badge in Dashboard sidebar (Free/Premium) with explicit fallback-mode warning.
- Added initial hybrid Communities/Topics backend block with feature flag (`FORUMS_ENABLED`) and protected APIs to list/create forums and list/create forum topics, while preserving legacy `/api/folders` flow.
- Added drag-and-drop move parity across legacy folders and structured subfolders, with backend topic-aware forwarding (`top_msg_id`) and source/target topic context in move payloads.

## [1.0.4] - 2026-02-13

### Fixes

- Finally squashed the grid overlap bug for real. Cards were using CSS `aspect-[4/3]` to size themselves, but the virtualizer was computing row heights separately — at certain window widths these disagreed and rows would bleed into each other. Now both use the same explicit pixel height, so no more overlap regardless of how you resize the window.

### Cleanup

- Went through the whole codebase and ripped out every `console.log` / `console.error` we'd left in from debugging (16 of them). The one in `ErrorBoundary` stays since that's the whole point of an error boundary.
- Got rid of all `as any` casts on the frontend — everything's properly typed now.
- Ran Clippy and fixed all 7 warnings, including a couple of `collapsible_match` ones in `fs.rs` that needed manual refactoring.
- Dropped `clsx`, `tailwind-merge`, and `@tauri-apps/plugin-opener` from `package.json` — none of them were actually imported anywhere.
- General comment cleanup throughout.

---

## [1.0.3] - 2026-02-09

### Bug Fixes

- **Grid Spacing Fix** - Fixed cards overlapping in grid view
- **Dynamic Row Height** - Grid now properly calculates row height based on window size
- **Virtualizer Re-measurement** - Grid correctly updates when resizing window

---

## [1.0.2] - 2026-02-07

### Automated Release Pipeline

- **GitHub Actions Workflow** - Automatic builds triggered on version tags
- **Cross-Platform Builds** - Windows, Linux, macOS (Intel + ARM) built in parallel
- **Signed Updates** - All builds signed with Ed25519 for secure auto-updates
- **Automatic Publishing** - Releases published to GitHub automatically

---

## [1.0.1] - 2026-02-07

### Auto-Update System

- **Automatic Update Checks** - App checks for updates 5 seconds after startup
- **Update Banner** - Beautiful animated banner when new version available
- **One-Click Updates** - Download and install updates with progress indicator
- **Cross-Platform** - Windows, Mac, and Linux users get platform-specific updates

### 🔧 Technical

- Added Tauri updater plugin with Ed25519 signing
- Created `useUpdateCheck` hook for update lifecycle management
- Added `UpdateBanner` component with download progress

---

## [1.0.0] - 2026-02-06 🎉

### First Stable Release

Telegram Drive is now production-ready! This release focuses on performance, reliability, and user experience polish.

### ✨ New Features

- **Virtual Scrolling** - Smooth performance with folders containing 1000+ files
- **Inline Thumbnails** - Image files now display thumbnails directly in the file grid
- **Thumbnail Caching** - Thumbnails are cached locally for instant loading on revisit
- **API Setup Help Guide** - Step-by-step modal explaining how to get Telegram API credentials

### 🚀 Performance Improvements

- Grid and list views now only render visible items (virtualized)
- Responsive column layout adapts to window width
- Lazy loading of thumbnails to reduce initial load time

### 🎨 UI/UX Improvements

- Refined grid spacing (6px gaps between cards)
- Gradient overlay on thumbnail cards for text readability
- Improved light mode support across all components

### 🔧 Technical

- Added `@tanstack/react-virtual` for virtualization
- Separate thumbnail cache directory (`app_data_dir/thumbnails/`)
- FileTypeIcon now supports multiple sizes

---

## [0.6.0] - 2026-02-05

### Reliability Update

- Session persistence (window state, UI state, active folder)
- Network resilience with connection status indicator
- Queue persistence for uploads/downloads
- Light mode UI fixes

---

## [0.5.0] - 2026-02-04

### Drag & Drop Update

- Stable hybrid drag-drop system
- External drop blocker
- GitHub Actions workflow fixes

---

## [0.4.0] - 2026-02-01

### Media & Performance

- Audio/Video streaming player
- Global search filter
- Internal drag & drop between folders
