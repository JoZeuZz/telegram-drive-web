import { useEffect, useCallback } from 'react';

interface UseKeyboardShortcutsProps {
    onSelectAll: () => void;
    onDelete: () => void;
    onEscape: () => void;
    onSearch: () => void;
    onEnter?: () => void;
    onExtendSelection?: (direction: 'up' | 'down') => void;
    enabled?: boolean;
}

export function useKeyboardShortcuts({
    onSelectAll,
    onDelete,
    onEscape,
    onSearch,
    onEnter,
    onExtendSelection,
    enabled = true
}: UseKeyboardShortcutsProps) {

    const handleKeyDown = useCallback((e: KeyboardEvent) => {
        if (!enabled) return;

        // Don't trigger shortcuts when typing in inputs
        const target = e.target as HTMLElement;
        if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
            // Only allow Escape in inputs
            if (e.key === 'Escape') {
                (target as HTMLInputElement).blur();
                onEscape();
            }
            return;
        }

        const isMod = e.metaKey || e.ctrlKey;

        // Cmd/Ctrl + A - Select All
        if (isMod && e.key === 'a') {
            e.preventDefault();
            onSelectAll();
            return;
        }

        // Cmd/Ctrl + F - Focus Search
        if (isMod && e.key === 'f') {
            e.preventDefault();
            onSearch();
            return;
        }

        // Delete / Backspace - Delete selected
        if (e.key === 'Delete' || e.key === 'Backspace') {
            e.preventDefault();
            onDelete();
            return;
        }

        // Shift + Arrow - Extend selection range
        if (e.shiftKey && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
            e.preventDefault();
            onExtendSelection?.(e.key === 'ArrowDown' ? 'down' : 'up');
            return;
        }

        // Escape - Clear selection
        if (e.key === 'Escape') {
            e.preventDefault();
            onEscape();
            return;
        }
        // Enter - Open / Preview
        if (e.key === 'Enter') {
            e.preventDefault();
            onEnter?.();
            return;
        }
    }, [enabled, onSelectAll, onDelete, onEscape, onSearch, onEnter, onExtendSelection]);

    useEffect(() => {
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [handleKeyDown]);
}
