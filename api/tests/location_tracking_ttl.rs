//! `location_pings` TTL index sanity check. We don't wait for the actual
//! 60s TTL monitor sweep — we read the index spec and assert
//! `expireAfterSeconds = 90 days`.

mod common;

use common::TestApp;

#[tokio::test]
async fn ttl_index_present_with_90_day_expiry() {
    let app = TestApp::spawn().await;

    let coll = app.state.db.database.collection::<bson::Document>("location_pings");
    let mut cursor = coll.list_indexes().await.expect("list indexes");

    let mut found_ttl = false;
    while cursor.advance().await.expect("advance index cursor") {
        let raw = cursor.deserialize_current().expect("index spec");
        // The driver returns IndexModel-shaped specs; we want the raw doc form
        // because `expireAfterSeconds` is an Option that won't be on most
        // indexes. Easiest is to roundtrip via bson.
        let bson_spec = bson::to_bson(&raw).expect("index to bson");
        let doc_spec = bson_spec.as_document().expect("index doc").clone();
        if doc_spec
            .get("name")
            .and_then(|v| v.as_str())
            == Some("location_pings_ttl")
        {
            // Mongo stores expireAfterSeconds as i32 or i64 depending on
            // driver version — accept either.
            let expire_secs = doc_spec
                .get("expireAfterSeconds")
                .and_then(|v| v.as_i32().map(|i| i as i64).or_else(|| v.as_i64()))
                .expect("expireAfterSeconds present");
            assert_eq!(expire_secs, 90 * 24 * 3600, "TTL should be 90 days");
            found_ttl = true;
        }
    }

    assert!(found_ttl, "location_pings_ttl index not found");
}
