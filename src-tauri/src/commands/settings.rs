use crate::models::{Settings, SettingsPublic};
use crate::services::crypto::CryptoService;
use crate::services::google_auth::GoogleAuthService;
use crate::utils::normalize_api_key;
use crate::services::state::AppState;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use tauri::{AppHandle, State};

const GOOGLE_REFRESH_TOKEN_KEY: &str = "google_refresh_token";
const GOOGLE_EMAIL_KEY: &str = "google_email";
const GOOGLE_ACCESS_TOKEN_KEY: &str = "google_access_token";
const GOOGLE_ACCESS_TOKEN_EXPIRES_AT_KEY: &str = "google_access_token_expires_at";
const COMPANY_NAME_KEY: &str = "company_name";
const COMPANY_EMAIL_KEY: &str = "company_email";
const COMPANY_ADDRESS_KEY: &str = "company_address";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    pub revenue_folder: Option<String>,
    pub payable_folder: Option<String>,
    pub company_name: Option<String>,
    pub company_email: Option<String>,
    pub company_address: Option<String>,
    pub openai_api_key: Option<String>,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsPublic, String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;

    let revenue_folder = db.get_setting("revenue_folder").map_err(|e| e.to_string())?;
    let payable_folder = db.get_setting("payable_folder").map_err(|e| e.to_string())?;
    let company_name = db
        .get_setting(COMPANY_NAME_KEY)
        .map_err(|e| e.to_string())?;
    let google_email = db.get_setting(GOOGLE_EMAIL_KEY).map_err(|e| e.to_string())?;
    let company_email = db
        .get_setting(COMPANY_EMAIL_KEY)
        .map_err(|e| e.to_string())?
        .or_else(|| google_email.clone());
    let company_address = db
        .get_setting(COMPANY_ADDRESS_KEY)
        .map_err(|e| e.to_string())?;
    let mut openai_api_key = db.get_setting("openai_api_key").map_err(|e| e.to_string())?;
    if matches!(openai_api_key.as_deref(), Some(value) if value.starts_with("keychain:")) {
        db.delete_setting("openai_api_key").map_err(|e| e.to_string())?;
        openai_api_key = None;
    }
    let (openai_api_key_present, openai_api_key_last4) = match openai_api_key {
        Some(ref encrypted) if !encrypted.trim().is_empty() => {
            let last4 = CryptoService::decrypt_api_key(encrypted).ok().map(|key| {
                let normalized = normalize_api_key(&key);
                let chars: Vec<char> = normalized.chars().collect();
                if chars.len() >= 4 {
                    chars[chars.len() - 4..].iter().collect::<String>()
                } else {
                    normalized
                }
            });
            (true, last4)
        }
        _ => (false, None),
    };
    let google_connected = db
        .get_setting(GOOGLE_REFRESH_TOKEN_KEY)
        .map_err(|e| e.to_string())?
        .as_deref()
        .map(|value| CryptoService::decrypt_secret(value).map(|token| !token.trim().is_empty()).unwrap_or(false))
        .unwrap_or(false);

    Ok(SettingsPublic {
        revenue_folder,
        payable_folder,
        company_name,
        company_email,
        company_address,
        openai_api_key_present,
        openai_api_key_last4,
        google_connected,
        google_email,
    })
}

