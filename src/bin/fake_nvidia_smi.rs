use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let util = current_utilization();
    println!("{}", util);
}

fn current_utilization() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let t = now.as_secs_f64();
    let wave = (t * 0.7).sin() * 0.5 + 0.5;
    let jitter = ((t * 13.37).sin() * 0.5 + 0.5) * 6.0;
    let value = wave * 90.0 + 5.0 + jitter;

    value.round().clamp(0.0, 100.0) as u64
}
