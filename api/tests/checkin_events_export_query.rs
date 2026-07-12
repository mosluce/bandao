//! Direct coverage of `CheckinEventRepository::list_by_org_in_range_for_export`
//! (the query layer backing `checkin-export-zhengdan`), seeded via
//! `db.checkin_events.create` so events can carry arbitrary
//! `occurred_at_client` values and event types without going through the
//! state-machine-enforcing `/app/checkin/events` submit path.

mod common;

use bandao_api::domain::{
    CheckinEventType, EventInitiatorKind, EventLocation, EventSource, GeoPoint,
};
use bson::DateTime as BsonDateTime;
use bson::oid::ObjectId;
use common::TestApp;
use serde_json::Value;

fn loc() -> EventLocation {
    EventLocation {
        coordinates: GeoPoint {
            lat: 25.03,
            lng: 121.56,
        },
        accuracy_meters: None,
        region_name: None,
        manual_label: None,
    }
}

async fn seed(
    app: &TestApp,
    org_id: ObjectId,
    app_user_id: ObjectId,
    event_type: CheckinEventType,
    occurred_at_client: BsonDateTime,
) {
    app.db()
        .checkin_events
        .create(
            org_id,
            app_user_id,
            event_type,
            occurred_at_client,
            occurred_at_client,
            EventSource::App,
            EventInitiatorKind::AppUser,
            app_user_id,
            loc(),
            None,
        )
        .await
        .unwrap();
}

fn ms(millis: i64) -> BsonDateTime {
    BsonDateTime::from_millis(millis)
}

#[tokio::test]
async fn excludes_transfer_events_and_out_of_range_events() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;
    let created = app.create_app_user(&admin, "alice", "Alice").await;
    let app_user_id = ObjectId::parse_str(created["user"]["id"].as_str().unwrap()).unwrap();
    let org_id_body: Value = admin
        .get(app.url("/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let org_id = ObjectId::parse_str(org_id_body["current_org"]["id"].as_str().unwrap()).unwrap();

    let day_start = ms(1_000_000_000_000); // arbitrary fixed anchor
    let day_end = ms(1_000_000_000_000 + 86_400_000);

    // In range, correct types — should be included.
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockIn,
        ms(day_start.timestamp_millis() + 1000),
    )
    .await;
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockOut,
        ms(day_start.timestamp_millis() + 2000),
    )
    .await;

    // In range, wrong types — must be excluded.
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::TransferOut,
        ms(day_start.timestamp_millis() + 1500),
    )
    .await;
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::TransferIn,
        ms(day_start.timestamp_millis() + 1600),
    )
    .await;

    // Out of range (before day_start, and at/after day_end) — must be excluded.
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockIn,
        ms(day_start.timestamp_millis() - 1),
    )
    .await;
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockOut,
        day_end,
    )
    .await;

    let results = app
        .db()
        .checkin_events
        .list_by_org_in_range_for_export(org_id, day_start, day_end)
        .await
        .unwrap();

    assert_eq!(
        results.len(),
        2,
        "expected exactly the two in-range clock events"
    );
    assert!(results.iter().all(|e| matches!(
        e.event_type,
        CheckinEventType::ClockIn | CheckinEventType::ClockOut
    )));
    // Ascending order.
    assert!(results[0].occurred_at_client <= results[1].occurred_at_client);
}

#[tokio::test]
async fn half_open_range_includes_start_excludes_end() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;
    let created = app.create_app_user(&admin, "bob", "Bob").await;
    let app_user_id = ObjectId::parse_str(created["user"]["id"].as_str().unwrap()).unwrap();
    let org_id_body: Value = admin
        .get(app.url("/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let org_id = ObjectId::parse_str(org_id_body["current_org"]["id"].as_str().unwrap()).unwrap();

    let day_start = ms(2_000_000_000_000);
    let day_end = ms(2_000_000_000_000 + 86_400_000);

    // Exactly at day_start: included.
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockIn,
        day_start,
    )
    .await;
    // Exactly at day_end: excluded (half-open).
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockOut,
        day_end,
    )
    .await;

    let results = app
        .db()
        .checkin_events
        .list_by_org_in_range_for_export(org_id, day_start, day_end)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].occurred_at_client, day_start);
}
