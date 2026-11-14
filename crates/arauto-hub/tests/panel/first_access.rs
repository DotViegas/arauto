//! The lifecycle of the installation code (R1 to R10 of the design of the first access): the
//! start code, the command code, one code at a time, typing that does not spend, the session
//! per code, a lost session that needs only the latest code, and the task that writes the block
//! in the log.
//!
//! Each test names the rule it guards and fails when that rule is removed.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_hub::channel::{self, Downloads, Options, Served};
use arauto_hub::config::ClientIpSource;
use arauto_hub::first_access::{
    self, NO_PANEL_LINE, NOT_ISSUED_LINE, Output, Prepared, REPEAT_MS, Step,
};
use arauto_hub::repo::panel::SESSION_IDLE_MS;
use arauto_hub::vault::Vault;
use arauto_protocol::clock::{Clock, ManualClock, SeededRng, SystemClock};
use arauto_protocol::message::DeclaredText;
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::hub::support::{self, PANEL_ORIGIN};
use arauto_testkit::passkey::SoftAuthenticator;
use uuid::Uuid;

use crate::{Browser, REFUSED, cookie_of, form_value, hub, rows};

/// What the task wrote, piece by piece: a count of writes is a count of blocks.
#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl Output for Recorder {
    fn write(&self, text: &str) {
        self.0.lock().unwrap().push(text.to_owned());
    }
}

impl Recorder {
    fn writes(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }

    fn count(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    /// The code of the last block that has one.
    fn last_code(&self) -> String {
        self.writes()
            .iter()
            .rev()
            .find_map(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("INSTALLATION CODE: "))
                    .map(str::to_owned)
            })
            .expect("a block with a code was written")
    }
}

fn user() -> DeclaredText {
    DeclaredText::lossy("tester")
}

/// A hub whose clock the test moves.
async fn hub_with_clock() -> (Fixture, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(SystemClock.now_ms()));
    let fixture = Fixture::with_clock(fixture::settings(), clock.clone()).await;
    (fixture, clock)
}

/// A value of one column of the only row of a table that matches.
async fn one<T>(fixture: &Fixture, query: &str, id: Uuid) -> T
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
{
    sqlx::query_scalar(sqlx::AssertSqlSafe(query.to_owned()))
        .bind(id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap()
}

async fn passkeys(fixture: &Fixture) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM panel_passkey")
        .fetch_one(&fixture.pool)
        .await
        .unwrap()
}

/// R6 and R4: typing a code opens a session and spends nothing; the registration of a passkey
/// in that session spends it, in the same write that keeps the passkey, and the digest goes.
#[tokio::test(flavor = "multi_thread")]
async fn typing_a_code_does_not_spend_it_and_the_passkey_that_is_kept_does() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let issued = fixture
        .admin
        .installation_code(fixture.now())
        .await
        .unwrap()
        .unwrap();
    let code = issued.code.reveal().to_string();

    let entered = browser.enter(&code).await;
    assert_eq!(entered.status, 303);
    let cookie = cookie_of(&entered).unwrap();
    // The code is still unspent, with its digest, and the session points at it.
    let state: String = one(
        &fixture,
        "SELECT state FROM panel_entry WHERE id = $1",
        issued.id,
    )
    .await;
    let digest: Option<Vec<u8>> = one(
        &fixture,
        "SELECT digest FROM panel_entry WHERE id = $1",
        issued.id,
    )
    .await;
    assert_eq!(state, "issued");
    assert_eq!(digest.map(|digest| digest.len()), Some(32));
    let reach: String = one(
        &fixture,
        "SELECT reach FROM panel_session WHERE entry_id = $1",
        issued.id,
    )
    .await;
    assert_eq!(reach, "setup");
    assert_eq!(passkeys(&fixture).await, 0);

    // Registering spends it.
    let mut first = SoftAuthenticator::new(PANEL_ORIGIN);
    assert_eq!(browser.register(&cookie, &mut first).await.status, 303);
    let state: String = one(
        &fixture,
        "SELECT state FROM panel_entry WHERE id = $1",
        issued.id,
    )
    .await;
    let digest: Option<Vec<u8>> = one(
        &fixture,
        "SELECT digest FROM panel_entry WHERE id = $1",
        issued.id,
    )
    .await;
    let used: Option<i64> = one(
        &fixture,
        "SELECT used_at FROM panel_entry WHERE id = $1",
        issued.id,
    )
    .await;
    assert_eq!(state, "used");
    assert!(digest.is_none() && used.is_some());
    assert_eq!(passkeys(&fixture).await, 1);
    assert!(browser.enter(&code).await.text().contains(REFUSED));

    // Another passkey in a session that reaches the whole panel depends on no code at all.
    let mut second = SoftAuthenticator::new(PANEL_ORIGIN);
    let kept = browser.register(&cookie, &mut second).await;
    assert_eq!(kept.status, 303);
    assert_eq!(kept.header("location"), Some("/"));
    assert_eq!(passkeys(&fixture).await, 2);
}

/// R6: a code that is no longer unspent when the passkey arrives spends nothing, keeps nothing
/// and closes the session that was opened with it.
#[tokio::test(flavor = "multi_thread")]
async fn a_passkey_is_not_kept_in_a_session_whose_code_is_no_longer_unspent() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let issued = fixture
        .admin
        .installation_code(fixture.now())
        .await
        .unwrap()
        .unwrap();
    let cookie = cookie_of(&browser.enter(&issued.code.reveal()).await).unwrap();
    // The code stops being unspent without the session being told (the race the lock covers).
    sqlx::query(
        "UPDATE panel_entry SET state = 'cancelled', digest = NULL, cancelled_at = 1, \
                                cancelled_by = 'command' WHERE id = $1",
    )
    .bind(issued.id)
    .execute(&fixture.pool)
    .await
    .unwrap();

    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    let answer = browser.register(&cookie, &mut authenticator).await;
    // The entry of the state, with the sentence that says the panel session ended.
    assert_eq!(answer.status, 200);
    assert!(answer.text().contains("Your panel session ended."));
    assert_eq!(passkeys(&fixture).await, 0);
    let mark: Option<i64> = sqlx::query_scalar("SELECT panel_first_passkey_at FROM organization")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert!(mark.is_none(), "no first access was done");
    let closed: String = one(
        &fixture,
        "SELECT state FROM panel_session WHERE entry_id = $1",
        issued.id,
    )
    .await;
    assert_eq!(closed, "closed");
    assert_eq!(
        browser.get("/first-access", Some(&cookie)).await.status,
        303
    );
}

