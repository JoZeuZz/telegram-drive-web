# Telegram Drive Server — API Reference

Base URL: `http://<host>:<port>/api`

All protected endpoints require a valid session cookie (`td_session`), obtained via `/api/app-auth/login`.
All mutating endpoints (`POST`, `PUT`, `PATCH`, `DELETE`) also require:

`X-Requested-With: XMLHttpRequest`

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

### `POST /api/app-auth/bootstrap` *(protected)*

Change the app admin password.

**Body**
```json
{ "current_password": "old-pass", "new_password": "new-strong-pass" }
```

**Response** `200`
```json
{ "success": true }
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

### `GET /api/files?folder_id=<id>&topic_id=<id>&topic_top_message=<msg_id>`

List files in a folder. Omit `folder_id` for Saved Messages (root).

For structured subfolders (forum topics):
- Set `folder_id` to the structured folder root ID.
- Set `topic_id` to the topic ID.
- Optionally set `topic_top_message` from `/api/forums/{forum_id}/topics` to avoid extra topic-resolution calls.

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

### `POST /api/files/upload?folder_id=<id>&topic_id=<id>&topic_top_message=<msg_id>&queue=<bool>&as_photo=<bool>`

Upload a file via `multipart/form-data`.

| Query Param | Type | Default | Description |
|---|---|---|---|
| `folder_id` | `i64?` | `null` | Target folder (channel ID). |
| `topic_id` | `i32?` | `null` | Target topic ID inside a structured folder root. |
| `topic_top_message` | `i32?` | `null` | Optional top message of the topic; reduces Telegram API round-trips when provided. |
| `queue` | `bool` | `false` | If `true`, enqueue for background upload. |
| `as_photo` | `bool` | `false` | If `true` and file is an image, upload as Telegram photo media. |

Upload limits:
- Per-file limit is enforced by `MAX_FILE_SIZE_BYTES` (default: `2097152000` bytes, ~2 GB decimal).
- Request payload is also capped by backend payload configuration and reverse proxy limits.
- Telegram account limits still apply (typically ~2 GB for non-Premium and ~4 GB for Premium accounts).

Upload behavior:
- Uploads use the original multipart filename when sending document/file media.
- `as_photo=true` prioritizes Telegram photo UX for images and does not guarantee exact filename preservation.
- `as_photo=false` sends document/file media and preserves filename/extension in Telegram attributes.

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

Supports legacy folders and structured subfolders (forum topics).

**Body**
```json
{
  "message_ids": [101, 102],
  "source_folder_id": 456,
  "source_topic_id": null,
  "target_folder_id": 789,
  "target_topic_id": null,
  "target_topic_top_message": null
}
```

Notes:
- `source_topic_id` is optional; set it when moving files from a structured subfolder.
- `target_topic_id` is optional; set it when moving files into a structured subfolder.
- `target_topic_top_message` is optional but recommended for structured destinations to reduce lookup calls.

**Response** `200`
```json
{ "success": true }
```

---

## Folders *(protected)*

### `GET /api/folders`

List all Telegram Drive folders (channels with Telegram Drive metadata in title/legacy marker).

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

### `POST /api/folders/sync`

Force a Telegram folder rescan and return integrity metrics for hierarchy resolution.

**Response** `200`
```json
{
  "folders": [
    { "id": 456, "name": "Documents", "parent_id": null },
    { "id": 789, "name": "Photos", "parent_id": 456 }
  ],
  "summary": {
    "resolved_by_title": 1,
    "resolved_by_about": 1,
    "orphans": 0,
    "migrated": 1
  }
}
```

---

## Forums / Communities *(protected, feature-flagged)*

These endpoints are enabled only when `FORUMS_ENABLED=true`.
They are additive and do not replace legacy `/api/folders` behavior.

### `GET /api/forums`

List forum-enabled Telegram communities (supergroups with forums enabled).

**Response** `200`
```json
{
  "forums": [
    { "id": 123456789, "name": "Engineering" }
  ]
}
```

### `POST /api/forums`

Create a new forum-enabled community.

**Body**
```json
{ "name": "New Community" }
```

**Response** `201`
```json
{ "id": 123456789, "name": "New Community" }
```

### `DELETE /api/forums/{forum_id}`

Delete a structured folder root (forum-enabled supergroup).

**Response** `200`
```json
{ "success": true }
```

### `GET /api/forums/{forum_id}/topics`

List topics for a specific forum community.

**Response** `200`
```json
{
  "topics": [
    {
      "id": 42,
      "forum_id": 123456789,
      "title": "Roadmap",
      "icon_color": 7313904,
      "icon_emoji_id": null,
      "closed": false,
      "hidden": false,
      "pinned": false,
      "top_message": 1001
    }
  ]
}
```

### `POST /api/forums/{forum_id}/topics`

Create a new topic in a forum community.

**Body**
```json
{
  "title": "Announcements",
  "icon_color": 7313904,
  "icon_emoji_id": null
}
```

**Response** `201`
```json
{
  "id": 43,
  "forum_id": 123456789,
  "title": "Announcements",
  "icon_color": 7313904,
  "icon_emoji_id": null,
  "closed": false,
  "hidden": false,
  "pinned": false,
  "top_message": 1002
}
```

### `DELETE /api/forums/{forum_id}/topics/{topic_id}`

Delete a structured subfolder (forum topic).

Optional query params:
- `top_message`: `i32` top message id for the topic. If omitted, the server resolves it.

**Response** `200`
```json
{ "success": true }
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

