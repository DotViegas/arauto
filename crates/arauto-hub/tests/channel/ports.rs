//! The two ports (`docs/14-protocolo-do-canal.md`, section 1; `docs/15-mvp.md`, section 6):
//! health only on the internal one, the downloads only from the fixed list, and 404 for
//! anything else.

use std::time::Duration;

use arauto_hub::channel::{BadDownloadName, Downloads};
use arauto_hub::config::ClientIpSource;
use arauto_testkit::http;
use bytes::Bytes;

use crate::support::{self, Behavior, TestPurposes};

const PATIENCE: Duration = Duration::from_secs(10);
const LISTED: &str = "arauto-connector-0.1.0-x86_64-linux";

#[tokio::test(flavor = "multi_thread")]
async fn health_answers_only_on_the_internal_port_and_says_nothing_else() {
    let hub = support::start(support::settings(), TestPurposes(Behavior::Park)).await;
    let internal = hub.running.internal_addr();
    let answer = http::get(internal, "/health", &[], PATIENCE).await.unwrap();
    assert_eq!(answer.status, 200);
    assert_eq!(answer.body, b"ok\n");
    assert_eq!(
        http::get(hub.addr(), "/health", &[], PATIENCE)
            .await
            .unwrap()
            .status,
        404
    );
    // The internal port has nothing of the connectors.
    for path in [
        "/v1/channel",
        &format!("/v1/download/{LISTED}"),
        "/",
        "/health/",
    ] {
        let answer = http::get(internal, path, &[], PATIENCE).await.unwrap();
        assert_eq!(answer.status, 404, "{path}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_download_is_served_only_by_a_name_of_the_list() {
    let content = Bytes::from_static(b"\x7fELF connector");
    let downloads = Downloads::empty()
        .with_file(LISTED, content.clone())
        .unwrap();
    let hub = support::start_with(
        support::settings(),
        TestPurposes(Behavior::Park),
        ClientIpSource::Socket,
        downloads,
    )
    .await;
    let answer = http::get(hub.addr(), &format!("/v1/download/{LISTED}"), &[], PATIENCE)
        .await
        .unwrap();
    assert_eq!((answer.status, answer.body.as_slice()), (200, &content[..]));
    for path in [
        "/v1/download/",
        "/v1/download/../Cargo.toml",
        "/v1/download/..%2FCargo.toml",
        "/v1/download/%2e%2e%2fCargo.toml",
        "/v1/download/..%5CCargo.toml",
        "/v1/download//etc/passwd",
        "/v1/download/%2Fetc%2Fpasswd",
        "/v1/download/C:%5CWindows%5Cwin.ini",
        "/v1/download/arauto-connector-0.1.0-x86_64-linux/",
        "/v1/download/arauto-connector-0.1.0-x86_64-linux%00",
        "/v1/download/ARAUTO-CONNECTOR-0.1.0-X86_64-LINUX",
        "/v1/download/sub/arauto-connector-0.1.0-x86_64-linux",
        "/v1/download/arauto-connector-0.1.0-x86_64-linu%78",
        "/v1/downloads/arauto-connector-0.1.0-x86_64-linux",
        "/v1/channel/",
    ] {
        let answer = http::get(hub.addr(), path, &[], PATIENCE).await.unwrap();
        assert_eq!(answer.status, 404, "{path}");
    }
}

/// R13: whoever opens the address of the connectors in a browser is told what it is, with a
/// fixed text that names no host, no port and no version.
#[tokio::test(flavor = "multi_thread")]
async fn the_root_of_the_connectors_says_it_is_not_the_panel_and_names_nothing() {
    let hub = support::start(support::settings(), TestPurposes(Behavior::Park)).await;
    let answer = http::get(hub.addr(), "/", &[], PATIENCE).await.unwrap();
    assert_eq!(answer.status, 200);
    assert_eq!(
        answer.header("content-type"),
        Some("text/plain; charset=utf-8")
    );
    assert_eq!(answer.header("cache-control"), Some("no-store"));
    assert_eq!(answer.header("x-content-type-options"), Some("nosniff"));
    let text = answer.text();
    let expected = [
        "This address is for the Arauto connectors: there is nothing to open here.",
        "The panel of this hub has its own address, the one set in ARAUTO_PANEL_URL.",
        "If the Open button of Easypanel brought you here, point that domain to the port of the panel.",
    ]
    .join("\n")
        + "\n";
    assert_eq!(text, expected);
    assert_eq!(text.lines().count(), 3);
    // Names no host, no port and no version.
    let port = hub.addr().port().to_string();
    assert!(!text.contains(&port) && !text.contains("127.0.0.1") && !text.contains("0.1.0"));
    // Other paths and methods stay 404, and the internal port has nothing there.
    for path in ["/v1/channel/", "/index.html", "//"] {
        let other = http::get(hub.addr(), path, &[], PATIENCE).await.unwrap();
        assert_eq!(other.status, 404, "{path}");
    }
    let posted = http::post(hub.addr(), "/", &[], b"", PATIENCE)
        .await
        .unwrap();
    assert_eq!(posted.status, 404);
    let internal = http::get(hub.running.internal_addr(), "/", &[], PATIENCE)
        .await
        .unwrap();
    assert_eq!(internal.status, 404);
}

/// The first milestone has no file to serve.
#[tokio::test(flavor = "multi_thread")]
async fn the_list_of_the_first_milestone_is_empty() {
    let hub = support::start(support::settings(), TestPurposes(Behavior::Park)).await;
    let answer = http::get(hub.addr(), &format!("/v1/download/{LISTED}"), &[], PATIENCE)
        .await
        .unwrap();
    assert_eq!(answer.status, 404);
}

#[test]
fn a_name_outside_the_narrow_format_never_enters_the_list() {
    for name in [
        "",
        ".hidden",
        "..",
        "../x",
        "a/b",
        "a\\b",
        "a b",
        "a%2e",
        "é",
        &"a".repeat(129),
    ] {
        assert_eq!(
            Downloads::empty()
                .with_file(name, Bytes::new())
                .unwrap_err(),
            BadDownloadName,
            "{name}"
        );
    }
    assert!(Downloads::empty().with_file(LISTED, Bytes::new()).is_ok());
}
