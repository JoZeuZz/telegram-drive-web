import { useState, useEffect } from 'react';
import * as api from '../lib/api';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { useConfirm } from '../context/ConfirmContext';
import { FolderSyncSummary, TelegramFolder } from '../types';
import { useNetworkStatus } from './useNetworkStatus';

/** Read a JSON value from localStorage */
function lsGet<T>(key: string): T | null {
    try {
        const v = localStorage.getItem(key);
        return v ? (JSON.parse(v) as T) : null;
    } catch { return null; }
}

/** Write a JSON value to localStorage */
function lsSet(key: string, value: unknown) {
    localStorage.setItem(key, JSON.stringify(value));
}

function normalizeFolders(list: TelegramFolder[]): TelegramFolder[] {
    return list.map((folder) => ({
        ...folder,
        parent_id: folder.parent_id ?? null,
    }));
}

function buildChildrenMap(folders: TelegramFolder[]): Map<number, number[]> {
    const map = new Map<number, number[]>();

    for (const folder of folders) {
        const parentId = folder.parent_id ?? null;
        if (parentId === null || parentId === folder.id) continue;

        const children = map.get(parentId) ?? [];
        children.push(folder.id);
        map.set(parentId, children);
    }

    return map;
}

function collectBranchIds(folders: TelegramFolder[], rootId: number): Set<number> {
    const childrenMap = buildChildrenMap(folders);
    const branchIds = new Set<number>();
    const stack = [rootId];

    while (stack.length > 0) {
        const currentId = stack.pop();
        if (currentId === undefined || branchIds.has(currentId)) continue;

        branchIds.add(currentId);

        const children = childrenMap.get(currentId) ?? [];
        for (const childId of children) {
            stack.push(childId);
        }
    }

    return branchIds;
}

