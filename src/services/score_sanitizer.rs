use serde_json::{json, Value};

/// Validates and fixes AI-generated scores against the known question max_score.
pub fn sanitize_scores(feedback: &mut Value, q_max_f: f64) {
    let mut total_score = 0.0;
    if let Some(Value::Array(questions)) = feedback.get_mut("questions") {
        for q in questions {
            let mut q_alloc = 0.0;
            if let Some(Value::Array(steps)) = q.get_mut("steps") {
                scale_step_max_scores(steps, q_max_f);
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
            q["allocated_score"] = json!(q_alloc);
            q["max_score"] = json!(q_max_f);
            total_score += q_alloc;
        }
    }
    feedback["score"] = json!(total_score);
}

/// Scales step max_scores proportionally so their sum equals the DB max_score.
fn scale_step_max_scores(steps: &mut [Value], q_max: f64) {
    if q_max <= 0.0 || steps.is_empty() { return; }
    let ai_sum: f64 = steps.iter()
        .map(|s| s.get("max_score").and_then(|v| v.as_f64()).unwrap_or(0.0))
        .sum();
    if ai_sum <= 0.0 || (ai_sum - q_max).abs() < 0.001 { return; }
    let ratio = q_max / ai_sum;
    for s in steps.iter_mut() {
        let old_max = s.get("max_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let old_alloc = s.get("allocated_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        s["max_score"] = json!((old_max * ratio * 1000.0).round() / 1000.0);
        s["allocated_score"] = json!((old_alloc * ratio * 1000.0).round() / 1000.0);
    }
}

/// Fixes contradictions: if step_desc says "sai"/"nhầm" but status is "Correct".
fn fix_status_consistency(steps: &mut [Value]) {
    let wrong_keywords = ["sai", "nhầm", "thiếu", "chưa", "không đúng", "bị trừ"];
    for s in steps.iter_mut() {
        let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let desc = s.get("step_desc").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
        if status == "Correct" && wrong_keywords.iter().any(|kw| desc.contains(kw)) {
            s["status"] = json!("Incorrect");
            s["allocated_score"] = json!(0.0);
        }
    }
}
