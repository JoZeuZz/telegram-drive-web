use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::{mpsc, RwLock, Semaphore};

use crate::app_state::AppState;
use crate::errors::AppError;
use crate::services::bandwidth::BandwidthManager;
use crate::services::telegram_files;

/// A job submitted to the upload queue.
pub struct UploadJob {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub content_type: Option<String>,
    pub folder_id: Option<i64>,
    pub size: u64,
    pub as_photo: bool,
}

/// Status of a queued upload.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Uploading,
    Completed,
    Failed,
    Cancelled,
}

/// A tracked upload job entry.
#[derive(Debug, Clone, Serialize)]
pub struct JobEntry {
    pub id: String,
    pub file_name: String,
    pub size: u64,
    pub folder_id: Option<i64>,
    pub status: JobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

const MAX_FINISHED_ENTRIES: usize = 200;

/// Handle to the background upload queue.
pub struct UploadQueue {
    tx: mpsc::Sender<UploadJob>,
    jobs: Arc<RwLock<HashMap<String, JobEntry>>>,
}

impl UploadQueue {
    /// Create a new upload queue and spawn a background processor.
    pub fn new(state: Arc<AppState>, bw: Arc<BandwidthManager>, max_concurrent: usize) -> Self {
        let (tx, rx) = mpsc::channel::<UploadJob>(256);
        let jobs: Arc<RwLock<HashMap<String, JobEntry>>> = Arc::new(RwLock::new(HashMap::new()));

        let queue = Self {
            tx,
            jobs: jobs.clone(),
        };

        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        tokio::spawn(Self::processor(rx, jobs, state, bw, semaphore));

        queue
    }

    /// Enqueue a new upload job. Returns the job ID.
    pub async fn enqueue(&self, job: UploadJob) -> Result<String, AppError> {
        let id = job.id.clone();
        let entry = JobEntry {
            id: id.clone(),
            file_name: job.file_name.clone(),
            size: job.size,
            folder_id: job.folder_id,
            status: JobStatus::Queued,
            error: None,
        };
        self.jobs.write().await.insert(id.clone(), entry);
        self.tx
            .send(job)
            .await
            .map_err(|_| AppError::Internal("Upload queue full".into()))?;
        Ok(id)
    }

    /// List all tracked jobs.
    pub async fn list_jobs(&self) -> Vec<JobEntry> {
        self.jobs.read().await.values().cloned().collect()
    }

    /// Cancel a queued (not yet uploading) job.
    pub async fn cancel_job(&self, id: &str) -> bool {
        let mut jobs = self.jobs.write().await;
        if let Some(entry) = jobs.get_mut(id) {
            if entry.status == JobStatus::Queued {
                entry.status = JobStatus::Cancelled;
                return true;
            }
        }
        false
    }

    /// Remove finished (completed/failed/cancelled) entries.
    pub async fn clear_finished(&self) -> usize {
        let mut jobs = self.jobs.write().await;
        let before = jobs.len();
        jobs.retain(|_, e| {
            !matches!(
                e.status,
                JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
            )
        });
        before - jobs.len()
    }

    /// Background task that processes enqueued uploads.
    async fn processor(
        mut rx: mpsc::Receiver<UploadJob>,
        jobs: Arc<RwLock<HashMap<String, JobEntry>>>,
        state: Arc<AppState>,
        bw: Arc<BandwidthManager>,
        semaphore: Arc<Semaphore>,
    ) {
        while let Some(job) = rx.recv().await {
            let jobs = jobs.clone();
            let state = state.clone();
            let bw = bw.clone();
            let semaphore = semaphore.clone();

            tokio::spawn(async move {
                // Check if already cancelled
                {
                    let guard = jobs.read().await;
                    if let Some(e) = guard.get(&job.id) {
                        if e.status == JobStatus::Cancelled {
                            let _ = std::fs::remove_file(&job.file_path);
                            return;
                        }
                    }
                }

                // Wait for a concurrency slot
                let _permit = match semaphore.acquire().await {
                    Ok(p) => p,
                    Err(_) => return,
                };

                // Mark as uploading (re-check cancel)
                {
                    let mut guard = jobs.write().await;
                    if let Some(e) = guard.get_mut(&job.id) {
                        if e.status == JobStatus::Cancelled {
                            let _ = std::fs::remove_file(&job.file_path);
                            return;
                        }
                        e.status = JobStatus::Uploading;
                    }
                }

                // Perform the upload
                let result = telegram_files::upload_file(
                    &state,
                    &bw,
                    &job.file_path,
                    job.folder_id,
                    &job.file_name,
                    job.content_type.as_deref(),
                    job.as_photo,
                    None,
                )
                .await;

                // Update status
                {
                    let mut guard = jobs.write().await;
                    if let Some(e) = guard.get_mut(&job.id) {
                        match result {
                            Ok(_) => e.status = JobStatus::Completed,
                            Err(err) => {
                                e.status = JobStatus::Failed;
                                e.error = Some(err.to_string());
                            }
                        }
                    }
                }

                // Clean up temp file
                let _ = std::fs::remove_file(&job.file_path);

                // Prune old finished entries
                {
                    let mut guard = jobs.write().await;
                    let finished: Vec<String> = guard
                        .iter()
                        .filter(|(_, e)| {
                            matches!(
                                e.status,
                                JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
                            )
                        })
                        .map(|(k, _)| k.clone())
                        .collect();
                    if finished.len() > MAX_FINISHED_ENTRIES {
                        let to_remove = finished.len() - MAX_FINISHED_ENTRIES;
                        for key in finished.iter().take(to_remove) {
                            guard.remove(key);
                        }
                    }
                }
            });
        }
    }
}
