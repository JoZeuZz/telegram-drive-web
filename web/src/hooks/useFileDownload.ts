import { useState } from 'react';
import { DownloadItem } from '../types';

/**
 * In the web version, downloads happen via native browser mechanism
 * (Content-Disposition: attachment headers from the server).
 * This hook is kept for UI symmetry with UploadQueue but is mostly a no-op.
 */
export function useFileDownload() {
    const [downloadQueue] = useState<DownloadItem[]>([]);

    const clearFinished = () => {
        // No-op in web: browser handles downloads natively
    };

    return {
        downloadQueue,
        clearFinished
    };
}
