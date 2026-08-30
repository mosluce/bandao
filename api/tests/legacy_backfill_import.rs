//! End-to-end coverage for the `legacy_backfill` example script's import
//! pipeline. `examples/` binaries aren't directly invokable from `cargo
//! test`, so these tests drive the same library functions the example's
//! `main()` calls (`route_action`, `build_checkin_event`,
//! `build_location_ping`, `legacy_query_filter`, and the repositories'
//! `upsert_legacy` methods) via a local `run_import` helper that mirrors
//! the script's loop. See `api/examples/legacy_backfill.rs`.

mod common;

use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use bandao_api::domain::AppUserCheckinStatus;
use bandao_api::services::checkin_status::reconcile_app_user;
use bandao_api::services::legacy_backfill::{
    LegacyCheckinDoc, RoutedAction, RunSummary, build_checkin_event, build_identity_map,
    build_location_ping, legacy_query_filter, route_action,
};
use bson::doc;
use bson::oid::ObjectId;
use common::{TestApp, current_org_id};
use mongodb::Collection;

/// Mirrors `legacy_backfill.rs`'s main loop against a fixture collection.
async fn run_import(
    app: &TestApp,
    legacy_coll: &Collection<LegacyCheckinDoc>,
    org_id: ObjectId,
    legacy_domain: ObjectId,
    since_days: u64,
    dry_run: bool,
) -> RunSummary {
    let db = app.db();
    let app_users = db
        .app_users
        .list_by_org(org_id)
        .await
        .expect("list app users");
    let identity_to_app_user = build_identity_map(app_users);
    let known_identities: Vec<String> = identity_to_app_user.keys().cloned().collect();

    let since = bson::DateTime::from_system_time(
        SystemTime::now() - Duration::from_secs(since_days.saturating_mul(24 * 3600)),
    );
    let filter = legacy_query_filter(legacy_domain, since, &known_identities);
    let mut cursor = legacy_coll
        .find(filter)
        .await
        .expect("query legacy fixture");

    let mut summary = RunSummary::default();
    let mut touched_app_users: HashSet<ObjectId> = HashSet::new();
    while cursor.advance().await.expect("advance cursor") {
        let doc: LegacyCheckinDoc = cursor
            .deserialize_current()
            .expect("deserialize legacy doc");
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
                touched_app_users.insert(app_user_id);
                if !dry_run {
                    let event = build_checkin_event(&doc, org_id, app_user_id, event_type);
                    db.checkin_events
                        .upsert_legacy(&event)
                        .await
                        .expect("upsert checkin event");
                }
            }
            RoutedAction::Path => {
                summary.location_pings += 1;
                if !dry_run {
                    let ping = build_location_ping(&doc, org_id, app_user_id);
                    db.location_pings
                        .upsert_legacy(&ping)
                        .await
                        .expect("upsert location ping");
                }
            }
            RoutedAction::Unrecognized => {
                summary.skipped_unrecognized_action += 1;
            }
        }
    }
    if !dry_run {
        for app_user_id in &touched_app_users {
            reconcile_app_user(&db, *app_user_id, org_id)
                .await
                .expect("reconcile checkin_user_status");
        }
    }
    summary
}

fn legacy_doc(
    action: &str,
    username: &str,
    domain: ObjectId,
    at: bson::DateTime,
) -> bson::Document {
    doc! {
        "_id": ObjectId::new(),
        "action": action,
        "at": at,
        "domain": domain,
        "signer": { "displayName": "Test User", "username": username },
        "comment": "office",
        "geo": { "lat": 22.588, "lng": 120.362 },
        "address": "高雄市鳳山區頂庄路",
    }
}

/// Mirrors real documents observed in KLCC's legacy `sbsigns` collection:
/// `signer` is present, but has no `username` sub-field at all (not even
/// `null`) — e.g. system-generated `路徑` pings with no resolved identity.
fn legacy_doc_no_username(action: &str, domain: ObjectId, at: bson::DateTime) -> bson::Document {
    doc! {
        "_id": ObjectId::new(),
        "action": action,
        "at": at,
        "domain": domain,
        "signer": { "displayName": "System" },
        "comment": "",
        "geo": { "lat": 22.588, "lng": 120.362 },
        "address": "高雄市鳳山區頂庄路",
    }
}

