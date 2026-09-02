use axum::{extract::{Multipart, Path, State}, http::StatusCode, Json};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::{
    middleware::auth::AuthenticatedUser,
    models::assignment::AssignmentQuestion,
    models::grading::{StudentSubmission, StudentSubmissionWithDetails},
    services::grading_service::GradingService,
    utils::{submission_limiter::SubmissionLimiter, submission_parser::parse_submission_form, turnstile::verify_turnstile_token},
};

/// Lists all grading submissions for a given student ID.
pub async fn list_student_submissions(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(student_id): Path<Uuid>,
) -> Result<Json<Vec<StudentSubmissionWithDetails>>, (StatusCode, String)> {
    if user.role != "admin" && user.id != student_id { return Err((StatusCode::FORBIDDEN, "Forbidden".into())); }
    let submissions = sqlx::query_as::<_, StudentSubmissionWithDetails>(
        "SELECT s.id, s.student_id, s.assignment_id, a.title as assignment_title, s.score, s.feedback, s.created_at \
         FROM student_submissions s JOIN assignments a ON s.assignment_id = a.id \
         WHERE s.student_id = $1 ORDER BY s.created_at DESC"
    ).bind(student_id).fetch_all(&pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(submissions))
}

/// Evaluates a single question with Cloudflare Turnstile bot verification and attempt limiter.
pub async fn grade_submission(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    multipart: Multipart,
) -> Result<(StatusCode, Json<StudentSubmission>), (StatusCode, String)> {
    let form = parse_submission_form(multipart).await.map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let s_id = form.student_id.ok_or((StatusCode::BAD_REQUEST, "Missing student_id".into()))?;
    let a_id = form.assignment_id.ok_or((StatusCode::BAD_REQUEST, "Missing assignment_id".into()))?;
    if user.role != "admin" && user.id != s_id { return Err((StatusCode::FORBIDDEN, "Forbidden".into())); }
    if form.student_files.is_empty() { return Err((StatusCode::BAD_REQUEST, "Missing files".into())); }

    let cf_secret = std::env::var("CLOUDFLARE_TURNSTILE_SECRET_KEY").unwrap_or_default();
    verify_turnstile_token(&cf_secret, form.turnstile_token.as_deref().unwrap_or(""), None).await.map_err(|e| (StatusCode::FORBIDDEN, e))?;

    let questions = sqlx::query_as::<_, AssignmentQuestion>(
        "SELECT id, assignment_id, question_number, reference_image_url, question_image_urls, solution_image_urls, native_prompt, max_score, created_at \
         FROM assignment_questions WHERE assignment_id = $1 ORDER BY question_number ASC"
    ).bind(a_id).fetch_all(&pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let question = form.question_number.and_then(|qn| questions.iter().find(|q| q.question_number == qn)).or_else(|| questions.first()).ok_or((StatusCode::BAD_REQUEST, "No questions found".into()))?;
    SubmissionLimiter::check_single_question(&pool, &user.role, s_id, a_id, question.question_number).await?;

    let grader = GradingService::new().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let mut feedback = grader.grade_question(question, &form.student_files, true).await.map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    feedback["student_image_urls"] = json!(form.file_urls);
    let score = feedback["score"].as_f64().unwrap_or(0.0);

    let sub = sqlx::query_as::<_, StudentSubmission>(
        "INSERT INTO student_submissions (student_id, assignment_id, score, feedback) VALUES ($1, $2, $3, $4) RETURNING id, student_id, assignment_id, score, feedback, created_at"
    ).bind(s_id).bind(a_id).bind(score).bind(&feedback).fetch_one(&pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::CREATED, Json(sub)))
}

/// Evaluates full exam with Cloudflare Turnstile bot verification and guaranteed complete evaluation.
pub async fn grade_full_exam(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    multipart: Multipart,
) -> Result<(StatusCode, Json<Vec<StudentSubmission>>), (StatusCode, String)> {
    let form = parse_submission_form(multipart).await.map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let s_id = form.student_id.ok_or((StatusCode::BAD_REQUEST, "Missing student_id".into()))?;
    let a_id = form.assignment_id.ok_or((StatusCode::BAD_REQUEST, "Missing assignment_id".into()))?;
    if user.role != "admin" && user.id != s_id { return Err((StatusCode::FORBIDDEN, "Forbidden".into())); }
    if form.student_files.is_empty() { return Err((StatusCode::BAD_REQUEST, "Missing files".into())); }

    let cf_secret = std::env::var("CLOUDFLARE_TURNSTILE_SECRET_KEY").unwrap_or_default();
    verify_turnstile_token(&cf_secret, form.turnstile_token.as_deref().unwrap_or(""), None).await.map_err(|e| (StatusCode::FORBIDDEN, e))?;

    let questions = sqlx::query_as::<_, AssignmentQuestion>(
        "SELECT id, assignment_id, question_number, reference_image_url, question_image_urls, solution_image_urls, native_prompt, max_score, created_at \
         FROM assignment_questions WHERE assignment_id = $1 ORDER BY question_number ASC"
    ).bind(a_id).fetch_all(&pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if questions.is_empty() { return Err((StatusCode::BAD_REQUEST, "No questions found".into()))?; }
    SubmissionLimiter::check_full_exam(&pool, &user.role, s_id, a_id, questions.len() as i64).await?;

    let grader = Arc::new(GradingService::new().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?);
    let transcripts = Arc::new(grader.transcribe_full_exam(&form.student_files).await.unwrap_or_default());
    let mut set = JoinSet::new();

    for q in questions {
        let (g, t) = (Arc::clone(&grader), Arc::clone(&transcripts));
        set.spawn(async move {
            let tr = t.get(&q.question_number).map(|s| s.as_str()).unwrap_or("Học sinh không làm bài này.");
            (q.question_number, g.grade_question_with_transcript(&q, tr).await)
        });
    }

    let mut saved_subs = Vec::new();
    while let Some(res) = set.join_next().await {
        if let Ok((_qn, Ok(mut fb))) = res {
            fb["student_image_urls"] = json!(form.file_urls);
            let score = fb["score"].as_f64().unwrap_or(0.0);
            if let Ok(sub) = sqlx::query_as::<_, StudentSubmission>(
                "INSERT INTO student_submissions (student_id, assignment_id, score, feedback) VALUES ($1, $2, $3, $4) RETURNING id, student_id, assignment_id, score, feedback, created_at"
            ).bind(s_id).bind(a_id).bind(score).bind(&fb).fetch_one(&pool).await {
                saved_subs.push(sub);
            }
        }
    }
    saved_subs.sort_by_key(|s| s.feedback.as_ref().and_then(|f| f.get("question_number")).and_then(|v| v.as_i64()).unwrap_or(0));
    Ok((StatusCode::CREATED, Json(saved_subs)))
}
