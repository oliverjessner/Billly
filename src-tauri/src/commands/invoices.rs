use crate::models::{InvoiceDetail, InvoiceOverride, InvoiceSummary};
use crate::services::processor::process_invoice;
use crate::services::state::AppState;
use serde::Deserialize;
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInvoicePayload {
    pub invoice_id: String,
    pub field_name: String,
    pub value: String,
}

#[tauri::command]
pub async fn get_invoices(
    category: String,
    year: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<InvoiceSummary>, String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    let mut summaries = db
        .get_invoice_summaries(&category, year.as_deref())
        .map_err(|e| e.to_string())?;

    for summary in summaries.iter_mut() {
        let overrides = db.get_overrides(&summary.id).map_err(|e| e.to_string())?;
        apply_overrides_to_summary(summary, &overrides);
    }

    Ok(summaries)
}

#[tauri::command]
pub async fn get_invoice_years(category: String, state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    db.get_invoice_years(&category)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_invoice_detail(invoice_id: String, state: State<'_, AppState>) -> Result<InvoiceDetail, String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    let mut invoice = db
        .get_invoice_by_id(&invoice_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Invoice not found".to_string())?;
    let overrides = db.get_overrides(&invoice_id).map_err(|e| e.to_string())?;
    apply_overrides(&mut invoice, &overrides);
    Ok(InvoiceDetail { invoice, overrides })
}

#[tauri::command]
pub async fn update_invoice_field(payload: UpdateInvoicePayload, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    db.set_override(&payload.invoice_id, &payload.field_name, &payload.value)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn clear_overrides(invoice_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    db.clear_all_overrides(&invoice_id)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn clear_override(invoice_id: String, field_name: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
    db.clear_override(&invoice_id, &field_name)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn reprocess_invoice(invoice_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let invoice = {
        let db = state.db.lock().map_err(|_| "DB lock".to_string())?;
        db.get_invoice_by_id(&invoice_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Invoice not found".to_string())?
    };

    let path = invoice
        .file_path
        .clone()
        .ok_or_else(|| "Missing file path".to_string())?;
    let settings = state
        .settings
        .lock()
        .map_err(|_| "Settings lock".to_string())?
        .clone();

    process_invoice(&state.db, std::path::Path::new(&path), &invoice.category, &settings)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn open_invoice_file(path: String) -> Result<(), String> {
    open::that(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn apply_overrides(invoice: &mut crate::models::Invoice, overrides: &[InvoiceOverride]) {
    for override_entry in overrides {
        match override_entry.field_name.as_str() {
            "invoice_number" => invoice.invoice_number = Some(override_entry.override_value.clone()),
            "invoice_date" => invoice.invoice_date = Some(override_entry.override_value.clone()),
            "due_date" => invoice.due_date = Some(override_entry.override_value.clone()),
            "counterparty_name" => invoice.counterparty_name = Some(override_entry.override_value.clone()),
            "total_amount" => invoice.total_amount = override_entry.override_value.clone(),
            "currency" => invoice.currency = override_entry.override_value.clone(),
            "tax_amount" => invoice.tax_amount = Some(override_entry.override_value.clone()),
            "net_amount" => invoice.net_amount = Some(override_entry.override_value.clone()),
            "status" => invoice.status = override_entry.override_value.clone(),
            "paid_at" => invoice.paid_at = Some(override_entry.override_value.clone()),
            _ => {}
        }
    }
}

fn apply_overrides_to_summary(summary: &mut InvoiceSummary, overrides: &[InvoiceOverride]) {
    for override_entry in overrides {
        match override_entry.field_name.as_str() {
            "invoice_date" => summary.invoice_date = Some(override_entry.override_value.clone()),
            "counterparty_name" => summary.counterparty_name = Some(override_entry.override_value.clone()),
            "total_amount" => summary.total_amount = override_entry.override_value.clone(),
            "status" => summary.status = override_entry.override_value.clone(),
            _ => {}
        }
    }
}
