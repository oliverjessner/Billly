use anyhow::{anyhow, Result};
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::{normalize_payable_tags, ExtractedCustomerData, ExtractedInvoiceData, PAYABLE_TAG_OPTIONS};

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    temperature: f32,
    messages: Vec<Message>,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

pub struct OpenAIExtractor;

impl OpenAIExtractor {
    pub async fn extract_invoice_data(api_key: &str, text: &str, category: &str) -> Result<(ExtractedInvoiceData, String)> {
        let schema = extraction_schema();
        let prompt = system_prompt(category);
        let user = format!("Invoice text:\n{}", text);

        let mut raw = call_openai(api_key, &prompt, &user).await?;
        let mut value = parse_json(&raw)?;

        if !validate_json(&schema, &value) {
            let fix_prompt = format!(
                "Fixe dieses JSON so, dass es exakt dem Schema entspricht. Nur JSON ausgeben. JSON:\n{}",
                raw
            );
            raw = call_openai(api_key, &prompt, &fix_prompt).await?;
            value = parse_json(&raw)?;
            if !validate_json(&schema, &value) {
                return Err(anyhow!("JSON validation failed"));
            }
        }

        let mut data: ExtractedInvoiceData = serde_json::from_value(value)?;
        if data.currency.is_none() {
            data.currency = Some("EUR".to_string());
        }
        data.tags = Some(match data.tags.take() {
            Some(tags) if category == "payable" => normalize_payable_tags(tags),
            _ => Vec::new(),
        });
        if data.extraction_notes.trim().is_empty() {
            data.extraction_notes = "notes missing".to_string();
        }

        if data.confidence_score.is_none() {
            data.confidence_score = Some(compute_confidence(&data));
        }

        Ok((data, raw))
    }

    pub async fn extract_customer_data(api_key: &str, text: &str) -> Result<ExtractedCustomerData> {
        let schema = customer_schema();
        let prompt = customer_prompt();
        let user = format!("Invoice plain text:\n{}", text);

        let mut raw = call_openai(api_key, &prompt, &user).await?;
        let mut value = parse_json(&raw)?;

        if !validate_json(&schema, &value) {
            let fix_prompt = format!(
                "Fix this JSON so it matches the schema exactly. Return JSON only. JSON:\n{}",
                raw
            );
            raw = call_openai(api_key, &prompt, &fix_prompt).await?;
            value = parse_json(&raw)?;
            if !validate_json(&schema, &value) {
                return Err(anyhow!("Customer JSON validation failed"));
            }
        }

        serde_json::from_value(value).map_err(Into::into)
    }
}

async fn call_openai(api_key: &str, system_prompt: &str, user_prompt: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let request = ChatRequest {
        model: "gpt-4o-mini".to_string(),
        temperature: 0.1,
        messages: vec![
            Message {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            },
        ],
        response_format: ResponseFormat {
            format_type: "json_object".to_string(),
        },
    };

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("OpenAI error {}: {}", status, body));
    }

    let body: ChatResponse = response.json().await?;
    let content = body
        .choices
        .get(0)
        .ok_or_else(|| anyhow!("Empty response"))?
        .message
        .content
        .trim()
        .to_string();
    Ok(content)
}

fn parse_json(raw: &str) -> Result<Value> {
    serde_json::from_str::<Value>(raw).map_err(|e| anyhow!("Invalid JSON: {}", e))
}

fn extraction_schema() -> JSONSchema {
    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["total_amount", "currency", "invoice_date", "tags", "extraction_notes"],
        "properties": {
            "invoice_number": {"type": ["string", "null"]},
            "invoice_date": {"type": ["string", "null"]},
            "due_date": {"type": ["string", "null"]},
            "counterparty_name": {"type": ["string", "null"]},
            "total_amount": {"type": ["number", "null"]},
            "currency": {"type": ["string", "null"]},
            "tax_amount": {"type": ["number", "null"]},
            "net_amount": {"type": ["number", "null"]},
            "tags": {
                "type": ["array", "null"],
                "items": {"type": "string", "enum": PAYABLE_TAG_OPTIONS}
            },
            "extraction_notes": {"type": "string"},
            "confidence_score": {"type": ["number", "null"]}
        }
    });

    JSONSchema::compile(&schema).expect("Invalid JSON schema")
}

fn customer_schema() -> JSONSchema {
    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["billing_address", "general_email"],
        "properties": {
            "billing_address": {"type": ["string", "null"]},
            "general_email": {"type": ["string", "null"]}
        }
    });

    JSONSchema::compile(&schema).expect("Invalid customer JSON schema")
}

fn validate_json(schema: &JSONSchema, value: &Value) -> bool {
    schema.is_valid(value)
}

fn compute_confidence(data: &ExtractedInvoiceData) -> f64 {
    let mut score: f64 = 0.4;
    if data.invoice_number.is_some() {
        score += 0.1;
    }
    if data.invoice_date.is_some() {
        score += 0.1;
    }
    if data.counterparty_name.is_some() {
        score += 0.1;
    }
    if data.total_amount.is_some() {
        score += 0.1;
    }
    if data.tax_amount.is_some() || data.net_amount.is_some() {
        score += 0.05;
    }
    score.clamp(0.0, 1.0)
}

fn system_prompt(category: &str) -> String {
    let payable_hint = if category == "payable" {
        format!(
            "For tags, choose zero to three labels only from this list: {}. Use [] if nothing fits clearly.",
            PAYABLE_TAG_OPTIONS.join(", ")
        )
    } else {
        "For tags, always return an empty array because tagging only applies to payables.".to_string()
    };

    format!(
        r#"You are an invoice extraction system. Return JSON only and match the schema exactly.
Fields:
- invoice_number (string|null)
- invoice_date (YYYY-MM-DD|null)
- due_date (YYYY-MM-DD|null)
- counterparty_name (string|null)
- total_amount (number|null)
- currency (string|null)
- tax_amount (number|null)
- net_amount (number|null)
- tags (array of strings|null)
- extraction_notes (string, short)
- confidence_score (number|null)
{}
"#,
        payable_hint
    )
}

fn customer_prompt() -> String {
    r#"You extract customer master data from an outgoing revenue invoice.
Return JSON only and match the schema exactly.

Rules:
- Identify the billed customer, not the invoice sender.
- billing_address should contain the customer's postal address as one concise string.
- general_email should be the best customer contact email explicitly shown in the invoice text.
- Use null when a value is not clearly present.

Fields:
- billing_address (string|null)
- general_email (string|null)
"#
    .to_string()
}
