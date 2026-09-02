use serde_json::{json, Value};

/// Validates and fixes AI-generated scores against the known question max_score.
pub fn sanitize_scores(feedback: &mut Value, q_max_f: f64) {
    let mut total_score = 0.0;
    if let Some(Value::Array(questions)) = feedback.get_mut("questions") {
        for q in questions {
            let mut q_alloc = 0.0;
            if let Some(Value::Array(steps)) = q.get_mut("steps") {
                fix_status_consistency(steps);
                for s in steps.iter_mut() {
                    let max = s.get("max_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let mut alloc = s.get("allocated_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    if alloc > max { alloc = max; }
                    if alloc < 0.0 { alloc = 0.0; }
                    s["allocated_score"] = json!(alloc);
                    q_alloc += alloc;
                }
            }
            if q_max_f > 0.0 && q_alloc > q_max_f {
                q_alloc = q_max_f;
            }
            q["allocated_score"] = json!(q_alloc);
            q["max_score"] = json!(q_max_f);
            total_score += q_alloc;
        }
    }
    feedback["score"] = json!(total_score);
}

/// Enforces strict consistency: Incorrect or error descriptions must have 0.0 score.
fn fix_status_consistency(steps: &mut [Value]) {
    let wrong_keywords = [
        "sai", "nhầm", "thiếu", "chưa", "không đúng", "bị trừ",
        "mất điểm", "không làm", "bỏ trống", "tính sai", "xác định sai", "chưa có"
    ];
    for s in steps.iter_mut() {
        let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let desc = s.get("step_desc").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();

        // 1. If status is Incorrect or Missing, allocated_score must strictly be 0.0
        if status == "Incorrect" || status == "Missing" {
            s["allocated_score"] = json!(0.0);
        }

        // 2. If description indicates an error or missing step, enforce status=Incorrect and score=0.0
        let has_error = wrong_keywords.iter().any(|kw| desc.contains(kw));
        if has_error && !desc.contains("không sai") && !desc.contains("chưa sai") {
            s["status"] = json!("Incorrect");
            s["allocated_score"] = json!(0.0);
        }
    }
}
