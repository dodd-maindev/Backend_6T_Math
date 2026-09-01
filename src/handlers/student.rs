use axum::{extract::State, http::StatusCode, Json};
use sqlx::PgPool;

use crate::{
    middleware::auth::AuthenticatedUser,
    models::classroom::CreateStudentRequest,
    models::user::{User, UserResponse},
    utils::hash::hash_password,
};

/// Endpoint for Admins to create a student account and map them to a classroom.
pub async fn create_student(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateStudentRequest>,
) -> Result<(StatusCode, Json<UserResponse>), (StatusCode, &'static str)> {
    // Check if current user is admin
    if user.role != "admin" {
        return Err((StatusCode::FORBIDDEN, "Only administrators can perform this action"));
    }

    let email = payload.email.trim();
    if email.is_empty() || payload.password.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Email and password cannot be empty"));
    }

    let password_hash = hash_password(&payload.password)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Password hashing failed"))?;

    // Execute in transaction
    let mut tx = pool.begin().await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Transaction failure"))?;

    // Check if classroom exists
    let classroom_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM classrooms WHERE id = $1)")
        .bind(payload.classroom_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify classroom"))?;

    if !classroom_exists {
        return Err((StatusCode::BAD_REQUEST, "Specified classroom does not exist"));
    }

    // Create user
    let student = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, password_hash, role) VALUES ($1, $2, 'student') RETURNING id, email, password_hash, role, created_at"
    )
    .bind(email)
    .bind(password_hash)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        if let Some(db_err) = e.as_database_error() {
            if db_err.is_unique_violation() {
                return (StatusCode::CONFLICT, "Email is already in use");
            }
        }
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create student user")
    })?;

    // Add to student_classrooms
    sqlx::query("INSERT INTO student_classrooms (student_id, classroom_id) VALUES ($1, $2)")
        .bind(student.id)
        .bind(payload.classroom_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to map student to classroom"))?;

    tx.commit().await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to commit database changes"))?;

    Ok((StatusCode::CREATED, Json(UserResponse::from_user(&student))))
}
