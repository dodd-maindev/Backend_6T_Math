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

    /// Transcribes the full exam once across all pages to guarantee no missed questions (e.g., 4b, 4c, geometry drawings).
    pub async fn transcribe_full_exam(&self, student_files: &[StudentFilePayload]) -> Result<HashMap<i32, String>, String> {
        let sys = "Bạn là chuyên gia OCR bài thi viết tay môn Toán. Nhiệm vụ: Đọc toàn bộ các trang bài thi viết tay từ trang 1 đến trang cuối. Với mỗi Bài (Bài 1, Bài 2, Bài 3, Bài 4, Bài 5, Bài 6, Bài 7...), trích xuất TRUNG THỰC 100% tất cả những gì học sinh đã làm (bao gồm tất cả các ý a, b, c..., hình vẽ, các phép biến đổi, công thức, số liệu, nghiệm số thực tế). Nếu học sinh không làm ý nào, ghi rõ 'Học sinh không làm ý...'. TUYỆT ĐỐI KHÔNG tự giải hộ.";
        let mut parts: Vec<Value> = Vec::new();
        for (idx, file) in student_files.iter().enumerate() {
            parts.push(json!({"text": format!("Trang bài thi học sinh ({}/{}):", idx + 1, student_files.len())}));
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
        let sys = format!("Bạn là chuyên gia OCR bài thi viết tay môn Toán. Đọc và trích xuất TRUNG THỰC 100% tất cả những gì học sinh ĐÃ VIẾT TAY cho Bài {} ({scan_note}). Ghi rõ từng câu (a, b, c...), các phép biến đổi, công thức, số liệu, nghiệm số thực tế. Nếu câu nào học sinh KHÔNG LÀM hoặc DỪNG LẠI DỞ DANG, hãy ghi rõ 'Học sinh chỉ viết... rồi dừng lại, chưa làm xong'.", question.question_number);
        let mut parts: Vec<Value> = Vec::new();
        for (idx, file) in student_files.iter().enumerate() {
            parts.push(json!({"text": format!("Trang ({}/{}):", idx + 1, student_files.len())}));
            parts.push(json!({"inlineData": {"mimeType": file.mime_type, "data": file.base64_data}}));
        }
        let transcript = self.client.transcribe_student_work(&sys, parts).await.unwrap_or_else(|_| "Không thể trích xuất".to_string());
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
