//! The admin command, from `init` to `revoke`, calling the same functions the program calls;
//! what it does with a database that came back from a copy; and the program itself.

use std::path::{Path, PathBuf};
use std::process::Command;

use arauto_hub::admin::render;
use arauto_hub::admin::{self, AdminError, InitOutcome, Session};
use arauto_hub::config::PublicUrl;
use arauto_hub::core::model::DeviceState;
use arauto_hub::db::{self, SchemaState};
use arauto_hub::mark::{Mark, MarkValue};
use arauto_hub::repo::alert::AlertFilter;
use arauto_hub::repo::approval::{ApproveOutcome, DenyOutcome, Recognition, Recognized};
use arauto_hub::repo::connection;
use arauto_hub::repo::enroll::{self, EnrollOutcome, Enrollment, RefusedEnrollments};
use arauto_hub::repo::hermes::WakeOutcome;
use arauto_hub::repo::swap::{self, ConfirmOutcome, IssueOutcome, Stranger, UnknownOutcome};
use arauto_hub::restore::{self, EnterOutcome, Evidence, Standing};
use arauto_hub::vault::{MASTER_KEY_FILE, Vault};
use arauto_protocol::clock::{Clock, SystemClock};
use arauto_protocol::code::VerificationCode;
use arauto_protocol::invite::Invite as InviteSecret;
use arauto_protocol::message::{Declared, DeclaredText, Generation, Saved};
use arauto_protocol::names::NameError;
use arauto_protocol::noise::{HandshakeHash, PublicKey};
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::common::{NOW, TestDb};
use crate::support::{address, another_code, identity};

fn os_user() -> DeclaredText {
    DeclaredText::parse("operator").unwrap()
}

/// A database with nothing in it and a data folder that does not exist yet.
struct Fresh {
    db: TestDb,
    dir: tempfile::TempDir,
}

impl Fresh {
    async fn new() -> Self {
        Self {
            db: TestDb::empty().await,
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn data_dir(&self) -> PathBuf {
        self.dir.path().join("data")
    }

    fn mark(&self) -> Mark {
        Mark::new(&self.data_dir())
    }

    async fn init(&self) -> Result<InitOutcome, AdminError> {
        admin::init(&self.db.pool, &self.data_dir(), &os_user(), false, NOW).await
    }

    async fn session(&self) -> Session {
        Session::open(self.db.pool.clone(), self.mark(), os_user())
            .await
            .unwrap()
    }
}

async fn count(pool: &PgPool, table: &'static str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Every row of every table that the admin command writes, as one text.
async fn everything(pool: &PgPool) -> String {
    let mut all = String::new();
    for table in [
        "installation",
        "hub_key",
        "organization",
        "admin",
        "hermes",
        "device",
        "invite",
        "audit",
        "alert",
        "peer_link",
    ] {
        let rows: Option<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT string_agg(t::text, E'\\n' ORDER BY t::text) FROM {table} t"
        )))
        .fetch_one(pool)
        .await
        .unwrap();
        all.push_str(&rows.unwrap_or_default());
    }
    all
}

/// What a connector does with an invite: the enrollment, through the entry function that
/// finds the organization by the digest of the invite.
async fn enroll(
    pool: &PgPool,
    mark: &Mark,
    invite: &str,
    who: &PublicKey,
) -> (Uuid, VerificationCode) {
    enroll_at(pool, mark, invite, who, NOW).await
}

/// [`enroll`] at a given time: the program itself runs on the clock of the system.
async fn enroll_at(
    pool: &PgPool,
    mark: &Mark,
    invite: &str,
    who: &PublicKey,
    now: i64,
) -> (Uuid, VerificationCode) {
    let invite = InviteSecret::parse(invite).unwrap();
    let organization = db::organization_of_invite(pool, &invite.digest())
        .await
        .unwrap()
        .expect("the invite belongs to an organization");
    let hash =
        HandshakeHash::from_bytes(*Uuid::new_v4().as_bytes().repeat(2).first_chunk().unwrap());
    let nonce = enroll::draw_nonce(&hash).unwrap();
    let declared = Declared {
        hostname: DeclaredText::parse("build-7").unwrap(),
        os: DeclaredText::parse("linux").unwrap(),
        hermes_version: DeclaredText::parse("1.2.3").unwrap(),
    };
    let outcome = enroll::enroll(
        pool,
        mark,
        organization,
        &RefusedEnrollments::new(),
        &invite.digest(),
        &Enrollment {
            identity: who,
            declared: &declared,
            connector_version: &DeclaredText::parse("0.1.0").unwrap(),
            origin: address(31),
            nonce: &nonce,
            handshake_hash: &hash,
        },
        now,
    )
    .await
    .unwrap();
    let EnrollOutcome::Enrolled(accepted) = outcome else {
        panic!("the enrollment was refused: {outcome:?}");
    };
    (
        accepted.device,
        VerificationCode::derive(&nonce, &hash).unwrap(),
    )
}

/// The line `invite:  arc1...` of the block.
fn invite_in(block: &str) -> String {
    block
        .lines()
        .find_map(|line| line.strip_prefix("invite:"))
        .expect("the block shows the invite")
        .trim()
        .to_owned()
}

// ---------------------------------------------------------------------------- init

#[tokio::test]
async fn init_installs_once_and_does_not_redo_what_exists() {
    let fresh = Fresh::new().await;
    assert_eq!(
        db::schema_state(&fresh.db.pool, &db::MIGRATOR)
            .await
            .unwrap(),
        SchemaState::Empty
    );

    let first = fresh.init().await.unwrap();
    let InitOutcome::Created(installed) = first else {
        panic!("not created: {first:?}");
    };
    // The master key and the mark are beside each other, outside the database.
    assert!(fresh.data_dir().join(MASTER_KEY_FILE).is_file());
    let mark = fresh.mark().read().await.unwrap();
    let last_security: i64 = sqlx::query_scalar("SELECT max(seq) FROM audit WHERE security")
        .fetch_one(&fresh.db.pool)
        .await
        .unwrap();
    assert_eq!(mark, MarkValue::At(last_security));
    for (table, rows) in [
        ("installation", 1),
        ("organization", 1),
        ("admin", 1),
        ("hub_key", 1),
        ("audit", 4),
    ] {
        assert_eq!(count(&fresh.db.pool, table).await, rows, "{table}");
    }
    // The key of the hub is written the one way it is ever written, and opens with the
    // master key of the folder.
    let text = installed.hub_key.to_text();
    assert!(text.starts_with("arh1") && text.len() == 56, "{text}");
    assert_eq!(PublicKey::parse_text(&text).unwrap(), installed.hub_key);
    let vault = Vault::open(&fresh.data_dir()).unwrap();
    let pair = admin::hub_keypair(&fresh.db.pool, &vault).await.unwrap();
    assert_eq!(pair.public(), &installed.hub_key);
    let events: Vec<(String, String, Option<String>, bool)> =
        sqlx::query_as("SELECT action, actor_kind, actor_note, security FROM audit ORDER BY seq")
            .fetch_all(&fresh.db.pool)
            .await
            .unwrap();
    let event = |action: &str| {
        (
            action.to_owned(),
            "admin".to_owned(),
            Some("operator".to_owned()),
            true,
        )
    };
    assert_eq!(
        events,
        [
            event("installation.created"),
            event("organization.created"),
            event("admin.created"),
            event("hub_key.created"),
        ]
    );

    // Again: nothing is redone.
    let before = everything(&fresh.db.pool).await;
    let key_file = std::fs::read(fresh.data_dir().join(MASTER_KEY_FILE)).unwrap();
    let second = fresh.init().await.unwrap();
    assert_eq!(second, InitOutcome::AlreadyInstalled(installed));
    assert_eq!(everything(&fresh.db.pool).await, before);
    assert_eq!(
        std::fs::read(fresh.data_dir().join(MASTER_KEY_FILE)).unwrap(),
        key_file
    );
    assert_eq!(fresh.mark().read().await.unwrap(), mark);
    assert!(render::init(&second).text.contains("already installed"));
}

