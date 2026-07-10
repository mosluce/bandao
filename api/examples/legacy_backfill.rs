//! One-off, developer-run import of a customer's legacy MongoDB check-in
//! history into `checkin_events` / `location_pings`. See
//! `openspec/changes/add-legacy-backfill-windows-and-pings/design.md` for
//! the full rationale.
//!
//! Usage:
//!
//! ```text
//! cargo run --example legacy_backfill -- \
//!     --org-id <bandao Org ObjectId hex> \
//!     --legacy-uri mongodb://user:pass@host:27017/legacy_db \
//!     --legacy-domain <legacy domain ObjectId hex> \
//!     [--legacy-collection checkin_events] \
//!     [--since-days 365] \
//!     [--dry-run]
//! ```
//!
//! `--legacy-collection` defaults to `checkin_events` but the actual name
//! varies per customer's legacy deployment (e.g. KLCC's is `sbsigns`) —
//! querying a collection that doesn't exist under that name silently
//! returns zero documents rather than erroring, so a dry-run reporting all
//! zeros is a sign to double-check this value against the customer's
//! MongoDB, not necessarily that there's no matching data.
//!
//! `--legacy-uri` / `--legacy-domain` / `--legacy-collection` may instead be
//! set via the `LEGACY_URI` / `LEGACY_DOMAIN` / `LEGACY_COLLECTION` env vars
//! (including via `.env`, loaded the same way as the API's `BANDAO_*` vars)
//! — handy so a customer's legacy connection string never has to be typed on
//! the command line / land in shell history. The CLI flag wins if both are
//! given.
//!
//! `bandao`'s own connection is read from the usual `BANDAO_MONGO_URI` /
//! `BANDAO_MONGO_DB` env vars (same as the API server). Safe to re-run: every
//! written row carries the legacy document's `_id` as `legacy_source_id`,
//! and both target collections have a partial unique index on that field —
//! re-processing the same legacy document is a no-op.

use std::process::ExitCode;
use std::time::{Duration, SystemTime};

use bandao_api::services::legacy_backfill::{
    LegacyCheckinDoc, RoutedAction, RunSummary, build_checkin_event, build_identity_map,
    build_location_ping, legacy_query_filter, route_action,
};
use bandao_api::{Config, Db};
use bson::oid::ObjectId;
use mongodb::Client as LegacyClient;

struct Args {
    org_id: ObjectId,
    legacy_uri: String,
    legacy_domain: ObjectId,
    legacy_collection: String,
    since_days: u64,
    dry_run: bool,
}

const DEFAULT_LEGACY_COLLECTION: &str = "checkin_events";

/// Cap on how many individual "failed to deserialize" lines get printed
/// per run — a legacy collection with widespread schema drift could
/// otherwise produce hundreds of thousands of near-identical lines.
const MAX_MALFORMED_WARNINGS: u64 = 5;

const USAGE: &str = "usage: legacy_backfill --org-id <id> --legacy-uri <uri> --legacy-domain <id> [--legacy-collection checkin_events] [--since-days 365] [--dry-run]";

