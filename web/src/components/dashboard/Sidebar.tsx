import { useEffect, useMemo, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
    ChevronRight,
    Folder,
    FolderTree,
    HardDrive,
    Loader2,
    LogOut,
    Plus,
    RefreshCw,
    Trash2,
} from 'lucide-react';
import { toast } from 'sonner';
import * as api from '../../lib/api';
import { SidebarItem } from './SidebarItem';
import { BandwidthWidget } from './BandwidthWidget';
import { BandwidthStats, FolderSyncSummary, TelegramFolder } from '../../types';
import type { ForumTopic } from '../../lib/api';
import { useConfirm } from '../../context/ConfirmContext';

export interface SidebarDropTarget {
    folderId: number | null;
    topicId?: number | null;
    topicTopMessage?: number | null;
    label?: string;
}

interface SidebarProps {
    folders: TelegramFolder[];
    activeFolderId: number | null;
    activeStructuredTopicId?: number | null;
    setActiveFolderId: (id: number | null) => void;
    onOpenStructuredTopic?: (
        forumId: number,
        topicId: number,
        title: string,
        topMessage?: number | null,
    ) => void;
    onDrop: (e: React.DragEvent, target: SidebarDropTarget) => void;
    onDelete: (id: number, name: string) => void;
    onCreate: (name: string, parentId?: number | null) => Promise<void>;
    isSyncing: boolean;
    isConnected: boolean;
    syncSummary: FolderSyncSummary | null;
    onSync: () => void;
    onLogout: () => void;
    bandwidth: BandwidthStats | null;
}

