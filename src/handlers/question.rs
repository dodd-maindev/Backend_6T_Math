use axum::{extract::{Multipart, Path, State}, http::StatusCode, Json};
use sqlx::PgPool;
use uuid::Uuid;
use crate::{middleware::auth::AuthenticatedUser, models::assignment::AssignmentQuestion};

/// Lists all questions for an assignment, sanitizing solutions for student users.
pub async fn list_questions(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(assignment_id): Path<Uuid>,
) -> Result<Json<Vec<AssignmentQuestion>>, (StatusCode, &'static str)> {
    let mut questions = sqlx::query_as::<_, AssignmentQuestion>(
        "SELECT id, assignment_id, question_number, reference_image_url, question_image_urls, solution_image_urls, native_prompt, max_score, created_at \
         FROM assignment_questions WHERE assignment_id = $1 ORDER BY question_number ASC"
    )
    .bind(assignment_id).fetch_all(&pool).await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch questions"))?;

    if user.role != "admin" {
        for q in &mut questions {
            q.reference_image_url = String::new();
            q.solution_image_urls = Some(serde_json::json!([]));
            q.native_prompt = None;
        }
    }
    Ok(Json(questions))
}

/// Adds or updates a question in an assignment with multi-part images.
pub async fn add_question(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(assignment_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, &'static str), (StatusCode, &'static str)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin required")); }
    let (mut question_number, mut native_prompt, mut max_score) = (None, None, 2.50);
    let (mut q_images, mut sol_images) = (Vec::new(), Vec::new());
    tokio::fs::create_dir_all("./uploads").await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Dir error"))?;

    while let Some(field) = multipart.next_field().await.map_err(|_| (StatusCode::BAD_REQUEST, "Multipart error"))? {
        let name = field.name().unwrap_or("").to_string();
        if name == "question_number" { question_number = field.text().await.unwrap_or_default().parse::<i32>().ok(); }
        else if name == "native_prompt" { let v = field.text().await.unwrap_or_default(); if !v.trim().is_empty() { native_prompt = Some(v.trim().to_string()); } }
        else if name == "max_score" { max_score = field.text().await.unwrap_or_default().parse::<f64>().unwrap_or(2.50); }
        else if name == "question_images" || name == "question_image" {
            let ext = if field.content_type().unwrap_or("").contains("png") { "png" } else { "jpg" };
            let filename = format!("{}.{}", Uuid::new_v4(), ext);
            if let Ok(d) = field.bytes().await { if !d.is_empty() { let _ = tokio::fs::write(format!("./uploads/{}", filename), &d).await; q_images.push(format!("/uploads/{}", filename)); } }
        } else if name == "solution_images" || name == "solution_image" || name == "image" || name == "file" {
            let ext = if field.content_type().unwrap_or("").contains("png") { "png" } else { "jpg" };
            let filename = format!("{}.{}", Uuid::new_v4(), ext);
            if let Ok(d) = field.bytes().await { if !d.is_empty() { let _ = tokio::fs::write(format!("./uploads/{}", filename), &d).await; sol_images.push(format!("/uploads/{}", filename)); } }
        }
    }

    let q_num = question_number.ok_or((StatusCode::BAD_REQUEST, "Missing question number"))?;
    let ref_img = sol_images.first().cloned().unwrap_or_default();
    sqlx::query(
        "INSERT INTO assignment_questions (assignment_id, question_number, reference_image_url, question_image_urls, solution_image_urls, native_prompt, max_score) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (assignment_id, question_number) \
         DO UPDATE SET reference_image_url = EXCLUDED.reference_image_url, question_image_urls = EXCLUDED.question_image_urls, solution_image_urls = EXCLUDED.solution_image_urls, native_prompt = EXCLUDED.native_prompt, max_score = EXCLUDED.max_score"
    )
    .bind(assignment_id).bind(q_num).bind(ref_img).bind(serde_json::json!(q_images)).bind(serde_json::json!(sol_images)).bind(native_prompt).bind(max_score).execute(&pool).await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to save question"))?;
    Ok((StatusCode::OK, "Question saved successfully"))
}

/// Deletes a specific question by its ID.
pub async fn delete_question(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(question_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin required")); }
    sqlx::query("DELETE FROM assignment_questions WHERE id = $1")
        .bind(question_id).execute(&pool).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete question"))?;
    Ok(StatusCode::NO_CONTENT)
}
