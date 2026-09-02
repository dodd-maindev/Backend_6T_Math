use axum::{extract::{Multipart, Path, State}, http::StatusCode, Json};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    middleware::auth::AuthenticatedUser,
    models::upload::{StudentQuestionUpload, UploadSummary},
};

/// Student uploads images for a specific question (UPSERT).
pub async fn upload_question_images(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<StudentQuestionUpload>), (StatusCode, String)> {
    let (mut assignment_id, mut question_number) = (None::<Uuid>, None::<i32>);
    let mut file_urls: Vec<String> = Vec::new();

    tokio::fs::create_dir_all("./uploads").await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    while let Some(field) = multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "assignment_id" => { assignment_id = Uuid::parse_str(&field.text().await.unwrap_or_default()).ok(); }
            "question_number" => { question_number = field.text().await.unwrap_or_default().trim().parse().ok(); }
            "image" | "images" | "file" | "files" => {
                if let Ok(data) = field.bytes().await {
                    if !data.is_empty() {
                        let filename = format!("{}.jpg", Uuid::new_v4());
                        let _ = tokio::fs::write(format!("./uploads/{}", filename), &data).await;
                        file_urls.push(format!("/uploads/{}", filename));
                    }
                }
            }
            _ => {}
        }
    }

    let a_id = assignment_id.ok_or((StatusCode::BAD_REQUEST, "Missing assignment_id".into()))?;
    let q_num = question_number.ok_or((StatusCode::BAD_REQUEST, "Missing question_number".into()))?;
    if file_urls.is_empty() { return Err((StatusCode::BAD_REQUEST, "No images uploaded".into())); }

    let urls_json = json!(file_urls);
    let row = sqlx::query_as::<_, StudentQuestionUpload>(
        "INSERT INTO student_question_uploads (student_id, assignment_id, question_number, image_urls) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (student_id, assignment_id, question_number) \
         DO UPDATE SET image_urls = $4, created_at = CURRENT_TIMESTAMP \
         RETURNING id, student_id, assignment_id, question_number, image_urls, created_at"
    ).bind(user.id).bind(a_id).bind(q_num).bind(&urls_json)
    .fetch_one(&pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(row)))
}

/// Student lists their own uploads for a specific assignment.
pub async fn list_my_uploads(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(assignment_id): Path<Uuid>,
) -> Result<Json<Vec<StudentQuestionUpload>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, StudentQuestionUpload>(
        "SELECT id, student_id, assignment_id, question_number, image_urls, created_at \
         FROM student_question_uploads WHERE student_id = $1 AND assignment_id = $2 \
         ORDER BY question_number ASC"
    ).bind(user.id).bind(assignment_id)
    .fetch_all(&pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

/// Admin gets upload summary for all students in an assignment.
pub async fn get_upload_summary(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(assignment_id): Path<Uuid>,
) -> Result<Json<Vec<UploadSummary>>, (StatusCode, String)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin only".into())); }

    let total_q: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assignment_questions WHERE assignment_id = $1")
        .bind(assignment_id).fetch_one(&pool).await.unwrap_or(0);

    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, i64)>(
        "SELECT u.id, u.email, u.full_name, COUNT(sq.id) as uploaded_count \
         FROM users u \
         JOIN student_classrooms sc ON sc.student_id = u.id \
         JOIN assignments a ON a.classroom_id = sc.classroom_id AND a.id = $1 \
         LEFT JOIN student_question_uploads sq ON sq.student_id = u.id AND sq.assignment_id = $1 \
         WHERE u.role = 'student' \
         GROUP BY u.id, u.email, u.full_name \
         ORDER BY u.email ASC"
    ).bind(assignment_id).fetch_all(&pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let summaries = rows.into_iter().map(|(sid, email, name, count)| UploadSummary {
        student_id: sid, email, full_name: name, uploaded_count: count, total_questions: total_q,
    }).collect();

    Ok(Json(summaries))
}
