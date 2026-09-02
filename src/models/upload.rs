use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a student's uploaded images for a specific question.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StudentQuestionUpload {
    pub id: Uuid,
    pub student_id: Uuid,
    pub assignment_id: Uuid,
    pub question_number: i32,
    pub image_urls: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Summary of a student's upload progress for an assignment.
#[derive(Debug, Serialize)]
pub struct UploadSummary {
    pub student_id: Uuid,
    pub email: String,
    pub full_name: Option<String>,
    pub uploaded_count: i64,
    pub total_questions: i64,
}

/// Request body for teacher-initiated grading from uploads.
#[derive(Debug, Deserialize)]
pub struct GradeUploadsRequest {
    pub student_id: Uuid,
    pub assignment_id: Uuid,
}
