use futures::stream::Stream;
use grammers_client::types::Media;

use crate::app_state::AppState;
use crate::errors::AppError;
use crate::services::helpers::resolve_peer;

/// Metadata about a streamable media file.
pub struct StreamInfo {
    pub mime_type: String,
    pub size: i64,
    pub file_name: String,
}

/// Prepare a media file for streaming.
/// Returns metadata and an async byte-chunk stream.
pub async fn prepare_stream(
    state: &AppState,
    folder_id: Option<i64>,
    message_id: i32,
) -> Result<(StreamInfo, impl Stream<Item = Result<Vec<u8>, String>>), AppError> {
    let client = state
        .telegram_client
        .lock()
        .await
        .clone()
        .ok_or(AppError::Unauthorized)?;

    let peer = resolve_peer(&client, folder_id)
        .await
        .map_err(|e| AppError::NotFound(e))?;

    let messages = client
        .get_messages_by_id(peer, &[message_id])
        .await
        .map_err(|e| AppError::Telegram(format!("Failed to fetch message: {}", e)))?;

    let msg = messages
        .into_iter()
        .next()
        .flatten()
        .ok_or(AppError::NotFound("Message not found".to_string()))?;

    let media = msg
        .media()
        .ok_or(AppError::NotFound("No media in message".to_string()))?;

    let size = match &media {
        Media::Document(d) => d.size(),
        _ => 0,
    };

    let mime = mime_type_from_media(&media);

    let file_name = match &media {
        Media::Document(d) => {
            let n = d.name().to_string();
            if n.is_empty() {
                "download.bin".to_string()
            } else {
                n
            }
        }
        Media::Photo(_) => "photo.jpg".to_string(),
        _ => "file.bin".to_string(),
    };

    let mut download_iter = client.iter_download(&media);
    let stream = async_stream::stream! {
        while let Some(chunk) = download_iter.next().await.transpose() {
            match chunk {
                Ok(bytes) => yield Ok(bytes),
                Err(e) => {
                    tracing::error!("Stream error: {}", e);
                    yield Err(e.to_string());
                    break;
                }
            }
        }
    };

    let info = StreamInfo {
        mime_type: mime,
        size,
        file_name,
    };

    Ok((info, stream))
}

/// Determine MIME type from a media object.
pub fn mime_type_from_media(media: &Media) -> String {
    match media {
        Media::Document(d) => d
            .mime_type()
            .unwrap_or("application/octet-stream")
            .to_string(),
        _ => "application/octet-stream".to_string(),
    }
}
