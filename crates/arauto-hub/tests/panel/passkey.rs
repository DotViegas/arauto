//! The passkey of the panel: it is what opens the whole panel, a code never stands in its
//! place, and the hub keeps only what checks it.

use arauto_testkit::http;
use arauto_testkit::hub::support::{PANEL_HOST, PANEL_ORIGIN};
use arauto_testkit::passkey::SoftAuthenticator;

use crate::session::everything;
use crate::{Browser, PATIENCE, audited, cookie_of, form_value, hub, rows};

/// The sentence of every refusal of a passkey.
const REFUSED: &str = "The passkey was not accepted. Try again.";

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
    assert!(refused.text().contains(REFUSED));
    assert!(cookie_of(&refused).is_none());
    // The right passkey, answering a page of another address.
    authenticator.answer_as("https://evil.test");
    let refused = browser.pass(&mut authenticator).await;
    assert!(refused.text().contains(REFUSED), "{}", refused.text());
    assert!(cookie_of(&refused).is_none());
    authenticator.answer_as(PANEL_ORIGIN);
    // Without the person: no fingerprint, face or PIN.
    authenticator.skip_user_verification(true);
    let refused = browser.pass(&mut authenticator).await;
    assert!(refused.text().contains(REFUSED), "{}", refused.text());
    assert!(cookie_of(&refused).is_none());
    authenticator.skip_user_verification(false);
    // A signature that is not of what the hub asked.
    authenticator.tamper_signature(true);
    let refused = browser.pass(&mut authenticator).await;
    assert!(refused.text().contains(REFUSED), "{}", refused.text());
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
        assert!(refused.text().contains(REFUSED), "{body}");
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
    assert!(replayed.text().contains(REFUSED));
    assert!(cookie_of(&replayed).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_passkey_is_only_registered_in_a_session_and_for_what_the_hub_asked() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);

    // Without a session there is no challenge and nothing is kept.
    let asked = browser.post("/passkey/register/start", None, "").await;
    assert_eq!(asked.status, 403);
    let sent = browser
        .post("/passkey/register", None, "credential=%7B%7D")
        .await;
    assert_eq!(sent.status, 303);
    assert_eq!(sent.header("location"), Some("/sign-in"));
    assert!(rows(&fixture, "panel_passkey").await.is_empty());

    let cookie = browser.entered(&fixture).await;
    // An answer made for another address, or without the person, is not kept.
    let mut authenticator = SoftAuthenticator::new("https://evil.test");
    let refused = browser.register(&cookie, &mut authenticator).await;
    assert_eq!(refused.status, 200);
    assert!(refused.text().contains(REFUSED));
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    authenticator.skip_user_verification(true);
    let refused = browser.register(&cookie, &mut authenticator).await;
    assert!(refused.text().contains(REFUSED), "{}", refused.text());
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
    assert!(again.text().contains(REFUSED));
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
        let asked = browser.post("/passkey/sign-in/start", None, "").await;
        assert_eq!(asked.status, 200);
    }
    assert_eq!(everything(&fixture).await, before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hub_with_no_passkey_issues_an_installation_code_and_one_with_a_passkey_does_not() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);

    // While no admin has a passkey, the start of the hub issues a code, valid for longer
    // than the one of the command: the time to read the log and open the panel.
    let now = fixture.now();
    let issued = fixture
        .admin
        .installation_code(now)
        .await
        .unwrap()
        .expect("a hub with no passkey issues a code");
    assert_eq!(
        issued.expires_at - now,
        arauto_hub::repo::panel::INSTALLATION_VALIDITY_MS
    );
    assert_eq!(audited(&fixture, "panel.code_issued").await, 1);
    // It is a code like the one of the command: it opens the registration and nothing else.
    let entered = browser.enter(&issued.code.reveal()).await;
    assert_eq!(entered.status, 303);
    assert_eq!(entered.header("location"), Some("/first-access"));
    let cookie = cookie_of(&entered).unwrap();
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    assert_eq!(
        browser.register(&cookie, &mut authenticator).await.status,
        303
    );

    // With a passkey, a start issues nothing and writes nothing: a code then only comes out
    // of the command, asked for by a person.
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
}
