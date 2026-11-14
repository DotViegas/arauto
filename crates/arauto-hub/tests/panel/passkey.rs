//! The passkey of the panel: it is what opens the whole panel, a code never stands in its
//! place, and the hub keeps only what checks it.

use arauto_testkit::http;
use arauto_testkit::hub::fixture::Fixture;
use arauto_testkit::hub::support::{PANEL_HOST, PANEL_ORIGIN};
use arauto_testkit::passkey::SoftAuthenticator;

use crate::session::everything;
use crate::{Browser, PASSKEY_REFUSED, PATIENCE, audited, cookie_of, form_value, hub, rows};

/// The sentence of every refusal of a passkey.
#[tokio::test(flavor = "multi_thread")]
async fn a_passkey_signs_in_and_only_the_one_the_hub_keeps() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let (first, mut authenticator) = browser.first_access(&fixture).await;
    assert_eq!(audited(&fixture, "panel.passkey_registered").await, 1);
    assert_eq!(rows(&fixture, "panel_passkey").await.len(), 1);
    browser.post("/sign-out", Some(&first), "").await;
    assert_eq!(browser.get("/", Some(&first)).await.status, 303);

    // The passkey opens a session that reaches the whole panel, with no code at all.
    let signed_before = audited(&fixture, "panel.signed_in").await;
    let passed = browser.pass(&mut authenticator).await;
    assert_eq!(passed.status, 303, "{}", passed.text());
    assert_eq!(passed.header("location"), Some("/"));
    let cookie = cookie_of(&passed).expect("the passkey sets the cookie of the session");
    assert_ne!(cookie, first);
    assert_eq!(browser.get("/", Some(&cookie)).await.status, 200);
    assert_eq!(browser.get("/accesses", Some(&cookie)).await.status, 200);
    assert_eq!(
        audited(&fixture, "panel.signed_in").await,
        signed_before + 1
    );
    let passkey = rows(&fixture, "panel_passkey").await.remove(0);
    assert!(!passkey.contains("\"last_used_at\":null"), "{passkey}");

    // What does not sign in, each with the same sentence and no session.
    let before = everything(&fixture).await;
    let sessions = rows(&fixture, "panel_session").await.len();
    // A passkey the hub never kept.
    let mut stranger = SoftAuthenticator::new(PANEL_ORIGIN);
    let asked = browser
        .post("/passkey/register/start", Some(&cookie), "")
        .await;
    stranger.create(&asked.text()).unwrap();
    let refused = browser.pass(&mut stranger).await;
    assert_eq!(refused.status, 200);
    assert!(refused.text().contains(PASSKEY_REFUSED));
    assert!(cookie_of(&refused).is_none());
    // The right passkey, answering a page of another address.
    authenticator.answer_as("https://evil.test");
    let refused = browser.pass(&mut authenticator).await;
    assert!(
        refused.text().contains(PASSKEY_REFUSED),
        "{}",
        refused.text()
    );
    assert!(cookie_of(&refused).is_none());
    authenticator.answer_as(PANEL_ORIGIN);
    // Without the person: no fingerprint, face or PIN.
    authenticator.skip_user_verification(true);
    let refused = browser.pass(&mut authenticator).await;
    assert!(
        refused.text().contains(PASSKEY_REFUSED),
        "{}",
        refused.text()
    );
    assert!(cookie_of(&refused).is_none());
    authenticator.skip_user_verification(false);
    // A signature that is not of what the hub asked.
    authenticator.tamper_signature(true);
    let refused = browser.pass(&mut authenticator).await;
    assert!(
        refused.text().contains(PASSKEY_REFUSED),
        "{}",
        refused.text()
    );
    assert!(cookie_of(&refused).is_none());
    authenticator.tamper_signature(false);
    // What is no answer of a browser.
    for body in [
        "",
        "credential=",
        "credential=%7B%7D",
        "credential=not-json",
        "other=1",
    ] {
        let refused = browser.post("/passkey/sign-in", None, body).await;
        assert_eq!(refused.status, 200, "{body}");
        assert!(refused.text().contains(PASSKEY_REFUSED), "{body}");
        assert!(cookie_of(&refused).is_none(), "{body}");
    }
    assert_eq!(rows(&fixture, "panel_session").await.len(), sessions);
    assert_eq!(
        everything(&fixture).await,
        before,
        "a refusal wrote something"
    );

    // An answer signs in once: sent again, its challenge is gone.
    let passed = browser.pass(&mut authenticator).await;
    assert_eq!(passed.status, 303, "{}", passed.text());
    let again = authenticator
        .replay()
        .expect("the authenticator answered before");
    let replayed = browser
        .post(
            "/passkey/sign-in",
            None,
            &format!("credential={}", form_value(&again)),
        )
        .await;
    assert_eq!(replayed.status, 200);
    assert!(replayed.text().contains(PASSKEY_REFUSED));
    assert!(cookie_of(&replayed).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_passkey_is_only_registered_in_a_session_and_for_what_the_hub_asked() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);

    // Without a session there is no challenge and nothing is kept.
    let asked = browser.post("/passkey/register/start", None, "").await;
    assert_eq!(asked.status, 401);
    let sent = browser
        .post("/passkey/register", None, "credential=%7B%7D")
        .await;
    // The entry of the state, with the sentence that says the panel session ended.
    assert_eq!(sent.status, 200);
    assert!(sent.text().contains("Your panel session ended."));
    assert!(rows(&fixture, "panel_passkey").await.is_empty());

    let cookie = browser.entered(&fixture).await;
    // An answer made for another address, or without the person, is not kept.
    let mut authenticator = SoftAuthenticator::new("https://evil.test");
    let refused = browser.register(&cookie, &mut authenticator).await;
    assert_eq!(refused.status, 200);
    assert!(refused.text().contains(PASSKEY_REFUSED));
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    authenticator.skip_user_verification(true);
    let refused = browser.register(&cookie, &mut authenticator).await;
    assert!(
        refused.text().contains(PASSKEY_REFUSED),
        "{}",
        refused.text()
    );
    assert!(rows(&fixture, "panel_passkey").await.is_empty());
    // And the session still reaches nothing else.
    assert_eq!(
        browser.get("/", Some(&cookie)).await.header("location"),
        Some("/first-access")
    );

    // A challenge serves one answer.
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    let asked = browser
        .post("/passkey/register/start", Some(&cookie), "")
        .await;
    let made = authenticator.create(&asked.text()).unwrap();
    let body = format!("credential={}", form_value(&made));
    let kept = browser
        .post("/passkey/register", Some(&cookie), &body)
        .await;
    assert_eq!(kept.status, 303, "{}", kept.text());
    let again = browser
        .post("/passkey/register", Some(&cookie), &body)
        .await;
    assert_eq!(again.status, 200);
    assert!(again.text().contains(PASSKEY_REFUSED));
    assert_eq!(rows(&fixture, "panel_passkey").await.len(), 1);

    // What the hub keeps checks a passkey and cannot be one: the row has the public part
    // and counters, and the audit names the row, never its bytes.
    let row = rows(&fixture, "panel_passkey").await.remove(0);
    for column in [
        "credential_id",
        "static_state",
        "dynamic_state",
        "user_handle",
    ] {
        assert!(
            row.contains(&format!("\"{column}\":\"\\\\x")),
            "{column}: {row}"
        );
    }
    let audit = rows(&fixture, "audit").await.join("\n");
    assert!(audit.contains("panel.passkey_registered"), "{audit}");
    for credential in authenticator.credential_ids() {
        let hex: String = credential
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert!(!audit.contains(&hex), "the audit holds a credential");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_code_never_stands_in_the_place_of_a_passkey() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let (_, mut first) = browser.first_access(&fixture).await;

    // With a passkey registered, a new code still opens only the registration: it is the
    // way back for who lost the passkey, and it shows nothing of the panel.
    let cookie = browser.entered(&fixture).await;
    for path in ["/", "/whatsapps", "/accesses"] {
        let sent = browser.get(path, Some(&cookie)).await;
        assert_eq!(sent.status, 303, "{path}");
        assert_eq!(sent.header("location"), Some("/first-access"));
        assert!(sent.body.is_empty());
    }
    let mut second = SoftAuthenticator::new(PANEL_ORIGIN);
    let kept = browser.register(&cookie, &mut second).await;
    assert_eq!(kept.status, 303, "{}", kept.text());
    assert_eq!(browser.get("/", Some(&cookie)).await.status, 200);
    assert_eq!(rows(&fixture, "panel_passkey").await.len(), 2);
    // Both passkeys sign in from then on.
    assert_eq!(browser.pass(&mut first).await.status, 303);
    assert_eq!(browser.pass(&mut second).await.status, 303);
}

