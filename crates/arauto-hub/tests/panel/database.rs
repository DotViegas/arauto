//! The first passkey of the installation replaces the provisional password of the database
//! role, when `ARAUTO_DATABASE_PASSWORD=rotate-at-first-access` asks for it: once, without a
//! restart, and without a word of it on any page.
//!
//! The hub connects as a role of its own ([`crate::role`]): the role of the test server is
//! shared by every test.

use std::sync::Arc;

use arauto_hub::channel::{self, Downloads, Options, Served};
use arauto_hub::config::{ClientIpSource, PasswordRotation, ServeConfig};
use arauto_hub::db_password::{
    DATABASE_SECRET_FILE, DATABASE_SECRET_LOCK_FILE, DATABASE_SECRET_NEXT_FILE,
};
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support;
use arauto_testkit::hub::support::PANEL_ORIGIN;
use arauto_testkit::passkey::SoftAuthenticator;
use sqlx::ConnectOptions;

use crate::role::{self, Role};
use crate::{Browser, audited};

/// The row of the database password in the check window: its state and what it says. The window
/// needs a panel session that reaches the whole panel.
async fn window_row(browser: Browser, cookie: &str) -> String {
    let answer = browser.get("/installation", Some(cookie)).await;
    assert_eq!(answer.status, 200, "{}", answer.text());
    let page = answer.text();
    let at = page
        .find("<th scope=\"row\">Database password</th>")
        .expect("the window has the row of the database password");
    page[at..at + page[at..].find("</tr>").unwrap()].to_owned()
}

/// The same row in what `arauto-hub admin check` prints: the line with the state and the lines
/// under it.
async fn command_row(fixture: &Fixture, config: &ServeConfig) -> String {
    let env = |_: &str| -> Option<String> { None };
    let rows = fixture
        .admin
        .check(&config.storage, &[], &env)
        .await
        .unwrap();
    let printed = arauto_hub::check::print(&rows);
    let lines: Vec<&str> = printed.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.ends_with("] Database password"))
        .expect("the command prints the row of the database password");
    lines[at..=at + 1].join(" ")
}

/// Letters only: the options of a connection give it back percent-encoded.
const PROVISIONAL: &str = "ProvisionalPasswordOfTheInstallation";

/// A hub of its own whose database role is `role`, with the panel.
async fn hub_as(rotation: PasswordRotation) -> (Fixture, Role, ServeConfig) {
    let mut fixture = Fixture::stopped(
        support::install().await,
        fixture::settings(),
        Arc::new(arauto_protocol::clock::SystemClock),
    )
    .await;
    let role = role::create(&fixture.pool, PROVISIONAL).await;
    let mut config = support::config(&fixture.installation, ClientIpSource::Socket);
    config.storage.database_url = role.url(&fixture.installation.url, Some(PROVISIONAL));
    config.storage.password_rotation = rotation;
    start(&mut fixture, &config).await;
    (fixture, role, config)
}

async fn start(fixture: &mut Fixture, config: &ServeConfig) {
    let running = channel::start(
        config,
        Options {
            settings: fixture.settings.clone(),
            downloads: Downloads::empty(),
            purposes: Served,
        },
        Arc::clone(&fixture.clock),
    )
    .await
    .unwrap();
    fixture.running = Some(running);
}

fn kept(fixture: &Fixture) -> Option<String> {
    std::fs::read_to_string(fixture.installation.data_dir.join(DATABASE_SECRET_FILE)).ok()
}

