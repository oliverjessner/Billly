use base64::{engine::general_purpose::STANDARD, engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{Local, NaiveDate};
use serde::Deserialize;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::commands::customers::sync_generated_customers;
use crate::commands::settings::{
    access_token_expires_at, load_saved_google_access_token, GOOGLE_ACCESS_TOKEN_EXPIRES_AT_KEY,
    GOOGLE_ACCESS_TOKEN_KEY, GOOGLE_EMAIL_KEY, GOOGLE_REFRESH_TOKEN_KEY,
};
use crate::db::Database;
use crate::models::{CustomerSummary, EmailInvoiceOption, EmailRecipientOption, EmailTemplate, EmailWorkspace};
use crate::services::crypto::CryptoService;
use crate::services::google_auth::GoogleAuthService;
use crate::services::state::AppState;
use crate::utils::{normalize_entity_name, now_rfc3339};

const EMAIL_SIGNATURE_KEY: &str = "email_signature";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEmailSignaturePayload {
    pub signature: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEmailTemplatePayload {
    pub template_id: Option<String>,
    pub name: String,
    pub subject: String,
    pub body: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendEmailPayload {
    pub recipient_id: String,
    pub invoice_id: String,
    pub subject: String,
    pub body: String,
}

#[tauri::command]
pub async fn get_email_workspace(state: State<'_, AppState>) -> Result<EmailWorkspace, String> {
    sync_generated_customers(&state)?;

    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    let signature = db.get_setting(EMAIL_SIGNATURE_KEY).map_err(|e| e.to_string())?;
    let sender_email = db.get_setting(GOOGLE_EMAIL_KEY).map_err(|e| e.to_string())?;
    let gmail_connected = db
        .get_setting(GOOGLE_REFRESH_TOKEN_KEY)
        .map_err(|e| e.to_string())?
        .as_deref()
        .map(|value| CryptoService::decrypt_secret(value).map(|token| !token.trim().is_empty()).unwrap_or(false))
        .unwrap_or(false);
    let templates = db.get_email_templates().map_err(|e| e.to_string())?;
    let customers = db.get_customer_summaries().map_err(|e| e.to_string())?;
    let recipients = collect_email_recipients(&db, &customers)?;
    let invoices = attach_customer_ids(customers, db.get_revenue_email_invoice_options().map_err(|e| e.to_string())?);

    Ok(EmailWorkspace {
        signature,
        sender_email,
        gmail_connected,
        templates,
        recipients,
        invoices,
    })
}

#[tauri::command]
pub async fn save_email_signature(
    payload: SaveEmailSignaturePayload,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    let signature = payload.signature.trim().to_string();
    if signature.is_empty() {
        db.delete_setting(EMAIL_SIGNATURE_KEY).map_err(|e| e.to_string())?;
    } else {
        db.set_setting(EMAIL_SIGNATURE_KEY, &signature)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn save_email_template(
    payload: SaveEmailTemplatePayload,
    state: State<'_, AppState>,
) -> Result<EmailTemplate, String> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err("Template name is required".to_string());
    }

    let now = now_rfc3339();
    let template = EmailTemplate {
        id: payload.template_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
        name: name.to_string(),
        subject: payload.subject,
        body: payload.body,
        created_at: now.clone(),
        updated_at: now,
    };

    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    let created_at = db
        .get_email_templates()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|existing| existing.id == template.id)
        .map(|existing| existing.created_at)
        .unwrap_or_else(|| template.created_at.clone());

    let template = EmailTemplate {
        created_at,
        ..template
    };
    db.upsert_email_template(&template).map_err(|e| e.to_string())?;
    Ok(template)
}

#[tauri::command]
pub async fn delete_email_template(template_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    db.delete_email_template(&template_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_email(
    payload: SendEmailPayload,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    sync_generated_customers(&state)?;

    let subject = payload.subject.trim();
    if subject.is_empty() {
        return Err("Subject is required".to_string());
    }

    let (recipient, invoice, sender_email, signature) = {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        let customers = db.get_customer_summaries().map_err(|e| e.to_string())?;
        let recipients = collect_email_recipients(&db, &customers)?;
        let recipient = recipients
            .into_iter()
            .find(|item| item.id == payload.recipient_id)
            .ok_or_else(|| "Recipient not found".to_string())?;
        let invoice = db
            .get_revenue_email_invoice_options()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|item| item.id == payload.invoice_id)
            .ok_or_else(|| "Revenue invoice not found".to_string())?;
        let sender_email = db
            .get_setting(GOOGLE_EMAIL_KEY)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Connect Google before sending emails".to_string())?;
        let signature = db.get_setting(EMAIL_SIGNATURE_KEY).map_err(|e| e.to_string())?;
        (recipient, invoice, sender_email, signature)
    };

    let attachment_path = invoice
        .file_path
        .clone()
        .ok_or_else(|| "Selected invoice does not have a file path".to_string())?;
    let attachment_bytes = std::fs::read(&attachment_path)
        .map_err(|e| format!("Failed to read invoice attachment: {e}"))?;
    let attachment_name = attachment_file_name(&attachment_path, invoice.invoice_number.as_deref());

    let rendered_subject = render_email_placeholders(subject, &recipient, invoice.invoice_date.as_deref());
    let rendered_body = render_email_body(
        &payload.body,
        signature.as_deref(),
        &recipient,
        invoice.invoice_date.as_deref(),
    );
    let mime_message = build_mime_message(
        &sender_email,
        &recipient.email,
        &rendered_subject,
        &rendered_body,
        &attachment_name,
        &attachment_bytes,
    );
    let raw_message = URL_SAFE_NO_PAD.encode(mime_message.as_bytes());
    let access_token = ensure_google_access_token(&state, &app).await?;
    if let Err(error) = GoogleAuthService::send_gmail_message(&access_token, &raw_message).await {
        let message = error.to_string();
        if message.contains("401 Unauthorized") {
            {
                let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
                db.delete_setting(GOOGLE_ACCESS_TOKEN_KEY)
                    .map_err(|e| e.to_string())?;
                db.delete_setting(GOOGLE_ACCESS_TOKEN_EXPIRES_AT_KEY)
                    .map_err(|e| e.to_string())?;
            }

            let refreshed_access_token = ensure_google_access_token(&state, &app).await?;
            GoogleAuthService::send_gmail_message(&refreshed_access_token, &raw_message)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            return Err(message);
        }
    }

    let send_date = Local::now().date_naive().format("%Y-%m-%d").to_string();
    {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        db.set_invoice_send_date_by_path(&attachment_path, &send_date)
            .map_err(|e| e.to_string())?;
    }

    Ok(recipient.email)
}

fn collect_email_recipients(db: &Database, customers: &[CustomerSummary]) -> Result<Vec<EmailRecipientOption>, String> {
    let mut recipients = Vec::new();

    for customer in customers {
        if let Some(email) = normalize_email(customer.general_email.as_deref()) {
            recipients.push(EmailRecipientOption {
                id: format!("customer:{}", customer.id),
                customer_id: customer.id.clone(),
                customer_name: customer.name.clone(),
                contact_id: None,
                display_name: customer.name.clone(),
                email,
                kind: "customer".to_string(),
            });
        }

        for contact in db
            .get_customer_contacts(&customer.id)
            .map_err(|e| e.to_string())?
            .into_iter()
        {
            let Some(email) = normalize_email(Some(contact.email.as_str())) else {
                continue;
            };
            let display_name = if contact.name.trim().is_empty() {
                customer.name.clone()
            } else {
                contact.name.trim().to_string()
            };
            recipients.push(EmailRecipientOption {
                id: format!("contact:{}", contact.id),
                customer_id: customer.id.clone(),
                customer_name: customer.name.clone(),
                contact_id: Some(contact.id),
                display_name,
                email,
                kind: "contact".to_string(),
            });
        }
    }

    recipients.sort_by(|left, right| {
        left.customer_name
            .to_lowercase()
            .cmp(&right.customer_name.to_lowercase())
            .then(left.kind.cmp(&right.kind))
            .then(left.display_name.to_lowercase().cmp(&right.display_name.to_lowercase()))
            .then(left.email.to_lowercase().cmp(&right.email.to_lowercase()))
    });

    Ok(recipients)
}

fn attach_customer_ids(customers: Vec<CustomerSummary>, invoices: Vec<EmailInvoiceOption>) -> Vec<EmailInvoiceOption> {
    let customer_ids_by_name = customers
        .into_iter()
        .map(|customer| (customer.normalized_name, customer.id))
        .collect::<std::collections::HashMap<_, _>>();

    invoices
        .into_iter()
        .map(|mut invoice| {
            invoice.customer_id = invoice
                .customer_name
                .as_deref()
                .map(normalize_entity_name)
                .and_then(|normalized| customer_ids_by_name.get(&normalized).cloned());
            invoice
        })
        .collect()
}

async fn ensure_google_access_token(state: &State<'_, AppState>, app: &AppHandle) -> Result<String, String> {
    if let Some(saved_access_token) = load_saved_google_access_token(state)? {
        GoogleAuthService::ensure_required_gmail_scope_for_access_token(&saved_access_token)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(saved_access_token);
    }

    let refresh_token = {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        let encrypted = db
            .get_setting(GOOGLE_REFRESH_TOKEN_KEY)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Connect Google before sending emails".to_string())?;
        CryptoService::decrypt_secret(&encrypted).map_err(|e| e.to_string())?
    };

    let refresh_session = match GoogleAuthService::refresh_access_token(app, &refresh_token).await {
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
                db.delete_setting(GOOGLE_EMAIL_KEY).map_err(|e| e.to_string())?;
                return Err(
                    "Google login expired or was revoked. Connect Google again to restore Gmail access."
                        .to_string(),
                );
            }
            if message.contains("insufficient scopes") {
                return Err(
                    "Google login is missing Gmail send permission. Update the Gmail scope in Google Cloud, then reconnect Google."
                        .to_string(),
                );
            }
            return Err(message);
        }
    };

    let email = GoogleAuthService::fetch_google_email(&refresh_session.access_token)
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
    }

    Ok(refresh_session.access_token)
}

