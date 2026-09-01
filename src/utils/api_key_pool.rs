use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct KeySlot {
    key: String,
    minute_start: Instant,
    rpm_count: u32,
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
        let keys: Vec<String> = raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let now = Instant::now();
        let slots = keys.into_iter().map(|k| KeySlot { key: k, minute_start: now, rpm_count: 0, cooldown_until: None }).collect();
        Self { slots: Arc::new(Mutex::new(slots)), cursor: Arc::new(Mutex::new(0)) }
    }

    /// Selects the optimal key ensuring RPM stays < 14 with Round-Robin distribution.
    pub fn acquire_key(&self) -> Result<(usize, String), String> {
        let mut slots = self.slots.lock().unwrap();
        if slots.is_empty() { return Err("No Gemini API keys configured in GEMINI_API_KEYS".into()); }
        let now = Instant::now();
        let total = slots.len();

        for slot in slots.iter_mut() {
            if now.duration_since(slot.minute_start) >= Duration::from_secs(60) {
                slot.minute_start = now;
                slot.rpm_count = 0;
            }
        }

        let mut curr_cursor = self.cursor.lock().unwrap();
        let start_idx = *curr_cursor % total;

        for offset in 0..total {
            let idx = (start_idx + offset) % total;
            let slot = &mut slots[idx];
            let in_cooldown = slot.cooldown_until.map(|until| now < until).unwrap_or(false);

            if !in_cooldown && slot.rpm_count < 14 {
                slot.rpm_count += 1;
                *curr_cursor = (idx + 1) % total;
                let masked = format!("{}...{}", &slot.key[..6.min(slot.key.len())], &slot.key[slot.key.len().saturating_sub(4)..]);
                println!("[ApiKeyPool] Using Key #{} ({}) | RPM: {}/15", idx + 1, masked, slot.rpm_count);
                return Ok((idx, slot.key.clone()));
            }
        }

        // If all keys at limit (>= 14), select the key with lowest RPM
        let best_idx = (0..total).min_by_key(|&i| slots[i].rpm_count).unwrap_or(0);
        slots[best_idx].rpm_count += 1;
        *curr_cursor = (best_idx + 1) % total;
        println!("[ApiKeyPool Warning] All keys >= 14 RPM. Selecting Key #{} (RPM: {})", best_idx + 1, slots[best_idx].rpm_count);
        Ok((best_idx, slots[best_idx].key.clone()))
    }

    /// Flags key into 30s cooldown when receiving 429 quota exhaustion.
    pub fn mark_rate_limited(&self, idx: usize) {
        if let Ok(mut slots) = self.slots.lock() {
            if let Some(slot) = slots.get_mut(idx) {
                slot.cooldown_until = Some(Instant::now() + Duration::from_secs(30));
                println!("[ApiKeyPool Cooldown] Key #{} placed in 30s cooldown due to 429 Quota Limit.", idx + 1);
            }
        }
    }
}
