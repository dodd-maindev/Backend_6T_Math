use axum::{extract::{Path, State}, http::StatusCode, Json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    middleware::auth::AuthenticatedUser,
    models::classroom::{Classroom, CreateClassroomRequest},
    models::user::{User, UserResponse},
};

/// Endpoint for Admins to create a new classroom.
pub async fn create_classroom(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateClassroomRequest>,
) -> Result<(StatusCode, Json<Classroom>), (StatusCode, &'static str)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin required")); }
    if payload.name.trim().is_empty() { return Err((StatusCode::BAD_REQUEST, "Name empty")); }

    let classroom = sqlx::query_as::<_, Classroom>(
        "INSERT INTO classrooms (name) VALUES ($1) RETURNING id, name, created_at"
    )
    .bind(payload.name.trim()).fetch_one(&pool).await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create classroom"))?;
    Ok((StatusCode::CREATED, Json(classroom)))
}

/// Endpoint for Admins to list all classrooms.
pub async fn list_classrooms(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
) -> Result<Json<Vec<Classroom>>, (StatusCode, &'static str)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin required")); }
    let classrooms = sqlx::query_as::<_, Classroom>(
        "SELECT id, name, created_at FROM classrooms ORDER BY name ASC"
    )
    .fetch_all(&pool).await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch classrooms"))?;
    Ok(Json(classrooms))
}

/// Endpoint for Admins to list all students in a specific classroom.
pub async fn list_classroom_students(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(classroom_id): Path<Uuid>,
) -> Result<Json<Vec<UserResponse>>, (StatusCode, &'static str)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin required")); }
    let students = sqlx::query_as::<_, User>(
        "SELECT u.id, u.email, u.password_hash, u.role, u.created_at \
         FROM users u JOIN student_classrooms sc ON u.id = sc.student_id \
         WHERE sc.classroom_id = $1 ORDER BY u.email ASC"
    )
    .bind(classroom_id).fetch_all(&pool).await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch classroom students"))?;
    let response = students.into_iter().map(|s| UserResponse::from_user(&s)).collect();
    Ok(Json(response))
}

/// Endpoint for a Student to retrieve their mapped classroom.
pub async fn get_my_classroom(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
) -> Result<Json<Classroom>, (StatusCode, &'static str)> {
    let classroom = sqlx::query_as::<_, Classroom>(
        "SELECT c.id, c.name, c.created_at \
         FROM classrooms c JOIN student_classrooms sc ON c.id = sc.classroom_id \
         WHERE sc.student_id = $1 LIMIT 1"
    )
    .bind(user.id).fetch_optional(&pool).await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to query classroom"))?
    .ok_or((StatusCode::NOT_FOUND, "No classroom mapped for student"))?;
    Ok(Json(classroom))
}
