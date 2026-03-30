import { useState, useEffect } from 'react';
import * as api from '../lib/api';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { useConfirm } from '../context/ConfirmContext';
import { TelegramFolder } from '../types';
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

export function useTelegramConnection(onLogoutParent: () => void) {
    const queryClient = useQueryClient();
    const { confirm } = useConfirm();

    const [ready, setReady] = useState(false);
    const [folders, setFolders] = useState<TelegramFolder[]>([]);
    const [activeFolderId, setActiveFolderId] = useState<number | null>(null);
    const [isSyncing, setIsSyncing] = useState(false);
    const [isConnected, setIsConnected] = useState(true);


    const networkIsOnline = useNetworkStatus();

    // Initialise from localStorage + check Telegram status
    useEffect(() => {
        const init = async () => {
            const savedFolders = lsGet<TelegramFolder[]>('folders');
            if (savedFolders) setFolders(savedFolders);

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
            localStorage.removeItem('api_hash');
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
            localStorage.removeItem('api_hash');
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
            const foundFolders = await api.listFolders();
            const merged = [...folders];
            let added = 0;
            for (const f of foundFolders) {
                if (!merged.find(existing => existing.id === f.id)) {
                    merged.push(f);
                    added++;
                }
            }
            if (added > 0) {
                setFolders(merged);
                lsSet('folders', merged);
                toast.success(`Scan complete. Found ${added} new folders.`);
            } else {
                toast.info("Scan complete. No new folders found.");
            }
        } catch {
            toast.error("Sync failed");
        } finally {
            setIsSyncing(false);
        }
    };

    const handleCreateFolder = async (name: string) => {
        try {
            const newFolder = await api.createFolder(name);
            const updated = [...folders, newFolder];
            setFolders(updated);
            lsSet('folders', updated);
            toast.success(`Folder "${name}" created.`);
        } catch (e) {
            toast.error("Failed to create folder: " + e);
            throw e;
        }
    };

    const handleFolderDelete = async (folderId: number, folderName: string) => {
        if (!await confirm({
            title: "Delete Folder",
            message: `Are you sure you want to delete "${folderName}"?\nThis will delete the channel on Telegram.`,
            confirmText: "Delete",
            variant: 'danger'
        })) return;

        try {
            await api.deleteFolder(folderId);
            const updated = folders.filter(f => f.id !== folderId);
            setFolders(updated);
            lsSet('folders', updated);
            if (activeFolderId === folderId) setActiveFolderId(null);
            toast.success(`Folder "${folderName}" deleted.`);
        } catch (e: unknown) {
            const errStr = String(e);
            if (errStr.includes("not found")) {
                if (await confirm({
                    title: "Folder Not Found",
                    message: `Folder "${folderName}" not found on Telegram (it may have been deleted externally).\nRemove from this app?`,
                    confirmText: "Remove",
                    variant: 'info'
                })) {
                    const updated = folders.filter(f => f.id !== folderId);
                    setFolders(updated);
                    lsSet('folders', updated);
                    if (activeFolderId === folderId) setActiveFolderId(null);
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
        isConnected,
        handleLogout,
        handleSyncFolders,
        handleCreateFolder,
        handleFolderDelete,
        isNetworkError,
        forceLogout
    };
}
