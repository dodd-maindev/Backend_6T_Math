use axum::extract::Multipart;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use uuid::Uuid;
use crate::services::grading_service::StudentFilePayload;

/// Parsed form data from student submission multipart request.
pub struct ParsedSubmissionForm {
    pub student_id: Option<Uuid>,
    pub assignment_id: Option<Uuid>,
    pub question_number: Option<i32>,
    pub turnstile_token: Option<String>,
    pub student_files: Vec<StudentFilePayload>,
    pub file_urls: Vec<String>,
}

/// Parses multipart form data for student submissions (images, PDF, turnstile token).
pub async fn parse_submission_form(mut multipart: Multipart) -> Result<ParsedSubmissionForm, String> {
    let (mut student_id, mut assignment_id, mut question_number, mut turnstile_token) = (None, None, None, None);
    let (mut student_files, mut file_urls) = (Vec::new(), Vec::new());
    tokio::fs::create_dir_all("./uploads").await.map_err(|e| format!("Upload dir error: {}", e))?;

    while let Some(field) = multipart.next_field().await.map_err(|e| e.to_string())? {
        let name = field.name().unwrap_or("").to_string();
        if name == "student_id" {
            student_id = Uuid::parse_str(&field.text().await.unwrap_or_default()).ok();
        } else if name == "assignment_id" {
            assignment_id = Uuid::parse_str(&field.text().await.unwrap_or_default()).ok();
        } else if name == "question_number" {
            question_number = field.text().await.unwrap_or_default().parse::<i32>().ok();
        } else if name == "cf_turnstile_response" || name == "turnstile_token" {
            turnstile_token = Some(field.text().await.unwrap_or_default());
        } else if name == "image" || name == "file" || name == "images" || name == "pdf" || name == "files" {
            let content_type = field.content_type().unwrap_or("").to_string();
            let orig_name = field.file_name().unwrap_or("").to_lowercase();
            let is_pdf = content_type.contains("pdf") || orig_name.ends_with(".pdf");
            let is_png = content_type.contains("png") || orig_name.ends_with(".png");
            let ext = if is_pdf { "pdf" } else if is_png { "png" } else { "jpg" };
            let mime_type = if is_pdf { "application/pdf".to_string() } else if is_png { "image/png".to_string() } else { "image/jpeg".to_string() };
            let filename = format!("{}.{}", Uuid::new_v4(), ext);

            if let Ok(data) = field.bytes().await {
                if !data.is_empty() {
                    let _ = tokio::fs::write(format!("./uploads/{}", filename), &data).await;
                    file_urls.push(format!("/uploads/{}", filename));
                    student_files.push(StudentFilePayload { mime_type, base64_data: STANDARD.encode(&data) });
                }
            }
        }
    }

    Ok(ParsedSubmissionForm { student_id, assignment_id, question_number, turnstile_token, student_files, file_urls })
}
