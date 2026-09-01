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
        let client = Client::new();
        let mut last_error = String::from("No models available");

        for model in &candidate_models {
            let (key_idx, api_key) = self.pool.acquire_key().map_err(|e| e)?;
            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key);

            match client.post(&url).json(&payload).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    if status == StatusCode::TOO_MANY_REQUESTS || text.contains("RESOURCE_EXHAUSTED") || text.contains("rate limit") {
                        eprintln!("[RateLimit Fallback] Model '{}' rate limited (status {}). Switching Key/Model...", model, status);
                        self.pool.mark_rate_limited(key_idx);
                        last_error = format!("Model {} on Key #{} rate limited", model, key_idx + 1);
                        continue;
                    }
                    if let Ok(res_json) = serde_json::from_str::<Value>(&text) {
                        if let Some(err) = res_json.get("error") {
                            let msg = err["message"].as_str().unwrap_or("Unknown Gemini Error");
                            if msg.contains("quota") || msg.contains("limit") || msg.contains("exhausted") {
                                eprintln!("[Quota Fallback] Model '{}' quota error on Key #{}. Switching...", model, key_idx + 1);
                                self.pool.mark_rate_limited(key_idx);
                                last_error = format!("Quota error: {}", msg);
                                continue;
                            }
                            return Err(format!("Gemini API Error: {}", msg));
                        }
                        if let Some(content) = res_json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                            return serde_json::from_str(content).map_err(|e| format!("JSON parse error: {}", e));
                        }
                    }
                    last_error = format!("Invalid response from {}: {}", model, text);
                }
                Err(e) => {
                    eprintln!("[Network Fallback] Model '{}' error: {}. Trying next...", model, e);
                    last_error = format!("Network error for {}: {}", model, e);
                }
            }
        }
        Err(format!("All candidate Gemini models/keys failed. Last error: {}", last_error))
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
