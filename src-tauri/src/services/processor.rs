use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::db::Database;
use crate::models::{ExtractedInvoiceData, Invoice, Settings};
use crate::services::crypto::CryptoService;
use crate::services::openai::OpenAIExtractor;
use crate::services::text_extraction::TextExtractor;
use crate::utils::{
    format_decimal, modified_time_rfc3339, normalize_api_key, normalize_date, now_rfc3339, sha256_file,
};

pub async fn process_invoice(
    db: &Arc<Mutex<Database>>,
    path: &Path,
    category: &str,
    settings: &Settings,
    force: bool,
) -> Result<Invoice> {
    let file_path = path.to_string_lossy().to_string();
    let file_hash = sha256_file(path)?;
    let file_modified_at = modified_time_rfc3339(path)?;

    let existing = {
        let db = db.lock().map_err(|_| anyhow!("DB lock poisoned"))?;
        db.get_invoice_by_path(&file_path)?
    };

    if !force {
        if let Some(existing) = &existing {
            if existing.file_hash == file_hash && existing.file_modified_at == file_modified_at {
                return Ok(existing.clone());
            }
        }
    }

    if existing.is_none() && !force {
        let by_hash = {
            let db = db.lock().map_err(|_| anyhow!("DB lock poisoned"))?;
            db.get_invoice_by_hash(&file_hash)?
        };

        if let Some(mut existing) = by_hash {
            if existing.category == category {
                if existing.file_path.as_deref() != Some(&file_path) {
                    existing.file_path = Some(file_path.clone());
                    existing.file_modified_at = file_modified_at.clone();
                    existing.updated_at = now_rfc3339();
                    let db = db.lock().map_err(|_| anyhow!("DB lock poisoned"))?;
                    db.upsert_invoice(&existing)?;
                }
                return Ok(existing);
            }
        }
    }

    let now = now_rfc3339();
    let mut invoice = existing.unwrap_or_else(|| Invoice {
        id: uuid::Uuid::new_v4().to_string(),
        category: category.to_string(),
        file_path: Some(file_path.clone()),
        file_hash: file_hash.clone(),
        file_modified_at: file_modified_at.clone(),
        ingestion_status: "pending".to_string(),
        ocr_text: None,
        extracted_json: "{}".to_string(),
        confidence_score: 0.0,
        invoice_number: None,
        invoice_date: None,
        due_date: None,
        counterparty_name: None,
        total_amount: "0.00".to_string(),
        currency: "EUR".to_string(),
        tax_amount: None,
        net_amount: None,
        status: "open".to_string(),
        paid_at: None,
        document_language: None,
        tags: Vec::new(),
        created_at: now.clone(),
        updated_at: now.clone(),
    });

    invoice.file_hash = file_hash.clone();
    invoice.file_modified_at = file_modified_at.clone();
    invoice.file_path = Some(file_path.clone());
    invoice.ingestion_status = "pending".to_string();
    invoice.updated_at = now.clone();

    {
        let db = db.lock().map_err(|_| anyhow!("DB lock poisoned"))?;
        db.upsert_invoice(&invoice)?;
    }

    let extracted = TextExtractor::extract_from_pdf(path)?;
    invoice.ocr_text = Some(extracted.text.clone());
    invoice.document_language = extracted.document_language;

    let api_key = settings
        .openai_api_key
        .as_ref()
        .ok_or_else(|| anyhow!("OpenAI API key missing"))?;
    let decrypted_key = CryptoService::decrypt_api_key(api_key)?;
    let normalized_key = normalize_api_key(&decrypted_key);
    if normalized_key.is_empty() {
        return Err(anyhow!("OpenAI API key is empty or invalid"));
    }

    let (data, raw_json) = OpenAIExtractor::extract_invoice_data(&normalized_key, invoice.ocr_text.as_deref().unwrap_or_default(), category).await?;
    apply_extracted(&mut invoice, data, raw_json);
    invoice.ingestion_status = "processed".to_string();
    invoice.updated_at = now_rfc3339();

    {
        let db = db.lock().map_err(|_| anyhow!("DB lock poisoned"))?;
        db.upsert_invoice(&invoice)?;
        db.log_processing(
            Some(&invoice.id),
            Some(&invoice.file_hash),
            "process",
            "success",
            None,
        )?;
    }

    Ok(invoice)
}

pub fn mark_failed(db: &Arc<Mutex<Database>>, invoice: &mut Invoice, message: &str) -> Result<()> {
    invoice.ingestion_status = "failed".to_string();
    invoice.updated_at = now_rfc3339();
    let db = db.lock().map_err(|_| anyhow!("DB lock poisoned"))?;
    db.upsert_invoice(invoice)?;
    db.log_processing(
        Some(&invoice.id),
        Some(&invoice.file_hash),
        "process",
        "failed",
        Some(message),
    )?;
    Ok(())
}

fn apply_extracted(invoice: &mut Invoice, data: ExtractedInvoiceData, raw_json: String) {
    invoice.extracted_json = raw_json;
    invoice.invoice_number = data.invoice_number;
    invoice.invoice_date = normalize_date(data.invoice_date);
    invoice.due_date = normalize_date(data.due_date);
    invoice.counterparty_name = data.counterparty_name;
    if let Some(total) = data.total_amount {
        let normalized = if invoice.category == "payable" { total.abs() } else { total };
        invoice.total_amount = format_decimal(normalized);
    }
    if let Some(currency) = data.currency {
        invoice.currency = currency;
    }
    invoice.tax_amount = data
        .tax_amount
        .map(|value| if invoice.category == "payable" { value.abs() } else { value })
        .map(format_decimal);
    invoice.net_amount = data
        .net_amount
        .map(|value| if invoice.category == "payable" { value.abs() } else { value })
        .map(format_decimal);
    if invoice.category == "payable" && invoice.tags.is_empty() {
        invoice.tags = data.tags.unwrap_or_default();
    }
    invoice.confidence_score = data.confidence_score.unwrap_or(0.5);
}
