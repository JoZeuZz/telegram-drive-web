use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 128;
const FINISHED_RETENTION_SECS: u64 = 10 * 60;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UploadProgressStatus {
    Uploading,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UploadProgressStage {
    BrowserToServer,
    ServerToTelegram,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadProgressSnapshot {
    pub upload_id: String,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub status: UploadProgressStatus,
    pub stage: UploadProgressStage,
    pub browser_to_server_bytes: u64,
    pub telegram_upload_bytes: u64,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct UploadProgressManager {
    entries: Arc<RwLock<HashMap<String, UploadProgressSnapshot>>>,
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<UploadProgressSnapshot>>>>,
}

impl UploadProgressManager {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn start_upload(&self, upload_id: &str, file_name: &str, file_size_bytes: u64) {
        let now = now_ms();
        let snapshot = UploadProgressSnapshot {
            upload_id: upload_id.to_string(),
            file_name: file_name.to_string(),
            file_size_bytes,
            status: UploadProgressStatus::Uploading,
            stage: UploadProgressStage::BrowserToServer,
            browser_to_server_bytes: 0,
            telegram_upload_bytes: 0,
            started_at_ms: now,
            updated_at_ms: now,
            error: None,
        };

        self.write_entries()
            .insert(upload_id.to_string(), snapshot.clone());
        self.publish(&snapshot);
    }

    pub fn set_file_size(&self, upload_id: &str, file_size_bytes: u64) {
        self.update(upload_id, |snapshot| {
            snapshot.file_size_bytes = file_size_bytes;
        });
    }

    pub fn update_browser_bytes(&self, upload_id: &str, bytes: u64) {
        self.update(upload_id, |snapshot| {
            snapshot.status = UploadProgressStatus::Uploading;
            snapshot.stage = UploadProgressStage::BrowserToServer;
            snapshot.browser_to_server_bytes = bytes;
        });
    }

    pub fn switch_to_telegram_stage(&self, upload_id: &str) {
        self.update(upload_id, |snapshot| {
            snapshot.status = UploadProgressStatus::Uploading;
            snapshot.stage = UploadProgressStage::ServerToTelegram;
        });
    }

    pub fn update_telegram_bytes(&self, upload_id: &str, bytes: u64) {
        self.update(upload_id, |snapshot| {
            snapshot.status = UploadProgressStatus::Uploading;
            snapshot.stage = UploadProgressStage::ServerToTelegram;
            snapshot.telegram_upload_bytes = bytes;
        });
    }

    pub fn mark_completed(&self, upload_id: &str) {
        self.update(upload_id, |snapshot| {
            snapshot.status = UploadProgressStatus::Completed;
            snapshot.stage = UploadProgressStage::Completed;
            snapshot.error = None;
            if snapshot.file_size_bytes > 0 {
                snapshot.browser_to_server_bytes = snapshot.file_size_bytes;
                snapshot.telegram_upload_bytes = snapshot.file_size_bytes;
            }
        });
        self.schedule_cleanup(upload_id.to_string());
    }

    pub fn mark_failed(&self, upload_id: &str, error: String) {
        self.update(upload_id, |snapshot| {
            snapshot.status = UploadProgressStatus::Failed;
            snapshot.stage = UploadProgressStage::Failed;
            snapshot.error = Some(error);
        });
        self.schedule_cleanup(upload_id.to_string());
    }

    pub fn mark_cancelled(&self, upload_id: &str) {
        self.update(upload_id, |snapshot| {
            snapshot.status = UploadProgressStatus::Cancelled;
            snapshot.stage = UploadProgressStage::Cancelled;
            snapshot.error = None;
        });
        self.schedule_cleanup(upload_id.to_string());
    }

    pub fn snapshot(&self, upload_id: &str) -> Option<UploadProgressSnapshot> {
        self.read_entries().get(upload_id).cloned()
    }

    pub fn subscribe(
        &self,
        upload_id: &str,
    ) -> Option<broadcast::Receiver<UploadProgressSnapshot>> {
        if !self.read_entries().contains_key(upload_id) {
            return None;
        }
        Some(self.sender_for(upload_id).subscribe())
    }

    pub fn remove(&self, upload_id: &str) {
        self.write_entries().remove(upload_id);
        self.write_channels().remove(upload_id);
    }

    fn update<F>(&self, upload_id: &str, mutator: F)
    where
        F: FnOnce(&mut UploadProgressSnapshot),
    {
        let snapshot = {
            let mut entries = self.write_entries();
            let Some(entry) = entries.get_mut(upload_id) else {
                return;
            };
            mutator(entry);
            entry.updated_at_ms = now_ms();
            entry.clone()
        };
        self.publish(&snapshot);
    }

    fn publish(&self, snapshot: &UploadProgressSnapshot) {
        let sender = self.sender_for(&snapshot.upload_id);
        let _ = sender.send(snapshot.clone());
    }

    fn sender_for(&self, upload_id: &str) -> broadcast::Sender<UploadProgressSnapshot> {
        if let Some(existing) = self.read_channels().get(upload_id).cloned() {
            return existing;
        }

        let (sender, _receiver) = broadcast::channel(CHANNEL_CAPACITY);
        self.write_channels()
            .insert(upload_id.to_string(), sender.clone());
        sender
    }

    fn schedule_cleanup(&self, upload_id: String) {
        let manager = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(FINISHED_RETENTION_SECS)).await;
            manager.remove(&upload_id);
        });
    }

    fn read_entries(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, HashMap<String, UploadProgressSnapshot>> {
        self.entries
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_entries(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, HashMap<String, UploadProgressSnapshot>> {
        self.entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn read_channels(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, HashMap<String, broadcast::Sender<UploadProgressSnapshot>>>
    {
        self.channels
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_channels(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, HashMap<String, broadcast::Sender<UploadProgressSnapshot>>>
    {
        self.channels
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for UploadProgressManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct UploadProgressReporter {
    manager: UploadProgressManager,
    upload_id: String,
}

impl UploadProgressReporter {
    pub fn new(manager: UploadProgressManager, upload_id: String) -> Self {
        Self { manager, upload_id }
    }

    pub fn update_telegram_bytes_nowait(&self, bytes: u64) {
        self.manager.update_telegram_bytes(&self.upload_id, bytes);
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
