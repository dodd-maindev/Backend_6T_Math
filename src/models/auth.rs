use serde::{Deserialize, Serialize};

/// Request payload for logging into the platform.
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Request payload for self-registration (e.g., initial setup or fallback).
#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub role: Option<String>, // 'admin' or 'student'
}
