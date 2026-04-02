use base64::{engine::general_purpose, Engine as _};
use grammers_client::types::Media;

use crate::app_state::AppState;
use crate::errors::AppError;
use crate::services::bandwidth::BandwidthManager;
use crate::services::helpers::resolve_peer;

const PREVIEW_CACHE_MAX_FILES: usize = 30;
const PREVIEW_CACHE_MAX_TOTAL_BYTES: u64 = 80 * 1024 * 1024;
const MEDIA_CACHE_SUBDIR: &str = "media";

fn folder_cache_key(folder_id: Option<i64>) -> String {
    folder_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "home".to_string())
}

/// Prune the preview cache directory: keep at most 30 files / 80 MB.
fn prune_preview_cache(cache_dir: &std::path::Path) {
    let read_dir = match std::fs::read_dir(cache_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    let mut files: Vec<(std::path::PathBuf, std::time::SystemTime, u64)> = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            files.push((path, modified, meta.len()));
        }
    }

    files.sort_by_key(|(_, modified, _)| *modified);

    let mut total_bytes: u64 = files.iter().map(|(_, _, len)| *len).sum();
    while files.len() > PREVIEW_CACHE_MAX_FILES || total_bytes > PREVIEW_CACHE_MAX_TOTAL_BYTES {
        if let Some((path, _, len)) = files.first().cloned() {
            let _ = std::fs::remove_file(&path);
            total_bytes = total_bytes.saturating_sub(len);
            files.remove(0);
        } else {
            break;
        }
    }
}

/// Download and return a preview for a message.
/// Returns a base64 data URL for images, or a local file path for other media.
pub async fn get_preview(
    state: &AppState,
    bw: &BandwidthManager,
    message_id: i32,
    folder_id: Option<i64>,
) -> Result<String, AppError> {
    let cache_dir_path = std::path::Path::new(&state.cache_dir).join(MEDIA_CACHE_SUBDIR);
    if !cache_dir_path.exists() {
        let _ = std::fs::create_dir_all(&cache_dir_path);
    }
    prune_preview_cache(&cache_dir_path);

    let client = state
        .telegram_client
        .lock()
        .await
        .clone()
        .ok_or(AppError::Unauthorized)?;

    let peer = resolve_peer(&client, folder_id)
        .await
        .map_err(|e| AppError::NotFound(e))?;

    let mut msgs = client.iter_messages(&peer);
    let mut target_message = None;
    while let Some(m) = msgs
        .next()
        .await
        .map_err(|e| AppError::Telegram(e.to_string()))?
    {
        if m.id() == message_id {
            target_message = Some(m);
            break;
        }
    }

    if let Some(msg) = target_message {
        if let Some(media) = msg.media() {
            let ext = media_extension(&media);
            let folder_key = folder_cache_key(folder_id);
            let save_path = cache_dir_path.join(format!("{}_{}.{}", folder_key, message_id, ext));
            let save_path_str = save_path.to_string_lossy().to_string();

            let file_ready = if save_path.exists() {
                true
            } else {
                let size = match &media {
                    Media::Document(d) => d.size() as u64,
                    Media::Photo(_) => 1024 * 1024,
                    _ => 0,
                };

                if bw.can_transfer(size).is_err() {
                    false
                } else {
                    match client.download_media(&media, &save_path_str).await {
                        Ok(_) => {
                            bw.add_down(size);
                            prune_preview_cache(&cache_dir_path);
                            true
                        }
                        Err(e) => {
                            tracing::error!("Preview Download Error: {}", e);
                            false
                        }
                    }
                }
            };

            if file_ready {
                let lower_ext = ext.to_lowercase();
                if is_image_ext(&lower_ext) {
                    match std::fs::read(&save_path) {
                        Ok(bytes) => {
                            let b64 = general_purpose::STANDARD.encode(&bytes);
                            let mime = mime_from_ext(&lower_ext);
                            return Ok(format!("data:{};base64,{}", mime, b64));
                        }
                        Err(e) => {
                            tracing::error!("Failed to read file for base64: {}", e);
                            return Ok(save_path_str);
                        }
                    }
                }
                return Ok(save_path_str);
            }
        }
    }

    Err(AppError::NotFound(
        "File not found or failed to download".to_string(),
    ))
}

