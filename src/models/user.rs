use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Representation of a User database row with backward-compatible defaults.
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    #[sqlx(default)]
    pub full_name: Option<String>,
    #[sqlx(default)]
    pub address: Option<String>,
    #[sqlx(default)]
    pub school: Option<String>,
    #[sqlx(default)]
    pub phone_number: Option<String>,
}

/// DTO for returning non-sensitive user profile information.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub full_name: Option<String>,
    pub address: Option<String>,
    pub school: Option<String>,
    pub phone_number: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl UserResponse {
    pub fn from_user(user: &User) -> Self {
        Self {
            id: user.id,
            email: user.email.clone(),
            role: user.role.clone(),
            full_name: user.full_name.clone(),
            address: user.address.clone(),
            school: user.school.clone(),
            phone_number: user.phone_number.clone(),
            created_at: user.created_at,
        }
    }
}

/// DTO for admin updating student profile and resetting password.
#[derive(Debug, Deserialize)]
pub struct UpdateStudentRequest {
    pub full_name: Option<String>,
    pub address: Option<String>,
    pub school: Option<String>,
    pub phone_number: Option<String>,
    pub new_password: Option<String>,
}
