use dotenvy::dotenv;
use std::env;

/// Configuration holder for the backend application loaded from environment variables.
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub access_token_expire_minutes: i64,
    pub refresh_token_expire_days: i64,
    pub port: u16,
    pub frontend_url: String,
}

impl Config {
    /// Loads configuration from the `.env` file and system environment variables.
    pub fn new() -> Self {
        dotenv().ok();

        let database_url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set in environmental variables");
        let jwt_secret = env::var("JWT_SECRET")
            .expect("JWT_SECRET must be set in environmental variables");
        
        let access_token_expire_minutes = env::var("ACCESS_TOKEN_EXPIRE_MINUTES")
            .unwrap_or_else(|_| "15".to_string())
            .parse::<i64>()
            .unwrap_or(15);
            
        let refresh_token_expire_days = env::var("REFRESH_TOKEN_EXPIRE_DAYS")
            .unwrap_or_else(|_| "7".to_string())
            .parse::<i64>()
            .unwrap_or(7);
            
        let port = env::var("PORT")
            .unwrap_or_else(|_| "8000".to_string())
            .parse::<u16>()
            .unwrap_or(8000);

        let frontend_url = env::var("FRONTEND_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());

        Self {
            database_url,
            jwt_secret,
            access_token_expire_minutes,
            refresh_token_expire_days,
            port,
            frontend_url,
        }
    }
}
