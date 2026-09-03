use std::collections::HashMap;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use crate::{models::assignment::AssignmentQuestion, services::score_sanitizer, utils::gemini_client::GeminiClient};

#[derive(Clone, Debug)]
pub struct StudentFilePayload {
    pub mime_type: String,
    pub base64_data: String,
}

pub struct GradingService {
    client: GeminiClient,
}

impl GradingService {
    pub fn new() -> Result<Self, String> {
        Ok(Self { client: GeminiClient::from_env()? })
    }

    /// Transcribes full exam with multi-page stitching, handwriting correction and diagram detection.
    pub async fn transcribe_full_exam(&self, files: &[StudentFilePayload]) -> Result<HashMap<i32, String>, String> {
        let sys = "Chuyên gia OCR bài thi Toán. Đọc tỉ mỉ tất cả các trang, trích xuất 100% tất cả các bài.\n\
        QUY TẮC: 1. HÌNH VẼ: Nếu có hình vẽ hình học, BẮT BUỘC ghi 'Hình vẽ: Có vẽ hình đầy đủ các đỉnh và góc vuông'. 2. LIÊN TRANG: Nếu bài viết dở ở cuối trang trước và viết tiếp ở đầu trang sau (như câu b, c), BẮT BUỘC GHÉP NỐI vào cùng bài. 3. SỬA ĐÈ/HÌNH HỌC: Nhận diện đúng đỉnh F, tỉ số BF/BC và tích BE.BF = BH.BC, bỏ qua phần gạch xóa theo dòng biến đổi tiếp theo.";
        let mut parts = Vec::new();
        for (idx, file) in files.iter().enumerate() {
            parts.push(json!({"text": format!("Trang ({}/{}):", idx + 1, files.len())}));
            parts.push(json!({"inlineData": {"mimeType": file.mime_type, "data": file.base64_data}}));
        }
        let res = self.client.transcribe_full_exam(sys, parts).await?;
        let mut map = HashMap::new();
        if let Some(arr) = res.get("transcripts").and_then(|t| t.as_array()) {
            for item in arr {
                let qn = item.get("question_number").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let work = item.get("student_work").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if qn > 0 && !work.is_empty() { map.insert(qn, work); }
            }
        }
        Ok(map)
    }

    /// Grades question sending BOTH teacher assets AND actual student submission images.
    pub async fn grade_question_with_transcript(&self, q: &AssignmentQuestion, transcript: &str, files: &[StudentFilePayload]) -> Result<Value, String> {
        let template = tokio::fs::read_to_string("prompts/grading_system_prompt.txt").await.unwrap_or_default();
        let sys = format!("Bạn là Giám khảo CLB 6T MATH. Chấm Bài {}.{}", q.question_number, template.replace("{QUESTION_NUMBER}", &q.question_number.to_string()).replace("{SCAN_NOTE}", ""));
        let mut parts = Vec::new();

        parts.push(json!({"text": format!("=== [CHUẨN MỰC GIÁO VIÊN BÀI SỐ {} (TỔNG: {} ĐIỂM)] ===", q.question_number, q.max_score)}));
        self.append_question_assets(&mut parts, q).await;

        parts.push(json!({"text": format!("=== [ẢNH/FILE BÀI LÀM VIẾT TAY GỐC CỦA HỌC SINH - BÀI {} (QUAN SÁT HÌNH VẼ & NÉT CHỮ)] ===", q.question_number)}));
        for (idx, f) in files.iter().enumerate() {
            parts.push(json!({"text": format!("Trang ({}/{}):", idx + 1, files.len())}));
            parts.push(json!({"inlineData": {"mimeType": f.mime_type, "data": f.base64_data}}));
        }
        if !transcript.is_empty() {
            parts.push(json!({"text": format!("=== [TRANSCRIPT THAM KHẢO TỪ BÀI LÀM] ===\n{}\n=============================================", transcript)}));
        }

        let mut feedback = self.client.evaluate_submission(&sys, parts).await?;
        feedback["student_work_transcript"] = json!(transcript);
        let q_max = q.max_score.to_string().parse::<f64>().unwrap_or(0.0);
        score_sanitizer::sanitize_scores(&mut feedback, q_max);
        feedback["question_number"] = json!(q.question_number);
        Ok(feedback)
    }

    /// Grades single question on-demand passing student images directly into evaluation.
    pub async fn grade_question(&self, q: &AssignmentQuestion, files: &[StudentFilePayload], is_targeted: bool) -> Result<Value, String> {
        let note = if is_targeted { "Ảnh chụp riêng bài này." } else { "Tìm đúng phần viết tay bài này." };
        let sys = format!("Chuyên gia OCR bài thi Toán Bài {}. {note}\nQUY TẮC: Ghi rõ 'Hình vẽ' nếu có hình. Ghép nối liên trang.", q.question_number);
        let mut parts = Vec::new();
        for (idx, f) in files.iter().enumerate() {
            parts.push(json!({"text": format!("Trang ({}/{}):", idx + 1, files.len())}));
            parts.push(json!({"inlineData": {"mimeType": f.mime_type, "data": f.base64_data}}));
        }
        let transcript = self.client.transcribe_student_work(&sys, parts).await.unwrap_or_else(|_| "Học sinh không làm bài này.".to_string());
        self.grade_question_with_transcript(q, &transcript, files).await
    }

    async fn append_question_assets(&self, parts: &mut Vec<Value>, question: &AssignmentQuestion) {
        if let Some(barem) = &question.barem_json {
            parts.push(json!({"text": format!("=== [KHUNG BAREM ĐIỂM CHUẨN CỦA GIÁO VIÊN (BẮT BUỘC ĐIỀN ĐÚNG CÁC BƯỚC NÀY)] ===\n{}\n=============================================", serde_json::to_string_pretty(barem).unwrap_or_default())}));
        }
        let mut sol_urls = Vec::new();
        if let Some(Value::Array(arr)) = &question.solution_image_urls {
            for v in arr { if let Some(s) = v.as_str() { sol_urls.push(s.to_string()); } }
        }
        if sol_urls.is_empty() && !question.reference_image_url.is_empty() {
            sol_urls.push(question.reference_image_url.clone());
        }
        for url in &sol_urls {
            if let Ok(b) = tokio::fs::read(format!(".{}", url)).await {
                parts.push(json!({"inlineData": {"mimeType": "image/jpeg", "data": STANDARD.encode(&b)}}));
            }
        }
    }
}
