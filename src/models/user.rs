use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Enumeration of roles a user can have in the system.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
pub enum UserRole {
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "student")]
    Student,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Admin => "admin",
            UserRole::Student => "student",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "admin" => UserRole::Admin,
            _ => UserRole::Student,
        }
    }
}

/// Representation of a User database row.
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub role: String, // Stored as a VARCHAR in DB
    pub created_at: DateTime<Utc>,
}

/// DTO for returning non-sensitive user profile information.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

impl UserResponse {
    /// Maps a database User object to a safe UserResponse DTO.
    pub fn from_user(user: &User) -> Self {
        Self {
            id: user.id,
            email: user.email.clone(),
            role: user.role.clone(),
            created_at: user.created_at,
        }
    }
}
