import { useState, useEffect, useCallback, useRef } from 'react';
import * as api from '../lib/api';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { QueueItem } from '../types';
import { formatBytes } from '../utils';

const DEFAULT_MAX_FILE_SIZE_BYTES = 2_097_152_000;
const PROGRESS_POLL_INTERVAL_MS = 1500;
const SPEED_SAMPLE_MIN_INTERVAL_MS = 250;

type UploadStage = NonNullable<QueueItem['stage']>;

interface SpeedSample {
    stage: UploadStage;
    bytes: number;
    timestampMs: number;
    smoothedBps: number;
}

function createQueueId(): string {
    if (globalThis.crypto?.randomUUID) {
        return globalThis.crypto.randomUUID();
    }

    return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

function clampRatio(value: number): number {
    if (!Number.isFinite(value)) {
        return 0;
    }

    if (value <= 0) {
        return 0;
    }

    if (value >= 1) {
        return 1;
    }

    return value;
}

function mapSnapshotStatus(status: api.UploadProgressSnapshot['status']): QueueItem['status'] {
    if (status === 'completed') return 'success';
    if (status === 'failed') return 'error';
    if (status === 'cancelled') return 'cancelled';
    return 'uploading';
}

function deriveProgress(
    fileSizeBytes: number,
    browserToServerBytes: number,
    telegramUploadBytes: number,
    stage: UploadStage,
    status: QueueItem['status'],
): { progressPercent: number; stageProgressPercent: number } {
    if (status === 'success' || stage === 'completed') {
        return { progressPercent: 100, stageProgressPercent: 100 };
    }

    if (fileSizeBytes <= 0) {
        return { progressPercent: 0, stageProgressPercent: 0 };
    }

    const browserRatio = clampRatio(browserToServerBytes / fileSizeBytes);
    const telegramRatio = clampRatio(telegramUploadBytes / fileSizeBytes);
    const progressRatio = clampRatio((browserRatio + telegramRatio) / 2);

    let stageRatio = progressRatio;
    if (stage === 'browser_to_server') {
        stageRatio = browserRatio;
    }
    if (stage === 'server_to_telegram') {
        stageRatio = telegramRatio;
    }

    return {
        progressPercent: Math.round(progressRatio * 100),
        stageProgressPercent: Math.round(stageRatio * 100),
    };
}

export function useFileUpload(activeFolderId: number | null, maxFileSizeBytes?: number) {
    const queryClient = useQueryClient();
    const [uploadQueue, setUploadQueue] = useState<QueueItem[]>([]);
    const [processing, setProcessing] = useState(false);
    const [isDragging, setIsDragging] = useState(false);
    const inFlightControllers = useRef(new Map<string, AbortController>());
    const progressEventSources = useRef(new Map<string, EventSource>());
    const progressPollers = useRef(new Map<string, number>());
    const speedSamples = useRef(new Map<string, SpeedSample>());
    const effectiveMaxFileSizeBytes = maxFileSizeBytes && maxFileSizeBytes > 0
        ? maxFileSizeBytes
        : DEFAULT_MAX_FILE_SIZE_BYTES;

    const clearProgressTracking = useCallback((uploadId: string) => {
        const source = progressEventSources.current.get(uploadId);
        if (source) {
            source.close();
            progressEventSources.current.delete(uploadId);
        }

        const poller = progressPollers.current.get(uploadId);
        if (poller !== undefined) {
            window.clearInterval(poller);
            progressPollers.current.delete(uploadId);
        }

        speedSamples.current.delete(uploadId);
    }, []);

    const estimateSpeedAndEta = useCallback((
        uploadId: string,
        stage: UploadStage,
        stageBytes: number,
        fileSizeBytes: number,
        timestampMs: number,
    ): { speedBps?: number; etaSeconds?: number } => {
        if (stage !== 'browser_to_server' && stage !== 'server_to_telegram') {
            speedSamples.current.delete(uploadId);
            return {};
        }

        const previous = speedSamples.current.get(uploadId);
        if (!previous || previous.stage !== stage || stageBytes < previous.bytes) {
            speedSamples.current.set(uploadId, {
                stage,
                bytes: stageBytes,
                timestampMs,
                smoothedBps: 0,
            });
            return {};
        }

        const elapsedMs = timestampMs - previous.timestampMs;
        if (elapsedMs < SPEED_SAMPLE_MIN_INTERVAL_MS) {
            if (previous.smoothedBps <= 0) {
                return {};
            }
            const remaining = Math.max(0, fileSizeBytes - stageBytes);
            return {
                speedBps: previous.smoothedBps,
                etaSeconds: remaining > 0 ? Math.ceil(remaining / previous.smoothedBps) : undefined,
            };
        }

        const deltaBytes = Math.max(0, stageBytes - previous.bytes);
        const instantBps = deltaBytes / (elapsedMs / 1000);
        const smoothedBps = previous.smoothedBps > 0
            ? previous.smoothedBps * 0.7 + instantBps * 0.3
            : instantBps;

        speedSamples.current.set(uploadId, {
            stage,
            bytes: stageBytes,
            timestampMs,
            smoothedBps,
        });

        if (smoothedBps <= 0) {
            return {};
        }

        const remaining = Math.max(0, fileSizeBytes - stageBytes);
        return {
            speedBps: smoothedBps,
            etaSeconds: remaining > 0 ? Math.ceil(remaining / smoothedBps) : undefined,
        };
    }, []);

    const applySnapshot = useCallback((uploadId: string, snapshot: api.UploadProgressSnapshot) => {
        setUploadQueue((queue) => queue.map((item) => {
            if (item.id !== uploadId) {
                return item;
            }

            if (item.status === 'cancelled' && snapshot.status === 'uploading') {
                return item;
            }

            const status = mapSnapshotStatus(snapshot.status);
            const stage = snapshot.stage;
            const fileSizeBytes = snapshot.file_size_bytes > 0
                ? snapshot.file_size_bytes
                : item.file.size;
            const browserToServerBytes = Math.min(snapshot.browser_to_server_bytes, fileSizeBytes);
            const telegramUploadBytes = Math.min(snapshot.telegram_upload_bytes, fileSizeBytes);

            const { progressPercent, stageProgressPercent } = deriveProgress(
                fileSizeBytes,
                browserToServerBytes,
                telegramUploadBytes,
                stage,
                status,
            );

            const stageBytes = stage === 'server_to_telegram'
                ? telegramUploadBytes
                : browserToServerBytes;

            const { speedBps, etaSeconds } = estimateSpeedAndEta(
                uploadId,
                stage,
                stageBytes,
                fileSizeBytes,
                snapshot.updated_at_ms || Date.now(),
            );

            return {
                ...item,
                status,
                stage,
                fileSizeBytes,
                browserToServerBytes,
                telegramUploadBytes,
                progressPercent,
                stageProgressPercent,
                uploadSpeedBps: status === 'uploading' ? speedBps : undefined,
                etaSeconds: status === 'uploading' ? etaSeconds : undefined,
                error: status === 'error' ? (snapshot.error ?? item.error) : undefined,
            };
        }));
    }, [estimateSpeedAndEta]);

    const processItem = useCallback(async (item: QueueItem) => {
        setProcessing(true);
        setUploadQueue((q) => q.map((i) => i.id === item.id && i.status === 'pending'
            ? {
                ...i,
                status: 'uploading',
                stage: 'browser_to_server',
                fileSizeBytes: i.file.size,
                browserToServerBytes: 0,
                telegramUploadBytes: 0,
                progressPercent: 0,
                stageProgressPercent: 0,
                uploadSpeedBps: undefined,
                etaSeconds: undefined,
                error: undefined,
            }
            : i,
        ));

        const controller = new AbortController();
        inFlightControllers.current.set(item.id, controller);

        const onSnapshot = (snapshot: api.UploadProgressSnapshot) => {
            applySnapshot(item.id, snapshot);
            if (snapshot.status !== 'uploading') {
                clearProgressTracking(item.id);
            }
        };

        const source = api.subscribeUploadProgress(item.id, onSnapshot, () => {
            const currentSource = progressEventSources.current.get(item.id);
            if (currentSource) {
                currentSource.close();
                progressEventSources.current.delete(item.id);
            }

            if (progressPollers.current.has(item.id)) {
                return;
            }

            const poller = window.setInterval(async () => {
                try {
                    const snapshot = await api.getUploadProgress(item.id);
                    onSnapshot(snapshot);
                } catch {
                    // Snapshot may be removed after retention; UI already has terminal state.
                }
            }, PROGRESS_POLL_INTERVAL_MS);

            progressPollers.current.set(item.id, poller);
        });
        progressEventSources.current.set(item.id, source);

        try {
            await api.uploadFile(item.file, item.folderId, {
                signal: controller.signal,
                uploadId: item.id,
                uploadSizeBytes: item.file.size,
                onProgress: (loaded: number, total: number) => {
                    const totalBytes = total > 0 ? total : item.file.size;
                    setUploadQueue((queue) => queue.map((queuedItem) => {
                        if (queuedItem.id !== item.id) {
                            return queuedItem;
                        }

                        if (queuedItem.status !== 'uploading' || queuedItem.stage === 'server_to_telegram') {
                            return queuedItem;
                        }

                        const fileSizeBytes = totalBytes > 0 ? totalBytes : queuedItem.file.size;
                        const browserToServerBytes = Math.min(
                            Math.max(loaded, queuedItem.browserToServerBytes ?? 0),
                            fileSizeBytes,
                        );
                        const telegramUploadBytes = queuedItem.telegramUploadBytes ?? 0;

                        const { progressPercent, stageProgressPercent } = deriveProgress(
                            fileSizeBytes,
                            browserToServerBytes,
                            telegramUploadBytes,
                            'browser_to_server',
                            'uploading',
                        );

                        const { speedBps, etaSeconds } = estimateSpeedAndEta(
                            item.id,
                            'browser_to_server',
                            browserToServerBytes,
                            fileSizeBytes,
                            Date.now(),
                        );

                        return {
                            ...queuedItem,
                            status: 'uploading',
                            stage: 'browser_to_server',
                            fileSizeBytes,
                            browserToServerBytes,
                            progressPercent,
                            stageProgressPercent,
                            uploadSpeedBps: speedBps,
                            etaSeconds,
                            error: undefined,
                        };
                    }));
                },
            });

            setUploadQueue((q) => q.map((i) => {
                if (i.id !== item.id) {
                    return i;
                }

                const fileSizeBytes = i.fileSizeBytes ?? item.file.size;
                return {
                    ...i,
                    status: 'success',
                    stage: 'completed',
                    fileSizeBytes,
                    browserToServerBytes: fileSizeBytes,
                    telegramUploadBytes: fileSizeBytes,
                    progressPercent: 100,
                    stageProgressPercent: 100,
                    uploadSpeedBps: undefined,
                    etaSeconds: undefined,
                    error: undefined,
                };
            }));

            queryClient.invalidateQueries({ queryKey: ['files', item.folderId] });
        } catch (e) {
            if (e instanceof DOMException && e.name === 'AbortError') {
                setUploadQueue((q) => q.map((i) => i.id === item.id
                    ? {
                        ...i,
                        status: 'cancelled',
                        stage: 'cancelled',
                        uploadSpeedBps: undefined,
                        etaSeconds: undefined,
                        error: undefined,
                    }
                    : i,
                ));
                return;
            }

            const message = e instanceof Error ? e.message : String(e);
            setUploadQueue((q) => q.map((i) => i.id === item.id
                ? {
                    ...i,
                    status: 'error',
                    stage: 'failed',
                    uploadSpeedBps: undefined,
                    etaSeconds: undefined,
                    error: message,
                }
                : i,
            ));
            toast.error(`Upload failed for ${item.file.name}: ${message}`);
        } finally {
            inFlightControllers.current.delete(item.id);
            clearProgressTracking(item.id);
            setProcessing(false);
        }
    }, [applySnapshot, clearProgressTracking, estimateSpeedAndEta, queryClient]);

    const enqueueValidatedFiles = useCallback((files: File[]) => {
        if (files.length === 0) return;

        const accepted: File[] = [];
        const rejected: File[] = [];

        for (const file of files) {
            if (file.size > effectiveMaxFileSizeBytes) {
                rejected.push(file);
            } else {
                accepted.push(file);
            }
        }

        if (rejected.length > 0) {
            toast.error(
                `${rejected.length} file(s) exceed ${formatBytes(effectiveMaxFileSizeBytes)} and were not queued`,
            );
        }

        if (accepted.length === 0) {
            return;
        }

        const newItems: QueueItem[] = accepted.map((file) => ({
            id: createQueueId(),
            file,
            folderId: activeFolderId,
            status: 'pending',
            stage: 'browser_to_server',
            progressPercent: 0,
            stageProgressPercent: 0,
            browserToServerBytes: 0,
            telegramUploadBytes: 0,
            fileSizeBytes: file.size,
            uploadSpeedBps: undefined,
            etaSeconds: undefined,
        }));

        setUploadQueue((prev) => [...prev, ...newItems]);
        toast.info(`Queued ${accepted.length} file(s) for upload`);
    }, [activeFolderId, effectiveMaxFileSizeBytes]);

    const cancelQueueItem = useCallback((id: string) => {
        const target = uploadQueue.find((item) => item.id === id);
        if (!target) {
            return;
        }

        if (target.status === 'pending') {
            setUploadQueue((q) => q.map((item) => item.id === id
                ? {
                    ...item,
                    status: 'cancelled',
                    stage: 'cancelled',
                    uploadSpeedBps: undefined,
                    etaSeconds: undefined,
                }
                : item,
            ));
            clearProgressTracking(id);
            toast.info(`Cancelled ${target.file.name}`);
            return;
        }

        if (target.status === 'uploading') {
            const controller = inFlightControllers.current.get(id);
            if (!controller) {
                toast.error(`Cannot cancel ${target.file.name} right now`);
                return;
            }

            controller.abort();
            clearProgressTracking(id);
            setUploadQueue((q) => q.map((item) => item.id === id
                ? {
                    ...item,
                    status: 'cancelled',
                    stage: 'cancelled',
                    uploadSpeedBps: undefined,
                    etaSeconds: undefined,
                    error: undefined,
                }
                : item,
            ));
            toast.info(`Cancelling ${target.file.name}...`);
        }
    }, [clearProgressTracking, uploadQueue]);

    useEffect(() => {
        return () => {
            for (const controller of inFlightControllers.current.values()) {
                controller.abort();
            }

            for (const source of progressEventSources.current.values()) {
                source.close();
            }

            for (const poller of progressPollers.current.values()) {
                window.clearInterval(poller);
            }

            inFlightControllers.current.clear();
            progressEventSources.current.clear();
            progressPollers.current.clear();
            speedSamples.current.clear();
        };
    }, []);

    // Process queue
    useEffect(() => {
        if (processing) return;
        const nextItem = uploadQueue.find(i => i.status === 'pending');
        if (nextItem) {
            processItem(nextItem);
        }
    }, [uploadQueue, processing, processItem]);

    // Opens browser file picker
    const handleManualUpload = useCallback(() => {
        const input = document.createElement('input');
        input.type = 'file';
        input.multiple = true;
        input.onchange = () => {
            const files = input.files;
            if (!files || files.length === 0) return;
            enqueueValidatedFiles(Array.from(files));
        };
        input.click();
    }, [enqueueValidatedFiles]);

    // Handle drop from browser drag-and-drop
    const handleDrop = useCallback((files: FileList) => {
        enqueueValidatedFiles(Array.from(files));
    }, [enqueueValidatedFiles]);

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
        cancelQueueItem,
        handleManualUpload,
        isDragging
    };
}
