-- Student question-level image uploads for per-question grading workflow
CREATE TABLE IF NOT EXISTS student_question_uploads (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    assignment_id UUID NOT NULL REFERENCES assignments(id) ON DELETE CASCADE,
    question_number INT NOT NULL,
    image_urls JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT unique_student_question_upload UNIQUE (student_id, assignment_id, question_number)
);

CREATE INDEX IF NOT EXISTS idx_student_uploads_student ON student_question_uploads(student_id);
CREATE INDEX IF NOT EXISTS idx_student_uploads_assignment ON student_question_uploads(assignment_id);