#[tokio::test]
async fn a_start_installs_an_empty_database_and_touches_no_other() {
    let install = |fresh: &Fresh| {
        let (pool, dir) = (fresh.db.pool.clone(), fresh.data_dir());
        async move { admin::install_if_new(&pool, &dir, &os_user(), false, NOW).await }
    };

    // An empty database: the start does what `init` does, once.
    let fresh = Fresh::new().await;
    let installed = install(&fresh).await.unwrap().expect("it installs itself");
    assert!(fresh.data_dir().join(MASTER_KEY_FILE).is_file());
    assert_eq!(count(&fresh.db.pool, "installation").await, 1);
    assert_eq!(count(&fresh.db.pool, "audit").await, 4);
    let vault = Vault::open(&fresh.data_dir()).unwrap();
    let pair = admin::hub_keypair(&fresh.db.pool, &vault).await.unwrap();
    assert_eq!(pair.public(), &installed.hub_key);
    // The next start finds an installation and does nothing.
    let before = everything(&fresh.db.pool).await;
    assert_eq!(install(&fresh).await.unwrap(), None);
    assert_eq!(everything(&fresh.db.pool).await, before);

    // A database with data and no master key is not an empty one: nothing is created over
    // it, and no new key appears beside it.
    let key_path = fresh.data_dir().join(MASTER_KEY_FILE);
    std::fs::remove_file(&key_path).unwrap();
    assert_eq!(install(&fresh).await.unwrap(), None);
    assert!(!key_path.exists(), "a start created a master key over data");
    assert_eq!(everything(&fresh.db.pool).await, before);

    // An empty database beside the mark of another installation is refused, as in `init`.
    let other = Fresh::new().await;
    std::fs::create_dir_all(other.data_dir()).unwrap();
    other.mark().advance(7).await.unwrap();
    let refused = install(&other).await;
    assert!(
        matches!(
            refused,
            Err(AdminError::MarkOfAnotherInstallation { mark: 7 })
        ),
        "{refused:?}"
    );
    assert_eq!(count(&other.db.pool, "installation").await, 0);
}

#[tokio::test]
async fn init_refuses_a_database_with_data_whose_master_key_is_gone() {
    let fresh = Fresh::new().await;
    fresh.init().await.unwrap();
    let before = everything(&fresh.db.pool).await;
    let key_path = fresh.data_dir().join(MASTER_KEY_FILE);
    std::fs::remove_file(&key_path).unwrap();

    let refused = fresh.init().await;
    let Err(AdminError::MasterKeyMissing { path }) = &refused else {
        panic!("a new key was accepted over existing data: {refused:?}");
    };
    assert_eq!(path, &key_path);
    assert!(!key_path.exists(), "no new master key was created");
    assert_eq!(everything(&fresh.db.pool).await, before);

    // A key that is not the one of this database is refused too.
    let elsewhere = tempfile::tempdir().unwrap();
    Vault::create(&elsewhere.path().join("data")).unwrap();
    std::fs::copy(
        elsewhere.path().join("data").join(MASTER_KEY_FILE),
        &key_path,
    )
    .unwrap();
    let refused = fresh.init().await;
    assert!(matches!(refused, Err(AdminError::Vault(_))), "{refused:?}");
    assert_eq!(everything(&fresh.db.pool).await, before);
}

#[tokio::test]
async fn init_does_not_erase_the_proof_of_a_restored_database() {
    // A database with data and no mark came back from a copy. `init` leaves that as it is.
    let fresh = Fresh::new().await;
    fresh.init().await.unwrap();
    fresh.mark().erase().await.unwrap();
    assert!(matches!(
        fresh.init().await.unwrap(),
        InitOutcome::AlreadyInstalled(_)
    ));
    assert_eq!(fresh.mark().read().await.unwrap(), MarkValue::Absent);
    let status = admin::status(&fresh.db.pool, &fresh.mark()).await.unwrap();
    assert_eq!(
        status.standing,
        Some(Standing::Restored(Evidence::MarkAbsent))
    );

    // An empty database beside a mark that already counted events: the mark is of another
    // installation, and nothing is installed over it.
    let other = Fresh::new().await;
    std::fs::create_dir_all(other.data_dir()).unwrap();
    other.mark().advance(9).await.unwrap();
    let refused = other.init().await;
    assert!(
        matches!(
            refused,
            Err(AdminError::MarkOfAnotherInstallation { mark: 9 })
        ),
        "{refused:?}"
    );
    assert_eq!(count(&other.db.pool, "installation").await, 0);
    assert_eq!(count(&other.db.pool, "audit").await, 0);
    assert_eq!(other.mark().read().await.unwrap(), MarkValue::At(9));
}

#[tokio::test]
async fn migrate_asks_for_the_word_about_the_backup() {
    let fresh = Fresh::new().await;
    fresh.init().await.unwrap();
    let refused = admin::migrate(&fresh.db.pool, false, NOW).await;
    assert!(
        matches!(refused, Err(AdminError::BackupNotConfirmed)),
        "{refused:?}"
    );
    let text = refused.unwrap_err().to_string();
    assert!(
        text.contains("backup") && text.contains("--i-have-a-backup"),
        "{text}"
    );
    // With it, on a database already at the version of the program, there is nothing to do.
    assert_eq!(
        admin::migrate(&fresh.db.pool, true, NOW).await.unwrap(),
        Vec::<i64>::new()
    );
    assert_eq!(count(&fresh.db.pool, "audit").await, 4);
}

// ---------------------------------------------------------------------------- end to end