Get a full preview of a media file. Results are cached in `CACHE_DIR/media/`.

### `GET /api/media/thumbnail/{message_id}?folder_id=<id>`

Get a small thumbnail for image files. Returns `404` if not an image.

---

## Bandwidth *(protected)*

### `GET /api/bandwidth`

Get daily bandwidth usage statistics.

**Response** `200`
```json
{
  "date": "2025-03-30",
  "up_bytes": 0,
  "down_bytes": 0,
  "limit_bytes": 268435456000,
  "remaining_bytes": 268435456000,
  "tier": "free",
  "dynamic_limits_enabled": true,
  "fallback_mode": false
}
```

---

## Account Info *(protected)*

### `GET /api/account-info`

Returns cached Telegram account profile (if available) plus effective limits.

**Response** `200`
```json
{
  "authenticated": true,
  "dynamic_limits_enabled": true,
  "fallback_mode": false,
  "tier": "premium",
  "limits": {
    "file_size_limit_bytes": 4294967296,
    "daily_bandwidth_limit_bytes": 858993459200
  },
  "bandwidth": {
    "date": "2025-03-30",
    "up_bytes": 0,
    "down_bytes": 0,
    "limit_bytes": 858993459200,
    "remaining_bytes": 858993459200
  },
  "profile": {
    "user_id": 123456789,
    "first_name": "Alice",
    "last_name": null,
    "username": "alice",
    "phone": "+1234567890",
    "is_premium": true,
    "checked_at_unix_ms": 1764595200000
  }
}
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

## Admin *(protected)*

### `POST /api/admin/clean-cache`

Trigger cache cleanup manually.

**Response** `200`
```json
{ "files_removed": 12, "bytes_freed": 10485760 }
```

---

## Metrics *(protected)*

### `GET /api/metrics`

Operational runtime metrics.

**Response** `200`
```json
{
  "uptime_secs": 3600,
  "cache_bytes": 12345,
  "cache_files": 12,
  "max_file_size_bytes": 2097152000,
  "max_file_size_tier": "free",
  "dynamic_limits_enabled": true,
  "fallback_mode": false,
  "telegram_account_cached": true,
  "bandwidth": {
    "date": "2026-03-31",
    "up_bytes": 0,
    "down_bytes": 0,
    "limit_bytes": 268435456000,
    "remaining_bytes": 268435456000,
    "tier": "free"
  },
  "telegram_connected": false,
  "upload_queue_length": 0
}
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
