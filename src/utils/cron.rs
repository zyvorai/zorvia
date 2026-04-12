// Cron expression parsing utilities

use chrono::{DateTime, Datelike, NaiveTime, Timelike, Utc};

/// Find the next run time matching a cron expression (minute hour day month weekday).
/// Supports: numbers, `*`, `*/N` step, `N,N,...` lists, and `N-N` ranges.
pub fn next_cron_time(from: DateTime<Utc>, expression: &str) -> Option<DateTime<Utc>> {
    let fields: Vec<&str> = expression.split_whitespace().collect();
    if fields.len() < 5 {
        return None;
    }

    let mut candidate = from + chrono::TimeDelta::minutes(1);
    // Zero out seconds
    candidate = candidate
        .date_naive()
        .and_time(NaiveTime::from_hms_opt(
            candidate.naive_utc().hour(),
            candidate.naive_utc().minute(),
            0,
        )?)
        .and_utc();

    // Try up to 366 days ahead
    for _ in 0..(366 * 24 * 60) {
        let min = candidate.naive_utc().minute();
        let hour = candidate.naive_utc().hour();
        let day = candidate.naive_utc().day();
        let month = candidate.naive_utc().month();
        let weekday = candidate.naive_utc().weekday().num_days_from_sunday(); // 0=Sun

        if field_matches(fields[0], min)
            && field_matches(fields[1], hour)
            && field_matches(fields[2], day)
            && field_matches(fields[3], month)
            && field_matches(fields[4], weekday)
        {
            return Some(candidate);
        }

        candidate += chrono::TimeDelta::minutes(1);
    }

    None
}

/// Check if a cron field matches a value. Supports `*`, `N`, `*/N`, `N-N/S`, `N,N,...`, and `N-N`
/// (including wrapping ranges like `5-1` for weekdays).
pub fn field_matches(field: &str, value: u32) -> bool {
    if field == "*" {
        return true;
    }

    for part in field.split(',') {
        let part = part.trim();

        // Handle step: either */N or N-N/N or N/N
        if let Some((range_part, step_str)) = part.split_once('/') {
            let step: u32 = match step_str.parse() {
                Ok(s) if s > 0 => s,
                _ => return false,
            };

            if range_part == "*" {
                if value % step == 0 {
                    return true;
                }
            } else if let Some((start_str, end_str)) = range_part.split_once('-') {
                let start: u32 = match start_str.parse() {
                    Ok(v) => v,
                    _ => return false,
                };
                let end: u32 = match end_str.parse() {
                    Ok(v) => v,
                    _ => return false,
                };
                // Check if value is in range with step
                if start <= end {
                    if value >= start && value <= end && (value - start) % step == 0 {
                        return true;
                    }
                } else {
                    // Wrapping range with step
                    if value >= start || value <= end {
                        let offset = if value >= start {
                            value - start
                        } else {
                            value + (60 - start) // approximate
                        };
                        if offset % step == 0 {
                            return true;
                        }
                    }
                }
            } else if let Ok(start) = range_part.parse::<u32>() {
                if value >= start && (value - start) % step == 0 {
                    return true;
                }
            }
        } else if let Some((start_str, end_str)) = part.split_once('-') {
            let start: u32 = match start_str.parse() {
                Ok(v) => v,
                _ => return false,
            };
            let end: u32 = match end_str.parse() {
                Ok(v) => v,
                _ => return false,
            };
            if start <= end {
                if value >= start && value <= end {
                    return true;
                }
            } else {
                // Wrapping range (e.g., 5-1 for Fri-Mon)
                if value >= start || value <= end {
                    return true;
                }
            }
        } else if let Ok(v) = part.parse::<u32>() {
            if v == value {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_matches_wildcard() {
        assert!(field_matches("*", 0));
        assert!(field_matches("*", 59));
    }

    #[test]
    fn test_field_matches_exact() {
        assert!(field_matches("5", 5));
        assert!(!field_matches("5", 6));
    }

    #[test]
    fn test_field_matches_step() {
        assert!(field_matches("*/5", 0));
        assert!(field_matches("*/5", 15));
        assert!(!field_matches("*/5", 3));
    }

    #[test]
    fn test_field_matches_range() {
        assert!(field_matches("1-5", 1));
        assert!(field_matches("1-5", 3));
        assert!(field_matches("1-5", 5));
        assert!(!field_matches("1-5", 0));
        assert!(!field_matches("1-5", 6));
    }

    #[test]
    fn test_field_matches_list() {
        assert!(field_matches("1,5,10", 1));
        assert!(field_matches("1,5,10", 5));
        assert!(field_matches("1,5,10", 10));
        assert!(!field_matches("1,5,10", 3));
    }

    #[test]
    fn test_next_cron_time_every_minute() {
        let now = Utc::now();
        let next = next_cron_time(now, "* * * * *");
        assert!(next.is_some());
        assert!(next.unwrap() > now);
    }

    #[test]
    fn test_next_cron_time_invalid() {
        let now = Utc::now();
        assert!(next_cron_time(now, "bad").is_none());
        assert!(next_cron_time(now, "").is_none());
    }
}