#[tokio::test]
async fn the_admin_command_from_init_to_revoke() {
    let fresh = Fresh::new().await;
    let pool = &fresh.db.pool;
    let mark = fresh.mark();
    // Before `init` there is nothing to administer.
    let refused = Session::open(pool.clone(), mark.clone(), os_user()).await;
    assert!(
        matches!(refused, Err(AdminError::Db(db::DbError::SchemaEmpty))),
        "{refused:?}"
    );

    let InitOutcome::Created(installed) = fresh.init().await.unwrap() else {
        panic!("not created");
    };
    let session = fresh.session().await;
    assert_eq!(session.organization(), installed.organization);

    // hermes add: the format and the reserved names.
    for (name, why) in [
        ("Vendas", NameError::FirstCharacter),
        ("vendas online", NameError::Character),
        ("", NameError::Length),
        ("admin", NameError::Reserved),
        ("hermes", NameError::Reserved),
    ] {
        let refused = session.hermes_add(name, NOW).await;
        assert!(
            matches!(&refused, Err(AdminError::InvalidName(found)) if *found == why),
            "{name:?}: {refused:?}"
        );
    }
    assert_eq!(count(pool, "hermes").await, 0);
    let hermes = session.hermes_add("vendas", NOW).await.unwrap();
    assert_eq!(hermes.name.as_str(), "vendas");
    assert!(matches!(
        session.hermes_add("vendas", NOW).await,
        Err(AdminError::NameTaken)
    ));
    let listed = render::hermes_list(&session.hermes_list(false).await.unwrap()).text;
    assert!(
        listed.contains("vendas") && listed.contains("no device"),
        "{listed}"
    );

    // invite: the address, the public key of the hub and the invite, which appears only here.
    let url = PublicUrl::parse("https://hub.example.com").unwrap();
    assert!(matches!(
        session.invite("nobody", &url, NOW).await,
        Err(AdminError::NoSuchHermes)
    ));
    let block = session.invite("vendas", &url, NOW).await.unwrap();
    let shown = render::block(&block, &[], NOW, render::Shown::Command).text;
    // With the connector to download, the block says where it is and what its file must be.
    let connector = arauto_hub::download::Listed {
        name: "arauto-connector-linux-x86_64".to_owned(),
        size: 3,
        sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned(),
    };
    let with_download = render::block(
        &block,
        std::slice::from_ref(&connector),
        NOW,
        render::Shown::Command,
    )
    .text;
    // The sentence is the default: the same invite, in the words the panel writes in English,
    // and none of the commands. Without a connector to point at, it is the block.
    let sentence = render::block(
        &block,
        std::slice::from_ref(&connector),
        NOW,
        render::Shown::Sentence,
    )
    .text;
    assert!(
        sentence.contains("Send this to the Hermes, as a new message in its command channel:"),
        "{sentence}"
    );
    assert!(
        sentence.contains(
            "I am asking you myself: connect this Hermes to our Arauto hub as \"vendas\"."
        ),
        "{sentence}"
    );
    assert!(
        sentence.contains("https://hub.example.com/v1/join/"),
        "{sentence}"
    );
    assert!(
        sentence.contains(&format!("Connector sha256: {}", connector.sha256)),
        "{sentence}"
    );
    assert!(
        sentence.contains(&format!(
            "until {} UTC): {}",
            // The deadline of the invite, to the minute.
            arauto_hub::join_page::utc_minute(block.issued.expires_at),
            block.issued.invite.reveal().as_str()
        )),
        "{sentence}"
    );
    // The hint and the closing line are whole, byte for byte: no run of spaces left where a
    // line break was, and the closing line is the block's own.
    assert!(
        sentence.contains(
            "\nSend it as a new message, not as a reply to one of its notices. If it asks you to \
             confirm, answer in your own words.\n"
        ),
        "{sentence}"
    );
    let closing = "The invite is shown only now. The Hermes answers a six-digit code: type it in \
                   `admin approve vendas`.\n";
    assert!(sentence.ends_with(closing), "{sentence}");
    assert!(with_download.ends_with(closing), "{with_download}");
    assert!(!sentence.contains("  "), "{sentence}");
    for command in ["curl", "sha256sum", "&&", "$HOME"] {
        assert!(!sentence.contains(command), "{command}: {sentence}");
    }
    assert_eq!(
        sentence
            .matches(block.issued.invite.reveal().as_str())
            .count(),
        1,
        "the invite appears once"
    );
    assert_eq!(
        render::block(&block, &[], NOW, render::Shown::Sentence).text,
        shown,
        "with no connector the text is the block's"
    );
    assert!(
        with_download.contains(
            "connector: https://hub.example.com/v1/download/arauto-connector-linux-x86_64"
        ),
        "{with_download}"
    );
    assert!(
        with_download.contains(&format!("sha256:    {}", connector.sha256)),
        "{with_download}"
    );
    // One chain: nothing runs unless the file matches the digest written in the block.
    let name = &connector.name;
    let chain = format!(
        "curl -fsSLo {name} \"https://hub.example.com/v1/download/{name}\" && echo \"{}  {name}\" \
         | sha256sum -c - && chmod 700 {name} && ./{name} join \"wss://hub.example.com/v1/channel\"",
        connector.sha256
    );
    assert!(with_download.contains(&chain), "{with_download}");
    assert!(!with_download.contains("| sh "), "{with_download}");
    assert!(!with_download.contains("| bash"), "{with_download}");
    assert!(
        !with_download.contains("\narauto-connector join"),
        "{with_download}"
    );
    assert!(!shown.contains("sha256"), "{shown}");
    let invite = invite_in(&shown);
    assert_eq!(invite, block.issued.invite.reveal().as_str());
    assert!(
        shown.contains("wss://hub.example.com/v1/channel"),
        "{shown}"
    );
    assert!(shown.contains(&installed.hub_key.to_text()), "{shown}");
    assert!(
        shown.contains(&format!(
            "arauto-connector join \"wss://hub.example.com/v1/channel\" \"{}\" \"{invite}\"",
            installed.hub_key.to_text()
        )),
        "{shown}"
    );
    assert!(
        !everything(pool).await.contains(&invite),
        "the invite is stored nowhere"
    );

    // The connector enrolls and shows a code; the admin sees the request, never the code.
    let who = identity();
    let (device, code) = enroll(pool, &mark, &invite, &who).await;
    let view = session.request("vendas").await.unwrap().unwrap();
    assert_eq!((view.request.id, view.replaces.clone()), (device, None));
    let request_shown = render::request("vendas", &view);
    assert!(request_shown.contains("build-7") && request_shown.contains("not verified"));
    assert!(request_shown.contains("192.0.2.31"), "{request_shown}");
    assert!(
        !request_shown.contains(&code.to_string()),
        "the code is never shown"
    );
    assert!(!request_shown.contains(&format!("{:06}", code.value())));
    let listed = render::hermes_list(&session.hermes_list(false).await.unwrap()).text;
    assert!(listed.contains("waiting for your code"), "{listed}");

    // approve: what is not six digits is not counted; a wrong code is; the right one approves.
    for not_a_code in ["", "12345", "1234567", "12a456", "123 456"] {
        assert!(
            matches!(
                session.approve("vendas", not_a_code, None, NOW).await,
                Err(AdminError::InvalidCode)
            ),
            "{not_a_code:?}"
        );
    }
    let wrong = another_code(code);
    let outcome = session
        .approve("vendas", &wrong.to_string(), None, NOW)
        .await
        .unwrap();
    assert_eq!(outcome, ApproveOutcome::WrongCode { attempts: 1 });
    let report = render::approved("vendas", &outcome);
    assert!(
        !report.done && report.text.contains("attempt 1 of 5"),
        "{}",
        report.text
    );
    // As the Hermes says it, with the hyphen, and with a line feed after it.
    let typed = format!("{code}\n");
    let outcome = session.approve("vendas", &typed, None, NOW).await.unwrap();
    assert!(
        matches!(outcome, ApproveOutcome::Approved(_)),
        "{outcome:?}"
    );
    assert!(render::approved("vendas", &outcome).done);
    assert_eq!(
        session.approve("vendas", &typed, None, NOW).await.unwrap(),
        ApproveOutcome::NoRequest
    );
    assert_eq!(
        session.deny("vendas", NOW).await.unwrap(),
        DenyOutcome::NoRequest
    );

    // The connector makes its first swap, through what the hub will call.
    let organization = db::organization_of_device(pool, &who)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(organization, installed.organization);
    let vault = Vault::open(&fresh.data_dir()).unwrap();
    let key = arauto_hub::repo::device::organization_key(pool, organization, &vault)
        .await
        .unwrap();
    let version = DeclaredText::parse("0.1.0").unwrap();
    let channel = connection::open(pool, organization, device, address(31), &version, 1, NOW)
        .await
        .unwrap();
    let IssueOutcome::Issued(issued) =
        swap::issue(pool, &mark, organization, &key, &channel, None, NOW)
            .await
            .unwrap()
    else {
        panic!("the first credential was not issued");
    };
    let saved = Saved {
        generation: issued.receipt.credential().generation,
        digest: issued.receipt.credential().digest,
    };
    let confirmed = swap::confirm(
        pool,
        &mark,
        organization,
        &channel,
        &issued.receipt,
        &saved,
        NOW,
    )
    .await
    .unwrap();
    assert!(matches!(confirmed, ConfirmOutcome::Confirmed { .. }));

    // status: the situation, the public key of the hub and the restored mode.
    let status = admin::status(pool, &mark).await.unwrap();
    assert_eq!(status.schema, SchemaState::Current);
    assert_eq!(status.hub_key, Some(installed.hub_key));
    assert_eq!(status.standing, Some(Standing::Consistent));
    assert!(!status.installation.unwrap().restored_mode);
    let report = render::status(&status);
    assert!(report.done);
    assert!(
        report
            .text
            .contains(&format!("hub key: {}", installed.hub_key.to_text()))
    );
    assert!(
        report.text.contains("restored mode: off"),
        "{}",
        report.text
    );
    assert!(report.text.contains("1 active"), "{}", report.text);

    // wake on and off.
    assert_eq!(
        session.wake("vendas", true, NOW).await.unwrap(),
        WakeOutcome::Changed
    );
    assert_eq!(
        session.wake("vendas", true, NOW).await.unwrap(),
        WakeOutcome::Unchanged
    );
    assert!(matches!(
        session.wake("nobody", true, NOW).await,
        Err(AdminError::NoSuchHermes)
    ));

    // A reconnect: the last use is asked about before the code is even looked at.
    let block = session.invite("vendas", &url, NOW).await.unwrap();
    assert!(block.issued.reconnect);
    let second_invite = block.issued.invite.reveal().as_str().to_owned();
    let second_who = identity();
    let (second_device, second_code) = enroll(pool, &mark, &second_invite, &second_who).await;
    let outcome = session
        .approve("vendas", &second_code.to_string(), None, NOW)
        .await
        .unwrap();
    let ApproveOutcome::NeedsRecognition(last_use) = &outcome else {
        panic!("approved without the answer about the last use: {outcome:?}");
    };
    assert_eq!(last_use.device, device);
    let report = render::approved("vendas", &outcome);
    assert!(!report.done);
    assert!(
        report.text.contains("--recognized or --not-recognized"),
        "{}",
        report.text
    );
    assert!(report.text.contains("192.0.2.31"), "{}", report.text);
    // The answer goes with the last use the command showed.
    let view = session.request("vendas").await.unwrap().unwrap();
    let shown_last_use = view
        .replaces
        .clone()
        .expect("a reconnect shows the last use");
    assert_eq!(&shown_last_use, last_use);
    let outcome = session
        .approve(
            "vendas",
            &second_code.to_string(),
            Some(Recognized {
                answer: Recognition::NotRecognized,
                shown: shown_last_use,
            }),
            NOW,
        )
        .await
        .unwrap();
    let ApproveOutcome::Approved(approved) = &outcome else {
        panic!("not approved: {outcome:?}");
    };
    assert_eq!(approved.quarantined, Some(device));
    // What was approved says again which last use the answer was about.
    let approved_report = render::approved("vendas", &outcome);
    assert!(
        approved_report.done
            && approved_report
                .text
                .contains("Last use of the current device")
            && approved_report.text.contains("192.0.2.31"),
        "{}",
        approved_report.text
    );

    // alerts, and marking one as seen.
    let alerts = session.alerts(AlertFilter::Unseen, 50).await.unwrap();
    assert_eq!(alerts.len(), 1);
    let shown = render::alerts(&alerts, &[]).text;
    assert!(
        shown.contains("copy_suspected") && shown.contains("NOT SEEN"),
        "{shown}"
    );
    assert!(session.acknowledge(alerts[0].id, NOW + 5).await.unwrap());
    assert!(!render::acknowledged(session.acknowledge(alerts[0].id, NOW).await.unwrap()).done);
    assert!(
        session
            .alerts(AlertFilter::Unseen, 50)
            .await
            .unwrap()
            .is_empty()
    );

    // revoke: the device in quarantine and the approved request are both closed.
    let closed = session.revoke("vendas", NOW).await.unwrap();
    assert_eq!(closed.devices.len(), 2);
    for closed in [device, second_device] {
        let state: String = sqlx::query_scalar("SELECT state FROM device WHERE id = $1")
            .bind(closed)
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(state, DeviceState::Revoked.as_str());
    }
    assert!(
        render::revoked("vendas", &closed)
            .text
            .contains("was revoked")
    );
    assert!(matches!(
        session.revoke("nobody", NOW).await,
        Err(AdminError::NoSuchHermes)
    ));

    // audit: every change, by the local admin, with the system user as a declared text.
    let rows = session.audit(1_000).await.unwrap();
    let by_admin: Vec<&str> = rows
        .iter()
        .filter(|row| row.actor_kind == "admin")
        .map(|row| row.action.as_str())
        .collect();
    assert_eq!(
        by_admin,
        [
            "installation.created",
            "organization.created",
            "admin.created",
            "hub_key.created",
            "hermes.created",
            "invite.issued",
            "device.code_rejected",
            "device.approved",
            "hermes.wake_changed",
            "invite.issued",
            "device.quarantined",
            "device.approved",
            "device.revoked",
            "device.revoked",
        ]
    );
    assert!(
        rows.iter()
            .filter(|row| row.actor_kind == "admin")
            .all(|row| row.actor_note.as_deref() == Some("operator"))
    );
    let tail = render::audit(&session.audit(3).await.unwrap()).text;
    assert_eq!(tail.lines().count(), 3);
    assert!(
        tail.contains("device.revoked") && tail.contains("(operator)"),
        "{tail}"
    );

    // hermes remove, and the name that never comes back.
    let removed = session.hermes_remove("vendas", NOW).await.unwrap();
    assert!(
        render::hermes_removed(&removed)
            .text
            .contains("never used again")
    );
    assert!(matches!(
        session.hermes_add("vendas", NOW).await,
        Err(AdminError::NameRetired)
    ));
    assert!(matches!(
        session.hermes_remove("vendas", NOW).await,
        Err(AdminError::NoSuchHermes)
    ));

    // Nothing the command printed along the way, other than the block, showed a secret.
    let printed = [
        listed,
        request_shown,
        report.text,
        shown,
        tail,
        render::status(&admin::status(pool, &mark).await.unwrap()).text,
        render::hermes_list(&session.hermes_list(true).await.unwrap()).text,
        render::audit(&rows).text,
    ]
    .join("\n");
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    for secret in [
        invite.clone(),
        second_invite.clone(),
        hex(issued.sealed.expose()),
    ] {
        assert!(!printed.contains(&secret), "the command printed {secret}");
    }
    let master = std::fs::read(fresh.data_dir().join(MASTER_KEY_FILE)).unwrap();
    assert!(!printed.contains(&hex(&master)));
}

