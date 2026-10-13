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

    /// Evaluates full exam in a single fast multimodal request directly from student images/PDF.
    pub async fn grade_full_exam_direct(&self, questions: &[AssignmentQuestion], files: &[StudentFilePayload]) -> Result<Vec<Value>, String> {
        let sys = tokio::fs::read_to_string("prompts/grading_system_prompt.txt").await.unwrap_or_default();
        let mut parts = Vec::new();
        let barem_summary = questions.iter().map(|q| json!({
            "question_number": q.question_number, "title": format!("Bài {}", q.question_number),
            "max_score": q.max_score, "barem": q.barem_json
        })).collect::<Vec<_>>();
        parts.push(json!({"text": format!("=== [KHUNG BAREM ĐỀ THI] ===\n{}\n============================", serde_json::to_string_pretty(&barem_summary).unwrap_or_default())}));

        for (idx, f) in files.iter().enumerate() {
            parts.push(json!({"text": format!("Trang ({}/{}):", idx + 1, files.len())}));
            parts.push(json!({"inlineData": {"mimeType": f.mime_type.clone(), "data": f.base64_data.clone()}}));
        }

        let res = self.client.evaluate_submission(&sys, parts).await?;
        let general_feedback = res.get("general_feedback").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let q_array = res.get("questions").and_then(|v| v.as_array()).cloned().unwrap_or_default();

        let mut results = Vec::new();
        for q in questions {
            let matched = q_array.iter().find(|item| {
                let t = item.get("question_title").and_then(|v| v.as_str()).unwrap_or("");
                t.contains(&format!("Bài {}", q.question_number)) || t.contains(&q.question_number.to_string())
            });
            let mut fb = matched.cloned().unwrap_or_else(|| json!({
                "question_title": format!("Bài {}", q.question_number), "allocated_score": 0.0,
                "max_score": q.max_score, "teacher_comment": "Học sinh không làm bài này.", "steps": []
            }));
            fb["question_number"] = json!(q.question_number);
            fb["general_feedback"] = json!(general_feedback);
            let q_max = q.max_score.to_string().parse::<f64>().unwrap_or(0.0);
            score_sanitizer::sanitize_scores(&mut fb, q_max);
            results.push(fb);
        }
        Ok(results)
    }

    /// Evaluates a single question on-demand with direct image inspection.
    pub async fn grade_question(&self, q: &AssignmentQuestion, files: &[StudentFilePayload], _targeted: bool) -> Result<Value, String> {
        let sys = tokio::fs::read_to_string("prompts/grading_system_prompt.txt").await.unwrap_or_default();
        let mut parts = Vec::new();
        parts.push(json!({"text": format!("=== [CHUẨN MỰC BÀI SỐ {} (TỔNG: {} ĐIỂM)] ===", q.question_number, q.max_score)}));
        self.append_question_assets(&mut parts, q).await;
        for (idx, f) in files.iter().enumerate() {
            parts.push(json!({"text": format!("Trang ({}/{}):", idx + 1, files.len())}));
            parts.push(json!({"inlineData": {"mimeType": f.mime_type.clone(), "data": f.base64_data.clone()}}));
        }
        let mut feedback = self.client.evaluate_submission(&sys, parts).await?;
        let q_max = q.max_score.to_string().parse::<f64>().unwrap_or(0.0);
        score_sanitizer::sanitize_scores(&mut feedback, q_max);
        feedback["question_number"] = json!(q.question_number);
        Ok(feedback)
    }

    async fn append_question_assets(&self, parts: &mut Vec<Value>, question: &AssignmentQuestion) {
        if let Some(barem) = &question.barem_json {
            parts.push(json!({"text": format!("=== [BAREM GIÁO VIÊN] ===\n{}\n========================", serde_json::to_string_pretty(barem).unwrap_or_default())}));
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
