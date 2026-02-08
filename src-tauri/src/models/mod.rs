use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: String,
    pub category: String,
    pub file_path: Option<String>,
    pub file_hash: String,
    pub file_modified_at: String,
    pub ingestion_status: String,
    pub ocr_text: Option<String>,
    pub extracted_json: String,
    pub confidence_score: f64,
    pub invoice_number: Option<String>,
    pub invoice_date: Option<String>,
    pub due_date: Option<String>,
    pub counterparty_name: Option<String>,
    pub total_amount: String,
    pub currency: String,
    pub tax_amount: Option<String>,
    pub net_amount: Option<String>,
    pub status: String,
    pub paid_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceOverride {
    pub id: String,
    pub invoice_id: String,
    pub field_name: String,
    pub override_value: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceSummary {
    pub id: String,
    pub invoice_date: Option<String>,
    pub counterparty_name: Option<String>,
    pub total_amount: String,
    pub status: String,
    pub confidence_score: f64,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceDetail {
    pub invoice: Invoice,
    pub overrides: Vec<InvoiceOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub revenue_folder: Option<String>,
    pub payable_folder: Option<String>,
    pub openai_api_key: Option<String>,
    pub ocr_language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPublic {
    pub revenue_folder: Option<String>,
    pub payable_folder: Option<String>,
    pub ocr_language: String,
    pub openai_api_key_present: bool,
    pub openai_api_key_last4: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub revenue_month: f64,
    pub revenue_year: f64,
    pub payable_month: f64,
    pub payable_year: f64,
    pub paid_payables_month: f64,
    pub profit_month: f64,
    pub profit_year: f64,
    pub open_payables: f64,
    pub open_revenue: f64,
    pub paid_revenue_month: f64,
    pub recent_revenue: Vec<InvoiceSummary>,
    pub recent_payables: Vec<InvoiceSummary>,
    pub chart_months: Vec<String>,
    pub chart_revenue: Vec<f64>,
    pub chart_payables: Vec<f64>,
    pub chart_profit: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedInvoiceData {
    pub invoice_number: Option<String>,
    pub invoice_date: Option<String>,
    pub due_date: Option<String>,
    pub counterparty_name: Option<String>,
    pub total_amount: Option<f64>,
    pub currency: Option<String>,
    pub tax_amount: Option<f64>,
    pub net_amount: Option<f64>,
    pub extraction_notes: String,
    pub confidence_score: Option<f64>,
}
