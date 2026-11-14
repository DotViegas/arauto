//! The instruction page of an invite and `GET /llms.txt` (`docs/14-protocolo-do-canal.md`,
//! section 1; `docs/decisoes/0042`): a read that changes nothing, a page that serves while the
//! invite is open or its request waits for the code, one 404 for everything else, and a handle
//! that is nowhere in the database in the clear.

use std::sync::Arc;
use std::time::Duration;

use arauto_hub::channel::Downloads;
use arauto_hub::join_page::PageHandle;
use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_testkit::device::{self, Enrolled};
use arauto_testkit::http::{self, Response};
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support::install;
use bytes::Bytes;
use data_encoding::{BASE32_NOPAD, BASE64, BASE64URL_NOPAD, HEXLOWER};
use sqlx::postgres::PgPool;

const PATIENCE: Duration = Duration::from_secs(10);
const CONNECTOR: &str = "arauto-connector-linux-x86_64";
const MINUTE_MS: i64 = 60_000;

/// What the 404 of a page says, as the file of the hub has it.
const GONE: &str = include_str!("../../src/join_page/gone.txt");
const LLMS: &str = include_str!("../../src/join_page/llms.txt");

async fn start(sweep_interval_ms: i64, clock: Arc<dyn Clock>, connector: bool) -> Fixture {
    let mut settings = fixture::settings();
    settings.sweep_interval_ms = sweep_interval_ms;
    let mut hub = Fixture::stopped(install().await, settings, clock).await;
    if connector {
        hub.downloads = Downloads::empty()
            .with_file(CONNECTOR, Bytes::from_static(b"the connector"))
            .unwrap();
    }
    hub.start().await;
    hub
}

async fn served() -> Fixture {
    start(200, Arc::new(SystemClock), true).await
}

fn path_of(handle: &PageHandle) -> String {
    format!("/v1/join/{}", handle.reveal().as_str())
}

async fn page(hub: &Fixture, handle: &PageHandle) -> Response {
    get(hub, &path_of(handle)).await
}

async fn get(hub: &Fixture, path: &str) -> Response {
    http::get(hub.addr(), path, &[], PATIENCE).await.unwrap()
}

/// What an answer is, without the clock of the server: the status, the headers but `date`, the
/// body.
fn shape(answer: &Response) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let headers = answer
        .headers
        .iter()
        .filter(|(name, _)| name != "date")
        .cloned()
        .collect();
    (answer.status, headers, answer.body.clone())
}

fn is_the_404(answer: &Response) -> bool {
    answer.status == 404 && answer.text() == GONE
}

async fn join_digest_is_null(hub: &Fixture, name: &str) -> bool {
    let present: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM invite i JOIN hermes h ON h.id = i.hermes_id \
         WHERE h.name = $1 AND i.join_digest IS NOT NULL",
    )
    .bind(name)
    .fetch_one(&hub.pool)
    .await
    .unwrap();
    present == 0
}

