/**
 * Typed HTTP client for the Telegram Drive server API.
 *
 * All methods use fetch() with credentials: "include" so the
 * `td_session` cookie is sent/received automatically.
 */

const BASE = "/api";

// ─── helpers ────────────────────────────────────────────────────────

async function request<T>(
  path: string,
  init?: RequestInit,
): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    credentials: "include",
    ...init,
    headers: {
      "X-Requested-With": "XMLHttpRequest",
      ...(init?.headers ?? {}),
    },
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new ApiError(res.status, body.error ?? res.statusText);
  }
  return res.json() as Promise<T>;
}

function json(body: unknown): RequestInit {
  return {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  };
}

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

// ─── response types ─────────────────────────────────────────────────

export interface HealthResponse {
  status: string;
  telegram_connected: boolean;
}

export interface VersionResponse {
  name: string;
  version: string;
}

export interface AuthStatusResponse {
  authenticated: boolean;
}

export interface TelegramStatusResponse {
  connected: boolean;
}

export interface AuthResult {
  success: boolean;
  next_step?: string;
}

export interface TelegramFile {
  id: number;
  folder_id: number | null;
  name: string;
  size: number;
  mime_type: string;
  file_ext: string;
  created_at: string;
  icon_type: string;
}

export interface TelegramFolder {
  id: number;
  name: string;
  parent_id: number | null;
}

export interface Forum {
  id: number;
  name: string;
}

export interface ForumTopic {
  id: number;
  forum_id: number;
  title: string;
  icon_color: number;
  icon_emoji_id: number | null;
  closed: boolean;
  hidden: boolean;
  pinned: boolean;
  top_message: number;
}

export interface FolderSyncSummary {
  resolved_by_title: number;
  resolved_by_about: number;
  orphans: number;
  migrated: number;
}

export interface FolderSyncResponse {
  folders: TelegramFolder[];
  summary: FolderSyncSummary;
}

export interface ListForumsResponse {
  forums: Forum[];
}

export interface ListForumTopicsResponse {
  topics: ForumTopic[];
}

export type AccountTier = "free" | "premium";

export interface BandwidthStats {
  date: string;
  up_bytes: number;
  down_bytes: number;
  limit_bytes?: number;
  remaining_bytes?: number;
  tier?: AccountTier;
  dynamic_limits_enabled?: boolean;
  fallback_mode?: boolean;
}

export interface MetricsResponse {
  uptime_secs: number;
  cache_bytes: number;
  cache_files: number;
  max_file_size_bytes: number;
  max_file_size_tier?: AccountTier;
  dynamic_limits_enabled?: boolean;
  fallback_mode?: boolean;
  telegram_account_cached?: boolean;
  bandwidth: BandwidthStats;
  telegram_connected: boolean;
  upload_queue_length: number;
}

export interface AccountInfoProfile {
  user_id: number;
  first_name: string | null;
  last_name: string | null;
  username: string | null;
  phone: string | null;
  is_premium: boolean;
  checked_at_unix_ms: number;
}

export interface AccountInfoResponse {
  authenticated: boolean;
  dynamic_limits_enabled: boolean;
  fallback_mode: boolean;
  tier: AccountTier;
  limits: {
    file_size_limit_bytes: number;
    daily_bandwidth_limit_bytes: number;
  };
  bandwidth: {
    date: string;
    up_bytes: number;
    down_bytes: number;
    limit_bytes: number;
    remaining_bytes: number;
  };
  profile: AccountInfoProfile | null;
}

export interface UploadJob {
  id: string;
  file_name: string;
  size: number;
  folder_id: number | null;
  status: "queued" | "uploading" | "completed" | "failed" | "cancelled";
  error: string | null;
}

export interface UploadProgressSnapshot {
  upload_id: string;
  file_name: string;
  file_size_bytes: number;
  status: "uploading" | "completed" | "failed" | "cancelled";
  stage:
    | "browser_to_server"
    | "server_to_telegram"
    | "completed"
    | "failed"
    | "cancelled";
  browser_to_server_bytes: number;
  telegram_upload_bytes: number;
  started_at_ms: number;
  updated_at_ms: number;
  error?: string;
}

export interface UploadFileOptions {
  queue?: boolean;
  asPhoto?: boolean;
  topicId?: number;
  topicTopMessage?: number;
  signal?: AbortSignal;
  uploadId?: string;
  uploadSizeBytes?: number;
  onProgress?: (loaded: number, total: number) => void;
}

export interface MoveFilesOptions {
  sourceTopicId?: number | null;
  targetTopicId?: number | null;
  targetTopicTopMessage?: number | null;
}

// ─── public endpoints ───────────────────────────────────────────────

export const health = () => request<HealthResponse>("/health");
export const version = () => request<VersionResponse>("/version");

// ─── app auth ───────────────────────────────────────────────────────

export const login = (password: string) =>
  request<{ success: boolean }>("/app-auth/login", json({ password }));

export const logout = () =>
  request<{ success: boolean }>("/app-auth/logout", { method: "POST" });

