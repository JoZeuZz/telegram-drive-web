use std::collections::{HashMap, HashSet};
use std::time::Duration;

use grammers_client::types::Peer;
use grammers_tl_types as tl;

use crate::app_state::AppState;
use crate::domain::models::FolderMetadata;
use crate::errors::{map_telegram_error, AppError};
use crate::services::helpers::parse_flood_wait;

const FOLDER_MARKER: &str = "[telegram-drive-folder]";
const FOLDER_SCHEMA_LINE: &str = "td_schema=1";
const FOLDER_PARENT_PREFIX: &str = "td_parent_id=";
const LEGACY_TITLE_MARKER: &str = "[td]";
const TITLE_METADATA_PREFIX: &str = " [TD|";
const TITLE_SCHEMA_PREFIX: &str = "s=";
const TITLE_PARENT_PREFIX: &str = "p=";
const TITLE_SCHEMA_VERSION: &str = "1";
const FALLBACK_MAX_RETRIES: usize = 2;
const FALLBACK_BASE_DELAY_MS: u64 = 200;

#[derive(Debug, Clone, Default)]
pub struct FolderSyncReport {
    pub folders: Vec<FolderMetadata>,
    pub resolved_by_title: usize,
    pub resolved_by_about: usize,
    pub orphans: usize,
    pub migrated: usize,
}

enum AboutLookupResult {
    Success(Option<String>),
    FloodWait(i64),
    Failed,
}

fn parent_value_string(parent_id: Option<i64>) -> String {
    parent_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn build_folder_about(parent_id: Option<i64>) -> String {
    let parent_value = parent_value_string(parent_id);

    format!(
        "Telegram Drive Storage Folder\n{}\n{}\n{}{}",
        FOLDER_MARKER, FOLDER_SCHEMA_LINE, FOLDER_PARENT_PREFIX, parent_value
    )
}

fn build_folder_title(name: &str, parent_id: Option<i64>) -> String {
    let normalized = normalize_folder_name(name);
    format!(
        "{} [TD|s={}|p={}]",
        normalized,
        TITLE_SCHEMA_VERSION,
        parent_value_string(parent_id)
    )
}

fn normalize_folder_name(raw_name: &str) -> String {
    let cleaned = display_folder_name(raw_name);
    if cleaned.trim().is_empty() {
        "Unnamed".to_string()
    } else {
        cleaned
    }
}

fn parse_parent_value(value: &str) -> Option<Option<i64>> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
        return Some(None);
    }

    let parent_id = trimmed.parse::<i64>().ok()?;
    if parent_id > 0 {
        Some(Some(parent_id))
    } else {
        None
    }
}

fn parse_parent_id_from_title(raw_title: &str) -> Option<Option<i64>> {
    let trimmed = raw_title.trim();
    let (_, metadata_with_bracket) = trimmed.rsplit_once(TITLE_METADATA_PREFIX)?;
    let metadata = metadata_with_bracket.strip_suffix(']')?;

    let mut schema_matches = false;
    let mut parent_id = None;

    for segment in metadata.split('|') {
        let segment = segment.trim();
        if let Some(value) = segment.strip_prefix(TITLE_SCHEMA_PREFIX) {
            schema_matches = value.trim() == TITLE_SCHEMA_VERSION;
            continue;
        }

        if let Some(value) = segment.strip_prefix(TITLE_PARENT_PREFIX) {
            parent_id = parse_parent_value(value);
        }
    }

    if !schema_matches {
        return None;
    }

    parent_id
}

fn is_legacy_folder_title(raw_title: &str) -> bool {
    raw_title
        .to_ascii_lowercase()
        .contains(LEGACY_TITLE_MARKER)
}

fn strip_title_metadata(raw_title: &str) -> String {
    let trimmed = raw_title.trim();
    if parse_parent_id_from_title(trimmed).is_some() {
        if let Some((name, _)) = trimmed.rsplit_once(TITLE_METADATA_PREFIX) {
            return name.trim().to_string();
        }
    }

    trimmed.to_string()
}

