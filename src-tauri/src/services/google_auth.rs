use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ring::rand::{SecureRandom, SystemRandom};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration, Instant};
use url::Url;

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const GOOGLE_SCOPE_GMAIL_READONLY: &str = "https://www.googleapis.com/auth/gmail.readonly";

pub struct GoogleAuthService;

pub struct GoogleAuthSession {
    pub email: String,
    pub access_token: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
}

pub struct GoogleRefreshSession {
    pub access_token: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
}

#[derive(Clone, Deserialize)]
struct GoogleInstalledClient {
    client_id: String,
    client_secret: Option<String>,
    auth_uri: String,
    token_uri: String,
}

#[derive(Deserialize)]
struct GoogleClientSecret {
    installed: GoogleInstalledClient,
}

#[derive(Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    expires_in: Option<i64>,
    refresh_token: Option<String>,
    scope: Option<String>,
}

#[derive(Deserialize)]
struct GoogleTokenInfoResponse {
    scope: Option<String>,
}

#[derive(Deserialize)]
struct GmailProfileResponse {
    #[serde(rename = "emailAddress")]
    email_address: String,
}

impl GoogleAuthService {
    pub async fn authenticate(app: &AppHandle) -> Result<GoogleAuthSession> {
        let client = load_client_config(app)?;
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .context("Failed to bind Google OAuth callback port")?;
        let port = listener.local_addr()?.port();
        let redirect_uri = format!("http://localhost:{port}");
        let state = random_token(24)?;
        let code_verifier = random_token(64)?;
        let code_challenge = pkce_challenge(&code_verifier);
        let auth_url = build_auth_url(&client, &redirect_uri, &state, &code_challenge)?;

        open::that(auth_url.as_str()).context("Failed to open Google login in browser")?;

        let code = wait_for_callback(listener, &state).await?;
        let token_response = exchange_code(&client, &code, &redirect_uri, &code_verifier).await?;
        ensure_gmail_scope(&token_response).await?;
        let email = fetch_gmail_email(&token_response.access_token).await?;

        Ok(GoogleAuthSession {
            email,
            access_token: token_response.access_token,
            expires_in: token_response.expires_in,
            refresh_token: token_response.refresh_token,
        })
    }

    pub async fn refresh_access_token(app: &AppHandle, refresh_token: &str) -> Result<GoogleRefreshSession> {
        let client = load_client_config(app)?;
        let response = exchange_refresh_token(&client, refresh_token).await?;
        ensure_gmail_scope(&response).await?;
        Ok(GoogleRefreshSession {
            access_token: response.access_token,
            expires_in: response.expires_in,
            refresh_token: response.refresh_token,
        })
    }

    pub async fn fetch_gmail_email(access_token: &str) -> Result<String> {
        fetch_gmail_email(access_token).await
    }
}

fn load_client_config(app: &AppHandle) -> Result<GoogleInstalledClient> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("client_secret.json"));
    }
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("client_secret.json"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../client_secret.json"));

    let path = candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| anyhow!("client_secret.json not found. Put it in the project root or ship it as a Tauri resource."))?;

    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read Google client config at {}", path.display()))?;
    let parsed: GoogleClientSecret = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse Google client config at {}", path.display()))?;
    Ok(parsed.installed)
}

fn build_auth_url(
    client: &GoogleInstalledClient,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> Result<Url> {
    let mut url = Url::parse(&client.auth_uri).context("Invalid Google auth URI")?;
    url.query_pairs_mut()
        .append_pair("client_id", &client.client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", GOOGLE_SCOPE_GMAIL_READONLY)
        .append_pair("access_type", "offline")
        .append_pair("include_granted_scopes", "true")
        .append_pair("prompt", "consent")
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url)
}

async fn wait_for_callback(listener: TcpListener, expected_state: &str) -> Result<String> {
    let deadline = Instant::now() + CALLBACK_TIMEOUT;

    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err(anyhow!("Timed out waiting for Google login callback"));
        }

        let (mut stream, _) = timeout(deadline - now, listener.accept())
            .await
            .context("Timed out waiting for browser callback")?
            .context("Failed to accept browser callback")?;

        let mut buffer = [0u8; 8192];
        let bytes_read = timeout(IO_TIMEOUT, stream.read(&mut buffer))
            .await
            .context("Timed out reading Google callback")?
            .context("Failed to read Google callback")?;

        let request = String::from_utf8_lossy(&buffer[..bytes_read]);
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .ok_or_else(|| anyhow!("Invalid Google callback request"))?;

        let url = Url::parse(&format!("http://localhost{path}")).context("Invalid Google callback URL")?;
        let code = url
            .query_pairs()
            .find(|(key, _)| key == "code")
            .map(|(_, value)| value.into_owned());
        let returned_state = url
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned());
        let error = url
            .query_pairs()
            .find(|(key, _)| key == "error")
            .map(|(_, value)| value.into_owned());

        if let Some(error) = error {
            let _ = respond_with_html(
                &mut stream,
                400,
                "Google login failed. You can close this window and try again in Billly.",
            )
            .await;
            return Err(anyhow!("Google login failed: {error}"));
        }

        if returned_state.as_deref() != Some(expected_state) {
            let _ = respond_with_html(
                &mut stream,
                400,
                "Google login state did not match. You can close this window and try again in Billly.",
            )
            .await;
            return Err(anyhow!("Google login state mismatch"));
        }

        if let Some(code) = code {
            let _ = respond_with_html(
                &mut stream,
                200,
                "Google account connected. You can close this window and return to Billly.",
            )
            .await;
            return Ok(code);
        }

        let _ = respond_with_html(
            &mut stream,
            400,
            "Google login callback was incomplete. You can close this window and try again in Billly.",
        )
        .await;
    }
}