/// Whether a backend of the database of `fixture` waits on a lock, within a few seconds.
async fn someone_waits_on_a_lock(fixture: &Fixture) -> bool {
    for _ in 0..200 {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_locks l JOIN pg_stat_activity a ON a.pid = l.pid \
             WHERE NOT l.granted AND a.datname = current_database()",
        )
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        if waiting > 0 {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    false
}

/// R6, the lock: while another write holds the row of a code, a registration in a session of
/// that code waits on that row before it touches its session or keeps anything; and when that
/// write cancels the code, nothing is kept and the session ends.
#[tokio::test(flavor = "multi_thread")]
async fn a_registration_waits_on_the_row_of_its_code_before_anything_else() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let issued = fixture
        .admin
        .installation_code(fixture.now())
        .await
        .unwrap()
        .unwrap();
    let cookie = cookie_of(&browser.enter(&issued.code.reveal()).await).unwrap();
    let session: Uuid = one(
        &fixture,
        "SELECT id FROM panel_session WHERE entry_id = $1",
        issued.id,
    )
    .await;
    // The challenge and the answer of the authenticator are ready before the race.
    let asked = browser
        .post("/passkey/register/start", Some(&cookie), "")
        .await;
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    let answer = format!(
        "credential={}",
        form_value(&authenticator.create(&asked.text()).unwrap())
    );

    // Another write holds the row of the code, as an issue that cancels it would.
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM panel_entry WHERE id = $1 FOR UPDATE")
        .bind(issued.id)
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let posted = {
        let cookie = cookie.clone();
        tokio::spawn(async move {
            browser
                .post("/passkey/register", Some(&cookie), &answer)
                .await
        })
    };
    assert!(
        someone_waits_on_a_lock(&fixture).await,
        "the registration never waited"
    );
    // It waits at the row of its code: its session is not locked yet.
    let mut probe = fixture.pool.begin().await.unwrap();
    let session_free = sqlx::query("SELECT id FROM panel_session WHERE id = $1 FOR UPDATE NOWAIT")
        .bind(session)
        .fetch_one(&mut *probe)
        .await;
    probe.rollback().await.unwrap();
    assert!(
        session_free.is_ok(),
        "the registration went past the row of its code without its lock"
    );

    // The other write cancels the code: the registration keeps nothing.
    sqlx::query(
        "UPDATE panel_entry SET state = 'cancelled', digest = NULL, cancelled_at = $2, \
                                cancelled_by = 'command' WHERE id = $1",
    )
    .bind(issued.id)
    .bind(fixture.now())
    .execute(&mut *holder)
    .await
    .unwrap();
    holder.commit().await.unwrap();
    let answered = posted.await.unwrap();
    assert_eq!(answered.status, 200);
    assert!(answered.text().contains("Your panel session ended."));
    assert_eq!(passkeys(&fixture).await, 0);
    let reason: Option<String> = one(
        &fixture,
        "SELECT close_reason FROM panel_session WHERE id = $1",
        session,
    )
    .await;
    assert_eq!(reason.as_deref(), Some("code_replaced"));
}

/// How many rounds of requests at once the run plays.
const RACE_ROUNDS: u64 = 30;

/// What runs at the same moment as the registration of a passkey in the session of a code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rival {
    /// The same code, typed again.
    Retyped,
    /// A command code, issued.
    Issued,
    /// Another registration, in the same session.
    Registered,
}

fn race_seed() -> u64 {
    std::env::var("ARAUTO_ROUNDS_SEED")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(0x5eed_f125_7ac5)
}

/// The answer of a fresh authenticator to a challenge of registration in the session of
/// `cookie`, as the form carries it.
async fn answer_of(browser: Browser, cookie: &str) -> String {
    let asked = browser
        .post("/passkey/register/start", Some(cookie), "")
        .await;
    assert_eq!(asked.status, 200, "{}", asked.text());
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    format!(
        "credential={}",
        form_value(&authenticator.create(&asked.text()).unwrap())
    )
}

