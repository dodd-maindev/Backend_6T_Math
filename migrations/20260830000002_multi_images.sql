-- Add support for multiple question images and solution images per question
ALTER TABLE assignment_questions 
    ADD COLUMN IF NOT EXISTS question_image_urls JSONB DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS solution_image_urls JSONB DEFAULT '[]'::jsonb;
