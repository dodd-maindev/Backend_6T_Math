use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post, put},
    Extension, Router,
};
use sqlx::PgPool;
use tower_http::services::ServeDir;

use crate::{
    config::Config,
    handlers::{
        classroom::{create_classroom, get_my_classroom, list_classrooms, list_classroom_students},
        login::login,
        logout::logout,
        me::get_me,
        student::{create_student, update_student},
        assignment::{create_assignment, list_assignments, delete_assignment},
        question::{list_questions, add_question, delete_question},
        grading::{list_student_submissions, grade_submission, grade_full_exam},
        upload::{upload_question_images, list_my_uploads, get_upload_summary},
        grade_uploads::grade_student_uploads,
    },
};

/// Configures and returns the main application router with state and configurations.
pub fn create_router(pool: PgPool, config: Config) -> Router {
    let auth_routes = Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(get_me))
        .with_state(pool.clone());

    let student_routes = Router::new()
        .route("/my-classroom", get(get_my_classroom))
        .route("/upload", post(upload_question_images))
        .route("/uploads/:assignment_id", get(list_my_uploads))
        .with_state(pool.clone());

    let admin_routes = Router::new()
        .route("/classroom", post(create_classroom).get(list_classrooms))
        .route("/classroom/:id/students", get(list_classroom_students))
        .route("/student", post(create_student))
        .route("/student/:id", put(update_student))
        .route("/classroom/:id/assignment", post(create_assignment))
        .route("/classroom/:id/assignments", get(list_assignments))
        .route("/assignment/:id", delete(delete_assignment))
        .route("/assignment/:id/question", post(add_question))
        .route("/assignment/:id/questions", get(list_questions))
        .route("/assignment/:id/upload-summary", get(get_upload_summary))
        .route("/question/:id", delete(delete_question))
        .route("/student/submission", post(grade_submission))
        .route("/student/grade-full-exam", post(grade_full_exam))
        .route("/student/:id/submissions", get(list_student_submissions))
        .route("/grade-uploads", post(grade_student_uploads))
        .with_state(pool);

    Router::new()
        .nest("/api/auth", auth_routes)
        .nest("/api/student", student_routes)
        .nest("/api/admin", admin_routes)
        .nest_service("/uploads", ServeDir::new("./uploads"))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(Extension(config))
}
