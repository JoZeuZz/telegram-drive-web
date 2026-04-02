use grammers_client::types::Media;
use grammers_client::InputMessage;
use grammers_session::types::{PeerAuth, PeerId, PeerRef};
use grammers_tl_types as tl;
use rand::Rng;
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

fn sanitize_topic_top_message(topic_top_message: Option<i32>) -> Option<i32> {
    topic_top_message.filter(|value| *value > 0)
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
        Some("jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tif" | "tiff" | "heic" | "heif")
    )
}

fn document_metadata_from_raw(
    message_id: i32,
    folder_id: Option<i64>,
    date: i32,
    doc_media: tl::types::MessageMediaDocument,
) -> Option<FileMetadata> {
    let document = doc_media.document?;
    let raw_doc = match document {
        tl::enums::Document::Document(doc) => doc,
        tl::enums::Document::Empty(_) => return None,
    };

    let name = raw_doc
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            tl::enums::DocumentAttribute::Filename(file) => Some(file.file_name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| format!("file_{}", message_id));

    let ext = std::path::Path::new(&name)
        .extension()
        .map(|os| os.to_str().unwrap_or("").to_string());

    Some(FileMetadata {
        id: message_id as i64,
        folder_id,
        name,
        size: raw_doc.size as u64,
        mime_type: Some(raw_doc.mime_type),
        file_ext: ext,
        created_at: date.to_string(),
        icon_type: "file".into(),
    })
}

fn photo_metadata_from_raw(message_id: i32, folder_id: Option<i64>, date: i32) -> FileMetadata {
    FileMetadata {
        id: message_id as i64,
        folder_id,
        name: format!("Photo_{}.jpg", message_id),
        size: 0,
        mime_type: Some("image/jpeg".to_string()),
        file_ext: Some("jpg".to_string()),
        created_at: date.to_string(),
        icon_type: "file".into(),
    }
}

fn extract_topic_files_from_messages(
    result: tl::enums::messages::Messages,
    folder_id: Option<i64>,
) -> Vec<FileMetadata> {
    let messages = match result {
        tl::enums::messages::Messages::Messages(m) => m.messages,
        tl::enums::messages::Messages::Slice(m) => m.messages,
        tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
        _ => return Vec::new(),
    };

    let mut files = Vec::new();

    for raw in messages {
        if let tl::enums::Message::Message(message) = raw {
            if let Some(media) = message.media {
                match media {
                    tl::enums::MessageMedia::Document(document) => {
                        if let Some(metadata) = document_metadata_from_raw(
                            message.id,
                            folder_id,
                            message.date,
                            document,
                        ) {
                            files.push(metadata);
                        }
                    }
                    tl::enums::MessageMedia::Photo(_) => {
                        files.push(photo_metadata_from_raw(message.id, folder_id, message.date));
                    }
                    _ => {}
                }
            }
        }
    }

    files
}

async fn resolve_topic_top_message(
    state: &AppState,
    forum_id: i64,
    topic_id: i32,
) -> Result<i32, AppError> {
    let topics = crate::services::telegram_forums::list_topics(state, forum_id).await?;
    topics
        .into_iter()
        .find(|topic| topic.id == topic_id)
        .map(|topic| topic.top_message)
        .ok_or(AppError::NotFound(format!(
            "Topic {} not found in structured folder {}",
            topic_id, forum_id
        )))
}

async fn resolve_topic_top_message_with_hint(
    state: &AppState,
    forum_id: i64,
    topic_id: i32,
    topic_top_message: Option<i32>,
) -> Result<i32, AppError> {
    if let Some(top_message) = sanitize_topic_top_message(topic_top_message) {
        return Ok(top_message);
    }

    resolve_topic_top_message(state, forum_id, topic_id).await
}

fn is_rate_limited_telegram_error(error: &AppError) -> bool {
    matches!(
        error,
        AppError::Telegram(message) if message.starts_with("Rate limited by Telegram")
    )
}

fn is_invalid_topic_reference_error(error: &AppError) -> bool {
    matches!(error, AppError::Telegram(message)
        if message.contains("MSG_ID_INVALID")
            || message.contains("TOPIC_ID_INVALID")
            || message.contains("TOPIC_DELETED")
            || message.contains("REPLY_MESSAGE_ID_INVALID"))
}

