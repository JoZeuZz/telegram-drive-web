import { useState, useEffect, useCallback, useRef } from 'react';
import { AnimatePresence } from 'framer-motion';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import * as api from '../lib/api';

import { TelegramFile } from '../types';
import { formatBytes } from '../utils';

// Components
import { Sidebar } from './dashboard/Sidebar';
import { TopBar } from './dashboard/TopBar';
import { FileExplorer } from './dashboard/FileExplorer';
import { UploadQueue } from './dashboard/UploadQueue';
import { DownloadQueue } from './dashboard/DownloadQueue';
import { MoveToFolderModal } from './dashboard/MoveToFolderModal';
import { PreviewModal } from './dashboard/PreviewModal';
import { MediaPlayer } from './dashboard/MediaPlayer';
import { DragDropOverlay } from './dashboard/DragDropOverlay';
import { ExternalDropBlocker } from './dashboard/ExternalDropBlocker';

// Hooks
import { useTelegramConnection } from '../hooks/useTelegramConnection';
import { useFileOperations } from '../hooks/useFileOperations';
import { useFileUpload } from '../hooks/useFileUpload';
import { useFileDownload } from '../hooks/useFileDownload';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';

export function Dashboard({ onLogout }: { onLogout: () => void }) {
    const queryClient = useQueryClient();


    const {
        folders, activeFolderId, setActiveFolderId, isSyncing, isConnected,
        lastSyncSummary, handleLogout, handleSyncFolders, handleCreateFolder, handleFolderDelete
    } = useTelegramConnection(onLogout);


    const [previewFile, setPreviewFile] = useState<TelegramFile | null>(null);
    const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');
    const [selectedIds, setSelectedIds] = useState<number[]>([]);
    const [selectionAnchorId, setSelectionAnchorId] = useState<number | null>(null);
    const [selectionFocusId, setSelectionFocusId] = useState<number | null>(null);
    const [visibleOrderedIds, setVisibleOrderedIds] = useState<number[]>([]);
    const [showMoveModal, setShowMoveModal] = useState(false);
    const [searchTerm, setSearchTerm] = useState("");
    const [searchResults, setSearchResults] = useState<TelegramFile[]>([]);
    const [isSearching, setIsSearching] = useState(false);
    const [isWindowVisible, setIsWindowVisible] = useState(
        typeof document === 'undefined' ? true : document.visibilityState !== 'hidden'
    );
    const [internalDragFileId, _setInternalDragFileId] = useState<number | null>(null);
    const internalDragRef = useRef<number | null>(null);

    const setInternalDragFileId = (id: number | null) => {
        internalDragRef.current = id;
        _setInternalDragFileId(id);
    };
    const [playingFile, setPlayingFile] = useState<TelegramFile | null>(null);
    const [previewContextFiles, setPreviewContextFiles] = useState<TelegramFile[]>([]);
    const [previewContextIndex, setPreviewContextIndex] = useState(-1);

    useEffect(() => {
        const saved = localStorage.getItem('viewMode') as 'grid' | 'list' | null;
        if (saved) setViewMode(saved);
    }, []);

    useEffect(() => {
        localStorage.setItem('viewMode', viewMode);
    }, [viewMode]);


    const { data: allFiles = [], isLoading, error } = useQuery<TelegramFile[]>({
        queryKey: ['files', activeFolderId],
        queryFn: () => api.listFiles(activeFolderId).then(res => res.map(f => ({
            ...f,
            sizeStr: formatBytes(f.size),
            type: (f.icon_type || (f.name.endsWith('/') ? 'folder' : 'file')) as 'file' | 'folder',
        }))),
    });

    const displayedFiles = searchTerm.length > 2
        ? searchResults
        : allFiles.filter((f: TelegramFile) => f.name.toLowerCase().includes(searchTerm.toLowerCase()));

    useEffect(() => {
        const onVisibilityChange = () => {
            setIsWindowVisible(document.visibilityState !== 'hidden');
        };

        document.addEventListener('visibilitychange', onVisibilityChange);
        return () => document.removeEventListener('visibilitychange', onVisibilityChange);
    }, []);

    const bandwidthPollingEnabled = isConnected && isWindowVisible;

    const { data: bandwidth } = useQuery({
        queryKey: ['bandwidth'],
        queryFn: () => api.getBandwidth(),
        enabled: bandwidthPollingEnabled,
        refetchInterval: bandwidthPollingEnabled ? 5000 : false,
        refetchOnWindowFocus: false,
    });

    const { data: metrics } = useQuery({
        queryKey: ['metrics'],
        queryFn: () => api.getMetrics(),
        enabled: isWindowVisible,
        refetchOnWindowFocus: false,
        staleTime: 60_000,
    });


    const {
        handleDelete, handleBulkDelete, handleDownload, handleBulkDownload,
        handleBulkMove, handleDownloadFolder, handleGlobalSearch, isDeleting, deleteProgress

    } = useFileOperations(activeFolderId, selectedIds, setSelectedIds, displayedFiles);

    const { uploadQueue, setUploadQueue, cancelQueueItem, handleManualUpload, isDragging } = useFileUpload(
        activeFolderId,
        metrics?.max_file_size_bytes,
    );
    const { downloadQueue, clearFinished: clearDownloads } = useFileDownload();


    const handleSelectAll = useCallback(() => {
        const ids = visibleOrderedIds.length > 0
            ? visibleOrderedIds
            : displayedFiles.map((f) => f.id);

        setSelectedIds(ids);
        setSelectionAnchorId(ids[0] ?? null);
        setSelectionFocusId(ids[ids.length - 1] ?? null);
    }, [visibleOrderedIds, displayedFiles]);

    const handleKeyboardDelete = useCallback(() => {
        if (selectedIds.length > 0) {
            handleBulkDelete();
        }
    }, [selectedIds, handleBulkDelete]);

    const handleEscape = useCallback(() => {
        setSelectedIds([]);
        setSelectionAnchorId(null);
        setSelectionFocusId(null);
        setSearchTerm("");
        setPreviewFile(null);
        setPlayingFile(null);
    }, []);

    const handleExtendSelection = useCallback((direction: 'up' | 'down') => {
        if (visibleOrderedIds.length === 0) return;

        const delta = direction === 'down' ? 1 : -1;
        const fallbackId = visibleOrderedIds[0] ?? null;
        const anchorId = selectionAnchorId ?? selectionFocusId ?? selectedIds[0] ?? fallbackId;
        if (anchorId === null) return;

        const anchorIndex = visibleOrderedIds.indexOf(anchorId);
        if (anchorIndex === -1) return;

        const baseFocusId = selectionFocusId ?? selectedIds[selectedIds.length - 1] ?? anchorId;
        const baseFocusIndex = visibleOrderedIds.indexOf(baseFocusId);
        const focusIndex = baseFocusIndex === -1 ? anchorIndex : baseFocusIndex;

        const nextIndex = Math.max(0, Math.min(visibleOrderedIds.length - 1, focusIndex + delta));
        const rangeStart = Math.min(anchorIndex, nextIndex);
        const rangeEnd = Math.max(anchorIndex, nextIndex);
        const rangeIds = visibleOrderedIds.slice(rangeStart, rangeEnd + 1);

        setSelectedIds(rangeIds);
        setSelectionAnchorId(anchorId);
        setSelectionFocusId(visibleOrderedIds[nextIndex] ?? baseFocusId);
    }, [visibleOrderedIds, selectionAnchorId, selectionFocusId, selectedIds]);

    const handleFocusSearch = useCallback(() => {
        const searchInput = document.querySelector('input[placeholder="Search files..."]') as HTMLInputElement;
        if (searchInput) {
            searchInput.focus();
            searchInput.select();
        }
    }, []);

    const handleEnter = useCallback(() => {
        if (selectedIds.length === 1) {
            const selected = displayedFiles.find(f => f.id === selectedIds[0]);
            if (selected) {
                if (selected.type === 'folder') {
                    setActiveFolderId(selected.id);
                } else {
                    handlePreview(selected, displayedFiles);
                }
            }
        }
    }, [selectedIds, displayedFiles, setActiveFolderId]);

    useKeyboardShortcuts({
        onSelectAll: handleSelectAll,
        onDelete: handleKeyboardDelete,
        onEscape: handleEscape,
        onSearch: handleFocusSearch,
        onEnter: handleEnter,
        onExtendSelection: handleExtendSelection,
        enabled: !previewFile && !playingFile && !showMoveModal // Disable when modals are open
    });


    useEffect(() => {
        setSelectedIds([]);
        setShowMoveModal(false);
        setSearchTerm("");
        setSearchResults([]);
        setPreviewFile(null);
        setPlayingFile(null);
        setPreviewContextFiles([]);
        setPreviewContextIndex(-1);
        setSelectionAnchorId(null);
        setSelectionFocusId(null);
        setVisibleOrderedIds([]);
    }, [activeFolderId]);


    useEffect(() => {
        if (searchTerm.length <= 2) {
            setSearchResults([]);
            return;
        }

        const timer = setTimeout(async () => {
            setIsSearching(true);
            const results = await handleGlobalSearch(searchTerm);
            setSearchResults(results.map(f => ({
                ...f,
                sizeStr: formatBytes(f.size),
                type: (f.icon_type || (f.name.endsWith('/') ? 'folder' : 'file')) as 'file' | 'folder',
            })));
            setIsSearching(false);
        }, 500);

        return () => clearTimeout(timer);
    }, [searchTerm]);




    const handleFileClick = (e: React.MouseEvent, id: number, orderedIds: number[]) => {
        e.stopPropagation();

        const useOrderedIds = orderedIds.length > 0 ? orderedIds : visibleOrderedIds;
        const isMod = e.metaKey || e.ctrlKey;

        if (e.shiftKey && useOrderedIds.length > 0) {
            const anchorId = selectionAnchorId ?? selectionFocusId ?? id;
            const anchorIndex = useOrderedIds.indexOf(anchorId);
            const targetIndex = useOrderedIds.indexOf(id);

            if (anchorIndex !== -1 && targetIndex !== -1) {
                const rangeStart = Math.min(anchorIndex, targetIndex);
                const rangeEnd = Math.max(anchorIndex, targetIndex);
                const rangeIds = useOrderedIds.slice(rangeStart, rangeEnd + 1);

                setSelectedIds((prev) => {
                    if (!isMod) return rangeIds;
                    return Array.from(new Set([...prev, ...rangeIds]));
                });

                if (selectionAnchorId === null) {
                    setSelectionAnchorId(anchorId);
                }

                setSelectionFocusId(id);
                return;
            }
        }

        if (isMod) {
            setSelectedIds(ids => ids.includes(id) ? ids.filter(i => i !== id) : [...ids, id]);
            setSelectionAnchorId(id);
            setSelectionFocusId(id);
        } else {
            setSelectedIds([id]);
            setSelectionAnchorId(id);
            setSelectionFocusId(id);
        }
    }

    const handlePreview = (file: TelegramFile, orderedFiles?: TelegramFile[]) => {
        const contextFiles = (orderedFiles || displayedFiles).filter((f) => f.type !== 'folder');
        const contextIndex = contextFiles.findIndex((f) => f.id === file.id);

        setPreviewContextFiles(contextFiles);
        setPreviewContextIndex(contextIndex);

        const isMedia = ['mp4', 'webm', 'ogg', 'mov', 'mkv', 'avi', 'mp3', 'wav', 'aac', 'flac', 'm4a', 'opus']
            .some(ext => file.name.toLowerCase().endsWith(ext));

        if (isMedia) {
            setPlayingFile(file);
            setPreviewFile(null);
        } else {
            setPreviewFile(file);
            setPlayingFile(null);
        }
    };

    const navigatePreview = useCallback((step: 1 | -1) => {
        if (previewContextFiles.length === 0) return;

        const currentFileId = previewFile?.id ?? playingFile?.id;
        if (!currentFileId) return;

        const currentIndex = previewContextFiles.findIndex((f) => f.id === currentFileId);
        if (currentIndex === -1) return;

        const nextIndex = (currentIndex + step + previewContextFiles.length) % previewContextFiles.length;
        const nextFile = previewContextFiles[nextIndex];
        if (!nextFile) return;

        setPreviewContextIndex(nextIndex);

        const isMedia = ['mp4', 'webm', 'ogg', 'mov', 'mkv', 'avi', 'mp3', 'wav', 'aac', 'flac', 'm4a', 'opus']
            .some(ext => nextFile.name.toLowerCase().endsWith(ext));

        if (isMedia) {
            setPlayingFile(nextFile);
            setPreviewFile(null);
        } else {
            setPreviewFile(nextFile);
            setPlayingFile(null);
        }
    }, [previewContextFiles, previewFile, playingFile]);

    const handleNextPreview = useCallback(() => {
        navigatePreview(1);
    }, [navigatePreview]);

    const handlePrevPreview = useCallback(() => {
        navigatePreview(-1);
    }, [navigatePreview]);

    const previewNeighborFiles = useCallback(() => {
        if (previewContextFiles.length === 0) {
            return { nextFile: null as TelegramFile | null, prevFile: null as TelegramFile | null };
        }

        const currentFileId = previewFile?.id ?? playingFile?.id;
        if (!currentFileId) {
            return { nextFile: null as TelegramFile | null, prevFile: null as TelegramFile | null };
        }

        const currentIdx = previewContextFiles.findIndex((f) => f.id === currentFileId);
        if (currentIdx === -1) {
            return { nextFile: null as TelegramFile | null, prevFile: null as TelegramFile | null };
        }

        const nextIdx = (currentIdx + 1) % previewContextFiles.length;
        const prevIdx = (currentIdx - 1 + previewContextFiles.length) % previewContextFiles.length;

        return {
            nextFile: previewContextFiles[nextIdx] || null,
            prevFile: previewContextFiles[prevIdx] || null,
        };
    }, [previewContextFiles, previewFile, playingFile]);

    const handleDropOnFolder = async (e: React.DragEvent, targetFolderId: number | null) => {
        e.preventDefault();
        e.stopPropagation();

        const dataTransferFileId = e.dataTransfer.getData("application/x-telegram-file-id");

        if (activeFolderId === targetFolderId) return;

        const fileId = internalDragRef.current || (dataTransferFileId ? parseInt(dataTransferFileId) : null);

        if (fileId) {
            try {
                const idsToMove = selectedIds.includes(fileId) ? selectedIds : [fileId];

                await api.moveFiles(idsToMove, activeFolderId, targetFolderId);

                queryClient.invalidateQueries({ queryKey: ['files', activeFolderId] });

                if (selectedIds.includes(fileId)) setSelectedIds([]);

                toast.success(`Moved ${idsToMove.length} file(s).`);

                setInternalDragFileId(null);
            } catch {
                toast.error(`Failed to move file(s).`);
            }
        }
    }

    const currentFolderName = activeFolderId === null
        ? "Saved Messages"
        : folders.find(f => f.id === activeFolderId)?.name || "Folder";


    const handleRootDragOver = (e: React.DragEvent) => {
        if (internalDragRef.current) {
            e.preventDefault();
            e.stopPropagation();
            e.dataTransfer.dropEffect = 'move';
        }
    };

    const handleRootDragEnter = (e: React.DragEvent) => {
        if (internalDragRef.current) {
            e.preventDefault();
            e.stopPropagation();
            e.dataTransfer.dropEffect = 'move';
        }
    };

    const previewNeighbors = previewNeighborFiles();

    return (
        <div
            className="flex h-screen w-full overflow-hidden bg-telegram-bg relative"
            onClick={() => setSelectedIds([])}
            onDragOver={handleRootDragOver}
            onDragEnter={handleRootDragEnter}
        >

            <ExternalDropBlocker onUploadClick={handleManualUpload} />

            <AnimatePresence>
                {showMoveModal && (
                    <MoveToFolderModal
                        folders={folders}
                        onClose={() => setShowMoveModal(false)}
                        onSelect={handleBulkMove}
                        activeFolderId={activeFolderId}
                        key="move-modal"
                    />
                )}
                {playingFile && (
                    <MediaPlayer
                        file={playingFile}
                        onClose={() => setPlayingFile(null)}
                        onNext={handleNextPreview}
                        onPrev={handlePrevPreview}
                        currentIndex={previewContextIndex}
                        totalItems={previewContextFiles.length}
                        activeFolderId={activeFolderId}
                        key="media-player"
                    />
                )}
                {isDragging && internalDragFileId === null && <DragDropOverlay key="drag-drop-overlay" />}
            </AnimatePresence>

            <Sidebar
                folders={folders}
                activeFolderId={activeFolderId}
                setActiveFolderId={setActiveFolderId}
                onDrop={handleDropOnFolder}
                onDelete={handleFolderDelete}
                onCreate={handleCreateFolder}
                isSyncing={isSyncing}
                isConnected={isConnected}
                syncSummary={lastSyncSummary}
                onSync={handleSyncFolders}
                onLogout={handleLogout}
                bandwidth={bandwidth || null}
            />

            <main className="flex-1 flex flex-col" onClick={(e) => { if (e.target === e.currentTarget) setSelectedIds([]); }}>
                <TopBar
                    currentFolderName={currentFolderName}
                    selectedIds={selectedIds}
                    onShowMoveModal={() => setShowMoveModal(true)}
                    onBulkDownload={handleBulkDownload}
                    onBulkDelete={handleBulkDelete}
                    onDownloadFolder={handleDownloadFolder}
                    isDeleting={isDeleting}
                    deleteProgress={deleteProgress}
                    viewMode={viewMode}
                    setViewMode={setViewMode}
                    searchTerm={searchTerm}
                    onSearchChange={setSearchTerm}
                />
                {searchTerm.length > 2 && (
                    <div className="px-6 pt-4 pb-0">
                        <h2 className="text-sm font-medium text-telegram-subtext">
                            Search Results for <span className="text-telegram-primary">"{searchTerm}"</span>
                        </h2>
                    </div>
                )}
                <FileExplorer

                    files={displayedFiles}
                    loading={isLoading || isSearching}
                    error={error}
                    viewMode={viewMode}
                    selectedIds={selectedIds}
                    activeFolderId={activeFolderId}
                    onFileClick={handleFileClick}
                    onVisibleOrderChange={setVisibleOrderedIds}
                    onDelete={handleDelete}
                    onDownload={handleDownload}
                    onPreview={handlePreview}
                    onManualUpload={handleManualUpload}
                    onSelectionClear={() => setSelectedIds([])}
                    onDrop={handleDropOnFolder}
                    onDragStart={(fileId) => setInternalDragFileId(fileId)}
                    onDragEnd={() => setTimeout(() => setInternalDragFileId(null), 50)}
                />
            </main>

            {previewFile && (
                <PreviewModal
                    file={previewFile}
                    activeFolderId={activeFolderId}
                    onClose={() => setPreviewFile(null)}
                    onNext={handleNextPreview}
                    onPrev={handlePrevPreview}
                    currentIndex={previewContextIndex}
                    totalItems={previewContextFiles.length}
                    nextFile={previewNeighbors.nextFile}
                    prevFile={previewNeighbors.prevFile}
                />
            )}


            <UploadQueue
                items={uploadQueue}
                onCancelItem={cancelQueueItem}
                onClearFinished={() => setUploadQueue(q => q.filter(
                    i => i.status !== 'success' && i.status !== 'error' && i.status !== 'cancelled',
                ))}
            />
            <DownloadQueue
                items={downloadQueue}
                onClearFinished={clearDownloads}
            />
        </div>
    );
}
