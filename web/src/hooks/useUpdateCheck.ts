/**
 * Update check hook — no-op in web version.
 * The server is updated by redeploying, not via in-app updates.
 */
export function useUpdateCheck() {
    return {
        checking: false,
        available: false,
        downloading: false,
        progress: 0,
        error: null as string | null,
        version: null as string | null,
        checkForUpdates: async () => {},
        downloadAndInstall: async () => {},
        dismissUpdate: () => {},
    };
}
