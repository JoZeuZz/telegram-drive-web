import { useEffect, useMemo, useState } from 'react';
import { Plus, HardDrive, Folder, ChevronRight } from 'lucide-react';
import { TelegramFolder } from '../../types';

interface MoveToFolderModalProps {
    folders: TelegramFolder[];
    onClose: () => void;
    onSelect: (id: number | null, onSuccess?: () => void) => void;
    activeFolderId: number | null;
}

export function MoveToFolderModal({ folders, onClose, onSelect, activeFolderId }: MoveToFolderModalProps) {
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

            for (const id of prev) {
                if (folderById.has(id)) {
                    next.add(id);
                }
            }

            // Keep root level expanded for discoverability.
            for (const root of rootFolders) {
                next.add(root.id);
            }

            // Auto-expand the active folder ancestry.
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
    }, [folderById, rootFolders, activeFolderId]);

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

    const renderFolderNode = (folder: TelegramFolder, depth: number): React.ReactNode => {
        const children = childrenByParent.get(folder.id) ?? [];
        const hasChildren = children.length > 0;
        const expanded = expandedIds.has(folder.id);
        const isCurrentFolder = folder.id === activeFolderId;

        return (
            <div key={folder.id} className="space-y-1">
                <button
                    onClick={() => {
                        if (isCurrentFolder) return;
                        onSelect(folder.id, onClose);
                    }}
                    disabled={isCurrentFolder}
                    className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm text-left transition-colors ${isCurrentFolder
                        ? 'text-telegram-subtext bg-telegram-hover/40 cursor-not-allowed'
                        : 'text-telegram-text hover:bg-telegram-hover'
                        }`}
                    style={{ paddingLeft: `${12 + depth * 16}px` }}
                    title={isCurrentFolder ? 'Current folder' : 'Move here'}
                >
                    <div
                        className="w-3 h-3 flex items-center justify-center shrink-0"
                        onClick={(e) => {
                            if (!hasChildren) return;
                            e.stopPropagation();
                            toggleExpanded(folder.id);
                        }}
                    >
                        {hasChildren && (
                            <ChevronRight className={`w-3 h-3 transition-transform ${expanded ? 'rotate-90' : ''}`} />
                        )}
                    </div>
                    <div className="w-7 h-7 rounded bg-telegram-hover flex items-center justify-center text-telegram-text shrink-0">
                        <Folder className="w-4 h-4" />
                    </div>
                    <span className="font-medium truncate flex-1">
                        {folder.name}
                        {isCurrentFolder ? ' (Current)' : ''}
                    </span>
                </button>

                {hasChildren && expanded && children.map((child) => renderFolderNode(child, depth + 1))}
            </div>
        );
    };

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 backdrop-blur-sm" onClick={onClose}>
            <div className="bg-telegram-surface border border-telegram-border rounded-xl w-80 shadow-2xl overflow-hidden flex flex-col max-h-[80vh]" onClick={e => e.stopPropagation()}>
                <div className="p-4 border-b border-telegram-border flex justify-between items-center">
                    <h3 className="text-telegram-text font-medium">Move to Folder</h3>
                    <button onClick={onClose} className="text-telegram-subtext hover:text-telegram-text"><Plus className="w-5 h-5 rotate-45" /></button>
                </div>
                <div className="flex-1 overflow-y-auto p-2 space-y-1">
                    {activeFolderId !== null && (
                        <button
                            onClick={() => onSelect(null, onClose)}
                            className="w-full flex items-center gap-3 px-3 py-3 rounded-lg text-sm text-left text-telegram-text hover:bg-telegram-hover transition-colors"
                        >
                            <div className="w-8 h-8 rounded bg-telegram-primary/20 flex items-center justify-center text-telegram-primary">
                                <HardDrive className="w-4 h-4" />
                            </div>
                            <span className="font-medium">Saved Messages</span>
                        </button>
                    )}

                    {rootFolders.map((folder) => renderFolderNode(folder, 0))}

                    {folders.length === 0 && activeFolderId === null && (
                        <div className="p-4 text-center text-xs text-telegram-subtext">No other folders available. Create one first!</div>
                    )}
                </div>
            </div>
        </div>
    )
}
