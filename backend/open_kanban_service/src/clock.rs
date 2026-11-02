use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

/// Current UTC time as an RFC 3339 string used by the database.
pub fn now_rfc3339() -> String {
    format_utc(OffsetDateTime::now_utc())
}

pub fn now_plus_seconds_rfc3339(seconds: u64) -> String {
    format_utc(OffsetDateTime::now_utc() + Duration::seconds(seconds as i64))
}

pub fn is_in_past(value: &str) -> bool {
    OffsetDateTime::parse(value, &Rfc3339)
        .map(|moment| moment <= OffsetDateTime::now_utc())
        .unwrap_or(true)
}

fn format_utc(moment: OffsetDateTime) -> String {
    moment
        .format(&Rfc3339)
        .expect("RFC 3339 formatting never fails for a valid timestamp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_parseable_and_expiry_is_detected() {
        let now = OffsetDateTime::parse(&now_rfc3339(), &Rfc3339).expect("RFC 3339 timestamp");
        let drift = (OffsetDateTime::now_utc() - now).whole_seconds().abs();
        assert!(drift <= 1, "stored timestamp drifted by {drift}s");

        let later = now_plus_seconds_rfc3339(60);
        assert!(!is_in_past(&later));
        assert!(is_in_past("2000-01-01T00:00:00Z"));
    }

    #[test]
    fn malformed_timestamps_are_treated_as_expired() {
        assert!(is_in_past("not-a-timestamp"));
        assert!(is_in_past("2000-01-01T00:00:00Z"));
    }
}