#[tokio::test]
async fn what_a_device_declared_is_escaped_on_the_way_out() {
    let fresh = Fresh::new().await;
    fresh.init().await.unwrap();
    let session = fresh.session().await;
    session.hermes_add("vendas", NOW).await.unwrap();
    let url = PublicUrl::parse("http://127.0.0.1:8443").unwrap();
    let block = session.invite("vendas", &url, NOW).await.unwrap();
    let invite = block.issued.invite.reveal().as_str().to_owned();
    let (device, _) = enroll(&fresh.db.pool, &fresh.mark(), &invite, &identity()).await;

    // What the protocol refuses on the way in is escaped on the way out anyway: a row is
    // not trusted for having been checked once.
    sqlx::query(
        "UPDATE device SET declared = $2, connector_version = $3, enrolled_from = $4 \
         WHERE id = $1",
    )
    .bind(device)
    .bind(sqlx::types::Json(serde_json::json!({
        "hostname": "srv\u{1b}[2J\u{1b}[1;1Hok",
        "os": "linux\u{202e}xunil",
        "hermes_version": "1.0\r\napproved: yes",
    })))
    .bind("0.1\u{7}.0")
    .bind("192.0.2.9\u{1b}[31m")
    .execute(&fresh.db.pool)
    .await
    .unwrap();
    // The reader of rows already drops what a declared text may not have...
    let view = session.request("vendas").await.unwrap().unwrap();
    assert_eq!(view.request.declared.hostname.as_str(), "srv[2J[1;1Hok");
    // ...and what is printed is plain ASCII on its own lines, whatever the column held.
    let shown = render::request("vendas", &view);
    assert!(
        shown
            .bytes()
            .all(|byte| byte == b'\n' || (0x20..=0x7e).contains(&byte)),
        "{shown:?}"
    );
    assert_eq!(shown.lines().count(), 2, "{shown:?}");
    assert!(shown.contains("0.1\\u{7}.0"), "{shown}");
    assert!(shown.contains("192.0.2.9\\u{1b}[31m"), "{shown}");
}

