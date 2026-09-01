use bcrypt::{hash, verify, DEFAULT_COST};

/// Hashes a plain text password using bcrypt with standard cost.
pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    hash(password, DEFAULT_COST)
}

/// Verifies a plain text password against a bcrypt hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    verify(password, hash).unwrap_or(false)
}