/// One round: a code, a session typed with it, and the registration of a passkey in that
/// session at the same moment as a rival, each after a delay drawn from `rng`. Returns whether
/// the registration kept its passkey, or what went wrong.
async fn race(
    fixture: &Fixture,
    browser: Browser,
    rng: &mut SeededRng,
    first: bool,
    rival: Rival,
) -> Result<bool, String> {
    let (entry, code) = if first {
        let issued = fixture
            .admin
            .installation_code(fixture.now())
            .await
            .unwrap()
            .unwrap();
        (issued.id, issued.code.reveal().to_string())
    } else {
        let issued = fixture.admin.panel_code(fixture.now()).await.unwrap();
        (issued.id, issued.code.reveal().to_string())
    };
    let cookie = cookie_of(&browser.enter(&code).await).ok_or("the code did not open")?;
    let before = passkeys(fixture).await;
    // Whether this round can make the first access: a round before it may have kept nothing,
    // when its rival cancelled the code.
    let pending: bool =
        sqlx::query_scalar("SELECT panel_first_passkey_at IS NULL FROM organization")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    let answer = answer_of(browser, &cookie).await;
    let other = match rival {
        Rival::Registered => Some(answer_of(browser, &cookie).await),
        Rival::Retyped | Rival::Issued => None,
    };
    let (mine, theirs) = (rng.below(12), rng.below(12));
    let register = async {
        tokio::time::sleep(Duration::from_millis(mine)).await;
        browser
            .post("/passkey/register", Some(&cookie), &answer)
            .await
    };
    let rival_runs = async {
        tokio::time::sleep(Duration::from_millis(theirs)).await;
        match rival {
            Rival::Retyped => Ok(Some(browser.enter(&code).await)),
            Rival::Issued => fixture
                .admin
                .panel_code(fixture.now())
                .await
                .map(|_| None)
                .map_err(|error| error.to_string()),
            Rival::Registered => Ok(Some(
                browser
                    .post(
                        "/passkey/register",
                        Some(&cookie),
                        other.as_deref().unwrap_or_default(),
                    )
                    .await,
            )),
        }
    };
    let (registered, rivalled) = tokio::join!(register, rival_runs);
    let rivalled = rivalled.map_err(|error| format!("the issue failed: {error}"))?;
    for answer in std::iter::once(&registered).chain(rivalled.as_ref()) {
        if answer.status >= 500 {
            return Err(format!("an answer of {}", answer.status));
        }
    }

    let kept = passkeys(fixture).await - before;
    let state: String = one(
        fixture,
        "SELECT state FROM panel_entry WHERE id = $1",
        entry,
    )
    .await;
    let most = if rival == Rival::Registered { 2 } else { 1 };
    if kept > most {
        return Err(format!("{kept} passkeys kept for one code"));
    }
    if rival == Rival::Registered && kept != 2 {
        return Err(format!(
            "{kept} passkeys kept by two registrations in one session"
        ));
    }
    if (state == "used") != (kept > 0) {
        return Err(format!("{kept} passkeys kept and the code is {state}"));
    }
    if rival == Rival::Registered {
        // The one that came second registers in a session that now reaches the whole panel.
        let second = rivalled.as_ref().ok_or("no answer")?;
        let mut welcomed = 0;
        for answer in [&registered, second] {
            match (answer.status, answer.header("location")) {
                (303, Some("/")) => {}
                (303, Some("/installation?welcome")) => welcomed += 1,
                _ => {
                    return Err(format!(
                        "a registration in the same session answered {} {:?}",
                        answer.status,
                        answer.header("location")
                    ));
                }
            }
        }
        // Only the passkey that makes the first access sends the person to the check.
        if welcomed != usize::from(pending) {
            return Err(format!(
                "{welcomed} answers sent to the check, first access pending: {pending}"
            ));
        }
    }
    let unspent: i64 =
        sqlx::query_scalar("SELECT count(*) FROM panel_entry WHERE state = 'issued'")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    if unspent > 1 {
        return Err(format!("{unspent} unspent codes"));
    }
    let stale: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM panel_session s JOIN panel_entry e ON e.id = s.entry_id \
         WHERE s.state = 'open' AND s.reach = 'setup' AND e.state <> 'issued'",
    )
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    if stale > 0 {
        return Err(format!(
            "{stale} sessions still open on a code that is spent"
        ));
    }
    let mark: Option<i64> = sqlx::query_scalar("SELECT panel_first_passkey_at FROM organization")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    if mark.is_some() != (passkeys(fixture).await > 0) {
        return Err("the mark of the first access does not follow the passkeys".to_owned());
    }
    Ok(kept > 0)
}

/// R6 and R7 under races, in rounds drawn from a seed (`ARAUTO_ROUNDS_SEED` replays one): the
/// registration of a passkey in the session of a code at the same moment as the same code typed
/// again, a command code issued, or another registration in the same session, with random
/// delays and repeated rounds. What must hold: no request fails; one code keeps at most one
/// passkey (two in one session that reaches the whole panel by then); a kept passkey always
/// comes with its code spent and the mark of the first access; a cancelled code never comes
/// with a kept passkey; never more than one unspent code; no session that reaches only the
/// registration is left open on a code that is no longer unspent.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rounds_of_requests_at_once_on_one_code_keep_at_most_one_passkey_for_it() {
    let (fixture, clock) = hub_with_clock().await;
    let browser = Browser::of(&fixture);
    let seed = race_seed();
    let mut rng = SeededRng::new(seed);
    let mut played = std::collections::BTreeMap::new();
    for round in 0..RACE_ROUNDS {
        // A minute between rounds: the ceiling of the typed codes is not what is tested here.
        clock.advance(61_000);
        let rival = match rng.below(3) {
            0 => Rival::Retyped,
            1 => Rival::Issued,
            _ => Rival::Registered,
        };
        match race(&fixture, browser, &mut rng, round == 0, rival).await {
            Ok(kept) => *played.entry((rival, kept)).or_insert(0_u32) += 1,
            Err(why) => panic!("seed {seed}, round {round} against {rival:?}: {why}"),
        }
    }
    // What was played, and who came first: printed, since the order is the scheduler's.
    println!("seed {seed}: (rival, passkey kept) -> rounds: {played:?}");
    for rival in [Rival::Retyped, Rival::Issued, Rival::Registered] {
        assert!(
            played.keys().any(|(played, _)| *played == rival),
            "{rival:?} was never played: {played:?}"
        );
    }
}

/// R11 at the start: a database without exactly one organization issues no start code, and the
/// block says nothing, since neither a restart nor the command would help.
#[tokio::test(flavor = "multi_thread")]
async fn two_organizations_issue_no_start_code_and_name_no_remedy() {
    let fixture = hub().await;
    let id = Uuid::new_v4();
    let vault = Vault::open(&fixture.installation.data_dir).unwrap();
    sqlx::query(
        "INSERT INTO organization (id, name, state, key_enc, created_at) \
         VALUES ($1, 'second', 'active', $2, $3)",
    )
    .bind(id)
    .bind(vault.generate_organization_key(id).unwrap())
    .bind(fixture.now())
    .execute(&fixture.pool)
    .await
    .unwrap();

    let written = Arc::new(Recorder::default());
    let prepared = first_access::prepare(fixture.hub(), written.clone(), user()).await;
    assert!(
        matches!(prepared, Prepared::NotOneOrganization),
        "{prepared:?}"
    );
    assert!(written.writes().is_empty());
    assert!(rows(&fixture, "panel_entry").await.is_empty());
}