/// C11: the panel lists a device the database does not know, with the time, the address and
/// what it declares, and says on every screen that the hub came back from a copy. In the MVP
/// the panel is this command.
#[tokio::test]
async fn the_devices_the_hub_does_not_know_and_the_restored_mode_are_shown_to_the_admin() {
    let fresh = Fresh::new().await;
    fresh.init().await.unwrap();
    let pool = &fresh.db.pool;
    let mark = fresh.mark();
    let session = fresh.session().await;
    let organization = session.organization();

    // Nothing yet.
    assert!(session.unknown_devices(50).await.unwrap().is_empty());
    assert_eq!(render::alerts(&[], &[]).text, "No alerts.\n");
    let status = admin::status(pool, &mark).await.unwrap();
    assert_eq!(status.unknown_devices, 0);
    assert!(!render::status(&status).text.contains("does not know"));
    let installation = session.installation().await.unwrap().unwrap();
    assert_eq!(render::restored_notice(&installation), None);

    // A device the database does not know presents a valid seal of this organization.
    let vault = Vault::open(&fresh.data_dir()).unwrap();
    let key = arauto_hub::repo::device::organization_key(pool, organization, &vault)
        .await
        .unwrap();
    let who = identity();
    let sealed = key
        .seal_credential(&who, Generation::new(4).unwrap(), NOW)
        .unwrap()
        .sealed;
    let version = DeclaredText::parse("0.1.0").unwrap();
    let outcome = swap::look_unknown(
        pool,
        organization,
        &key,
        &Stranger {
            identity: &who,
            presented: Some(sealed.expose()),
            connector_version: Some(&version),
            address: address(77),
        },
        NOW + 60_000,
    )
    .await
    .unwrap();
    let UnknownOutcome::Listed {
        unknown,
        evidence: Some(evidence),
    } = outcome
    else {
        panic!("not listed as proof: {outcome:?}");
    };

    // The admin sees it: in the status, and listed after the alerts.
    let listed = session.unknown_devices(50).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (listed[0].id, listed[0].seen_at, listed[0].proof_pending),
        (unknown.id, NOW + 60_000, true)
    );
    let shown = render::alerts(&[], &listed).text;
    for expected in [
        "No alerts.",
        "Devices the hub does not know",
        "192.0.2.77",
        "connector \"0.1.0\" (informed by the device, not verified)",
        "2027-01-15 08:01:00Z",
        "PROOF OF A RESTORATION NOT HANDLED YET",
        &unknown.id.as_hyphenated().to_string(),
    ] {
        assert!(shown.contains(expected), "{expected:?} is not in:\n{shown}");
    }
    let status = admin::status(pool, &mark).await.unwrap();
    assert_eq!(status.unknown_devices, 1);
    let status_shown = render::status(&status).text;
    assert!(
        status_shown.contains("devices the hub does not know: 1"),
        "{status_shown}"
    );
    // It shows no secret: neither the identity nor the credential that was presented.
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    for secret in [hex(who.as_bytes()), hex(sealed.expose())] {
        assert!(!shown.contains(&secret) && !status_shown.contains(&secret));
    }

    // What the device declared is escaped on the way out, whatever the row holds.
    sqlx::query("UPDATE unknown_device SET declared = $2, address = $3 WHERE id = $1")
        .bind(unknown.id)
        .bind(sqlx::types::Json(serde_json::json!({
            "connector_version": "0.1\u{1b}[2J\u{202e}",
        })))
        .bind("192.0.2.9\r\nNo alerts.")
        .execute(pool)
        .await
        .unwrap();
    let hostile = render::alerts(&[], &session.unknown_devices(50).await.unwrap()).text;
    assert!(
        hostile
            .bytes()
            .all(|byte| byte == b'\n' || (0x20..=0x7e).contains(&byte)),
        "{hostile:?}"
    );
    assert_eq!(hostile.lines().count(), 3, "{hostile:?}");

    // The hub turns the restored mode on with that proof: from then on every screen of the
    // command says so, and the listing is no pending proof.
    let entered = restore::enter(pool, &mark, NOW + 120_000, evidence)
        .await
        .unwrap();
    assert!(matches!(entered, EnterOutcome::Entered(_)));
    let installation = session.installation().await.unwrap().unwrap();
    let notice = render::restored_notice(&installation).expect("the mode is on");
    for expected in [
        "RESTORED MODE",
        // The last event the copy brought with it: the listing of that device.
        "the hub came back from a copy of 2027-01-15 08:01:00Z",
        "Reconnect each Hermes",
        "the list of admins",
    ] {
        assert!(
            notice.contains(expected),
            "{expected:?} is not in:\n{notice}"
        );
    }
    let after = session.unknown_devices(50).await.unwrap();
    assert!(!after[0].proof_pending);
    assert!(!render::alerts(&[], &after).text.contains("PROOF"));
}

// ---------------------------------------------------------------------------- a restored database

#[tokio::test]
async fn admin_on_a_database_behind_the_mark_writes_nothing_and_leaves_the_mark() {
    let fresh = Fresh::new().await;
    fresh.init().await.unwrap();
    let pool = &fresh.db.pool;
    let session = fresh.session().await;
    let url = PublicUrl::parse("https://hub.example.com").unwrap();
    session.hermes_add("vendas", NOW).await.unwrap();
    session.hermes_add("suporte", NOW).await.unwrap();
    let block = session.invite("vendas", &url, NOW).await.unwrap();
    let invite = block.issued.invite.reveal().as_str().to_owned();
    let (_, code) = enroll(pool, &fresh.mark(), &invite, &identity()).await;
    session.invite("suporte", &url, NOW).await.unwrap();

    // The database "comes back from a copy": the mark knows of events it does not have.
    let MarkValue::At(at) = fresh.mark().read().await.unwrap() else {
        panic!("there is a mark");
    };
    let ahead = fresh.mark().advance(at + 3).await.unwrap();
    assert_eq!(ahead, at + 3);
    let before = everything(pool).await;
    let evidence = Evidence::BehindMark {
        mark: at + 3,
        database: at,
    };

    // A new session, as a new run of the command, with what it remembers of the mark empty.
    let session = fresh.session().await;
    let restored = |outcome: AdminError| match outcome {
        AdminError::Restored(found) => found,
        other => panic!("another error: {other}"),
    };
    let code = code.to_string();
    assert_eq!(
        restored(session.hermes_add("novo", NOW).await.unwrap_err()),
        evidence
    );
    assert_eq!(
        restored(session.hermes_remove("suporte", NOW).await.unwrap_err()),
        evidence
    );
    assert_eq!(
        restored(session.wake("vendas", true, NOW).await.unwrap_err()),
        evidence
    );
    assert_eq!(
        restored(session.invite("vendas", &url, NOW).await.unwrap_err()),
        evidence
    );
    assert_eq!(
        restored(
            session
                .approve("vendas", &code, None, NOW)
                .await
                .unwrap_err()
        ),
        evidence
    );
    assert_eq!(
        restored(
            session
                .approve("vendas", "000000", None, NOW)
                .await
                .unwrap_err()
        ),
        evidence
    );
    assert_eq!(
        restored(session.deny("vendas", NOW).await.unwrap_err()),
        evidence
    );
    assert_eq!(
        restored(session.revoke("vendas", NOW).await.unwrap_err()),
        evidence
    );

    assert_eq!(everything(pool).await, before, "something was written");
    assert_eq!(
        fresh.mark().read().await.unwrap(),
        MarkValue::At(at + 3),
        "the mark was moved"
    );
    // Reading still works, and says what happened and what to do.
    let status = admin::status(pool, &fresh.mark()).await.unwrap();
    assert_eq!(status.standing, Some(Standing::Restored(evidence)));
    let report = render::status(&status);
    assert!(!report.done);
    assert!(
        report.text.contains("RESTORED FROM AN OLDER COPY"),
        "{}",
        report.text
    );
    assert!(!session.hermes_list(false).await.unwrap().is_empty());
    let text = AdminError::Restored(evidence).to_string();
    assert!(
        text.contains("Nothing was written") && text.contains("arauto-hub serve"),
        "{text}"
    );

    // The proof is still there for the hub: the same check it makes when it starts.
    let standing = arauto_hub::restore::check(pool, &fresh.mark())
        .await
        .unwrap();
    assert_eq!(standing, Standing::Restored(evidence));
}

