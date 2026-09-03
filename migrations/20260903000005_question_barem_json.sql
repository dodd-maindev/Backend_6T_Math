-- Add barem_json column to assignment_questions table for pre-compiled grading barems
ALTER TABLE assignment_questions 
ADD COLUMN IF NOT EXISTS barem_json JSONB;