/// R1b inside the write that issues: a start code asked for once the mark of the first access
/// is set is not issued, even by whoever skips any read made before.
#[tokio::test(flavor = "multi_thread")]
async fn a_start_code_is_not_issued_once_the_mark_is_set_whoever_asks() {
    let fixture = hub().await;
    sqlx::query("UPDATE organization SET panel_first_passkey_at = $1")
        .bind(fixture.now())
        .execute(&fixture.pool)
        .await
        .unwrap();
    let written = Arc::new(Recorder::default());
    let prepared = first_access::prepare(fixture.hub(), written.clone(), user()).await;
    assert!(matches!(prepared, Prepared::AlreadyDone), "{prepared:?}");
    assert!(written.writes().is_empty());
    assert!(rows(&fixture, "panel_entry").await.is_empty());
}

/// R7: typing the code again closes the session it opened before.
#[tokio::test(flavor = "multi_thread")]
async fn typing_the_same_code_again_closes_the_session_it_opened_before() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let issued = fixture
        .admin
        .installation_code(fixture.now())
        .await
        .unwrap()
        .unwrap();
    let code = issued.code.reveal().to_string();
    let first = cookie_of(&browser.enter(&code).await).unwrap();
    assert_eq!(browser.get("/first-access", Some(&first)).await.status, 200);
    let second = cookie_of(&browser.enter(&code).await).unwrap();
    assert_ne!(first, second);

    let gone = browser.get("/first-access", Some(&first)).await;
    assert_eq!(
        (gone.status, gone.header("location")),
        (303, Some("/sign-in"))
    );
    assert_eq!(
        browser.get("/first-access", Some(&second)).await.status,
        200
    );
    let open: i64 = sqlx::query_scalar("SELECT count(*) FROM panel_session WHERE state = 'open'")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(open, 1, "one session per unspent code");
}

/// R2 and R3: a code of either kind cancels the unspent one in the same write, says which kind
/// cancelled it, and closes the sessions of the cancelled code that never reached the whole
/// panel, and only those.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_code_cancels_the_unspent_one_and_closes_its_unfinished_sessions_only() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let start = fixture
        .admin
        .installation_code(fixture.now())
        .await
        .unwrap()
        .unwrap();
    let typed = cookie_of(&browser.enter(&start.code.reveal()).await).unwrap();

    // A command code cancels the start code.
    let command = fixture.admin.panel_code(fixture.now()).await.unwrap();
    let by: Option<String> = one(
        &fixture,
        "SELECT cancelled_by FROM panel_entry WHERE id = $1",
        start.id,
    )
    .await;
    assert_eq!(by.as_deref(), Some("command"));
    let at: Option<i64> = one(
        &fixture,
        "SELECT cancelled_at FROM panel_entry WHERE id = $1",
        start.id,
    )
    .await;
    assert!(at.is_some());
    assert!(
        browser
            .enter(&start.code.reveal())
            .await
            .text()
            .contains(REFUSED)
    );
    let gone = browser.get("/first-access", Some(&typed)).await;
    assert_eq!(
        (gone.status, gone.header("location")),
        (303, Some("/sign-in"))
    );
    let reason: String = one(
        &fixture,
        "SELECT close_reason FROM panel_session WHERE entry_id = $1",
        start.id,
    )
    .await;
    assert_eq!(reason, "code_replaced");

    // A start cancels a command code, and says so. (The first access was not done.)
    let again = fixture
        .admin
        .installation_code(fixture.now())
        .await
        .unwrap()
        .unwrap();
    let by: Option<String> = one(
        &fixture,
        "SELECT cancelled_by FROM panel_entry WHERE id = $1",
        command.id,
    )
    .await;
    assert_eq!(by.as_deref(), Some("start"));
    let unspent: i64 =
        sqlx::query_scalar("SELECT count(*) FROM panel_entry WHERE state = 'issued'")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(unspent, 1, "one code at a time");

    // The first access is done. A session that reaches the whole panel survives a new code;
    // one that does not reach it ends with its code.
    let full = cookie_of(&browser.enter(&again.code.reveal()).await).unwrap();
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    assert_eq!(
        browser.register(&full, &mut authenticator).await.status,
        303
    );
    let command = fixture.admin.panel_code(fixture.now()).await.unwrap();
    let setup = cookie_of(&browser.enter(&command.code.reveal()).await).unwrap();
    assert_eq!(browser.get("/first-access", Some(&setup)).await.status, 200);
    fixture.admin.panel_code(fixture.now()).await.unwrap();
    assert_eq!(browser.get("/", Some(&full)).await.status, 200);
    assert_eq!(browser.get("/first-access", Some(&setup)).await.status, 303);
    // And a start no longer comes back: the command is the way.
    assert!(
        fixture
            .admin
            .installation_code(fixture.now())
            .await
            .unwrap()
            .is_none()
    );
    let kinds = rows(&fixture, "panel_entry").await;
    assert_eq!(kinds.len(), 5);
}

/// R8, through the function `serve` calls: a lost panel session (a closed browser, 30 minutes
/// idle, hours later, a restarted hub) needs only the latest code: the same one when the hub
/// did not restart, the one of the newest block when it did.
#[tokio::test(flavor = "multi_thread")]
async fn a_lost_panel_session_needs_only_the_latest_code_through_the_real_start() {
    let (mut fixture, clock) = hub_with_clock().await;
    let written = Arc::new(Recorder::default());
    let task = first_access::start(fixture.hub(), written.clone(), user()).await;
    assert_eq!(written.count(), 1);
    let code = written.last_code();
    let browser = Browser::of(&fixture);

    // The browser was closed: the same code, typed again.
    let lost = cookie_of(&browser.enter(&code).await).unwrap();
    let found = cookie_of(&browser.enter(&code).await).unwrap();
    assert_eq!(browser.get("/first-access", Some(&lost)).await.status, 303);
    assert_eq!(browser.get("/first-access", Some(&found)).await.status, 200);

    // 30 minutes idle, and then hours: a start code has no deadline.
    clock.advance(SESSION_IDLE_MS);
    assert_eq!(browser.get("/first-access", Some(&found)).await.status, 303);
    clock.advance(5 * 3_600_000);
    let hours_later = cookie_of(&browser.enter(&code).await).unwrap();
    assert_eq!(
        browser
            .get("/first-access", Some(&hours_later))
            .await
            .status,
        200
    );

    // The hub restarts: it writes a new block with a new code, and the old one is dead.
    task.stop().await;
    fixture.stop().await;
    fixture.start().await;
    let browser = Browser::of(&fixture);
    let restarted = Arc::new(Recorder::default());
    let task = first_access::start(fixture.hub(), restarted.clone(), user()).await;
    let newest = restarted.last_code();
    assert_ne!(newest, code);
    assert_eq!(
        browser
            .get("/first-access", Some(&hours_later))
            .await
            .status,
        303
    );
    assert!(browser.enter(&code).await.text().contains(REFUSED));
    let cookie = cookie_of(&browser.enter(&newest).await).unwrap();
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    let kept = browser.register(&cookie, &mut authenticator).await;
    assert_eq!(
        (kept.status, kept.header("location")),
        (303, Some("/installation?welcome"))
    );
    task.stop().await;
}

