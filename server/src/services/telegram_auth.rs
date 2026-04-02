use grammers_client::Client;
use grammers_client::SignInError;
use grammers_mtsender::SenderPool;
use grammers_session::storages::SqliteSession;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::Duration;

use crate::app_state::AppState;
use crate::domain::models::AuthResult;
use crate::errors::{map_telegram_error, AppError};

/// Ensures the Telegram client is initialized.
///
/// Properly manages runner lifecycle to prevent stack overflow:
/// before spawning a new runner, it signals the old runner to shutdown.
pub async fn ensure_client_initialized(state: &AppState, api_id: i32) -> Result<Client, AppError> {
    let mut client_guard = state.telegram_client.lock().await;

    if let Some(client) = client_guard.as_ref() {
        return Ok(client.clone());
    }

    // Shutdown existing runner before creating a new one
    {
        let mut shutdown_guard = state.runner_shutdown.lock().await;
        if let Some(shutdown_tx) = shutdown_guard.take() {
            tracing::info!("Signaling old runner to shutdown...");
            let _ = shutdown_tx.send(());
            drop(shutdown_guard);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    let runner_num = state.runner_count.fetch_add(1, Ordering::SeqCst) + 1;
    tracing::info!(
        "Initializing Telegram Client #{} with API ID: {}",
        runner_num,
        api_id
    );

    // Resolve session path
    std::fs::create_dir_all(&state.data_dir)
        .map_err(|e| AppError::Internal(format!("Failed to create data dir: {}", e)))?;

    let session_path = crate::storage::telegram_session::session_path(&state.data_dir);
    let session_path_str = session_path.to_string_lossy().to_string();
    tracing::info!("Opening session at: {}", session_path_str);

    // Grammers initialization with corruption recovery
    let session = match SqliteSession::open(&session_path) {
        Ok(s) => s,
        Err(_) => {
            tracing::warn!("Session file corrupted or invalid. Recreating...");
            let _ = std::fs::remove_file(&session_path);
            let _ = std::fs::remove_file(format!("{}-wal", session_path_str));
            let _ = std::fs::remove_file(format!("{}-shm", session_path_str));

            SqliteSession::open(&session_path).map_err(|e| {
                AppError::Internal(format!("Failed to open session after recreation: {}", e))
            })?
        }
    };

    let session = Arc::new(session);
    let pool = SenderPool::new(session, api_id);
    let client = Client::new(&pool);

    // Create shutdown channel for this runner
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    *state.runner_shutdown.lock().await = Some(shutdown_tx);

    // Spawn the network runner with shutdown support
    let SenderPool { runner, .. } = pool;
    tokio::spawn(async move {
        tokio::select! {
            _ = runner.run() => {
                tracing::info!("Runner #{} exited normally", runner_num);
            }
            _ = shutdown_rx => {
                tracing::info!("Runner #{} shutdown requested, exiting", runner_num);
            }
        }
    });

    *client_guard = Some(client.clone());
    Ok(client)
}

/// Store API ID and initialize client.
pub async fn connect(state: &AppState, api_id: i32) -> Result<bool, AppError> {
    *state.api_id.lock().await = Some(api_id);
    ensure_client_initialized(state, api_id).await?;
    Ok(true)
}

/// Check if the connection is alive; auto-reconnect if possible.
pub async fn check_connection(state: &AppState) -> Result<bool, AppError> {
    let client_opt = { state.telegram_client.lock().await.clone() };

    if let Some(client) = client_opt {
        if client.get_me().await.is_ok() {
            return Ok(true);
        }
        tracing::warn!("Connection check failed (get_me). Attempting reconnect...");
    } else {
        tracing::warn!("Connection check: No client found. Checking for saved API ID...");
    }

    let api_id_opt = *state.api_id.lock().await;
    if let Some(api_id) = api_id_opt {
        *state.telegram_client.lock().await = None;

        let client = ensure_client_initialized(state, api_id).await?;
        if client.get_me().await.is_ok() {
            tracing::info!("Auto-reconnect successful.");
            return Ok(true);
        } else {
            return Err(AppError::Telegram(
                "Reconnect succeeded but ping failed.".to_string(),
            ));
        }
    }

    Ok(false)
}

/// Full logout: shutdown runner, sign out, clear state, remove session files.
pub async fn logout(state: &AppState) -> Result<bool, AppError> {
    tracing::info!("Logging out...");

    // 1. Shutdown the network runner
    {
        let mut shutdown_guard = state.runner_shutdown.lock().await;
        if let Some(shutdown_tx) = shutdown_guard.take() {
            tracing::info!("Signaling runner shutdown for logout...");
            let _ = shutdown_tx.send(());
        }
    }

    // 2. Try to sign out from Telegram
    let client_opt = { state.telegram_client.lock().await.clone() };
    if let Some(client) = client_opt {
        let _ = client.sign_out().await;
    }

    // 3. Clear state
    *state.telegram_client.lock().await = None;
    *state.login_token.lock().await = None;
    *state.password_token.lock().await = None;
    *state.api_id.lock().await = None;

    // 4. Remove session files
    let session_path = crate::storage::telegram_session::session_path(&state.data_dir);
    let session_path_str = session_path.to_string_lossy().to_string();
    let _ = std::fs::remove_file(&session_path);
    let _ = std::fs::remove_file(format!("{}-wal", session_path_str));
    let _ = std::fs::remove_file(format!("{}-shm", session_path_str));

    tracing::info!(
        "Logout complete. Runner count: {}",
        state.runner_count.load(Ordering::SeqCst)
    );
    Ok(true)
}

/// Request a login code sent to the given phone number.
pub async fn request_code(
    state: &AppState,
    phone: &str,
    api_id: i32,
    api_hash: &str,
) -> Result<String, AppError> {
    if api_hash.trim().is_empty() {
        return Err(AppError::BadRequest(
            "API Hash cannot be empty.".to_string(),
        ));
    }

    *state.api_id.lock().await = Some(api_id);

    let client = ensure_client_initialized(state, api_id).await?;

    tracing::info!("Requesting code for {}", phone);

    let mut last_error = String::new();

    for attempt in 1..=2 {
        match client.request_login_code(phone, api_hash).await {
            Ok(token) => {
                *state.login_token.lock().await = Some(token);
                return Ok("code_sent".to_string());
            }
            Err(e) => {
                let err_msg = e.to_string();
                tracing::warn!("Error requesting code (Attempt {}): {}", attempt, err_msg);

                if err_msg.contains("AUTH_RESTART") || err_msg.contains("500") {
                    tracing::info!("AUTH_RESTART error detected. Retrying...");
                    last_error = err_msg;
                    continue;
                }

                return Err(map_telegram_error(e));
            }
        }
    }

    Err(AppError::Telegram(format!(
        "Telegram Error after retry: {}",
        last_error
    )))
}

/// Sign in with the received code.
pub async fn sign_in(state: &AppState, code: &str) -> Result<AuthResult, AppError> {
    tracing::info!("Signing in with code...");

    let client = {
        let guard = state.telegram_client.lock().await;
        guard
            .as_ref()
            .ok_or(AppError::BadRequest("Client not initialized".to_string()))?
            .clone()
    };

    let token_guard = state.login_token.lock().await;
    let login_token = token_guard.as_ref().ok_or(AppError::BadRequest(
        "No login session found (restart flow)".to_string(),
    ))?;

    match client.sign_in(login_token, code).await {
        Ok(_user) => {
            tracing::info!("Successfully logged in.");
            Ok(AuthResult {
                success: true,
                next_step: Some("dashboard".to_string()),
                error: None,
            })
        }
        Err(SignInError::PasswordRequired(token)) => {
            *state.password_token.lock().await = Some(token);
            Ok(AuthResult {
                success: false,
                next_step: Some("password".to_string()),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("Sign in error: {}", e);
            Err(AppError::Telegram(format!("Sign in failed: {}", e)))
        }
    }
}

/// Check the 2FA password.
pub async fn check_password(state: &AppState, password: &str) -> Result<AuthResult, AppError> {
    let client = {
        let guard = state.telegram_client.lock().await;
        guard
            .as_ref()
            .ok_or(AppError::BadRequest("Client not initialized".to_string()))?
            .clone()
    };

    let pw_token = state
        .password_token
        .lock()
        .await
        .take()
        .ok_or(AppError::BadRequest(
            "No password session found".to_string(),
        ))?;

    match client.check_password(pw_token, password).await {
        Ok(_user) => {
            tracing::info!("2FA Success.");
            Ok(AuthResult {
                success: true,
                next_step: Some("dashboard".to_string()),
                error: None,
            })
        }
        Err(e) => Err(AppError::Telegram(format!("2FA Failed: {}", e))),
    }
}
