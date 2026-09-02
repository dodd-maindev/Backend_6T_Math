use serde_json::{json, Value};

/// Builds payload for structured JSON grading evaluation.
pub fn build_grading_payload(sys: &str, parts: Vec<Value>) -> Value {
    json!({
        "systemInstruction": {"parts": [{"text": sys}]},
        "contents": [{"parts": parts}],
        "generationConfig": {
            "temperature": 0.0, "topP": 0.1, "seed": 42,
            "responseMimeType": "application/json",
            "responseSchema": {
                "type": "OBJECT",
                "properties": {
                    "student_work_transcript": {"type": "STRING"},
                    "score": {"type": "NUMBER"}, "general_feedback": {"type": "STRING"},
                    "questions": {
                        "type": "ARRAY",
                        "items": {
                            "type": "OBJECT",
                            "properties": {
                                "question_title": {"type": "STRING"}, "allocated_score": {"type": "NUMBER"},
                                "max_score": {"type": "NUMBER"}, "teacher_comment": {"type": "STRING"},
                                "steps": {
                                    "type": "ARRAY",
                                    "items": {
                                        "type": "OBJECT",
                                        "properties": {
                                            "step_desc": {"type": "STRING"}, "allocated_score": {"type": "NUMBER"},
                                            "max_score": {"type": "NUMBER"}, "status": {"type": "STRING", "enum": ["Correct", "Incorrect", "Missing"]}
                                        },
                                        "required": ["step_desc", "allocated_score", "max_score", "status"]
                                    }
                                }
                            },
                            "required": ["question_title", "allocated_score", "max_score", "teacher_comment", "steps"]
                        }
                    }
                },
                "required": ["student_work_transcript", "score", "general_feedback", "questions"]
            }
        }
    })
}

/// Builds payload for structured full exam transcription (Phase 1 Full Exam OCR).
pub fn build_full_exam_transcription_payload(sys: &str, parts: Vec<Value>) -> Value {
    json!({
        "systemInstruction": {"parts": [{"text": sys}]},
        "contents": [{"parts": parts}],
        "generationConfig": {
            "temperature": 0.0, "topP": 0.1, "seed": 42,
            "responseMimeType": "application/json",
            "responseSchema": {
                "type": "OBJECT",
                "properties": {
                    "transcripts": {
                        "type": "ARRAY",
                        "items": {
                            "type": "OBJECT",
                            "properties": {
                                "question_number": {"type": "INTEGER"},
                                "student_work": {"type": "STRING"}
                            },
                            "required": ["question_number", "student_work"]
                        }
                    }
                },
                "required": ["transcripts"]
            }
        }
    })
}

/// Builds payload for raw text transcription (Single question OCR).
pub fn build_transcription_payload(sys: &str, parts: Vec<Value>) -> Value {
    json!({
        "systemInstruction": {"parts": [{"text": sys}]},
        "contents": [{"parts": parts}],
        "generationConfig": { "temperature": 0.0, "topP": 0.1, "seed": 42 }
    })
}