export const authStatus = () =>
  request<AuthStatusResponse>("/app-auth/status");

// ─── telegram auth ──────────────────────────────────────────────────

export const telegramConnect = (apiId: number) =>
  request<void>("/telegram/auth/connect", json({ api_id: apiId }));

export const telegramStatus = () =>
  request<TelegramStatusResponse>("/telegram/auth/status");

export const telegramRequestCode = (
  phone: string,
  apiId: number,
  apiHash: string,
) =>
  request<void>(
    "/telegram/auth/request-code",
    json({ phone, api_id: apiId, api_hash: apiHash }),
  );

export const telegramSignIn = (code: string) =>
  request<AuthResult>("/telegram/auth/sign-in", json({ code }));

export const telegramCheckPassword = (password: string) =>
  request<AuthResult>("/telegram/auth/check-password", json({ password }));

export const telegramLogout = () =>
  request<void>("/telegram/auth/logout", { method: "POST" });

// ─── files ──────────────────────────────────────────────────────────

export const listFiles = (
  folderId: number | null,
  topicId?: number | null,
  topicTopMessage?: number | null,
) => {
  const params = new URLSearchParams();
  if (folderId != null) params.set("folder_id", String(folderId));
  if (topicId != null) params.set("topic_id", String(topicId));
  if (topicTopMessage != null) {
    params.set("topic_top_message", String(topicTopMessage));
  }
  const qs = params.toString() ? `?${params.toString()}` : "";
  return request<TelegramFile[]>(`/files${qs}`);
};

export const deleteFile = (
  messageId: number,
  folderId: number | null,
  topicId?: number | null,
) => {
  const params = new URLSearchParams();
  if (folderId != null) params.set("folder_id", String(folderId));
  if (topicId != null) params.set("topic_id", String(topicId));
  const qs = params.toString() ? `?${params.toString()}` : "";
  return request<{ success: boolean }>(`/files/${messageId}${qs}`, {
    method: "DELETE",
  });
};

export const moveFiles = (
  messageIds: number[],
  sourceFolderId: number | null,
  targetFolderId: number | null,
  options: MoveFilesOptions = {},
) =>
  request<{ success: boolean }>(
    "/files/move",
    json({
      message_ids: messageIds,
      source_folder_id: sourceFolderId,
      target_folder_id: targetFolderId,
      source_topic_id: options.sourceTopicId,
      target_topic_id: options.targetTopicId,
      target_topic_top_message: options.targetTopicTopMessage,
    }),
  );

export const uploadFile = async (
  file: File,
  folderId: number | null,
  options: UploadFileOptions = {},
): Promise<{ message?: string; id?: string; status?: string }> => {
  const queue = options.queue ?? false;
  const asPhoto =
    options.asPhoto ?? file.type.toLowerCase().startsWith("image/");

  const fd = new FormData();
  fd.append("file", file);
  const qs = new URLSearchParams();
  if (folderId != null) qs.set("folder_id", String(folderId));
  if (options.topicId != null) qs.set("topic_id", String(options.topicId));
  if (options.topicTopMessage != null) {
    qs.set("topic_top_message", String(options.topicTopMessage));
  }
  if (queue) qs.set("queue", "true");
  qs.set("as_photo", asPhoto ? "true" : "false");
  if (options.uploadId) qs.set("upload_id", options.uploadId);
  if (options.uploadSizeBytes && options.uploadSizeBytes > 0) {
    qs.set("upload_size_bytes", String(options.uploadSizeBytes));
  }
  const qsStr = qs.toString() ? `?${qs}` : "";

  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const abortHandler = () => xhr.abort();

    const cleanup = () => {
      options.signal?.removeEventListener("abort", abortHandler);
    };

    xhr.upload.addEventListener("progress", (event) => {
      if (!event.lengthComputable || !options.onProgress) return;
      options.onProgress(event.loaded, event.total);
    });

    xhr.addEventListener("load", () => {
      cleanup();

      let payload: Record<string, unknown> = {};
      if (xhr.responseText) {
        try {
          payload = JSON.parse(xhr.responseText) as Record<string, unknown>;
        } catch {
          payload = {};
        }
      }

      if (xhr.status >= 200 && xhr.status < 300) {
        resolve(payload as { message?: string; id?: string; status?: string });
        return;
      }

      reject(
        new ApiError(
          xhr.status,
          String(payload.error ?? xhr.statusText ?? "Upload failed"),
        ),
      );
    });

    xhr.addEventListener("error", () => {
      cleanup();
      reject(new Error("Network error during upload"));
    });

    xhr.addEventListener("abort", () => {
      cleanup();
      reject(new DOMException("Upload aborted", "AbortError"));
    });

    xhr.open("POST", `${BASE}/files/upload${qsStr}`, true);
    xhr.withCredentials = true;
    xhr.setRequestHeader("X-Requested-With", "XMLHttpRequest");

    options.signal?.addEventListener("abort", abortHandler);
    xhr.send(fd);
  });
};