fn display_folder_name(raw_title: &str) -> String {
    strip_title_metadata(raw_title)
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
            return parse_parent_value(value).unwrap_or(None);
        }
    }

    None
}

async fn fetch_channel_about_with_retry(
    client: &grammers_client::Client,
    channel_id: i64,
    access_hash: i64,
) -> AboutLookupResult {
    let mut backoff_ms = FALLBACK_BASE_DELAY_MS;

    for attempt in 0..=FALLBACK_MAX_RETRIES {
        let input_chan = tl::enums::InputChannel::Channel(tl::types::InputChannel {
            channel_id,
            access_hash,
        });

        match client
            .invoke(&tl::functions::channels::GetFullChannel {
                channel: input_chan,
            })
            .await
        {
            Ok(tl::enums::messages::ChatFull::Full(full)) => {
                let about = match full.full_chat {
                    tl::enums::ChatFull::Full(chat_full) => Some(chat_full.about),
                    _ => None,
                };
                return AboutLookupResult::Success(about);
            }
            Err(error) => {
                let error_string = error.to_string();
                if let Some(wait_secs) = parse_flood_wait(&error_string) {
                    tracing::warn!(
                        "GetFullChannel FLOOD_WAIT for channel {} ({}s)",
                        channel_id,
                        wait_secs
                    );
                    return AboutLookupResult::FloodWait(wait_secs);
                }

                if attempt >= FALLBACK_MAX_RETRIES {
                    tracing::warn!(
                        "GetFullChannel failed for channel {} after {} attempts: {}",
                        channel_id,
                        FALLBACK_MAX_RETRIES + 1,
                        error_string
                    );
                    return AboutLookupResult::Failed;
                }

                tracing::debug!(
                    "GetFullChannel retry {}/{} for channel {} after error: {}",
                    attempt + 1,
                    FALLBACK_MAX_RETRIES,
                    channel_id,
                    error_string
                );
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = backoff_ms.saturating_mul(2);
            }
        }
    }

    AboutLookupResult::Failed
}

