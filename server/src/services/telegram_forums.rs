use grammers_client::types::Peer;
use grammers_tl_types as tl;
use rand::Rng;
use std::collections::BTreeMap;

use crate::app_state::AppState;
use crate::domain::models::{ForumMetadata, ForumTopicMetadata};
use crate::errors::{map_telegram_error, AppError};
use crate::services::helpers::resolve_peer;
use crate::storage::app_db::{self, StructuredFolderCacheEntry};

const DEFAULT_TOPIC_ICON_COLOR: i32 = 0x6FB9F0;
const STRUCTURED_TITLE_MARKER: &str = " [TD|SF]";
const STRUCTURED_ABOUT_MARKER: &str = "[telegram-drive-structured-folder]";
const STRUCTURED_ABOUT_SCHEMA_LINE: &str = "td_structured_folder_schema=1";
const LEGACY_STRUCTURED_ABOUT_MARKER: &str = "Telegram Drive Community Forum";

fn ensure_forums_enabled(state: &AppState) -> Result<(), AppError> {
    if state.forums_enabled {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "Structured folders feature is disabled. Set FORUMS_ENABLED=true to enable structured folders."
                .to_string(),
        ))
    }
}

async fn require_client(state: &AppState) -> Result<grammers_client::Client, AppError> {
    state
        .telegram_client
        .lock()
        .await
        .clone()
        .ok_or(AppError::Unauthorized)
}

fn normalize_name(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn build_structured_title(name: &str) -> String {
    let normalized = normalize_name(name, "Structured Folder");
    if normalized.contains(STRUCTURED_TITLE_MARKER) {
        normalized
    } else {
        format!("{}{}", normalized, STRUCTURED_TITLE_MARKER)
    }
}

fn display_structured_name(raw_title: &str) -> String {
    let cleaned = raw_title.replace(STRUCTURED_TITLE_MARKER, "");
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Structured Folder".to_string()
    } else {
        trimmed.to_string()
    }
}

fn build_structured_about() -> String {
    format!(
        "Telegram Drive Structured Folder Root\n{}\n{}",
        STRUCTURED_ABOUT_MARKER, STRUCTURED_ABOUT_SCHEMA_LINE
    )
}

fn about_has_structured_marker(about: &str) -> bool {
    about.contains(STRUCTURED_ABOUT_MARKER) || about.contains(LEGACY_STRUCTURED_ABOUT_MARKER)
}

fn title_has_structured_marker(title: &str) -> bool {
    title.contains(STRUCTURED_TITLE_MARKER)
}

async fn fetch_channel_about(
    client: &grammers_client::Client,
    channel_id: i64,
    access_hash: i64,
) -> Option<String> {
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
        Ok(tl::enums::messages::ChatFull::Full(full)) => match full.full_chat {
            tl::enums::ChatFull::Full(chat_full) => Some(chat_full.about),
            _ => None,
        },
        Err(err) => {
            tracing::debug!(
                channel_id,
                error = %err,
                "GetFullChannel failed while probing structured folder marker"
            );
            None
        }
    }
}

fn load_cached_structured_roots(state: &AppState) -> Vec<StructuredFolderCacheEntry> {
    app_db::load_structured_folders(&state.data_dir)
}

fn cached_access_hash(state: &AppState, folder_id: i64) -> Option<i64> {
    load_cached_structured_roots(state)
        .into_iter()
        .find(|entry| entry.id == folder_id)
        .and_then(|entry| entry.access_hash)
}

fn cache_structured_root(
    state: &AppState,
    id: i64,
    name: String,
    access_hash: Option<i64>,
) {
    let entry = StructuredFolderCacheEntry {
        id,
        name,
        access_hash,
    };

    if let Err(err) = app_db::upsert_structured_folder(&state.data_dir, entry) {
        tracing::warn!(error = %err, "Failed to upsert structured folder cache entry");
    }
}

fn remove_cached_structured_root(state: &AppState, id: i64) {
    if let Err(err) = app_db::remove_structured_folder(&state.data_dir, id) {
        tracing::warn!(error = %err, folder_id = id, "Failed to remove structured folder cache entry");
    }
}

fn extract_topics(
    result: tl::enums::messages::ForumTopics,
    forum_id: i64,
) -> Vec<ForumTopicMetadata> {
    let topics = match result {
        tl::enums::messages::ForumTopics::Topics(value) => value.topics,
    };

    topics
        .into_iter()
        .filter_map(|topic| match topic {
            tl::enums::ForumTopic::Topic(topic) => Some(ForumTopicMetadata {
                id: topic.id,
                forum_id,
                title: topic.title,
                icon_color: topic.icon_color,
                icon_emoji_id: topic.icon_emoji_id,
                closed: topic.closed,
                hidden: topic.hidden,
                pinned: topic.pinned,
                top_message: topic.top_message,
            }),
            tl::enums::ForumTopic::Deleted(_) => None,
        })
        .collect()
}

