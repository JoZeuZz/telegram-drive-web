use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::errors::AppError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BandwidthStats {
    pub date: String,
    pub up_bytes: u64,
    pub down_bytes: u64,
}

impl Default for BandwidthStats {
    fn default() -> Self {
        Self {
            date: Local::now().format("%Y-%m-%d").to_string(),
            up_bytes: 0,
            down_bytes: 0,
        }
    }
}

pub struct BandwidthManager {
    file_path: PathBuf,
    stats: Mutex<BandwidthStats>,
    free_limit: u64,
    premium_limit: u64,
}

impl BandwidthManager {
    /// Create a new BandwidthManager backed by a JSON file in `data_dir`.
    pub fn new(data_dir: &str) -> Self {
        Self::with_limits(data_dir, 250 * 1024 * 1024 * 1024, 250 * 1024 * 1024 * 1024)
    }

    /// Create a manager with explicit free and premium daily limits.
    pub fn with_limits(data_dir: &str, free_limit: u64, premium_limit: u64) -> Self {
        let data_path = PathBuf::from(data_dir);
        if !data_path.exists() {
            let _ = fs::create_dir_all(&data_path);
        }
        let file_path = data_path.join("bandwidth.json");

        let stats = if file_path.exists() {
            let content = fs::read_to_string(&file_path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            BandwidthStats::default()
        };

        Self {
            file_path,
            stats: Mutex::new(stats),
            free_limit,
            premium_limit,
        }
    }

    /// Reset stats when the date changes.
    fn check_and_reset(&self) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let mut stats = self.stats.lock().unwrap();
        if stats.date != today {
            tracing::info!(
                "New day detected. Resetting bandwidth stats. Old: {}, New: {}",
                stats.date,
                today
            );
            stats.date = today;
            stats.up_bytes = 0;
            stats.down_bytes = 0;
            self.save_locked(&stats);
        }
    }

    /// Check if `bytes` can be transferred within the daily limit.
    pub fn can_transfer(&self, bytes: u64) -> Result<(), AppError> {
        self.can_transfer_with_limit(bytes, self.free_limit)
    }

    /// Check if `bytes` can be transferred within the provided daily limit.
    pub fn can_transfer_with_limit(&self, bytes: u64, limit: u64) -> Result<(), AppError> {
        self.check_and_reset();
        let stats = self.stats.lock().unwrap();
        let total = stats.up_bytes + stats.down_bytes + bytes;
        if total > limit {
            return Err(AppError::BadRequest(format!(
                "Daily bandwidth limit ({}) exceeded! Used: {}",
                Self::format_bytes(limit),
                Self::format_bytes(total)
            )));
        }
        Ok(())
    }

    /// Return configured limit for the specified tier.
    pub fn limit_for_tier(&self, is_premium: bool) -> u64 {
        if is_premium {
            self.premium_limit
        } else {
            self.free_limit
        }
    }

    /// Record uploaded bytes.
    pub fn add_up(&self, bytes: u64) {
        self.check_and_reset();
        let mut stats = self.stats.lock().unwrap();
        stats.up_bytes += bytes;
        self.save_locked(&stats);
    }

    /// Record downloaded bytes.
    pub fn add_down(&self, bytes: u64) {
        self.check_and_reset();
        let mut stats = self.stats.lock().unwrap();
        stats.down_bytes += bytes;
        self.save_locked(&stats);
    }

    /// Get current bandwidth stats.
    pub fn get_stats(&self) -> BandwidthStats {
        self.check_and_reset();
        self.stats.lock().unwrap().clone()
    }

    /// Get total transferred bytes for the current day.
    pub fn total_used_bytes(&self) -> u64 {
        self.check_and_reset();
        let stats = self.stats.lock().unwrap();
        stats.up_bytes + stats.down_bytes
    }

    /// Calculate remaining bytes for a given daily limit.
    pub fn remaining_bytes(&self, limit: u64) -> u64 {
        let used = self.total_used_bytes();
        limit.saturating_sub(used)
    }

    fn save_locked(&self, stats: &BandwidthStats) {
        if let Ok(json) = serde_json::to_string(stats) {
            let _ = fs::write(&self.file_path, json);
        }
    }

    fn format_bytes(bytes: u64) -> String {
        const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
        let mut v = bytes as f64;
        let mut i = 0;
        while v >= 1024.0 && i < UNITS.len() - 1 {
            v /= 1024.0;
            i += 1;
        }
        format!("{:.2} {}", v, UNITS[i])
    }
}