#[tauri::command]
pub async fn save_settings(
    payload: SettingsPayload,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;

        if let Some(value) = payload.revenue_folder.clone() {
            db.set_setting("revenue_folder", &value).map_err(|e| e.to_string())?;
        }
        if let Some(value) = payload.payable_folder.clone() {
            db.set_setting("payable_folder", &value).map_err(|e| e.to_string())?;
        }
        if let Some(value) = payload.company_name.clone() {
            db.set_setting(COMPANY_NAME_KEY, &value).map_err(|e| e.to_string())?;
        }
        if let Some(value) = payload.company_email.clone() {
            db.set_setting(COMPANY_EMAIL_KEY, &value).map_err(|e| e.to_string())?;
        }
        if let Some(value) = payload.company_address.clone() {
            db.set_setting(COMPANY_ADDRESS_KEY, &value).map_err(|e| e.to_string())?;
        }
        if let Some(api_key) = payload.openai_api_key.clone() {
            let normalized = normalize_api_key(&api_key);
            if !normalized.is_empty() {
                let encrypted = CryptoService::encrypt_api_key(&normalized).map_err(|e| e.to_string())?;
                db.set_setting("openai_api_key", &encrypted)
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    let settings = load_settings_internal(&state).map_err(|e| e.to_string())?;
    state.update_settings(settings, &app).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn test_openai_key(api_key: String) -> Result<bool, String> {
    test_openai_key_internal(&api_key).await
}

#[tauri::command]
pub async fn pick_folder() -> Result<Option<String>, String> {
    let selection = rfd::FileDialog::new()
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string());
    Ok(selection)
}

#[tauri::command]
pub async fn remove_openai_key(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        db.delete_setting("openai_api_key").map_err(|e| e.to_string())?;
    }
    let settings = load_settings_internal(&state).map_err(|e| e.to_string())?;
    state.update_settings(settings, &app).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn connect_google_account(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    let session = GoogleAuthService::authenticate(&app)
        .await
        .map_err(|e| e.to_string())?;

    let refresh_token = session
        .refresh_token
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "Google did not return a refresh token. Remove Billly from your Google account permissions, then connect again."
                .to_string()
        })?;
    let encrypted_refresh_token =
        CryptoService::encrypt_secret(&refresh_token).map_err(|e| e.to_string())?;
    let encrypted_access_token =
        CryptoService::encrypt_secret(&session.access_token).map_err(|e| e.to_string())?;
    let access_token_expires_at = access_token_expires_at(session.expires_in).to_rfc3339();

    {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        db.set_setting(GOOGLE_REFRESH_TOKEN_KEY, &encrypted_refresh_token)
            .map_err(|e| e.to_string())?;
        db.set_setting(GOOGLE_ACCESS_TOKEN_KEY, &encrypted_access_token)
            .map_err(|e| e.to_string())?;
        db.set_setting(GOOGLE_ACCESS_TOKEN_EXPIRES_AT_KEY, &access_token_expires_at)
            .map_err(|e| e.to_string())?;
        db.set_setting(GOOGLE_EMAIL_KEY, &session.email)
            .map_err(|e| e.to_string())?;
        db.set_setting(COMPANY_EMAIL_KEY, &session.email)
            .map_err(|e| e.to_string())?;
    }

    Ok(session.email)
}

