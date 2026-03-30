use grammers_client::Client;
use grammers_client::types::Peer;

/// Resolve a folder_id to a Telegram Peer by scanning dialogs.
/// If `folder_id` is None, resolves to the authenticated user (Saved Messages).
pub async fn resolve_peer(client: &Client, folder_id: Option<i64>) -> Result<Peer, String> {
    if let Some(fid) = folder_id {
        let mut dialogs = client.iter_dialogs();
        while let Some(dialog) = dialogs.next().await.map_err(|e| e.to_string())? {
            match &dialog.peer {
                Peer::Channel(c) => {
                    if c.raw.id == fid {
                        return Ok(dialog.peer.clone());
                    }
                }
                Peer::User(u) => {
                    if u.raw.id() == fid {
                        return Ok(dialog.peer.clone());
                    }
                }
                _ => {}
            }
        }
        Err(format!("Folder/Chat {} not found", fid))
    } else {
        match client.get_me().await {
            Ok(me) => Ok(Peer::User(me)),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Format a FLOOD_WAIT error string (used in service-level error mapping).
pub fn parse_flood_wait(err_str: &str) -> Option<i64> {
    if !err_str.contains("FLOOD_WAIT") {
        return None;
    }
    if let Some(start) = err_str.find("(value: ") {
        let rest = &err_str[start + 8..];
        if let Some(end) = rest.find(')') {
            if let Ok(seconds) = rest[..end].parse::<i64>() {
                return Some(seconds);
            }
        }
    }
    Some(60) // fallback
}