async fn until_the_page_is_erased(hub: &Fixture, name: &str) {
    for _ in 0..300 {
        if join_digest_is_null(hub, name).await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the digest of the page of {name} was not erased");
}

async fn count(pool: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn rows_of_invite(pool: &PgPool) -> String {
    let rows: Option<String> =
        sqlx::query_scalar("SELECT string_agg(i::text, E'\\n' ORDER BY i::text) FROM invite i")
            .fetch_one(pool)
            .await
            .unwrap();
    rows.unwrap_or_default()
}

/// The principle 5: a read does not change state. Five reads are the same bytes and leave the
/// invite, the devices and the audit as they were, and the invite is still good for the join.
#[tokio::test(flavor = "multi_thread")]
async fn reading_the_page_changes_nothing_and_join_still_works() {
    let hub = served().await;
    let name = hub.hermes().await;
    let (invite, handle) = hub.invite_with_page(&name).await;
    let before = (
        rows_of_invite(&hub.pool).await,
        count(&hub.pool, "device").await,
        count(&hub.pool, "audit").await,
    );
    let first = page(&hub, &handle).await;
    assert_eq!(first.status, 200);
    assert_eq!(
        first.header("content-type"),
        Some("text/markdown; charset=utf-8")
    );
    assert_eq!(first.header("cache-control"), Some("no-store"));
    assert_eq!(first.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(first.header("referrer-policy"), Some("no-referrer"));
    assert_eq!(first.header("x-robots-tag"), Some("noindex"));
    assert!(first.text().contains(&format!("a Hermes named `{name}`")));
    for _ in 0..4 {
        assert_eq!(shape(&page(&hub, &handle).await), shape(&first));
    }
    let after = (
        rows_of_invite(&hub.pool).await,
        count(&hub.pool, "device").await,
        count(&hub.pool, "audit").await,
    );
    assert_eq!(before, after, "five reads wrote nothing");
    // The invite still works, and the page serves while the request waits for the code.
    let device = hub.device();
    let Enrolled::Code(_) = device.enroll(&invite, device::declared()).await.unwrap() else {
        panic!("the invite was spent by the reads");
    };
    assert_eq!(page(&hub, &handle).await.status, 200);
}

/// The page lives until the request is decided, and the digest goes with it.
#[tokio::test(flavor = "multi_thread")]
async fn the_page_serves_while_the_request_waits_and_dies_when_decided() {
    let clock = ManualClock::new(SystemClock.now_ms());
    let hub = start(200, Arc::new(clock.clone()), true).await;

    // Approved.
    let name = hub.hermes().await;
    let (invite, handle) = hub.invite_with_page(&name).await;
    let device = hub.device();
    let Enrolled::Code(code) = device.enroll(&invite, device::declared()).await.unwrap() else {
        panic!("refused");
    };
    assert_eq!(page(&hub, &handle).await.status, 200, "pending");
    hub.approve(&name, &code.to_string()).await;
    assert!(is_the_404(&page(&hub, &handle).await), "approved");
    assert!(join_digest_is_null(&hub, &name).await);

    // Denied.
    let name = hub.hermes().await;
    let (invite, handle) = hub.invite_with_page(&name).await;
    let device = hub.device();
    device.enroll(&invite, device::declared()).await.unwrap();
    assert_eq!(page(&hub, &handle).await.status, 200);
    hub.admin.deny(&name, hub.now()).await.unwrap();
    assert!(is_the_404(&page(&hub, &handle).await), "denied");
    assert!(join_digest_is_null(&hub, &name).await);

    // The request that waits too long: the page stops at the deadline, and the sweep erases it.
    let name = hub.hermes().await;
    let (invite, handle) = hub.invite_with_page(&name).await;
    let device = hub.device();
    device.enroll(&invite, device::declared()).await.unwrap();
    assert_eq!(page(&hub, &handle).await.status, 200);
    clock.advance(11 * MINUTE_MS);
    assert!(is_the_404(&page(&hub, &handle).await), "timed out");
    until_the_page_is_erased(&hub, &name).await;
}

/// A handle nobody holds, a malformed one, an expired one (before and after the sweep), a
/// cancelled one, an approved one and a denied one all get the same bytes.
#[tokio::test(flavor = "multi_thread")]
async fn unknown_expired_and_decided_handles_get_one_identical_answer() {
    let clock = ManualClock::new(SystemClock.now_ms());
    let hub = start(200, Arc::new(clock.clone()), true).await;
    let slow_clock = ManualClock::new(SystemClock.now_ms());
    // A sweep that never comes in this test: the page must be 404 on its own at read time.
    let slow = start(3_600_000, Arc::new(slow_clock.clone()), true).await;

    let unknown = shape(&page(&hub, &PageHandle::generate().unwrap()).await);
    assert_eq!(unknown.0, 404);
    assert_eq!(unknown.2, GONE.as_bytes());
    let mut answers = vec![("unknown", unknown.clone())];
    for (what, path) in [
        ("empty", "/v1/join/".to_owned()),
        ("short", "/v1/join/abc".to_owned()),
        ("long", format!("/v1/join/{}", "a".repeat(27))),
        ("uppercase", format!("/v1/join/{}", "A".repeat(26))),
        ("zero", format!("/v1/join/{}", "0".repeat(26))),
        ("not canonical", format!("/v1/join/{}b", "a".repeat(25))),
        ("deeper", format!("/v1/join/{}/x", "a".repeat(26))),
        ("dots", "/v1/join/../v1/channel".to_owned()),
    ] {
        answers.push((what, shape(&get(&hub, &path).await)));
    }

    // Expired, without the sweep.
    let name = slow.hermes().await;
    let (_, handle) = slow.invite_with_page(&name).await;
    assert_eq!(page(&slow, &handle).await.status, 200);
    slow_clock.advance(16 * MINUTE_MS);
    answers.push(("expired, no sweep", shape(&page(&slow, &handle).await)));
    assert!(
        !join_digest_is_null(&slow, &name).await,
        "the sweep did not run"
    );

    // Expired, with the sweep.
    let name = hub.hermes().await;
    let (_, handle) = hub.invite_with_page(&name).await;
    clock.advance(16 * MINUTE_MS);
    until_the_page_is_erased(&hub, &name).await;
    answers.push(("expired, swept", shape(&page(&hub, &handle).await)));

    // Cancelled by a new invite.
    let name = hub.hermes().await;
    let (_, old) = hub.invite_with_page(&name).await;
    let (_, _new) = hub.invite_with_page(&name).await;
    answers.push(("cancelled", shape(&page(&hub, &old).await)));

    // Approved, and denied.
    let name = hub.hermes().await;
    let (invite, approved) = hub.invite_with_page(&name).await;
    let device = hub.device();
    let Enrolled::Code(code) = device.enroll(&invite, device::declared()).await.unwrap() else {
        panic!("refused");
    };
    hub.approve(&name, &code.to_string()).await;
    answers.push(("approved", shape(&page(&hub, &approved).await)));
    let name = hub.hermes().await;
    let (invite, denied) = hub.invite_with_page(&name).await;
    hub.device()
        .enroll(&invite, device::declared())
        .await
        .unwrap();
    hub.admin.deny(&name, hub.now()).await.unwrap();
    answers.push(("denied", shape(&page(&hub, &denied).await)));

    for (what, answer) in &answers {
        assert_eq!(answer, &unknown, "{what}");
    }
}

/// One request for one Hermes: a new invite kills the page of the old one, whether the old
/// invite was still issued or already used with a request waiting.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_invite_for_the_same_hermes_kills_the_old_page() {
    let hub = served().await;
    // The old invite is still issued.
    let name = hub.hermes().await;
    let (_, old) = hub.invite_with_page(&name).await;
    assert_eq!(page(&hub, &old).await.status, 200);
    let (_, new) = hub.invite_with_page(&name).await;
    assert!(is_the_404(&page(&hub, &old).await));
    assert_eq!(page(&hub, &new).await.status, 200);

    // The old invite was used and its request waits for the code.
    let name = hub.hermes().await;
    let (invite, old) = hub.invite_with_page(&name).await;
    hub.device()
        .enroll(&invite, device::declared())
        .await
        .unwrap();
    assert_eq!(page(&hub, &old).await.status, 200);
    let (_, new) = hub.invite_with_page(&name).await;
    assert!(is_the_404(&page(&hub, &old).await));
    assert_eq!(page(&hub, &new).await.status, 200);
    let present: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM invite i JOIN hermes h ON h.id = i.hermes_id \
         WHERE h.name = $1 AND i.join_digest IS NOT NULL",
    )
    .bind(&name)
    .fetch_one(&hub.pool)
    .await
    .unwrap();
    assert_eq!(present, 1, "only the page of the new invite is left");
}

/// The database holds the digest, never the handle: not its text, not its bytes, in any table.
#[tokio::test(flavor = "multi_thread")]
async fn the_handle_is_nowhere_in_clear_in_the_database() {
    let hub = served().await;
    let name = hub.hermes().await;
    let (invite, handle) = hub.invite_with_page(&name).await;
    let device = hub.device();
    device.enroll(&invite, device::declared()).await.unwrap();
    assert_eq!(page(&hub, &handle).await.status, 200);
    let text = handle.reveal();
    let bytes = BASE32_NOPAD
        .decode(text.to_ascii_uppercase().as_bytes())
        .unwrap();
    let spellings = [
        text.to_string(),
        text.to_ascii_uppercase(),
        HEXLOWER.encode(&bytes),
        HEXLOWER.encode(&bytes).to_ascii_uppercase(),
        BASE64.encode(&bytes),
        BASE64URL_NOPAD.encode(&bytes),
        BASE32_NOPAD.encode(&bytes),
    ];
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_type = 'BASE TABLE' \
           AND table_name <> '_sqlx_migrations'",
    )
    .fetch_all(&hub.pool)
    .await
    .unwrap();
    assert!(tables.iter().any(|table| table == "invite"));
    for table in tables {
        let rows: Option<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT string_agg(t::text, E'\\n' ORDER BY t::text) FROM {table} t"
        )))
        .fetch_one(&hub.pool)
        .await
        .unwrap();
        let rows = rows.unwrap_or_default();
        for spelling in &spellings {
            assert!(
                !rows.contains(spelling.as_str()),
                "{table} holds {spelling}"
            );
        }
    }
    // Only its digest is there, and it is that of the domain of the pages.
    let digest: Vec<u8> =
        sqlx::query_scalar("SELECT join_digest FROM invite WHERE join_digest IS NOT NULL")
            .fetch_one(&hub.pool)
            .await
            .unwrap();
    assert_eq!(digest, handle.digest().as_bytes().as_slice());
}

