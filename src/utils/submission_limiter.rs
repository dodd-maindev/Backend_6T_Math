use axum::http::StatusCode;
use sqlx::PgPool;
use uuid::Uuid;

/// Enforces attempt limits per student per assignment/question (5 per question for students, 10 for teachers).
pub struct SubmissionLimiter;

impl SubmissionLimiter {
    /// Validates attempt limit for a single question.
    pub async fn check_single_question(
        pool: &PgPool,
        role: &str,
        student_id: Uuid,
        assignment_id: Uuid,
        question_number: i32,
    ) -> Result<(), (StatusCode, String)> {
        let max_attempts = if role == "admin" { 10 } else { 5 };
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM student_submissions \
             WHERE student_id = $1 AND assignment_id = $2 AND (feedback->>'question_number')::int = $3"
        )
        .bind(student_id).bind(assignment_id).bind(question_number)
        .fetch_one(pool).await.unwrap_or(0);

        if count >= max_attempts {
            let role_name = if role == "admin" { "Giáo viên" } else { "Học sinh" };
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                format!("{} đã sử dụng hết {}/{} lượt chấm cho Bài {}.", role_name, count, max_attempts, question_number),
            ));
        }
        Ok(())
    }

    /// Validates attempt limit for a full exam submission.
    pub async fn check_full_exam(
        pool: &PgPool,
        role: &str,
        student_id: Uuid,
        assignment_id: Uuid,
        total_questions: i64,
    ) -> Result<(), (StatusCode, String)> {
        let per_question_limit = if role == "admin" { 10 } else { 5 };
        let max_total_attempts = (per_question_limit * total_questions).max(per_question_limit);
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM student_submissions WHERE student_id = $1 AND assignment_id = $2"
        )
        .bind(student_id).bind(assignment_id)
        .fetch_one(pool).await.unwrap_or(0);

        if count >= max_total_attempts {
            let role_name = if role == "admin" { "Giáo viên" } else { "Học sinh" };
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                format!("{} đã đạt giới hạn tối đa {} lượt chấm cho đề thi này ({}/{} lượt đã dùng).", role_name, max_total_attempts, count, max_total_attempts),
            ));
        }
        Ok(())
    }
}
