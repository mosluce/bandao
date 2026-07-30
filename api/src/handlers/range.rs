//! Shared `from` / `to` query-range parsing and validation.
//!
//! Four read surfaces take the same RFC3339 `from` / `to` pair and must
//! answer identically: the ping list (self + admin), the ping xlsx export,
//! and the checkin-event history (self + admin). They were duplicating the
//! parse-then-validate block, which is exactly the kind of thing that drifts
//! once one caller gets a new rule — so the rule lives here once.
//!
//! Every failure mode collapses to `INVALID_RANGE`: an unparseable bound is
//! not distinguished from an inverted or oversized one, because the client
//! fix is the same in all three cases.

use bson::DateTime;

use crate::error::{ApiError, ApiResult};
use crate::handlers::app_checkin::parse_rfc3339;

/// Cap on the `to - from` span of a single ranged query.
///
/// This is NOT a retention floor: `from` may reach arbitrarily far back.
/// The floor that used to exist was written hand-in-hand with
/// `location_pings`' old 90-day TTL (querying past it was pointless when
/// nothing that old could survive). The TTL is gone, and
/// `legacy_backfill`-imported pings and events are routinely older than 90
/// days — a floor would make that data permanently unreachable through
/// every read surface despite being safely stored. The span cap stays
/// because it bounds one query's result size, which is a real concern
/// independent of retention.
const RANGE_MAX_DAYS: i64 = 90;
const MILLIS_PER_DAY: i64 = 24 * 3600 * 1000;

/// Parse an optional `from` / `to` pair, validating the span when either
/// side is present. Absent sides skip their own check, so single-sided
/// ranges are allowed.
pub fn parse_optional_range(
    from: Option<&str>,
    to: Option<&str>,
) -> ApiResult<(Option<DateTime>, Option<DateTime>)> {
    let from = parse_bound(from)?;
    let to = parse_bound(to)?;
    if from.is_some() || to.is_some() {
        validate_range(from, to)?;
    }
    Ok((from, to))
}

/// Parse a `from` / `to` pair where both sides are mandatory (the export
/// surface). A missing side is `INVALID_RANGE`, same as an unparseable one.
pub fn parse_required_range(
    from: Option<&str>,
    to: Option<&str>,
) -> ApiResult<(DateTime, DateTime)> {
    let from = parse_bound(from)?.ok_or(ApiError::InvalidRange)?;
    let to = parse_bound(to)?.ok_or(ApiError::InvalidRange)?;
    validate_range(Some(from), Some(to))?;
    Ok((from, to))
}

fn parse_bound(raw: Option<&str>) -> ApiResult<Option<DateTime>> {
    match raw {
        Some(raw) => Ok(Some(
            parse_rfc3339(raw).map_err(|_| ApiError::InvalidRange)?,
        )),
        None => Ok(None),
    }
}

fn validate_range(from: Option<DateTime>, to: Option<DateTime>) -> ApiResult<()> {
    let span_max_millis = RANGE_MAX_DAYS * MILLIS_PER_DAY;
    if let (Some(f), Some(t)) = (from, to) {
        let from_ms = f.timestamp_millis();
        let to_ms = t.timestamp_millis();
        if to_ms < from_ms || to_ms - from_ms > span_max_millis {
            return Err(ApiError::InvalidRange);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = MILLIS_PER_DAY;

    fn at(millis: i64) -> String {
        DateTime::from_millis(millis)
            .try_to_rfc3339_string()
            .expect("rfc3339")
    }

    #[test]
    fn optional_range_accepts_both_absent() {
        let (from, to) = parse_optional_range(None, None).expect("no range");
        assert!(from.is_none() && to.is_none());
    }

    #[test]
    fn optional_range_accepts_single_sided() {
        let (from, to) = parse_optional_range(None, Some(&at(0))).expect("to only");
        assert!(from.is_none());
        assert!(to.is_some());
    }

    #[test]
    fn optional_range_accepts_span_at_the_cap() {
        let start = 1_700_000_000_000;
        parse_optional_range(Some(&at(start)), Some(&at(start + 90 * DAY_MS)))
            .expect("90 days is allowed");
    }

    #[test]
    fn optional_range_rejects_span_over_the_cap() {
        let start = 1_700_000_000_000;
        let err = parse_optional_range(Some(&at(start)), Some(&at(start + 91 * DAY_MS)));
        assert!(matches!(err, Err(ApiError::InvalidRange)));
    }

    #[test]
    fn optional_range_rejects_inverted() {
        let start = 1_700_000_000_000;
        let err = parse_optional_range(Some(&at(start)), Some(&at(start - DAY_MS)));
        assert!(matches!(err, Err(ApiError::InvalidRange)));
    }

    #[test]
    fn optional_range_rejects_unparseable() {
        let err = parse_optional_range(Some("not-a-timestamp"), None);
        assert!(matches!(err, Err(ApiError::InvalidRange)));
    }

    /// The floor removal is a deliberate contract, not an oversight — a
    /// bound far outside any retention window must survive validation so
    /// legacy-imported rows stay readable.
    #[test]
    fn optional_range_allows_bounds_far_in_the_past() {
        let long_ago = 1_000_000_000_000;
        parse_optional_range(Some(&at(long_ago)), Some(&at(long_ago + DAY_MS)))
            .expect("old range with a small span is allowed");
    }

    #[test]
    fn required_range_rejects_missing_side() {
        assert!(matches!(
            parse_required_range(None, Some(&at(0))),
            Err(ApiError::InvalidRange)
        ));
        assert!(matches!(
            parse_required_range(Some(&at(0)), None),
            Err(ApiError::InvalidRange)
        ));
    }

    #[test]
    fn required_range_returns_both_bounds() {
        let start = 1_700_000_000_000;
        let (from, to) =
            parse_required_range(Some(&at(start)), Some(&at(start + DAY_MS))).expect("valid");
        assert_eq!(from.timestamp_millis(), start);
        assert_eq!(to.timestamp_millis(), start + DAY_MS);
    }
}
