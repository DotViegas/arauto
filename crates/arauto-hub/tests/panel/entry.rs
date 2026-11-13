//! The port of the panel and the entry by a code of one use.

use std::sync::Arc;

use arauto_hub::repo::panel::ENTRY_VALIDITY_MS;
use arauto_protocol::clock::ManualClock;
use arauto_testkit::http;
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support::{PANEL_HOST, PANEL_ORIGIN};
use data_encoding::{BASE32_NOPAD, BASE64URL_NOPAD, HEXLOWER, HEXUPPER};

use crate::{Browser, COOKIE, PATIENCE, REFUSED, audited, code, cookie_of, hub, rows};

#[tokio::test(flavor = "multi_thread")]
async fn the_panel_answers_only_on_its_own_port() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    assert_eq!(browser.get("/sign-in", None).await.status, 200);

    // Nothing of the panel exists on the port of the connectors nor on the internal one, by
    // any name: the login of the admin never travels there.
    for port in [
        fixture.running().connectors_addr(),
        fixture.running().internal_addr(),
    ] {
        for path in [
            "/",
            "/sign-in",
            "/whatsapps",
            "/accesses",
            "/assets/panel.css",
        ] {
            for host in [PANEL_HOST, "arauto.test"] {
                let answer = http::get(port, path, &[("Host", host)], PATIENCE)
                    .await
                    .unwrap();
                assert_eq!(answer.status, 404, "{port} {path} {host}");
            }
        }
        let headers = [
            ("Host", PANEL_HOST),
            ("Origin", PANEL_ORIGIN),
            ("Content-Type", "application/x-www-form-urlencoded"),
        ];
        let answer = http::post(port, "/sign-in", &headers, b"code=x", PATIENCE)
            .await
            .unwrap();
        assert_eq!(answer.status, 404, "{port}");
    }

    // And nothing of the two other ports is answered by the panel.
    for path in ["/health", "/v1/channel", "/v1/download/arauto-connector"] {
        assert_eq!(browser.get(path, None).await.status, 404, "{path}");
    }
    let events = "/v1/engine/00000000-0000-4000-8000-000000000001/events";
    assert_eq!(browser.post(events, None, "{}").await.status, 404);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_that_does_not_name_the_panel_is_refused_before_anything_is_read() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let entry = code(&fixture).await;

    // Another name in `Host`: the request reached this port without being for the panel.
    for host in [
        "evil.test",
        "localhost",
        "localhost:8445.evil.test",
        "127.0.0.1:8445",
    ] {
        let answer = http::get(browser.panel, "/sign-in", &[("Host", host)], PATIENCE)
            .await
            .unwrap();
        assert_eq!(answer.status, 421, "{host}");
    }

    // A form that did not come from a page of the panel: no `Origin`, or another one.
    let form = ("Content-Type", "application/x-www-form-urlencoded");
    let body = format!("code={entry}");
    for origin in [
        None,
        Some("http://evil.test"),
        Some("https://localhost:8445"),
        Some("http://localhost:8445.evil.test"),
        Some("null"),
    ] {
        let mut headers = vec![("Host", PANEL_HOST), form];
        if let Some(origin) = origin {
            headers.push(("Origin", origin));
        }
        let answer = http::post(
            browser.panel,
            "/sign-in",
            &headers,
            body.as_bytes(),
            PATIENCE,
        )
        .await
        .unwrap();
        assert_eq!(answer.status, 403, "{origin:?}");
        assert!(cookie_of(&answer).is_none(), "{origin:?}");
    }
    // The same for the request that closes a session.
    let answer = http::post(
        browser.panel,
        "/sign-out",
        &[("Host", PANEL_HOST)],
        b"",
        PATIENCE,
    )
    .await
    .unwrap();
    assert_eq!(answer.status, 403);

    // None of that spent the code: it was refused before the form was read.
    assert_eq!(audited(&fixture, "panel.signed_in").await, 0);
    assert_eq!(browser.enter(&entry).await.status, 303);

    // And what is neither a read nor a form is not handled at all.
    for method in ["PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"] {
        let answer = http::send(
            method,
            browser.panel,
            "/",
            &[("Host", PANEL_HOST)],
            b"",
            PATIENCE,
        )
        .await
        .unwrap();
        assert_eq!(answer.status, 405, "{method}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_code_opens_one_session_and_only_once() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let entry = code(&fixture).await;
    assert_eq!(audited(&fixture, "panel.code_issued").await, 1);

    // Opening the page of the entry spends nothing, however many times.
    for _ in 0..3 {
        let page = browser.get("/sign-in", None).await;
        assert_eq!(page.status, 200);
        assert!(page.text().contains("Código de entrada"));
        assert!(
            !page.text().contains(&entry),
            "the panel never shows a code"
        );
    }

    let answer = browser.enter(&entry).await;
    assert_eq!(answer.status, 303);
    assert_eq!(answer.header("location"), Some("/"));
    let set = answer.header("set-cookie").unwrap();
    assert!(set.starts_with(&format!("{COOKIE}=")), "{set}");
    for attribute in ["Path=/", "HttpOnly", "SameSite=Strict", "Max-Age=43200"] {
        assert!(
            set.split("; ").any(|part| part == attribute),
            "{attribute}: {set}"
        );
    }
    let cookie = cookie_of(&answer).unwrap();

    // With the cookie, the panel; with it, the page of the entry leads to the panel.
    let home = browser.get("/", Some(&cookie)).await;
    assert_eq!(home.status, 200);
    assert!(
        home.text()
            .contains("Seus Hermes e os <b>[números]</b> deles")
    );
    assert_eq!(browser.get("/sign-in", Some(&cookie)).await.status, 303);

    // The code is spent: typed again, by anyone, it opens nothing.
    let again = browser.enter(&entry).await;
    assert_eq!(again.status, 200);
    assert!(again.text().contains(REFUSED));
    assert!(cookie_of(&again).is_none());
    assert_eq!(audited(&fixture, "panel.signed_in").await, 1);

    // What the database keeps of it: the row, without the digest.
    let kept = rows(&fixture, "panel_entry").await;
    assert_eq!(kept.len(), 1);
    assert!(kept[0].contains("\"state\":\"used\""), "{}", kept[0]);
    assert!(kept[0].contains("\"digest\":null"), "{}", kept[0]);
}

#[tokio::test(flavor = "multi_thread")]
async fn what_is_not_the_code_is_refused_with_one_sentence_and_no_session() {
    let clock = Arc::new(ManualClock::new(1_800_000_000_000));
    let fixture = Fixture::with_clock(fixture::settings(), clock.clone()).await;
    let browser = Browser::of(&fixture);
    let entry = code(&fixture).await;

    // A code that was never issued, in the right form; one letter changed; and what is not
    // a code at all. Every answer is the same page, and none says which case it was.
    let never_issued = "abcd-efgh-ijkl-mnop-qrst-uvwx-yzab-cdef";
    let mut changed = entry.clone();
    let first = if changed.starts_with('a') { "b" } else { "a" };
    changed.replace_range(0..1, first);
    for typed in [
        never_issued.to_owned(),
        changed,
        entry[..entry.len() - 1].to_owned(),
        String::new(),
        "%3Cscript%3Ealert(1)%3C%2Fscript%3E".to_owned(),
        "x".repeat(2_000),
    ] {
        let answer = browser.enter(&typed).await;
        assert_eq!(answer.status, 200, "{typed}");
        assert!(answer.text().contains(REFUSED), "{typed}");
        assert!(
            !answer.text().contains("<script>alert"),
            "what was typed is not echoed"
        );
        assert!(cookie_of(&answer).is_none(), "{typed}");
    }
    // A form without the field, and what is not a form.
    assert!(
        browser
            .post("/sign-in", None, "other=1")
            .await
            .text()
            .contains(REFUSED)
    );
    let not_a_form = [
        ("Host", PANEL_HOST),
        ("Origin", PANEL_ORIGIN),
        ("Content-Type", "application/json"),
    ];
    let answer = http::post(browser.panel, "/sign-in", &not_a_form, b"{}", PATIENCE)
        .await
        .unwrap();
    assert_eq!(answer.status, 415);

    // The right code, one millisecond before its 10 minutes: still good. A deadline passed
    // when the clock of the hub reached it.
    clock.advance(ENTRY_VALIDITY_MS);
    let late = browser.enter(&entry).await;
    assert!(
        late.text().contains(REFUSED),
        "at 10 minutes the code is dead"
    );
    assert!(cookie_of(&late).is_none());
    assert!(rows(&fixture, "panel_session").await.is_empty());
    assert_eq!(audited(&fixture, "panel.signed_in").await, 0);

    let fresh = code(&fixture).await;
    clock.advance(ENTRY_VALIDITY_MS - 1);
    assert_eq!(browser.enter(&fresh).await.status, 303);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_code_cancels_the_one_before() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let first = code(&fixture).await;
    let second = code(&fixture).await;
    assert_ne!(first, second);

    assert!(browser.enter(&first).await.text().contains(REFUSED));
    assert_eq!(browser.enter(&second).await.status, 303);

    let kept = rows(&fixture, "panel_entry").await;
    assert_eq!(kept.len(), 2);
    assert_eq!(
        kept.iter()
            .filter(|row| row.contains("\"state\":\"cancelled\""))
            .count(),
        1
    );
    assert!(
        kept.iter().all(|row| row.contains("\"digest\":null")),
        "{kept:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn neither_secret_is_kept_in_the_database_nor_written_in_the_audit() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let entry = code(&fixture).await;
    // A second code stays issued, so that a digest of a code is in the database too.
    let answer = browser.enter(&entry).await;
    let waiting = code(&fixture).await;
    let cookie = cookie_of(&answer).unwrap();
    let identifier = cookie.split_once('=').unwrap().1.to_owned();

    // Every way the bytes of each secret could have been written.
    let mut secrets = Vec::new();
    for code in [&entry, &waiting] {
        let plain = code.replace('-', "");
        let bytes = BASE32_NOPAD
            .decode(plain.to_ascii_uppercase().as_bytes())
            .unwrap();
        secrets.extend([code.clone(), plain.clone(), plain.to_ascii_uppercase()]);
        secrets.extend([HEXLOWER.encode(&bytes), HEXUPPER.encode(&bytes)]);
    }
    let bytes = BASE64URL_NOPAD.decode(identifier.as_bytes()).unwrap();
    assert_eq!(bytes.len(), 32);
    secrets.extend([identifier, HEXLOWER.encode(&bytes), HEXUPPER.encode(&bytes)]);

    let mut written = String::new();
    for table in ["panel_entry", "panel_session", "audit", "alert"] {
        written.push_str(&rows(&fixture, table).await.join("\n"));
    }
    assert!(written.contains("panel.signed_in"), "the audit was read");
    for secret in &secrets {
        assert!(
            !written.contains(secret.as_str()),
            "a secret is written in the database"
        );
    }
    // The detector sees a secret when one is there.
    assert!(format!("{written}{}", secrets[0]).contains(secrets[0].as_str()));

    // What is kept is a digest of 32 bytes, for the open session and for the code that waits.
    let open: Vec<Vec<u8>> =
        sqlx::query_scalar("SELECT digest FROM panel_session WHERE state = 'open'")
            .fetch_all(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].len(), 32);
    assert_ne!(open[0], bytes);
    let issued: Vec<Vec<u8>> =
        sqlx::query_scalar("SELECT digest FROM panel_entry WHERE state = 'issued'")
            .fetch_all(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(issued.len(), 1);
    assert_eq!(issued[0].len(), 32);
}
