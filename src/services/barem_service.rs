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
        1. TRÍCH XUẤT BIỂU THỨC & ĐẲNG THỨC CHI TIẾT (TUYỆT ĐỐI KHÔNG GHI CHUNG CHUNG):\n\
           - BẮT BUỘC sao chép chính xác biểu thức toán học, phép tính, và nghiệm số trong ảnh vào step_title và criteria (đặt trong $...$).\n\
           - Ví dụ:\n\
             * Thay vì 'Câu a: Thay số' => 'Câu a: Thay đúng $x = 1$ ta có $B = \\frac{-1-2}{1-4}$'\n\
             * Thay vì 'Câu a: Tính toán' => 'Câu a: Tính đúng $B = \\frac{-3}{-3} = 1$'\n\
             * Thay vì 'Câu b: Đổi dấu' => 'Câu b: Đổi dấu đúng $A = \\frac{3x^2-4}{(x-2)(x+2)} - \\frac{2}{x+2} - \\frac{x}{x-2}$'\n\
             * Thay vì 'Câu b: Quy đồng' => 'Câu b: Quy đồng đúng $A = \\frac{3x^2-4}{(x-2)(x+2)} - \\frac{2(x-2)}{(x-2)(x+2)} - \\frac{x(x+2)}{(x-2)(x+2)}$'\n\
             * Thay vì 'Câu b: Phá ngoặc' => 'Câu b: Phá ngoặc đúng $A = \\frac{3x^2-4-2x+4-x^2-2x}{(x-2)(x+2)}$'\n\
             * Thay vì 'Câu b: Thu gọn' => 'Câu b: Thu gọn đúng $A = \\frac{2x^2-4x}{(x-2)(x+2)}$'\n\
             * Thay vì 'Câu b: Rút gọn' => 'Câu b: Đặt nhân tử chung và rút gọn ra $A = \\frac{2x}{x+2}$'\n\
        2. BÀI TOÀN PHẦN (ALL-OR-NOTHING): Nếu một câu (như Câu c) chỉ có 1 dòng chữ đỏ ở cuối dạng 'Suy luận chặt chẽ và xác định đúng hết mới chấm điểm (0,5 điểm)' => TẠO ĐÚNG 1 BƯỚC với max_score = 0.5đ, trích xuất đầy đủ phép biến đổi và nghiệm (ví dụ: 'Câu c: Tính $P = A.B = -2 - \\frac{8}{x-4}$, tìm $x \\in \\{5;6;8;12;3;2;0;-4\\} \\Rightarrow x = 12$').\n\
        3. CHÍNH XÁC ĐIỂM SỐ: max_score của mỗi bước BẮT BUỘC khớp 100% với con số mực đỏ (0.125, 0.25, 0.5, 0.75, 1.0,...).\n\
        4. criteria: Nêu cụ thể điều kiện và dòng biến đổi tương ứng.";

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
