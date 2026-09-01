use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use crate::models::assignment::AssignmentQuestion;
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

    pub async fn grade_question(&self, question: &AssignmentQuestion, student_files: &[StudentFilePayload], is_targeted_scan: bool) -> Result<Value, String> {
        let sys_prompt = self.build_system_instruction(question.question_number, is_targeted_scan).await;
        let mut parts: Vec<Value> = Vec::new();

        parts.push(json!({"text": format!("=== [CHUẨN MỰC GIÁO VIÊN] BÀI SỐ {} (ĐIỂM TỐI ĐA: {} ĐIỂM) ===", question.question_number, question.max_score)}));
        self.append_question_assets(&mut parts, question).await;

        parts.push(json!({"text": "=== [BÀI LÀM CỦA HỌC SINH CẦN ĐÁNH GIÁ] ==="}));
        for (idx, file) in student_files.iter().enumerate() {
            parts.push(json!({"text": format!("Tài liệu bài làm học sinh ({}/{} - {}):", idx + 1, student_files.len(), file.mime_type)}));
            parts.push(json!({"inlineData": {"mimeType": file.mime_type, "data": file.base64_data}}));
        }

        let mut feedback = self.client.evaluate_submission(&sys_prompt, parts).await?;
        Self::sanitize_scores(&mut feedback);
        feedback["question_number"] = json!(question.question_number);
        Ok(feedback)
    }

    async fn build_system_instruction(&self, q_num: i32, is_targeted_scan: bool) -> String {
        let scan_note = if is_targeted_scan {
            format!("\nLƯU Ý QUAN TRỌNG: Hãy rà soát kỹ các trang ảnh để tìm đúng phần lời giải của Bài {} (học sinh có thể viết 'Bài {}', 'Câu {}', 'Câu I/II...', hoặc viết trực tiếp các phép tính của bài này). Đối chiếu từng ý với Barem để cho điểm chuẩn xác nhất.", q_num, q_num, q_num)
        } else {
            String::new()
        };

        if let Ok(tpl) = tokio::fs::read_to_string("prompts/grading_system_prompt.txt").await {
            return tpl.replace("{QUESTION_NUMBER}", &q_num.to_string()).replace("{SCAN_NOTE}", &scan_note);
        }
        format!("Bạn là Giám khảo CLB 6T MATH. Chấm Bài {}.{}", q_num, scan_note)
    }

    fn sanitize_scores(feedback: &mut Value) {
        let mut total_score = 0.0;
        if let Some(Value::Array(questions)) = feedback.get_mut("questions") {
            for q in questions {
                let mut q_alloc = 0.0;
                if let Some(Value::Array(steps)) = q.get_mut("steps") {
                    for s in steps {
                        let max = s.get("max_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let mut alloc = s.get("allocated_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        if alloc > max { alloc = max; }
                        if alloc < 0.0 { alloc = 0.0; }
                        s["allocated_score"] = json!(alloc);
                        q_alloc += alloc;
                    }
                }
                q["allocated_score"] = json!(q_alloc);
                total_score += q_alloc;
            }
        }
        feedback["score"] = json!(total_score);
    }

    async fn append_question_assets(&self, parts: &mut Vec<Value>, question: &AssignmentQuestion) {
        if let Some(Value::Array(arr)) = &question.question_image_urls {
            for v in arr {
                if let Some(s) = v.as_str() {
                    if let Ok(bytes) = tokio::fs::read(format!(".{}", s)).await {
                        parts.push(json!({"text": format!("Ảnh Đề bài {}:", question.question_number)}));
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
        parts.push(json!({"text": format!("Ảnh Đáp án mẫu & Thang điểm Bài {}:", question.question_number)}));
        for url in &sol_urls {
            if let Ok(bytes) = tokio::fs::read(format!(".{}", url)).await {
                parts.push(json!({"inlineData": {"mimeType": "image/jpeg", "data": STANDARD.encode(&bytes)}}));
            }
        }
    }
}