async fn respond_with_html(
    stream: &mut tokio::net::TcpStream,
    status_code: u16,
    message: &str,
) -> Result<()> {
    let body = format!(
        "<!doctype html><html><body style=\"font-family: sans-serif; padding: 32px; background: #111827; color: #f3f4f6;\"><h2>Billly</h2><p>{message}</p></body></html>"
    );
    let status_text = if status_code == 200 { "OK" } else { "Bad Request" };
    let response = format!(
        "HTTP/1.1 {status_code} {status_text}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

async fn exchange_code(
    client: &GoogleInstalledClient,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<GoogleTokenResponse> {
    let mut form = vec![
        ("client_id", client.client_id.clone()),
        ("code", code.to_string()),
        ("code_verifier", code_verifier.to_string()),
        ("grant_type", "authorization_code".to_string()),
        ("redirect_uri", redirect_uri.to_string()),
    ];
    if let Some(client_secret) = client.client_secret.clone() {
        form.push(("client_secret", client_secret));
    }

    let response = reqwest::Client::new()
        .post(&client.token_uri)
        .form(&form)
        .send()
        .await
        .context("Google token exchange failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Google token exchange failed ({status}): {body}"));
    }

    response
        .json::<GoogleTokenResponse>()
        .await
        .context("Failed to parse Google token response")
}

async fn exchange_refresh_token(
    client: &GoogleInstalledClient,
    refresh_token: &str,
) -> Result<GoogleTokenResponse> {
    let mut form = vec![
        ("client_id", client.client_id.clone()),
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.to_string()),
    ];
    if let Some(client_secret) = client.client_secret.clone() {
        form.push(("client_secret", client_secret));
    }

    let response = reqwest::Client::new()
        .post(&client.token_uri)
        .form(&form)
        .send()
        .await
        .context("Google token refresh failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Google token refresh failed ({status}): {body}"));
    }

    response
        .json::<GoogleTokenResponse>()
        .await
        .context("Failed to parse Google refresh response")
}

async fn fetch_gmail_email(access_token: &str) -> Result<String> {
    let response = reqwest::Client::new()
        .get("https://gmail.googleapis.com/gmail/v1/users/me/profile")
        .bearer_auth(access_token)
        .send()
        .await
        .context("Failed to call Gmail profile API")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Gmail API request failed ({status}): {body}"));
    }

    let profile = response
        .json::<GmailProfileResponse>()
        .await
        .context("Failed to parse Gmail profile response")?;
    Ok(profile.email_address)
}

async fn ensure_gmail_scope(token_response: &GoogleTokenResponse) -> Result<()> {
    let granted_scopes = match token_response.scope.clone() {
        Some(scope) if !scope.trim().is_empty() => scope,
        _ => fetch_tokeninfo_scopes(&token_response.access_token).await?,
    };

    if granted_scopes
        .split_whitespace()
        .any(|scope| scope == GOOGLE_SCOPE_GMAIL_READONLY)
    {
        return Ok(());
    }

    Err(anyhow!(
        "Google granted insufficient scopes. Expected {required}, got: {granted}",
        required = GOOGLE_SCOPE_GMAIL_READONLY,
        granted = granted_scopes
    ))
}

async fn fetch_tokeninfo_scopes(access_token: &str) -> Result<String> {
    let response = reqwest::Client::new()
        .get("https://www.googleapis.com/oauth2/v3/tokeninfo")
        .query(&[("access_token", access_token)])
        .send()
        .await
        .context("Failed to inspect Google access token")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Google token inspection failed ({status}): {body}"));
    }

    let info = response
        .json::<GoogleTokenInfoResponse>()
        .await
        .context("Failed to parse Google token inspection response")?;

    Ok(info.scope.unwrap_or_default())
}

fn random_token(size: usize) -> Result<String> {
    let mut bytes = vec![0u8; size];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| anyhow!("Failed to generate secure random token"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn pkce_challenge(code_verifier: &str) -> String {
    let digest = Sha256::digest(code_verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}
