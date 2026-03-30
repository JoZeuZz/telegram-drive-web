use grammers_client::types::Media;
use grammers_client::InputMessage;
use grammers_tl_types as tl;

use crate::app_state::AppState;
use crate::domain::models::FileMetadata;
use crate::errors::{AppError, map_telegram_error};
use crate::services::bandwidth::BandwidthManager;
use crate::services::helpers::resolve_peer;

/// Get the authenticated Telegram client or return an error.
async fn require_client(state: &AppState) -> Result<grammers_client::Client, AppError> {
    state
        .telegram_client
        .lock()
        .await
        .clone()
        .ok_or(AppError::Unauthorized)
}

/// Upload a file to a folder (channel) or Saved Messages.
pub async fn upload_file(
    state: &AppState,
    bw: &BandwidthManager,
    path: &str,
    folder_id: Option<i64>,
) -> Result<String, AppError> {
    let size = std::fs::metadata(path)
        .map_err(|e| AppError::BadRequest(format!("Cannot read file: {}", e)))?
        .len();
    bw.can_transfer(size)?;

    let client = require_client(state).await?;

    let path_owned = path.to_string();
    let client_clone = client.clone();

    let uploaded_file = tokio::spawn(async move {
        client_clone.upload_file(&path_owned).await
    })
    .await
    .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?
    .map_err(|e| map_telegram_error(e))?;

    let message = InputMessage::new().text("").file(uploaded_file);
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
    while let Some(m) = msgs.next().await.map_err(|e| AppError::Telegram(e.to_string()))? {
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

    while let Some(msg) = msgs.next().await.map_err(|e| AppError::Telegram(e.to_string()))? {
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
pub async fn search_global(
    state: &AppState,
    query: &str,
) -> Result<Vec<FileMetadata>, AppError> {
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
                            tl::enums::DocumentAttribute::Filename(f) => {
                                Some(f.file_name.clone())
                            }
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
