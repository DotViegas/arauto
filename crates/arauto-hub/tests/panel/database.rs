//! The first passkey of the installation replaces the provisional password of the database
//! role, when `ARAUTO_DATABASE_PASSWORD=rotate-at-first-access` asks for it: once, without a
//! restart, and without a word of it on any page.
//!
//! The hub connects as a role of its own ([`crate::role`]): the role of the test server is
//! shared by every test.

use std::sync::Arc;

use arauto_hub::channel::{self, Downloads, Options, Served};
use arauto_hub::config::{ClientIpSource, PasswordRotation, ServeConfig};
use arauto_hub::db_password::{DATABASE_SECRET_FILE, DATABASE_SECRET_NEXT_FILE};
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support;
use arauto_testkit::hub::support::PANEL_ORIGIN;
use arauto_testkit::passkey::SoftAuthenticator;
use sqlx::ConnectOptions;

use crate::role::{self, Role};
use crate::{Browser, audited};

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

    // The first access: the code, then the passkey. The page answers as always.
    let (cookie, _) = browser.first_access(&fixture).await;
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
    let (mut fixture, role, _) = hub_as(PasswordRotation::Never).await;
    let browser = Browser::of(&fixture);
    let before = role.verifier(&fixture.pool).await;

    browser.first_access(&fixture).await;
    assert_eq!(kept(&fixture), None);
    assert_eq!(role.verifier(&fixture.pool).await, before);
    assert_eq!(audited(&fixture, "database.password_replaced").await, 0);
    assert_eq!(in_use(&fixture), [Some(PROVISIONAL.to_owned())]);

    fixture.stop().await;
    role.drop(&fixture.pool).await;
}
