use axum::{extract::State, http::StatusCode, Json};
use sqlx::PgPool;

use crate::{
    middleware::auth::AuthenticatedUser,
    models::user::{User, UserResponse},
};

/// Returns the profile details of the currently authenticated user.
pub async fn get_me(
    State(pool): State<PgPool>,
    user: AuthenticatedUser,
) -> Result<Json<UserResponse>, (StatusCode, &'static str)> {
    // Retrieve latest user details from the database
    let db_user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::NOT_FOUND, "User not found"))?;

    Ok(Json(UserResponse::from_user(&db_user)))
}
