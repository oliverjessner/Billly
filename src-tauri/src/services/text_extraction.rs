use anyhow::{anyhow, Result};
use std::path::Path;
use whatlang::{detect, Lang};

pub struct TextExtractor;

pub struct ExtractedText {
    pub text: String,
    pub document_language: Option<String>,
}

impl TextExtractor {
    pub fn extract_from_pdf(path: &Path) -> Result<ExtractedText> {
        if let Ok(text) = pdf_extract::extract_text(path) {
            if !text.trim().is_empty() {
                return Ok(ExtractedText {
                    document_language: detect_language(&text),
                    text,
                });
            }
        }
        Self::extract_via_ocr(path)
    }

    fn extract_via_ocr(path: &Path) -> Result<ExtractedText> {
        let path_str = path.to_str().ok_or_else(|| anyhow!("Invalid path"))?;

        let mut last_error = None;
        for language in ["deu+eng", "deu", "eng"] {
            let text_result = (|| -> Result<String> {
                tesseract::Tesseract::new(None, Some(language))
                    .map_err(|e| anyhow!("Tesseract init: {}", e))?
                    .set_image(path_str)
                    .map_err(|e| anyhow!("Tesseract image: {}", e))?
                    .recognize()
                    .map_err(|e| anyhow!("Tesseract recognize: {}", e))?
                    .get_text()
                    .map_err(|e| anyhow!("OCR text: {}", e))
            })();

            match text_result {
                Ok(text) if !text.trim().is_empty() => {
                    let document_language = detect_language(&text).or_else(|| match language {
                        "deu" => Some("deu".to_string()),
                        "eng" => Some("eng".to_string()),
                        _ => None,
                    });
                    return Ok(ExtractedText { text, document_language });
                }
                Ok(_) => {
                    last_error = Some(anyhow!("OCR text empty"));
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("OCR failed")))
    }

    pub fn validate_text_quality(text: &str) -> bool {
        text.len() > 50 && text.split_whitespace().count() > 10
    }
}

fn detect_language(text: &str) -> Option<String> {
    let info = detect(text)?;
    match info.lang() {
        Lang::Deu => Some("deu".to_string()),
        Lang::Eng => Some("eng".to_string()),
        _ => None,
    }
}
