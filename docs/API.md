# Telegram Drive Server — API Reference

Base URL: `http://<host>:<port>/api`

All protected endpoints require a valid session cookie (`td_session`), obtained via `/api/app-auth/login`.

---

## Public Endpoints

### `GET /api/health`

Health check.

**Response** `200`
```json
{ "status": "ok", "telegram_connected": false }
```

### `GET /api/version`

Server version.

**Response** `200`
```json
{ "name": "telegram-drive-server", "version": "0.1.0" }
```

---

## App Authentication

### `POST /api/app-auth/login`

Authenticate with the admin password. Sets the `td_session` cookie.

**Body** `application/json`
```json
{ "password": "your-admin-password" }
```

**Response** `200`
```json
{ "success": true }
```

**Error** `401` — wrong password.

### `POST /api/app-auth/logout`

Clear the current session.

**Response** `200`
```json
{ "success": true }
```

### `GET /api/app-auth/status`

Check current authentication state.

**Response** `200`
```json
{ "authenticated": true }
```

---

## Telegram Authentication *(protected)*

### `POST /api/telegram/auth/connect`

Initialize the Telegram client.

**Body**
```json
{ "api_id": 12345 }
```

### `GET /api/telegram/auth/status`

Check Telegram connection status.

**Response** `200`
```json
{ "connected": false }
```

### `POST /api/telegram/auth/request-code`

Request a login code via SMS/Telegram.

**Body**
```json
{ "phone": "+1234567890", "api_id": 12345, "api_hash": "abc123" }
```

### `POST /api/telegram/auth/sign-in`

Submit the received verification code.

**Body**
```json
{ "code": "12345" }
```

**Response** `200` — `AuthResult` with `next_step`: `"dashboard"` or `"password"`.

### `POST /api/telegram/auth/check-password`

Submit 2FA password if required.

**Body**
```json
{ "password": "my2fa" }
```

### `POST /api/telegram/auth/logout`

Disconnect from Telegram, remove session.

---

## Files *(protected)*

### `GET /api/files?folder_id=<id>`

List files in a folder. Omit `folder_id` for Saved Messages (root).

**Response** `200`
```json
[
  {
    "id": 123,
    "folder_id": 456,
    "name": "document.pdf",
    "size": 1048576,
    "mime_type": "application/pdf",
    "file_ext": "pdf",
    "created_at": "2025-01-15 10:30:00 UTC",
    "icon_type": "file"
  }
]
```

### `POST /api/files/upload?folder_id=<id>&queue=<bool>`

Upload a file via `multipart/form-data`.

| Query Param | Type | Default | Description |
|---|---|---|---|
| `folder_id` | `i64?` | `null` | Target folder (channel ID). |
| `queue` | `bool` | `false` | If `true`, enqueue for background upload. |

**Synchronous** (`queue=false`) — Response `200`
```json
{ "message": "Uploaded: document.pdf" }
```

**Queued** (`queue=true`) — Response `202`
```json
{ "id": "uuid-of-job", "status": "queued" }
```

### `GET /api/files/{message_id}/download?folder_id=<id>`

Download a file. Returns the file as a streaming response with `Content-Disposition: attachment`.

### `DELETE /api/files/{message_id}?folder_id=<id>`

Delete a file.

**Response** `200`
```json
{ "success": true }
```

### `POST /api/files/move`

Move files between folders.

**Body**
```json
{
  "message_ids": [101, 102],
  "source_folder_id": 456,
  "target_folder_id": 789
}
```

**Response** `200`
```json
{ "success": true }
```

---

## Folders *(protected)*

### `GET /api/folders`

List all Telegram Drive folders (channels with `[TD]` tag).

**Response** `200`
```json
[
  { "id": 456, "name": "Documents", "parent_id": null }
]
```

### `POST /api/folders`

Create a new folder (private channel).
If `parent_id` is set, the folder is created as a subfolder of that parent in the app hierarchy.

**Body**
```json
{ "name": "My Folder", "parent_id": null }
```

**Response** `201`
```json
{ "id": 789, "name": "My Folder", "parent_id": null }
```

### `DELETE /api/folders/{folder_id}`

Delete a folder branch in cascade order (children first, then parent).

**Response** `200`
```json
{ "success": true, "deleted_count": 3 }
```

---

## Search *(protected)*

### `GET /api/search?q=<query>`

Search files globally across all folders via Telegram's search.

**Response** `200` — same format as file list.

---

## Media *(protected)*

### `GET /api/media/stream/{message_id}?folder_id=<id>`

Stream a media file (video, audio) directly from Telegram. Returns a chunked response with the correct `Content-Type`.

### `GET /api/media/preview/{message_id}?folder_id=<id>`

Get a full preview of a media file. Results are cached in `CACHE_DIR/previews/`.

### `GET /api/media/thumbnail/{message_id}?folder_id=<id>`

Get a small thumbnail for image files. Returns `404` if not an image.

---

## Bandwidth *(protected)*

### `GET /api/bandwidth`

Get daily bandwidth usage statistics.

**Response** `200`
```json
{ "date": "2025-03-30", "up_bytes": 0, "down_bytes": 0 }
```

---

## Upload Queue *(protected)*

### `GET /api/uploads`

List all tracked upload jobs.

**Response** `200`
```json
[
  {
    "id": "uuid",
    "file_name": "video.mp4",
    "size": 52428800,
    "folder_id": 456,
    "status": "uploading",
    "error": null
  }
]
```

Statuses: `queued`, `uploading`, `completed`, `failed`, `cancelled`.

### `POST /api/uploads/{id}/cancel`

Cancel a queued upload (only `queued` jobs).

**Response** `200`
```json
{ "success": true }
```

### `DELETE /api/uploads/finished`

Remove completed/failed/cancelled entries.

**Response** `200`
```json
{ "removed": 5 }
```

---

## Error Responses

All errors return JSON:

```json
{ "error": "Description of the error" }
```

| Code | Meaning |
|---|---|
| `400` | Bad request / validation error |
| `401` | Not authenticated |
| `403` | Forbidden |
| `404` | Resource not found |
| `502` | Telegram error (upstream) |
| `500` | Internal server error |

---

## Planned (Phase 5+)

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/admin/clean-cache` | Clear preview/thumbnail cache |
| `GET` | `/api/metrics` | Server metrics |
