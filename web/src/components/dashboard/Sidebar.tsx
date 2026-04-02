import { useEffect, useMemo, useState } from 'react';
import { HardDrive, Folder, Plus, RefreshCw, LogOut } from 'lucide-react';
import { SidebarItem } from './SidebarItem';
import { BandwidthWidget } from './BandwidthWidget';
import { BandwidthStats, FolderSyncSummary, TelegramFolder } from '../../types';

interface SidebarProps {
    folders: TelegramFolder[];
    activeFolderId: number | null;
    setActiveFolderId: (id: number | null) => void;
    onDrop: (e: React.DragEvent, folderId: number | null) => void;
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
    folders, activeFolderId, setActiveFolderId, onDrop, onDelete, onCreate,
    isSyncing, isConnected, syncSummary, onSync, onLogout, bandwidth
}: SidebarProps) {
    const [createTargetParentId, setCreateTargetParentId] = useState<number | null | undefined>(undefined);
    const [newFolderName, setNewFolderName] = useState("");
    const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set());

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
                    onDrop={(e: React.DragEvent) => onDrop(e, folder.id)}
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
                    onDrop={(e: React.DragEvent) => onDrop(e, null)}
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