async fn require_forum_input_peer(
    state: &AppState,
    client: &grammers_client::Client,
    forum_id: i64,
) -> Result<tl::enums::InputPeer, AppError> {
    if let Ok(peer) = resolve_peer(client, Some(forum_id)).await {
        return match peer {
            Peer::Channel(channel) => {
                let access_hash = channel
                    .raw
                    .access_hash
                    .or_else(|| cached_access_hash(state, forum_id))
                    .ok_or(AppError::Telegram(
                        "Missing access hash for structured folder".to_string(),
                    ))?;

                Ok(tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
                    channel_id: channel.raw.id,
                    access_hash,
                }))
            }
            _ => Err(AppError::BadRequest(
                "Target folder is not a valid structured folder root".to_string(),
            )),
        };
    }

    if let Some(access_hash) = cached_access_hash(state, forum_id) {
        return Ok(tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
            channel_id: forum_id,
            access_hash,
        }));
    }

    Err(AppError::NotFound(format!(
        "Structured folder {} not found",
        forum_id
    )))
}

/// Resolve a structured folder root into an InputPeer with cache fallback.
pub async fn resolve_forum_input_peer(
    state: &AppState,
    forum_id: i64,
) -> Result<tl::enums::InputPeer, AppError> {
    ensure_forums_enabled(state)?;
    let client = require_client(state).await?;
    require_forum_input_peer(state, &client, forum_id).await
}

/// List structured folder roots.
pub async fn list_forums(state: &AppState) -> Result<Vec<ForumMetadata>, AppError> {
    ensure_forums_enabled(state)?;

    let client = require_client(state).await?;
    let mut dialogs = client.iter_dialogs();
    let mut roots_by_id: BTreeMap<i64, ForumMetadata> = load_cached_structured_roots(state)
        .into_iter()
        .map(|entry| {
            (
                entry.id,
                ForumMetadata {
                    id: entry.id,
                    name: entry.name,
                },
            )
        })
        .collect();

    while let Some(dialog) = dialogs
        .next()
        .await
        .map_err(|e| AppError::Telegram(e.to_string()))?
    {
        if let Peer::Channel(channel) = &dialog.peer {
            if !channel.raw.megagroup {
                continue;
            }

            let mut is_structured = channel.raw.forum || title_has_structured_marker(&channel.raw.title);

            if !is_structured {
                if let Some(access_hash) = channel.raw.access_hash {
                    if let Some(about) = fetch_channel_about(&client, channel.raw.id, access_hash).await {
                        is_structured = about_has_structured_marker(&about);
                    }
                }
            }

            if is_structured {
                let display_name = display_structured_name(&channel.raw.title);

                roots_by_id.insert(
                    channel.raw.id,
                    ForumMetadata {
                        id: channel.raw.id,
                        name: display_name.clone(),
                    },
                );

                cache_structured_root(state, channel.raw.id, display_name, channel.raw.access_hash);
            }
        }
    }

    let mut forums: Vec<ForumMetadata> = roots_by_id.into_values().collect();
    forums.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(forums)
}

/// Create a structured folder root.
pub async fn create_forum(state: &AppState, name: &str) -> Result<ForumMetadata, AppError> {
    ensure_forums_enabled(state)?;

    let client = require_client(state).await?;
    let normalized_name = normalize_name(name, "Structured Folder");
    let title = build_structured_title(&normalized_name);

    let result = client
        .invoke(&tl::functions::channels::CreateChannel {
            broadcast: false,
            megagroup: true,
            title,
            about: build_structured_about(),
            geo_point: None,
            address: None,
            for_import: false,
            forum: true,
            ttl_period: None,
        })
        .await
        .map_err(map_telegram_error)?;

    let (forum_id, access_hash) = match result {
        tl::enums::Updates::Updates(update) => {
            let chat = update.chats.first().ok_or(AppError::Telegram(
                "No chat in CreateChannel response".to_string(),
            ))?;
            match chat {
                tl::enums::Chat::Channel(channel) => (channel.id, channel.access_hash),
                _ => {
                    return Err(AppError::Telegram(
                        "Created chat is not a channel".to_string(),
                    ))
                }
            }
        }
        _ => {
            return Err(AppError::Telegram(
                "Unexpected CreateChannel response".to_string(),
            ))
        }
    };

    cache_structured_root(state, forum_id, normalized_name.clone(), access_hash);

    Ok(ForumMetadata {
        id: forum_id,
        name: normalized_name,
    })
}

