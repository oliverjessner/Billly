use serde::{Deserialize, Serialize};

pub const PAYABLE_TAG_OPTIONS: [&str; 7] = ["Device", "SaaS", "Insurance", "Membership", "Marketing", "Domain", "Rent"];

pub fn normalize_payable_tags(tags: Vec<String>) -> Vec<String> {
    PAYABLE_TAG_OPTIONS
        .iter()
        .filter(|option| tags.iter().any(|tag| tag.eq_ignore_ascii_case(option)))
        .map(|option| option.to_string())
        .collect()
}

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
    pub document_language: Option<String>,
    pub tags: Vec<String>,
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
pub struct Customer {
    pub id: String,
    pub name: String,
    pub source_name: String,
    pub normalized_name: String,
    pub billing_address: Option<String>,
    pub general_email: Option<String>,
    pub latest_invoice_id: Option<String>,
    pub first_invoice_date: Option<String>,
    pub last_invoice_date: Option<String>,
    pub invoice_count: i64,
    pub lifetime_value: f64,
    pub avg_monthly_revenue: f64,
    pub active_months: i64,
    pub enrichment_status: String,
    pub enriched_at: Option<String>,
    pub enrichment_source_invoice_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerContact {
    pub id: String,
    pub customer_id: String,
    pub name: String,
    pub email: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerSummary {
    pub id: String,
    pub name: String,
    pub general_email: Option<String>,
    pub first_invoice_date: Option<String>,
    pub last_invoice_date: Option<String>,
    pub invoice_count: i64,
    pub contact_count: i64,
    pub lifetime_value: f64,
    pub avg_monthly_revenue: f64,
    pub active_months: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerDetail {
    pub customer: Customer,
    pub contacts: Vec<CustomerContact>,
}

#[derive(Debug, Clone)]
pub struct CustomerSourceInvoice {
    pub id: String,
    pub counterparty_name: String,
    pub invoice_date: Option<String>,
    pub total_amount: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub revenue_folder: Option<String>,
    pub payable_folder: Option<String>,
    pub company_name: Option<String>,
    pub company_email: Option<String>,
    pub company_address: Option<String>,
    pub openai_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPublic {
    pub revenue_folder: Option<String>,
    pub payable_folder: Option<String>,
    pub company_name: Option<String>,
    pub company_email: Option<String>,
    pub company_address: Option<String>,
    pub openai_api_key_present: bool,
    pub openai_api_key_last4: Option<String>,
    pub google_connected: bool,
    pub google_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardListItem {
    pub label: String,
    pub total_amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub revenue_month: f64,
    pub revenue_year: f64,
    pub payable_month: f64,
    pub payable_year: f64,
    pub profit_month: f64,
    pub profit_year: f64,
    pub overdue_revenue: f64,
    pub overdue_payables: f64,
    pub open_revenue_items: Vec<InvoiceSummary>,
    pub open_payables_items: Vec<InvoiceSummary>,
    pub best_customers: Vec<DashboardListItem>,
    pub highest_costs: Vec<DashboardListItem>,
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
    pub tags: Option<Vec<String>>,
    pub extraction_notes: String,
    pub confidence_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedCustomerData {
    pub billing_address: Option<String>,
    pub general_email: Option<String>,
}