#[tokio::test]
async fn admin_on_a_database_with_no_mark_writes_nothing_and_creates_no_mark() {
    let fresh = Fresh::new().await;
    fresh.init().await.unwrap();
    let session = fresh.session().await;
    session.hermes_add("vendas", NOW).await.unwrap();
    fresh.mark().erase().await.unwrap();
    let before = everything(&fresh.db.pool).await;

    let session = fresh.session().await;
    for refused in [
        session.hermes_add("novo", NOW).await.map(|_| ()),
        session.revoke("vendas", NOW).await.map(|_| ()),
        session.wake("vendas", true, NOW).await.map(|_| ()),
    ] {
        assert!(
            matches!(refused, Err(AdminError::Restored(Evidence::MarkAbsent))),
            "{refused:?}"
        );
    }
    assert_eq!(everything(&fresh.db.pool).await, before);
    assert_eq!(fresh.mark().read().await.unwrap(), MarkValue::Absent);
    assert!(!fresh.data_dir().join("audit.mark").exists());
}

// ---------------------------------------------------------------------------- dev-reset

#[tokio::test]
async fn dev_reset_erases_the_database_and_the_mark_together_and_only_when_confirmed() {
    let fresh = Fresh::new().await;
    fresh.init().await.unwrap();
    let pool = &fresh.db.pool;
    let session = fresh.session().await;
    session.hermes_add("vendas", NOW).await.unwrap();
    let before = everything(pool).await;
    let mark = fresh.mark();

    let refused = admin::dev_reset(pool, &mark, false).await;
    assert!(
        matches!(refused, Err(AdminError::ResetNotConfirmed)),
        "{refused:?}"
    );
    assert_eq!(everything(pool).await, before);
    assert!(matches!(mark.read().await.unwrap(), MarkValue::At(_)));

    // The name of a test database starts with `arauto_t_`: here it runs.
    admin::dev_reset(pool, &mark, true).await.unwrap();
    assert_eq!(
        db::schema_state(pool, &db::MIGRATOR).await.unwrap(),
        SchemaState::Empty
    );
    assert_eq!(mark.read().await.unwrap(), MarkValue::Absent);
    assert_eq!(fresh.mark().read().await.unwrap(), MarkValue::Absent);
    // Erasing both leaves nothing that reads as a restoration: `init` installs again, with
    // the master key that stayed, and the same handle of the mark follows.
    let again = fresh.init().await.unwrap();
    assert!(matches!(again, InitOutcome::Created(_)), "{again:?}");
    let session = Session::open(pool.clone(), mark.clone(), os_user())
        .await
        .unwrap();
    session.hermes_add("vendas", NOW).await.unwrap();
    assert_eq!(count(pool, "hermes").await, 1);
    // Running it twice in a row is harmless.
    admin::dev_reset(pool, &mark, true).await.unwrap();
    admin::dev_reset(pool, &mark, true).await.unwrap();
}

#[tokio::test]
async fn dev_reset_refuses_a_database_that_is_not_of_development_or_of_the_tests() {
    let fresh = Fresh {
        db: TestDb::empty_with_a_plain_name().await,
        dir: tempfile::tempdir().unwrap(),
    };
    let name = fresh.db.name().to_owned();
    assert!(
        !name.starts_with("arauto_t_") && !name.ends_with("_dev"),
        "{name}"
    );
    fresh.init().await.unwrap();
    let pool = &fresh.db.pool;
    let before = everything(pool).await;
    let mark = fresh.mark();
    let marked = mark.read().await.unwrap();

    let refused = admin::dev_reset(pool, &mark, true).await;
    assert!(
        matches!(refused, Err(AdminError::NotADevelopmentDatabase)),
        "{refused:?}"
    );
    assert_eq!(everything(pool).await, before, "something was erased");
    assert_eq!(mark.read().await.unwrap(), marked);
    // The refusal does not repeat the name of the database.
    assert!(!refused.unwrap_err().to_string().contains(&name));
}

// ---------------------------------------------------------------------------- the program

struct Ran {
    ok: bool,
    out: String,
    err: String,
}

fn run(fresh: &Fresh, public_url: Option<&str>, args: &[&str]) -> Ran {
    let mut command = Command::new(env!("CARGO_BIN_EXE_arauto-hub"));
    command
        .args(args)
        .env_remove("ARAUTO_PUBLIC_URL")
        .env_remove("ARAUTO_LISTEN")
        .env_remove("ARAUTO_LISTEN_INTERNAL")
        .env_remove("ARAUTO_CLIENT_IP_SOURCE")
        .env_remove("ARAUTO_TRUSTED_PROXIES")
        .env_remove("ARAUTO_ALLOW_UNSAFE_DB")
        .env_remove("ARAUTO_LISTEN_PANEL")
        .env_remove("ARAUTO_PANEL_URL")
        .env("ARAUTO_DATABASE_URL", fresh.db.url().expose())
        .env("ARAUTO_DATA_DIR", fresh.data_dir())
        .env("USER", "tester")
        .stdin(std::process::Stdio::null());
    if let Some(url) = public_url {
        command.env("ARAUTO_PUBLIC_URL", url);
    }
    let output = command.output().unwrap();
    Ran {
        ok: output.status.success(),
        out: String::from_utf8(output.stdout).unwrap(),
        err: String::from_utf8(output.stderr).unwrap(),
    }
}