/// R9, with the fake clock of the test: the block is written again every 10 minutes, and not
/// before; every write is preceded by a read of the database, and a read that fails writes
/// nothing.
#[tokio::test(flavor = "multi_thread")]
async fn the_block_is_written_again_every_ten_minutes_and_a_failed_read_writes_nothing() {
    let (fixture, _clock) = hub_with_clock().await;
    let written = Arc::new(Recorder::default());
    let Prepared::Ready(mut task) =
        first_access::prepare(fixture.hub(), written.clone(), user()).await
    else {
        panic!("a start code was issued");
    };
    assert_eq!(written.count(), 1);
    let start = fixture.now();

    assert_eq!(task.tick(start + REPEAT_MS - 1).await, Step::Wait(1));
    assert_eq!(written.count(), 1, "not before its time");
    assert_eq!(task.tick(start + REPEAT_MS).await, Step::Wait(REPEAT_MS));
    assert_eq!(written.count(), 2);
    let blocks = written.writes();
    let code_line = |block: &str| {
        block
            .lines()
            .find(|line| line.starts_with("INSTALLATION CODE: "))
            .map(str::to_owned)
    };
    assert_eq!(code_line(&blocks[0]), code_line(&blocks[1]));
    assert_ne!(blocks[0], blocks[1], "the time of the block moved");
    assert_eq!(
        task.tick(start + REPEAT_MS + 5).await,
        Step::Wait(REPEAT_MS - 5)
    );
    assert_eq!(written.count(), 2);

    // The database does not answer: nothing is written, and the read is made again.
    fixture.hub().pool().close().await;
    assert_eq!(task.tick(start + 2 * REPEAT_MS).await, Step::Wait(60_000));
    assert_eq!(written.count(), 2, "a read that fails writes nothing");
    assert!(task.holds_the_code());
}

/// R9: a command code that took the place of the start code makes the task say so, once, and
/// stop; another start taking its place stops it in silence. The holder is cleared.
#[tokio::test(flavor = "multi_thread")]
async fn the_task_says_replaced_once_for_a_command_code_and_stops_silent_for_another_start() {
    let (fixture, _clock) = hub_with_clock().await;
    let start = fixture.now();

    let first = Arc::new(Recorder::default());
    let Prepared::Ready(mut replaced) =
        first_access::prepare(fixture.hub(), first.clone(), user()).await
    else {
        panic!("a start code was issued");
    };
    fixture.admin.panel_code(fixture.now()).await.unwrap();
    assert_eq!(replaced.tick(start + REPEAT_MS).await, Step::Finished);
    let writes = first.writes();
    assert_eq!(writes.len(), 2);
    assert!(
        writes[1].contains(
            "The code above was replaced by one issued with arauto-hub admin panel code."
        )
    );
    assert!(!writes[1].contains("INSTALLATION CODE"));
    assert!(!replaced.holds_the_code());

    // The hub starts again: a second start code, and a third that cancels it.
    let second = Arc::new(Recorder::default());
    let Prepared::Ready(mut superseded) =
        first_access::prepare(fixture.hub(), second.clone(), user()).await
    else {
        panic!("a start code was issued");
    };
    let third = Arc::new(Recorder::default());
    let Prepared::Ready(_latest) =
        first_access::prepare(fixture.hub(), third.clone(), user()).await
    else {
        panic!("a start code was issued");
    };
    assert_eq!(superseded.tick(start + REPEAT_MS).await, Step::Finished);
    assert_eq!(second.count(), 1, "cancelled by another start: silence");
    assert!(!superseded.holds_the_code());
}

/// R10, R9 and R5: no request writes the block; only the authenticated registration of the first
/// passkey signals the task, which then writes the closing line once, clears its holder and ends.
///
/// What this test sees is what goes through the output of the task and the count of the
/// signals. A request handler that wrote straight to the standard error of the process would
/// escape it; what keeps the code out of every request is that no handler can reach the holder
/// or an output of the task (`first_access`, R5), not this test.
#[tokio::test(flavor = "multi_thread")]
async fn no_request_moves_the_writer_of_the_block_but_the_first_passkey() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let written = Arc::new(Recorder::default());
    let task = first_access::start(fixture.hub(), written.clone(), user()).await;
    assert_eq!(written.count(), 1);
    let code = written.last_code();

    // Anonymous reads, refused forms, a wrong code, a garbage credential.
    for path in [
        "/",
        "/sign-in",
        "/first-access",
        "/assets/panel.css",
        "/installation",
    ] {
        browser.get(path, None).await;
    }
    for (path, body) in [
        ("/sign-in", "code=abcd-efgh"),
        ("/passkey/register/start", ""),
        ("/passkey/register", "credential=x"),
        ("/passkey/sign-in/start", ""),
        ("/passkey/sign-in", "credential=x"),
        ("/sign-out", ""),
    ] {
        browser.post(path, None, body).await;
    }
    // Typing the right code and a refused credential in its session do not signal either.
    let cookie = cookie_of(&browser.enter(&code).await).unwrap();
    browser
        .post("/passkey/register", Some(&cookie), "credential=garbage")
        .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(written.count(), 1, "nothing but the start wrote the block");
    assert_eq!(
        fixture.hub().first_passkey_signals(),
        0,
        "no request but the first passkey signals the task"
    );
    assert!(task.running());

    // The first passkey: the task is signalled and writes the closing line once.
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    assert_eq!(
        browser.register(&cookie, &mut authenticator).await.status,
        303
    );
    for _ in 0..200 {
        if written.count() >= 2 && !task.running() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(!task.running(), "the task ended");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let writes = written.writes();
    assert_eq!(writes.len(), 2);
    assert!(
        writes[1].starts_with("=== ARAUTO FIRST ACCESS: done ("),
        "{}",
        writes[1]
    );
    assert!(writes[1].ends_with("UTC). From now on the panel is entered with your passkey. ===\n"));
    assert!(!writes[1].contains(&code));
    assert_eq!(fixture.hub().first_passkey_signals(), 1);

    // A second passkey, and a sign-in with the first, are not the first passkey.
    let mut second = SoftAuthenticator::new(PANEL_ORIGIN);
    assert_eq!(browser.register(&cookie, &mut second).await.status, 303);
    assert_eq!(browser.pass(&mut authenticator).await.status, 303);
    assert_eq!(fixture.hub().first_passkey_signals(), 1);
    assert_eq!(written.count(), 2);
}