/// Fresh legacy fixture collection per test, isolated by database name
/// within the same test-container Mongo client.
fn legacy_fixture(
    app: &TestApp,
    db_name: &str,
) -> (Collection<LegacyCheckinDoc>, Collection<bson::Document>) {
    let db = app.db().database.client().database(db_name);
    (
        db.collection::<LegacyCheckinDoc>("checkin_events"),
        db.collection::<bson::Document>("checkin_events"),
    )
}

#[tokio::test]
async fn external_auth_shadow_users_match_by_external_key() {
    // Mirrors real KLCC: an external-auth AppUser has no `username` at all,
    // only `external_key` (the ERP account number, `USERNO`) — the legacy
    // system's `signer.username` was itself populated from that same value.
    let app = TestApp::spawn().await;
    let (_, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id = ObjectId::parse_str(current_org_id(&body)).unwrap();
    let shadow_user = app
        .db()
        .app_users
        .upsert_shadow(org_id, "mosluce", "陳聖夫")
        .await
        .expect("seed external-auth shadow AppUser");
    assert!(shadow_user.username.is_none());

    let legacy_domain = ObjectId::new();
    let (typed_coll, raw_coll) = legacy_fixture(&app, "legacy_fixture_external_auth");
    let now = bson::DateTime::now();
    raw_coll
        .insert_one(legacy_doc("上班", "mosluce", legacy_domain, now))
        .await
        .expect("seed clock_in for shadow user");

    let summary = run_import(&app, &typed_coll, org_id, legacy_domain, 365, true).await;

    assert_eq!(
        summary.clock_in, 1,
        "signer.username matching an AppUser's external_key should import"
    );
    assert_eq!(summary.skipped_unmatched_username, 0);
}

#[tokio::test]
async fn documents_missing_signer_username_are_excluded_by_the_identity_scoped_query() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id = ObjectId::parse_str(current_org_id(&body)).unwrap();
    let _ = app.create_app_user(&admin, "fang", "張正芳").await;

    let legacy_domain = ObjectId::new();
    let (typed_coll, raw_coll) = legacy_fixture(&app, "legacy_fixture_no_username");
    let now = bson::DateTime::now();
    raw_coll
        .insert_one(legacy_doc("上班", "fang", legacy_domain, now))
        .await
        .expect("seed matched clock_in");
    raw_coll
        .insert_one(legacy_doc_no_username("路徑", legacy_domain, now))
        .await
        .expect("seed signer-less path ping");

    let summary = run_import(&app, &typed_coll, org_id, legacy_domain, 365, true).await;

    assert_eq!(summary.clock_in, 1, "the matched document still imports");
    assert_eq!(
        summary.location_pings, 0,
        "the signer-less document does not import as a location ping"
    );
    // The query's `signer.username: { $in: [...] }` clause can't match a
    // document with no `signer.username` at all, so it's excluded before
    // it's ever fetched — it never reaches (and so never increments) the
    // client-side unmatched-username counter either. `LegacyCheckinDoc`'s
    // tolerance for a missing `signer.username` (see
    // `services::legacy_backfill::tests::deserialize_tolerates_missing_signer_username`)
    // is defense-in-depth for a query that doesn't filter this way.
    assert_eq!(summary.skipped_unmatched_username, 0);
}

#[tokio::test]
async fn dry_run_computes_summary_without_writing() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id = ObjectId::parse_str(current_org_id(&body)).unwrap();
    let _ = app.create_app_user(&admin, "fang", "張正芳").await;

    let legacy_domain = ObjectId::new();
    let (typed_coll, raw_coll) = legacy_fixture(&app, "legacy_fixture_dry_run");
    let now = bson::DateTime::now();
    raw_coll
        .insert_one(legacy_doc("上班", "fang", legacy_domain, now))
        .await
        .expect("seed clock_in");
    raw_coll
        .insert_one(legacy_doc("路徑", "fang", legacy_domain, now))
        .await
        .expect("seed path");

    let summary = run_import(&app, &typed_coll, org_id, legacy_domain, 365, true).await;

    assert_eq!(summary.clock_in, 1);
    assert_eq!(summary.location_pings, 1);
    assert_eq!(summary.total_imported(), 2);

    let checkin_count = app
        .db()
        .database
        .collection::<bson::Document>("checkin_events")
        .count_documents(doc! {})
        .await
        .expect("count checkin_events");
    assert_eq!(checkin_count, 0, "dry-run must not write checkin_events");
    let ping_count = app
        .db()
        .database
        .collection::<bson::Document>("location_pings")
        .count_documents(doc! {})
        .await
        .expect("count location_pings");
    assert_eq!(ping_count, 0, "dry-run must not write location_pings");
}