fn data_dir_is_ready(path: &Path) -> bool {
    path.join(MASTER_KEY_FILE).is_file() && path.join("audit.mark").is_file()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_program_runs_the_commands_and_fails_when_nothing_was_done() {
    let fresh = Fresh::new().await;
    let url = fresh.db.url();
    let run = |public_url: Option<&'static str>, args: &'static [&'static str]| {
        let fresh = &fresh;
        async move { tokio::task::block_in_place(|| run(fresh, public_url, args)) }
    };

    // `serve` without the variables of the hub names every one that is missing, and touches
    // nothing: the next command still finds the database empty.
    let serve = run(None, &["serve"]).await;
    assert!(
        !serve.ok
            && [
                "ARAUTO_LISTEN",
                "ARAUTO_LISTEN_INTERNAL",
                "ARAUTO_CLIENT_IP_SOURCE"
            ]
            .iter()
            .all(|variable| serve.err.contains(variable)),
        "{}",
        serve.err
    );
    // Before `init`, the admin commands say what is missing.
    let early = run(None, &["admin", "hermes", "list"]).await;
    assert!(
        !early.ok && early.err.contains("arauto-hub init"),
        "{}",
        early.err
    );

    let init = run(None, &["init"]).await;
    assert!(init.ok, "{}", init.err);
    assert!(init.out.contains("The hub is installed.") && init.out.contains("hub key: arh1"));
    assert!(data_dir_is_ready(&fresh.data_dir()));
    let again = run(None, &["init"]).await;
    assert!(
        again.ok && again.out.contains("already installed"),
        "{}",
        again.out
    );

    let status = run(None, &["admin", "status"]).await;
    assert!(status.ok, "{}{}", status.out, status.err);
    assert!(status.out.contains("database: current") && status.out.contains("restored mode: off"));

    let added = run(None, &["admin", "hermes", "add", "vendas"]).await;
    assert!(
        added.ok && added.out.contains("vendas was created"),
        "{}",
        added.err
    );
    let reserved = run(None, &["admin", "hermes", "add", "admin"]).await;
    assert!(
        !reserved.ok && reserved.err.contains("reserved"),
        "{}",
        reserved.err
    );
    let listed = run(None, &["admin", "hermes", "list"]).await;
    assert!(listed.ok && listed.out.contains("vendas"), "{}", listed.out);

    // The invite needs the public address, and says which variable is missing.
    let no_url = run(None, &["admin", "invite", "vendas"]).await;
    assert!(
        !no_url.ok && no_url.err.contains("ARAUTO_PUBLIC_URL"),
        "{}",
        no_url.err
    );
    let invited = run(
        Some("https://hub.example.com"),
        &["admin", "invite", "vendas"],
    )
    .await;
    assert!(invited.ok, "{}", invited.err);
    let invite = invite_in(&invited.out);
    assert!(InviteSecret::parse(&invite).is_ok());
    assert!(invited.out.contains("wss://hub.example.com/v1/channel"));

    // Nobody enrolled yet: nothing to approve, and the program says it did nothing.
    let approve = run(None, &["admin", "approve", "vendas", "123-456"]).await;
    assert!(
        !approve.ok && approve.out.contains("no pending request"),
        "{}",
        approve.out
    );
    let wake = run(None, &["admin", "wake", "vendas", "on"]).await;
    assert!(wake.ok && wake.out.contains("can be woken"), "{}", wake.out);
    let audit = run(None, &["admin", "audit", "--tail", "3"]).await;
    assert!(
        audit.ok && audit.out.contains("hermes.wake_changed"),
        "{}",
        audit.out
    );
    assert!(audit.out.contains("(tester)"), "{}", audit.out);
    let alerts = run(None, &["admin", "alerts"]).await;
    assert!(
        alerts.ok && alerts.out.contains("No alerts"),
        "{}",
        alerts.out
    );

    // A first connection: the request is shown, with what the device declares, even when
    // the code comes on the command line; the code itself never is.
    let pool = &fresh.db.pool;
    let first_who = identity();
    let (first_device, first_code) = enroll_at(
        pool,
        &fresh.mark(),
        &invite,
        &first_who,
        SystemClock.now_ms(),
    )
    .await;
    let first_typed = first_code.to_string();
    let approved = tokio::task::block_in_place(|| {
        crate::admin::run(&fresh, None, &["admin", "approve", "vendas", &first_typed])
    });
    assert!(approved.ok, "{}{}", approved.out, approved.err);
    for expected in [
        "Request of Hermes vendas",
        "Informed by the device, not verified",
        "build-7",
        "vendas was approved",
    ] {
        assert!(approved.out.contains(expected), "{}", approved.out);
    }
    assert!(!approved.out.contains(&first_typed));

    // The connector makes its first swap, and the Hermes is reconnected with a new invite.
    let organization = db::organization_of_device(pool, &first_who)
        .await
        .unwrap()
        .unwrap();
    let vault = Vault::open(&fresh.data_dir()).unwrap();
    let key = arauto_hub::repo::device::organization_key(pool, organization, &vault)
        .await
        .unwrap();
    let version = DeclaredText::parse("0.1.0").unwrap();
    let used_at = SystemClock.now_ms();
    let channel = connection::open(
        pool,
        organization,
        first_device,
        address(31),
        &version,
        1,
        used_at,
    )
    .await
    .unwrap();
    let IssueOutcome::Issued(issued) = swap::issue(
        pool,
        &fresh.mark(),
        organization,
        &key,
        &channel,
        None,
        used_at,
    )
    .await
    .unwrap() else {
        panic!("the first credential was not issued");
    };
    let saved = Saved {
        generation: issued.receipt.credential().generation,
        digest: issued.receipt.credential().digest,
    };
    let confirmed = swap::confirm(
        pool,
        &fresh.mark(),
        organization,
        &channel,
        &issued.receipt,
        &saved,
        used_at,
    )
    .await
    .unwrap();
    assert!(matches!(confirmed, ConfirmOutcome::Confirmed { .. }));
    let reinvited = run(
        Some("https://hub.example.com"),
        &["admin", "invite", "vendas"],
    )
    .await;
    assert!(
        reinvited.ok && reinvited.out.contains("Invite to reconnect Hermes vendas"),
        "{}{}",
        reinvited.out,
        reinvited.err
    );
    let second_invite = invite_in(&reinvited.out);
    let (_, second_code) = enroll_at(
        pool,
        &fresh.mark(),
        &second_invite,
        &identity(),
        SystemClock.now_ms(),
    )
    .await;
    let second_typed = second_code.to_string();

    // A reconnect: with the code and no answer, nothing is approved, and the last use of the
    // device being replaced is on the screen.
    let last_use_shown = format!(
        "Last use of the current device (active): {}, from 192.0.2.31.",
        render::time(used_at)
    );
    let unanswered = tokio::task::block_in_place(|| {
        crate::admin::run(&fresh, None, &["admin", "approve", "vendas", &second_typed])
    });
    assert!(!unanswered.ok, "{}", unanswered.out);
    for expected in [
        last_use_shown.as_str(),
        "--recognized or --not-recognized",
        "Nothing was approved",
    ] {
        assert!(unanswered.out.contains(expected), "{}", unanswered.out);
    }
    // With the answer on the same line as the code, the last use is shown all the same,
    // before the approval and again beside it: the answer is never given in the dark.
    let answered = tokio::task::block_in_place(|| {
        crate::admin::run(
            &fresh,
            None,
            &["admin", "approve", "vendas", &second_typed, "--recognized"],
        )
    });
    assert!(answered.ok, "{}{}", answered.out, answered.err);
    let request_at = answered.out.find("Request of Hermes vendas").unwrap();
    let approved_at = answered.out.find("vendas was approved").unwrap();
    assert!(request_at < approved_at, "{}", answered.out);
    assert_eq!(
        answered.out.matches(last_use_shown.as_str()).count(),
        2,
        "{}",
        answered.out
    );
    assert!(!answered.out.contains(&second_typed));

    let revoked = run(None, &["admin", "revoke", "vendas"]).await;
    assert!(
        revoked.ok && revoked.out.contains("was revoked"),
        "{}",
        revoked.out
    );

    // No output of any command showed the address of the database, and only the block
    // showed the invite.
    let everything_printed = [
        &serve,
        &early,
        &init,
        &again,
        &status,
        &added,
        &reserved,
        &listed,
        &no_url,
        &approve,
        &wake,
        &audit,
        &alerts,
        &approved,
        &unanswered,
        &answered,
        &revoked,
    ];
    for ran in everything_printed {
        for text in [&ran.out, &ran.err] {
            assert!(!text.contains(url.expose()), "{text}");
            assert!(!text.contains(&invite), "{text}");
            assert!(!text.contains(&second_invite), "{text}");
        }
    }

    // dev-reset wants its confirmation; with it, the database and the mark go together.
    let refused = run(None, &["dev-reset"]).await;
    assert!(
        !refused.ok && refused.err.contains("--yes-erase-everything"),
        "{}",
        refused.err
    );
    assert!(data_dir_is_ready(&fresh.data_dir()));
    let reset = run(None, &["dev-reset", "--yes-erase-everything"]).await;
    assert!(reset.ok, "{}", reset.err);
    assert!(!fresh.data_dir().join("audit.mark").exists());
    let after = run(None, &["admin", "status"]).await;
    assert!(
        !after.ok && after.out.contains("database: empty"),
        "{}",
        after.out
    );
}

/// C11: while the restored mode is on, every screen says that the hub came back from a copy.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_program_says_on_every_screen_that_the_hub_came_back_from_a_copy() {
    let fresh = Fresh::new().await;
    let run = |args: &[&str]| tokio::task::block_in_place(|| crate::admin::run(&fresh, None, args));
    assert!(run(&["init"]).ok);
    assert!(run(&["admin", "hermes", "add", "vendas"]).ok);
    let screens: [&[&str]; 3] = [
        &["admin", "hermes", "list"],
        &["admin", "alerts"],
        &["admin", "audit", "--tail", "2"],
    ];
    for screen in screens {
        let ran = run(screen);
        assert!(ran.ok && !ran.out.contains("RESTORED MODE"), "{}", ran.out);
    }

    // The data folder is lost with its mark, and the hub turns the restored mode on.
    fresh.mark().erase().await.unwrap();
    let entered = restore::enter(
        &fresh.db.pool,
        &fresh.mark(),
        SystemClock.now_ms(),
        Evidence::MarkAbsent,
    )
    .await
    .unwrap();
    assert!(matches!(entered, EnterOutcome::Entered(_)));

    for screen in screens {
        let ran = run(screen);
        assert!(ran.ok, "{}{}", ran.out, ran.err);
        assert!(
            ran.out
                .starts_with("RESTORED MODE: the hub came back from a copy of "),
            "{screen:?}:\n{}",
            ran.out
        );
        assert!(ran.out.contains("Reconnect each Hermes"), "{}", ran.out);
    }
    let status = run(&["admin", "status"]);
    assert!(status.out.contains("restored mode: ON"), "{}", status.out);
}

