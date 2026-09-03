use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use bigdecimal::BigDecimal;

#[derive(Debug, Deserialize)]
pub struct CreateAssignmentRequest {
    pub title: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Assignment {
    pub id: Uuid,
    pub classroom_id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AssignmentQuestion {
    pub id: Uuid,
    pub assignment_id: Uuid,
    pub question_number: i32,
    pub reference_image_url: String,
    pub question_image_urls: Option<serde_json::Value>,
    pub solution_image_urls: Option<serde_json::Value>,
    pub native_prompt: Option<String>,
    pub max_score: BigDecimal,
    pub barem_json: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBaremRequest {
    pub barem_json: serde_json::Value,
}
