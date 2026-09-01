use axum::{
    extract::State,
    http::StatusCode,
    Extension, Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use sqlx::PgPool;

use crate::{
    config::Config,
    models::auth::LoginRequest,
    models::user::{User, UserResponse},
    utils::{hash::verify_password, jwt::generate_token},
};

/// Handles user login, verifies credentials, and issues an HTTP-Only JWT Cookie.
pub async fn login(
    State(pool): State<PgPool>,
    Extension(config): Extension<Config>,
    jar: CookieJar,
    Json(payload): Json<LoginRequest>,
) -> Result<(CookieJar, Json<UserResponse>), (StatusCode, &'static str)> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, email, password_hash, role, created_at FROM users WHERE email = $1"
    )
    .bind(payload.email.trim())
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("[Login Error] Database query failure: {:?}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Database error")
    })?
    .ok_or((StatusCode::UNAUTHORIZED, "Invalid email or password"))?;

    if !verify_password(&payload.password, &user.password_hash) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid email or password"));
    }

    let token = generate_token(
        user.id,
        &user.email,
        &user.role,
        &config.jwt_secret,
        chrono::Duration::minutes(config.access_token_expire_minutes),
    )
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Token generation failed"))?;

    let mut cookie = Cookie::new("access_token", token);
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);

    let updated_jar = jar.add(cookie);
    Ok((updated_jar, Json(UserResponse::from_user(&user))))
}