// ---------------------------------------------------------------------------- serve, whole

/// A `serve` of the program that is running, with the lines it writes on the standard error.
struct Serving {
    child: std::process::Child,
    lines: std::sync::mpsc::Receiver<String>,
    panel: u16,
}

impl Serving {
    /// Starts `arauto-hub serve` on free ports of this machine, with a panel. Its standard
    /// error is a pipe, not a terminal.
    fn start(fresh: &Fresh) -> Self {
        use std::io::BufRead;
        let ports: Vec<std::net::TcpListener> = (0..3)
            .map(|_| std::net::TcpListener::bind("127.0.0.1:0").unwrap())
            .collect();
        let port = |index: usize| ports[index].local_addr().unwrap().port();
        let (connectors, internal, panel) = (port(0), port(1), port(2));
        drop(ports);
        let mut child = Command::new(env!("CARGO_BIN_EXE_arauto-hub"))
            .arg("serve")
            .env_remove("ARAUTO_TRUSTED_PROXIES")
            .env_remove("RUST_LOG")
            .env("ARAUTO_DATABASE_URL", fresh.db.url().expose())
            .env("ARAUTO_DATA_DIR", fresh.data_dir())
            .env("ARAUTO_ALLOW_UNSAFE_DB", "1")
            .env("ARAUTO_LISTEN", format!("127.0.0.1:{connectors}"))
            .env("ARAUTO_LISTEN_INTERNAL", format!("127.0.0.1:{internal}"))
            .env("ARAUTO_CLIENT_IP_SOURCE", "socket")
            .env(
                "ARAUTO_PUBLIC_URL",
                format!("http://localhost:{connectors}"),
            )
            .env("ARAUTO_LISTEN_PANEL", format!("127.0.0.1:{panel}"))
            .env("ARAUTO_PANEL_URL", format!("http://localhost:{panel}"))
            .env("USER", "tester")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let stderr = child.stderr.take().unwrap();
        let (sender, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr).lines() {
                let Ok(line) = line else { return };
                if sender.send(line).is_err() {
                    return;
                }
            }
        });
        Self {
            child,
            lines,
            panel,
        }
    }

    /// Everything the program wrote until the end of the block with the installation code, and
    /// the code.
    fn until_the_code(&self) -> (Vec<String>, String) {
        let mut seen = Vec::new();
        let mut code = None;
        loop {
            let line = self
                .lines
                .recv_timeout(std::time::Duration::from_secs(60))
                .unwrap_or_else(|_| panic!("no complete block was written. Seen: {seen:#?}"));
            if let Some(found) = line.strip_prefix("INSTALLATION CODE: ") {
                code = Some(found.to_owned());
            }
            let end = line.starts_with("=== end of ARAUTO FIRST ACCESS");
            seen.push(line);
            if end && let Some(code) = code {
                return (seen, code);
            }
        }
    }

    async fn type_code(&self, code: &str) -> arauto_testkit::http::Response {
        let host = format!("localhost:{}", self.panel);
        let origin = format!("http://{host}");
        let headers = [
            ("Host", host.as_str()),
            ("Origin", origin.as_str()),
            ("Content-Type", "application/x-www-form-urlencoded"),
        ];
        arauto_testkit::http::post(
            std::net::SocketAddr::from(([127, 0, 0, 1], self.panel)),
            "/sign-in",
            &headers,
            format!("code={code}").as_bytes(),
            std::time::Duration::from_secs(15),
        )
        .await
        .unwrap()
    }

    fn stop(self) {
        drop(self);
    }
}

impl Drop for Serving {
    /// The program is killed whatever happens to the test, so that none is left running.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// R1, R8, R9, R16 and R17 through the program itself: `serve` writes the block after the panel
/// listens (a connection to the panel works by then), with the origin of the panel and no color
/// on a pipe; the code in it opens the registration of a passkey; and a second `serve` on the
/// same database writes the block of a new code, after which the old one is refused.
///
/// The stop of the program is not exercised: a console signal cannot be sent to a child on
/// Windows without `unsafe`, so the process is killed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn serve_writes_the_block_once_the_panel_listens_with_no_color_on_a_pipe() {
    let fresh = Fresh::new().await;
    let (first, first_code, first_seen) = tokio::task::block_in_place(|| {
        let serving = Serving::start(&fresh);
        let (seen, code) = serving.until_the_code();
        (serving, code, seen)
    });
    let text = first_seen.join("\n");
    let listening = first_seen
        .iter()
        .position(|line| line.contains("the panel of the admin is listening"))
        .unwrap_or_else(|| panic!("the panel was not announced before the block: {text}"));
    let block = first_seen
        .iter()
        .position(|line| line.starts_with("=== ARAUTO FIRST ACCESS ("))
        .unwrap();
    assert!(
        listening < block,
        "the block came before the panel listened"
    );
    assert!(
        first_seen
            .iter()
            .any(|line| line == "Type the code there and create your passkey.")
    );
    assert!(
        first_seen
            .iter()
            .any(|line| line == &format!("Open: http://localhost:{}", first.panel))
    );
    // A pipe is not a terminal: no color escape.
    assert!(!text.contains(char::from(27)), "{text}");
    // The panel answers by now, and the code opens the registration of a passkey.
    let typed = first.type_code(&first_code).await;
    assert_eq!(
        (typed.status, typed.header("location")),
        (303, Some("/first-access")),
        "{}",
        typed.text()
    );
    first.stop();

    // A second start writes a new block with a new code, and the old one is dead.
    let (second, second_code) = tokio::task::block_in_place(|| {
        let serving = Serving::start(&fresh);
        let (_, code) = serving.until_the_code();
        (serving, code)
    });
    assert_ne!(second_code, first_code);
    let old = second.type_code(&first_code).await;
    assert_eq!(old.status, 200);
    assert!(old.text().contains("That code did not work"));
    let new = second.type_code(&second_code).await;
    assert_eq!(new.status, 303);
    second.stop();
}
