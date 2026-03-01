pub mod batch;
pub mod cron;
pub mod error;
pub mod schedule;

pub use batch::BatchConfig;
pub use error::ZorviaError;

/// Generate a unique ID using microsecond timestamp + random suffix
pub fn generate_id(prefix: &str, name: &str) -> String {
    use chrono::Utc;
    use rand::Rng;
    let micros = Utc::now().timestamp_micros();
    let random: u16 = rand::thread_rng().gen();
    format!(
        "{}-{}-{}-{}",
        prefix,
        name.to_lowercase().replace(' ', "-"),
        micros,
        random
    )
}

/// Format bytes into a human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const KI: u64 = 1024;
    const MI: u64 = KI * 1024;
    const GI: u64 = MI * 1024;
    const TI: u64 = GI * 1024;

    if bytes >= TI {
        format!("{:.2} TiB", bytes as f64 / TI as f64)
    } else if bytes >= GI {
        format!("{:.2} GiB", bytes as f64 / GI as f64)
    } else if bytes >= MI {
        format!("{:.2} MiB", bytes as f64 / MI as f64)
    } else if bytes >= KI {
        format!("{:.2} KiB", bytes as f64 / KI as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Convert a percentage (0.0-100.0) to u8 with saturation
pub fn percent_to_u8(pct: f64) -> u8 {
    if pct <= 0.0 {
        0
    } else if pct >= 100.0 {
        100
    } else {
        pct.round() as u8
    }
}
