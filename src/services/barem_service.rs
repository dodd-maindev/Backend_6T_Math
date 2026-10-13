use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;
use crate::{models::assignment::AssignmentQuestion, utils::gemini_client::GeminiClient};

pub struct BaremService;

impl BaremService {
    /// Extracts canonical grading barem from teacher question and solution assets using Gemini.
    pub async fn extract_canonical_barem(client: &GeminiClient, q: &AssignmentQuestion) -> Result<Value, String> {
        let sys = "Bạn là chuyên gia sư phạm Toán CLB 6T MATH. Nhiệm vụ: Đọc ảnh Lời giải & Barem của Giáo viên, trích xuất cấu trúc Barem chấm điểm chuẩn dạng JSON.\n\
        QUY TẮC:\n\
        1. TIỀN TỐ CÂU: Mỗi bước cần ghi rõ thuộc câu nào ('Câu a:', 'Câu b:', 'Câu c:', 'Phần 1: Trắc nghiệm'...). Đối với vẽ hình: ghi 'Câu a: Vẽ hình đúng'.\n\
        2. KÝ HIỆU TOÁN: Biểu thức toán học, góc, tam giác, tỉ số đặt trong $...$.\n\
        3. TRẮC NGHIỆM: Nếu là đáp án trắc nghiệm, trích xuất đầy đủ danh sách đáp án chuẩn từng câu (ví dụ: 1-D, 2-B, 3-C, ..., 15-C) vào tiêu chí (criteria).\n\
        4. max_score: Khớp chính xác điểm từng bước theo Barem giáo viên (hoặc chia đều thang điểm đề ra).";

        let mut parts = Vec::new();
        parts.push(json!({"text": format!("Bài số: {} (Tổng điểm: {}đ)", q.question_number, q.max_score)}));
        if let Some(prompt) = &q.native_prompt {
            parts.push(json!({"text": format!("Ghi chú đặc thù của giáo viên: {}", prompt)}));
        }

        Self::append_images(&mut parts, q).await;
        client.extract_barem(sys, parts).await
    }

    /// Retrieves existing barem from DB or compiles it on demand.
    pub async fn get_or_compile_barem(pool: &PgPool, client: &GeminiClient, q: &AssignmentQuestion) -> Result<Value, String> {
        if let Some(barem) = &q.barem_json {
            if barem.get("steps").and_then(|s| s.as_array()).map_or(false, |a| !a.is_empty()) {
                return Ok(barem.clone());
            }
        }
        let extracted = Self::extract_canonical_barem(client, q).await?;
        let _ = sqlx::query("UPDATE assignment_questions SET barem_json = $1 WHERE id = $2")
            .bind(&extracted).bind(q.id).execute(pool).await;
        Ok(extracted)
    }

    /// Saves teacher-edited barem JSON directly to database.
    pub async fn save_barem(pool: &PgPool, question_id: Uuid, barem: &Value) -> Result<(), String> {
        sqlx::query("UPDATE assignment_questions SET barem_json = $1 WHERE id = $2")
            .bind(barem).bind(question_id).execute(pool).await
            .map_err(|e| format!("Failed to update barem: {}", e))?;
        Ok(())
    }

    async fn append_images(parts: &mut Vec<Value>, q: &AssignmentQuestion) {
        let mut seen = std::collections::HashSet::new();
        if let Some(Value::Array(arr)) = &q.solution_image_urls {
            for v in arr {
                if let Some(s) = v.as_str() {
                    if seen.insert(s.to_string()) {
                        if let Ok(b) = tokio::fs::read(format!(".{}", s)).await {
                            let mime = if s.ends_with(".png") { "image/png" } else { "image/jpeg" };
                            parts.push(json!({"inlineData": {"mimeType": mime, "data": STANDARD.encode(&b)}}));
                        }
                    }
                }
            }
        }
        if !q.reference_image_url.is_empty() && seen.insert(q.reference_image_url.clone()) {
            if let Ok(b) = tokio::fs::read(format!(".{}", q.reference_image_url)).await {
                let mime = if q.reference_image_url.ends_with(".png") { "image/png" } else { "image/jpeg" };
                parts.push(json!({"inlineData": {"mimeType": mime, "data": STANDARD.encode(&b)}}));
            }
        }
    }
}
