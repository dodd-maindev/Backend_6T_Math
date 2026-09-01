use axum::{extract::{Path, State}, http::StatusCode, Json};
use sqlx::PgPool;
use uuid::Uuid;
use crate::{middleware::auth::AuthenticatedUser, models::assignment::{Assignment, CreateAssignmentRequest}};

/// Creates a new assignment within a classroom.
pub async fn create_assignment(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(classroom_id): Path<Uuid>,
    Json(payload): Json<CreateAssignmentRequest>,
) -> Result<(StatusCode, Json<Assignment>), (StatusCode, &'static str)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin required")); }
    let assignment = sqlx::query_as::<_, Assignment>(
        "INSERT INTO assignments (classroom_id, title) VALUES ($1, $2) RETURNING id, classroom_id, title, created_at"
    )
    .bind(classroom_id).bind(&payload.title).fetch_one(&pool).await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create assignment"))?;
    Ok((StatusCode::CREATED, Json(assignment)))
}

/// Retrieves all assignments belonging to a specific classroom.
pub async fn list_assignments(
    State(pool): State<PgPool>,
    _user: AuthenticatedUser,
    Path(classroom_id): Path<Uuid>,
) -> Result<Json<Vec<Assignment>>, (StatusCode, &'static str)> {
    let assignments = sqlx::query_as::<_, Assignment>(
        "SELECT id, classroom_id, title, created_at FROM assignments WHERE classroom_id = $1 ORDER BY created_at DESC"
    )
    .bind(classroom_id).fetch_all(&pool).await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch assignments"))?;
    Ok(Json(assignments))
}

/// Deletes an assignment and its associated questions and submissions.
pub async fn delete_assignment(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(assignment_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin required")); }
    let _ = sqlx::query("DELETE FROM student_submissions WHERE assignment_id = $1").bind(assignment_id).execute(&pool).await;
    let _ = sqlx::query("DELETE FROM assignment_questions WHERE assignment_id = $1").bind(assignment_id).execute(&pool).await;
    sqlx::query("DELETE FROM assignments WHERE id = $1")
        .bind(assignment_id).execute(&pool).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete assignment"))?;
    Ok(StatusCode::NO_CONTENT)
}
