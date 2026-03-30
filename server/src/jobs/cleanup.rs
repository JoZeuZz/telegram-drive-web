use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

use crate::app_state::AppState;
use crate::storage;

/// Maximum cache age before automatic eviction (24 hours).
const MAX_CACHE_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// How often the cleanup task runs (every 30 minutes).
const CLEANUP_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Maximum total cache size in bytes (500 MB).
const MAX_CACHE_BYTES: u64 = 500 * 1024 * 1024;

/// Spawns a periodic task that evicts stale preview/thumbnail cache files.
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = time::interval(CLEANUP_INTERVAL);
        loop {
            interval.tick().await;
            let (removed, _) = cleanup_cache(&state.cache_dir);
            if removed > 0 {
                tracing::info!(removed, dir = %state.cache_dir, "Cache cleanup completed");
            }
        }
    });
}

/// Remove files older than `MAX_CACHE_AGE` or if total exceeds `MAX_CACHE_BYTES`.
/// Returns (files_removed, bytes_freed).
pub fn cleanup_cache(cache_dir: &str) -> (usize, u64) {
    let dir = Path::new(cache_dir);
    if !dir.exists() {
        return (0, 0);
    }

    let mut entries = storage::cache::walkdir_flat(dir);

    let now = std::time::SystemTime::now();
    let mut removed = 0usize;
    let mut bytes_freed = 0u64;
    let mut total_size: u64 = entries.iter().map(|(_, _, sz)| *sz).sum();

    // Sort oldest first for size-based eviction
    entries.sort_by_key(|(_, mtime, _)| *mtime);

    for (path, mtime, size) in &entries {
        let age = now.duration_since(*mtime).unwrap_or(Duration::ZERO);
        let over_limit = total_size > MAX_CACHE_BYTES;

        if age > MAX_CACHE_AGE || over_limit {
            if std::fs::remove_file(path).is_ok() {
                total_size = total_size.saturating_sub(*size);
                bytes_freed += size;
                removed += 1;
            }
        }
    }

    (removed, bytes_freed)
}
