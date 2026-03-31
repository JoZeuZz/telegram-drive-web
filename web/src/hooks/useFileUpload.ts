import { useState, useEffect, useCallback } from 'react';
import * as api from '../lib/api';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { QueueItem } from '../types';

function createQueueId(): string {
    if (globalThis.crypto?.randomUUID) {
        return globalThis.crypto.randomUUID();
    }

    return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export function useFileUpload(activeFolderId: number | null) {
    const queryClient = useQueryClient();
    const [uploadQueue, setUploadQueue] = useState<QueueItem[]>([]);
    const [processing, setProcessing] = useState(false);
    const [isDragging, setIsDragging] = useState(false);

    // Process queue
    useEffect(() => {
        if (processing) return;
        const nextItem = uploadQueue.find(i => i.status === 'pending');
        if (nextItem) {
            processItem(nextItem);
        }
    }, [uploadQueue, processing]);

    const processItem = async (item: QueueItem) => {
        setProcessing(true);
        setUploadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'uploading' } : i));
        try {
            await api.uploadFile(item.file, item.folderId);
            setUploadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'success' } : i));
            queryClient.invalidateQueries({ queryKey: ['files', item.folderId] });
        } catch (e) {
            setUploadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'error', error: String(e) } : i));
            toast.error(`Upload failed for ${item.file.name}: ${e}`);
        } finally {
            setProcessing(false);
        }
    };

    // Opens browser file picker
    const handleManualUpload = useCallback(() => {
        const input = document.createElement('input');
        input.type = 'file';
        input.multiple = true;
        input.onchange = () => {
            const files = input.files;
            if (!files || files.length === 0) return;
            const newItems: QueueItem[] = Array.from(files).map((file) => ({
                id: createQueueId(),
                file,
                folderId: activeFolderId,
                status: 'pending',
            }));
            setUploadQueue(prev => [...prev, ...newItems]);
            toast.info(`Queued ${files.length} file(s) for upload`);
        };
        input.click();
    }, [activeFolderId]);

    // Handle drop from browser drag-and-drop
    const handleDrop = useCallback((files: FileList) => {
        const newItems: QueueItem[] = Array.from(files).map((file) => ({
            id: createQueueId(),
            file,
            folderId: activeFolderId,
            status: 'pending',
        }));
        setUploadQueue(prev => [...prev, ...newItems]);
        toast.info(`Queued ${files.length} file(s) for upload`);
    }, [activeFolderId]);

    // Global drag/drop listeners
    useEffect(() => {
        let dragCounter = 0;

        const handleDragEnter = (e: DragEvent) => {
            if (e.dataTransfer?.types.includes('Files')) {
                e.preventDefault();
                dragCounter++;
                setIsDragging(true);
            }
        };
        const handleDragOver = (e: DragEvent) => {
            if (e.dataTransfer?.types.includes('Files')) {
                e.preventDefault();
            }
        };
        const handleDragLeave = (e: DragEvent) => {
            if (e.dataTransfer?.types.includes('Files')) {
                dragCounter--;
                if (dragCounter <= 0) {
                    dragCounter = 0;
                    setIsDragging(false);
                }
            }
        };
        const handleDropEvent = (e: DragEvent) => {
            e.preventDefault();
            dragCounter = 0;
            setIsDragging(false);
            if (e.dataTransfer?.files && e.dataTransfer.files.length > 0) {
                handleDrop(e.dataTransfer.files);
            }
        };

        document.addEventListener('dragenter', handleDragEnter);
        document.addEventListener('dragover', handleDragOver);
        document.addEventListener('dragleave', handleDragLeave);
        document.addEventListener('drop', handleDropEvent);
        return () => {
            document.removeEventListener('dragenter', handleDragEnter);
            document.removeEventListener('dragover', handleDragOver);
            document.removeEventListener('dragleave', handleDragLeave);
            document.removeEventListener('drop', handleDropEvent);
        };
    }, [handleDrop]);

    return {
        uploadQueue,
        setUploadQueue,
        handleManualUpload,
        isDragging
    };
}
