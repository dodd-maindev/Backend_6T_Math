use serde::{Deserialize, Serialize};

/// Represents a configurable Gemini AI model entry with fallback priority.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub priority: u32,
    pub rpm: u32,
    pub rpd: u32,
    pub description: String,
}

/// Registry responsible for loading prioritized Gemini models for automated rate-limit fallback.
pub struct ModelRegistry;

impl ModelRegistry {
    /// Loads candidate models ordered by priority from config file or fallback defaults.
    pub async fn load_candidate_models(primary_override: Option<&str>) -> Vec<String> {
        let mut models = match tokio::fs::read_to_string("config/gemini_models.json").await {
            Ok(content) => match serde_json::from_str::<Vec<ModelEntry>>(&content) {
                Ok(mut list) => {
                    list.sort_by_key(|m| m.priority);
                    list.into_iter().map(|m| m.id).collect::<Vec<_>>()
                }
                Err(_) => Self::default_model_ids(),
            },
            Err(_) => Self::default_model_ids(),
        };

        if let Some(preferred) = primary_override {
            if let Some(pos) = models.iter().position(|m| m == preferred) {
                let item = models.remove(pos);
                models.insert(0, item);
            } else if !preferred.trim().is_empty() {
                models.insert(0, preferred.trim().to_string());
            }
        }
        models
    }

    /// Hardcoded default model order for standalone reliability.
    fn default_model_ids() -> Vec<String> {
        vec![
            "gemini-3.1-flash-lite".to_string(),
            "gemini-3.5-flash-lite".to_string(),
            "gemini-3.7-flash".to_string(),
            "gemini-3.6-flash".to_string(),
            "gemini-3.5-flash".to_string(),
            "gemini-3-flash".to_string(),
            "gemini-2.5-flash".to_string(),
            "gemini-2.5-flash-lite".to_string(),
        ]
    }
}