/// R1: a hub with no panel issues no start code and says so.
#[tokio::test(flavor = "multi_thread")]
async fn a_hub_with_no_panel_issues_no_start_code_and_says_so() {
    let settings = fixture::settings();
    let fixture = Fixture::stopped(
        support::install().await,
        settings.clone(),
        Arc::new(SystemClock),
    )
    .await;
    let mut config = support::config(&fixture.installation, ClientIpSource::Socket);
    config.panel = None;
    let running = channel::start(
        &config,
        Options {
            settings,
            downloads: Downloads::empty(),
            purposes: Served,
        },
        Arc::new(SystemClock),
    )
    .await
    .unwrap();
    assert!(running.panel_addr().is_none());

    let written = Arc::new(Recorder::default());
    let task = first_access::start(running.hub(), written.clone(), user()).await;
    assert_eq!(written.writes(), [format!("{NO_PANEL_LINE}\n")]);
    assert!(!task.running());
    assert!(rows(&fixture, "panel_entry").await.is_empty());
    running.stop().await;
}

/// The start code could not be issued: one line that names the remedy and carries no error.
#[tokio::test(flavor = "multi_thread")]
async fn a_start_code_that_could_not_be_issued_says_so_without_the_error() {
    let fixture = hub().await;
    fixture.hub().pool().close().await;
    let written = Arc::new(Recorder::default());
    let prepared = first_access::prepare(fixture.hub(), written.clone(), user()).await;
    assert!(matches!(prepared, Prepared::NotIssued));
    assert_eq!(written.writes(), [format!("{NOT_ISSUED_LINE}\n")]);
}

/// R5: when the hub stops, the task ends and its holder goes with it. The task owns the holder
/// and the output together, so the output being let go shows that the holder was dropped.
#[tokio::test(flavor = "multi_thread")]
async fn stopping_the_task_drops_its_holder_with_it() {
    let fixture = hub().await;
    let written = Arc::new(Recorder::default());
    let task = first_access::start(fixture.hub(), written.clone(), user()).await;
    assert!(task.running());
    assert_eq!(
        Arc::strong_count(&written),
        2,
        "the task holds the output beside the holder"
    );
    task.stop().await;
    assert_eq!(
        Arc::strong_count(&written),
        1,
        "the task, its output and its holder are gone"
    );
    assert_eq!(written.count(), 1, "stopping writes nothing");
}

/// R9: a first passkey whose write the audit mark could not follow (another process holds the
/// lock of the mark) is kept all the same, the admin is told, and the task hears of it at once:
/// the closing line does not wait for the next look, 10 minutes later.
#[tokio::test(flavor = "multi_thread")]
async fn a_first_passkey_the_audit_mark_did_not_follow_still_signals_the_task() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let written = Arc::new(Recorder::default());
    let task = first_access::start(fixture.hub(), written.clone(), user()).await;
    let cookie = cookie_of(&browser.enter(&written.last_code()).await).unwrap();

    let other_process = arauto_platform::FileLock::try_acquire(
        &fixture
            .installation
            .data_dir
            .join(arauto_hub::mark::MARK_LOCK_FILE),
    )
    .unwrap();
    let mut authenticator = SoftAuthenticator::new(PANEL_ORIGIN);
    let answered = browser.register(&cookie, &mut authenticator).await;
    drop(other_process);
    // The passkey is kept and the session reaches the whole panel: the person lands on the check
    // window, where the frame shows the alert, and not on a bare 500 that says the passkey is
    // lost.
    assert_eq!(
        (answered.status, answered.header("location")),
        (303, Some("/installation?welcome")),
        "{}",
        answered.text()
    );
    let welcome = browser.get("/installation?welcome", Some(&cookie)).await;
    assert_eq!(welcome.status, 200);
    assert_eq!(passkeys(&fixture).await, 1, "the passkey is kept");
    let alerts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM alert WHERE kind = 'mark_not_advanced'")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(alerts, 1, "the admin is told");
    assert_eq!(fixture.hub().first_passkey_signals(), 1);

    for _ in 0..200 {
        if !task.running() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        !task.running(),
        "the task ended without waiting for its next look"
    );
    let writes = written.writes();
    assert_eq!(writes.len(), 2);
    assert!(
        writes[1].starts_with("=== ARAUTO FIRST ACCESS: done ("),
        "{}",
        writes[1]
    );
    task.stop().await;
}