async fn migrate_title_metadata(
    client: &grammers_client::Client,
    channel_id: i64,
    access_hash: i64,
    folder_name: &str,
    parent_id: Option<i64>,
) -> bool {
    let input_channel = tl::enums::InputChannel::Channel(tl::types::InputChannel {
        channel_id,
        access_hash,
    });
    let new_title = build_folder_title(folder_name, parent_id);

    match client
        .invoke(&tl::functions::channels::EditTitle {
            channel: input_channel,
            title: new_title,
        })
        .await
    {
        Ok(_) => true,
        Err(error) => {
            tracing::warn!(
                "Failed to migrate title metadata for channel {}: {}",
                channel_id,
                error
            );
            false
        }
    }
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
    let normalized_name = normalize_folder_name(name);
    let title = build_folder_title(&normalized_name, parent_id);

    tracing::info!("Creating Telegram Channel: {}", normalized_name);

    let result = client
        .invoke(&tl::functions::channels::CreateChannel {
            broadcast: true,
            megagroup: false,
            title,
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
        name: normalized_name,
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
/// Detection: title metadata is primary; legacy `[TD]` title marker is supported
/// and uses `about` as fallback for parent recovery and lazy migration.
pub async fn scan_folders(state: &AppState) -> Result<Vec<FolderMetadata>, AppError> {
    Ok(scan_folders_with_report(state).await?.folders)
}

pub async fn scan_folders_with_report(state: &AppState) -> Result<FolderSyncReport, AppError> {
    let client = require_client(state).await?;

    let mut folders = Vec::new();
    let mut dialogs = client.iter_dialogs();
    let mut resolved_by_title = 0usize;
    let mut resolved_by_about = 0usize;
    let mut migrated = 0usize;
    let mut about_fallback_enabled = true;

    tracing::info!("Starting Folder Scan...");

    while let Some(dialog) = dialogs
        .next()
        .await
        .map_err(|e| AppError::Telegram(e.to_string()))?
    {
        match &dialog.peer {
            Peer::Channel(c) => {
                let id = c.raw.id;
                let raw_title = c.raw.title.clone();
                let access_hash = c.raw.access_hash.unwrap_or(0);
                let title_parent = parse_parent_id_from_title(&raw_title);
                let title_is_folder = title_parent.is_some();
                let legacy_title_is_folder = is_legacy_folder_title(&raw_title);

                tracing::debug!("[SCAN] Processing Channel: '{}' (ID: {})", raw_title, id);

                if !title_is_folder && !legacy_title_is_folder {
                    continue;
                }

                let mut parent_id = title_parent.unwrap_or(None);
                let mut resolved_from_about = false;
                let mut migrated_title = false;

                if !title_is_folder && about_fallback_enabled && access_hash != 0 {
                    match fetch_channel_about_with_retry(&client, id, access_hash).await {
                        AboutLookupResult::Success(about) => {
                            let about_is_folder = about
                                .as_deref()
                                .map(|value| value.contains(FOLDER_MARKER))
                                .unwrap_or(false);

                            if about_is_folder {
                                parent_id = about.as_deref().and_then(parse_parent_id_from_about);
                                resolved_from_about = true;
                                let display_name = display_folder_name(&raw_title);
                                migrated_title = migrate_title_metadata(
                                    &client,
                                    id,
                                    access_hash,
                                    &display_name,
                                    parent_id,
                                )
                                .await;
                            }
                        }
                        AboutLookupResult::FloodWait(wait_secs) => {
                            about_fallback_enabled = false;
                            tracing::warn!(
                                "Disabling about fallback for this scan after FLOOD_WAIT ({}s)",
                                wait_secs
                            );
                        }
                        AboutLookupResult::Failed => {}
                    }
                }

                if title_is_folder {
                    resolved_by_title += 1;
                } else if resolved_from_about {
                    resolved_by_about += 1;
                }
                if migrated_title {
                    migrated += 1;
                }

                let display_name = display_folder_name(&raw_title);
                tracing::info!(
                    " -> MATCH (title_meta={}, legacy_title={}, about_fallback={}, migrated={}) {}",
                    title_is_folder,
                    legacy_title_is_folder,
                    resolved_from_about,
                    migrated_title,
                    display_name
                );

                folders.push(FolderMetadata {
                    id,
                    name: display_name,
                    parent_id,
                });
            }
            peer => {
                tracing::debug!("[SCAN] Skipped Peer: {:?}", peer);
            }
        }
    }

    let folder_ids: HashSet<i64> = folders.iter().map(|folder| folder.id).collect();
    let orphans = folders
        .iter()
        .filter(|folder| {
            folder
                .parent_id
                .map(|parent_id| !folder_ids.contains(&parent_id))
                .unwrap_or(false)
        })
        .count();

    tracing::info!(
        "Scan complete. Found {} folders (title={}, about={}, orphans={}, migrated={}).",
        folders.len(),
        resolved_by_title,
        resolved_by_about,
        orphans,
        migrated
    );

    Ok(FolderSyncReport {
        folders,
        resolved_by_title,
        resolved_by_about,
        orphans,
        migrated,
    })
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
    fn build_folder_title_includes_expected_metadata() {
        let title = build_folder_title("Documents", Some(55));
        assert_eq!(title, "Documents [TD|s=1|p=55]");

        let root_title = build_folder_title("Root", None);
        assert_eq!(root_title, "Root [TD|s=1|p=null]");
    }

    #[test]
    fn parse_parent_id_from_title_handles_values() {
        let root_title = "Root [TD|s=1|p=null]";
        let child_title = "Child [TD|s=1|p=777]";

        assert_eq!(parse_parent_id_from_title(root_title), Some(None));
        assert_eq!(parse_parent_id_from_title(child_title), Some(Some(777)));
        assert_eq!(parse_parent_id_from_title("Legacy [TD]"), None);
    }

    #[test]
    fn display_folder_name_strips_metadata_suffixes() {
        assert_eq!(
            display_folder_name("Projects [TD|s=1|p=11]"),
            "Projects".to_string()
        );
        assert_eq!(display_folder_name("Photos [TD]"), "Photos".to_string());
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
