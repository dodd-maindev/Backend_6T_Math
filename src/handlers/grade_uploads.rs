use axum::{extract::State, http::StatusCode, Json};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::task::JoinSet;

use crate::{
    middleware::auth::AuthenticatedUser,
    models::assignment::AssignmentQuestion,
    models::grading::StudentSubmission,
    models::upload::{GradeUploadsRequest, StudentQuestionUpload},
    services::grading_service::{GradingService, StudentFilePayload},
};

/// Teacher-initiated grading: reads uploaded images per-question and grades each via grade_question().
pub async fn grade_student_uploads(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Json(req): Json<GradeUploadsRequest>,
) -> Result<(StatusCode, Json<Vec<StudentSubmission>>), (StatusCode, String)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin only".into())); }

    let uploads = sqlx::query_as::<_, StudentQuestionUpload>(
        "SELECT id, student_id, assignment_id, question_number, image_urls, created_at \
         FROM student_question_uploads WHERE student_id = $1 AND assignment_id = $2"
    ).bind(req.student_id).bind(req.assignment_id)
    .fetch_all(&pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if uploads.is_empty() { return Err((StatusCode::BAD_REQUEST, "Student has no uploads".into())); }

    let questions = sqlx::query_as::<_, AssignmentQuestion>(
        "SELECT id, assignment_id, question_number, reference_image_url, question_image_urls, \
         solution_image_urls, native_prompt, max_score, barem_json, created_at \
         FROM assignment_questions WHERE assignment_id = $1"
    ).bind(req.assignment_id).fetch_all(&pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let grader = Arc::new(GradingService::new().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?);
    let mut set = JoinSet::new();

    for upload in &uploads {
        let q = questions.iter().find(|q| q.question_number == upload.question_number);
        if q.is_none() { continue; }
        let question = q.unwrap().clone();
        let urls: Vec<String> = upload.image_urls.as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let g = Arc::clone(&grader);
        let file_urls_clone = urls.clone();

        set.spawn(async move {
            let mut files = Vec::new();
            for url in &file_urls_clone {
                let path = format!(".{}", url);
                if let Ok(data) = tokio::fs::read(&path).await {
                    files.push(StudentFilePayload { mime_type: "image/jpeg".into(), base64_data: STANDARD.encode(&data) });
                }
            }
            if files.is_empty() { return None; }
            let mut fb = g.grade_question(&question, &files, true).await.ok()?;
            fb["student_image_urls"] = json!(file_urls_clone);
            Some((question.question_number, fb))
        });
    }

    let mut saved = Vec::new();
    while let Some(res) = set.join_next().await {
        if let Ok(Some((_qn, fb))) = res {
            let score = fb["score"].as_f64().unwrap_or(0.0);
            if let Ok(sub) = sqlx::query_as::<_, StudentSubmission>(
                "INSERT INTO student_submissions (student_id, assignment_id, score, feedback) \
                 VALUES ($1, $2, $3, $4) RETURNING id, student_id, assignment_id, score, feedback, created_at"
            ).bind(req.student_id).bind(req.assignment_id).bind(score).bind(&fb)
            .fetch_one(&pool).await { saved.push(sub); }
        }
    }

    saved.sort_by_key(|s| s.feedback.as_ref().and_then(|f| f.get("question_number")).and_then(|v| v.as_i64()).unwrap_or(0));
    Ok((StatusCode::CREATED, Json(saved)))
}