/// The page, the 404 and `llms.txt` hold none of the three values only the sentence carries.
#[tokio::test(flavor = "multi_thread")]
async fn the_page_the_404_and_llms_never_carry_a_secret() {
    let hub = served().await;
    let name = hub.hermes().await;
    let (invite, handle) = hub.invite_with_page(&name).await;
    let sha256 = hub.downloads.listed()[0].sha256.clone();
    let key = hub.hub_key_text();
    let bodies = [
        page(&hub, &handle).await.text(),
        get(&hub, &format!("/v1/join/{}", "a".repeat(26)))
            .await
            .text(),
        get(&hub, "/llms.txt").await.text(),
    ];
    for body in &bodies {
        for (what, secret) in [
            ("invite", invite.reveal().to_string()),
            ("hub key", key.clone()),
            ("digest", sha256.clone()),
            ("handle", handle.reveal().to_string()),
        ] {
            // The handle is in the address, never in a body.
            assert!(!body.contains(&secret), "{what} in a body");
        }
    }
    assert!(bodies[0].contains("by its form, not by its label"));
}

/// Only a GET reads a page; anything else is the empty 404 of today and touches nothing.
#[tokio::test(flavor = "multi_thread")]
async fn non_get_on_the_new_routes_gets_the_404_of_today() {
    let hub = served().await;
    let name = hub.hermes().await;
    let (_, handle) = hub.invite_with_page(&name).await;
    let before = rows_of_invite(&hub.pool).await;
    for path in [path_of(&handle), "/llms.txt".to_owned()] {
        for method in ["POST", "PUT", "DELETE", "HEAD"] {
            let body: &[u8] = if method == "HEAD" { b"" } else { b"x" };
            let answer = http::send(method, hub.addr(), &path, &[], body, PATIENCE)
                .await
                .unwrap();
            assert_eq!(answer.status, 404, "{method} {path}");
            assert!(answer.body.is_empty(), "{method} {path}");
        }
    }
    assert_eq!(rows_of_invite(&hub.pool).await, before);
    assert_eq!(page(&hub, &handle).await.status, 200);
}

