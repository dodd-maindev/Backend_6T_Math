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

/// Fixes contradictions: if step_desc says "sai"/"nhầm" but status is "Correct".
fn fix_status_consistency(steps: &mut [Value]) {
    let wrong_keywords = ["sai", "nhầm", "thiếu", "chưa", "không đúng", "bị trừ", "mất điểm", "không làm", "bỏ trống"];
    for s in steps.iter_mut() {
        let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let desc = s.get("step_desc").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
        if status == "Correct" && wrong_keywords.iter().any(|kw| desc.contains(kw)) {
            s["status"] = json!("Incorrect");
            s["allocated_score"] = json!(0.0);
        }
    }
}
