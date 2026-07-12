//! Out-of-process integrations behind small traits so tests can stub them
//! (`reverse_geocoder`), plus small in-process utilities that don't fit the
//! `auth/` or `db/` layers (`timezone`).

pub mod legacy_backfill;
pub mod reverse_geocoder;
pub mod timezone;
pub mod utc_offset;