/// R5 and R9: a code that was cancelled is let go of at the next waking of the task, a minute
/// later, and not at the next block ten minutes later: the holder is cleared and the "replaced"
/// block is written, or the task stops in silence for another start.
#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_code_is_let_go_at_the_next_waking_and_not_at_the_next_block() {
    let (fixture, _clock) = hub_with_clock().await;
    let start = fixture.now();

    let first = Arc::new(Recorder::default());
    let Prepared::Ready(mut replaced) =
        first_access::prepare(fixture.hub(), first.clone(), user()).await
    else {
        panic!("a start code was issued");
    };
    // Nothing happened: a waking a minute later writes nothing and keeps the code.
    assert_eq!(
        replaced.tick(start + 60_000).await,
        Step::Wait(REPEAT_MS - 60_000)
    );
    assert_eq!(first.count(), 1);
    assert!(replaced.holds_the_code());
    fixture.admin.panel_code(fixture.now()).await.unwrap();
    assert_eq!(replaced.tick(start + 61_000).await, Step::Finished);
    assert!(
        !replaced.holds_the_code(),
        "the dead code is let go at once"
    );
    let writes = first.writes();
    assert_eq!(writes.len(), 2);
    assert!(writes[1].contains("was replaced by one issued with arauto-hub admin panel code"));

    let second = Arc::new(Recorder::default());
    let Prepared::Ready(mut superseded) =
        first_access::prepare(fixture.hub(), second.clone(), user()).await
    else {
        panic!("a start code was issued");
    };
    let third = Arc::new(Recorder::default());
    let Prepared::Ready(_latest) =
        first_access::prepare(fixture.hub(), third.clone(), user()).await
    else {
        panic!("a start code was issued");
    };
    assert_eq!(superseded.tick(start + 62_000).await, Step::Finished);
    assert!(!superseded.holds_the_code());
    assert_eq!(second.count(), 1, "cancelled by another start: silence");
}

/// R1b under a race, in the interleaving `register_passkey` really runs: another hub is keeping
/// the first passkey and holds the row of the start code, and has not yet spent it nor touched
/// the organization. A start code asked for meanwhile waits on that row; the other then spends
/// the code, sets the mark and commits, and the start code is not issued. If the mark were read
/// before the cancel waited on the row of the code, it would be read as not set: a code with no
/// deadline would be issued after the first access, or the two writes would deadlock on the
/// organization.
#[tokio::test(flavor = "multi_thread")]
async fn a_start_code_waits_for_a_first_passkey_being_kept_and_is_then_not_issued() {
    let fixture = hub().await;
    let first = fixture
        .admin
        .installation_code(fixture.now())
        .await
        .unwrap()
        .unwrap();
    // Only what `register_passkey` takes first: the row of the code.
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM panel_entry WHERE id = $1 FOR UPDATE")
        .bind(first.id)
        .fetch_one(&mut *holder)
        .await
        .unwrap();

    let issue = fixture.admin.installation_code(fixture.now());
    let keep = async {
        assert!(
            someone_waits_on_a_lock(&fixture).await,
            "the start code never waited on the row of the code"
        );
        // Now, with the start code waiting, the rest of the keeping of the first passkey.
        sqlx::query(
            "UPDATE panel_entry SET state = 'used', digest = NULL, used_at = $2 WHERE id = $1",
        )
        .bind(first.id)
        .bind(fixture.now())
        .execute(&mut *holder)
        .await
        .unwrap();
        sqlx::query("UPDATE organization SET panel_first_passkey_at = $1")
            .bind(fixture.now())
            .execute(&mut *holder)
            .await
            .unwrap();
        holder.commit().await.unwrap();
    };
    let (second, ()) = tokio::join!(issue, keep);
    assert!(
        second.unwrap().is_none(),
        "a start code was issued after the first access"
    );
    let unspent: i64 =
        sqlx::query_scalar("SELECT count(*) FROM panel_entry WHERE state = 'issued'")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(unspent, 0);
    let states: Vec<(String,)> = sqlx::query_as("SELECT state FROM panel_entry")
        .fetch_all(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(
        states,
        vec![("used".to_owned(),)],
        "the spent code stays spent"
    );
}

/// A passkey row as an old version of the hub could have kept it: removed, and with the mark of
/// the first access not set.
async fn plant_a_passkey_without_the_mark(fixture: &Fixture, credential: &[u8], created_at: i64) {
    sqlx::query(
        "INSERT INTO panel_passkey (id, organization_id, admin_id, credential_id, user_handle, \
                                    static_state, dynamic_state, state, created_at, removed_at) \
         SELECT $1, o.id, a.id, $2, $2, $2, $2, 'removed', $3, $3 \
         FROM organization o CROSS JOIN admin a",
    )
    .bind(Uuid::new_v4())
    .bind(credential)
    .bind(created_at)
    .execute(&fixture.pool)
    .await
    .unwrap();
}

/// D4: a passkey of the organization, in any state, says the first access was done even when the
/// mark is not there (an old hub kept it): no start code is issued, nothing is written where the
/// block goes, and the mark is set from the oldest passkey.
#[tokio::test(flavor = "multi_thread")]
async fn a_passkey_kept_without_the_mark_gives_no_start_code_and_sets_the_mark() {
    let fixture = hub().await;
    plant_a_passkey_without_the_mark(&fixture, b"credential-b", 5_000).await;
    plant_a_passkey_without_the_mark(&fixture, b"credential-a", 1_234).await;
    let before: Option<i64> = sqlx::query_scalar("SELECT panel_first_passkey_at FROM organization")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(before, None);

    let written = Arc::new(Recorder::default());
    let prepared = first_access::prepare(fixture.hub(), written.clone(), user()).await;
    assert!(matches!(prepared, Prepared::AlreadyDone), "{prepared:?}");
    assert!(written.writes().is_empty());
    assert!(rows(&fixture, "panel_entry").await.is_empty());
    let after: Option<i64> = sqlx::query_scalar("SELECT panel_first_passkey_at FROM organization")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(after, Some(1_234), "the mark is the oldest passkey");
    // The entry follows the mark that was set.
    let page = Browser::of(&fixture).get("/sign-in", None).await.text();
    assert!(page.contains("Good to have you back"), "{page}");
}

/// D4 with a code waiting: the mark that heals itself keeps a command code someone issued (it is
/// still typed) and cancels a start code left from before, which is dead with the first access
/// done.
#[tokio::test(flavor = "multi_thread")]
async fn the_mark_that_heals_itself_keeps_a_command_code_and_cancels_a_start_code() {
    let fixture = hub().await;
    let command = fixture.admin.panel_code(fixture.now()).await.unwrap();
    plant_a_passkey_without_the_mark(&fixture, b"credential-a", 1_234).await;
    let written = Arc::new(Recorder::default());
    let prepared = first_access::prepare(fixture.hub(), written.clone(), user()).await;
    assert!(matches!(prepared, Prepared::AlreadyDone), "{prepared:?}");
    assert!(written.writes().is_empty());
    let state: String = one(
        &fixture,
        "SELECT state FROM panel_entry WHERE id = $1",
        command.id,
    )
    .await;
    assert_eq!(
        state, "issued",
        "a start that issues nothing replaced a command code"
    );
    let browser = Browser::of(&fixture);
    assert!(
        cookie_of(&browser.enter(&command.code.reveal()).await).is_some(),
        "the command code is still typed"
    );

    let fixture = hub().await;
    let left = fixture
        .admin
        .installation_code(fixture.now())
        .await
        .unwrap()
        .unwrap();
    plant_a_passkey_without_the_mark(&fixture, b"credential-a", 1_234).await;
    let prepared =
        first_access::prepare(fixture.hub(), Arc::new(Recorder::default()), user()).await;
    assert!(matches!(prepared, Prepared::AlreadyDone), "{prepared:?}");
    let (state, by): (String, Option<String>) =
        sqlx::query_as("SELECT state, cancelled_by FROM panel_entry WHERE id = $1")
            .bind(left.id)
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(
        (state.as_str(), by.as_deref()),
        ("cancelled", Some("start"))
    );
    let page = Browser::of(&fixture)
        .enter(&left.code.reveal())
        .await
        .text();
    assert!(page.contains(REFUSED), "{page}");
}

/// R2: a command code is valid for 10 minutes, said in a number and not by the constant that
/// holds it.
#[tokio::test(flavor = "multi_thread")]
async fn a_command_code_is_valid_for_ten_minutes_exactly() {
    let (fixture, clock) = hub_with_clock().await;
    let now = clock.now_ms();
    let issued = fixture.admin.panel_code(now).await.unwrap();
    assert_eq!(issued.expires_at - now, 600_000);
    let stored: Option<i64> = one(
        &fixture,
        "SELECT expires_at FROM panel_entry WHERE id = $1",
        issued.id,
    )
    .await;
    assert_eq!(stored, Some(now + 600_000));
    let browser = Browser::of(&fixture);
    let code = issued.code.reveal().to_string();
    clock.advance(599_000);
    assert_eq!(browser.enter(&code).await.status, 303);
    clock.advance(1_000);
    assert!(browser.enter(&code).await.text().contains(REFUSED));
}

/// R3: a new code closes the sessions of the code it cancels that never reached the whole panel,
/// and only those: a session that reached the whole panel stays open, even when it hangs on a
/// code that is still unspent (the filter on how far it reaches is what keeps it).
#[tokio::test(flavor = "multi_thread")]
async fn a_new_code_leaves_open_a_session_that_reached_the_whole_panel() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let start = fixture
        .admin
        .installation_code(fixture.now())
        .await
        .unwrap()
        .unwrap();
    let cookie = cookie_of(&browser.enter(&start.code.reveal()).await).unwrap();
    assert_eq!(
        browser.get("/first-access", Some(&cookie)).await.status,
        200
    );
    // A session that reaches the whole panel with its code still unspent: not a state a request
    // makes, so it is made here.
    sqlx::query("UPDATE panel_session SET reach = 'full' WHERE entry_id = $1")
        .bind(start.id)
        .execute(&fixture.pool)
        .await
        .unwrap();
    fixture.admin.panel_code(fixture.now()).await.unwrap();
    assert_eq!(
        browser.get("/", Some(&cookie)).await.status,
        200,
        "a session that reached the whole panel is not closed with its code"
    );
}

