import { useState, useEffect } from 'react';
import * as api from '../lib/api';

/**
 * Network detection for the web client.
 * Uses the browser's navigator.onLine + periodic health checks to the server.
 */
export function useNetworkStatus() {
    const [isOnline, setIsOnline] = useState(navigator.onLine);

    useEffect(() => {
        const handleOnline = () => setIsOnline(true);
        const handleOffline = () => setIsOnline(false);

        window.addEventListener('online', handleOnline);
        window.addEventListener('offline', handleOffline);

        // Periodic server health check
        const check = async () => {
            try {
                await api.health();
                setIsOnline(true);
            } catch {
                setIsOnline(false);
            }
        };

        const interval = setInterval(check, 15000);

        return () => {
            window.removeEventListener('online', handleOnline);
            window.removeEventListener('offline', handleOffline);
            clearInterval(interval);
        };
    }, []);

    return isOnline;
}
