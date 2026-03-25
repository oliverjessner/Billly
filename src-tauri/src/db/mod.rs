use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde_json;
use std::path::PathBuf;
use uuid::Uuid;

use crate::models::{
    Customer, CustomerContact, CustomerSourceInvoice, CustomerSummary, DashboardListItem, EmailInvoiceOption,
    EmailTemplate, Invoice, InvoiceOverride, InvoiceSummary,
};
use crate::utils::now_rfc3339;

pub struct Database {
    conn: Connection,
}

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_init_schema.sql",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../migrations/001_init_schema.sql")),
    ),
    (
        "002_add_email_templates.sql",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../migrations/002_add_email_templates.sql")),
    ),
    (
        "003_add_invoice_send_date.sql",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../migrations/003_add_invoice_send_date.sql")),
    ),
];

impl Database {
    pub fn new(db_path: PathBuf) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let mut db = Database { conn };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&mut self) -> SqlResult<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                name TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
        )?;

        for (name, sql) in MIGRATIONS {
            if self.is_migration_applied(name)? {
                continue;
            }
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (name, applied_at) VALUES (?1, datetime('now'))",
                params![name],
            )?;
            tx.commit()?;
        }

        Ok(())
    }

    fn is_migration_applied(&self, name: &str) -> SqlResult<bool> {
        let applied: Option<String> = self
            .conn
            .query_row(
                "SELECT name FROM schema_migrations WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;
        Ok(applied.is_some())
    }

    pub fn upsert_invoice(&self, invoice: &Invoice) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO invoices (
                id, category, file_path, file_hash, file_modified_at, ingestion_status,
                ocr_text, extracted_json, confidence_score, invoice_number, invoice_date,
                due_date, counterparty_name, total_amount, currency, tax_amount, net_amount,
                status, paid_at, send_date, document_language, tags, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
            params![
                invoice.id,
                invoice.category,
                invoice.file_path,
                invoice.file_hash,
                invoice.file_modified_at,
                invoice.ingestion_status,
                invoice.ocr_text,
                invoice.extracted_json,
                invoice.confidence_score,
                invoice.invoice_number,
                invoice.invoice_date,
                invoice.due_date,
                invoice.counterparty_name,
                invoice.total_amount,
                invoice.currency,
                invoice.tax_amount,
                invoice.net_amount,
                invoice.status,
                invoice.paid_at,
                invoice.send_date,
                invoice.document_language,
                tags_to_json(&invoice.tags),
                invoice.created_at,
                invoice.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn get_invoice_by_id(&self, id: &str) -> SqlResult<Option<Invoice>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, file_path, file_hash, file_modified_at, ingestion_status,
                    ocr_text, extracted_json, confidence_score, invoice_number, invoice_date,
                    due_date, counterparty_name, total_amount, currency, tax_amount, net_amount,
                    status, paid_at, send_date, document_language, tags, created_at, updated_at
             FROM invoices WHERE id = ?1",
        )?;

        stmt.query_row(params![id], |row| {
            Ok(Invoice {
                id: row.get(0)?,
                category: row.get(1)?,
                file_path: row.get(2)?,
                file_hash: row.get(3)?,
                file_modified_at: row.get(4)?,
                ingestion_status: row.get(5)?,
                ocr_text: row.get(6)?,
                extracted_json: row.get(7)?,
                confidence_score: row.get(8)?,
                invoice_number: row.get(9)?,
                invoice_date: row.get(10)?,
                due_date: row.get(11)?,
                counterparty_name: row.get(12)?,
                total_amount: row.get(13)?,
                currency: row.get(14)?,
                tax_amount: row.get(15)?,
                net_amount: row.get(16)?,
                status: row.get(17)?,
                paid_at: row.get(18)?,
                send_date: row.get(19)?,
                document_language: row.get(20)?,
                tags: parse_tags_json(row.get::<_, String>(21)?),
                created_at: row.get(22)?,
                updated_at: row.get(23)?,
            })
        })
        .optional()
    }

    pub fn get_invoice_by_path(&self, path: &str) -> SqlResult<Option<Invoice>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, file_path, file_hash, file_modified_at, ingestion_status,
                    ocr_text, extracted_json, confidence_score, invoice_number, invoice_date,
                    due_date, counterparty_name, total_amount, currency, tax_amount, net_amount,
                    status, paid_at, send_date, document_language, tags, created_at, updated_at
             FROM invoices WHERE file_path = ?1",
        )?;

        stmt.query_row(params![path], |row| {
            Ok(Invoice {
                id: row.get(0)?,
                category: row.get(1)?,
                file_path: row.get(2)?,
                file_hash: row.get(3)?,
                file_modified_at: row.get(4)?,
                ingestion_status: row.get(5)?,
                ocr_text: row.get(6)?,
                extracted_json: row.get(7)?,
                confidence_score: row.get(8)?,
                invoice_number: row.get(9)?,
                invoice_date: row.get(10)?,
                due_date: row.get(11)?,
                counterparty_name: row.get(12)?,
                total_amount: row.get(13)?,
                currency: row.get(14)?,
                tax_amount: row.get(15)?,
                net_amount: row.get(16)?,
                status: row.get(17)?,
                paid_at: row.get(18)?,
                send_date: row.get(19)?,
                document_language: row.get(20)?,
                tags: parse_tags_json(row.get::<_, String>(21)?),
                created_at: row.get(22)?,
                updated_at: row.get(23)?,
            })
        })
        .optional()
    }

    pub fn get_invoice_by_hash(&self, file_hash: &str) -> SqlResult<Option<Invoice>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, file_path, file_hash, file_modified_at, ingestion_status,
                    ocr_text, extracted_json, confidence_score, invoice_number, invoice_date,
                    due_date, counterparty_name, total_amount, currency, tax_amount, net_amount,
                    status, paid_at, send_date, document_language, tags, created_at, updated_at
             FROM invoices
             WHERE file_hash = ?1
             ORDER BY updated_at DESC
             LIMIT 1",
        )?;

        stmt.query_row(params![file_hash], |row| {
            Ok(Invoice {
                id: row.get(0)?,
                category: row.get(1)?,
                file_path: row.get(2)?,
                file_hash: row.get(3)?,
                file_modified_at: row.get(4)?,
                ingestion_status: row.get(5)?,
                ocr_text: row.get(6)?,
                extracted_json: row.get(7)?,
                confidence_score: row.get(8)?,
                invoice_number: row.get(9)?,
                invoice_date: row.get(10)?,
                due_date: row.get(11)?,
                counterparty_name: row.get(12)?,
                total_amount: row.get(13)?,
                currency: row.get(14)?,
                tax_amount: row.get(15)?,
                net_amount: row.get(16)?,
                status: row.get(17)?,
                paid_at: row.get(18)?,
                send_date: row.get(19)?,
                document_language: row.get(20)?,
                tags: parse_tags_json(row.get::<_, String>(21)?),
                created_at: row.get(22)?,
                updated_at: row.get(23)?,
            })
        })
        .optional()
    }

    pub fn get_invoice_summaries(&self, category: &str, year: Option<&str>) -> SqlResult<Vec<InvoiceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT invoices.id, invoice_date, counterparty_name, total_amount, status, confidence_score, file_path
             FROM invoices
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             WHERE (?1 = 'all' OR invoices.category = ?1)
               AND (
                 ?2 IS NULL
                 OR substr(COALESCE(date_override.override_value, invoices.invoice_date), 1, 4) = ?2
                 OR COALESCE(date_override.override_value, invoices.invoice_date) IS NULL
                 OR COALESCE(date_override.override_value, invoices.invoice_date) = ''
               )
             ORDER BY COALESCE(date_override.override_value, invoices.invoice_date) DESC",
        )?;

        let rows = stmt.query_map(params![category, year], |row| {
            Ok(InvoiceSummary {
                id: row.get(0)?,
                invoice_date: row.get(1)?,
                counterparty_name: row.get(2)?,
                total_amount: row.get(3)?,
                status: row.get(4)?,
                confidence_score: row.get(5)?,
                file_path: row.get(6)?,
            })
        })?;

        rows.collect()
    }

    pub fn get_invoice_years(&self, category: &str) -> SqlResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT substr(COALESCE(date_override.override_value, invoices.invoice_date), 1, 4) AS year
             FROM invoices
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             WHERE (?1 = 'all' OR invoices.category = ?1)
               AND COALESCE(date_override.override_value, invoices.invoice_date) IS NOT NULL
               AND substr(COALESCE(date_override.override_value, invoices.invoice_date), 1, 4) != ''
             ORDER BY year DESC",
        )?;

        let rows = stmt.query_map(params![category], |row| row.get(0))?;
        rows.collect()
    }

    pub fn get_revenue_customer_source_invoices(&self) -> SqlResult<Vec<CustomerSourceInvoice>> {
        let mut stmt = self.conn.prepare(
            "SELECT invoices.id,
                    TRIM(COALESCE(NULLIF(counterparty.override_value, ''), invoices.counterparty_name)) AS counterparty_name,
                    COALESCE(NULLIF(date_override.override_value, ''), invoices.invoice_date) AS invoice_date,
                    COALESCE(NULLIF(amt.override_value, ''), invoices.total_amount) AS total_amount,
                    invoices.created_at
             FROM invoices
             LEFT JOIN invoice_overrides amt
               ON amt.invoice_id = invoices.id AND amt.field_name = 'total_amount'
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             LEFT JOIN invoice_overrides counterparty
               ON counterparty.invoice_id = invoices.id AND counterparty.field_name = 'counterparty_name'
             WHERE invoices.category = 'revenue'
               AND TRIM(COALESCE(NULLIF(counterparty.override_value, ''), invoices.counterparty_name, '')) != ''
             ORDER BY COALESCE(NULLIF(date_override.override_value, ''), invoices.invoice_date) DESC,
                      invoices.created_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(CustomerSourceInvoice {
                id: row.get(0)?,
                counterparty_name: row.get(1)?,
                invoice_date: row.get(2)?,
                total_amount: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        rows.collect()
    }

    pub fn save_customer(&self, customer: &Customer) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO customers (
                id, name, source_name, normalized_name, billing_address, general_email,
                latest_invoice_id, first_invoice_date, last_invoice_date, invoice_count,
                lifetime_value, avg_monthly_revenue, active_months, enrichment_status,
                enriched_at, enrichment_source_invoice_id, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                source_name = excluded.source_name,
                normalized_name = excluded.normalized_name,
                billing_address = excluded.billing_address,
                general_email = excluded.general_email,
                latest_invoice_id = excluded.latest_invoice_id,
                first_invoice_date = excluded.first_invoice_date,
                last_invoice_date = excluded.last_invoice_date,
                invoice_count = excluded.invoice_count,
                lifetime_value = excluded.lifetime_value,
                avg_monthly_revenue = excluded.avg_monthly_revenue,
                active_months = excluded.active_months,
                enrichment_status = excluded.enrichment_status,
                enriched_at = excluded.enriched_at,
                enrichment_source_invoice_id = excluded.enrichment_source_invoice_id,
                updated_at = excluded.updated_at",
            params![
                customer.id,
                customer.name,
                customer.source_name,
                customer.normalized_name,
                customer.billing_address,
                customer.general_email,
                customer.latest_invoice_id,
                customer.first_invoice_date,
                customer.last_invoice_date,
                customer.invoice_count,
                customer.lifetime_value,
                customer.avg_monthly_revenue,
                customer.active_months,
                customer.enrichment_status,
                customer.enriched_at,
                customer.enrichment_source_invoice_id,
                customer.created_at,
                customer.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn get_customer_by_id(&self, customer_id: &str) -> SqlResult<Option<Customer>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, source_name, normalized_name, billing_address, general_email,
                    latest_invoice_id, first_invoice_date, last_invoice_date, invoice_count,
                    lifetime_value, avg_monthly_revenue, active_months, enrichment_status,
                    enriched_at, enrichment_source_invoice_id, created_at, updated_at
             FROM customers
             WHERE id = ?1",
        )?;

        stmt.query_row(params![customer_id], parse_customer_row).optional()
    }

    pub fn get_customer_by_normalized_name(&self, normalized_name: &str) -> SqlResult<Option<Customer>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, source_name, normalized_name, billing_address, general_email,
                    latest_invoice_id, first_invoice_date, last_invoice_date, invoice_count,
                    lifetime_value, avg_monthly_revenue, active_months, enrichment_status,
                    enriched_at, enrichment_source_invoice_id, created_at, updated_at
             FROM customers
             WHERE normalized_name = ?1",
        )?;

        stmt.query_row(params![normalized_name], parse_customer_row).optional()
    }

    pub fn get_customer_summaries(&self) -> SqlResult<Vec<CustomerSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT customers.id,
                    customers.name,
                    customers.normalized_name,
                    customers.general_email,
                    customers.first_invoice_date,
                    customers.last_invoice_date,
                    customers.invoice_count,
                    COUNT(customer_contacts.id) AS contact_count,
                    customers.lifetime_value,
                    customers.avg_monthly_revenue,
                    customers.active_months
             FROM customers
             LEFT JOIN customer_contacts
               ON customer_contacts.customer_id = customers.id
             GROUP BY customers.id
             ORDER BY customers.lifetime_value DESC,
                      customers.last_invoice_date DESC,
                      customers.name ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(CustomerSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                normalized_name: row.get(2)?,
                general_email: row.get(3)?,
                first_invoice_date: row.get(4)?,
                last_invoice_date: row.get(5)?,
                invoice_count: row.get(6)?,
                contact_count: row.get(7)?,
                lifetime_value: row.get(8)?,
                avg_monthly_revenue: row.get(9)?,
                active_months: row.get(10)?,
            })
        })?;

        rows.collect()
    }

    pub fn update_customer_field(&self, customer_id: &str, field_name: &str, value: &str) -> SqlResult<()> {
        let updated_at = now_rfc3339();
        match field_name {
            "name" => {
                self.conn.execute(
                    "UPDATE customers SET name = ?2, updated_at = ?3 WHERE id = ?1",
                    params![customer_id, value, updated_at],
                )?;
            }
            "billing_address" => {
                self.conn.execute(
                    "UPDATE customers SET billing_address = ?2, updated_at = ?3 WHERE id = ?1",
                    params![customer_id, empty_to_none(value), updated_at],
                )?;
            }
            "general_email" => {
                self.conn.execute(
                    "UPDATE customers SET general_email = ?2, updated_at = ?3 WHERE id = ?1",
                    params![customer_id, empty_to_none(value), updated_at],
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn get_customer_contacts(&self, customer_id: &str) -> SqlResult<Vec<CustomerContact>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, customer_id, name, email, created_at, updated_at
             FROM customer_contacts
             WHERE customer_id = ?1
             ORDER BY
               CASE WHEN TRIM(name) = '' THEN 1 ELSE 0 END,
               LOWER(name) ASC,
               created_at ASC",
        )?;

        let rows = stmt.query_map(params![customer_id], parse_customer_contact_row)?;
        rows.collect()
    }

    pub fn add_customer_contact(&self, customer_id: &str) -> SqlResult<CustomerContact> {
        let contact = CustomerContact {
            id: Uuid::new_v4().to_string(),
            customer_id: customer_id.to_string(),
            name: String::new(),
            email: String::new(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        };
        self.save_customer_contact(&contact)?;
        Ok(contact)
    }

    pub fn save_customer_contact(&self, contact: &CustomerContact) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO customer_contacts (id, customer_id, name, email, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                customer_id = excluded.customer_id,
                name = excluded.name,
                email = excluded.email,
                updated_at = excluded.updated_at",
            params![
                contact.id,
                contact.customer_id,
                contact.name,
                contact.email,
                contact.created_at,
                contact.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn update_customer_contact_field(&self, contact_id: &str, field_name: &str, value: &str) -> SqlResult<()> {
        let updated_at = now_rfc3339();
        match field_name {
            "name" => {
                self.conn.execute(
                    "UPDATE customer_contacts SET name = ?2, updated_at = ?3 WHERE id = ?1",
                    params![contact_id, value, updated_at],
                )?;
            }
            "email" => {
                self.conn.execute(
                    "UPDATE customer_contacts SET email = ?2, updated_at = ?3 WHERE id = ?1",
                    params![contact_id, value, updated_at],
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn delete_customer_contact(&self, contact_id: &str) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM customer_contacts WHERE id = ?1", params![contact_id])?;
        Ok(())
    }

    pub fn delete_customer_by_normalized_name(&self, normalized_name: &str) -> SqlResult<()> {
        self.conn.execute(
            "DELETE FROM customers WHERE normalized_name = ?1",
            params![normalized_name],
        )?;
        Ok(())
    }

    pub fn get_email_templates(&self) -> SqlResult<Vec<EmailTemplate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, subject, body, created_at, updated_at
             FROM email_templates
             ORDER BY LOWER(name) ASC, created_at ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(EmailTemplate {
                id: row.get(0)?,
                name: row.get(1)?,
                subject: row.get(2)?,
                body: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;

        rows.collect()
    }

    pub fn upsert_email_template(&self, template: &EmailTemplate) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO email_templates (id, name, subject, body, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                subject = excluded.subject,
                body = excluded.body,
                updated_at = excluded.updated_at",
            params![
                template.id,
                template.name,
                template.subject,
                template.body,
                template.created_at,
                template.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_email_template(&self, template_id: &str) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM email_templates WHERE id = ?1", params![template_id])?;
        Ok(())
    }

    pub fn get_revenue_email_invoice_options(&self) -> SqlResult<Vec<EmailInvoiceOption>> {
        let mut stmt = self.conn.prepare(
            "SELECT invoices.id,
                    COALESCE(NULLIF(counterparty.override_value, ''), invoices.counterparty_name) AS customer_name,
                    COALESCE(NULLIF(number_override.override_value, ''), invoices.invoice_number) AS invoice_number,
                    COALESCE(NULLIF(date_override.override_value, ''), invoices.invoice_date) AS invoice_date,
                    COALESCE(NULLIF(amount_override.override_value, ''), invoices.total_amount) AS total_amount,
                    invoices.currency,
                    COALESCE(NULLIF(status_override.override_value, ''), invoices.status) AS status,
                    invoices.file_path
             FROM invoices
             LEFT JOIN invoice_overrides counterparty
               ON counterparty.invoice_id = invoices.id AND counterparty.field_name = 'counterparty_name'
             LEFT JOIN invoice_overrides number_override
               ON number_override.invoice_id = invoices.id AND number_override.field_name = 'invoice_number'
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             LEFT JOIN invoice_overrides amount_override
               ON amount_override.invoice_id = invoices.id AND amount_override.field_name = 'total_amount'
             LEFT JOIN invoice_overrides status_override
               ON status_override.invoice_id = invoices.id AND status_override.field_name = 'status'
             WHERE invoices.category = 'revenue'
             ORDER BY COALESCE(NULLIF(date_override.override_value, ''), invoices.invoice_date) DESC,
                      invoices.created_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let file_path: Option<String> = row.get(7)?;
            let file_name = file_path.as_deref().and_then(|path| {
                std::path::Path::new(path)
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
            });

            Ok(EmailInvoiceOption {
                id: row.get(0)?,
                customer_id: None,
                customer_name: row.get(1)?,
                invoice_number: row.get(2)?,
                invoice_date: row.get(3)?,
                total_amount: row.get(4)?,
                currency: row.get(5)?,
                status: row.get(6)?,
                file_name,
                file_path,
            })
        })?;

        rows.collect()
    }

    pub fn set_invoice_send_date_by_path(&self, file_path: &str, send_date: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE invoices
             SET send_date = ?1, updated_at = ?2
             WHERE file_path = ?3",
            params![send_date, now_rfc3339(), file_path],
        )?;
        Ok(())
    }

    pub fn mark_invoice_missing(&self, file_path: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE invoices SET ingestion_status = 'missing', updated_at = datetime('now') WHERE file_path = ?1",
            params![file_path],
        )?;
        Ok(())
    }

    pub fn set_override(&self, invoice_id: &str, field_name: &str, value: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO invoice_overrides (id, invoice_id, field_name, override_value, created_at, updated_at)
             VALUES (
                COALESCE((SELECT id FROM invoice_overrides WHERE invoice_id = ?1 AND field_name = ?2), hex(randomblob(16))),
                ?1, ?2, ?3, datetime('now'), datetime('now')
             )",
            params![invoice_id, field_name, value],
        )?;
        Ok(())
    }

    pub fn get_overrides(&self, invoice_id: &str) -> SqlResult<Vec<InvoiceOverride>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, invoice_id, field_name, override_value, created_at, updated_at
             FROM invoice_overrides WHERE invoice_id = ?1",
        )?;

        let rows = stmt.query_map(params![invoice_id], |row| {
            Ok(InvoiceOverride {
                id: row.get(0)?,
                invoice_id: row.get(1)?,
                field_name: row.get(2)?,
                override_value: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;

        rows.collect()
    }

    pub fn clear_override(&self, invoice_id: &str, field_name: &str) -> SqlResult<()> {
        self.conn.execute(
            "DELETE FROM invoice_overrides WHERE invoice_id = ?1 AND field_name = ?2",
            params![invoice_id, field_name],
        )?;
        Ok(())
    }

    pub fn clear_all_overrides(&self, invoice_id: &str) -> SqlResult<()> {
        self.conn.execute(
            "DELETE FROM invoice_overrides WHERE invoice_id = ?1",
            params![invoice_id],
        )?;
        Ok(())
    }

    pub fn set_invoice_tags(&self, invoice_id: &str, tags: &[String]) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE invoices
             SET tags = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![invoice_id, tags_to_json(tags)],
        )?;
        Ok(())
    }

    pub fn delete_invoice(&mut self, invoice_id: &str) -> SqlResult<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM processing_logs WHERE invoice_id = ?1",
            params![invoice_id],
        )?;
        tx.execute(
            "DELETE FROM invoice_overrides WHERE invoice_id = ?1",
            params![invoice_id],
        )?;
        tx.execute("DELETE FROM invoices WHERE id = ?1", params![invoice_id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, datetime('now'))",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> SqlResult<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        stmt.query_row(params![key], |row| row.get(0)).optional()
    }

    pub fn delete_setting(&self, key: &str) -> SqlResult<()> {
        self.conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn get_monthly_sum(&self, category: &str, year_month: &str) -> SqlResult<f64> {
        let mut stmt = self.conn.prepare(
            "SELECT SUM(CAST(COALESCE(amt.override_value, invoices.total_amount) AS REAL))
             FROM invoices
             LEFT JOIN invoice_overrides amt
               ON amt.invoice_id = invoices.id AND amt.field_name = 'total_amount'
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             WHERE invoices.category = ?1
               AND substr(COALESCE(date_override.override_value, invoices.invoice_date), 1, 7) = ?2",
        )?;

        let total: Option<f64> = stmt.query_row(params![category, year_month], |row| row.get(0))?;
        Ok(total.unwrap_or(0.0))
    }

    pub fn get_yearly_sum(&self, category: &str, year: &str) -> SqlResult<f64> {
        let mut stmt = self.conn.prepare(
            "SELECT SUM(CAST(COALESCE(amt.override_value, invoices.total_amount) AS REAL))
             FROM invoices
             LEFT JOIN invoice_overrides amt
               ON amt.invoice_id = invoices.id AND amt.field_name = 'total_amount'
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             WHERE invoices.category = ?1
               AND substr(COALESCE(date_override.override_value, invoices.invoice_date), 1, 4) = ?2",
        )?;

        let total: Option<f64> = stmt.query_row(params![category, year], |row| row.get(0))?;
        Ok(total.unwrap_or(0.0))
    }

    pub fn get_overdue_total(&self, category: &str, today: &str) -> SqlResult<f64> {
        let mut stmt = self.conn.prepare(
            "SELECT SUM(CAST(COALESCE(NULLIF(amt.override_value, ''), invoices.total_amount) AS REAL))
             FROM invoices
             LEFT JOIN invoice_overrides amt
               ON amt.invoice_id = invoices.id AND amt.field_name = 'total_amount'
             LEFT JOIN invoice_overrides status
               ON status.invoice_id = invoices.id AND status.field_name = 'status'
             LEFT JOIN invoice_overrides due_override
               ON due_override.invoice_id = invoices.id AND due_override.field_name = 'due_date'
             WHERE invoices.category = ?1
               AND COALESCE(NULLIF(status.override_value, ''), invoices.status) = 'open'
               AND COALESCE(NULLIF(due_override.override_value, ''), invoices.due_date) IS NOT NULL
               AND COALESCE(NULLIF(due_override.override_value, ''), invoices.due_date) != ''
               AND COALESCE(NULLIF(due_override.override_value, ''), invoices.due_date) < ?2",
        )?;

        let total: Option<f64> = stmt.query_row(params![category, today], |row| row.get(0))?;
        Ok(total.unwrap_or(0.0))
    }

    pub fn get_invoice_summaries_by_status(&self, category: &str, status_value: &str) -> SqlResult<Vec<InvoiceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT invoices.id,
                    COALESCE(NULLIF(date_override.override_value, ''), invoices.invoice_date) AS invoice_date,
                    COALESCE(NULLIF(counterparty.override_value, ''), invoices.counterparty_name) AS counterparty_name,
                    COALESCE(NULLIF(amt.override_value, ''), invoices.total_amount) AS total_amount,
                    COALESCE(NULLIF(status.override_value, ''), invoices.status) AS status,
                    invoices.confidence_score,
                    invoices.file_path
             FROM invoices
             LEFT JOIN invoice_overrides status
               ON status.invoice_id = invoices.id AND status.field_name = 'status'
             LEFT JOIN invoice_overrides amt
               ON amt.invoice_id = invoices.id AND amt.field_name = 'total_amount'
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             LEFT JOIN invoice_overrides counterparty
               ON counterparty.invoice_id = invoices.id AND counterparty.field_name = 'counterparty_name'
             WHERE invoices.category = ?1
               AND COALESCE(NULLIF(status.override_value, ''), invoices.status) = ?2
             ORDER BY COALESCE(NULLIF(date_override.override_value, ''), invoices.invoice_date) DESC,
                      invoices.created_at DESC",
        )?;

        let rows = stmt.query_map(params![category, status_value], |row| {
            Ok(InvoiceSummary {
                id: row.get(0)?,
                invoice_date: row.get(1)?,
                counterparty_name: row.get(2)?,
                total_amount: row.get(3)?,
                status: row.get(4)?,
                confidence_score: row.get(5)?,
                file_path: row.get(6)?,
            })
        })?;

        rows.collect()
    }

    pub fn get_top_counterparties_by_year(
        &self,
        category: &str,
        year: &str,
        limit: usize,
    ) -> SqlResult<Vec<DashboardListItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(NULLIF(TRIM(COALESCE(NULLIF(counterparty.override_value, ''), invoices.counterparty_name)), ''), 'Unknown') AS label,
                    SUM(CAST(COALESCE(NULLIF(amt.override_value, ''), invoices.total_amount) AS REAL)) AS total_amount
             FROM invoices
             LEFT JOIN invoice_overrides amt
               ON amt.invoice_id = invoices.id AND amt.field_name = 'total_amount'
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             LEFT JOIN invoice_overrides counterparty
               ON counterparty.invoice_id = invoices.id AND counterparty.field_name = 'counterparty_name'
             WHERE invoices.category = ?1
               AND substr(COALESCE(NULLIF(date_override.override_value, ''), invoices.invoice_date), 1, 4) = ?2
             GROUP BY label
             ORDER BY total_amount DESC, label ASC
             LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![category, year, limit as i64], |row| {
            Ok(DashboardListItem {
                label: row.get(0)?,
                total_amount: row.get(1)?,
            })
        })?;

        rows.collect()
    }

    pub fn log_processing(
        &self,
        invoice_id: Option<&str>,
        file_hash: Option<&str>,
        process_type: &str,
        status: &str,
        message: Option<&str>,
    ) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO processing_logs (id, invoice_id, file_hash, process_type, status, message, created_at)
             VALUES (hex(randomblob(16)), ?1, ?2, ?3, ?4, ?5, datetime('now'))",
            params![invoice_id, file_hash, process_type, status, message],
        )?;
        Ok(())
    }
}

fn parse_tags_json(raw: String) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default()
}

fn tags_to_json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string())
}

fn parse_customer_row(row: &rusqlite::Row<'_>) -> SqlResult<Customer> {
    Ok(Customer {
        id: row.get(0)?,
        name: row.get(1)?,
        source_name: row.get(2)?,
        normalized_name: row.get(3)?,
        billing_address: row.get(4)?,
        general_email: row.get(5)?,
        latest_invoice_id: row.get(6)?,
        first_invoice_date: row.get(7)?,
        last_invoice_date: row.get(8)?,
        invoice_count: row.get(9)?,
        lifetime_value: row.get(10)?,
        avg_monthly_revenue: row.get(11)?,
        active_months: row.get(12)?,
        enrichment_status: row.get(13)?,
        enriched_at: row.get(14)?,
        enrichment_source_invoice_id: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

fn parse_customer_contact_row(row: &rusqlite::Row<'_>) -> SqlResult<CustomerContact> {
    Ok(CustomerContact {
        id: row.get(0)?,
        customer_id: row.get(1)?,
        name: row.get(2)?,
        email: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn empty_to_none(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