fn parse_args() -> Result<Args, String> {
    let mut org_id_raw = None;
    let mut legacy_uri = None;
    let mut legacy_domain_raw = None;
    let mut legacy_collection = None;
    let mut since_days = 365u64;
    let mut dry_run = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--org-id" => org_id_raw = Some(args.next().ok_or("--org-id requires a value")?),
            "--legacy-uri" => {
                legacy_uri = Some(args.next().ok_or("--legacy-uri requires a value")?)
            }
            "--legacy-domain" => {
                legacy_domain_raw = Some(args.next().ok_or("--legacy-domain requires a value")?)
            }
            "--legacy-collection" => {
                legacy_collection = Some(args.next().ok_or("--legacy-collection requires a value")?)
            }
            "--since-days" => {
                let v = args.next().ok_or("--since-days requires a value")?;
                since_days = v
                    .parse()
                    .map_err(|_| format!("invalid --since-days value: `{v}`"))?;
            }
            "--dry-run" => dry_run = true,
            other => return Err(format!("unrecognized argument: `{other}`")),
        }
    }

    // CLI flag wins; fall back to env vars (including `.env`, already loaded
    // by `dotenvy::dotenv()` before this runs) so a legacy connection string
    // never has to be typed on the command line.
    let legacy_uri = legacy_uri.or_else(|| env_var("LEGACY_URI"));
    let legacy_domain_raw = legacy_domain_raw.or_else(|| env_var("LEGACY_DOMAIN"));
    let legacy_collection = legacy_collection
        .or_else(|| env_var("LEGACY_COLLECTION"))
        .unwrap_or_else(|| DEFAULT_LEGACY_COLLECTION.to_string());

    let org_id_raw = org_id_raw.ok_or("--org-id is required")?;
    let org_id = ObjectId::parse_str(&org_id_raw).map_err(|e| format!("invalid --org-id: {e}"))?;
    let legacy_uri = legacy_uri.ok_or("--legacy-uri is required (flag or LEGACY_URI env var)")?;
    let legacy_domain_raw =
        legacy_domain_raw.ok_or("--legacy-domain is required (flag or LEGACY_DOMAIN env var)")?;
    let legacy_domain = ObjectId::parse_str(&legacy_domain_raw)
        .map_err(|e| format!("invalid --legacy-domain: {e}"))?;

    Ok(Args {
        org_id,
        legacy_uri,
        legacy_domain,
        legacy_collection,
        since_days,
        dry_run,
    })
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