#[tokio::test(flavor = "multi_thread")]
async fn asking_for_a_challenge_writes_nothing_and_is_never_a_read() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.entered(&fixture).await;
    let before = everything(&fixture).await;

    // An address that is opened asks for nothing.
    for path in [
        "/passkey/register/start",
        "/passkey/register",
        "/passkey/sign-in/start",
        "/passkey/sign-in",
    ] {
        let opened = browser.get(path, Some(&cookie)).await;
        assert_eq!(opened.status, 404, "{path}");
        // And a form that did not come from a page of the panel is refused.
        let elsewhere = http::post(
            browser.panel,
            path,
            &[("Host", PANEL_HOST), ("Origin", "https://evil.test")],
            b"",
            PATIENCE,
        )
        .await
        .unwrap();
        assert_eq!(elsewhere.status, 403, "{path}");
    }
    // The page of the registration and the two challenges: the database stays as it was.
    assert_eq!(
        browser.get("/first-access", Some(&cookie)).await.status,
        200
    );
    for _ in 0..3 {
        let asked = browser
            .post("/passkey/register/start", Some(&cookie), "")
            .await;
        assert_eq!(asked.status, 200);
        assert_eq!(asked.header("content-type"), Some("application/json"));
        assert_eq!(asked.header("cache-control"), Some("no-store"));
        // Before the first access there is no passkey to sign in with.
        let asked = browser.post("/passkey/sign-in/start", None, "").await;
        assert_eq!(asked.status, 404);
    }
    assert_eq!(everything(&fixture).await, before);
}

