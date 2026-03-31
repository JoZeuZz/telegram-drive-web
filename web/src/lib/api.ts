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

export interface BandwidthStats {
  date: string;
  up_bytes: number;
  down_bytes: number;
}

export interface UploadJob {
  id: string;
  file_name: string;
  size: number;
  folder_id: number | null;
  status: "queued" | "uploading" | "completed" | "failed" | "cancelled";
  error: string | null;
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

export const listFiles = (folderId: number | null) => {
  const qs = folderId != null ? `?folder_id=${folderId}` : "";
  return request<TelegramFile[]>(`/files${qs}`);
};

export const deleteFile = (messageId: number, folderId: number | null) => {
  const qs = folderId != null ? `?folder_id=${folderId}` : "";
  return request<{ success: boolean }>(`/files/${messageId}${qs}`, {
    method: "DELETE",
  });
};

export const moveFiles = (
  messageIds: number[],
  sourceFolderId: number | null,
  targetFolderId: number | null,
) =>
  request<{ success: boolean }>(
    "/files/move",
    json({
      message_ids: messageIds,
      source_folder_id: sourceFolderId,
      target_folder_id: targetFolderId,
    }),
  );

export const uploadFile = async (
  file: File,
  folderId: number | null,
  queue = false,
): Promise<{ message?: string; id?: string; status?: string }> => {
  const fd = new FormData();
  fd.append("file", file);
  const qs = new URLSearchParams();
  if (folderId != null) qs.set("folder_id", String(folderId));
  if (queue) qs.set("queue", "true");
  const qsStr = qs.toString() ? `?${qs}` : "";

  const res = await fetch(`${BASE}/files/upload${qsStr}`, {
    method: "POST",
    credentials: "include",
    headers: { "X-Requested-With": "XMLHttpRequest" },
    body: fd,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new ApiError(res.status, body.error ?? res.statusText);
  }
  return res.json();
};

export const downloadFileUrl = (
  messageId: number,
  folderId: number | null,
) => {
  const qs = folderId != null ? `?folder_id=${folderId}` : "";
  return `${BASE}/files/${messageId}/download${qs}`;
};

// ─── folders ────────────────────────────────────────────────────────

export const listFolders = () => request<TelegramFolder[]>("/folders");

export const createFolder = (name: string, parentId: number | null = null) =>
  request<TelegramFolder>("/folders", json({ name, parent_id: parentId }));

export const deleteFolder = (folderId: number) =>
  request<{ success: boolean; deleted_count: number }>(`/folders/${folderId}`, {
    method: "DELETE",
  });

// ─── search ─────────────────────────────────────────────────────────

export const searchFiles = (query: string) =>
  request<TelegramFile[]>(`/search?q=${encodeURIComponent(query)}`);

// ─── media ──────────────────────────────────────────────────────────

export const streamUrl = (messageId: number, folderId: number | null) => {
  const qs = folderId != null ? `?folder_id=${folderId}` : "";
  return `${BASE}/media/stream/${messageId}${qs}`;
};

export const previewUrl = (messageId: number, folderId: number | null) => {
  const qs = folderId != null ? `?folder_id=${folderId}` : "";
  return `${BASE}/media/preview/${messageId}${qs}`;
};

export const thumbnailUrl = (messageId: number, folderId: number | null) => {
  const qs = folderId != null ? `?folder_id=${folderId}` : "";
  return `${BASE}/media/thumbnail/${messageId}${qs}`;
};

// ─── bandwidth ──────────────────────────────────────────────────────

export const getBandwidth = () => request<BandwidthStats>("/bandwidth");

// ─── upload queue ───────────────────────────────────────────────────

export const getUploads = () => request<UploadJob[]>("/uploads");

export const cancelUpload = (id: string) =>
  request<{ success: boolean }>(`/uploads/${id}/cancel`, { method: "POST" });

export const clearFinishedUploads = () =>
  request<{ removed: number }>("/uploads/finished", { method: "DELETE" });
