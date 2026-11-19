//! Why an invite is issued (decision 0053): the reason is derived under the lock, before the
//! request in progress is closed, and stored on the invite.

use arauto_hub::core::model::Device;
use arauto_hub::core::reconnect::{
    Clause, CoreVersion, FirstLine, Reason, ReasonCode, Served, newest_final,
};
use arauto_hub::repo::closing::{self, RevokeOutcome};
use arauto_hub::repo::hermes;
use arauto_hub::repo::invite::{self, INVITE_VALIDITY_MS, IssueOutcome, IssuedInvite};
use arauto_protocol::names::HermesName;
use uuid::Uuid;

use crate::support::{World, address, identity};

async fn issue(world: &World, name: &HermesName, served: &Served) -> Box<IssuedInvite> {
    match invite::issue(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        name,
        served,
        world.now(),
    )
    .await
    .unwrap()
    {
        IssueOutcome::Issued(issued) => issued,
        IssueOutcome::NotFound => panic!("there is no Hermes {name}"),
    }
}

/// The four columns of the reason of an invite, as stored.
async fn stored(
    world: &World,
    id: Uuid,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    sqlx::query_as(
        "SELECT reason, version_clause, version_from, version_to FROM invite \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(world.org.id())
    .bind(id)
    .fetch_one(world.pool())
    .await
    .unwrap()
}

async fn audited(world: &World, id: Uuid) -> serde_json::Value {
    sqlx::query_scalar(
        "SELECT details FROM audit WHERE organization_id = $1 AND target_id = $2 \
         AND action = 'invite.issued'",
    )
    .bind(world.org.id())
    .bind(id)
    .fetch_one(world.pool())
    .await
    .unwrap()
}

async fn revoke(world: &World, name: &HermesName) {
    let outcome = closing::revoke(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        name,
        world.now(),
    )
    .await
    .unwrap();
    assert!(matches!(outcome, RevokeOutcome::Revoked(_)));
}

async fn rows_of(world: &World, name: &HermesName) -> Vec<Device> {
    let id = world.hermes_id(name).await;
    sqlx::query_as("SELECT * FROM device WHERE organization_id = $1 AND hermes_id = $2")
        .bind(world.org.id())
        .bind(id)
        .fetch_all(world.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn the_reason_is_read_before_the_request_is_closed() {
    let world = World::shared().await;
    let served = Served::nothing();

    // An approved request: once closed by this very invite its state would be gone.
    let name = world.hermes().await;
    world.approved(&name, None).await;
    world.clock.advance(1_000);
    let issued = issue(&world, &name, &served).await;
    assert_eq!(issued.reason.code, ReasonCode::RequestApproved);
    assert!(issued.closed_request.is_some(), "the request was closed");
    assert_eq!(
        stored(&world, issued.id).await.0.as_deref(),
        Some("request_approved")
    );
    assert_eq!(
        audited(&world, issued.id).await["reason"],
        "request_approved"
    );

    // A pending one.
    let name = world.hermes().await;
    world.pending(&name).await;
    world.clock.advance(1_000);
    let issued = issue(&world, &name, &served).await;
    assert_eq!(issued.reason.code, ReasonCode::RequestPending);
    assert_eq!(
        stored(&world, issued.id).await.0.as_deref(),
        Some("request_pending")
    );
    assert_eq!(
        audited(&world, issued.id).await["reason"],
        "request_pending"
    );
    // The invite after it finds the request closed by an invite: that row says nothing.
    world.clock.advance(1_000);
    let again = issue(&world, &name, &served).await;
    assert_eq!(again.reason.code, ReasonCode::Other);
}

#[tokio::test]
async fn a_connected_hermes_gets_other_and_the_reconnect_line() {
    let world = World::shared().await;
    let name = world.hermes().await;
    world.active(&name).await;
    world.clock.advance(1_000);
    let issued = issue(&world, &name, &Served::nothing()).await;
    assert_eq!(issued.reason.code, ReasonCode::Other);
    assert_eq!(issued.reason.first_line, FirstLine::Replaces);
    assert!(issued.reason.shows_why());
    assert!(issued.reconnect);
    assert_eq!(stored(&world, issued.id).await.0.as_deref(), Some("other"));
}

#[tokio::test]
async fn a_revoked_one_gets_revoked_and_comes_back() {
    let world = World::shared().await;
    let name = world.hermes().await;
    world.active(&name).await;
    world.clock.advance(1_000);
    revoke(&world, &name).await;
    world.clock.advance(1_000);
    let issued = issue(&world, &name, &Served::nothing()).await;
    assert_eq!(issued.reason.code, ReasonCode::Revoked);
    assert_eq!(issued.reason.first_line, FirstLine::BringsBack);
    assert!(!issued.reconnect, "there is no device in use");
    assert_eq!(
        stored(&world, issued.id).await.0.as_deref(),
        Some("revoked")
    );
}

#[tokio::test]
async fn one_that_left_gets_left() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    world.clock.advance(1_000);
    closing::leave(
        world.pool(),
        &world.home.mark,
        world.org,
        active.device,
        address(150),
        world.now(),
    )
    .await
    .unwrap();
    world.clock.advance(1_000);
    let issued = issue(&world, &name, &Served::nothing()).await;
    assert_eq!(issued.reason.code, ReasonCode::Left);
    assert_eq!(issued.reason.first_line, FirstLine::BringsBack);
    assert_eq!(stored(&world, issued.id).await.0.as_deref(), Some("left"));
}

#[tokio::test]
async fn an_invite_that_passed_its_deadline_unused_is_the_reason() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let served = Served::nothing();
    issue(&world, &name, &served).await;
    world.clock.advance(INVITE_VALIDITY_MS);
    let second = issue(&world, &name, &served).await;
    assert_eq!(second.reason.code, ReasonCode::InviteUnused);
    assert_eq!(second.reason.first_line, FirstLine::First);
    assert!(second.reason.shows_why());
    assert_eq!(
        stored(&world, second.id).await.0.as_deref(),
        Some("invite_unused")
    );

    // One millisecond before the deadline it is still valid: not a reason.
    let name = world.hermes().await;
    issue(&world, &name, &served).await;
    world.clock.advance(1);
    let third = issue(&world, &name, &served).await;
    assert_eq!(third.reason.code, ReasonCode::Other);
    assert!(!third.reason.shows_why());
}

#[tokio::test]
async fn the_enrollment_keeps_the_program_that_enrolled() {
    let world = World::shared().await;
    let mark = "b".repeat(64);
    let declared = format!("0.2.0+{mark}");
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let who = identity();
    let (outcome, code) = world
        .enroll_declaring(&issued, &who, address(10), &declared, 2)
        .await;
    let accepted = match outcome {
        arauto_hub::repo::enroll::EnrollOutcome::Enrolled(accepted) => accepted,
        other => panic!("not enrolled: {other:?}"),
    };
    let row = world.device(accepted.device).await;
    assert_eq!(row.enrolled_connector.as_deref(), Some(declared.as_str()));
    assert_eq!(row.enrolled_proto, Some(2));
    assert_eq!(row.connector_version.as_deref(), Some(declared.as_str()));

    // An older connector speaking protocol 1.
    let other = world.hermes().await;
    let issued = world.invite(&other).await;
    let (outcome, _) = world
        .enroll_declaring(&issued, &identity(), address(11), "0.1.0", 1)
        .await;
    let arauto_hub::repo::enroll::EnrollOutcome::Enrolled(old) = outcome else {
        panic!("not enrolled");
    };
    let row = world.device(old.device).await;
    assert_eq!(row.enrolled_proto, Some(1));
    assert_eq!(row.enrolled_connector.as_deref(), Some("0.1.0"));

    // The first swap overwrites what runs, and leaves what enrolled.
    let approved = world.approve(&name, code, None).await;
    assert!(matches!(
        approved,
        arauto_hub::repo::approval::ApproveOutcome::Approved(_)
    ));
    let request = crate::support::Request {
        device: accepted.device,
        identity: who,
        code,
    };
    world.first_swap(&request).await;
    let row = world.device(accepted.device).await;
    assert_eq!(row.connector_version.as_deref(), Some("0.1.0"));
    assert_eq!(row.proto, Some(1));
    assert_eq!(row.enrolled_connector.as_deref(), Some(declared.as_str()));
    assert_eq!(row.enrolled_proto, Some(2));
}

#[tokio::test]
async fn the_list_and_the_rule_agree_on_the_newest_final_device() {
    let world = World::shared().await;
    let served = Served::nothing();
    let line_of = |lines: Vec<hermes::HermesLine>, name: &HermesName| {
        lines
            .into_iter()
            .find(|line| line.hermes.name == *name)
            .unwrap()
    };
    let list = || async { hermes::list(world.pool(), world.org, false).await.unwrap() };

    // Revoked, then an approved request: the revoked row is the newest final one.
    let name = world.hermes().await;
    let first = world.active(&name).await;
    world.clock.advance(1_000);
    revoke(&world, &name).await;
    world.clock.advance(1_000);
    world.approved(&name, None).await;
    let rows = rows_of(&world, &name).await;
    let rule = newest_final(&rows).map(|d| d.id);
    let line = line_of(list().await, &name);
    assert_eq!(rule, Some(first.device));
    assert_eq!(line.newest_final.map(|d| d.id), rule);

    // Replaced rows never count: after a reconnect that took over, nothing is newer.
    let name = world.hermes().await;
    world.active(&name).await;
    world.clock.advance(1_000);
    issue(&world, &name, &served).await;
    let rows_before = rows_of(&world, &name).await;
    assert!(newest_final(&rows_before).is_none());
    let line = line_of(list().await, &name);
    assert!(line.newest_final.is_none());

    // A closed row older than the device in use is not the line's.
    let name = world.hermes().await;
    world.active(&name).await;
    world.clock.advance(1_000);
    revoke(&world, &name).await;
    world.clock.advance(1_000);
    world.active(&name).await;
    let rows = rows_of(&world, &name).await;
    assert!(newest_final(&rows).is_none());
    let line = line_of(list().await, &name);
    assert!(line.in_use.is_some() && line.newest_final.is_none());
}

#[tokio::test]
async fn the_preview_is_the_reason_the_write_would_store() {
    let world = World::shared().await;
    let served = Served::nothing();
    let preview = |name: HermesName| {
        let world = &world;
        let served = &served;
        async move {
            invite::reason_preview(world.pool(), world.org, &name, served, world.now())
                .await
                .unwrap()
        }
    };

    let name = world.hermes().await;
    world.approved(&name, None).await;
    world.clock.advance(1_000);
    let rows = world.audit_rows().await;
    let shown = preview(name.clone()).await.unwrap();
    assert_eq!(world.audit_rows().await, rows, "a preview writes nothing");
    let written = issue(&world, &name, &served).await;
    assert_eq!(shown, written.reason);

    let name = world.hermes().await;
    world.active(&name).await;
    world.clock.advance(1_000);
    revoke(&world, &name).await;
    world.clock.advance(1_000);
    let shown = preview(name.clone()).await.unwrap();
    assert_eq!(shown.code, ReasonCode::Revoked);
    let written = issue(&world, &name, &served).await;
    assert_eq!(shown, written.reason);

    let missing = HermesName::parse("nobody-here").unwrap();
    assert!(preview(missing).await.is_none());
}

#[tokio::test]
async fn the_audit_of_the_invite_holds_only_fixed_words() {
    let world = World::shared().await;
    let mark = "c".repeat(64);
    let declared = format!("0.1.0+{mark}");
    let name = world.hermes().await;
    world.active_declaring(&name, &declared, 1).await;
    world.clock.advance(1_000);
    let bundle = CoreVersion::parse_core("0.1.1");
    let served = Served::of(&[], bundle);
    let issued = issue(&world, &name, &served).await;
    let expected = Reason {
        code: ReasonCode::Other,
        clause: Some(Clause::Numbers),
        first_line: FirstLine::Replaces,
        version_from: CoreVersion::parse_core("0.1.0"),
        version_to: bundle,
    };
    assert_eq!(issued.reason, expected);
    assert_eq!(
        stored(&world, issued.id).await,
        (
            Some("other".to_owned()),
            Some("numbers".to_owned()),
            Some("0.1.0".to_owned()),
            Some("0.1.1".to_owned()),
        )
    );
    let details = audited(&world, issued.id).await;
    assert_eq!(details["reason"], "other");
    assert_eq!(details["version_clause"], "numbers");
    assert_eq!(details["version_from"], "0.1.0");
    assert_eq!(details["version_to"], "0.1.1");
    // Nothing the device declared is there: not the mark, not the text.
    let all = details.to_string();
    assert!(!all.contains(&mark) && !all.contains('+'), "{all}");
    let anywhere: Vec<String> = sqlx::query_scalar(
        "SELECT details::text FROM audit WHERE organization_id = $1 AND details::text LIKE $2",
    )
    .bind(world.org.id())
    .bind(format!("%{mark}%"))
    .fetch_all(world.pool())
    .await
    .unwrap();
    assert!(anywhere.is_empty());
}
