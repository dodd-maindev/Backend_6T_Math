use std::collections::HashMap;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use crate::models::assignment::AssignmentQuestion;
use crate::services::score_sanitizer;
use crate::utils::gemini_client::GeminiClient;

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

    /// Transcribes full exam once across all pages with robust question number and contextual handwriting recognition.
    pub async fn transcribe_full_exam(&self, student_files: &[StudentFilePayload]) -> Result<HashMap<i32, String>, String> {
        let sys = "Bạn là chuyên gia OCR bài thi viết tay môn Toán. Nhiệm vụ: Đọc toàn bộ các trang bài thi viết tay từ trang 1 đến trang cuối một cách tỉ mỉ, trích xuất TRUNG THỰC 100% tất cả các bài (Bài 1, Bài 2, Bài 3, Bài 4, Bài 5, Bài 6, Bài 7...).\n\
        QUY TẮC:\n\
        1. BẮT BUỘC TRÍCH XUẤT ĐỦ TẤT CẢ CÁC BÀI có trên bài thi (Bài 1, Bài 2, Bài 3, Bài 4, Bài 5, Bài 6, Bài 7). Điền đúng số nguyên question_number (1, 2, 3, 4, 5, 6, 7).\n\
        2. HÌNH VẼ HÌNH HỌC: Nếu có hình vẽ, BẮT BUỘC ghi rõ 'Hình vẽ: Có vẽ hình đầy đủ các điểm và góc vuông'.\n\
        3. NHẬN DIỆN NÉT CHỮ THEO NGỮ CẢNH TOÁN HỌC: Nếu chữ cái (như F, E, H, D...) có nét sửa đè, hãy đối chiếu các dòng biến đổi tiếp theo (ví dụ có tỉ lệ BE/BC = BH/BF và tích BF.BE = BC.BH) để nhận diện đúng đỉnh tam giác là BHF và BEC, tránh đọc nhầm thành BHE.\n\
        4. TỪNG CÂU CON (a, b, c...): Trích xuất chi tiết biểu thức, biến đổi, kết quả. Đặt ký hiệu toán học trong $...$.\n\
        5. BỎ TRỐNG: Nếu học sinh không làm bài nào thì ghi 'Học sinh không làm bài này'.";

        let mut parts: Vec<Value> = Vec::new();
        for (idx, file) in student_files.iter().enumerate() {
            parts.push(json!({"text": format!("Trang bài thi ({}/{}):", idx + 1, student_files.len())}));
            parts.push(json!({"inlineData": {"mimeType": file.mime_type, "data": file.base64_data}}));
        }

        let res = self.client.transcribe_full_exam(sys, parts).await?;
        let mut map = HashMap::new();
        if let Some(arr) = res.get("transcripts").and_then(|t| t.as_array()) {
            for item in arr {
                let qn = Self::parse_question_num(item);
                let work = item.get("student_work").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if qn > 0 && !work.is_empty() { map.insert(qn, work); }
            }
        }
        Ok(map)
    }

    /// Robustly extracts integer question number from JSON item.
    fn parse_question_num(item: &Value) -> i32 {
        if let Some(n) = item.get("question_number").and_then(|v| v.as_i64()) { return n as i32; }
        if let Some(n) = item.get("question_number").and_then(|v| v.as_f64()) { return n as i32; }
        if let Some(s) = item.get("question_number").and_then(|v| v.as_str()) {
            let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<i32>() { return n; }
        }
        0
    }

    /// Grades a single question using pre-extracted transcript against teacher barem.
    pub async fn grade_question_with_transcript(&self, question: &AssignmentQuestion, transcript: &str) -> Result<Value, String> {
        let sys_prompt = self.build_grading_instruction(question.question_number).await;
        let mut parts: Vec<Value> = Vec::new();

        parts.push(json!({"text": format!("=== [BÀI LÀM VIẾT TAY THỰC TẾ CỦA HỌC SINH - BÀI {}] ===\n{}\n=============================================", question.question_number, transcript)}));
        parts.push(json!({"text": format!("=== [CHUẨN MỰC GIÁO VIÊN BÀI SỐ {} (TỔNG: {} ĐIỂM)] ===", question.question_number, question.max_score)}));
        self.append_question_assets(&mut parts, question).await;

        let mut feedback = self.client.evaluate_submission(&sys_prompt, parts).await?;
        feedback["student_work_transcript"] = json!(transcript);
        let q_max = question.max_score.to_string().parse::<f64>().unwrap_or(0.0);
        score_sanitizer::sanitize_scores(&mut feedback, q_max);
        feedback["question_number"] = json!(question.question_number);
        Ok(feedback)
    }

    /// Grades a single question on-demand (fallback or single submission).
    pub async fn grade_question(&self, question: &AssignmentQuestion, student_files: &[StudentFilePayload], is_targeted_scan: bool) -> Result<Value, String> {
        let scan_note = if is_targeted_scan { "Ảnh chụp riêng bài này." } else { "Tìm đúng phần viết tay của Bài này." };
        let sys = format!("Bạn là chuyên gia OCR bài thi viết tay môn Toán. Đọc và trích xuất TRUNG THỰC 100% tất cả những gì học sinh ĐÃ VIẾT TAY cho Bài {} ({scan_note}). Nhận diện chữ viết theo ngữ cảnh hình học (ví dụ BHF và BEC). Ghi rõ hình vẽ nếu có, từng câu (a, b, c...), các phép biến đổi, công thức, số liệu, nghiệm số thực tế. Nếu câu nào học sinh KHÔNG LÀM hoặc DỪNG LẠI DỞ DANG, hãy ghi rõ 'Học sinh chỉ viết... rồi dừng lại, chưa làm xong'.", question.question_number);
        let mut parts: Vec<Value> = Vec::new();
        for (idx, file) in student_files.iter().enumerate() {
            parts.push(json!({"text": format!("Trang ({}/{}):", idx + 1, student_files.len())}));
            parts.push(json!({"inlineData": {"mimeType": file.mime_type, "data": file.base64_data}}));
        }
        let transcript = self.client.transcribe_student_work(&sys, parts).await.unwrap_or_else(|_| "Học sinh không làm bài này.".to_string());
        self.grade_question_with_transcript(question, &transcript).await
    }

    /// Builds the system instruction from template for barem evaluation.
    async fn build_grading_instruction(&self, q_num: i32) -> String {
        let template = tokio::fs::read_to_string("prompts/grading_system_prompt.txt").await.unwrap_or_default();
        let prompt = template.replace("{QUESTION_NUMBER}", &q_num.to_string()).replace("{SCAN_NOTE}", "");
        format!("Bạn là Giám khảo CLB 6T MATH. Chấm Bài {}.{}", q_num, prompt)
    }

    /// Appends question and solution images as inline data parts.
    async fn append_question_assets(&self, parts: &mut Vec<Value>, question: &AssignmentQuestion) {
        if let Some(Value::Array(arr)) = &question.question_image_urls {
            for v in arr {
                if let Some(s) = v.as_str() {
                    if let Ok(bytes) = tokio::fs::read(format!(".{}", s)).await {
                        parts.push(json!({"text": format!("Ảnh Đề bài in Bài {}:", question.question_number)}));
                        parts.push(json!({"inlineData": {"mimeType": "image/jpeg", "data": STANDARD.encode(&bytes)}}));
                    }
                }
            }
        }
        let mut sol_urls = Vec::new();
        if let Some(Value::Array(arr)) = &question.solution_image_urls {
            for v in arr { if let Some(s) = v.as_str() { sol_urls.push(s.to_string()); } }
        }
        if sol_urls.is_empty() && !question.reference_image_url.is_empty() {
            sol_urls.push(question.reference_image_url.clone());
        }
        let note = question.native_prompt.as_deref().unwrap_or("Chuẩn");
        parts.push(json!({"text": format!("Ảnh Đáp án & Barem chuẩn Bài {} (Lưu ý: {}):", question.question_number, note)}));
        for url in &sol_urls {
            if let Ok(bytes) = tokio::fs::read(format!(".{}", url)).await {
                parts.push(json!({"inlineData": {"mimeType": "image/jpeg", "data": STANDARD.encode(&bytes)}}));
            }
        }
    }
}