/// R1, R1b: the start code has no deadline; the first passkey writes the durable mark; and
/// with the mark a start no longer issues one, even when every passkey is gone.
#[tokio::test(flavor = "multi_thread")]
async fn a_hub_that_never_had_a_passkey_issues_a_start_code_with_no_deadline_and_never_again_after()
{
    let fixture = hub().await;
    let browser = Browser::of(&fixture);

    let issued = fixture
        .admin
        .installation_code(fixture.now())
        .await
        .unwrap()
        .expect("a hub that never had a passkey issues a start code");
    assert_eq!(audited(&fixture, "panel.code_issued").await, 1);
    let row: (String, Option<i64>) =
        sqlx::query_as("SELECT kind, expires_at FROM panel_entry WHERE id = $1")
            .bind(issued.id)
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(row, ("start".to_owned(), None));
    // The database keeps a start code without a deadline and a command code with one, and
    // refuses the other two pairs by the constraint of the kind. Each probe row is valid in
    // everything else, so only that constraint can refuse it.
    for (kind, deadline, kept) in [
        ("start", None, true),
        ("command", Some(5_i64), true),
        ("start", Some(5_i64), false),
        ("command", None, false),
    ] {
        let probe = probe_entry(
            &fixture,
            ProbeEntry {
                kind,
                deadline,
                ..ProbeEntry::USED
            },
        )
        .await;
        refused_by(
            probe,
            kept,
            "panel_entry_deadline_of_kind",
            &format!("{kind} {deadline:?}"),
        );
    }
    // A cancelled code says when and by which kind; a code that is not cancelled says neither.
    for (state, cancelled_at, cancelled_by, kept) in [
        ("cancelled", Some(1_i64), Some("command"), true),
        ("cancelled", Some(1_i64), Some("start"), true),
        ("cancelled", None, Some("command"), false),
        ("cancelled", Some(1_i64), None, false),
        ("cancelled", None, None, false),
        ("used", Some(1_i64), None, false),
        ("used", None, Some("start"), false),
        ("used", Some(1_i64), Some("start"), false),
    ] {
        let used_at = (state == "used").then_some(1_i64);
        let probe = probe_entry(
            &fixture,
            ProbeEntry {
                state,
                used_at,
                cancelled_at,
                cancelled_by,
                ..ProbeEntry::USED
            },
        )
        .await;
        refused_by(
            probe,
            kept,
            "panel_entry_cancelled_has_its_facts",
            &format!("{state} {cancelled_at:?} {cancelled_by:?}"),
        );
    }

    // Years later, it still opens the registration and nothing else.
    let entered = browser.enter(&issued.code.reveal()).await;
    assert_eq!(entered.status, 303);
    assert_eq!(entered.header("location"), Some("/first-access"));
    let cookie = cookie_of(&entered).unwrap();
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    assert_eq!(
        browser.register(&cookie, &mut authenticator).await.status,
        303
    );
    let mark: Option<i64> = sqlx::query_scalar("SELECT panel_first_passkey_at FROM organization")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert!(mark.is_some(), "the first passkey wrote the durable mark");

    // With a passkey, a start issues nothing and writes nothing.
    let before = everything(&fixture).await;
    assert!(
        fixture
            .admin
            .installation_code(fixture.now())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(everything(&fixture).await, before);

    // Every passkey lost: the start code does not come back, the command is the way back.
    sqlx::query("UPDATE panel_passkey SET state = 'removed', removed_at = 1")
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(
        fixture
            .admin
            .installation_code(fixture.now())
            .await
            .unwrap()
            .is_none()
    );
    // Not even the database lets the mark be cleared or changed.
    for change in [
        "UPDATE organization SET panel_first_passkey_at = NULL",
        "UPDATE organization SET panel_first_passkey_at = 1",
    ] {
        assert!(sqlx::query(change).execute(&fixture.pool).await.is_err());
    }
    let command = fixture.admin.panel_code(fixture.now()).await.unwrap();
    assert_eq!(browser.enter(&command.code.reveal()).await.status, 303);
}

/// A row of `panel_entry` to probe the constraints of the database with: valid in every column
/// the probe does not change.
struct ProbeEntry {
    kind: &'static str,
    deadline: Option<i64>,
    state: &'static str,
    used_at: Option<i64>,
    cancelled_at: Option<i64>,
    cancelled_by: Option<&'static str>,
}

impl ProbeEntry {
    /// A spent command code: a row every constraint accepts.
    const USED: Self = Self {
        kind: "command",
        deadline: Some(5),
        state: "used",
        used_at: Some(1),
        cancelled_at: None,
        cancelled_by: None,
    };
}

/// Inserts the probe row in a transaction that is rolled back, so the probe leaves nothing.
async fn probe_entry(
    fixture: &Fixture,
    probe: ProbeEntry,
) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
    let mut tx = fixture.pool.begin().await.unwrap();
    let inserted = sqlx::query(
        "INSERT INTO panel_entry (id, organization_id, digest, state, kind, created_by, \
                                  created_at, expires_at, used_at, cancelled_at, cancelled_by) \
         SELECT gen_random_uuid(), organization_id, NULL, $1, $2, created_by, 1, $3, $4, $5, $6 \
         FROM panel_entry LIMIT 1",
    )
    .bind(probe.state)
    .bind(probe.kind)
    .bind(probe.deadline)
    .bind(probe.used_at)
    .bind(probe.cancelled_at)
    .bind(probe.cancelled_by)
    .execute(&mut *tx)
    .await;
    tx.rollback().await.unwrap();
    inserted
}

/// The probe was kept, or refused by the named constraint and by no other.
fn refused_by(
    probe: Result<sqlx::postgres::PgQueryResult, sqlx::Error>,
    kept: bool,
    constraint: &str,
    what: &str,
) {
    match probe {
        Ok(done) => {
            assert!(kept, "{what}: the database kept a row it should refuse");
            assert_eq!(done.rows_affected(), 1, "{what}");
        }
        Err(error) => {
            assert!(!kept, "{what}: the database refused a valid row: {error}");
            let named = error.as_database_error().and_then(|e| e.constraint());
            assert_eq!(named, Some(constraint), "{what}: {error}");
        }
    }
}