/// List topics inside a forum-enabled supergroup.
pub async fn list_topics(
    state: &AppState,
    forum_id: i64,
) -> Result<Vec<ForumTopicMetadata>, AppError> {
    ensure_forums_enabled(state)?;

    let client = require_client(state).await?;
    let input_peer = require_forum_input_peer(state, &client, forum_id).await?;

    let result = client
        .invoke(&tl::functions::messages::GetForumTopics {
            peer: input_peer,
            q: None,
            offset_date: 0,
            offset_id: 0,
            offset_topic: 0,
            limit: 100,
        })
        .await
        .map_err(map_telegram_error)?;

    Ok(extract_topics(result, forum_id))
}

/// Create a topic inside a forum-enabled supergroup.
pub async fn create_topic(
    state: &AppState,
    forum_id: i64,
    title: &str,
    icon_color: Option<i32>,
    icon_emoji_id: Option<i64>,
) -> Result<ForumTopicMetadata, AppError> {
    ensure_forums_enabled(state)?;

    let client = require_client(state).await?;
    let input_peer = require_forum_input_peer(state, &client, forum_id).await?;
    let normalized_title = normalize_name(title, "Subfolder");
    let mut rng = rand::thread_rng();
    let random_id: i64 = rng.gen();

    let _ = client
        .invoke(&tl::functions::messages::CreateForumTopic {
            title_missing: false,
            peer: input_peer,
            title: normalized_title.clone(),
            icon_color,
            icon_emoji_id,
            random_id,
            send_as: None,
        })
        .await
        .map_err(map_telegram_error)?;

    // Best-effort lookup after creation to return the created topic metadata.
    let topics = list_topics(state, forum_id).await?;
    if let Some(created) = topics
        .into_iter()
        .filter(|topic| topic.title == normalized_title)
        .max_by_key(|topic| topic.id)
    {
        return Ok(created);
    }

    Ok(ForumTopicMetadata {
        id: 0,
        forum_id,
        title: normalized_title,
        icon_color: icon_color.unwrap_or(DEFAULT_TOPIC_ICON_COLOR),
        icon_emoji_id,
        closed: false,
        hidden: false,
        pinned: false,
        top_message: 0,
    })
}

async fn resolve_topic_top_message(
    client: &grammers_client::Client,
    input_peer: tl::enums::InputPeer,
    forum_id: i64,
    topic_id: i32,
    topic_top_message: Option<i32>,
) -> Result<i32, AppError> {
    if let Some(top_message) = topic_top_message {
        if top_message > 0 {
            return Ok(top_message);
        }
    }

    let result = client
        .invoke(&tl::functions::messages::GetForumTopicsById {
            peer: input_peer,
            topics: vec![topic_id],
        })
        .await
        .map_err(map_telegram_error)?;

    extract_topics(result, forum_id)
        .into_iter()
        .find(|topic| topic.id == topic_id)
        .map(|topic| topic.top_message)
        .filter(|top_message| *top_message > 0)
        .ok_or(AppError::NotFound(format!(
            "Structured subfolder {} not found in {}",
            topic_id, forum_id
        )))
}

/// Delete a structured folder root (forum-enabled supergroup).
pub async fn delete_forum(state: &AppState, forum_id: i64) -> Result<(), AppError> {
    ensure_forums_enabled(state)?;

    let client = require_client(state).await?;
    let input_peer = match require_forum_input_peer(state, &client, forum_id).await {
        Ok(peer) => peer,
        Err(AppError::NotFound(err)) => {
            // Clear stale cache if the root no longer exists on Telegram.
            remove_cached_structured_root(state, forum_id);
            return Err(AppError::NotFound(err));
        }
        Err(err) => return Err(err),
    };

    let input_channel = match input_peer {
        tl::enums::InputPeer::Channel(channel) => {
            tl::enums::InputChannel::Channel(tl::types::InputChannel {
                channel_id: channel.channel_id,
                access_hash: channel.access_hash,
            })
        }
        _ => {
            return Err(AppError::BadRequest(
                "Target folder is not a valid structured folder root".to_string(),
            ))
        }
    };

    client
        .invoke(&tl::functions::channels::DeleteChannel {
            channel: input_channel,
        })
        .await
        .map_err(map_telegram_error)?;

    remove_cached_structured_root(state, forum_id);
    Ok(())
}

/// Delete a structured subfolder (forum topic).
pub async fn delete_topic(
    state: &AppState,
    forum_id: i64,
    topic_id: i32,
    topic_top_message: Option<i32>,
) -> Result<(), AppError> {
    ensure_forums_enabled(state)?;

    let client = require_client(state).await?;
    let input_peer = require_forum_input_peer(state, &client, forum_id).await?;
    let top_message = resolve_topic_top_message(
        &client,
        input_peer.clone(),
        forum_id,
        topic_id,
        topic_top_message,
    )
    .await?;

    let _ = client
        .invoke(&tl::functions::messages::DeleteTopicHistory {
            peer: input_peer,
            top_msg_id: top_message,
        })
        .await
        .map_err(map_telegram_error)?;

    Ok(())
}
