use axum::http::{header, HeaderValue, Method};
use std::net::SocketAddr;
use tower_http::cors::{AllowOrigin, CorsLayer};

mod config;
mod db;
mod handlers;
mod middleware;
mod models;
mod routes;
mod services;
mod utils;

#[tokio::main]
async fn main() {
    let config = config::Config::new();

    // 1. Initialize DB and run migrations
    let pool = db::establish_connection(&config.database_url).await;
    db::run_migrations(&pool).await;

    // 2. Seed default administrator
    seed_admin(&pool).await;

    // 3. Configure CORS with cookie credentials support for localhost and production domains
    let mut origins = vec![
        "http://localhost:3000".parse::<HeaderValue>().unwrap(),
        "http://127.0.0.1:3000".parse::<HeaderValue>().unwrap(),
        "https://6tmath.io.vn".parse::<HeaderValue>().unwrap(),
        "https://www.6tmath.io.vn".parse::<HeaderValue>().unwrap(),
    ];

    if let Ok(env_frontend) = std::env::var("FRONTEND_URL") {
        for url in env_frontend.split(',') {
            if let Ok(val) = url.trim().parse::<HeaderValue>() {
                if !origins.contains(&val) { origins.push(val); }
            }
        }
    }

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .allow_credentials(true);

    // 4. Create App Router
    let app = routes::create_router(pool, config.clone()).layer(cors);

    // 5. Start Server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    println!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await
        .expect("Failed to bind port");
    axum::serve(listener, app).await.expect("Failed to start server");
}

/// Seeds administrator accounts if not present in the database.
async fn seed_admin(pool: &sqlx::PgPool) {
    let admins = [
        ("admin@6tmath.vn", "adminpassword"),
        ("hongxuan", "HongXuan@6TMath#2026!Secure"),
        ("hongxuan@6tmath.vn", "HongXuan@6TMath#2026!Secure"),
    ];

    for (user, pass) in admins {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
            .bind(user).fetch_one(pool).await.unwrap_or(false);
        if !exists {
            if let Ok(hash) = utils::hash::hash_password(pass) {
                let _ = sqlx::query("INSERT INTO users (email, password_hash, role) VALUES ($1, $2, 'admin')")
                    .bind(user).bind(hash).execute(pool).await;
                println!("Administrator user seeded: {}", user);
            }
        }
    }
}
