use axum::http::StatusCode;
use axum_extra::extract::cookie::{Cookie, CookieJar};

/// Handles user logout by clearing the JWT cookie.
pub async fn logout(jar: CookieJar) -> (CookieJar, StatusCode) {
    // Overwrite the cookie with an expired date to remove it
    let mut cookie = Cookie::new("access_token", "");
    cookie.set_path("/");
    cookie.set_max_age(cookie::time::Duration::ZERO);

    let updated_jar = jar.add(cookie);
    (updated_jar, StatusCode::OK)
}
