use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;
use crate::{
    middleware::auth::AuthenticatedUser,
    models::assignment::{AssignmentQuestion, UpdateBaremRequest},
    services::barem_service::BaremService,
    utils::gemini_client::GeminiClient,
};

/// Retrieves the canonical barem JSON for a question.
pub async fn get_question_barem(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(question_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin required".into())); }
    let q = sqlx::query_as::<_, AssignmentQuestion>(
        "SELECT id, assignment_id, question_number, reference_image_url, question_image_urls, \
         solution_image_urls, native_prompt, max_score, barem_json, created_at \
         FROM assignment_questions WHERE id = $1"
    ).bind(question_id).fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Question not found".into()))?;

    let client = GeminiClient::from_env().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let barem = BaremService::get_or_compile_barem(&pool, &client, &q).await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    Ok(Json(barem))
}

/// Triggers AI to extract/rerun the canonical barem from solution images and saves to DB.
pub async fn extract_question_barem(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(question_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin required".into())); }
    let q = sqlx::query_as::<_, AssignmentQuestion>(
        "SELECT id, assignment_id, question_number, reference_image_url, question_image_urls, \
         solution_image_urls, native_prompt, max_score, barem_json, created_at \
         FROM assignment_questions WHERE id = $1"
    ).bind(question_id).fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Question not found".into()))?;

    let client = GeminiClient::from_env().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let extracted = BaremService::extract_canonical_barem(&client, &q).await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    BaremService::save_barem(&pool, question_id, &extracted).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(extracted))
}

/// Updates the canonical barem JSON with teacher edits.
pub async fn update_question_barem(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(question_id): Path<Uuid>,
    Json(payload): Json<UpdateBaremRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin required".into())); }
    BaremService::save_barem(&pool, question_id, &payload.barem_json).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(payload.barem_json))
}
