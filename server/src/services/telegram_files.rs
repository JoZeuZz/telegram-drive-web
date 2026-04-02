use grammers_client::types::Media;
use grammers_client::InputMessage;
use grammers_tl_types as tl;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};

use crate::app_state::AppState;
use crate::domain::models::FileMetadata;
use crate::errors::{map_telegram_error, AppError};
use crate::services::bandwidth::BandwidthManager;
use crate::services::helpers::resolve_peer;
use crate::services::upload_progress::UploadProgressReporter;

/// Get the authenticated Telegram client or return an error.
async fn require_client(state: &AppState) -> Result<grammers_client::Client, AppError> {
    state
        .telegram_client
        .lock()
        .await
        .clone()
        .ok_or(AppError::Unauthorized)
}

fn sanitize_upload_name(file_name: &str) -> String {
    let candidate = std::path::Path::new(file_name)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed".to_string());

    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        "unnamed".to_string()
    } else {
        trimmed.to_string()
    }
}

fn should_send_as_photo(file_name: &str, content_type: Option<&str>, as_photo: bool) -> bool {
    if !as_photo {
        return false;
    }

    if let Some(mime) = content_type {
        if mime.trim().to_ascii_lowercase().starts_with("image/") {
            return true;
        }
    }

    let ext = std::path::Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    matches!(
        ext.as_deref(),
        Some("jpg"
            | "jpeg"
            | "png"
            | "gif"
            | "bmp"
            | "webp"
            | "tif"
            | "tiff"
            | "heic"
            | "heif")
    )
}

/// Upload a file to a folder (channel) or Saved Messages.
pub async fn upload_file(
    state: &AppState,
    bw: &BandwidthManager,
    path: &str,
    folder_id: Option<i64>,
    file_name: &str,
    content_type: Option<&str>,
    as_photo: bool,
    progress_reporter: Option<UploadProgressReporter>,
) -> Result<String, AppError> {
    let size = std::fs::metadata(path)
        .map_err(|e| AppError::BadRequest(format!("Cannot read file: {}", e)))?
        .len();
    bw.can_transfer(size)?;

    let client = require_client(state).await?;
    let upload_len = usize::try_from(size)
        .map_err(|_| AppError::BadRequest("File too large for this platform".to_string()))?;
    let upload_name = sanitize_upload_name(file_name);
    let upload_file = tokio::fs::File::open(path)
        .await
        .map_err(|e| AppError::BadRequest(format!("Cannot open file for upload: {}", e)))?;
    let mut upload_stream = ProgressReader::new(upload_file, progress_reporter);

    let uploaded_file = client
        .upload_stream(&mut upload_stream, upload_len, upload_name.clone())
        .await
        .map_err(|e| map_telegram_error(e))?;

    let mut message = InputMessage::new().text("");
    if should_send_as_photo(&upload_name, content_type, as_photo) {
        message = message.photo(uploaded_file);
    } else {
        if let Some(mime) = content_type.filter(|mime| !mime.trim().is_empty()) {
            message = message.mime_type(mime);
        }
        message = message.file(uploaded_file);
    }

    let peer = resolve_peer(&client, folder_id)
        .await
        .map_err(|e| AppError::NotFound(e))?;

    client
        .send_message(&peer, message)
        .await
        .map_err(|e| map_telegram_error(e))?;

    bw.add_up(size);
    Ok("File uploaded successfully".to_string())
}

struct ProgressReader<R> {
    inner: R,
    reporter: Option<UploadProgressReporter>,
    bytes_read: u64,
}

impl<R> ProgressReader<R> {
    fn new(inner: R, reporter: Option<UploadProgressReporter>) -> Self {
        Self {
            inner,
            reporter,
            bytes_read: 0,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for ProgressReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let bytes_before = buf.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);

        if let Poll::Ready(Ok(())) = &result {
            let bytes_after = buf.filled().len();
            if bytes_after > bytes_before {
                self.bytes_read = self
                    .bytes_read
                    .saturating_add((bytes_after - bytes_before) as u64);
                if let Some(reporter) = &self.reporter {
                    reporter.update_telegram_bytes_nowait(self.bytes_read);
                }
            }
        }

        result
    }
}

/// Delete a file (message) from a folder.
pub async fn delete_file(
    state: &AppState,
    message_id: i32,
    folder_id: Option<i64>,
) -> Result<bool, AppError> {
    let client = require_client(state).await?;
    let peer = resolve_peer(&client, folder_id)
        .await
        .map_err(|e| AppError::NotFound(e))?;
    client
        .delete_messages(&peer, &[message_id])
        .await
        .map_err(|e| map_telegram_error(e))?;
    Ok(true)
}

/// Download a file to the given path.
pub async fn download_file(
    state: &AppState,
    bw: &BandwidthManager,
    message_id: i32,
    save_path: &str,
    folder_id: Option<i64>,
) -> Result<String, AppError> {
    let client = require_client(state).await?;
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
            let size = match &media {
                Media::Document(d) => d.size() as u64,
                Media::Photo(_) => 1024 * 1024,
                _ => 0,
            };

            bw.can_transfer(size)?;
            client
                .download_media(&media, save_path)
                .await
                .map_err(|e| map_telegram_error(e))?;
            bw.add_down(size);
            return Ok("Download successful".to_string());
        }
    }
    Err(AppError::NotFound("File not found".to_string()))
}

