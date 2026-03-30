# Telegram Drive - Project Overview

## Purpose
Desktop app (Tauri + React) that turns a Telegram account into unlimited cloud storage.
Uses "Saved Messages" and created Channels as folders, providing a file explorer UI.

## Tech Stack
- **Frontend**: React 19, TypeScript 5.8, TailwindCSS 4, Framer Motion, Vite 7
- **Backend**: Rust (Tauri 2), Grammers (Telegram MTProto client)
- **Streaming Server**: Actix-Web 4 (localhost:14200) for media streaming
- **State**: @tanstack/react-query, @tauri-apps/plugin-store (local JSON)
- **UI Components**: lucide-react icons, sonner toasts, Framer Motion animations

## Architecture

### Backend (Rust - src-tauri/)
- `lib.rs` - Entry point: initializes Tauri plugins, TelegramState, BandwidthManager, starts streaming server on port 14200
- `models.rs` - Data models: AuthState, AuthResult, FileMetadata, FolderMetadata, Drive
- `server.rs` - Actix-Web streaming server: `/stream/{folder_id}/{message_id}?token=X`
- `bandwidth.rs` - BandwidthManager: daily 250GB limit, tracks up/down, persists to bandwidth.json
- `commands/mod.rs` - TelegramState struct (client, login_token, password_token, api_id, runner_shutdown)
- `commands/auth.rs` - Auth commands: connect, check_connection, logout, request_code, sign_in, check_password
- `commands/fs.rs` - File system: create_folder, delete_folder, upload_file, delete_file, download_file, move_files, get_files, search_global, scan_folders
- `commands/preview.rs` - Preview/thumbnail generation with LRU cache (30 files, 80MB max)
- `commands/streaming.rs` - StreamToken management
- `commands/network.rs` - Network availability check (TCP to Telegram DC)
- `commands/utils.rs` - resolve_peer helper, logging, bandwidth getter, FLOOD_WAIT error mapping

### Frontend (React/TS - src/)
- `App.tsx` - Root: QueryClient, ThemeProvider, ConfirmProvider, auth routing
- `types.ts` - Interfaces: TelegramFile, TelegramFolder, QueueItem, DownloadItem, BandwidthStats
- `utils.ts` - formatBytes helper

### Components
- `AuthWizard.tsx` - Multi-step login (API setup → phone → code → password → dashboard)
- `Dashboard.tsx` - Main dashboard orchestrating all sub-components
- `dashboard/Sidebar.tsx` - Folder list, create/delete/sync folders, bandwidth widget
- `dashboard/TopBar.tsx` - Search, view mode toggle, bulk actions
- `dashboard/FileExplorer.tsx` - Grid/list view with virtual scrolling, sorting, context menu
- `dashboard/FileCard.tsx` / `FileListItem.tsx` - File display components
- `dashboard/MediaPlayer.tsx` - Video/audio streaming player (uses actix server)
- `dashboard/PreviewModal.tsx` - Image/file preview with LRU cache
- `dashboard/UploadQueue.tsx` / `DownloadQueue.tsx` - Transfer progress UI
- `dashboard/MoveToFolderModal.tsx` - Move files between folders
- `dashboard/ContextMenu.tsx` - Right-click file actions
- `dashboard/DragDropOverlay.tsx` / `ExternalDropBlocker.tsx` - DnD UI
- `dashboard/BandwidthWidget.tsx` - Daily bandwidth meter
- `dashboard/EmptyState.tsx` - Empty folder placeholder

### Hooks
- `useTelegramConnection.ts` - Manages Telegram connection, folder CRUD, sync, logout
- `useFileUpload.ts` - Upload queue with persistence (uses cmd_upload_file)
- `useFileDownload.ts` - Download queue with persistence (uses cmd_download_file)
- `useFileOperations.ts` - Delete, bulk delete/download/move, search
- `useFileDrop.ts` - Drag & drop detection
- `useKeyboardShortcuts.ts` - Keyboard shortcuts (delete, select all, search, escape)
- `useNetworkStatus.ts` - Network connectivity monitoring
- `useUpdateCheck.ts` - Auto-update via tauri-plugin-updater

### Contexts
- `ThemeContext.tsx` - Light/dark theme provider
- `ConfirmContext.tsx` - Confirmation dialog provider
- `DropZoneContext.tsx` - Drag-drop zone provider

## Key Tauri Commands (IPC Bridge)
All communication between frontend and backend is via `invoke()`:
- `cmd_connect(apiId)` → bool
- `cmd_check_connection()` → bool
- `cmd_auth_request_code(phone, apiId, apiHash)` → string
- `cmd_auth_sign_in(code)` → AuthResult
- `cmd_auth_check_password(password)` → AuthResult
- `cmd_logout()` → bool
- `cmd_get_files(folderId?)` → FileMetadata[]
- `cmd_upload_file(path, folderId?)` → string
- `cmd_download_file(messageId, savePath, folderId?)` → string
- `cmd_delete_file(messageId, folderId?)` → bool
- `cmd_move_files(messageIds, sourceFolderId?, targetFolderId?)` → bool
- `cmd_create_folder(name)` → FolderMetadata
- `cmd_delete_folder(folderId)` → bool
- `cmd_scan_folders()` → FolderMetadata[]
- `cmd_search_global(query)` → FileMetadata[]
- `cmd_get_preview(messageId, folderId?)` → string (base64 or path)
- `cmd_get_thumbnail(messageId, folderId?)` → string (base64)
- `cmd_get_bandwidth()` → BandwidthStats
- `cmd_is_network_available()` → bool
- `cmd_clean_cache()` → void
- `cmd_get_stream_token()` → string
- `cmd_log(message)` → void

## How Telegram Storage Works
- **Home/Root** = "Saved Messages" (user's own chat)
- **Folders** = Private Telegram Channels created with `[TD]` suffix and `[telegram-drive-folder]` in about
- **Files** = Messages with document attachments in those channels
- **File ID** = message_id within the channel
- **Folder ID** = channel_id (i64)
- **Move** = forward message to target + delete from source
- **Streaming** = Actix server streams media chunks from Telegram on localhost:14200
