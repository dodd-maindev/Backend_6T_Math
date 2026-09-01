use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct SiteVerifyPayload<'a> {
    secret: &'a str,
    response: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    remoteip: Option<&'a str>,
}

#[derive(Deserialize, Debug)]
struct SiteVerifyResponse {
    success: bool,
    #[serde(rename = "error-codes")]
    error_codes: Option<Vec<String>>,
}

/// Verifies Cloudflare Turnstile token to protect grading API from automated bots.
pub async fn verify_turnstile_token(secret: &str, token: &str, remote_ip: Option<&str>) -> Result<(), String> {
    // If secret key is not set, skip verification for local test mode
    if secret.trim().is_empty() || secret == "dummy_secret_key" {
        return Ok(());
    }

    if token.trim().is_empty() {
        return Err("Vui lòng hoàn thành xác thực bảo vệ Cloudflare (Chống Bot).".to_string());
    }

    let client = Client::new();
    let payload = SiteVerifyPayload { secret, response: token, remoteip: remote_ip };

    let res = client
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&payload)
        .send()
        .await
        .map_err(|e| format!("Lỗi kết nối máy chủ Cloudflare: {}", e))?;

    let verify_res: SiteVerifyResponse = res
        .json()
        .await
        .map_err(|e| format!("Lỗi phân tích phản hồi Cloudflare: {}", e))?;

    if verify_res.success {
        Ok(())
    } else {
        let errs = verify_res.error_codes.unwrap_or_default().join(", ");
        Err(format!("Xác thực Cloudflare Bot Protection thất bại (Mã lỗi: {}). Vui lòng thử lại.", errs))
    }
}
