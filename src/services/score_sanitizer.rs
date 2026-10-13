use serde_json::{json, Value};

/// Validates and fixes AI-generated scores against the known question max_score.
pub fn sanitize_scores(feedback: &mut Value, q_max_f: f64) {
    if let Some(Value::Array(steps)) = feedback.get_mut("steps") {
        let mut q_alloc = 0.0;
        fix_status_consistency(steps);
        for s in steps.iter_mut() {
            let max = s.get("max_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let mut alloc = s.get("allocated_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            if max > 0.0 && alloc > max { alloc = max; }
            if alloc < 0.0 { alloc = 0.0; }
            s["allocated_score"] = json!(alloc);
            q_alloc += alloc;
        }
        if q_max_f > 0.0 && q_alloc > q_max_f { q_alloc = q_max_f; }
        feedback["allocated_score"] = json!(q_alloc);
        feedback["max_score"] = json!(q_max_f);
        feedback["score"] = json!(q_alloc);
        return;
    }

    let mut total_score = 0.0;
    if let Some(Value::Array(questions)) = feedback.get_mut("questions") {
        for q in questions {
            let mut q_alloc = 0.0;
            if let Some(Value::Array(steps)) = q.get_mut("steps") {
                fix_status_consistency(steps);
                for s in steps.iter_mut() {
                    let max = s.get("max_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let mut alloc = s.get("allocated_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    if max > 0.0 && alloc > max { alloc = max; }
                    if alloc < 0.0 { alloc = 0.0; }
                    s["allocated_score"] = json!(alloc);
                    q_alloc += alloc;
                }
            }
            if q_max_f > 0.0 && q_alloc > q_max_f { q_alloc = q_max_f; }
            q["allocated_score"] = json!(q_alloc);
            q["max_score"] = json!(q_max_f);
            total_score += q_alloc;
        }
    }
    feedback["score"] = json!(total_score);
}

/// Enforces score and status consistency without wiping partial points.
fn fix_status_consistency(steps: &mut [Value]) {
    for s in steps.iter_mut() {
        let alloc = s.get("allocated_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if alloc > 0.0 && status == "Missing" {
            s["status"] = json!("Correct");
        } else if alloc == 0.0 && status == "Correct" {
            s["status"] = json!("Incorrect");
        }
    }
}
