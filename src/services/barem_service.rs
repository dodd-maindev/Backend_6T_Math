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
        1. 100% CÁC BƯỚC BẮT BUỘC CÓ TIỀN TỐ CÂU ('Câu a:', 'Câu b:', 'Câu c:'):\n\
           - TUYỆT ĐỐI KHÔNG để bất kỳ bước nào trơ trọi thiếu tiền tố 'Câu a:', 'Câu b:', 'Câu c:'.\n\
           - ĐẶC BIỆT VỚI BÀI HÌNH HỌC (Ví dụ Bài 5, Bài 6):\n\
             * Bước vẽ hình: BẮT BUỘC ghi 'Câu a: Vẽ hình đúng' (hoặc gắn vào Câu a).\n\
             * Mọi bước nhỏ thuộc ý 1/ý a (chứng minh góc vuông, tam giác đồng dạng, tỉ số, tích cạnh): TẤT CẢ ĐỀU PHẢI CÓ TIỀN TỐ 'Câu a:' (Ví dụ: 'Câu a: Chứng minh $\\widehat{BAC} = \\widehat{BHA} = 90^\\circ$', 'Câu a: Chứng minh $\\Delta ABC \\sim \\Delta HBA$', 'Câu a: Lập tỉ số $\\frac{BA}{BH} = \\frac{BC}{BA}$', 'Câu a: Kết luận $BA^2 = BH.BC$').\n\
             * Mọi bước nhỏ thuộc ý 2/ý b: TẤT CẢ ĐỀU PHẢI CÓ TIỀN TỐ 'Câu b:' (Ví dụ: 'Câu b: Chứng minh $\\widehat{BEC} = \\widehat{BHF} = 90^\\circ$', 'Câu b: Chứng minh $\\Delta BEC \\sim \\Delta BHF$', 'Câu b: Lập tỉ số $\\frac{BE}{BH} = \\frac{BC}{BF}$', 'Câu b: Kết luận $BE.BF = BH.BC$').\n\
             * Mọi bước thuộc ý 3/ý c: TẤT CẢ ĐỀU PHẢI CÓ TIỀN TỐ 'Câu c:'.\n\
        2. TRÍCH XUẤT BIỂU THỨC & KÝ HIỆU TOÁN HỌC CHI TIẾT (ĐẶT TRONG $...$):\n\
           - Sao chép chính xác biểu thức toán học, góc $\\widehat{...}$, tam giác $\\Delta...$, tỉ số $\\frac{...}{...}$, hệ thức vào step_title và criteria.\n\
        3. BÀI TOÀN PHẦN (ALL-OR-NOTHING): Nếu một câu (như Câu c) chỉ có 1 dòng chữ đỏ ở cuối dạng 'Suy luận chặt chẽ và xác định đúng hết mới chấm điểm' => Tạo 1 bước duy nhất với max_score ghi trong ngoặc.\n\
        4. max_score: Khớp chính xác 100% với con số mực đỏ (0.125, 0.25, 0.5, 0.75, 1.0,...).";

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