/// The password the pools of the running hub open their next connections with.
fn in_use(fixture: &Fixture) -> Vec<Option<String>> {
    let hub = fixture.running().hub();
    [hub.pool()]
        .iter()
        .map(|pool| {
            pool.connect_options()
                .to_url_lossy()
                .password()
                .map(str::to_owned)
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_first_passkey_replaces_the_provisional_password_once_and_without_a_restart() {
    let (mut fixture, role, config) = hub_as(PasswordRotation::AtFirstAccess).await;
    let browser = Browser::of(&fixture);
    let before = role.verifier(&fixture.pool).await;
    assert!(role::opens(&before, PROVISIONAL));
    assert_eq!(kept(&fixture), None);
    // Asked for, and not yet: nothing failed, the replacement comes with the first passkey.
    let command = command_row(&fixture, &config).await;
    assert!(
        command.starts_with("[later] Database password"),
        "{command}"
    );

    // The first access: the code, then the passkey. The page answers as always.
    let (cookie, _) = browser.first_access(&fixture).await;
    // Replaced: the window and the command say so, from the file the hub kept.
    let row = window_row(browser, &cookie).await;
    assert!(
        row.contains(">ok<") && row.contains("Replaced by one nobody saw."),
        "{row}"
    );
    let command = command_row(&fixture, &config).await;
    assert!(command.starts_with("[ok] Database password"), "{command}");
    let kept_now = kept(&fixture).expect("the password is kept in the data folder");
    assert_eq!(kept_now.len(), 64);
    assert!(
        !fixture
            .installation
            .data_dir
            .join(DATABASE_SECRET_NEXT_FILE)
            .exists()
    );
    let after = role.verifier(&fixture.pool).await;
    assert!(role::opens(&after, &kept_now));
    assert!(!role::opens(&after, PROVISIONAL));
    assert_eq!(audited(&fixture, "database.password_replaced").await, 1);
    // The running hub uses it from now on, with no restart, and the panel goes on.
    assert_eq!(in_use(&fixture), [Some(kept_now.clone())]);
    let page = browser.get("/", Some(&cookie)).await;
    assert_eq!(page.status, 200);
    assert!(!page.text().contains(&kept_now));

    // A second passkey replaces nothing: the hub never does it twice by itself.
    let entered = browser.entered(&fixture).await;
    let mut second = SoftAuthenticator::new(PANEL_ORIGIN);
    let registered = browser.register(&entered, &mut second).await;
    assert_eq!(registered.status, 303, "{}", registered.text());
    assert_eq!(kept(&fixture).as_deref(), Some(kept_now.as_str()));
    assert_eq!(role.verifier(&fixture.pool).await, after);
    assert_eq!(audited(&fixture, "database.password_replaced").await, 1);

    // A restart opens the database with the kept password, from the same configuration.
    fixture.stop().await;
    start(&mut fixture, &config).await;
    assert_eq!(in_use(&fixture), [Some(kept_now.clone())]);
    // The panel listens on a port of its own again.
    let browser = Browser::of(&fixture);
    assert_eq!(browser.pass(&mut second).await.status, 303);

    fixture.stop().await;
    role.drop(&fixture.pool).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_configuration_the_first_passkey_replaces_nothing() {
    let (mut fixture, role, config) = hub_as(PasswordRotation::Never).await;
    let browser = Browser::of(&fixture);
    let before = role.verifier(&fixture.pool).await;

    let (cookie, _) = browser.first_access(&fixture).await;
    // Not asked for: the window and the command say the installation keeps its own password.
    let row = window_row(browser, &cookie).await;
    assert!(
        row.contains(">check<") && row.contains("Not replaced by the hub"),
        "{row}"
    );
    let command = command_row(&fixture, &config).await;
    assert!(
        command.starts_with("[check] Database password"),
        "{command}"
    );
    assert_eq!(kept(&fixture), None);
    assert_eq!(role.verifier(&fixture.pool).await, before);
    assert_eq!(audited(&fixture, "database.password_replaced").await, 0);
    assert_eq!(in_use(&fixture), [Some(PROVISIONAL.to_owned())]);

    fixture.stop().await;
    role.drop(&fixture.pool).await;
}

/// A first passkey whose audit mark could not follow (another process holds the lock of the
/// mark) still replaces the password: the mark of the first access is set, so no later passkey
/// would ever replace it.
#[tokio::test(flavor = "multi_thread")]
async fn a_first_passkey_the_audit_mark_did_not_follow_still_replaces_the_password() {
    let (mut fixture, role, config) = hub_as(PasswordRotation::AtFirstAccess).await;
    let browser = Browser::of(&fixture);
    let cookie = browser.entered(&fixture).await;
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    let asked = browser
        .post("/passkey/register/start", Some(&cookie), "")
        .await;
    let made = authenticator.create(&asked.text()).unwrap();

    let other_process = arauto_platform::FileLock::try_acquire(
        &fixture
            .installation
            .data_dir
            .join(arauto_hub::mark::MARK_LOCK_FILE),
    )
    .unwrap();
    // The mark waits for its lock twice: the passkey, then the replacement.
    let headers = [
        ("Host", support::PANEL_HOST),
        ("Origin", PANEL_ORIGIN),
        ("Content-Type", "application/x-www-form-urlencoded"),
        ("Cookie", cookie.as_str()),
    ];
    let answered = arauto_testkit::http::post(
        browser.panel,
        "/passkey/register",
        &headers,
        format!("credential={}", crate::form_value(&made)).as_bytes(),
        std::time::Duration::from_secs(60),
    )
    .await
    .unwrap();
    drop(other_process);
    // The passkey is kept and the person lands on the check window, not on a bare 500.
    assert_eq!(
        (answered.status, answered.header("location")),
        (303, Some("/installation?welcome")),
        "{}",
        answered.text()
    );

    let kept_now = kept(&fixture).expect("the password was replaced all the same");
    // The mark of the first access is set, so the row says replaced, not "later".
    let row = window_row(browser, &cookie).await;
    assert!(
        row.contains(">ok<") && row.contains("Replaced by one nobody saw."),
        "{row}"
    );
    let command = command_row(&fixture, &config).await;
    assert!(command.starts_with("[ok] Database password"), "{command}");
    let after = role.verifier(&fixture.pool).await;
    assert!(role::opens(&after, &kept_now));
    assert!(!role::opens(&after, PROVISIONAL));
    assert_eq!(in_use(&fixture), [Some(kept_now)]);

    fixture.stop().await;
    role.drop(&fixture.pool).await;
}

/// The replacement was asked for and did not happen (another process holds the lock of the
/// password for longer than the hub waits): the passkey is kept all the same, and the window
/// and the command say it failed and name the command that does it later.
#[tokio::test(flavor = "multi_thread")]
async fn a_replacement_that_could_not_happen_is_a_failed_row() {
    let (mut fixture, role, config) = hub_as(PasswordRotation::AtFirstAccess).await;
    let browser = Browser::of(&fixture);
    let cookie = browser.entered(&fixture).await;
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    let asked = browser
        .post("/passkey/register/start", Some(&cookie), "")
        .await;
    let made = authenticator.create(&asked.text()).unwrap();

    let other_process = arauto_platform::FileLock::try_acquire(
        &fixture
            .installation
            .data_dir
            .join(DATABASE_SECRET_LOCK_FILE),
    )
    .unwrap();
    let headers = [
        ("Host", support::PANEL_HOST),
        ("Origin", PANEL_ORIGIN),
        ("Content-Type", "application/x-www-form-urlencoded"),
        ("Cookie", cookie.as_str()),
    ];
    let answered = arauto_testkit::http::post(
        browser.panel,
        "/passkey/register",
        &headers,
        format!("credential={}", crate::form_value(&made)).as_bytes(),
        std::time::Duration::from_secs(60),
    )
    .await
    .unwrap();
    drop(other_process);
    assert_eq!(answered.status, 303, "{}", answered.text());
    assert_eq!(kept(&fixture), None, "nothing was replaced");

    let row = window_row(browser, &cookie).await;
    assert!(
        row.contains(">failed<")
            && row.contains("The replacement failed.")
            && row.contains("arauto-hub admin database rotate-password"),
        "{row}"
    );
    let command = command_row(&fixture, &config).await;
    assert!(
        command.starts_with("[failed] Database password"),
        "{command}"
    );

    fixture.stop().await;
    role.drop(&fixture.pool).await;
}