fn push_unique_topic_msg_id(candidates: &mut Vec<i32>, msg_id: i32) {
    if msg_id > 0 && !candidates.contains(&msg_id) {
        candidates.push(msg_id);
    }
}

async fn search_topic_files(
    client: &grammers_client::Client,
    input_peer: tl::enums::InputPeer,
    forum_id: i64,
    top_message: i32,
) -> Result<Vec<FileMetadata>, AppError> {
    let result = client
        .invoke(&tl::functions::messages::Search {
            peer: input_peer,
            q: String::new(),
            from_id: None,
            saved_peer_id: None,
            saved_reaction: None,
            top_msg_id: Some(top_message),
            filter: tl::enums::MessagesFilter::InputMessagesFilterEmpty,
            min_date: 0,
            max_date: 0,
            offset_id: 0,
            add_offset: 0,
            limit: 100,
            max_id: 0,
            min_id: 0,
            hash: 0,
        })
        .await
        .map_err(map_telegram_error)?;

    let mut files = extract_topic_files_from_messages(result, Some(forum_id));
    files.retain(|item| item.id != i64::from(top_message));
    Ok(files)
}

async fn get_topic_files(
    state: &AppState,
    forum_id: i64,
    topic_id: i32,
    topic_top_message: Option<i32>,
) -> Result<Vec<FileMetadata>, AppError> {
    let client = require_client(state).await?;
    let input_peer = crate::services::telegram_forums::resolve_forum_input_peer(state, forum_id)
        .await?;

    let mut msg_id_candidates = Vec::new();
    let hint_top_message = sanitize_topic_top_message(topic_top_message);
    if let Some(top_message) = hint_top_message {
        push_unique_topic_msg_id(&mut msg_id_candidates, top_message);
    }
    // Telegram forum topic ids are often valid thread roots as well.
    push_unique_topic_msg_id(&mut msg_id_candidates, topic_id);

    let mut last_invalid_error: Option<AppError> = None;

    for msg_id in msg_id_candidates {
        let result = client
            .invoke(&tl::functions::messages::GetReplies {
                peer: input_peer.clone(),
                msg_id,
                offset_id: 0,
                offset_date: 0,
                add_offset: 0,
                limit: 100,
                max_id: 0,
                min_id: 0,
                hash: 0,
            })
            .await;

        let result = match result {
            Ok(result) => result,
            Err(error) => {
                let mapped = map_telegram_error(error);
                if is_rate_limited_telegram_error(&mapped) {
                    tracing::warn!(
                        forum_id,
                        topic_id,
                        msg_id,
                        "GetReplies rate limited, retrying topic listing with Search(top_msg_id)"
                    );
                    return search_topic_files(&client, input_peer, forum_id, msg_id).await;
                }
                if is_invalid_topic_reference_error(&mapped) {
                    tracing::debug!(
                        forum_id,
                        topic_id,
                        msg_id,
                        error = %mapped,
                        "Invalid topic reference while listing files; trying next candidate"
                    );
                    last_invalid_error = Some(mapped);
                    continue;
                }
                return Err(mapped);
            }
        };

        let mut files = extract_topic_files_from_messages(result, Some(forum_id));
        files.retain(|item| item.id != i64::from(msg_id));
        return Ok(files);
    }

    let resolved_top_message = resolve_topic_top_message_with_hint(state, forum_id, topic_id, None).await?;
    let result = client
        .invoke(&tl::functions::messages::GetReplies {
            peer: input_peer.clone(),
            msg_id: resolved_top_message,
            offset_id: 0,
            offset_date: 0,
            add_offset: 0,
            limit: 100,
            max_id: 0,
            min_id: 0,
            hash: 0,
        })
        .await;

    let result = match result {
        Ok(result) => result,
        Err(error) => {
            let mapped = map_telegram_error(error);
            if is_rate_limited_telegram_error(&mapped) {
                tracing::warn!(
                    forum_id,
                    topic_id,
                    resolved_top_message,
                    "GetReplies rate limited after top_message refresh, retrying with Search(top_msg_id)"
                );
                return search_topic_files(&client, input_peer, forum_id, resolved_top_message)
                    .await;
            }
            if is_invalid_topic_reference_error(&mapped) {
                return Err(last_invalid_error.unwrap_or(mapped));
            }
            return Err(mapped);
        }
    };

    let mut files = extract_topic_files_from_messages(result, Some(forum_id));
    files.retain(|item| item.id != i64::from(resolved_top_message));
    Ok(files)
}

