use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use crate::utils::model_registry::ModelRegistry;

/// Client for communicating with Google Gemini API with automated multi-model fallback.
#[derive(Clone, Debug)]
pub struct GeminiClient {
    api_key: String,
    preferred_model: String,
}

impl GeminiClient {
    pub fn from_env() -> Result<Self, String> {
        let api_key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| "Missing GEMINI_API_KEY environment variable".to_string())?;
        let preferred_model = std::env::var("GEMINI_MODEL")
            .unwrap_or_else(|_| "gemini-3.1-flash-lite".to_string());
        Ok(Self { api_key, preferred_model })
    }

    pub async fn evaluate_submission(&self, system_instruction: &str, parts: Vec<Value>) -> Result<Value, String> {
        let candidate_models = ModelRegistry::load_candidate_models(Some(&self.preferred_model)).await;
        let payload = Self::build_payload(system_instruction, parts);
        let client = Client::new();
        let mut last_error = String::from("No models available");

        for model in &candidate_models {
            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, self.api_key);
            match client.post(&url).json(&payload).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    if status == StatusCode::TOO_MANY_REQUESTS || text.contains("RESOURCE_EXHAUSTED") || text.contains("rate limit") {
                        eprintln!("[RateLimit Fallback] Model '{}' rate limited (status {}). Trying next fallback model...", model, status);
                        last_error = format!("Model {} rate limited: {}", model, text);
                        continue;
                    }
                    if let Ok(res_json) = serde_json::from_str::<Value>(&text) {
                        if let Some(err) = res_json.get("error") {
                            let msg = err["message"].as_str().unwrap_or("Unknown Gemini Error");
                            if msg.contains("quota") || msg.contains("limit") || msg.contains("exhausted") {
                                eprintln!("[RateLimit Fallback] Model '{}' quota exhausted: {}. Falling back...", model, msg);
                                last_error = format!("Model {} quota error: {}", model, msg);
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
                    eprintln!("[Network Fallback] Model '{}' network error: {}. Trying next...", model, e);
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
