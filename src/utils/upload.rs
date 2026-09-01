use axum::{extract::Multipart, http::StatusCode};
use tokio::fs::{create_dir_all, File};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

/**
 * Parses a multipart request, extracts the uploaded image/file, 
 * saves it locally to './uploads', and returns its relative URI.
 */
pub async fn save_uploaded_file(mut multipart: Multipart) -> Result<String, (StatusCode, &'static str)> {
    create_dir_all("./uploads").await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create uploads directory"))?;

    while let Some(field) = multipart.next_field().await
        .map_err(|_| (StatusCode::BAD_REQUEST, "Multipart parsing error"))? 
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "image" || name == "file" {
            let content_type = field.content_type().unwrap_or("image/jpeg").to_string();
            let extension = if content_type.contains("png") { "png" } else { "jpg" };
            let filename = format!("{}.{}", Uuid::new_v4(), extension);
            let filepath = format!("./uploads/{}", filename);

            let data = field.bytes().await
                .map_err(|_| (StatusCode::BAD_REQUEST, "Failed to read file bytes"))?;

            let mut file = File::create(&filepath).await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create local file"))?;
            file.write_all(&data).await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write file to disk"))?;

            return Ok(format!("/uploads/{}", filename));
        }
    }

    Err((StatusCode::BAD_REQUEST, "No file uploaded in multipart request"))
}
