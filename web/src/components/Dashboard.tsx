import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { AnimatePresence } from 'framer-motion';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import * as api from '../lib/api';

import { TelegramFile } from '../types';
import { formatBytes } from '../utils';

// Components
import { Sidebar, type SidebarDropTarget } from './dashboard/Sidebar';
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
        folders, activeFolderId, setActiveFolderId: setActiveFolderIdRaw, isSyncing, isConnected,
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
    const [activeStructuredTopic, setActiveStructuredTopic] = useState<{
        forumId: number;
        topicId: number;
        title: string;
        topMessage: number | null;
    } | null>(null);

    useEffect(() => {
        const saved = localStorage.getItem('viewMode') as 'grid' | 'list' | null;
        if (saved) setViewMode(saved);
    }, []);

    useEffect(() => {
        localStorage.setItem('viewMode', viewMode);
    }, [viewMode]);

    const setActiveFolderId = useCallback((id: number | null) => {
        setActiveStructuredTopic(null);
        setActiveFolderIdRaw(id);
    }, [setActiveFolderIdRaw]);

    const openStructuredTopic = useCallback((
        forumId: number,
        topicId: number,
        title: string,
        topMessage?: number | null,
    ) => {
        setActiveFolderIdRaw(forumId);
        setActiveStructuredTopic({
            forumId,
            topicId,
            title,
            topMessage: topMessage ?? null,
        });
    }, [setActiveFolderIdRaw]);


    const folderIdSet = useMemo(() => new Set(folders.map((folder) => folder.id)), [folders]);

    const { data: structuredFoldersResponse } = useQuery({
        queryKey: ['structured-folders'],
        queryFn: () => api.listForums(),
        enabled: isConnected,
        staleTime: 30_000,
        refetchOnWindowFocus: false,
    });

    const structuredFolderById = useMemo(() => {
        return new Map((structuredFoldersResponse?.forums ?? []).map((forum) => [forum.id, forum]));
    }, [structuredFoldersResponse]);

    const activeStructuredFolder = useMemo(() => {
        if (activeFolderId === null) return null;
        if (folderIdSet.has(activeFolderId)) return null;
        return structuredFolderById.get(activeFolderId) ?? null;
    }, [activeFolderId, folderIdSet, structuredFolderById]);

    useEffect(() => {
        if (!activeStructuredTopic) return;
        if (activeFolderId !== activeStructuredTopic.forumId) {
            setActiveStructuredTopic(null);
        }
    }, [activeFolderId, activeStructuredTopic]);

    const isLegacyFolderView = activeFolderId === null || folderIdSet.has(activeFolderId);
    const isStructuredTopicView = !isLegacyFolderView
        && activeStructuredTopic !== null
        && activeStructuredTopic.forumId === activeFolderId;
    const isStructuredRootView = !isLegacyFolderView && !isStructuredTopicView;
    const activeTopicId = isStructuredTopicView ? activeStructuredTopic?.topicId ?? null : null;

    const {
        data: structuredTopics = [],
        isLoading: structuredTopicsLoading,
        error: structuredTopicsError,
    } = useQuery<api.ForumTopic[]>({
        queryKey: ['structured-folder-topics', activeStructuredFolder?.id],
        enabled: activeStructuredFolder !== null && isStructuredRootView,
        queryFn: async () => {
            if (!activeStructuredFolder) return [];

            const response = await api.listForumTopics(activeStructuredFolder.id);
            return response.topics;
        },
        staleTime: 15_000,
        refetchOnWindowFocus: false,
    });

    const structuredTopicFiles = useMemo<TelegramFile[]>(() => {
        return structuredTopics.map((topic) => ({
                id: topic.id,
                name: topic.title,
                size: 0,
                sizeStr: 'Structured subfolder',
                created_at: undefined,
                type: 'folder' as const,
                folder_id: activeStructuredFolder?.id ?? null,
            }));
    }, [structuredTopics, activeStructuredFolder?.id]);

    const structuredTopicTitleById = useMemo(
        () => new Map<number, string>(structuredTopics.map((topic) => [topic.id, topic.title])),
        [structuredTopics],
    );

    const structuredTopicTopMessageById = useMemo(
        () => new Map<number, number>(structuredTopics.map((topic) => [topic.id, topic.top_message])),
        [structuredTopics],
    );

    const activeTopicTopMessage = useMemo(() => {
        if (!isStructuredTopicView || !activeStructuredTopic) return null;

        if (activeStructuredTopic.topMessage && activeStructuredTopic.topMessage > 0) {
            return activeStructuredTopic.topMessage;
        }

        return structuredTopicTopMessageById.get(activeStructuredTopic.topicId) ?? null;
    }, [isStructuredTopicView, activeStructuredTopic, structuredTopicTopMessageById]);

    const { data: allFiles = [], isLoading: filesLoading, error: filesError } = useQuery<TelegramFile[]>({
        queryKey: ['files', activeFolderId, activeTopicId, activeTopicTopMessage],
        queryFn: async () => {
            if (isStructuredRootView) return [];

            const res = await api.listFiles(activeFolderId, activeTopicId, activeTopicTopMessage);
            return res.map((f) => ({
                ...f,
                sizeStr: formatBytes(f.size),
                type: (f.icon_type || (f.name.endsWith('/') ? 'folder' : 'file')) as 'file' | 'folder',
            }));
        },
    });

    const visibleBaseFiles = isStructuredRootView ? structuredTopicFiles : allFiles;

    const displayedFiles = searchTerm.length > 2 && isLegacyFolderView
        ? searchResults
        : visibleBaseFiles.filter((f: TelegramFile) => f.name.toLowerCase().includes(searchTerm.toLowerCase()));

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

    const { data: accountInfo } = useQuery({
        queryKey: ['account-info'],
        queryFn: () => api.getAccountInfo(),
        enabled: isWindowVisible,
        refetchInterval: isWindowVisible ? 30_000 : false,
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

    } = useFileOperations(activeFolderId, activeTopicId, selectedIds, setSelectedIds, displayedFiles);

    const { uploadQueue, setUploadQueue, cancelQueueItem, handleManualUpload, isDragging } = useFileUpload(
        activeFolderId,
        activeTopicId,
        activeTopicTopMessage,
        metrics?.max_file_size_bytes,
    );
    const { downloadQueue, clearFinished: clearDownloads } = useFileDownload();


    const handleSelectAll = useCallback(() => {
        if (isStructuredRootView) return;

        const ids = visibleOrderedIds.length > 0
            ? visibleOrderedIds
            : displayedFiles.map((f) => f.id);

        setSelectedIds(ids);
        setSelectionAnchorId(ids[0] ?? null);
        setSelectionFocusId(ids[ids.length - 1] ?? null);
    }, [visibleOrderedIds, displayedFiles, isStructuredRootView]);

    const handleKeyboardDelete = useCallback(() => {
        if (isStructuredRootView) {
            toast.info('Open a structured subfolder to manage files.');
            return;
        }

        if (selectedIds.length > 0) {
            handleBulkDelete();
        }
    }, [selectedIds, handleBulkDelete, isStructuredRootView]);

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
                    if (isStructuredRootView) {
                        if (activeStructuredFolder) {
                            openStructuredTopic(
                                activeStructuredFolder.id,
                                selected.id,
                                selected.name,
                                structuredTopicTopMessageById.get(selected.id) ?? null,
                            );
                        }
                    } else {
                        setActiveFolderId(selected.id);
                    }
                } else {
                    handlePreview(selected, displayedFiles);
                }
            }
        }
    }, [
        selectedIds,
        displayedFiles,
        setActiveFolderId,
        isStructuredRootView,
        activeStructuredFolder,
        structuredTopicTopMessageById,
        openStructuredTopic,
    ]);

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
    }, [activeFolderId, activeTopicId]);


    useEffect(() => {
        if (!isLegacyFolderView || searchTerm.length <= 2) {
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
    }, [searchTerm, isLegacyFolderView]);




    const handleFileClick = (e: React.MouseEvent, id: number, orderedIds: number[]) => {
        e.stopPropagation();

        if (isStructuredRootView && activeStructuredFolder) {
            const topicTitle = structuredTopicTitleById.get(id);
            if (topicTitle) {
                openStructuredTopic(
                    activeStructuredFolder.id,
                    id,
                    topicTitle,
                    structuredTopicTopMessageById.get(id) ?? null,
                );
                return;
            }
        }

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

    const handleDropOnTarget = useCallback(async (e: React.DragEvent, target: SidebarDropTarget) => {
        e.preventDefault();
        e.stopPropagation();

        const dataTransferFileId = e.dataTransfer.getData("application/x-telegram-file-id");
        const targetTopicId = target.topicId ?? null;

        if (activeFolderId === target.folderId && (activeTopicId ?? null) === targetTopicId) {
            return;
        }

        const fileId = internalDragRef.current || (dataTransferFileId ? parseInt(dataTransferFileId) : null);

        if (fileId) {
            try {
                const idsToMove = selectedIds.includes(fileId) ? selectedIds : [fileId];

                await api.moveFiles(idsToMove, activeFolderId, target.folderId, {
                    sourceTopicId: activeTopicId,
                    targetTopicId,
                    targetTopicTopMessage: target.topicTopMessage ?? null,
                });

                queryClient.invalidateQueries({ queryKey: ['files', activeFolderId, activeTopicId] });
                queryClient.invalidateQueries({ queryKey: ['files', target.folderId, targetTopicId] });

                if (selectedIds.includes(fileId)) setSelectedIds([]);

                if (target.label) {
                    toast.success(`Moved ${idsToMove.length} file(s) to ${target.label}.`);
                } else {
                    toast.success(`Moved ${idsToMove.length} file(s).`);
                }

                setInternalDragFileId(null);
            } catch {
                toast.error(`Failed to move file(s).`);
            }
        }
    }, [activeFolderId, activeTopicId, queryClient, selectedIds]);

    const handleDropOnLegacyFolder = useCallback((e: React.DragEvent, targetFolderId: number) => {
        void handleDropOnTarget(e, { folderId: targetFolderId });
    }, [handleDropOnTarget]);

    const notifyStructuredViewAction = useCallback(() => {
        toast.info('Open a structured subfolder to explore and upload files.');
    }, []);

    const handleManualUploadInView = useCallback(() => {
        if (isStructuredRootView) {
            notifyStructuredViewAction();
            return;
        }
        handleManualUpload();
    }, [isStructuredRootView, notifyStructuredViewAction, handleManualUpload]);

    const handleDeleteInView = useCallback((id: number) => {
        if (isStructuredRootView) {
            notifyStructuredViewAction();
            return;
        }
        void handleDelete(id);
    }, [isStructuredRootView, notifyStructuredViewAction, handleDelete]);

    const handleDownloadInView = useCallback((id: number, name: string) => {
        if (isStructuredRootView) {
            notifyStructuredViewAction();
            return;
        }
        void handleDownload(id, name);
    }, [isStructuredRootView, notifyStructuredViewAction, handleDownload]);

    const handlePreviewInView = useCallback((file: TelegramFile, orderedFiles?: TelegramFile[]) => {
        if (isStructuredRootView) {
            notifyStructuredViewAction();
            return;
        }
        handlePreview(file, orderedFiles);
    }, [isStructuredRootView, notifyStructuredViewAction, handlePreview]);

    const handleDownloadFolderInView = useCallback(() => {
        if (isStructuredRootView) {
            notifyStructuredViewAction();
            return;
        }
        void handleDownloadFolder();
    }, [isStructuredRootView, notifyStructuredViewAction, handleDownloadFolder]);

    const explorerLoading = isStructuredRootView
        ? structuredTopicsLoading
        : (filesLoading || (isLegacyFolderView && isSearching));

    const explorerError = isStructuredRootView
        ? (structuredTopicsError instanceof Error ? structuredTopicsError : null)
        : (filesError instanceof Error ? filesError : null);

    const showStructuredEmptyState = isStructuredRootView
        && !explorerLoading
        && !explorerError
        && displayedFiles.length === 0;

    const currentFolderName = activeFolderId === null
        ? "Saved Messages"
        : isStructuredTopicView && activeStructuredTopic
            ? `${activeStructuredFolder?.name || 'Structured folder'} / ${activeStructuredTopic.title}`
            : folders.find(f => f.id === activeFolderId)?.name || activeStructuredFolder?.name || "Folder";


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

            <ExternalDropBlocker onUploadClick={handleManualUploadInView} />

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
                activeStructuredTopicId={activeTopicId}
                setActiveFolderId={setActiveFolderId}
                onOpenStructuredTopic={openStructuredTopic}
                onDrop={handleDropOnTarget}
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
                    onDownloadFolder={handleDownloadFolderInView}
                    isDeleting={isDeleting}
                    deleteProgress={deleteProgress}
                    viewMode={viewMode}
                    setViewMode={setViewMode}
                    searchTerm={searchTerm}
                    onSearchChange={setSearchTerm}
                    accountTier={accountInfo?.tier ?? null}
                    disableFileActions={isStructuredRootView}
                />
                {searchTerm.length > 2 && isLegacyFolderView && (
                    <div className="px-6 pt-4 pb-0">
                        <h2 className="text-sm font-medium text-telegram-subtext">
                            Search Results for <span className="text-telegram-primary">"{searchTerm}"</span>
                        </h2>
                    </div>
                )}
                {showStructuredEmptyState ? (
                    <div className="flex-1 p-6 flex items-center justify-center text-telegram-subtext text-sm">
                        No subfolders yet in this structured folder.
                    </div>
                ) : (
                    <FileExplorer
                        files={displayedFiles}
                        loading={explorerLoading}
                        error={explorerError}
                        viewMode={viewMode}
                        selectedIds={selectedIds}
                        activeFolderId={activeFolderId}
                        onFileClick={handleFileClick}
                        onVisibleOrderChange={setVisibleOrderedIds}
                        onDelete={handleDeleteInView}
                        onDownload={handleDownloadInView}
                        onPreview={handlePreviewInView}
                        onManualUpload={handleManualUploadInView}
                        onSelectionClear={() => setSelectedIds([])}
                        onDrop={isStructuredRootView ? undefined : handleDropOnLegacyFolder}
                        onDragStart={(fileId) => setInternalDragFileId(fileId)}
                        onDragEnd={() => setTimeout(() => setInternalDragFileId(null), 50)}
                    />
                )}
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