fn normalize_email(value: Option<&str>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn render_email_placeholders(
    template: &str,
    recipient: &EmailRecipientOption,
    invoice_date: Option<&str>,
) -> String {
    let (month, year) = email_date_placeholders(invoice_date);
    template
        .replace("<name>", &recipient.display_name)
        .replace("<email>", &recipient.email)
        .replace("<month>", &month)
        .replace("<year>", &year)
}

fn render_email_body(
    body: &str,
    signature: Option<&str>,
    recipient: &EmailRecipientOption,
    invoice_date: Option<&str>,
) -> String {
    let body = render_email_placeholders(body, recipient, invoice_date)
        .trim()
        .to_string();
    let signature = signature.unwrap_or("").trim();
    if body.is_empty() {
        return signature.to_string();
    }
    if signature.is_empty() {
        return body;
    }
    format!("{body}\n\n{signature}")
}

fn email_date_placeholders(invoice_date: Option<&str>) -> (String, String) {
    let Some(raw) = invoice_date else {
        return (String::new(), String::new());
    };
    let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") else {
        return (String::new(), String::new());
    };

    (date.format("%B").to_string(), date.format("%Y").to_string())
}

fn build_mime_message(
    from_email: &str,
    to_email: &str,
    subject: &str,
    body: &str,
    attachment_name: &str,
    attachment_bytes: &[u8],
) -> String {
    let boundary = format!("billly-{}", Uuid::new_v4().simple());
    let content_type = if attachment_name.to_ascii_lowercase().ends_with(".pdf") {
        "application/pdf"
    } else {
        "application/octet-stream"
    };
    let encoded_subject = encode_header(subject);
    let wrapped_attachment = wrap_base64_lines(&STANDARD.encode(attachment_bytes));
    let normalized_body = normalize_crlf(body);

    format!(
        "From: {from_email}\r\n\
To: {to_email}\r\n\
Subject: {encoded_subject}\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"{boundary}\"\r\n\
\r\n\
--{boundary}\r\n\
Content-Type: text/plain; charset=\"UTF-8\"\r\n\
Content-Transfer-Encoding: 8bit\r\n\
\r\n\
{normalized_body}\r\n\
--{boundary}\r\n\
Content-Type: {content_type}; name=\"{attachment_name}\"\r\n\
Content-Transfer-Encoding: base64\r\n\
Content-Disposition: attachment; filename=\"{attachment_name}\"\r\n\
\r\n\
{wrapped_attachment}\r\n\
--{boundary}--\r\n"
    )
}

fn attachment_file_name(file_path: &str, invoice_number: Option<&str>) -> String {
    let candidate = std::path::Path::new(file_path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            invoice_number
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| format!("invoice-{value}.pdf"))
        })
        .unwrap_or_else(|| "invoice.pdf".to_string());

    let mut sanitized = candidate
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();

    if sanitized.trim_matches('_').is_empty() {
        sanitized = "invoice.pdf".to_string();
    }
    sanitized
}

fn encode_header(value: &str) -> String {
    if value.is_ascii() {
        value.to_string()
    } else {
        format!("=?UTF-8?B?{}?=", STANDARD.encode(value.as_bytes()))
    }
}

fn normalize_crlf(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n").replace('\n', "\r\n")
}

fn wrap_base64_lines(value: &str) -> String {
    let mut wrapped = String::with_capacity(value.len() + value.len() / 76);
    for chunk in value.as_bytes().chunks(76) {
        wrapped.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        wrapped.push_str("\r\n");
    }
    wrapped.trim_end().to_string()
}
