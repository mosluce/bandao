//! Parses a caller-supplied UTC offset (`+HH:MM` / `-HH:MM`, no IANA names —
//! this is a fixed numeric offset, not a timezone) and computes the UTC
//! day-window boundaries for a given calendar date at that offset. Backs the
//! `checkin-export-zhengdan` capability's day-window query — the server
//! always computes "today" from its own UTC clock, shifted by the caller's
//! offset; it never trusts a caller-supplied date for the default case. See
//! `openspec/specs/checkin-export-zhengdan/spec.md`.

use bson::DateTime as BsonDateTime;
use time::{Date, Duration as TimeDuration, Month, OffsetDateTime, UtcOffset};

use crate::error::ApiError;

/// Parses `+HH:MM` / `-HH:MM` into a `time::UtcOffset`. Anything else
/// (missing sign, wrong width, out-of-range hours/minutes, IANA names) is
/// rejected.
pub fn parse_offset(raw: &str) -> Result<UtcOffset, ApiError> {
    let bytes = raw.as_bytes();
    if bytes.len() != 6 || bytes[3] != b':' {
        return Err(invalid_offset(raw));
    }
    let sign: i8 = match bytes[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return Err(invalid_offset(raw)),
    };
    let hours: i8 = raw[1..3].parse().map_err(|_| invalid_offset(raw))?;
    let minutes: i8 = raw[4..6].parse().map_err(|_| invalid_offset(raw))?;
    if hours > 23 || minutes > 59 {
        return Err(invalid_offset(raw));
    }
    UtcOffset::from_hms(sign * hours, sign * minutes, 0).map_err(|_| invalid_offset(raw))
}

/// Parses `YYYY-MM-DD` into a `time::Date`. Deliberately hand-rolled rather
/// than pulling in `time`'s `macros` feature for a `format_description!` —
/// this shape is simple enough not to need it.
pub fn parse_date(raw: &str) -> Result<Date, ApiError> {
    let parts: Vec<&str> = raw.split('-').collect();
    let [y, m, d] = parts.as_slice() else {
        return Err(invalid_date(raw));
    };
    let year: i32 = y.parse().map_err(|_| invalid_date(raw))?;
    let month_num: u8 = m.parse().map_err(|_| invalid_date(raw))?;
    let day: u8 = d.parse().map_err(|_| invalid_date(raw))?;
    let month = Month::try_from(month_num).map_err(|_| invalid_date(raw))?;
    Date::from_calendar_date(year, month, day).map_err(|_| invalid_date(raw))
}

/// "Today" as observed at `offset`, using the server's current UTC clock.
/// Never trust a caller-supplied notion of the current date — this is what
/// backs the endpoint's default when no `date` query param is given.
pub fn today_at_offset(offset: UtcOffset) -> Date {
    OffsetDateTime::now_utc().to_offset(offset).date()
}

/// Given a calendar date and a UTC offset, computes the half-open
/// `[day_start, day_end)` window in UTC that corresponds to local
/// `[00:00, 24:00)` at that offset. E.g. `date` at `+08:00` maps to
/// `[UTC date-1 16:00, UTC date 16:00)`.
pub fn day_window_utc(date: Date, offset: UtcOffset) -> (BsonDateTime, BsonDateTime) {
    let local_midnight = date.midnight().assume_offset(offset);
    let start = local_midnight.to_offset(UtcOffset::UTC);
    let end = start + TimeDuration::days(1);
    (to_bson(start), to_bson(end))
}

fn to_bson(dt: OffsetDateTime) -> BsonDateTime {
    let millis = (dt.unix_timestamp_nanos() / 1_000_000) as i64;
    BsonDateTime::from_millis(millis)
}

fn invalid_offset(raw: &str) -> ApiError {
    ApiError::Validation(format!(
        "invalid utc_offset (expected +HH:MM / -HH:MM): `{raw}`"
    ))
}

fn invalid_date(raw: &str) -> ApiError {
    ApiError::Validation(format!("invalid date (expected YYYY-MM-DD): `{raw}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_offset_accepts_plus_and_minus() {
        let plus8 = parse_offset("+08:00").unwrap();
        assert_eq!(plus8.whole_hours(), 8);
        let minus5 = parse_offset("-05:00").unwrap();
        assert_eq!(minus5.whole_hours(), -5);
        let zero = parse_offset("+00:00").unwrap();
        assert_eq!(zero.whole_hours(), 0);
    }

    #[test]
    fn parse_offset_rejects_garbage() {
        for bad in [
            "8:00",
            "+8:00",
            "+08:0",
            "+24:00",
            "+08:60",
            "Asia/Taipei",
            "",
            "+08:00Z",
        ] {
            assert!(
                parse_offset(bad).is_err(),
                "expected `{bad}` to be rejected"
            );
        }
    }

    #[test]
    fn parse_date_accepts_valid_calendar_dates() {
        let d = parse_date("2026-07-10").unwrap();
        assert_eq!(d.year(), 2026);
        assert_eq!(d.month() as u8, 7);
        assert_eq!(d.day(), 10);
    }

    #[test]
    fn parse_date_rejects_garbage() {
        for bad in ["2026/07/10", "2026-13-01", "2026-02-30", "not-a-date", ""] {
            assert!(parse_date(bad).is_err(), "expected `{bad}` to be rejected");
        }
    }

    #[test]
    fn day_window_plus_8_maps_to_expected_utc_range() {
        let date = parse_date("2026-07-10").unwrap();
        let offset = parse_offset("+08:00").unwrap();
        let (start, end) = day_window_utc(date, offset);

        let expected_start = parse_date("2026-07-09")
            .unwrap()
            .with_hms(16, 0, 0)
            .unwrap()
            .assume_utc();
        let expected_end = date.with_hms(16, 0, 0).unwrap().assume_utc();

        assert_eq!(start, to_bson(expected_start));
        assert_eq!(end, to_bson(expected_end));
    }

    #[test]
    fn day_window_default_offset_is_plain_utc_day() {
        let date = parse_date("2026-07-10").unwrap();
        let offset = parse_offset("+00:00").unwrap();
        let (start, end) = day_window_utc(date, offset);

        assert_eq!(start, to_bson(date.midnight().assume_utc()));
        assert_eq!(
            end,
            to_bson(date.next_day().unwrap().midnight().assume_utc())
        );
    }
}
