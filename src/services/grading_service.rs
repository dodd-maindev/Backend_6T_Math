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

    /// Grades a single question by sending student + teacher assets to Gemini.
    pub async fn grade_question(&self, question: &AssignmentQuestion, student_files: &[StudentFilePayload], is_targeted_scan: bool) -> Result<Value, String> {
        let sys_prompt = self.build_system_instruction(question.question_number, is_targeted_scan).await;
        let mut parts: Vec<Value> = Vec::new();

        parts.push(json!({"text": "=== [BÀI LÀM CỦA HỌC SINH CẦN ĐÁNH GIÁ] ==="}));
        for (idx, file) in student_files.iter().enumerate() {
            parts.push(json!({"text": format!("Tài liệu bài làm học sinh ({}/{} - {}):", idx + 1, student_files.len(), file.mime_type)}));
            parts.push(json!({"inlineData": {"mimeType": file.mime_type, "data": file.base64_data}}));
        }

        parts.push(json!({"text": format!("=== [CHUẨN MỰC GIÁO VIÊN] BÀI SỐ {} (ĐIỂM TỐI ĐA: {} ĐIỂM) ===", question.question_number, question.max_score)}));
        self.append_question_assets(&mut parts, question).await;

        let mut feedback = self.client.evaluate_submission(&sys_prompt, parts).await?;
        let q_max = question.max_score.to_string().parse::<f64>().unwrap_or(0.0);
        score_sanitizer::sanitize_scores(&mut feedback, q_max);
        feedback["question_number"] = json!(question.question_number);
        Ok(feedback)
    }

    /// Builds the system instruction from the prompt template file.
    async fn build_system_instruction(&self, q_num: i32, is_targeted_scan: bool) -> String {
        let scan_note = if is_targeted_scan {
            "\n\nLƯU Ý ĐẶC BIỆT: Bài làm học sinh được SCAN CHỤP RIÊNG TỪNG CÂU."
        } else {
            "\n\nLƯU Ý: Bài làm là ảnh TOÀN BỘ bài, hãy tìm đúng phần Bài tương ứng."
        };
        let template = tokio::fs::read_to_string("prompts/grading_system_prompt.txt").await.unwrap_or_default();
        let prompt = template.replace("{QUESTION_NUMBER}", &q_num.to_string()).replace("{SCAN_NOTE}", scan_note);
        format!("Bạn là Giám khảo CLB 6T MATH. Chấm Bài {}.{}", q_num, prompt)
    }

    /// Appends question and solution images as inline data parts.
    async fn append_question_assets(&self, parts: &mut Vec<Value>, question: &AssignmentQuestion) {
        if let Some(Value::Array(arr)) = &question.question_image_urls {
            for v in arr {
                if let Some(s) = v.as_str() {
                    if let Ok(bytes) = tokio::fs::read(format!(".{}", s)).await {
                        parts.push(json!({"text": format!("Ảnh Đề bài (có thể chứa thang điểm) {}:", question.question_number)}));
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
        parts.push(json!({"text": format!("Ảnh Đáp án mẫu & Thang điểm Bài {} (Lưu ý: {}):", question.question_number, note)}));
        for url in &sol_urls {
            if let Ok(bytes) = tokio::fs::read(format!(".{}", url)).await {
                parts.push(json!({"inlineData": {"mimeType": "image/jpeg", "data": STANDARD.encode(&bytes)}}));
            }
        }
    }
}
