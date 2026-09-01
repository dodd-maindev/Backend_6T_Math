use axum::{extract::{Path, State}, http::StatusCode, Json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    middleware::auth::AuthenticatedUser,
    models::classroom::CreateStudentRequest,
    models::user::{UpdateStudentRequest, User, UserResponse},
    utils::hash::hash_password,
};

/// Endpoint for Admins to create a student account and map them to a classroom.
pub async fn create_student(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateStudentRequest>,
) -> Result<(StatusCode, Json<UserResponse>), (StatusCode, &'static str)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Only administrators can perform this action")); }

    let email = payload.email.trim();
    if email.is_empty() || payload.password.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Email and password cannot be empty"));
    }

    let password_hash = hash_password(&payload.password)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Password hashing failed"))?;

    let mut tx = pool.begin().await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Transaction failure"))?;
    let classroom_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM classrooms WHERE id = $1)")
        .bind(payload.classroom_id).fetch_one(&mut *tx).await.unwrap_or(false);

    if !classroom_exists { return Err((StatusCode::BAD_REQUEST, "Specified classroom does not exist")); }

    let student = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, password_hash, role, full_name, address, school, phone_number) \
         VALUES ($1, $2, 'student', $3, $4, $5, $6) \
         RETURNING id, email, password_hash, role, full_name, address, school, phone_number, created_at"
    )
    .bind(email).bind(password_hash).bind(payload.full_name).bind(payload.address).bind(payload.school).bind(payload.phone_number)
    .fetch_one(&mut *tx).await.map_err(|_| (StatusCode::CONFLICT, "Email is already in use"))?;

    sqlx::query("INSERT INTO student_classrooms (student_id, classroom_id) VALUES ($1, $2)")
        .bind(student.id).bind(payload.classroom_id).execute(&mut *tx).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to map student to classroom"))?;

    tx.commit().await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to commit database changes"))?;
    Ok((StatusCode::CREATED, Json(UserResponse::from_user(&student))))
}

/// Endpoint for Admins to update a student's profile (name, school, address, phone) or reset their password.
pub async fn update_student(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
    Path(student_id): Path<Uuid>,
    Json(payload): Json<UpdateStudentRequest>,
) -> Result<Json<UserResponse>, (StatusCode, String)> {
    if user.role != "admin" { return Err((StatusCode::FORBIDDEN, "Admin required".into())); }

    let mut query_builder = sqlx::QueryBuilder::<sqlx::Postgres>::new("UPDATE users SET ");
    let mut separated = query_builder.separated(", ");

    if let Some(ref name) = payload.full_name { separated.push("full_name = "); separated.push_bind_unseparated(name.trim()); }
    if let Some(ref addr) = payload.address { separated.push("address = "); separated.push_bind_unseparated(addr.trim()); }
    if let Some(ref sch) = payload.school { separated.push("school = "); separated.push_bind_unseparated(sch.trim()); }
    if let Some(ref phone) = payload.phone_number { separated.push("phone_number = "); separated.push_bind_unseparated(phone.trim()); }

    if let Some(ref new_pass) = payload.new_password {
        if !new_pass.trim().is_empty() {
            let pass_hash = hash_password(new_pass.trim()).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            separated.push("password_hash = "); separated.push_bind_unseparated(pass_hash);
        }
    }

    query_builder.push(" WHERE id = ");
    query_builder.push_bind(student_id);
    query_builder.push(" RETURNING id, email, password_hash, role, full_name, address, school, phone_number, created_at");

    let updated_user = query_builder.build_query_as::<User>().fetch_one(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to update student: {}", e)))?;

    Ok(Json(UserResponse::from_user(&updated_user)))
}
