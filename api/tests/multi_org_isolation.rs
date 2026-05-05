mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

/// User with two memberships sees only `current_org`'s members. Switching the
/// active Org changes what `/dashboard-users` returns.
#[tokio::test]
async fn member_list_is_scoped_to_current_org() {
    let app = TestApp::spawn().await;

    // OrgA has two admins: owner_a + the visitor.
    let (owner_a, body_a) = app.register_admin("a-owner@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();

    // OrgB has its own owner; visitor will join via /me/memberships.
    let (owner_b, body_b) = app.register_admin("b-owner@example.com", "OrgB").await;
    let code_b = body_b["current_org"]["code"].as_str().unwrap().to_string();

    let (visitor, visitor_body) = app
        .register_member(&owner_a, "visitor@example.com", &code_a)
        .await;
    let visitor_id = visitor_body["user"]["id"].as_str().unwrap().to_string();
    // Promote visitor in OrgA so they can see the cooldowns endpoint too.
    owner_a
        .patch(app.url(&format!("/dashboard-users/{visitor_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();

    // Visitor requests to join OrgB via /me/memberships → pending request,
    // owner_b approves.
    let join = visitor
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code_b }))
        .send()
        .await
        .unwrap();
    assert_eq!(join.status(), StatusCode::OK);
    let pending_b: Value = owner_b
        .get(app.url("/orgs/me/join-requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let req_id_b = pending_b
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["email"] == "visitor@example.com")
        .and_then(|r| r["id"].as_str())
        .expect("pending visitor in OrgB")
        .to_string();
    let approve_b = owner_b
        .post(app.url(&format!("/orgs/me/join-requests/{req_id_b}/approve")))
        .send()
        .await
        .unwrap();
    assert_eq!(approve_b.status(), StatusCode::NO_CONTENT);
    // Switch visitor's current_org to OrgB explicitly so the next list call
    // is scoped there.
    let org_b_id = body_b["current_org"]["id"].as_str().unwrap().to_string();
    visitor
        .post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_b_id }))
        .send()
        .await
        .unwrap();

    // OrgB list visible to visitor: owner_b + visitor.
    let resp = visitor
        .get(app.url("/dashboard-users"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list: Value = resp.json().await.unwrap();
    let arr = list.as_array().unwrap();
    let emails: Vec<&str> = arr.iter().map(|u| u["email"].as_str().unwrap()).collect();
    assert!(emails.contains(&"b-owner@example.com"));
    assert!(emails.contains(&"visitor@example.com"));
    assert!(!emails.contains(&"a-owner@example.com"));

    // Switch back to OrgA.
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();
    visitor
        .post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_a_id }))
        .send()
        .await
        .unwrap();
    let resp = visitor
        .get(app.url("/dashboard-users"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list: Value = resp.json().await.unwrap();
    let arr = list.as_array().unwrap();
    let emails: Vec<&str> = arr.iter().map(|u| u["email"].as_str().unwrap()).collect();
    assert!(emails.contains(&"a-owner@example.com"));
    assert!(emails.contains(&"visitor@example.com"));
    assert!(!emails.contains(&"b-owner@example.com"));
}

/// Cooldown listing is also scoped: kicks in OrgA do not appear when the user
/// is acting on OrgB.
#[tokio::test]
async fn cooldown_list_is_scoped_to_current_org() {
    let app = TestApp::spawn().await;

    let (admin_a, body_a) = app.register_admin("a-owner@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();
    let (_kicked, kicked_body) = app
        .register_member(&admin_a, "kicked-a@example.com", &code_a)
        .await;
    let kicked_id = kicked_body["user"]["id"].as_str().unwrap().to_string();
    admin_a
        .delete(app.url(&format!("/dashboard-users/{kicked_id}")))
        .send()
        .await
        .unwrap();

    // multi owns OrgB and joins OrgA via /me/memberships (pending → admin_a approves).
    let (multi, body_b) = app.register_admin("multi@example.com", "OrgB").await;
    let org_b_id = body_b["current_org"]["id"].as_str().unwrap().to_string();
    multi
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code_a }))
        .send()
        .await
        .unwrap();
    let pending_a: Value = admin_a
        .get(app.url("/orgs/me/join-requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let req_id_a = pending_a
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["email"] == "multi@example.com")
        .and_then(|r| r["id"].as_str())
        .expect("pending multi in OrgA")
        .to_string();
    let approve_a = admin_a
        .post(app.url(&format!("/orgs/me/join-requests/{req_id_a}/approve")))
        .send()
        .await
        .unwrap();
    assert_eq!(approve_a.status(), StatusCode::NO_CONTENT);
    // Switch multi to OrgA explicitly.
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();
    multi
        .post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_a_id }))
        .send()
        .await
        .unwrap();
    // multi joined as a member in OrgA, can't list cooldowns there.
    let r = multi
        .get(app.url("/dashboard-users/cooldowns"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);

    // Switch to OrgB; multi is admin there.
    multi
        .post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_b_id }))
        .send()
        .await
        .unwrap();
    let r = multi
        .get(app.url("/dashboard-users/cooldowns"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let arr = r.json::<Value>().await.unwrap();
    let arr = arr.as_array().unwrap();
    // OrgB has no cooldowns yet — kicked-a was an OrgA event.
    assert_eq!(arr.len(), 0);
}
