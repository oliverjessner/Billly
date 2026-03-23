use std::collections::HashMap;

use chrono::{Datelike, NaiveDate, Utc};
use serde::Deserialize;
use tauri::State;

use crate::models::{Customer, CustomerContact, CustomerDetail, CustomerSummary};
use crate::services::crypto::CryptoService;
use crate::services::openai::OpenAIExtractor;
use crate::services::state::AppState;
use crate::utils::{normalize_api_key, normalize_entity_name, now_rfc3339, parse_decimal, redact_profile_data};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCustomerPayload {
    pub customer_id: String,
    pub field_name: String,
    pub value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCustomerContactPayload {
    pub contact_id: String,
    pub field_name: String,
    pub value: String,
}

#[tauri::command]
pub async fn get_customers(state: State<'_, AppState>) -> Result<Vec<CustomerSummary>, String> {
    sync_generated_customers(&state)?;
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    db.get_customer_summaries().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_customer_detail(customer_id: String, state: State<'_, AppState>) -> Result<CustomerDetail, String> {
    sync_generated_customers(&state)?;
    ensure_customer_enriched(&state, &customer_id).await?;

    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    let customer = db
        .get_customer_by_id(&customer_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Customer not found".to_string())?;
    let contacts = db
        .get_customer_contacts(&customer_id)
        .map_err(|e| e.to_string())?;
    Ok(CustomerDetail { customer, contacts })
}

#[tauri::command]
pub async fn update_customer_field(payload: UpdateCustomerPayload, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    db.update_customer_field(&payload.customer_id, &payload.field_name, &payload.value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_customer_contact(customer_id: String, state: State<'_, AppState>) -> Result<CustomerContact, String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    db.add_customer_contact(&customer_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_customer_contact_field(
    payload: UpdateCustomerContactPayload,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    db.update_customer_contact_field(&payload.contact_id, &payload.field_name, &payload.value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_customer_contact(contact_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    db.delete_customer_contact(&contact_id)
        .map_err(|e| e.to_string())
}

fn sync_generated_customers(state: &State<'_, AppState>) -> Result<(), String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "Settings lock".to_string())?
        .clone();
    let own_company_name = settings.company_name.clone();
    let own_company_normalized = own_company_name
        .as_deref()
        .map(normalize_entity_name)
        .filter(|value| !value.is_empty());
    let own_address_normalized = settings
        .company_address
        .as_deref()
        .map(normalize_profile_value)
        .filter(|value| !value.is_empty());
    let own_email_normalized = settings
        .company_email
        .as_deref()
        .map(normalize_profile_value)
        .filter(|value| !value.is_empty());

    let source_rows = {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        if let Some(own_company_normalized) = own_company_normalized.as_deref() {
            db.delete_customer_by_normalized_name(own_company_normalized)
                .map_err(|e| e.to_string())?;
        }
        db.get_revenue_customer_source_invoices()
            .map_err(|e| e.to_string())?
    };

    let mut aggregates: HashMap<String, CustomerAggregate> = HashMap::new();
    for row in source_rows {
        let normalized_name = normalize_entity_name(&row.counterparty_name);
        if normalized_name.is_empty() {
            continue;
        }
        if own_company_normalized
            .as_ref()
            .map(|own| own == &normalized_name)
            .unwrap_or(false)
        {
            continue;
        }

        let entry = aggregates.entry(normalized_name).or_insert_with(|| CustomerAggregate {
            source_name: row.counterparty_name.clone(),
            latest_invoice_id: row.id.clone(),
            latest_invoice_date: row.invoice_date.clone(),
            latest_created_at: row.created_at.clone(),
            first_invoice_date: row.invoice_date.clone(),
            last_invoice_date: row.invoice_date.clone(),
            invoice_count: 0,
            lifetime_value: 0.0,
        });

        entry.invoice_count += 1;
        entry.lifetime_value += parse_decimal(&row.total_amount).unwrap_or(0.0);

        if let Some(invoice_date) = row.invoice_date.clone() {
            if entry
                .first_invoice_date
                .as_ref()
                .map(|current| invoice_date < *current)
                .unwrap_or(true)
            {
                entry.first_invoice_date = Some(invoice_date.clone());
            }
            if entry
                .last_invoice_date
                .as_ref()
                .map(|current| invoice_date > *current)
                .unwrap_or(true)
            {
                entry.last_invoice_date = Some(invoice_date.clone());
            }
        }

        if is_row_newer(
            row.invoice_date.as_deref(),
            &row.created_at,
            entry.latest_invoice_date.as_deref(),
            &entry.latest_created_at,
        ) {
            entry.source_name = row.counterparty_name.clone();
            entry.latest_invoice_id = row.id.clone();
            entry.latest_invoice_date = row.invoice_date.clone();
            entry.latest_created_at = row.created_at.clone();
        }
    }

    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    for (normalized_name, aggregate) in aggregates {
        let existing = db
            .get_customer_by_normalized_name(&normalized_name)
            .map_err(|e| e.to_string())?;
        let now = now_rfc3339();
        let active_months = calculate_active_months(aggregate.first_invoice_date.as_deref());
        let avg_monthly_revenue = if active_months > 0 {
            aggregate.lifetime_value / active_months as f64
        } else {
            aggregate.lifetime_value
        };

        let customer = if let Some(mut customer) = existing {
            let removed_own_profile_data = strip_own_profile_data(
                &mut customer,
                own_address_normalized.as_deref(),
                own_email_normalized.as_deref(),
            );
            let preserve_manual_name = !customer.name.trim().is_empty() && customer.name != customer.source_name;
            customer.name = if preserve_manual_name {
                customer.name.clone()
            } else {
                aggregate.source_name.clone()
            };
            customer.source_name = aggregate.source_name.clone();
            customer.normalized_name = normalized_name.clone();
            customer.latest_invoice_id = Some(aggregate.latest_invoice_id.clone());
            customer.first_invoice_date = aggregate.first_invoice_date.clone();
            customer.last_invoice_date = aggregate.last_invoice_date.clone();
            customer.invoice_count = aggregate.invoice_count;
            customer.lifetime_value = aggregate.lifetime_value;
            customer.avg_monthly_revenue = avg_monthly_revenue;
            customer.active_months = active_months;
            if removed_own_profile_data {
                customer.enrichment_status = "pending".to_string();
                customer.enriched_at = None;
                customer.enrichment_source_invoice_id = None;
            }
            customer.updated_at = now.clone();
            customer
        } else {
            Customer {
                id: uuid::Uuid::new_v4().to_string(),
                name: aggregate.source_name.clone(),
                source_name: aggregate.source_name.clone(),
                normalized_name: normalized_name.clone(),
                billing_address: None,
                general_email: None,
                latest_invoice_id: Some(aggregate.latest_invoice_id.clone()),
                first_invoice_date: aggregate.first_invoice_date.clone(),
                last_invoice_date: aggregate.last_invoice_date.clone(),
                invoice_count: aggregate.invoice_count,
                lifetime_value: aggregate.lifetime_value,
                avg_monthly_revenue,
                active_months,
                enrichment_status: "pending".to_string(),
                enriched_at: None,
                enrichment_source_invoice_id: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            }
        };

        db.save_customer(&customer).map_err(|e| e.to_string())?;
    }

    Ok(())
}

async fn ensure_customer_enriched(state: &State<'_, AppState>, customer_id: &str) -> Result<(), String> {
    let (openai_api_key, company_name, company_email, company_address) = {
        let settings = state
            .settings
            .lock()
            .map_err(|_| "Settings lock".to_string())?
            .clone();
        let Some(encrypted_key) = settings.openai_api_key else {
            return Ok(());
        };
        let Ok(decrypted) = CryptoService::decrypt_api_key(&encrypted_key) else {
            return Ok(());
        };
        let normalized = normalize_api_key(&decrypted);
        if normalized.is_empty() {
            return Ok(());
        }
        (
            normalized,
            settings.company_name,
            settings.company_email,
            settings.company_address,
        )
    };
    let own_address_normalized = company_address
        .as_deref()
        .map(normalize_profile_value)
        .filter(|value| !value.is_empty());
    let own_email_normalized = company_email
        .as_deref()
        .map(normalize_profile_value)
        .filter(|value| !value.is_empty());

    let mut customer = {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        match db.get_customer_by_id(customer_id).map_err(|e| e.to_string())? {
            Some(customer) => customer,
            None => return Ok(()),
        }
    };

    let removed_own_profile_data = strip_own_profile_data(
        &mut customer,
        own_address_normalized.as_deref(),
        own_email_normalized.as_deref(),
    );
    if removed_own_profile_data {
        customer.enrichment_status = "pending".to_string();
        customer.enriched_at = None;
        customer.enrichment_source_invoice_id = None;
        customer.updated_at = now_rfc3339();
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        db.save_customer(&customer).map_err(|e| e.to_string())?;
    } else if customer.enrichment_status != "pending" {
        return Ok(());
    }

    let invoice_text = {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        let Some(latest_invoice_id) = customer.latest_invoice_id.clone() else {
            return Ok(());
        };
        let Some(invoice) = db
            .get_invoice_by_id(&latest_invoice_id)
            .map_err(|e| e.to_string())?
        else {
            return Ok(());
        };
        invoice.ocr_text.unwrap_or_default()
    };

    let sanitized_text = redact_profile_data(
        &invoice_text,
        company_name.as_deref(),
        company_email.as_deref(),
        company_address.as_deref(),
    );

    if sanitized_text.trim().is_empty() {
        customer.enrichment_status = "skipped".to_string();
        customer.enriched_at = Some(now_rfc3339());
        customer.enrichment_source_invoice_id = customer.latest_invoice_id.clone();
        customer.updated_at = now_rfc3339();
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        db.save_customer(&customer).map_err(|e| e.to_string())?;
        return Ok(());
    }

    match OpenAIExtractor::extract_customer_data(&openai_api_key, &sanitized_text).await {
        Ok(extracted) => {
            let extracted_billing_address = normalize_optional_text(extracted.billing_address)
                .filter(|value| !matches_own_profile_value(value, own_address_normalized.as_deref()));
            let extracted_general_email = normalize_optional_text(extracted.general_email)
                .filter(|value| !matches_own_profile_value(value, own_email_normalized.as_deref()));
            if customer
                .billing_address
                .as_deref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
            {
                customer.billing_address = extracted_billing_address;
            }
            if customer
                .general_email
                .as_deref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
            {
                customer.general_email = extracted_general_email;
            }
            customer.enrichment_status = "completed".to_string();
            customer.enriched_at = Some(now_rfc3339());
            customer.enrichment_source_invoice_id = customer.latest_invoice_id.clone();
            customer.updated_at = now_rfc3339();
            let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
            db.save_customer(&customer).map_err(|e| e.to_string())?;
        }
        Err(_) => {
            customer.enrichment_status = "failed".to_string();
            customer.enriched_at = Some(now_rfc3339());
            customer.enrichment_source_invoice_id = customer.latest_invoice_id.clone();
            customer.updated_at = now_rfc3339();
            let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
            db.save_customer(&customer).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

struct CustomerAggregate {
    source_name: String,
    latest_invoice_id: String,
    latest_invoice_date: Option<String>,
    latest_created_at: String,
    first_invoice_date: Option<String>,
    last_invoice_date: Option<String>,
    invoice_count: i64,
    lifetime_value: f64,
}

fn calculate_active_months(first_invoice_date: Option<&str>) -> i64 {
    let Some(raw_date) = first_invoice_date else {
        return 1;
    };
    let Ok(first_date) = NaiveDate::parse_from_str(raw_date, "%Y-%m-%d") else {
        return 1;
    };
    let today = Utc::now().date_naive();
    let months = ((today.year() - first_date.year()) * 12)
        + (today.month() as i32 - first_date.month() as i32)
        + 1;
    months.max(1) as i64
}

fn is_row_newer(
    row_invoice_date: Option<&str>,
    row_created_at: &str,
    current_invoice_date: Option<&str>,
    current_created_at: &str,
) -> bool {
    match (row_invoice_date, current_invoice_date) {
        (Some(row), Some(current)) => {
            if row != current {
                row > current
            } else {
                row_created_at > current_created_at
            }
        }
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => row_created_at > current_created_at,
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_profile_value(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace() && !",;:|/\\-()".contains(*ch))
        .collect()
}

fn matches_own_profile_value(value: &str, own_value: Option<&str>) -> bool {
    let Some(own_value) = own_value else {
        return false;
    };
    let normalized = normalize_profile_value(value);
    !normalized.is_empty() && normalized == own_value
}

fn strip_own_profile_data(
    customer: &mut Customer,
    own_address: Option<&str>,
    own_email: Option<&str>,
) -> bool {
    let mut changed = false;

    if customer
        .billing_address
        .as_deref()
        .map(|value| matches_own_profile_value(value, own_address))
        .unwrap_or(false)
    {
        customer.billing_address = None;
        changed = true;
    }

    if customer
        .general_email
        .as_deref()
        .map(|value| matches_own_profile_value(value, own_email))
        .unwrap_or(false)
    {
        customer.general_email = None;
        changed = true;
    }

    changed
}
