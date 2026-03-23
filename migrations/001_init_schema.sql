CREATE TABLE IF NOT EXISTS invoices (
    id TEXT PRIMARY KEY,
    category TEXT NOT NULL CHECK(category IN ('revenue', 'payable')),
    file_path TEXT UNIQUE,
    file_hash TEXT NOT NULL,
    file_modified_at TEXT NOT NULL,
    ingestion_status TEXT NOT NULL DEFAULT 'pending',
    ocr_text TEXT,
    extracted_json TEXT NOT NULL,
    confidence_score REAL NOT NULL DEFAULT 0.0,
    invoice_number TEXT,
    invoice_date TEXT,
    due_date TEXT,
    counterparty_name TEXT,
    total_amount TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'EUR',
    tax_amount TEXT,
    net_amount TEXT,
    status TEXT NOT NULL DEFAULT 'open',
    paid_at TEXT,
    document_language TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_invoices_category_date ON invoices(category, invoice_date);
CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices(status);

CREATE TABLE IF NOT EXISTS invoice_overrides (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL,
    field_name TEXT NOT NULL,
    override_value TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(invoice_id) REFERENCES invoices(id) ON DELETE CASCADE,
    UNIQUE(invoice_id, field_name)
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS processing_logs (
    id TEXT PRIMARY KEY,
    invoice_id TEXT,
    file_hash TEXT,
    process_type TEXT NOT NULL,
    status TEXT NOT NULL,
    message TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(invoice_id) REFERENCES invoices(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS customers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source_name TEXT NOT NULL,
    normalized_name TEXT NOT NULL UNIQUE,
    billing_address TEXT,
    general_email TEXT,
    latest_invoice_id TEXT,
    first_invoice_date TEXT,
    last_invoice_date TEXT,
    invoice_count INTEGER NOT NULL DEFAULT 0,
    lifetime_value REAL NOT NULL DEFAULT 0,
    avg_monthly_revenue REAL NOT NULL DEFAULT 0,
    active_months INTEGER NOT NULL DEFAULT 1,
    enrichment_status TEXT NOT NULL DEFAULT 'pending',
    enriched_at TEXT,
    enrichment_source_invoice_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(latest_invoice_id) REFERENCES invoices(id) ON DELETE SET NULL,
    FOREIGN KEY(enrichment_source_invoice_id) REFERENCES invoices(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_customers_lifetime_value ON customers(lifetime_value DESC);
CREATE INDEX IF NOT EXISTS idx_customers_last_invoice_date ON customers(last_invoice_date DESC);

CREATE TABLE IF NOT EXISTS customer_contacts (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    email TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_customer_contacts_customer_id ON customer_contacts(customer_id);
