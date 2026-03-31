import * as api from '../lib/api';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { useConfirm } from '../context/ConfirmContext';
import { TelegramFile } from '../types';
import { downloadFileUrl } from '../lib/api';
import { useState } from 'react';

export function useFileOperations(
    activeFolderId: number | null,
    selectedIds: number[],
    setSelectedIds: (ids: number[]) => void,
    displayedFiles: TelegramFile[]
) {
    const queryClient = useQueryClient();
    const { confirm } = useConfirm();
    const [isDeleting, setIsDeleting] = useState(false);
    const [deleteProgress, setDeleteProgress] = useState({ done: 0, total: 0 });

    const handleDelete = async (id: number) => {
        if (isDeleting) {
            toast.info("A delete operation is already in progress.");
            return;
        }

        if (!await confirm({ title: "Delete File", message: "Are you sure you want to delete this file?", confirmText: "Delete", variant: 'danger' })) return;

        const toastId = `delete-single-${id}`;
        setIsDeleting(true);
        setDeleteProgress({ done: 0, total: 1 });
        toast.loading("Deleting file...", { id: toastId });

        try {
            await api.deleteFile(id, activeFolderId);
            setDeleteProgress({ done: 1, total: 1 });
            queryClient.invalidateQueries({ queryKey: ['files', activeFolderId] });
            toast.success("File deleted", { id: toastId });
        } catch (e) {
            toast.error(`Delete failed: ${e}`, { id: toastId });
        } finally {
            setIsDeleting(false);
            setDeleteProgress({ done: 0, total: 0 });
        }
    }

    const handleBulkDelete = async () => {
        if (isDeleting) {
            toast.info("A delete operation is already in progress.");
            return;
        }

        if (selectedIds.length === 0) return;
        if (!await confirm({ title: "Delete Files", message: `Are you sure you want to delete ${selectedIds.length} files?`, confirmText: "Delete All", variant: 'danger' })) return;

        const total = selectedIds.length;
        const toastId = 'delete-bulk-progress';
        setIsDeleting(true);
        setDeleteProgress({ done: 0, total });
        toast.loading(`Deleting files... (0/${total})`, { id: toastId });

        let success = 0;
        let fail = 0;
        let processed = 0;

        for (const id of selectedIds) {
            try {
                await api.deleteFile(id, activeFolderId);
                success++;
            } catch {
                fail++;
            }

            processed++;
            setDeleteProgress({ done: processed, total });
            toast.loading(`Deleting files... (${processed}/${total})`, { id: toastId });
        }

        setSelectedIds([]);
        queryClient.invalidateQueries({ queryKey: ['files', activeFolderId] });

        if (fail === 0) {
            toast.success(`Deleted ${success} files.`, { id: toastId });
        } else if (success === 0) {
            toast.error(`Failed to delete ${fail} files.`, { id: toastId });
        } else {
            toast.warning(`Deleted ${success} files, failed ${fail}.`, { id: toastId });
        }

        setIsDeleting(false);
        setDeleteProgress({ done: 0, total: 0 });
    }

    /** Trigger a browser download via a hidden <a> tag */
    const handleDownload = async (_id: number, name: string) => {
        const url = downloadFileUrl(_id, activeFolderId);
        const a = document.createElement('a');
        a.href = url;
        a.download = name;
        document.body.appendChild(a);
        a.click();
        a.remove();
        toast.info(`Download started: ${name}`);
    }

    const handleBulkDownload = async () => {
        if (selectedIds.length === 0) return;
        const targetFiles = displayedFiles.filter((f) => selectedIds.includes(f.id));
        for (const file of targetFiles) {
            const url = downloadFileUrl(file.id, activeFolderId);
            const a = document.createElement('a');
            a.href = url;
            a.download = file.name;
            document.body.appendChild(a);
            a.click();
            a.remove();
        }
        toast.info(`Started download of ${targetFiles.length} files`);
        setSelectedIds([]);
    }

    const handleBulkMove = async (targetFolderId: number | null, onSuccess?: () => void) => {
        if (selectedIds.length === 0) return;
        try {
            await api.moveFiles(selectedIds, activeFolderId, targetFolderId);
            toast.success(`Moved ${selectedIds.length} files.`);
            queryClient.invalidateQueries({ queryKey: ['files', activeFolderId] });
            setSelectedIds([]);
            if (onSuccess) onSuccess();
        } catch {
            toast.error('Failed to move files');
        }
    };

    const handleDownloadFolder = async () => {
        if (displayedFiles.length === 0) {
            toast.info("Folder is empty.");
            return;
        }
        for (const file of displayedFiles) {
            const url = downloadFileUrl(file.id, activeFolderId);
            const a = document.createElement('a');
            a.href = url;
            a.download = file.name;
            document.body.appendChild(a);
            a.click();
            a.remove();
        }
        toast.info(`Started download of ${displayedFiles.length} files from folder`);
    }

    return {
        handleDelete,
        handleBulkDelete,
        handleDownload,
        handleBulkDownload,
        handleBulkMove,
        handleDownloadFolder,
        isDeleting,
        deleteProgress,
        handleGlobalSearch: async (query: string) => {
            try {
                return await api.searchFiles(query);
            } catch {
                return [];
            }
        }
    };
}
