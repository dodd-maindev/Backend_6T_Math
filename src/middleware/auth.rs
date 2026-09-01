use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use axum_extra::extract::cookie::CookieJar;
use uuid::Uuid;

use crate::{config::Config, utils::jwt::verify_token};

/// Represents an authenticated user injected into route handlers.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub email: String,
    pub role: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Retrieve Config from extensions which was injected during app configuration
        let config = parts
            .extensions
            .get::<Config>()
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Config extension not found"))?;

        // Extract cookie jar from request headers
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get("access_token")
            .map(|cookie| cookie.value())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing access token"))?;

        // Verify the token
        let claims = verify_token(token, &config.jwt_secret)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired access token"))?;

        Ok(Self {
            id: claims.sub,
            email: claims.email,
            role: claims.role,
        })
    }
}