#[tokio::test]
async fn real_run_routes_skips_and_reruns_are_idempotent() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id = ObjectId::parse_str(current_org_id(&body)).unwrap();
    let create_body = app.create_app_user(&admin, "fang", "張正芳").await;
    let app_user_id = ObjectId::parse_str(create_body["user"]["id"].as_str().unwrap()).unwrap();

    let legacy_domain = ObjectId::new();
    let other_domain = ObjectId::new();
    let (typed_coll, raw_coll) = legacy_fixture(&app, "legacy_fixture_real_run");
    let now = bson::DateTime::now();

    raw_coll
        .insert_one(legacy_doc("上班", "fang", legacy_domain, now))
        .await
        .expect("seed clock_in");
    raw_coll
        .insert_one(legacy_doc("路徑", "fang", legacy_domain, now))
        .await
        .expect("seed path");
    raw_coll
        .insert_one(legacy_doc("午休", "fang", legacy_domain, now))
        .await
        .expect("seed unrecognized action");
    raw_coll
        .insert_one(legacy_doc("上班", "nobody", legacy_domain, now))
        .await
        .expect("seed unmatched username");
    raw_coll
        .insert_one(legacy_doc("上班", "fang", other_domain, now))
        .await
        .expect("seed other-domain record");

    let first_summary = run_import(&app, &typed_coll, org_id, legacy_domain, 365, false).await;
    assert_eq!(first_summary.clock_in, 1);
    assert_eq!(first_summary.location_pings, 1);
    assert_eq!(first_summary.skipped_unrecognized_action, 1);
    // The "nobody" record is excluded by the query's signer.username $in
    // filter before it's ever fetched — it never reaches the client-side
    // unmatched-username check, so that counter stays 0.
    assert_eq!(first_summary.skipped_unmatched_username, 0);

    let checkin_count = app
        .db()
        .database
        .collection::<bson::Document>("checkin_events")
        .count_documents(doc! { "app_user_id": app_user_id })
        .await
        .expect("count checkin_events");
    assert_eq!(
        checkin_count, 1,
        "only the in-domain, matched, recognized clock_in should be imported"
    );
    let ping_count = app
        .db()
        .database
        .collection::<bson::Document>("location_pings")
        .count_documents(doc! { "app_user_id": app_user_id })
        .await
        .expect("count location_pings");
    assert_eq!(ping_count, 1);

    // Re-run against the same fixture: counts are recomputed identically,
    // but no new documents are written (partial unique index on
    // legacy_source_id makes the upsert a no-op).
    let second_summary = run_import(&app, &typed_coll, org_id, legacy_domain, 365, false).await;
    assert_eq!(second_summary, first_summary);

    let checkin_count_after_rerun = app
        .db()
        .database
        .collection::<bson::Document>("checkin_events")
        .count_documents(doc! { "app_user_id": app_user_id })
        .await
        .expect("count checkin_events after rerun");
    assert_eq!(
        checkin_count_after_rerun, 1,
        "rerun must not duplicate rows"
    );
    let ping_count_after_rerun = app
        .db()
        .database
        .collection::<bson::Document>("location_pings")
        .count_documents(doc! { "app_user_id": app_user_id })
        .await
        .expect("count location_pings after rerun");
    assert_eq!(ping_count_after_rerun, 1, "rerun must not duplicate rows");

    // The import reconciles checkin_user_status itself: the AppUser was
    // created off_duty, the imported clock_in is now their latest event, and
    // no API restart happened in between.
    let status = app
        .db()
        .checkin_user_status
        .find(app_user_id)
        .await
        .expect("find status")
        .expect("status row exists");
    assert_eq!(status.status, AppUserCheckinStatus::OnSite);

    let latest = app
        .db()
        .checkin_events
        .latest_for_app_user(app_user_id)
        .await
        .expect("latest event")
        .expect("an event was imported");
    assert_eq!(
        status.last_event_id,
        Some(latest.id),
        "the projection must point at the imported event, not just match its status"
    );
    assert_eq!(
        status.current_shift_started_at,
        Some(latest.occurred_at_client),
        "an open shift is stamped from the event that opened it"
    );
}