#[tauri::command]
pub async fn remove_google_account(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    db.delete_setting(GOOGLE_REFRESH_TOKEN_KEY)
        .map_err(|e| e.to_string())?;
    db.delete_setting(GOOGLE_ACCESS_TOKEN_KEY)
        .map_err(|e| e.to_string())?;
    db.delete_setting(GOOGLE_ACCESS_TOKEN_EXPIRES_AT_KEY)
        .map_err(|e| e.to_string())?;
    db.delete_setting(GOOGLE_EMAIL_KEY)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn test_gmail_connection(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    if let Some(saved_access_token) = load_saved_google_access_token(&state)? {
        match GoogleAuthService::fetch_gmail_email(&saved_access_token).await {
            Ok(email) => {
                let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
                db.set_setting(GOOGLE_EMAIL_KEY, &email)
                    .map_err(|e| e.to_string())?;
                db.set_setting(COMPANY_EMAIL_KEY, &email)
                    .map_err(|e| e.to_string())?;
                return Ok(email);
            }
            Err(_) => {
                let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
                db.delete_setting(GOOGLE_ACCESS_TOKEN_KEY)
                    .map_err(|e| e.to_string())?;
                db.delete_setting(GOOGLE_ACCESS_TOKEN_EXPIRES_AT_KEY)
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    let refresh_token = {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        let encrypted = db
            .get_setting(GOOGLE_REFRESH_TOKEN_KEY)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "No saved Google account".to_string())?;
        CryptoService::decrypt_secret(&encrypted).map_err(|e| e.to_string())?
    };
    let refresh_session = match GoogleAuthService::refresh_access_token(&app, &refresh_token).await {
        Ok(session) => session,
        Err(error) => {
            let message = error.to_string();
            if message.contains("invalid_grant") {
                let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
                db.delete_setting(GOOGLE_REFRESH_TOKEN_KEY)
                    .map_err(|e| e.to_string())?;
                db.delete_setting(GOOGLE_ACCESS_TOKEN_KEY)
                    .map_err(|e| e.to_string())?;
                db.delete_setting(GOOGLE_ACCESS_TOKEN_EXPIRES_AT_KEY)
                    .map_err(|e| e.to_string())?;
                db.delete_setting(GOOGLE_EMAIL_KEY)
                    .map_err(|e| e.to_string())?;
                return Err(
                    "Google login expired or was revoked. Connect Google again to restore Gmail access."
                        .to_string(),
                );
            }
            return Err(message);
        }
    };
    let email = GoogleAuthService::fetch_gmail_email(&refresh_session.access_token)
        .await
        .map_err(|e| e.to_string())?;

    {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        let encrypted_access_token =
            CryptoService::encrypt_secret(&refresh_session.access_token).map_err(|e| e.to_string())?;
        let expires_at = access_token_expires_at(refresh_session.expires_in).to_rfc3339();
        db.set_setting(GOOGLE_ACCESS_TOKEN_KEY, &encrypted_access_token)
            .map_err(|e| e.to_string())?;
        db.set_setting(GOOGLE_ACCESS_TOKEN_EXPIRES_AT_KEY, &expires_at)
            .map_err(|e| e.to_string())?;
        if let Some(rotated_refresh_token) = refresh_session.refresh_token {
            let encrypted_refresh_token =
                CryptoService::encrypt_secret(&rotated_refresh_token).map_err(|e| e.to_string())?;
            db.set_setting(GOOGLE_REFRESH_TOKEN_KEY, &encrypted_refresh_token)
                .map_err(|e| e.to_string())?;
        }
        db.set_setting(GOOGLE_EMAIL_KEY, &email)
            .map_err(|e| e.to_string())?;
        db.set_setting(COMPANY_EMAIL_KEY, &email)
            .map_err(|e| e.to_string())?;
    }

    Ok(email)
}

#[tauri::command]
pub async fn test_openai_key_saved(state: State<'_, AppState>) -> Result<bool, String> {
    let api_key = {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        let encrypted = db
            .get_setting("openai_api_key")
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "No saved OpenAI key".to_string())?;
        if encrypted.starts_with("keychain:") {
            return Err("Saved key uses keychain storage, which is disabled. Please re-save the key."
                .to_string());
        }
        CryptoService::decrypt_api_key(&encrypted).map_err(|e| e.to_string())?
    };
    test_openai_key_internal(&api_key).await
}

async fn test_openai_key_internal(api_key: &str) -> Result<bool, String> {
    let normalized = normalize_api_key(api_key);
    if normalized.is_empty() {
        return Err("OpenAI key is empty".to_string());
    }
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.openai.com/v1/models")
        .bearer_auth(normalized)
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    Ok(response.status().is_success())
}

fn load_settings_internal(state: &State<'_, AppState>) -> Result<Settings, String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    let revenue_folder = db.get_setting("revenue_folder").map_err(|e| e.to_string())?;
    let payable_folder = db.get_setting("payable_folder").map_err(|e| e.to_string())?;
    let company_name = db.get_setting(COMPANY_NAME_KEY).map_err(|e| e.to_string())?;
    let company_email = db.get_setting(COMPANY_EMAIL_KEY).map_err(|e| e.to_string())?;
    let company_address = db.get_setting(COMPANY_ADDRESS_KEY).map_err(|e| e.to_string())?;
    let mut openai_api_key = db.get_setting("openai_api_key").map_err(|e| e.to_string())?;
    if matches!(openai_api_key.as_deref(), Some(value) if value.starts_with("keychain:")) {
        db.delete_setting("openai_api_key").map_err(|e| e.to_string())?;
        openai_api_key = None;
    }
    Ok(Settings {
        revenue_folder,
        payable_folder,
        company_name,
        company_email,
        company_address,
        openai_api_key,
    })
}

fn access_token_expires_at(expires_in_seconds: Option<i64>) -> DateTime<Utc> {
    let ttl = expires_in_seconds.unwrap_or(3600).max(60);
    Utc::now() + Duration::seconds(ttl.saturating_sub(60))
}

fn load_saved_google_access_token(state: &State<'_, AppState>) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    let encrypted_access_token = match db
        .get_setting(GOOGLE_ACCESS_TOKEN_KEY)
        .map_err(|e| e.to_string())?
    {
        Some(value) => value,
        None => return Ok(None),
    };
    let expires_at = match db
        .get_setting(GOOGLE_ACCESS_TOKEN_EXPIRES_AT_KEY)
        .map_err(|e| e.to_string())?
    {
        Some(value) => value,
        None => return Ok(None),
    };
    let expires_at = DateTime::parse_from_rfc3339(&expires_at)
        .map_err(|e| e.to_string())?
        .with_timezone(&Utc);
    if expires_at <= Utc::now() {
        return Ok(None);
    }
    let access_token = CryptoService::decrypt_secret(&encrypted_access_token).map_err(|e| e.to_string())?;
    if access_token.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(access_token))
}