export const downloadFileUrl = (
  messageId: number,
  folderId: number | null,
  topicId?: number | null,
) => {
  const params = new URLSearchParams();
  if (folderId != null) params.set("folder_id", String(folderId));
  if (topicId != null) params.set("topic_id", String(topicId));
  const qs = params.toString() ? `?${params.toString()}` : "";
  return `${BASE}/files/${messageId}/download${qs}`;
};

// ─── folders ────────────────────────────────────────────────────────

export const listFolders = () => request<TelegramFolder[]>("/folders");

export const createFolder = (name: string, parentId: number | null = null) =>
  request<TelegramFolder>("/folders", json({ name, parent_id: parentId }));

export const syncFolders = () =>
  request<FolderSyncResponse>("/folders/sync", { method: "POST" });

export const deleteFolder = (folderId: number) =>
  request<{ success: boolean; deleted_count: number }>(`/folders/${folderId}`, {
    method: "DELETE",
  });

// ─── forums / communities ─────────────────────────────────────────

export const listForums = () => request<ListForumsResponse>("/forums");

export const createForum = (name: string) =>
  request<Forum>("/forums", json({ name }));

export const deleteForum = (forumId: number) =>
  request<{ success: boolean }>(`/forums/${forumId}`, { method: "DELETE" });

export const listForumTopics = (forumId: number) =>
  request<ListForumTopicsResponse>(`/forums/${forumId}/topics`);

export const createForumTopic = (
  forumId: number,
  title: string,
  iconColor?: number,
  iconEmojiId?: number,
) =>
  request<ForumTopic>(
    `/forums/${forumId}/topics`,
    json({
      title,
      icon_color: iconColor,
      icon_emoji_id: iconEmojiId,
    }),
  );

export const deleteForumTopic = (
  forumId: number,
  topicId: number,
  topMessage?: number | null,
) => {
  const params = new URLSearchParams();
  if (topMessage != null) params.set("top_message", String(topMessage));
  const qs = params.toString() ? `?${params.toString()}` : "";

  return request<{ success: boolean }>(
    `/forums/${forumId}/topics/${topicId}${qs}`,
    { method: "DELETE" },
  );
};

// ─── search ─────────────────────────────────────────────────────────

export const searchFiles = (query: string) =>
  request<TelegramFile[]>(`/search?q=${encodeURIComponent(query)}`);

// ─── media ──────────────────────────────────────────────────────────

export const streamUrl = (
  messageId: number,
  folderId: number | null,
  topicId?: number | null,
) => {
  const params = new URLSearchParams();
  if (folderId != null) params.set("folder_id", String(folderId));
  if (topicId != null) params.set("topic_id", String(topicId));
  const qs = params.toString() ? `?${params.toString()}` : "";
  return `${BASE}/media/stream/${messageId}${qs}`;
};

export const previewUrl = (
  messageId: number,
  folderId: number | null,
  topicId?: number | null,
) => {
  const params = new URLSearchParams();
  if (folderId != null) params.set("folder_id", String(folderId));
  if (topicId != null) params.set("topic_id", String(topicId));
  const qs = params.toString() ? `?${params.toString()}` : "";
  return `${BASE}/media/preview/${messageId}${qs}`;
};

export const thumbnailUrl = (
  messageId: number,
  folderId: number | null,
  topicId?: number | null,
) => {
  const params = new URLSearchParams();
  if (folderId != null) params.set("folder_id", String(folderId));
  if (topicId != null) params.set("topic_id", String(topicId));
  const qs = params.toString() ? `?${params.toString()}` : "";
  return `${BASE}/media/thumbnail/${messageId}${qs}`;
};

// ─── bandwidth ──────────────────────────────────────────────────────

export const getBandwidth = () => request<BandwidthStats>("/bandwidth");

export const getMetrics = () => request<MetricsResponse>("/metrics");

export const getAccountInfo = () => request<AccountInfoResponse>("/account-info");

// ─── upload queue ───────────────────────────────────────────────────

export const getUploads = () => request<UploadJob[]>("/uploads");

export const getUploadProgress = (uploadId: string) =>
  request<UploadProgressSnapshot>(`/uploads/${encodeURIComponent(uploadId)}`);

export const subscribeUploadProgress = (
  uploadId: string,
  onSnapshot: (snapshot: UploadProgressSnapshot) => void,
  onError?: () => void,
) => {
  const source = new EventSource(
    `${BASE}/uploads/${encodeURIComponent(uploadId)}/events`,
    { withCredentials: true },
  );

  source.onmessage = (event) => {
    try {
      const snapshot = JSON.parse(event.data) as UploadProgressSnapshot;
      onSnapshot(snapshot);
    } catch {
      // Ignore malformed events and keep stream alive.
    }
  };

  source.onerror = () => {
    onError?.();
  };

  return source;
};

export const cancelUpload = (id: string) =>
  request<{ success: boolean }>(`/uploads/${id}/cancel`, { method: "POST" });

export const clearFinishedUploads = () =>
  request<{ removed: number }>("/uploads/finished", { method: "DELETE" });