#[tokio::test]
async fn since_days_window_excludes_records_older_than_the_cutoff() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id = ObjectId::parse_str(current_org_id(&body)).unwrap();
    let _ = app.create_app_user(&admin, "fang", "張正芳").await;

    let legacy_domain = ObjectId::new();
    let (typed_coll, raw_coll) = legacy_fixture(&app, "legacy_fixture_window");

    let recent =
        bson::DateTime::from_system_time(SystemTime::now() - Duration::from_secs(10 * 24 * 3600));
    let old =
        bson::DateTime::from_system_time(SystemTime::now() - Duration::from_secs(400 * 24 * 3600));
    raw_coll
        .insert_one(legacy_doc("上班", "fang", legacy_domain, recent))
        .await
        .expect("seed recent record");
    raw_coll
        .insert_one(legacy_doc("下班", "fang", legacy_domain, old))
        .await
        .expect("seed old record");

    let summary = run_import(&app, &typed_coll, org_id, legacy_domain, 365, true).await;

    assert_eq!(
        summary.clock_in, 1,
        "recent record is within the 365-day window"
    );
    assert_eq!(
        summary.clock_out, 0,
        "record older than the 365-day window must be excluded"
    );

    // Widening the window with an override picks up the old record too.
    let widened = run_import(&app, &typed_coll, org_id, legacy_domain, 500, true).await;
    assert_eq!(widened.clock_in, 1);
    assert_eq!(
        widened.clock_out, 1,
        "override widens the window to include it"
    );
}

/// Force `checkin_user_status` into a state that disagrees with the event
/// log, the way an appender that never touches the projection leaves it.
/// `last_event_id` is deliberately left pointing at a stale event.
async fn drift_status_to_on_site(
    app: &TestApp,
    app_user_id: ObjectId,
    stale_event_id: ObjectId,
    started_at: bson::DateTime,
) {
    app.db()
        .database
        .collection::<bson::Document>("checkin_user_status")
        .update_one(
            doc! { "_id": app_user_id },
            doc! { "$set": {
                "status": "on_site",
                "current_shift_started_at": started_at,
                "last_event_id": stale_event_id,
            }},
        )
        .await
        .expect("drift status row");
}

async fn raw_status(app: &TestApp, app_user_id: ObjectId) -> bson::Document {
    app.db()
        .database
        .collection::<bson::Document>("checkin_user_status")
        .find_one(doc! { "_id": app_user_id })
        .await
        .expect("read raw status row")
        .expect("status row exists")
}

#[tokio::test]
async fn a_rerun_over_already_imported_documents_still_repairs_drift() {
    // The case that motivated in-run reconciliation: by the time the fix
    // ships, the drifted AppUsers' recent events are already imported, so a
    // run that inserts nothing new must still correct the projection.
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id = ObjectId::parse_str(current_org_id(&body)).unwrap();
    let create_body = app.create_app_user(&admin, "fang", "張正芳").await;
    let app_user_id = ObjectId::parse_str(create_body["user"]["id"].as_str().unwrap()).unwrap();

    let legacy_domain = ObjectId::new();
    let (typed_coll, raw_coll) = legacy_fixture(&app, "legacy_fixture_rerun_drift");
    let clock_in_at =
        bson::DateTime::from_system_time(SystemTime::now() - Duration::from_secs(9 * 3600));
    let clock_out_at =
        bson::DateTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
    raw_coll
        .insert_one(legacy_doc("上班", "fang", legacy_domain, clock_in_at))
        .await
        .expect("seed clock_in");
    raw_coll
        .insert_one(legacy_doc("下班", "fang", legacy_domain, clock_out_at))
        .await
        .expect("seed clock_out");

    let first = run_import(&app, &typed_coll, org_id, legacy_domain, 365, false).await;
    assert_eq!(first.clock_in, 1);
    assert_eq!(first.clock_out, 1);

    // Rewind the projection to the frozen state the production board was
    // stuck in: on_site since the morning's clock_in, blind to the clock_out
    // that landed after it.
    let clock_in_event = app
        .db()
        .checkin_events
        .list_by_app_user_paginated(app_user_id, None, None, None, 50)
        .await
        .expect("list events")
        .into_iter()
        .find(|e| e.occurred_at_client == clock_in_at)
        .expect("the imported clock_in");
    drift_status_to_on_site(&app, app_user_id, clock_in_event.id, clock_in_at).await;

    // Every document in the window is already imported — this run inserts
    // nothing.
    let second = run_import(&app, &typed_coll, org_id, legacy_domain, 365, false).await;
    assert_eq!(second, first, "the rerun imports nothing new");

    let status = app
        .db()
        .checkin_user_status
        .find(app_user_id)
        .await
        .expect("find status")
        .expect("status row exists");
    assert_eq!(
        status.status,
        AppUserCheckinStatus::OffDuty,
        "the clock_out is the latest event, so the AppUser is off duty"
    );
    assert_eq!(
        status.current_shift_started_at, None,
        "clocking out clears the shift start"
    );
    let latest = app
        .db()
        .checkin_events
        .latest_for_app_user(app_user_id)
        .await
        .expect("latest event")
        .expect("events were imported");
    assert_eq!(status.last_event_id, Some(latest.id));
    assert_ne!(
        status.last_event_id,
        Some(clock_in_event.id),
        "the stale pointer must not survive"
    );
}

