use crate::models::{Settings, SettingsPublic};
use crate::services::crypto::CryptoService;
use crate::utils::normalize_api_key;
use crate::services::state::AppState;
use serde::Deserialize;
use tauri::{AppHandle, State};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    pub revenue_folder: Option<String>,
    pub payable_folder: Option<String>,
    pub openai_api_key: Option<String>,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsPublic, String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;

    let revenue_folder = db.get_setting("revenue_folder").map_err(|e| e.to_string())?;
    let payable_folder = db.get_setting("payable_folder").map_err(|e| e.to_string())?;
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

    Ok(SettingsPublic {
        revenue_folder,
        payable_folder,
        openai_api_key_present,
        openai_api_key_last4,
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
    let mut openai_api_key = db.get_setting("openai_api_key").map_err(|e| e.to_string())?;
    if matches!(openai_api_key.as_deref(), Some(value) if value.starts_with("keychain:")) {
        db.delete_setting("openai_api_key").map_err(|e| e.to_string())?;
        openai_api_key = None;
    }
    Ok(Settings {
        revenue_folder,
        payable_folder,
        openai_api_key,
    })
}
