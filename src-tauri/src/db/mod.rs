use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use std::path::PathBuf;

use crate::models::{Invoice, InvoiceOverride, InvoiceSummary};

pub struct Database {
    conn: Connection,
}

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

        let migrations = vec![
            (
                "001_create_invoices.sql",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../migrations/001_create_invoices.sql"
                )),
            ),
            (
                "002_create_overrides_and_settings.sql",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../migrations/002_create_overrides_and_settings.sql"
                )),
            ),
            (
                "003_create_processing_logs_table.sql",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../migrations/003_create_processing_logs_table.sql"
                )),
            ),
            (
                "004_normalize_payables.sql",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../migrations/004_normalize_payables.sql"
                )),
            ),
        ];

        for (name, sql) in migrations {
            let applied: Option<String> = self
                .conn
                .query_row(
                    "SELECT name FROM schema_migrations WHERE name = ?1",
                    params![name],
                    |row| row.get(0),
                )
                .optional()?;

            if applied.is_none() {
                let tx = self.conn.transaction()?;
                tx.execute_batch(sql)?;
                tx.execute(
                    "INSERT INTO schema_migrations (name, applied_at) VALUES (?1, datetime('now'))",
                    params![name],
                )?;
                tx.commit()?;
            }
        }

        Ok(())
    }

    pub fn upsert_invoice(&self, invoice: &Invoice) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO invoices (
                id, category, file_path, file_hash, file_modified_at, ingestion_status,
                ocr_text, extracted_json, confidence_score, invoice_number, invoice_date,
                due_date, counterparty_name, total_amount, currency, tax_amount, net_amount,
                status, paid_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
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
                    status, paid_at, created_at, updated_at
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
                created_at: row.get(19)?,
                updated_at: row.get(20)?,
            })
        })
        .optional()
    }

    pub fn get_invoice_by_path(&self, path: &str) -> SqlResult<Option<Invoice>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, file_path, file_hash, file_modified_at, ingestion_status,
                    ocr_text, extracted_json, confidence_score, invoice_number, invoice_date,
                    due_date, counterparty_name, total_amount, currency, tax_amount, net_amount,
                    status, paid_at, created_at, updated_at
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
                created_at: row.get(19)?,
                updated_at: row.get(20)?,
            })
        })
        .optional()
    }

    pub fn get_invoice_by_hash(&self, file_hash: &str) -> SqlResult<Option<Invoice>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, file_path, file_hash, file_modified_at, ingestion_status,
                    ocr_text, extracted_json, confidence_score, invoice_number, invoice_date,
                    due_date, counterparty_name, total_amount, currency, tax_amount, net_amount,
                    status, paid_at, created_at, updated_at
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
                created_at: row.get(19)?,
                updated_at: row.get(20)?,
            })
        })
        .optional()
    }

    pub fn get_invoices(&self, category: &str) -> SqlResult<Vec<Invoice>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, file_path, file_hash, file_modified_at, ingestion_status,
                    ocr_text, extracted_json, confidence_score, invoice_number, invoice_date,
                    due_date, counterparty_name, total_amount, currency, tax_amount, net_amount,
                    status, paid_at, created_at, updated_at
             FROM invoices
             WHERE category = ?1
             ORDER BY invoice_date DESC",
        )?;

        let rows = stmt.query_map(params![category], |row| {
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
                created_at: row.get(19)?,
                updated_at: row.get(20)?,
            })
        })?;

        rows.collect()
    }

    pub fn get_invoice_summaries(&self, category: &str, year: Option<&str>) -> SqlResult<Vec<InvoiceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT invoices.id, invoice_date, counterparty_name, total_amount, status, confidence_score, file_path
             FROM invoices
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             WHERE invoices.category = ?1
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
             WHERE invoices.category = ?1
               AND COALESCE(date_override.override_value, invoices.invoice_date) IS NOT NULL
               AND substr(COALESCE(date_override.override_value, invoices.invoice_date), 1, 4) != ''
             ORDER BY year DESC",
        )?;

        let rows = stmt.query_map(params![category], |row| row.get(0))?;
        rows.collect()
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

    pub fn get_paid_payables_month_total(&self, year_month: &str) -> SqlResult<f64> {
        let mut stmt = self.conn.prepare(
            "SELECT SUM(CAST(COALESCE(amt.override_value, invoices.total_amount) AS REAL))
             FROM invoices
             LEFT JOIN invoice_overrides status
               ON status.invoice_id = invoices.id AND status.field_name = 'status'
             LEFT JOIN invoice_overrides amt
               ON amt.invoice_id = invoices.id AND amt.field_name = 'total_amount'
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             WHERE invoices.category = 'payable'
               AND COALESCE(status.override_value, invoices.status) = 'paid'
               AND substr(COALESCE(date_override.override_value, invoices.invoice_date), 1, 7) = ?1",
        )?;

        let total: Option<f64> = stmt.query_row(params![year_month], |row| row.get(0))?;
        Ok(total.unwrap_or(0.0))
    }

    pub fn get_open_payables_month_total(&self, year_month: &str) -> SqlResult<f64> {
        let mut stmt = self.conn.prepare(
            "SELECT SUM(CAST(COALESCE(amt.override_value, invoices.total_amount) AS REAL))
             FROM invoices
             LEFT JOIN invoice_overrides status
               ON status.invoice_id = invoices.id AND status.field_name = 'status'
             LEFT JOIN invoice_overrides amt
               ON amt.invoice_id = invoices.id AND amt.field_name = 'total_amount'
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             WHERE invoices.category = 'payable'
               AND COALESCE(status.override_value, invoices.status) = 'open'
               AND substr(COALESCE(date_override.override_value, invoices.invoice_date), 1, 7) = ?1",
        )?;

        let total: Option<f64> = stmt.query_row(params![year_month], |row| row.get(0))?;
        Ok(total.unwrap_or(0.0))
    }

    pub fn get_status_total_for_month(&self, category: &str, status_value: &str, year_month: &str) -> SqlResult<f64> {
        let mut stmt = self.conn.prepare(
            "SELECT SUM(CAST(COALESCE(amt.override_value, invoices.total_amount) AS REAL))
             FROM invoices
             LEFT JOIN invoice_overrides status
               ON status.invoice_id = invoices.id AND status.field_name = 'status'
             LEFT JOIN invoice_overrides amt
               ON amt.invoice_id = invoices.id AND amt.field_name = 'total_amount'
             LEFT JOIN invoice_overrides date_override
               ON date_override.invoice_id = invoices.id AND date_override.field_name = 'invoice_date'
             WHERE invoices.category = ?1
               AND COALESCE(status.override_value, invoices.status) = ?2
               AND substr(COALESCE(date_override.override_value, invoices.invoice_date), 1, 7) = ?3",
        )?;

        let total: Option<f64> = stmt.query_row(params![category, status_value, year_month], |row| row.get(0))?;
        Ok(total.unwrap_or(0.0))
    }

    pub fn get_recent_invoices(&self, category: &str, limit: usize) -> SqlResult<Vec<InvoiceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, invoice_date, counterparty_name, total_amount, status, confidence_score, file_path
             FROM invoices
             WHERE category = ?1
             ORDER BY invoice_date DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![category, limit as i32], |row| {
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