/// The host in the page is the configured one: a `Host` of the request names nothing in it.
#[tokio::test(flavor = "multi_thread")]
async fn the_host_of_the_page_comes_from_the_configuration() {
    let hub = served().await;
    let name = hub.hermes().await;
    let (_, handle) = hub.invite_with_page(&name).await;
    let answer = http::get(
        hub.addr(),
        &path_of(&handle),
        &[
            ("Host", "evil.example.org"),
            ("X-Forwarded-Host", "evil.example.org"),
        ],
        PATIENCE,
    )
    .await
    .unwrap();
    assert_eq!(answer.status, 200);
    let text = answer.text();
    assert!(!text.contains("evil.example.org"));
    assert!(text.contains("`127.0.0.1:1`"), "the configured host");
    assert!(text.contains(&format!("http://127.0.0.1:1/v1/download/{CONNECTOR}")));
}

#[tokio::test(flavor = "multi_thread")]
async fn llms_txt_is_fixed() {
    let hub = served().await;
    let first = get(&hub, "/llms.txt").await;
    assert_eq!(first.status, 200);
    assert_eq!(
        first.header("content-type"),
        Some("text/markdown; charset=utf-8")
    );
    assert_eq!(first.header("cache-control"), Some("no-store"));
    assert_eq!(first.header("x-robots-tag"), Some("noindex"));
    // The text of the file, and the source line only when the workspace names one.
    assert!(first.text().starts_with(LLMS));
    assert_eq!(first.text().lines().count(), LLMS.lines().count());
    // It does not depend on the request, nor on the invites.
    let name = hub.hermes().await;
    hub.invite_with_page(&name).await;
    let other = http::get(
        hub.addr(),
        "/llms.txt",
        &[("Host", "evil.example.org")],
        PATIENCE,
    )
    .await
    .unwrap();
    assert_eq!(shape(&other), shape(&first));
    assert!(!first.text().contains("127.0.0.1"));
    // The root and the downloads stay as they were.
    assert!(
        get(&hub, "/")
            .await
            .text()
            .starts_with("This address is for the Arauto connectors")
    );
}

