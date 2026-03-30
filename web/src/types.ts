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
    parent_id?: number | null;
}

export interface QueueItem {
    id: string;
    file: File;
    folderId: number | null;
    status: 'pending' | 'uploading' | 'success' | 'error';
    error?: string;
}

export interface BandwidthStats {
    date?: string;
    up_bytes: number;
    down_bytes: number;
}

export interface DownloadItem {
    id: string;
    messageId: number;
    filename: string;
    folderId: number | null;
    status: 'pending' | 'downloading' | 'success' | 'error';
    error?: string;
}