fn input_peer_to_peer_ref(input_peer: tl::enums::InputPeer) -> Result<PeerRef, AppError> {
    match input_peer {
        tl::enums::InputPeer::Empty => Err(AppError::BadRequest(
            "Invalid peer resolution for move operation".to_string(),
        )),
        tl::enums::InputPeer::Channel(channel) => Ok(PeerRef {
            id: PeerId::channel(channel.channel_id),
            auth: PeerAuth::from_hash(channel.access_hash),
        }),
        tl::enums::InputPeer::Chat(chat) => Ok(PeerRef {
            id: PeerId::chat(chat.chat_id),
            auth: PeerAuth::default(),
        }),
        tl::enums::InputPeer::User(user) => Ok(PeerRef {
            id: PeerId::user(user.user_id),
            auth: PeerAuth::from_hash(user.access_hash),
        }),
        tl::enums::InputPeer::PeerSelf => Ok(PeerRef {
            id: grammers_session::types::PeerId::self_user(),
            auth: PeerAuth::default(),
        }),
        tl::enums::InputPeer::UserFromMessage(user) => Ok(PeerRef {
            id: PeerId::user(user.user_id),
            auth: PeerAuth::default(),
        }),
        tl::enums::InputPeer::ChannelFromMessage(channel) => Ok(PeerRef {
            id: PeerId::channel(channel.channel_id),
            auth: PeerAuth::default(),
        }),
    }
}

async fn resolve_move_input_peer(
    state: &AppState,
    client: &grammers_client::Client,
    folder_id: Option<i64>,
    topic_id: Option<i32>,
    field_name: &str,
) -> Result<tl::enums::InputPeer, AppError> {
    if topic_id.is_some() {
        let forum_id = folder_id.ok_or(AppError::BadRequest(format!(
            "{} requires folder_id (structured folder root)",
            field_name
        )))?;
        return crate::services::telegram_forums::resolve_forum_input_peer(state, forum_id).await;
    }

    let peer = resolve_peer(client, folder_id)
        .await
        .map_err(AppError::NotFound)?;
    let peer_ref = PeerRef::from(peer);
    Ok(peer_ref.into())
}