/// Without a connector to point at, the hub serves no page: the 404, even for a live invite.
#[tokio::test(flavor = "multi_thread")]
async fn a_hub_without_a_connector_serves_no_page() {
    let hub = start(200, Arc::new(SystemClock), false).await;
    let name = hub.hermes().await;
    let (_, handle) = hub.invite_with_page(&name).await;
    assert!(is_the_404(&page(&hub, &handle).await));
    assert_eq!(get(&hub, "/llms.txt").await.status, 200);
}

/// The ceiling on reads at once: with no read to spare, a live page is answered "busy", with
/// the fixed text, and nothing is written. With one read to spare, the same page serves.
#[tokio::test(flavor = "multi_thread")]
async fn a_read_over_the_ceiling_is_answered_busy_and_changes_nothing() {
    let mut settings = fixture::settings();
    settings.join_lookups_max = 0;
    let mut hub = Fixture::stopped(install().await, settings, Arc::new(SystemClock)).await;
    hub.downloads = Downloads::empty()
        .with_file(CONNECTOR, Bytes::from_static(b"the connector"))
        .unwrap();
    hub.start().await;
    let name = hub.hermes().await;
    let (_, handle) = hub.invite_with_page(&name).await;
    let before = rows_of_invite(&hub.pool).await;
    let busy = page(&hub, &handle).await;
    assert_eq!(busy.status, 503);
    assert_eq!(busy.text(), "The hub is busy. Try again in a minute.\n");
    assert_eq!(busy.header("cache-control"), Some("no-store"));
    assert_eq!(rows_of_invite(&hub.pool).await, before);
    // A malformed handle never takes a place: it is the 404, not the "busy".
    assert!(is_the_404(&get(&hub, "/v1/join/x").await));

    let hub = served().await;
    let name = hub.hermes().await;
    let (_, handle) = hub.invite_with_page(&name).await;
    assert_eq!(page(&hub, &handle).await.status, 200);
}