export function Sidebar({
    folders, activeFolderId, activeStructuredTopicId = null, setActiveFolderId, onOpenStructuredTopic,
    onDrop, onDelete, onCreate,
    isSyncing, isConnected, syncSummary, onSync, onLogout, bandwidth
}: SidebarProps) {
    const queryClient = useQueryClient();
    const { confirm } = useConfirm();
    const [createTargetParentId, setCreateTargetParentId] = useState<number | null | undefined>(undefined);
    const [newFolderName, setNewFolderName] = useState("");
    const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set());
    const [expandedStructuredIds, setExpandedStructuredIds] = useState<Set<number>>(new Set());
    const [structuredSubfoldersById, setStructuredSubfoldersById] = useState<Record<number, ForumTopic[]>>({});
    const [structuredSubfoldersLoadingIds, setStructuredSubfoldersLoadingIds] = useState<Set<number>>(new Set());
    const [structuredSubfoldersErrors, setStructuredSubfoldersErrors] = useState<Record<number, string>>({});
    const [showStructuredCreateInput, setShowStructuredCreateInput] = useState(false);
    const [newStructuredFolderName, setNewStructuredFolderName] = useState('');
    const [createStructuredSubfolderTargetId, setCreateStructuredSubfolderTargetId] = useState<number | null>(null);
    const [newStructuredSubfolderName, setNewStructuredSubfolderName] = useState('');
    const [dragDropTargetKey, setDragDropTargetKey] = useState<string | null>(null);

    const hasInternalDragPayload = (e: React.DragEvent) =>
        Array.from(e.dataTransfer.types).includes('application/x-telegram-file-id');

    const handleDragEnterTarget = (e: React.DragEvent, key: string) => {
        if (!hasInternalDragPayload(e)) return;
        e.preventDefault();
        e.stopPropagation();
        setDragDropTargetKey(key);
    };

    const handleDragOverTarget = (e: React.DragEvent, key: string) => {
        if (!hasInternalDragPayload(e)) return;
        e.preventDefault();
        e.stopPropagation();
        e.dataTransfer.dropEffect = 'move';
        if (dragDropTargetKey !== key) {
            setDragDropTargetKey(key);
        }
    };

    const handleDragLeaveTarget = (e: React.DragEvent, key: string) => {
        if (!hasInternalDragPayload(e)) return;
        e.preventDefault();
        e.stopPropagation();
        if (dragDropTargetKey === key) {
            setDragDropTargetKey(null);
        }
    };

    const handleDropTarget = (e: React.DragEvent, target: SidebarDropTarget) => {
        e.preventDefault();
        e.stopPropagation();
        setDragDropTargetKey(null);
        onDrop(e, target);
    };

    const {
        data: structuredFoldersResponse,
        isLoading: structuredFoldersLoading,
        error: structuredFoldersError,
        refetch: refetchStructuredFolders,
    } = useQuery({
        queryKey: ['structured-folders'],
        queryFn: () => api.listForums(),
        enabled: isConnected,
        staleTime: 30_000,
        refetchOnWindowFocus: false,
    });

    const structuredFolders = structuredFoldersResponse?.forums || [];

    const describeError = (error: unknown, fallback: string) => {
        if (error instanceof api.ApiError) return error.message;
        if (error instanceof Error) return error.message;
        return fallback;
    };

    const structuredFoldersDisabled = useMemo(() => {
        if (!(structuredFoldersError instanceof api.ApiError)) return false;
        return structuredFoldersError.status === 400
            && structuredFoldersError.message.toLowerCase().includes('disabled');
    }, [structuredFoldersError]);

    const loadStructuredSubfolders = async (structuredFolderId: number, force = false) => {
        if (!force && structuredSubfoldersById[structuredFolderId]) return;

        setStructuredSubfoldersLoadingIds((prev) => {
            const next = new Set(prev);
            next.add(structuredFolderId);
            return next;
        });

        try {
            const response = await api.listForumTopics(structuredFolderId);
            setStructuredSubfoldersById((prev) => ({
                ...prev,
                [structuredFolderId]: response.topics,
            }));
            setStructuredSubfoldersErrors((prev) => {
                const next = { ...prev };
                delete next[structuredFolderId];
                return next;
            });
        } catch (error) {
            setStructuredSubfoldersErrors((prev) => ({
                ...prev,
                [structuredFolderId]: describeError(error, 'Could not load subfolders.'),
            }));
        } finally {
            setStructuredSubfoldersLoadingIds((prev) => {
                const next = new Set(prev);
                next.delete(structuredFolderId);
                return next;
            });
        }
    };

    const toggleStructuredExpanded = (structuredFolderId: number) => {
        let shouldLoad = false;

        setExpandedStructuredIds((prev) => {
            const next = new Set(prev);
            if (next.has(structuredFolderId)) {
                next.delete(structuredFolderId);
            } else {
                next.add(structuredFolderId);
                shouldLoad = true;
            }
            return next;
        });

        if (shouldLoad) {
            void loadStructuredSubfolders(structuredFolderId);
        }
    };

    const submitCreateStructuredFolder = async () => {
        const name = newStructuredFolderName.trim();
        if (!name) return;

        try {
            await api.createForum(name);
            setNewStructuredFolderName('');
            setShowStructuredCreateInput(false);
            await refetchStructuredFolders();
            toast.success('Structured folder created.');
        } catch (error) {
            toast.error(describeError(error, 'Could not create structured folder.'));
        }
    };

    const openCreateStructuredSubfolder = (structuredFolderId: number) => {
        setCreateStructuredSubfolderTargetId(structuredFolderId);
        setNewStructuredSubfolderName('');
        if (!expandedStructuredIds.has(structuredFolderId)) {
            setExpandedStructuredIds((prev) => {
                const next = new Set(prev);
                next.add(structuredFolderId);
                return next;
            });
            void loadStructuredSubfolders(structuredFolderId);
        }
    };

    const submitCreateStructuredSubfolder = async (structuredFolderId: number) => {
        const name = newStructuredSubfolderName.trim();
        if (!name) return;

        try {
            await api.createForumTopic(structuredFolderId, name);
            setCreateStructuredSubfolderTargetId(null);
            setNewStructuredSubfolderName('');
            await loadStructuredSubfolders(structuredFolderId, true);
            toast.success('Subfolder created.');
        } catch (error) {
            toast.error(describeError(error, 'Could not create subfolder.'));
        }
    };

    const deleteStructuredFolder = async (structuredFolderId: number, structuredFolderName: string) => {
        if (!await confirm({
            title: 'Delete Structured Folder',
            message: `Are you sure you want to delete "${structuredFolderName}"?\nAll topic history inside this structured folder will be removed on Telegram.`,
            confirmText: 'Delete',
            variant: 'danger',
        })) {
            return;
        }

        try {
            await api.deleteForum(structuredFolderId);

            setExpandedStructuredIds((prev) => {
                const next = new Set(prev);
                next.delete(structuredFolderId);
                return next;
            });

            setStructuredSubfoldersById((prev) => {
                const next = { ...prev };
                delete next[structuredFolderId];
                return next;
            });

            setStructuredSubfoldersErrors((prev) => {
                const next = { ...prev };
                delete next[structuredFolderId];
                return next;
            });

            if (activeFolderId === structuredFolderId) {
                setActiveFolderId(null);
            }

            await refetchStructuredFolders();
            await queryClient.invalidateQueries({ queryKey: ['structured-folder-topics', structuredFolderId] });
            await queryClient.invalidateQueries({ queryKey: ['files', structuredFolderId] });

            toast.success('Structured folder deleted.');
        } catch (error) {
            toast.error(describeError(error, 'Could not delete structured folder.'));
        }
    };

    const deleteStructuredSubfolder = async (structuredFolderId: number, subfolder: ForumTopic) => {
        if (!await confirm({
            title: 'Delete Subfolder',
            message: `Are you sure you want to delete "${subfolder.title}"?\nThis removes the topic history on Telegram.`,
            confirmText: 'Delete',
            variant: 'danger',
        })) {
            return;
        }

        try {
            await api.deleteForumTopic(structuredFolderId, subfolder.id, subfolder.top_message);

            setStructuredSubfoldersById((prev) => ({
                ...prev,
                [structuredFolderId]: (prev[structuredFolderId] || []).filter((topic) => topic.id !== subfolder.id),
            }));

            if (activeFolderId === structuredFolderId && activeStructuredTopicId === subfolder.id) {
                setActiveFolderId(structuredFolderId);
            }

            await queryClient.invalidateQueries({ queryKey: ['structured-folder-topics', structuredFolderId] });
            await queryClient.invalidateQueries({ queryKey: ['files', structuredFolderId, subfolder.id] });

            toast.success('Subfolder deleted.');
        } catch (error) {
            toast.error(describeError(error, 'Could not delete subfolder.'));
        }
    };

    const folderById = useMemo(() => {
        return new Map<number, TelegramFolder>(folders.map((folder) => [folder.id, folder]));
    }, [folders]);

    const childrenByParent = useMemo(() => {
        const map = new Map<number | null, TelegramFolder[]>();

        for (const folder of folders) {
            const rawParentId = folder.parent_id ?? null;
            const parentId = rawParentId !== null && rawParentId !== folder.id && folderById.has(rawParentId)
                ? rawParentId
                : null;

            const siblings = map.get(parentId) ?? [];
            siblings.push(folder);
            map.set(parentId, siblings);
        }

        for (const [, nodes] of map) {
            nodes.sort((a, b) => a.name.localeCompare(b.name));
        }

        return map;
    }, [folders, folderById]);

    const rootFolders = childrenByParent.get(null) ?? [];

    useEffect(() => {
        setExpandedIds((prev) => {
            const next = new Set<number>();
            const currentIds = new Set(folders.map((folder) => folder.id));

            for (const id of prev) {
                if (currentIds.has(id)) {
                    next.add(id);
                }
            }

            let cursor = activeFolderId;
            const guard = new Set<number>();
            while (cursor !== null && !guard.has(cursor)) {
                guard.add(cursor);

                const current = folderById.get(cursor);
                if (!current) break;

                const parentId = current.parent_id ?? null;
                if (parentId !== null && parentId !== current.id && folderById.has(parentId)) {
                    next.add(parentId);
                    cursor = parentId;
                } else {
                    break;
                }
            }

            return next;
        });
    }, [folders, activeFolderId, folderById]);

    useEffect(() => {
        if (createTargetParentId === undefined || createTargetParentId === null) return;
        if (!folderById.has(createTargetParentId)) {
            setCreateTargetParentId(undefined);
            setNewFolderName('');
        }
    }, [createTargetParentId, folderById]);

    const openCreateInput = (parentId: number | null) => {
        setCreateTargetParentId(parentId);
        setNewFolderName('');

        if (parentId !== null) {
            setExpandedIds((prev) => {
                const next = new Set(prev);
                next.add(parentId);
                return next;
            });
        }
    };

    const toggleExpanded = (folderId: number) => {
        setExpandedIds((prev) => {
            const next = new Set(prev);
            if (next.has(folderId)) {
                next.delete(folderId);
            } else {
                next.add(folderId);
            }
            return next;
        });
    };

    const submitCreate = async () => {
        if (!newFolderName.trim()) return;

        const parentId = createTargetParentId === undefined ? null : createTargetParentId;
        try {
            await onCreate(newFolderName.trim(), parentId);
            setNewFolderName('');
            setCreateTargetParentId(undefined);
        } catch {
            // handled by parent
        }
    };

    const renderCreateInput = (parentId: number | null, depth: number) => {
        if (createTargetParentId !== parentId) return null;

        return (
            <div className="px-3 py-1" style={{ paddingLeft: `${12 + depth * 16}px` }}>
                <input
                    autoFocus
                    type="text"
                    className="w-full bg-white/10 rounded px-2 py-1 text-sm text-white focus:outline-none focus:ring-1 focus:ring-telegram-primary"
                    placeholder={parentId === null ? 'Folder Name' : 'Subfolder Name'}
                    value={newFolderName}
                    onChange={e => setNewFolderName(e.target.value)}
                    onKeyDown={e => {
                        if (e.key === 'Enter') submitCreate();
                        if (e.key === 'Escape') {
                            setCreateTargetParentId(undefined);
                            setNewFolderName('');
                        }
                    }}
                    onBlur={() => {
                        if (!newFolderName.trim()) {
                            setCreateTargetParentId(undefined);
                        }
                    }}
                />
            </div>
        );
    };

    const renderTree = (folder: TelegramFolder, depth: number): React.ReactNode => {
        const children = childrenByParent.get(folder.id) ?? [];
        const hasChildren = children.length > 0;
        const expanded = expandedIds.has(folder.id);

        return (
            <div key={folder.id} className="space-y-1">
                <SidebarItem
                    icon={Folder}
                    label={folder.name}
                    active={activeFolderId === folder.id}
                    depth={depth}
                    hasChildren={hasChildren}
                    expanded={expanded}
                    onToggleExpand={hasChildren ? () => toggleExpanded(folder.id) : undefined}
                    onClick={() => setActiveFolderId(folder.id)}
                    onDrop={(e: React.DragEvent) => handleDropTarget(e, {
                        folderId: folder.id,
                        label: folder.name,
                    })}
                    onDelete={() => onDelete(folder.id, folder.name)}
                    onCreateChild={() => openCreateInput(folder.id)}
                    folderId={folder.id}
                />

                {renderCreateInput(folder.id, depth + 1)}

                {hasChildren && expanded && children.map((child) => renderTree(child, depth + 1))}
            </div>
        );
    };

    return (
        <aside className="w-64 bg-telegram-surface border-r border-telegram-border flex flex-col" onClick={e => e.stopPropagation()}>
            <div className="p-4 flex items-center gap-2">
                <img src="/logo.svg" className="w-8 h-8 drop-shadow-lg" alt="Logo" />
                <span className="font-bold text-lg text-telegram-text tracking-tight">Telegram Drive</span>
            </div>

            <nav className="flex-1 px-2 py-4 space-y-1">
                <SidebarItem
                    icon={HardDrive}
                    label="Saved Messages"
                    active={activeFolderId === null}
                    onClick={() => setActiveFolderId(null)}
                    onDrop={(e: React.DragEvent) => handleDropTarget(e, {
                        folderId: null,
                        label: 'Saved Messages',
                    })}
                    folderId={null}
                />

                {renderCreateInput(null, 0)}

                {createTargetParentId !== null && (
                    <button
                        onClick={() => openCreateInput(null)}
                        className="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium text-telegram-subtext hover:bg-telegram-hover hover:text-telegram-text transition-colors border border-dashed border-telegram-border mt-2"
                    >
                        <Plus className="w-4 h-4" />
                        Create Folder
                    </button>
                )}

                <div className="space-y-1 mt-2">
                    {rootFolders.map((folder) => renderTree(folder, 0))}
                </div>

                <div className="mt-4 border-t border-telegram-border/70 pt-3">
                    <div className="mb-2 flex items-center justify-between px-2">
                        <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-wide text-telegram-subtext">
                            <FolderTree className="h-3.5 w-3.5" />
                            <span>Structured Folders</span>
                        </div>
                        <button
                            onClick={() => {
                                setShowStructuredCreateInput((prev) => !prev);
                                setNewStructuredFolderName('');
                            }}
                            className="rounded p-1 text-telegram-subtext transition-colors hover:bg-telegram-hover hover:text-telegram-text"
                            title="Create structured folder"
                        >
                            <Plus className="h-3.5 w-3.5" />
                        </button>
                    </div>

                    {showStructuredCreateInput && (
                        <div className="mb-2 px-2">
                            <input
                                autoFocus
                                type="text"
                                className="w-full rounded bg-white/10 px-2 py-1 text-xs text-white focus:outline-none focus:ring-1 focus:ring-telegram-primary"
                                placeholder="Structured folder name"
                                value={newStructuredFolderName}
                                onChange={(e) => setNewStructuredFolderName(e.target.value)}
                                onKeyDown={(e) => {
                                    if (e.key === 'Enter') void submitCreateStructuredFolder();
                                    if (e.key === 'Escape') {
                                        setShowStructuredCreateInput(false);
                                        setNewStructuredFolderName('');
                                    }
                                }}
                                onBlur={() => {
                                    if (!newStructuredFolderName.trim()) {
                                        setShowStructuredCreateInput(false);
                                    }
                                }}
                            />
                        </div>
                    )}

                    {!isConnected && (
                        <div className="px-2 text-[11px] text-telegram-subtext">
                            Connect Telegram to load structured folders.
                        </div>
                    )}

                    {isConnected && structuredFoldersLoading && (
                        <div className="flex items-center gap-2 px-2 text-[11px] text-telegram-subtext">
                            <Loader2 className="h-3.5 w-3.5 animate-spin" />
                            <span>Loading structured folders...</span>
                        </div>
                    )}

                    {isConnected && structuredFoldersDisabled && (
                        <div className="px-2 text-[11px] text-telegram-subtext">
                            Structured folders are disabled on the server.
                        </div>
                    )}

                    {isConnected && !structuredFoldersDisabled && structuredFoldersError && (
                        <div className="px-2 text-[11px] text-red-400">
                            {describeError(structuredFoldersError, 'Could not load structured folders.')}
                        </div>
                    )}

                    {isConnected && !structuredFoldersDisabled && !structuredFoldersLoading && structuredFolders.length === 0 && (
                        <div className="px-2 text-[11px] text-telegram-subtext">
                            No structured folders yet.
                        </div>
                    )}

                    <div className="space-y-1 px-1">
                        {structuredFolders.map((structuredFolder) => {
                            const expanded = expandedStructuredIds.has(structuredFolder.id);
                            const subfolders = structuredSubfoldersById[structuredFolder.id] || [];
                            const subfoldersLoading = structuredSubfoldersLoadingIds.has(structuredFolder.id);
                            const subfoldersError = structuredSubfoldersErrors[structuredFolder.id];
                            const rootDropKey = `structured-root:${structuredFolder.id}`;

                            return (
                                <div key={structuredFolder.id} className="rounded-lg">
                                    <div className="group flex items-center gap-1 px-1 py-1 hover:bg-telegram-hover/80">
                                        <button
                                            onClick={() => toggleStructuredExpanded(structuredFolder.id)}
                                            className="rounded p-0.5 text-telegram-subtext hover:text-telegram-text"
                                            title={expanded ? 'Collapse subfolders' : 'Expand subfolders'}
                                        >
                                            <ChevronRight className={`h-3.5 w-3.5 transition-transform ${expanded ? 'rotate-90' : ''}`} />
                                        </button>

                                        <button
                                            onClick={() => setActiveFolderId(structuredFolder.id)}
                                            onDragEnter={(e) => handleDragEnterTarget(e, rootDropKey)}
                                            onDragOver={(e) => handleDragOverTarget(e, rootDropKey)}
                                            onDragLeave={(e) => handleDragLeaveTarget(e, rootDropKey)}
                                            onDrop={(e) => handleDropTarget(e, {
                                                folderId: structuredFolder.id,
                                                label: structuredFolder.name,
                                            })}
                                            className={`flex min-w-0 flex-1 items-center gap-2 rounded px-1 py-0.5 text-left ${
                                                dragDropTargetKey === rootDropKey
                                                    ? 'bg-telegram-primary/25 ring-1 ring-telegram-primary'
                                                    : ''
                                            } ${
                                                activeFolderId === structuredFolder.id && activeStructuredTopicId === null
                                                    ? 'bg-telegram-primary/15 text-telegram-text'
                                                    : ''
                                            }`}
                                            title="Open root folder"
                                        >
                                            <Folder className="h-3.5 w-3.5 text-cyan-300" />
                                            <span className="truncate text-xs text-telegram-text">{structuredFolder.name}</span>
                                        </button>

                                        <button
                                            onClick={() => openCreateStructuredSubfolder(structuredFolder.id)}
                                            className="rounded p-0.5 text-telegram-subtext opacity-0 transition group-hover:opacity-100 hover:bg-telegram-hover hover:text-telegram-text"
                                            title="Create subfolder"
                                        >
                                            <Plus className="h-3.5 w-3.5" />
                                        </button>

                                        <button
                                            onClick={() => {
                                                void deleteStructuredFolder(structuredFolder.id, structuredFolder.name);
                                            }}
                                            className="rounded p-0.5 text-telegram-subtext opacity-0 transition group-hover:opacity-100 hover:bg-telegram-hover hover:text-red-300"
                                            title="Delete structured folder"
                                        >
                                            <Trash2 className="h-3.5 w-3.5" />
                                        </button>
                                    </div>

                                    {createStructuredSubfolderTargetId === structuredFolder.id && (
                                        <div className="px-5 pb-1">
                                            <input
                                                autoFocus
                                                type="text"
                                                className="w-full rounded bg-white/10 px-2 py-1 text-xs text-white focus:outline-none focus:ring-1 focus:ring-telegram-primary"
                                                placeholder="Subfolder name"
                                                value={newStructuredSubfolderName}
                                                onChange={(e) => setNewStructuredSubfolderName(e.target.value)}
                                                onKeyDown={(e) => {
                                                    if (e.key === 'Enter') {
                                                        void submitCreateStructuredSubfolder(structuredFolder.id);
                                                    }
                                                    if (e.key === 'Escape') {
                                                        setCreateStructuredSubfolderTargetId(null);
                                                        setNewStructuredSubfolderName('');
                                                    }
                                                }}
                                                onBlur={() => {
                                                    if (!newStructuredSubfolderName.trim()) {
                                                        setCreateStructuredSubfolderTargetId(null);
                                                    }
                                                }}
                                            />
                                        </div>
                                    )}

                                    {expanded && (
                                        <div className="space-y-1 pb-1 pl-7 pr-2">
                                            {subfoldersLoading && (
                                                <div className="flex items-center gap-1.5 text-[11px] text-telegram-subtext">
                                                    <Loader2 className="h-3 w-3 animate-spin" />
                                                    <span>Loading subfolders...</span>
                                                </div>
                                            )}

                                            {!subfoldersLoading && subfoldersError && (
                                                <button
                                                    onClick={() => void loadStructuredSubfolders(structuredFolder.id, true)}
                                                    className="text-left text-[11px] text-red-400 hover:text-red-300"
                                                >
                                                    {subfoldersError}. Retry.
                                                </button>
                                            )}

                                            {!subfoldersLoading && !subfoldersError && subfolders.length === 0 && (
                                                <div className="text-[11px] text-telegram-subtext">
                                                    No subfolders yet.
                                                </div>
                                            )}

                                            {!subfoldersLoading && !subfoldersError && subfolders.map((subfolder) => {
                                                const active = activeFolderId === structuredFolder.id
                                                    && activeStructuredTopicId === subfolder.id;
                                                const topicDropKey = `structured-topic:${structuredFolder.id}:${subfolder.id}`;

                                                return (
                                                    <div key={subfolder.id} className="group flex items-center gap-1">
                                                        <button
                                                            onClick={() => {
                                                                if (onOpenStructuredTopic) {
                                                                    onOpenStructuredTopic(
                                                                        structuredFolder.id,
                                                                        subfolder.id,
                                                                        subfolder.title,
                                                                        subfolder.top_message,
                                                                    );
                                                                } else {
                                                                    setActiveFolderId(structuredFolder.id);
                                                                }
                                                            }}
                                                            onDragEnter={(e) => handleDragEnterTarget(e, topicDropKey)}
                                                            onDragOver={(e) => handleDragOverTarget(e, topicDropKey)}
                                                            onDragLeave={(e) => handleDragLeaveTarget(e, topicDropKey)}
                                                            onDrop={(e) => handleDropTarget(e, {
                                                                folderId: structuredFolder.id,
                                                                topicId: subfolder.id,
                                                                topicTopMessage: subfolder.top_message,
                                                                label: `${structuredFolder.name} / ${subfolder.title}`,
                                                            })}
                                                            className={`flex min-w-0 flex-1 items-center gap-1.5 rounded px-1 py-0.5 text-left text-[11px] ${
                                                                dragDropTargetKey === topicDropKey
                                                                    ? 'bg-telegram-primary/25 ring-1 ring-telegram-primary text-telegram-text'
                                                                    : ''
                                                            } ${
                                                                active
                                                                    ? 'bg-telegram-primary/15 text-telegram-text'
                                                                    : 'text-telegram-subtext hover:bg-telegram-hover/70'
                                                            }`}
                                                            title={subfolder.title}
                                                        >
                                                            <Folder className="h-3 w-3 shrink-0 text-telegram-subtext/80" />
                                                            <span className="truncate">{subfolder.title}</span>
                                                        </button>

                                                        <button
                                                            onClick={() => {
                                                                void deleteStructuredSubfolder(structuredFolder.id, subfolder);
                                                            }}
                                                            className="rounded p-0.5 text-telegram-subtext opacity-0 transition group-hover:opacity-100 hover:bg-telegram-hover hover:text-red-300"
                                                            title="Delete subfolder"
                                                        >
                                                            <Trash2 className="h-3 w-3" />
                                                        </button>
                                                    </div>
                                                );
                                            })}
                                        </div>
                                    )}
                                </div>
                            );
                        })}
                    </div>
                </div>
            </nav>

            <div className="p-4 border-t border-telegram-border">
                <div className="flex items-center gap-2 text-telegram-subtext text-xs">
                    <div className={`w-2 h-2 rounded-full ${isConnected ? 'bg-green-500 animate-pulse' : 'bg-red-500'}`}></div>
                    <span>{isConnected ? 'Connected to Telegram' : 'Disconnected from Telegram'}</span>
                </div>

                <div className="flex gap-2 mt-4">
                    <button
                        onClick={onSync}
                        disabled={isSyncing}
                        className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs font-medium text-blue-500 hover:text-blue-600 bg-blue-500/10 hover:bg-blue-500/20 rounded-lg transition-colors ${isSyncing ? 'opacity-50 cursor-not-allowed' : ''}`}
                        title="Scan for existing folders"
                    >
                        <RefreshCw className={`w-3 h-3 ${isSyncing ? 'animate-spin' : ''}`} />
                        {isSyncing ? 'Syncing...' : 'Sync'}
                    </button>
                    <button
                        onClick={onLogout}
                        className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs font-medium text-red-500 hover:text-red-600 bg-red-500/10 hover:bg-red-500/20 rounded-lg transition-colors"
                        title="Sign Out"
                    >
                        <LogOut className="w-3 h-3" />
                        Logout
                    </button>
                </div>

                {syncSummary && (
                    <div className="mt-3 text-[11px] leading-4">
                        <div className="text-telegram-subtext">
                            Sync: title {syncSummary.resolved_by_title}, fallback {syncSummary.resolved_by_about}, migrated {syncSummary.migrated}
                        </div>
                        {syncSummary.orphans > 0 && (
                            <div className="text-amber-400 mt-1">
                                Warning: {syncSummary.orphans} orphan folder(s) rendered at root.
                            </div>
                        )}
                    </div>
                )}

                {bandwidth && <BandwidthWidget bandwidth={bandwidth} />}
            </div>

        </aside>
    )
}
