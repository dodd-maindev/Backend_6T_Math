use reqwest::{Client, StatusCode};
use serde_json::Value;
use crate::utils::{api_key_pool::ApiKeyPool, gemini_payload::{build_grading_payload, build_transcription_payload}, model_registry::ModelRegistry};

/// Client for communicating with Google Gemini API with smart multi-key load balancing and 20-model fallback.
#[derive(Clone, Debug)]
pub struct GeminiClient {
    pool: ApiKeyPool,
    preferred_model: String,
}

impl GeminiClient {
    pub fn from_env() -> Result<Self, String> {
        let pool = ApiKeyPool::from_env();
        let preferred_model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-3.1-flash-lite".to_string());
        Ok(Self { pool, preferred_model })
    }

    /// Evaluates submission and parses structured JSON grading result.
    pub async fn evaluate_submission(&self, sys: &str, parts: Vec<Value>) -> Result<Value, String> {
        let payload = build_grading_payload(sys, parts);
        let raw_text = self.execute_request(&payload).await?;
        let clean_json = raw_text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
        serde_json::from_str::<Value>(clean_json).map_err(|e| format!("Invalid JSON response: {}. Raw: {}", e, clean_json))
    }

    /// Transcribes student handwriting to pure text (Phase 1 Blind OCR).
    pub async fn transcribe_student_work(&self, sys: &str, parts: Vec<Value>) -> Result<String, String> {
        let payload = build_transcription_payload(sys, parts);
        self.execute_request(&payload).await
    }

    /// Executes Gemini API request with multi-model fallback and key rotation.
    async fn execute_request(&self, payload: &Value) -> Result<String, String> {
        let candidate_models = ModelRegistry::load_candidate_models(Some(&self.preferred_model)).await;
        let client = Client::builder().timeout(std::time::Duration::from_secs(45)).build().unwrap_or_default();
        let mut last_error = String::from("No models available");

        for model in &candidate_models {
            let (key_idx, api_key) = match self.pool.acquire_key() { Ok(k) => k, Err(e) => return Err(e) };
            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key);

            match client.post(&url).json(payload).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    if !status.is_success() || text.contains("RESOURCE_EXHAUSTED") || text.contains("rate limit") {
                        if status == StatusCode::TOO_MANY_REQUESTS || text.contains("RESOURCE_EXHAUSTED") {
                            self.pool.mark_rate_limited(key_idx);
                        }
                        eprintln!("[Fallback] Model '{}' (Status {}) on Key #{}. Trying next...", model, status, key_idx + 1);
                        last_error = format!("Status {} on {}: {}", status, model, text.chars().take(80).collect::<String>());
                        continue;
                    }
                    if let Ok(res_json) = serde_json::from_str::<Value>(&text) {
                        if let Some(err) = res_json.get("error") {
                            eprintln!("[Model Error] Model '{}': {:?}. Trying next...", model, err);
                            last_error = format!("Gemini Error on {}: {:?}", model, err);
                            continue;
                        }
                        if let Some(candidate) = res_json.get("candidates").and_then(|c| c.get(0)) {
                            if let Some(t) = candidate.get("content").and_then(|c| c.get("parts")).and_then(|p| p.get(0)).and_then(|p| p.get("text")).and_then(|t| t.as_str()) {
                                return Ok(t.to_string());
                            }
                        }
                    }
                    last_error = format!("Invalid response format from {}", model);
                }
                Err(e) => {
                    eprintln!("[Network Fallback] Model '{}' error: {}. Trying next...", model, e);
                    last_error = format!("Network error for {}: {}", model, e);
                }
            }
        }
        Err(format!("All candidate Gemini models failed. Last error: {}", last_error))
    }
}
