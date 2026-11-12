//! A hub of its own with the fake engine configured as its engine, and the steps of the admin
//! as functions.

use std::sync::Arc;
use std::time::Duration;

use arauto_hub::admin::whatsapp::{PairMethod, PairOutcome};
use arauto_hub::channel::Hub;
use arauto_hub::core::number::NumberState;
use arauto_hub::db::ResolvedOrg;
use arauto_hub::engine::registry::LoadedEngine;
use arauto_hub::engine::waha::Waha;
use arauto_hub::engine::{ApiKey, Deadlines, EngineAddress, SessionName};
use arauto_hub::repo::number::Number;
use arauto_hub::vault::Vault;
use arauto_protocol::names::NumberAlias;
use arauto_testkit::engine::FakeEngine;
use serde_json::{Value, json};

use crate::fixture::{self, Fixture};

/// The key the fake engine takes.
pub const KEY: &str = "fake-engine-key-0001";

/// Short deadlines: the fake answers at once unless told to be slow.
pub const DEADLINES: Deadlines = Deadlines {
    connect: Duration::from_secs(2),
    call: Duration::from_secs(2),
};

/// A telephone nobody uses, with the code of the country.
pub fn phone(n: u32) -> String {
    format!("55119876{n:05}")
}

/// An adapter for the fake engine, outside any hub.
pub fn waha(fake: &FakeEngine) -> Waha {
    Waha::new(
        EngineAddress::parse(&fake.address()).unwrap(),
        Some(ApiKey::parse(KEY).unwrap()),
        DEADLINES,
    )
}

/// A session of the engine of its own.
pub fn session() -> SessionName {
    SessionName::draw().unwrap()
}

/// A message as the engine keeps it.
pub fn message(chat: &str, id: &str, at_s: i64, from_me: bool, body: &str) -> Value {
    json!({
        "id": format!("{}_{chat}_{id}", if from_me { "true" } else { "false" }),
        "timestamp": at_s,
        "from": chat,
        "fromMe": from_me,
        "body": body,
        "hasMedia": false,
        "ack": 1,
    })
}

/// A hub with the fake engine as its engine.
pub struct World {
    pub fixture: Fixture,
    pub fake: FakeEngine,
    pub vault: Vault,
}

impl World {
    pub async fn new() -> Self {
        Self::with(|_| {}, Arc::new(arauto_protocol::clock::SystemClock)).await
    }

    /// A world whose hub runs on `clock`, with its settings changed by `change`.
    pub async fn with(
        change: impl FnOnce(&mut arauto_hub::channel::ChannelSettings),
        clock: Arc<dyn arauto_protocol::clock::Clock>,
    ) -> Self {
        let mut settings = fixture::settings();
        // Only the test runs the supervision, and the admin command does not wake it.
        settings.listen_admin = false;
        settings.engine.supervise_interval_ms = 3_600_000;
        settings.engine.pairing_poll_ms = 50;
        settings.engine.deadlines = DEADLINES;
        change(&mut settings);
        let fixture = Fixture::with_clock(settings, clock).await;
        let fake = FakeEngine::start(KEY).await.unwrap();
        let vault = Vault::open(&fixture.installation.data_dir).unwrap();
        let world = Self {
            fixture,
            fake,
            vault,
        };
        let events = format!("http://{}", world.fixture.running().internal_addr());
        world
            .fixture
            .admin
            .engine_set(&world.vault, &world.fake.address(), &events, world.now())
            .await
            .unwrap();
        world
            .fixture
            .admin
            .engine_set_key(&world.vault, KEY, world.now())
            .await
            .unwrap();
        world
    }

    pub fn now(&self) -> i64 {
        self.fixture.now()
    }

    pub fn hub(&self) -> &Arc<Hub> {
        self.fixture.hub()
    }

    pub fn organization(&self) -> ResolvedOrg {
        self.fixture.installation.organization
    }

    pub async fn engine(&self) -> Arc<LoadedEngine> {
        self.hub()
            .engines()
            .of(self.hub().pool(), &self.vault, self.organization())
            .await
            .unwrap()
    }

    /// `admin number add`.
    pub async fn add(&self, alias: &str, phone: &str) -> Number {
        self.fixture
            .admin
            .number_add(&self.vault, alias, phone, false, self.now())
            .await
            .unwrap()
            .number
    }

    /// `admin number pair ALIAS` with the QR, the telephone `pairs_as` using it.
    pub async fn pair(&self, alias: &str, pairs_as: &str) -> PairOutcome {
        let number = self.number(alias).await;
        self.fake.pairs_as(number.session.as_str(), pairs_as);
        let settings = self.hub().engines().settings().clone();
        self.fixture
            .admin
            .number_pair(
                &self.vault,
                alias,
                PairMethod::Qr,
                &settings,
                &arauto_protocol::clock::SystemClock,
                |_| {},
            )
            .await
            .unwrap()
    }

    /// A number added and paired with its own telephone.
    pub async fn connected(&self, alias: &str, phone: &str) -> Number {
        self.add(alias, phone).await;
        assert_eq!(self.pair(alias, phone).await, PairOutcome::Connected);
        self.number(alias).await
    }

    pub async fn number(&self, alias: &str) -> Number {
        arauto_hub::repo::number::by_alias(
            self.hub().pool(),
            self.organization(),
            &NumberAlias::parse(alias).unwrap(),
        )
        .await
        .unwrap()
        .expect("the number exists")
    }

    pub async fn state(&self, alias: &str) -> NumberState {
        self.number(alias).await.state
    }

    /// How many rows the inbox has, of each kind.
    pub async fn events(&self, kind: &str) -> i64 {
        sqlx::query_scalar(
            "SELECT count(*) FROM engine_event WHERE organization_id = $1 AND kind = $2",
        )
        .bind(self.organization().id())
        .bind(kind)
        .fetch_one(self.hub().pool())
        .await
        .unwrap()
    }

    /// The kinds of alert raised, oldest first.
    pub async fn alerts(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT kind FROM alert WHERE organization_id = $1 ORDER BY at, id")
            .bind(self.organization().id())
            .fetch_all(self.hub().pool())
            .await
            .unwrap()
    }

    /// The actions of the audit, oldest first, with the kind of actor.
    pub async fn audit(&self) -> Vec<(String, String)> {
        sqlx::query_as(
            "SELECT action, actor_kind FROM audit WHERE organization_id = $1 ORDER BY seq",
        )
        .bind(self.organization().id())
        .fetch_all(self.hub().pool())
        .await
        .unwrap()
    }
}