/// Move files from one folder to another (forward + delete originals).
pub async fn move_files(
    state: &AppState,
    message_ids: &[i32],
    source_folder_id: Option<i64>,
    target_folder_id: Option<i64>,
) -> Result<bool, AppError> {
    if source_folder_id == target_folder_id {
        return Ok(true);
    }
    let client = require_client(state).await?;

    let source_peer = resolve_peer(&client, source_folder_id)
        .await
        .map_err(|e| AppError::NotFound(e))?;
    let target_peer = resolve_peer(&client, target_folder_id)
        .await
        .map_err(|e| AppError::NotFound(e))?;

    client
        .forward_messages(&target_peer, message_ids, &source_peer)
        .await
        .map_err(|e| AppError::Telegram(format!("Forward failed: {}", e)))?;

    client
        .delete_messages(&source_peer, message_ids)
        .await
        .map_err(|e| AppError::Telegram(format!("Delete original failed: {}", e)))?;

    Ok(true)
}

/// List files in a folder (up to 100 messages with media).
pub async fn get_files(
    state: &AppState,
    folder_id: Option<i64>,
) -> Result<Vec<FileMetadata>, AppError> {
    let client = require_client(state).await?;
    let peer = resolve_peer(&client, folder_id)
        .await
        .map_err(|e| AppError::NotFound(e))?;

    let mut files = Vec::new();
    let mut msgs = client.iter_messages(&peer);
    let mut count = 0;

    while let Some(msg) = msgs
        .next()
        .await
        .map_err(|e| AppError::Telegram(e.to_string()))?
    {
        if let Some(doc) = msg.media() {
            let (name, size, mime, ext) = match doc {
                Media::Document(d) => {
                    let n = d.name().to_string();
                    let s = d.size();
                    let m = d.mime_type().map(|s| s.to_string());
                    let e = std::path::Path::new(&n)
                        .extension()
                        .map(|os| os.to_str().unwrap_or("").to_string());
                    (n, s, m, e)
                }
                Media::Photo(_) => (
                    "Photo.jpg".to_string(),
                    0,
                    Some("image/jpeg".into()),
                    Some("jpg".into()),
                ),
                _ => ("Unknown".to_string(), 0, None, None),
            };
            files.push(FileMetadata {
                id: msg.id() as i64,
                folder_id,
                name,
                size: size as u64,
                mime_type: mime,
                file_ext: ext,
                created_at: msg.date().to_string(),
                icon_type: "file".into(),
            });
            count += 1;
        }
        if count > 100 {
            break;
        }
    }

    Ok(files)
}

/// Search for files across all folders using Telegram's SearchGlobal.
pub async fn search_global(state: &AppState, query: &str) -> Result<Vec<FileMetadata>, AppError> {
    let client = require_client(state).await?;

    tracing::info!("Searching global for: {}", query);

    let result = client
        .invoke(&tl::functions::messages::SearchGlobal {
            q: query.to_string(),
            filter: tl::enums::MessagesFilter::InputMessagesFilterDocument,
            min_date: 0,
            max_date: 0,
            offset_rate: 0,
            offset_peer: tl::enums::InputPeer::Empty,
            offset_id: 0,
            limit: 50,
            folder_id: None,
            broadcasts_only: false,
            groups_only: false,
            users_only: false,
        })
        .await
        .map_err(|e| map_telegram_error(e))?;

    let files = extract_files_from_messages(result);
    Ok(files)
}

/// Extract FileMetadata from a SearchGlobal response.
fn extract_files_from_messages(result: tl::enums::messages::Messages) -> Vec<FileMetadata> {
    let messages = match result {
        tl::enums::messages::Messages::Messages(m) => m.messages,
        tl::enums::messages::Messages::Slice(m) => m.messages,
        tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
        _ => return Vec::new(),
    };

    let mut files = Vec::new();
    for msg in messages {
        if let tl::enums::Message::Message(m) = msg {
            if let Some(tl::enums::MessageMedia::Document(d)) = m.media {
                if let Some(tl::enums::Document::Document(doc)) = d.document {
                    let name = doc
                        .attributes
                        .iter()
                        .find_map(|a| match a {
                            tl::enums::DocumentAttribute::Filename(f) => Some(f.file_name.clone()),
                            _ => None,
                        })
                        .unwrap_or_else(|| "Unknown".to_string());
                    let size = doc.size as u64;
                    let mime = doc.mime_type.clone();
                    let ext = std::path::Path::new(&name)
                        .extension()
                        .map(|os| os.to_str().unwrap_or("").to_string());
                    let folder_id = match m.peer_id {
                        tl::enums::Peer::Channel(c) => Some(c.channel_id),
                        tl::enums::Peer::User(u) => Some(u.user_id),
                        tl::enums::Peer::Chat(c) => Some(c.chat_id),
                    };
                    files.push(FileMetadata {
                        id: m.id as i64,
                        folder_id,
                        name,
                        size,
                        mime_type: Some(mime),
                        file_ext: ext,
                        created_at: m.date.to_string(),
                        icon_type: "file".into(),
                    });
                }
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::{sanitize_upload_name, should_send_as_photo};

    #[test]
    fn sanitize_upload_name_removes_path_segments() {
        assert_eq!(sanitize_upload_name("../../tmp/photo.png"), "photo.png");
    }

    #[test]
    fn sanitize_upload_name_falls_back_when_empty() {
        assert_eq!(sanitize_upload_name("   "), "unnamed");
    }

    #[test]
    fn photo_mode_uses_content_type_when_available() {
        assert!(should_send_as_photo("upload.bin", Some("image/jpeg"), true));
    }

    #[test]
    fn photo_mode_uses_extension_when_content_type_missing() {
        assert!(should_send_as_photo("camera-shot.webp", None, true));
    }

    #[test]
    fn photo_mode_rejects_non_images() {
        assert!(!should_send_as_photo("archive.zip", Some("application/zip"), true));
    }

    #[test]
    fn photo_mode_respects_flag() {
        assert!(!should_send_as_photo("photo.jpg", Some("image/jpeg"), false));
    }
}
