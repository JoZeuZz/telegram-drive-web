import { QueueItem } from "../../types";
import { formatBytes } from "../../utils";

interface UploadQueueProps {
    items: QueueItem[];
    onCancelItem: (id: string) => void;
    onClearFinished: () => void;
}

function clampPercent(value: number): number {
    if (!Number.isFinite(value)) return 0;
    if (value <= 0) return 0;
    if (value >= 100) return 100;
    return value;
}

function formatSpeed(speedBps?: number): string | null {
    if (!speedBps || !Number.isFinite(speedBps) || speedBps <= 0) {
        return null;
    }

    return `${formatBytes(speedBps, 1)}/s`;
}

function formatEta(etaSeconds?: number): string | null {
    if (!etaSeconds || !Number.isFinite(etaSeconds) || etaSeconds <= 0) {
        return null;
    }

    const totalSeconds = Math.ceil(etaSeconds);
    if (totalSeconds < 60) {
        return `${totalSeconds}s ETA`;
    }

    const totalMinutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    if (totalMinutes < 60) {
        return `${totalMinutes}m ${seconds}s ETA`;
    }

    const hours = Math.floor(totalMinutes / 60);
    const minutes = totalMinutes % 60;
    return `${hours}h ${minutes}m ETA`;
}

function getStageLabel(item: QueueItem): string {
    if (item.stage === 'browser_to_server') return 'Browser -> Server';
    if (item.stage === 'server_to_telegram') return 'Server -> Telegram';
    if (item.stage === 'completed') return 'Completed';
    if (item.stage === 'failed') return 'Failed';
    if (item.stage === 'cancelled') return 'Cancelled';

    if (item.status === 'pending') return 'Queued';
    if (item.status === 'uploading') return 'Uploading';
    if (item.status === 'success') return 'Completed';
    if (item.status === 'error') return 'Failed';
    return 'Cancelled';
}

function getStatusText(item: QueueItem): string {
    if (item.status === 'pending') return 'Queued';
    if (item.status === 'uploading') {
        return `${Math.round(clampPercent(item.progressPercent ?? 0))}%`;
    }
    if (item.status === 'success') return 'Done';
    if (item.status === 'error') return 'Error';
    return 'Cancelled';
}

function getStatusDotClass(item: QueueItem): string {
    if (item.status === 'pending') return 'bg-yellow-500';
    if (item.status === 'uploading') return 'bg-blue-500 animate-pulse';
    if (item.status === 'error') return 'bg-red-500';
    if (item.status === 'cancelled') return 'bg-zinc-500';
    return 'bg-green-500';
}

function getProgressBarClass(item: QueueItem): string {
    if (item.status === 'success') return 'bg-green-500';
    if (item.status === 'error') return 'bg-red-500';
    if (item.status === 'cancelled') return 'bg-zinc-500';
    return 'bg-blue-500';
}

export function UploadQueue({ items, onCancelItem, onClearFinished }: UploadQueueProps) {
    if (items.length === 0) return null;

    return (
        <div className="fixed bottom-4 right-4 w-80 bg-telegram-surface border border-telegram-border rounded-xl shadow-2xl overflow-hidden z-[100]">
            <div className="p-3 border-b border-telegram-border bg-telegram-hover flex justify-between items-center">
                <h4 className="text-sm font-medium text-telegram-text">Uploads</h4>
                <button onClick={onClearFinished} className="text-xs text-telegram-primary hover:text-telegram-text transition-colors">Clear Finished</button>
            </div>
            <div className="max-h-60 overflow-y-auto p-2 space-y-2">
                {items.map(item => (
                    <div key={item.id} className="flex flex-col gap-1 p-2 bg-telegram-hover rounded">
                        {(() => {
                            const progressPercent = clampPercent(item.progressPercent ?? (item.status === 'success' ? 100 : 0));
                            const stageProgressPercent = clampPercent(item.stageProgressPercent ?? progressPercent);
                            const fileSizeBytes = item.fileSizeBytes ?? item.file.size;
                            const stageBytes = item.stage === 'server_to_telegram'
                                ? (item.telegramUploadBytes ?? 0)
                                : (item.browserToServerBytes ?? 0);
                            const speedLabel = formatSpeed(item.uploadSpeedBps);
                            const etaLabel = formatEta(item.etaSeconds);

                            return (
                                <>
                                    <div className="flex items-center gap-3 text-sm">
                                        <div className={`w-2 h-2 rounded-full ${getStatusDotClass(item)}`} />
                                        <div className="flex-1 truncate text-telegram-subtext" title={item.file.name}>
                                            {item.file.name}
                                        </div>
                                        {(item.status === 'pending' || item.status === 'uploading') && (
                                            <button
                                                onClick={() => onCancelItem(item.id)}
                                                className="text-xs text-zinc-300 hover:text-telegram-text transition-colors"
                                            >
                                                Cancel
                                            </button>
                                        )}
                                        <div className="text-xs text-telegram-subtext">{getStatusText(item)}</div>
                                    </div>

                                    {(item.status === 'uploading' || item.status === 'success' || item.status === 'error' || item.status === 'cancelled') && (
                                        <>
                                            <div className="w-full bg-telegram-border h-1 mt-1 rounded-full overflow-hidden">
                                                <div
                                                    className={`${getProgressBarClass(item)} h-full transition-[width] duration-300`}
                                                    style={{ width: `${progressPercent}%` }}
                                                />
                                            </div>
                                            <div className="flex justify-between text-[11px] text-telegram-subtext mt-1">
                                                <span>{getStageLabel(item)} ({Math.round(stageProgressPercent)}%)</span>
                                                <span>{formatBytes(stageBytes)} / {formatBytes(fileSizeBytes)}</span>
                                            </div>
                                            {(speedLabel || etaLabel) && item.status === 'uploading' && (
                                                <div className="flex justify-between text-[11px] text-blue-300">
                                                    <span>{speedLabel ?? ''}</span>
                                                    <span>{etaLabel ?? ''}</span>
                                                </div>
                                            )}
                                        </>
                                    )}

                                    {item.status === 'error' && item.error && (
                                        <div className="text-[11px] text-red-300 truncate" title={item.error}>{item.error}</div>
                                    )}
                                </>
                            );
                        })()}
                    </div>
                ))}
            </div>
        </div>
    )
}
