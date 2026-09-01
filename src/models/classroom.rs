use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Representation of a Classroom database row.
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Classroom {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// Representation of a Student-Classroom relation database row.
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct StudentClassroom {
    pub student_id: Uuid,
    pub classroom_id: Uuid,
    pub joined_at: DateTime<Utc>,
}

/// DTO for creating a new classroom.
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateClassroomRequest {
    pub name: String,
}

/// DTO for adding a new student to a classroom.
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateStudentRequest {
    pub email: String,
    pub password: String,
    pub classroom_id: Uuid,
}