#[tokio::main]
async fn main() -> ExitCode {
    dotenvy::dotenv().ok();

    let args = match parse_args() {
        Ok(a) => a,
        Err(err) => {
            eprintln!("error: {err}");
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("error: failed to load bandao configuration: {err}");
            return ExitCode::from(1);
        }
    };

    let db = match Db::connect(&config.mongo_uri, &config.mongo_db).await {
        Ok(db) => db,
        Err(err) => {
            eprintln!("error: failed to connect to bandao MongoDB: {err}");
            return ExitCode::from(1);
        }
    };
    // Make sure the legacy_source_id partial unique indexes exist even if
    // this script is run against a database the API hasn't booted against
    // yet with this change's code.
    if let Err(err) = db.ensure_indexes().await {
        eprintln!("error: failed to ensure indexes: {err}");
        return ExitCode::from(1);
    }

    let legacy_client = match LegacyClient::with_uri_str(&args.legacy_uri).await {
        Ok(c) => c,
        Err(err) => {
            eprintln!("error: failed to connect to legacy MongoDB: {err}");
            return ExitCode::from(1);
        }
    };
    let legacy_db = match legacy_client.default_database() {
        Some(d) => d,
        None => {
            eprintln!("error: --legacy-uri must include a database name, e.g. mongodb://host/mydb");
            return ExitCode::from(2);
        }
    };
    let legacy_coll = legacy_db.collection::<LegacyCheckinDoc>(&args.legacy_collection);

    let app_users = match db.app_users.list_by_org(args.org_id).await {
        Ok(v) => v,
        Err(err) => {
            eprintln!("error: failed to load AppUsers for --org-id: {err}");
            return ExitCode::from(1);
        }
    };
    let identity_to_app_user = build_identity_map(app_users);
    if identity_to_app_user.is_empty() {
        eprintln!(
            "error: no AppUsers in --org-id have a username or external_key to match against — nothing to import"
        );
        return ExitCode::from(1);
    }
    let known_identities: Vec<String> = identity_to_app_user.keys().cloned().collect();

    let since = bson::DateTime::from_system_time(
        SystemTime::now() - Duration::from_secs(args.since_days.saturating_mul(24 * 3600)),
    );
    // Scoped to only the identities we actually have AppUsers for — the
    // legacy collection can be huge (KLCC's is ~978K documents) and the
    // overwhelming majority never belong to anyone onboarded into bandao.
    // Pushing this into the query (rather than fetching everything and
    // discarding client-side) matters especially because this script is
    // meant to be re-run repeatedly during cutover.
    let filter = legacy_query_filter(args.legacy_domain, since, &known_identities);

    let mut cursor = match legacy_coll.find(filter).await {
        Ok(c) => c,
        Err(err) => {
            eprintln!("error: failed to query legacy collection: {err}");
            return ExitCode::from(1);
        }
    };

    let mut summary = RunSummary::default();
    let mut newly_inserted = 0u64;
    let mut already_present = 0u64;

    loop {
        let advanced = match cursor.advance().await {
            Ok(a) => a,
            Err(err) => {
                eprintln!("error: cursor advance failed: {err}");
                return ExitCode::from(1);
            }
        };
        if !advanced {
            break;
        }
        let doc: LegacyCheckinDoc = match cursor.deserialize_current() {
            Ok(d) => d,
            Err(err) => {
                summary.skipped_malformed_document += 1;
                if summary.skipped_malformed_document <= MAX_MALFORMED_WARNINGS {
                    eprintln!("warning: skipping a document that failed to deserialize: {err}");
                } else if summary.skipped_malformed_document == MAX_MALFORMED_WARNINGS + 1 {
                    eprintln!(
                        "warning: further malformed-document warnings suppressed; see the final summary for the total count"
                    );
                }
                continue;
            }
        };

        // The query already scopes to `signer.username` in
        // `known_identities`, so this should always find a match — kept as
        // a defensive check (not a hot path) rather than an assumption.
        let Some(app_user_id) = doc
            .signer
            .username
            .as_deref()
            .and_then(|username| identity_to_app_user.get(username))
            .copied()
        else {
            summary.skipped_unmatched_username += 1;
            continue;
        };

        match route_action(&doc.action) {
            RoutedAction::Checkin(event_type) => {
                summary.record_checkin(event_type);
                if !args.dry_run {
                    let event = build_checkin_event(&doc, args.org_id, app_user_id, event_type);
                    match db.checkin_events.upsert_legacy(&event).await {
                        Ok(true) => newly_inserted += 1,
                        Ok(false) => already_present += 1,
                        Err(err) => {
                            eprintln!(
                                "warning: failed to upsert checkin_events for legacy _id {}: {err}",
                                doc.id
                            );
                        }
                    }
                }
            }
            RoutedAction::Path => {
                summary.location_pings += 1;
                if !args.dry_run {
                    let ping = build_location_ping(&doc, args.org_id, app_user_id);
                    match db.location_pings.upsert_legacy(&ping).await {
                        Ok(true) => newly_inserted += 1,
                        Ok(false) => already_present += 1,
                        Err(err) => {
                            eprintln!(
                                "warning: failed to upsert location_pings for legacy _id {}: {err}",
                                doc.id
                            );
                        }
                    }
                }
            }
            RoutedAction::Unrecognized => {
                summary.skipped_unrecognized_action += 1;
            }
        }
    }

    println!(
        "{} legacy_backfill run for org {} (collection `{}`, domain {}, since {} days ago{})",
        if args.dry_run { "DRY-RUN" } else { "REAL" },
        args.org_id,
        args.legacy_collection,
        args.legacy_domain,
        args.since_days,
        if args.dry_run {
            ", no writes performed"
        } else {
            ""
        },
    );
    println!("  clock_in:      {}", summary.clock_in);
    println!("  clock_out:     {}", summary.clock_out);
    println!("  transfer_out:  {}", summary.transfer_out);
    println!("  transfer_in:   {}", summary.transfer_in);
    println!("  location_pings:{}", summary.location_pings);
    println!("  total matched: {}", summary.total_imported());
    println!(
        "  skipped (unmatched username): {}",
        summary.skipped_unmatched_username
    );
    println!(
        "  skipped (unrecognized action): {}",
        summary.skipped_unrecognized_action
    );
    println!(
        "  skipped (malformed document): {}",
        summary.skipped_malformed_document
    );
    if !args.dry_run {
        println!("  newly inserted: {newly_inserted}");
        println!("  already present (re-run no-op): {already_present}");
        println!(
            "\nRestart the bandao-api process now so repair_checkin_status_drift reconciles checkin_user_status."
        );
    }

    ExitCode::SUCCESS
}
