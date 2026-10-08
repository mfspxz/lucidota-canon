//! Timestamps: Rust-native, no subprocess. Howard Hinnant's civil-from-days.
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

pub fn iso8601(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let sod = (secs % 86400) as i64;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, sod / 3600, (sod % 3600) / 60, sod % 60
    )
}

pub fn now_iso() -> String {
    iso8601(now_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn epoch_zero() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
    }
    #[test]
    fn known_date() {
        assert_eq!(iso8601(1791478800), "2026-10-08T17:00:00Z");
    }
}
