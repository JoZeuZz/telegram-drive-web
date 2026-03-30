use std::path::Path;

/// Return (total_bytes, file_count) for the cache directory.
pub fn cache_stats(cache_dir: &str) -> (u64, usize) {
    let dir = Path::new(cache_dir);
    if !dir.exists() {
        return (0, 0);
    }
    let files = walkdir_flat(dir);
    let total: u64 = files.iter().map(|(_, _, sz)| *sz).sum();
    (total, files.len())
}

/// Total size in bytes of a cache directory (recursive).
pub fn cache_size_bytes(cache_dir: &str) -> u64 {
    cache_stats(cache_dir).0
}

/// Collect all files under `dir` with their modified-time and size.
pub fn walkdir_flat(dir: &Path) -> Vec<(std::path::PathBuf, std::time::SystemTime, u64)> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_dir() {
                out.append(&mut walkdir_flat(&entry.path()));
            } else {
                let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                out.push((entry.path(), mtime, meta.len()));
            }
        }
    }
    out
}