/// R9: the loop that runs in the hub. A signal with the code unspent writes nothing; a command
/// code that replaced the start code makes the running task write the "replaced" block once and
/// end, and not again a minute later.
#[tokio::test(flavor = "multi_thread")]
async fn the_running_task_says_replaced_once_and_ends_and_writes_nothing_before_its_time() {
    let (fixture, _clock) = hub_with_clock().await;
    let written = Arc::new(Recorder::default());
    let task = first_access::start(fixture.hub(), written.clone(), user()).await;
    assert_eq!(written.count(), 1);

    // The hub clock did not move: a look writes nothing.
    fixture.hub().first_passkey_kept();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(written.count(), 1, "nothing is written before its time");
    assert!(task.running());

    fixture.admin.panel_code(fixture.now()).await.unwrap();
    fixture.hub().first_passkey_kept();
    for _ in 0..200 {
        if !task.running() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(!task.running(), "the task ends after the replaced block");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let writes = written.writes();
    assert_eq!(writes.len(), 2, "{writes:?}");
    assert!(writes[1].contains("was replaced by one issued with arauto-hub admin panel code"));
}

/// R11 and the join of the entry with the start: when the panel does not know which organization
/// it serves (it started with two), no start code is issued even if the database has only one by
/// the time the start code is asked for. One resolution for both, or the log would write a block
/// for an entry that shows the page of who comes back.
#[tokio::test(flavor = "multi_thread")]
async fn no_start_code_is_issued_when_the_panel_does_not_know_its_organization() {
    let mut fixture = hub().await;
    let id = Uuid::new_v4();
    let vault = Vault::open(&fixture.installation.data_dir).unwrap();
    sqlx::query(
        "INSERT INTO organization (id, name, state, key_enc, created_at) \
         VALUES ($1, 'second', 'active', $2, $3)",
    )
    .bind(id)
    .bind(vault.generate_organization_key(id).unwrap())
    .bind(fixture.now())
    .execute(&fixture.pool)
    .await
    .unwrap();
    // The hub starts with two organizations: its panel serves none.
    fixture.stop().await;
    fixture.start().await;
    sqlx::query("DELETE FROM organization WHERE id = $1")
        .bind(id)
        .execute(&fixture.pool)
        .await
        .unwrap();

    let written = Arc::new(Recorder::default());
    let prepared = first_access::prepare(fixture.hub(), written.clone(), user()).await;
    assert!(
        matches!(prepared, Prepared::NotOneOrganization),
        "{prepared:?}"
    );
    assert!(written.writes().is_empty());
    assert!(rows(&fixture, "panel_entry").await.is_empty());
}
