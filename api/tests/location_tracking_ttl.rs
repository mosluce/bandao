//! `location_pings` no longer carries a TTL index — see the
//! `location-tracking` spec's "Location pings are persisted with dual
//! timestamps" requirement (the 90-day TTL was removed in
//! `add-legacy-backfill-windows-and-pings` so legacy-imported historical
//! pings aren't deleted on arrival). This test asserts the TTL index is
//! absent, including on a database that predates this change (`ensure_indexes`
//! actively drops it — see `db/mod.rs`).

mod common;

use common::TestApp;

#[tokio::test]
async fn ttl_index_is_absent() {
    let app = TestApp::spawn().await;

    let coll = app
        .state
        .db
        .database
        .collection::<bson::Document>("location_pings");
    let mut cursor = coll.list_indexes().await.expect("list indexes");

    let mut found_ttl = false;
    while cursor.advance().await.expect("advance index cursor") {
        let raw = cursor.deserialize_current().expect("index spec");
        let bson_spec = bson::to_bson(&raw).expect("index to bson");
        let doc_spec = bson_spec.as_document().expect("index doc").clone();
        if doc_spec.get("name").and_then(|v| v.as_str()) == Some("location_pings_ttl") {
            found_ttl = true;
        }
    }

    assert!(
        !found_ttl,
        "location_pings_ttl index should have been removed"
    );
}

/// Simulates an old deployment that still has the pre-this-change TTL index
/// installed (created directly, bypassing `ensure_indexes`), then confirms a
/// fresh `ensure_indexes()` call drops it.
#[tokio::test]
async fn ensure_indexes_drops_a_pre_existing_ttl_index() {
    use mongodb::IndexModel;
    use mongodb::options::IndexOptions;
    use std::time::Duration;

    let app = TestApp::spawn().await;
    let coll = app
        .state
        .db
        .database
        .collection::<bson::Document>("location_pings");

    coll.create_index(
        IndexModel::builder()
            .keys(bson::doc! { "occurred_at_server": 1 })
            .options(
                IndexOptions::builder()
                    .expire_after(Duration::from_secs(90 * 24 * 3600))
                    .name("location_pings_ttl".to_string())
                    .build(),
            )
            .build(),
    )
    .await
    .expect("create legacy ttl index");

    app.state.db.ensure_indexes().await.expect("ensure indexes");

    let mut cursor = coll.list_indexes().await.expect("list indexes");
    let mut found_ttl = false;
    while cursor.advance().await.expect("advance index cursor") {
        let raw = cursor.deserialize_current().expect("index spec");
        let bson_spec = bson::to_bson(&raw).expect("index to bson");
        let doc_spec = bson_spec.as_document().expect("index doc").clone();
        if doc_spec.get("name").and_then(|v| v.as_str()) == Some("location_pings_ttl") {
            found_ttl = true;
        }
    }
    assert!(
        !found_ttl,
        "ensure_indexes should have dropped the old TTL index"
    );
}