export function useTelegramConnection(onLogoutParent: () => void) {
    const queryClient = useQueryClient();
    const { confirm } = useConfirm();

    const [ready, setReady] = useState(false);
    const [folders, setFolders] = useState<TelegramFolder[]>([]);
    const [activeFolderId, setActiveFolderId] = useState<number | null>(null);
    const [isSyncing, setIsSyncing] = useState(false);
    const [isConnected, setIsConnected] = useState(true);
    const [lastSyncSummary, setLastSyncSummary] = useState<FolderSyncSummary | null>(null);


    const networkIsOnline = useNetworkStatus();

    // Initialise from localStorage + check Telegram status
    useEffect(() => {
        const init = async () => {
            const savedFolders = lsGet<TelegramFolder[]>('folders');
            if (savedFolders) setFolders(normalizeFolders(savedFolders));

            const savedActive = lsGet<number | null>('activeFolderId');
            if (savedActive !== null && savedActive !== undefined) setActiveFolderId(savedActive);

            try {
                const st = await api.telegramStatus();
                setIsConnected(st.connected);
                if (st.connected) {
                    queryClient.invalidateQueries({ queryKey: ['files'] });
                }
            } catch {
                setIsConnected(false);
            }
            setReady(true);
        };
        init();
    }, [queryClient]);


    useEffect(() => {
        setIsConnected(networkIsOnline);
    }, [networkIsOnline]);


    const isNetworkError = (error: string): boolean => {
        const keywords = ['timeout', 'connection', 'network', 'socket', 'disconnected', 'EOF', 'ECONNREFUSED', 'overflow'];
        return keywords.some(k => error.toLowerCase().includes(k.toLowerCase()));
    };

    const forceLogout = async () => {
        setIsConnected(false);
        try {
            await api.telegramLogout().catch(() => { });
            localStorage.removeItem('api_id');
            sessionStorage.removeItem('api_hash');
            localStorage.removeItem('folders');
        } catch {
            // best effort cleanup
        }
        toast.error("Connection lost. Please log in again.");
        onLogoutParent();
    };


    const handleLogout = async () => {
        if (!await confirm({ title: "Sign Out", message: "Are you sure you want to sign out? This will disconnect your active session.", confirmText: "Sign Out", variant: 'danger' })) return;

        try {
            await api.telegramLogout();
            localStorage.removeItem('api_id');
            sessionStorage.removeItem('api_hash');
            localStorage.removeItem('folders');
            onLogoutParent();
        } catch {
            toast.error("Error signing out");
            onLogoutParent();
        }
    };

    const handleSyncFolders = async () => {
        setIsSyncing(true);
        try {
            const syncResult = await api.syncFolders();
            const foundFolders = normalizeFolders(syncResult.folders);
            const oldById = new Map(folders.map((f) => [f.id, f]));

            let added = 0;
            let updated = 0;
            let removed = 0;

            for (const folder of foundFolders) {
                const existing = oldById.get(folder.id);
                if (!existing) {
                    added++;
                    continue;
                }

                if (existing.name !== folder.name || (existing.parent_id ?? null) !== (folder.parent_id ?? null)) {
                    updated++;
                }
            }

            for (const oldFolder of folders) {
                if (!foundFolders.some((f) => f.id === oldFolder.id)) {
                    removed++;
                }
            }

            setFolders(foundFolders);
            lsSet('folders', foundFolders);
            setLastSyncSummary(syncResult.summary);

            if (activeFolderId !== null && !foundFolders.some((f) => f.id === activeFolderId)) {
                setActiveFolderId(null);
                lsSet('activeFolderId', null);
            }

            const detail = `title:${syncResult.summary.resolved_by_title} fallback:${syncResult.summary.resolved_by_about} migrated:${syncResult.summary.migrated}`;

            if (added > 0 || updated > 0 || removed > 0) {
                toast.success(`Sync complete. +${added} new, ~${updated} updated, -${removed} removed (${detail}).`);
            } else {
                toast.info(`Sync complete. No folder changes found (${detail}).`);
            }

            if (syncResult.summary.orphans > 0) {
                toast.warning(`Sync detected ${syncResult.summary.orphans} orphan folder(s). They are shown at root until parent metadata is restored.`);
            }
        } catch {
            toast.error("Sync failed");
        } finally {
            setIsSyncing(false);
        }
    };

    const handleCreateFolder = async (name: string, parentId: number | null = null) => {
        try {
            const created = await api.createFolder(name, parentId);
            const newFolder = { ...created, parent_id: created.parent_id ?? null };
            const exists = folders.some((folder) => folder.id === newFolder.id);
            const updated = exists
                ? folders.map((folder) => (folder.id === newFolder.id ? newFolder : folder))
                : [...folders, newFolder];

            setFolders(updated);
            lsSet('folders', updated);

            if (parentId === null) {
                toast.success(`Folder "${name}" created.`);
            } else {
                toast.success(`Subfolder "${name}" created.`);
            }
        } catch (e) {
            toast.error("Failed to create folder: " + e);
            throw e;
        }
    };

    const handleFolderDelete = async (folderId: number, folderName: string) => {
        const branchIds = collectBranchIds(folders, folderId);
        const descendants = Math.max(0, branchIds.size - 1);

        if (!await confirm({
            title: "Delete Folder",
            message: descendants > 0
                ? `Are you sure you want to delete "${folderName}"?\nThis will delete this folder and ${descendants} subfolder(s) on Telegram.`
                : `Are you sure you want to delete "${folderName}"?\nThis will delete the channel on Telegram.`,
            confirmText: "Delete",
            variant: 'danger'
        })) return;

        try {
            const res = await api.deleteFolder(folderId);

            let updated: TelegramFolder[] = [];
            try {
                updated = normalizeFolders(await api.listFolders());
            } catch {
                updated = folders.filter((folder) => !branchIds.has(folder.id));
            }

            setFolders(updated);
            lsSet('folders', updated);
            if (activeFolderId !== null && !updated.some((f) => f.id === activeFolderId)) {
                setActiveFolderId(null);
                lsSet('activeFolderId', null);
            }

            const deletedCount = Math.max(1, res.deleted_count ?? 1);
            toast.success(`Deleted ${deletedCount} folder(s), including "${folderName}".`);
        } catch (e: unknown) {
            const errStr = String(e);
            if (errStr.includes("not found")) {
                if (await confirm({
                    title: "Folder Not Found",
                    message: `Folder "${folderName}" not found on Telegram (it may have been deleted externally).\nRemove from this app?`,
                    confirmText: "Remove",
                    variant: 'info'
                })) {
                    const updated = folders.filter((folder) => !branchIds.has(folder.id));
                    setFolders(updated);
                    lsSet('folders', updated);
                    if (activeFolderId !== null && !updated.some((f) => f.id === activeFolderId)) {
                        setActiveFolderId(null);
                        lsSet('activeFolderId', null);
                    }
                }
            } else {
                toast.error(`Failed to delete folder: ${e}`);
            }
        }
    };


    const handleSetActiveFolderId = (id: number | null) => {
        setActiveFolderId(id);
        lsSet('activeFolderId', id);
    };

    return {
        ready,
        folders,
        activeFolderId,
        setActiveFolderId: handleSetActiveFolderId,
        isSyncing,
        lastSyncSummary,
        isConnected,
        handleLogout,
        handleSyncFolders,
        handleCreateFolder,
        handleFolderDelete,
        isNetworkError,
        forceLogout
    };
}
