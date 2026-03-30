use std::sync::Arc;
use std::time::Duration;
use tokio::time;

use crate::app_state::AppState;
use crate::services::telegram_auth::ensure_client_initialized;

/// How often the reconnect task checks (every 60 seconds).
const RECONNECT_INTERVAL: Duration = Duration::from_secs(60);

/// Spawns a periodic task that checks the Telegram connection and
/// attempts to reconnect using stored credentials if the client
/// has been lost (e.g., network glitch, Telegram server restart).
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        // Wait a bit for the rest of the server to finish initializing
        time::sleep(Duration::from_secs(10)).await;

        let mut interval = time::interval(RECONNECT_INTERVAL);
        loop {
            interval.tick().await;

            let is_connected = state.telegram_client.lock().await.is_some();
            if is_connected {
                continue;
            }

            // Only attempt reconnect if API-ID is known (user has connected before)
            let api_id = *state.api_id.lock().await;
            let api_id = match api_id {
                Some(id) => id,
                None => {
                    // Also try the config value
                    if state.config_api_id != 0 {
                        state.config_api_id
                    } else {
                        continue;
                    }
                }
            };

            tracing::info!("Telegram client disconnected — attempting reconnect");

            match ensure_client_initialized(&state, api_id).await {
                Ok(_) => {
                    tracing::info!("Telegram reconnected successfully");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Telegram reconnect failed, will retry");
                }
            }
        }
    });
}
