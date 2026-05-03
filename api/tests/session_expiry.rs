mod common;

use bson::{DateTime as BsonDateTime, doc};
use common::TestApp;
use reqwest::StatusCode;

#[tokio::test]
async fn expired_session_is_rejected_and_cookie_cleared() {
    let app = TestApp::spawn().await;

    let (client, _) = app.register_admin("founder@example.com", "Acme").await;
    // Find the session token from the cookie store.
    let me = client.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    // Pull the token directly from the DB — the cookie jar is private to the
    // reqwest client. There is exactly one session at this point.
    let session_doc = app
        .state
        .db
        .database
        .collection::<bson::Document>("dashboard_sessions")
        .find_one(doc! {})
        .await
        .unwrap()
        .expect("session row");
    let token = session_doc.get_str("_id").unwrap().to_string();

    // Force the session's expires_at into the past, simulating natural TTL elapse
    // without waiting (we don't trust the Mongo TTL monitor for unit-test timing,
    // but the middleware does an explicit expiry check on read).
    let past = BsonDateTime::from_millis(0);
    let updated = app
        .state
        .db
        .database
        .collection::<bson::Document>("dashboard_sessions")
        .update_one(doc! { "_id": &token }, doc! { "$set": { "expires_at": past } })
        .await
        .unwrap();
    assert_eq!(updated.matched_count, 1, "expected to find the session row");

    let me = client.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);

    // Middleware should emit a Set-Cookie clearing argus_session.
    let cleared = me
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .any(|v| {
            let s = v.to_str().unwrap_or("");
            s.starts_with("argus_session=") && (s.contains("Max-Age=0") || s.contains("max-age=0"))
        });
    assert!(
        cleared,
        "expected clearing Set-Cookie for argus_session, got headers: {:?}",
        me.headers().get_all(reqwest::header::SET_COOKIE)
    );
}
