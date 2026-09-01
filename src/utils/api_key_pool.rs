use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct KeySlot {
    key: String,
    minute_start: Instant,
    rpm_count: u32,
    day_start: Instant,
    rpd_count: u32,
    cooldown_until: Option<Instant>,
}

/// Thread-safe Smart Multi-Key Round-Robin Load Balancer for Gemini API keys.
#[derive(Clone, Debug)]
pub struct ApiKeyPool {
    slots: Arc<Mutex<Vec<KeySlot>>>,
    cursor: Arc<Mutex<usize>>,
}

impl ApiKeyPool {
    pub fn from_env() -> Self {
        let raw = std::env::var("GEMINI_API_KEYS")
            .or_else(|_| std::env::var("GEMINI_API_KEY"))
            .unwrap_or_default();

        let mut keys: Vec<String> = if raw.trim().starts_with('[') {
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
        };
        keys.retain(|k| !k.is_empty());

        let now = Instant::now();
        let slots = keys.into_iter().map(|k| KeySlot {
            key: k, minute_start: now, rpm_count: 0, day_start: now, rpd_count: 0, cooldown_until: None,
        }).collect();
        Self { slots: Arc::new(Mutex::new(slots)), cursor: Arc::new(Mutex::new(0)) }
    }

    /// Selects the optimal key ensuring RPM < 14 and RPD < 1450 with Round-Robin distribution.
    pub fn acquire_key(&self) -> Result<(usize, String), String> {
        let mut slots = self.slots.lock().unwrap();
        if slots.is_empty() { return Err("Chưa cấu hình GEMINI_API_KEYS trong file .env".into()); }
        let now = Instant::now();
        let total = slots.len();

        for slot in slots.iter_mut() {
            if now.duration_since(slot.minute_start) >= Duration::from_secs(60) {
                slot.minute_start = now;
                slot.rpm_count = 0;
            }
            if now.duration_since(slot.day_start) >= Duration::from_secs(86400) {
                slot.day_start = now;
                slot.rpd_count = 0;
            }
        }

        let mut curr_cursor = self.cursor.lock().unwrap();
        let start_idx = *curr_cursor % total;

        for offset in 0..total {
            let idx = (start_idx + offset) % total;
            let slot = &mut slots[idx];
            let in_cooldown = slot.cooldown_until.map(|until| now < until).unwrap_or(false);

            if !in_cooldown && slot.rpm_count < 14 && slot.rpd_count < 1450 {
                slot.rpm_count += 1;
                slot.rpd_count += 1;
                *curr_cursor = (idx + 1) % total;
                let masked = format!("{}...{}", &slot.key[..6.min(slot.key.len())], &slot.key[slot.key.len().saturating_sub(4)..]);
                println!("[ApiKeyPool] Điều phối Key #{}/{} ({}) | RPM: {}/15 | RPD: {}", idx + 1, total, masked, slot.rpm_count, slot.rpd_count);
                return Ok((idx, slot.key.clone()));
            }
        }

        // If all keys at limit, pick the one with lowest RPM
        let best_idx = (0..total).min_by_key(|&i| slots[i].rpm_count).unwrap_or(0);
        slots[best_idx].rpm_count += 1;
        slots[best_idx].rpd_count += 1;
        *curr_cursor = (best_idx + 1) % total;
        println!("[ApiKeyPool Warning] Tất cả các Key đều chạm ngưỡng. Sử dụng tạm Key #{}/{} (RPM: {})", best_idx + 1, total, slots[best_idx].rpm_count);
        Ok((best_idx, slots[best_idx].key.clone()))
    }

    /// Flags key into 30s cooldown when receiving 429 quota exhaustion.
    pub fn mark_rate_limited(&self, idx: usize) {
        if let Ok(mut slots) = self.slots.lock() {
            if let Some(slot) = slots.get_mut(idx) {
                slot.cooldown_until = Some(Instant::now() + Duration::from_secs(30));
                println!("[ApiKeyPool Cooldown] Key #{} tạm dừng 30s do chạm giới hạn 429 Quota từ Google.", idx + 1);
            }
        }
    }
}