#[tokio::test]
async fn reconciliation_is_scoped_to_app_users_seen_in_the_run() {
    // Reconciliation re-seats rows destructively, so it must not reach
    // AppUsers the run had no checkin documents for.
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id = ObjectId::parse_str(current_org_id(&body)).unwrap();
    let ping_only_body = app.create_app_user(&admin, "fang", "張正芳").await;
    let ping_only_id = ObjectId::parse_str(ping_only_body["user"]["id"].as_str().unwrap()).unwrap();
    let absent_body = app.create_app_user(&admin, "ming", "王小明").await;
    let absent_id = ObjectId::parse_str(absent_body["user"]["id"].as_str().unwrap()).unwrap();

    let legacy_domain = ObjectId::new();
    let (typed_coll, raw_coll) = legacy_fixture(&app, "legacy_fixture_scope");
    let now = bson::DateTime::now();
    raw_coll
        .insert_one(legacy_doc("路徑", "fang", legacy_domain, now))
        .await
        .expect("seed path ping");

    // Both AppUsers are drifted; neither should be touched. Their rows have
    // no events behind them, so a reconciliation would reset them to
    // off_duty — staying on_site is the observable proof it didn't run.
    let stale_event_id = ObjectId::new();
    drift_status_to_on_site(&app, ping_only_id, stale_event_id, now).await;
    drift_status_to_on_site(&app, absent_id, stale_event_id, now).await;

    let summary = run_import(&app, &typed_coll, org_id, legacy_domain, 365, false).await;
    assert_eq!(summary.location_pings, 1);
    assert_eq!(
        summary.clock_in + summary.clock_out + summary.transfer_in + summary.transfer_out,
        0,
        "the window holds no checkin documents, only a location ping"
    );

    for (id, label) in [
        (ping_only_id, "an AppUser with only location_pings"),
        (absent_id, "an AppUser with no documents at all"),
    ] {
        let status = app
            .db()
            .checkin_user_status
            .find(id)
            .await
            .expect("find status")
            .expect("status row exists");
        assert_eq!(
            status.status,
            AppUserCheckinStatus::OnSite,
            "{label} must be left untouched by reconciliation"
        );
        assert_eq!(status.last_event_id, Some(stale_event_id));
    }
}

#[tokio::test]
async fn dry_run_does_not_reconcile_status() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id = ObjectId::parse_str(current_org_id(&body)).unwrap();
    let create_body = app.create_app_user(&admin, "fang", "張正芳").await;
    let app_user_id = ObjectId::parse_str(create_body["user"]["id"].as_str().unwrap()).unwrap();

    let legacy_domain = ObjectId::new();
    let (typed_coll, raw_coll) = legacy_fixture(&app, "legacy_fixture_dry_run_status");
    let clock_out_at =
        bson::DateTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
    raw_coll
        .insert_one(legacy_doc("下班", "fang", legacy_domain, clock_out_at))
        .await
        .expect("seed clock_out");

    // Import for real, then drift — a dry run over the same window now has
    // something it would visibly fix if it were allowed to write.
    run_import(&app, &typed_coll, org_id, legacy_domain, 365, false).await;
    let latest = app
        .db()
        .checkin_events
        .latest_for_app_user(app_user_id)
        .await
        .expect("latest event")
        .expect("the imported clock_out");
    drift_status_to_on_site(&app, app_user_id, ObjectId::new(), clock_out_at).await;
    let before = raw_status(&app, app_user_id).await;

    let summary = run_import(&app, &typed_coll, org_id, legacy_domain, 365, true).await;
    assert_eq!(summary.clock_out, 1, "the dry run does see the document");

    let after = raw_status(&app, app_user_id).await;
    assert_eq!(
        before, after,
        "a dry run must leave the status row byte-for-byte unchanged"
    );
    assert_ne!(
        after.get_object_id("last_event_id").ok(),
        Some(latest.id),
        "the drifted pointer is still drifted — nothing was reconciled"
    );
}