/// Upload a file to a folder (channel) or Saved Messages.
pub async fn upload_file(
    state: &AppState,
    bw: &BandwidthManager,
    path: &str,
    folder_id: Option<i64>,
    topic_id: Option<i32>,
    topic_top_message: Option<i32>,
    file_name: &str,
    content_type: Option<&str>,
    as_photo: bool,
    progress_reporter: Option<UploadProgressReporter>,
) -> Result<String, AppError> {
    let size = std::fs::metadata(path)
        .map_err(|e| AppError::BadRequest(format!("Cannot read file: {}", e)))?
        .len();
    let limit = state.effective_daily_bandwidth_limit_bytes().await;
    bw.can_transfer_with_limit(size, limit)?;

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

    if let Some(topic_id) = topic_id {
        let forum_id = folder_id.ok_or(AppError::BadRequest(
            "topic_id requires folder_id (structured folder root)".to_string(),
        ))?;
        let top_message =
            resolve_topic_top_message_with_hint(state, forum_id, topic_id, topic_top_message)
                .await?;
        message = message.reply_to(Some(top_message));

        let input_peer = crate::services::telegram_forums::resolve_forum_input_peer(state, forum_id)
            .await?;
        let peer_ref = input_peer_to_peer_ref(input_peer)?;

        client
            .send_message(peer_ref, message)
            .await
            .map_err(map_telegram_error)?;

        bw.add_up(size);
        return Ok("File uploaded successfully".to_string());
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

            let limit = state.effective_daily_bandwidth_limit_bytes().await;
            bw.can_transfer_with_limit(size, limit)?;
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
    source_topic_id: Option<i32>,
    target_folder_id: Option<i64>,
    target_topic_id: Option<i32>,
    target_topic_top_message: Option<i32>,
) -> Result<bool, AppError> {
    let source_topic_id = source_topic_id.filter(|value| *value > 0);
    let target_topic_id = target_topic_id.filter(|value| *value > 0);
    let target_topic_top_message = sanitize_topic_top_message(target_topic_top_message);

    if source_folder_id == target_folder_id && source_topic_id == target_topic_id {
        return Ok(true);
    }

    if message_ids.is_empty() {
        return Ok(true);
    }

    let client = require_client(state).await?;

    let source_input_peer = resolve_move_input_peer(
        state,
        &client,
        source_folder_id,
        source_topic_id,
        "source_topic_id",
    )
    .await?;

    let target_input_peer = resolve_move_input_peer(
        state,
        &client,
        target_folder_id,
        target_topic_id,
        "target_topic_id",
    )
    .await?;

    let source_peer = input_peer_to_peer_ref(source_input_peer.clone())?;
    let target_peer = input_peer_to_peer_ref(target_input_peer.clone())?;

    if let Some(target_topic_id) = target_topic_id {
        let forum_id = target_folder_id.ok_or(AppError::BadRequest(
            "target_topic_id requires folder_id (structured folder root)".to_string(),
        ))?;
        let top_message = resolve_topic_top_message_with_hint(
            state,
            forum_id,
            target_topic_id,
            target_topic_top_message,
        )
        .await?;

        let mut rng = rand::thread_rng();
        let random_id: Vec<i64> = (0..message_ids.len()).map(|_| rng.gen()).collect();

        let request = tl::functions::messages::ForwardMessages {
            silent: false,
            background: false,
            with_my_score: false,
            drop_author: false,
            drop_media_captions: false,
            from_peer: source_input_peer,
            id: message_ids.to_vec(),
            random_id,
            to_peer: target_input_peer,
            top_msg_id: Some(top_message),
            reply_to: None,
            schedule_date: None,
            schedule_repeat_period: None,
            send_as: None,
            noforwards: false,
            quick_reply_shortcut: None,
            allow_paid_floodskip: false,
            video_timestamp: None,
            allow_paid_stars: None,
            suggested_post: None,
        };

        client
            .invoke(&request)
            .await
            .map_err(map_telegram_error)?;
    } else {
        client
            .forward_messages(target_peer, message_ids, source_peer)
            .await
            .map_err(|e| AppError::Telegram(format!("Forward failed: {}", e)))?;
    }

    client
        .delete_messages(source_peer, message_ids)
        .await
        .map_err(|e| AppError::Telegram(format!("Delete original failed: {}", e)))?;

    Ok(true)
}

/// List files in a folder (up to 100 messages with media).
pub async fn get_files(
    state: &AppState,
    folder_id: Option<i64>,
    topic_id: Option<i32>,
    topic_top_message: Option<i32>,
) -> Result<Vec<FileMetadata>, AppError> {
    if let Some(topic_id) = topic_id {
        let forum_id = folder_id.ok_or(AppError::BadRequest(
            "topic_id requires folder_id (structured folder root)".to_string(),
        ))?;
        return get_topic_files(state, forum_id, topic_id, topic_top_message).await;
    }

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
    use super::{
        is_invalid_topic_reference_error, push_unique_topic_msg_id, sanitize_upload_name,
        should_send_as_photo,
    };
    use crate::errors::AppError;

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
        assert!(!should_send_as_photo(
            "archive.zip",
            Some("application/zip"),
            true
        ));
    }

    #[test]
    fn photo_mode_respects_flag() {
        assert!(!should_send_as_photo(
            "photo.jpg",
            Some("image/jpeg"),
            false
        ));
    }

    #[test]
    fn topic_msg_id_candidates_are_unique_and_positive() {
        let mut candidates = vec![];

        push_unique_topic_msg_id(&mut candidates, 0);
        push_unique_topic_msg_id(&mut candidates, -3);
        push_unique_topic_msg_id(&mut candidates, 11);
        push_unique_topic_msg_id(&mut candidates, 11);
        push_unique_topic_msg_id(&mut candidates, 12);

        assert_eq!(candidates, vec![11, 12]);
    }

    #[test]
    fn detects_invalid_topic_reference_errors() {
        assert!(is_invalid_topic_reference_error(&AppError::Telegram(
            "RPC error: MSG_ID_INVALID".to_string()
        )));
        assert!(is_invalid_topic_reference_error(&AppError::Telegram(
            "RPC error: TOPIC_ID_INVALID".to_string()
        )));
        assert!(!is_invalid_topic_reference_error(&AppError::Telegram(
            "Rate limited by Telegram. Retry later.".to_string()
        )));
    }
}
