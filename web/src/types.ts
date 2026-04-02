export interface TelegramFile {
    id: number;
    name: string;
    size: number;
    sizeStr: string; // Computed client-side from size
    created_at?: string;
    type?: 'folder' | 'file'; // derived from icon_type
    mime_type?: string;
    file_ext?: string;
    folder_id?: number | null;
}

export interface TelegramFolder {
    id: number;
    name: string;
    parent_id: number | null;
}

export interface FolderSyncSummary {
    resolved_by_title: number;
    resolved_by_about: number;
    orphans: number;
    migrated: number;
}

export interface QueueItem {
    id: string;
    file: File;
    folderId: number | null;
    topicId?: number | null;
    topicTopMessage?: number | null;
    status: 'pending' | 'uploading' | 'success' | 'error' | 'cancelled';
    stage?: 'browser_to_server' | 'server_to_telegram' | 'completed' | 'failed' | 'cancelled';
    progressPercent?: number;
    stageProgressPercent?: number;
    browserToServerBytes?: number;
    telegramUploadBytes?: number;
    fileSizeBytes?: number;
    uploadSpeedBps?: number;
    etaSeconds?: number;
    error?: string;
}

export interface BandwidthStats {
    date?: string;
    up_bytes: number;
    down_bytes: number;
    limit_bytes?: number;
    remaining_bytes?: number;
    tier?: 'free' | 'premium';
    dynamic_limits_enabled?: boolean;
    fallback_mode?: boolean;
}

export interface DownloadItem {
    id: string;
    messageId: number;
    filename: string;
    folderId: number | null;
    status: 'pending' | 'downloading' | 'success' | 'error';
    error?: string;
}