/// Clean the entire preview cache.
pub fn clean_cache(cache_dir: &str) -> Result<(), AppError> {
    let preview_dir = std::path::Path::new(cache_dir).join(MEDIA_CACHE_SUBDIR);
    if preview_dir.exists() {
        let _ = std::fs::remove_dir_all(preview_dir);
    }
    Ok(())
}

/// Get a small thumbnail for inline display.
/// Returns base64 data URL for images, empty string for non-image files.
pub async fn get_thumbnail(
    state: &AppState,
    message_id: i32,
    folder_id: Option<i64>,
) -> Result<String, AppError> {
    let cache_dir_path = std::path::Path::new(&state.cache_dir).join(MEDIA_CACHE_SUBDIR);
    if !cache_dir_path.exists() {
        let _ = std::fs::create_dir_all(&cache_dir_path);
    }

    let folder_key = folder_cache_key(folder_id);
    let file_prefix = format!("{}_{}.", folder_key, message_id);

    // Check for cached thumbnail
    if let Ok(entries) = std::fs::read_dir(&cache_dir_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&file_prefix) {
                if let Ok(bytes) = std::fs::read(entry.path()) {
                    let ext = name.rsplit('.').next().unwrap_or("jpg");
                    let mime = mime_from_ext(ext);
                    let b64 = general_purpose::STANDARD.encode(&bytes);
                    return Ok(format!("data:{};base64,{}", mime, b64));
                }
            }
        }
    }

    // No cache — fetch from Telegram
    let client = state
        .telegram_client
        .lock()
        .await
        .clone()
        .ok_or(AppError::Unauthorized)?;

    let peer = resolve_peer(&client, folder_id)
        .await
        .map_err(|e| AppError::NotFound(e))?;

    let mut msgs = client.iter_messages(&peer);
    while let Some(m) = msgs
        .next()
        .await
        .map_err(|e| AppError::Telegram(e.to_string()))?
    {
        if m.id() == message_id {
            if let Some(media) = m.media() {
                let (is_image, ext) = match &media {
                    Media::Photo(_) => (true, "jpg".to_string()),
                    Media::Document(d) => {
                        let mime = d.mime_type().unwrap_or("");
                        if mime.starts_with("image/") {
                            let e = match mime {
                                "image/png" => "png",
                                "image/gif" => "gif",
                                "image/webp" => "webp",
                                _ => "jpg",
                            };
                            (true, e.to_string())
                        } else {
                            return Ok(String::new());
                        }
                    }
                    _ => return Ok(String::new()),
                };

                if is_image {
                    let save_path =
                        cache_dir_path.join(format!("{}_{}.{}", folder_key, message_id, ext));
                    let save_path_str = save_path.to_string_lossy().to_string();

                    if client.download_media(&media, &save_path_str).await.is_ok() {
                        if let Ok(bytes) = std::fs::read(&save_path) {
                            let mime = mime_from_ext(&ext);
                            let b64 = general_purpose::STANDARD.encode(&bytes);
                            return Ok(format!("data:{};base64,{}", mime, b64));
                        }
                    }
                }
            }
            break;
        }
    }

    Ok(String::new())
}

fn media_extension(media: &Media) -> String {
    match media {
        Media::Document(d) => {
            let mut e = std::path::Path::new(d.name())
                .extension()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if e.is_empty() {
                if let Some(mime) = d.mime_type() {
                    e = match mime {
                        "image/jpeg" => "jpg".to_string(),
                        "image/png" => "png".to_string(),
                        "video/mp4" => "mp4".to_string(),
                        _ => "bin".to_string(),
                    };
                } else {
                    e = "bin".to_string();
                }
            }
            e
        }
        Media::Photo(_) => "jpg".to_string(),
        _ => "bin".to_string(),
    }
}

fn is_image_ext(ext: &str) -> bool {
    matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "svg")
}

fn mime_from_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => "image/jpeg",
    }
}
