use grammers_client::types::Peer;
use grammers_tl_types as tl;

use crate::app_state::AppState;
use crate::domain::models::FolderMetadata;
use crate::errors::{AppError, map_telegram_error};

/// Get the authenticated Telegram client or return an error.
async fn require_client(state: &AppState) -> Result<grammers_client::Client, AppError> {
    state
        .telegram_client
        .lock()
        .await
        .clone()
        .ok_or(AppError::Unauthorized)
}

/// Create a new folder (private Telegram channel with [TD] suffix).
pub async fn create_folder(state: &AppState, name: &str) -> Result<FolderMetadata, AppError> {
    let client = require_client(state).await?;

    tracing::info!("Creating Telegram Channel: {}", name);

    let result = client
        .invoke(&tl::functions::channels::CreateChannel {
            broadcast: true,
            megagroup: false,
            title: format!("{} [TD]", name),
            about: "Telegram Drive Storage Folder\n[telegram-drive-folder]".to_string(),
            geo_point: None,
            address: None,
            for_import: false,
            forum: false,
            ttl_period: None,
        })
        .await
        .map_err(|e| map_telegram_error(e))?;

    let (chat_id, access_hash) = match result {
        tl::enums::Updates::Updates(u) => {
            let chat = u
                .chats
                .first()
                .ok_or(AppError::Telegram("No chat in updates".to_string()))?;
            match chat {
                tl::enums::Chat::Channel(c) => (c.id, c.access_hash.unwrap_or(0)),
                _ => {
                    return Err(AppError::Telegram(
                        "Created chat is not a channel".to_string(),
                    ))
                }
            }
        }
        _ => {
            return Err(AppError::Telegram(
                "Unexpected response (not Updates::Updates)".to_string(),
            ))
        }
    };

    // Disable TTL on the channel
    let _ = client
        .invoke(&tl::functions::messages::SetHistoryTtl {
            peer: tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
                channel_id: chat_id,
                access_hash,
            }),
            period: 0,
        })
        .await;

    Ok(FolderMetadata {
        id: chat_id,
        name: name.to_string(),
        parent_id: None,
    })
}

/// Delete a folder (Telegram channel).
pub async fn delete_folder(state: &AppState, folder_id: i64) -> Result<bool, AppError> {
    let client = require_client(state).await?;

    tracing::info!("Deleting folder/channel: {}", folder_id);

    let peer = crate::services::helpers::resolve_peer(&client, Some(folder_id))
        .await
        .map_err(|e| AppError::NotFound(e))?;

    let input_channel = match peer {
        Peer::Channel(c) => {
            let chan = &c.raw;
            tl::enums::InputChannel::Channel(tl::types::InputChannel {
                channel_id: chan.id,
                access_hash: chan
                    .access_hash
                    .ok_or(AppError::Telegram("No access hash for channel".to_string()))?,
            })
        }
        _ => {
            return Err(AppError::BadRequest(
                "Only channels (folders) can be deleted.".to_string(),
            ))
        }
    };

    client
        .invoke(&tl::functions::channels::DeleteChannel {
            channel: input_channel,
        })
        .await
        .map_err(|e| AppError::Telegram(format!("Failed to delete channel: {}", e)))?;

    Ok(true)
}

/// Scan all dialogs for channels that are Telegram Drive folders.
/// Detection: title contains "[TD]" or about contains "[telegram-drive-folder]".
pub async fn scan_folders(state: &AppState) -> Result<Vec<FolderMetadata>, AppError> {
    let client = require_client(state).await?;

    let mut folders = Vec::new();
    let mut dialogs = client.iter_dialogs();

    tracing::info!("Starting Folder Scan...");

    while let Some(dialog) = dialogs
        .next()
        .await
        .map_err(|e| AppError::Telegram(e.to_string()))?
    {
        match &dialog.peer {
            Peer::Channel(c) => {
                let id = c.raw.id;
                let name = c.raw.title.clone();
                let access_hash = c.raw.access_hash.unwrap_or(0);

                tracing::debug!("[SCAN] Processing Channel: '{}' (ID: {})", name, id);

                // Strategy 1: Title contains [TD]
                if name.to_lowercase().contains("[td]") {
                    tracing::info!(" -> MATCH via Title: {}", name);
                    let display_name = name
                        .replace(" [TD]", "")
                        .replace(" [td]", "")
                        .replace("[TD]", "")
                        .replace("[td]", "")
                        .trim()
                        .to_string();
                    folders.push(FolderMetadata {
                        id,
                        name: display_name,
                        parent_id: None,
                    });
                    continue;
                }

                // Strategy 2: About field contains marker
                let input_chan =
                    tl::enums::InputChannel::Channel(tl::types::InputChannel {
                        channel_id: c.raw.id,
                        access_hash,
                    });

                match client
                    .invoke(&tl::functions::channels::GetFullChannel {
                        channel: input_chan,
                    })
                    .await
                {
                    Ok(tl::enums::messages::ChatFull::Full(f)) => {
                        if let tl::enums::ChatFull::Full(cf) = f.full_chat {
                            if cf.about.contains("[telegram-drive-folder]") {
                                tracing::info!(" -> MATCH via About: {}", name);
                                folders.push(FolderMetadata {
                                    id,
                                    name: name.clone(),
                                    parent_id: None,
                                });
                            }
                        }
                    }
                    Err(e) => tracing::warn!(" -> Failed to get full info: {}", e),
                }
            }
            peer => {
                tracing::debug!("[SCAN] Skipped Peer: {:?}", peer);
            }
        }
    }

    tracing::info!("Scan complete. Found {} folders.", folders.len());
    Ok(folders)
}
