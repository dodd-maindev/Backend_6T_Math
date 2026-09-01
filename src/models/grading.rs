use serde::Serialize;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use bigdecimal::BigDecimal;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StudentSubmission {
    pub id: Uuid,
    pub student_id: Uuid,
    pub assignment_id: Uuid,
    pub score: Option<BigDecimal>,
    pub feedback: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StudentSubmissionWithDetails {
    pub id: Uuid,
    pub student_id: Uuid,
    pub assignment_id: Uuid,
    pub assignment_title: String,
    pub score: Option<BigDecimal>,
    pub feedback: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}
