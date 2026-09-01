use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use crate::utils::{api_key_pool::ApiKeyPool, model_registry::ModelRegistry};

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

    pub async fn evaluate_submission(&self, system_instruction: &str, parts: Vec<Value>) -> Result<Value, String> {
        let candidate_models = ModelRegistry::load_candidate_models(Some(&self.preferred_model)).await;
        let payload = Self::build_payload(system_instruction, parts);
        let client = Client::builder().timeout(std::time::Duration::from_secs(45)).build().unwrap_or_default();
        let mut last_error = String::from("No models available");

        for model in &candidate_models {
            let (key_idx, api_key) = match self.pool.acquire_key() { Ok(k) => k, Err(e) => return Err(e) };
            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key);

            match client.post(&url).json(&payload).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    if !status.is_success() || text.contains("RESOURCE_EXHAUSTED") || text.contains("rate limit") {
                        if status == StatusCode::TOO_MANY_REQUESTS || text.contains("RESOURCE_EXHAUSTED") {
                            self.pool.mark_rate_limited(key_idx);
                        }
                        eprintln!("[Fallback] Model '{}' (Status {}) on Key #{}. Trying next model...", model, status, key_idx + 1);
                        last_error = format!("Status {} on {}: {}", status, model, text.chars().take(80).collect::<String>());
                        continue;
                    }
                    if let Ok(res_json) = serde_json::from_str::<Value>(&text) {
                        if let Some(err) = res_json.get("error") {
                            eprintln!("[Model Error Fallback] Model '{}' returned error: {:?}. Trying next...", model, err);
                            last_error = format!("Gemini Error on {}: {:?}", model, err);
                            continue;
                        }
                        if let Some(candidate) = res_json.get("candidates").and_then(|c| c.get(0)) {
                            if let Some(text_content) = candidate.get("content").and_then(|c| c.get("parts")).and_then(|p| p.get(0)).and_then(|p| p.get("text")).and_then(|t| t.as_str()) {
                                let clean_json = text_content.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
                                if let Ok(parsed) = serde_json::from_str::<Value>(clean_json) {
                                    return Ok(parsed);
                                }
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

    fn build_payload(sys: &str, parts: Vec<Value>) -> Value {
        json!({
            "systemInstruction": {"parts": [{"text": sys}]},
            "contents": [{"parts": parts}],
            "generationConfig": {
                "temperature": 0.0, "topP": 0.1, "seed": 42,
                "responseMimeType": "application/json",
                "responseSchema": {
                    "type": "OBJECT",
                    "properties": {
                        "score": {"type": "NUMBER"}, "general_feedback": {"type": "STRING"},
                        "questions": {
                            "type": "ARRAY",
                            "items": {
                                "type": "OBJECT",
                                "properties": {
                                    "question_title": {"type": "STRING"}, "allocated_score": {"type": "NUMBER"},
                                    "max_score": {"type": "NUMBER"}, "teacher_comment": {"type": "STRING"},
                                    "steps": {
                                        "type": "ARRAY",
                                        "items": {
                                            "type": "OBJECT",
                                            "properties": {
                                                "step_desc": {"type": "STRING"}, "allocated_score": {"type": "NUMBER"},
                                                "max_score": {"type": "NUMBER"}, "status": {"type": "STRING", "enum": ["Correct", "Incorrect", "Missing"]}
                                            },
                                            "required": ["step_desc", "allocated_score", "max_score", "status"]
                                        }
                                    }
                                },
                                "required": ["question_title", "allocated_score", "max_score", "teacher_comment", "steps"]
                            }
                        }
                    },
                    "required": ["score", "general_feedback", "questions"]
                }
            }
        })
    }
}
