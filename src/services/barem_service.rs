use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;
use crate::{models::assignment::AssignmentQuestion, utils::gemini_client::GeminiClient};

pub struct BaremService;

impl BaremService {
    /// Extracts canonical grading barem from teacher question and solution assets using Gemini.
    pub async fn extract_canonical_barem(client: &GeminiClient, q: &AssignmentQuestion) -> Result<Value, String> {
        let sys = "Bạn là chuyên gia sư phạm Toán CLB 6T MATH. Nhiệm vụ: Đọc ảnh Lời giải mẫu & Barem điểm của Giáo viên, trích xuất chính xác cấu trúc Barem chấm điểm chuẩn dạng JSON.\n\
        QUY TẮC BẮT BUỘC:\n\
        1. PHÂN CẤP CÂU CON: Mỗi mốc điểm mực đỏ (0.125đ, 0.25đ, 0.5đ, 1.0đ) là 1 bước trong mảng steps. step_title BẮT BUỘC bắt đầu bằng tiền tố câu tương ứng (Ví dụ: 'Câu a: Thay số đúng', 'Câu b: Đổi dấu đúng', 'Câu b: Quy đồng đúng',...).\n\
        2. BÀI TOÀN PHẦN (ALL-OR-NOTHING): Nếu một câu (như Câu c) chỉ có 1 dòng chữ đỏ ở cuối dạng 'Suy luận chặt chẽ và xác định đúng hết mới chấm điểm (0,5 điểm)' hoặc 'Đúng hết mới cho điểm' => TẠO ĐÚNG 1 BƯỚC TOÀN PHẦN với max_score ghi trong ngoặc (ví dụ 0.5đ), KHÔNG tự ý chia nhỏ điểm thành phần.\n\
        3. CHÍNH XÁC ĐIỂM SỐ: max_score của mỗi bước BẮT BUỘC khớp 100% với con số mực đỏ (0.125, 0.25, 0.5, 0.75, 1.0,...). Tổng max_score các steps PHẢI BẰNG ĐÚNG tổng điểm bài thi.\n\
        4. criteria: Nêu rõ tiêu chuẩn cần đạt cho bước đó (công thức, điều kiện, phép tính, nghiệm số).";

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
        if let Some(Value::Array(arr)) = &q.solution_image_urls {
            for v in arr {
                if let Some(s) = v.as_str() {
                    if let Ok(b) = tokio::fs::read(format!(".{}", s)).await {
                        parts.push(json!({"inlineData": {"mimeType": "image/jpeg", "data": STANDARD.encode(&b)}}));
                    }
                }
            }
        }
        if !q.reference_image_url.is_empty() {
            if let Ok(b) = tokio::fs::read(format!(".{}", q.reference_image_url)).await {
                parts.push(json!({"inlineData": {"mimeType": "image/jpeg", "data": STANDARD.encode(&b)}}));
            }
        }
    }
}
