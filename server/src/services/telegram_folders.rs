use std::collections::HashMap;

use grammers_client::types::Peer;
use grammers_tl_types as tl;

use crate::app_state::AppState;
use crate::domain::models::FolderMetadata;
use crate::errors::{AppError, map_telegram_error};

const FOLDER_MARKER: &str = "[telegram-drive-folder]";
const FOLDER_SCHEMA_LINE: &str = "td_schema=1";
const FOLDER_PARENT_PREFIX: &str = "td_parent_id=";

fn build_folder_about(parent_id: Option<i64>) -> String {
    let parent_value = parent_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "null".to_string());

    format!(
        "Telegram Drive Storage Folder\n{}\n{}\n{}{}",
        FOLDER_MARKER, FOLDER_SCHEMA_LINE, FOLDER_PARENT_PREFIX, parent_value
    )
}

fn display_folder_name(raw_title: &str) -> String {
    raw_title
        .replace(" [TD]", "")
        .replace(" [td]", "")
        .replace("[TD]", "")
        .replace("[td]", "")
        .trim()
        .to_string()
}

fn parse_parent_id_from_about(about: &str) -> Option<i64> {
    for line in about.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix(FOLDER_PARENT_PREFIX) {
            if value.is_empty() || value.eq_ignore_ascii_case("null") {
                return None;
            }
            if let Ok(parent_id) = value.parse::<i64>() {
                if parent_id > 0 {
                    return Some(parent_id);
                }
            }
            return None;
        }
    }

    None
}

fn collect_cascade_delete_order(
    root_id: i64,
    children_by_parent: &HashMap<i64, Vec<i64>>,
) -> Vec<i64> {
    let mut depth_by_id: HashMap<i64, usize> = HashMap::new();
    let mut stack = vec![(root_id, 0usize)];

    while let Some((folder_id, depth)) = stack.pop() {
        if depth_by_id.contains_key(&folder_id) {
            continue;
        }

        depth_by_id.insert(folder_id, depth);

        if let Some(children) = children_by_parent.get(&folder_id) {
            for child_id in children {
                stack.push((*child_id, depth + 1));
            }
        }
    }

    let mut ordered: Vec<(i64, usize)> = depth_by_id.into_iter().collect();
    ordered.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ordered.into_iter().map(|(id, _)| id).collect()
}

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
pub async fn create_folder(
    state: &AppState,
    name: &str,
    parent_id: Option<i64>,
) -> Result<FolderMetadata, AppError> {
    let client = require_client(state).await?;

    tracing::info!("Creating Telegram Channel: {}", name);

    let result = client
        .invoke(&tl::functions::channels::CreateChannel {
            broadcast: true,
            megagroup: false,
            title: format!("{} [TD]", name),
            about: build_folder_about(parent_id),
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
        parent_id,
    })
}

async fn delete_folder_channel(state: &AppState, folder_id: i64) -> Result<(), AppError> {
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

    Ok(())
}

/// Delete a folder branch in cascade order (children first, then parent).
pub async fn delete_folder(state: &AppState, folder_id: i64) -> Result<usize, AppError> {
    let folders = scan_folders(state).await?;

    // If the folder is not in scan results (stale UI), fallback to direct delete.
    if !folders.iter().any(|f| f.id == folder_id) {
        delete_folder_channel(state, folder_id).await?;
        return Ok(1);
    }

    let mut children_by_parent: HashMap<i64, Vec<i64>> = HashMap::new();
    for folder in folders {
        if let Some(parent_id) = folder.parent_id {
            children_by_parent
                .entry(parent_id)
                .or_default()
                .push(folder.id);
        }
    }

    let delete_order = collect_cascade_delete_order(folder_id, &children_by_parent);
    let mut deleted_count = 0usize;

    for id in delete_order {
        match delete_folder_channel(state, id).await {
            Ok(()) => deleted_count += 1,
            Err(AppError::NotFound(_)) => {
                tracing::warn!("Folder {} not found during cascade delete; skipping", id)
            }
            Err(err) => return Err(err),
        }
    }

    Ok(deleted_count)
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
                let title_is_folder = name.to_lowercase().contains("[td]");

                tracing::debug!("[SCAN] Processing Channel: '{}' (ID: {})", name, id);

                let about = if access_hash == 0 {
                    None
                } else {
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
                            match f.full_chat {
                                tl::enums::ChatFull::Full(cf) => Some(cf.about),
                                _ => None,
                            }
                        }
                        Err(e) => {
                            tracing::warn!(" -> Failed to get full info: {}", e);
                            None
                        }
                    }
                };

                let about_is_folder = about
                    .as_deref()
                    .map(|value| value.contains(FOLDER_MARKER))
                    .unwrap_or(false);

                if title_is_folder || about_is_folder {
                    let parent_id = about.as_deref().and_then(parse_parent_id_from_about);
                    let display_name = display_folder_name(&name);

                    tracing::info!(
                        " -> MATCH (title={}, about={}) {}",
                        title_is_folder,
                        about_is_folder,
                        display_name
                    );

                    folders.push(FolderMetadata {
                        id,
                        name: display_name,
                        parent_id,
                    });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_parent_id_from_about_handles_values() {
        let root_about = "Telegram Drive Storage Folder\n[telegram-drive-folder]\ntd_schema=1\ntd_parent_id=null";
        let child_about =
            "Telegram Drive Storage Folder\n[telegram-drive-folder]\ntd_schema=1\ntd_parent_id=12345";
        let invalid_about =
            "Telegram Drive Storage Folder\n[telegram-drive-folder]\ntd_schema=1\ntd_parent_id=abc";

        assert_eq!(parse_parent_id_from_about(root_about), None);
        assert_eq!(parse_parent_id_from_about(child_about), Some(12345));
        assert_eq!(parse_parent_id_from_about(invalid_about), None);
    }

    #[test]
    fn build_folder_about_includes_expected_metadata() {
        let about = build_folder_about(Some(42));
        assert!(about.contains(FOLDER_MARKER));
        assert!(about.contains(FOLDER_SCHEMA_LINE));
        assert!(about.contains("td_parent_id=42"));

        let root_about = build_folder_about(None);
        assert!(root_about.contains("td_parent_id=null"));
    }

    #[test]
    fn collect_cascade_delete_order_is_deepest_first() {
        let mut map: HashMap<i64, Vec<i64>> = HashMap::new();
        map.insert(1, vec![2, 3]);
        map.insert(2, vec![4]);

        let order = collect_cascade_delete_order(1, &map);
        assert_eq!(order, vec![4, 2, 3, 1]);
    }
}
