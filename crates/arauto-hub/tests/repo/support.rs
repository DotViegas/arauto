//! What the tests of the repositories share: an organization with its admin and its key, a
//! manual clock, and the steps that take a Hermes from a line to an active device.

use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};

use arauto_hub::core::judge::Credential;
use arauto_hub::core::model::Device;
use arauto_hub::db::ResolvedOrg;
use arauto_hub::repo::approval::{self, Answer, ApproveOutcome, Recognition, Recognized};
use arauto_hub::repo::connection::{self, ChannelConnection};
use arauto_hub::repo::enroll::{self, EnrollOutcome, Enrollment, RefusedEnrollments};
use arauto_hub::repo::hermes::{self, CreateOutcome};
use arauto_hub::repo::invite::{self, IssueOutcome, IssuedInvite};
use arauto_hub::repo::swap::{self, ConfirmOutcome, IssueOutcome as SwapOutcome, Issued};
use arauto_hub::repo::{self, Admin};
use arauto_hub::vault::OrganizationKey;
use arauto_protocol::clock::{Clock, ManualClock};
use arauto_protocol::code::VerificationCode;
use arauto_protocol::message::{Declared, DeclaredText, Saved};
use arauto_protocol::names::HermesName;
use arauto_protocol::noise::{HandshakeHash, PublicKey};
use arauto_protocol::secret::SealedCredential;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::common::{self, NOW, TestDb, TestHome};

static COUNTER: AtomicU64 = AtomicU64::new(1);

fn next() -> u64 {
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// An address of the documentation range.
pub fn address(last: u8) -> Option<IpAddr> {
    Some(IpAddr::from([192, 0, 2, last]))
}

/// An identity nobody else in this run has.
pub fn identity() -> PublicKey {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    PublicKey::from_bytes(bytes)
}

/// One organization, as a test sees it.
pub struct World {
    pub db: TestDb,
    pub home: TestHome,
    pub org: ResolvedOrg,
    pub admin: Admin,
    pub key: OrganizationKey,
    pub clock: ManualClock,
    pub refusals: RefusedEnrollments,
}

/// A request that enrolled, and what the test needs to approve it.
pub struct Request {
    pub device: Uuid,
    pub identity: PublicKey,
    pub code: VerificationCode,
}

/// A device that confirmed a credential, and what its connector holds.
pub struct Connected {
    pub device: Uuid,
    pub identity: PublicKey,
    pub connection: ChannelConnection,
    /// The current credential, as the connector stores it.
    pub sealed: SealedCredential,
    pub credential: Credential,
}

impl World {
    /// An organization of its own in the database every test shares.
    pub async fn shared() -> Self {
        Self::on(TestDb::shared().await).await
    }

    /// A database of its own, installed: for what touches the whole installation.
    pub async fn own() -> Self {
        let db = TestDb::own().await;
        common::install(&db.pool).await;
        Self::on(db).await
    }

    async fn on(db: TestDb) -> Self {
        let home = TestHome::new();
        home.mark.advance(0).await.unwrap();
        let org = common::organization(&db.pool, &home).await;
        sqlx::query(
            "INSERT INTO admin (id, organization_id, name, kind, created_at) \
             VALUES ($1, $2, 'local', 'local', $3)",
        )
        .bind(Uuid::new_v4())
        .bind(org.id())
        .bind(NOW)
        .execute(&db.pool)
        .await
        .unwrap();
        let admin = repo::local_admin(&db.pool, org, DeclaredText::parse("tester").unwrap())
            .await
            .unwrap()
            .unwrap();
        let key = repo::device::organization_key(&db.pool, org, &home.vault)
            .await
            .unwrap();
        Self {
            db,
            home,
            org,
            admin,
            key,
            clock: ManualClock::new(NOW),
            refusals: RefusedEnrollments::new(),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.db.pool
    }

    pub fn now(&self) -> i64 {
        self.clock.now_ms()
    }

    /// Creates a Hermes line with a name of its own.
    pub async fn hermes(&self) -> HermesName {
        let name = HermesName::parse(&format!("t{}", next())).unwrap();
        let created = hermes::create(
            self.pool(),
            &self.home.mark,
            self.org,
            &self.admin,
            &name,
            self.now(),
        )
        .await
        .unwrap();
        assert!(matches!(created, CreateOutcome::Created(_)));
        name
    }

    pub async fn hermes_id(&self, name: &HermesName) -> Uuid {
        hermes::find(self.pool(), self.org, name)
            .await
            .unwrap()
            .unwrap()
            .id
    }

    pub async fn invite(&self, name: &HermesName) -> Box<IssuedInvite> {
        match invite::issue(
            self.pool(),
            &self.home.mark,
            self.org,
            &self.admin,
            name,
            self.now(),
        )
        .await
        .unwrap()
        {
            IssueOutcome::Issued(issued) => issued,
            IssueOutcome::NotFound => panic!("there is no Hermes {name}"),
        }
    }

    /// One enrollment with this invite and this identity, from `origin`. Returns what the
    /// hub answered and the code a real connector would have shown.
    pub async fn enroll_from(
        &self,
        issued: &IssuedInvite,
        identity: &PublicKey,
        origin: Option<IpAddr>,
    ) -> (EnrollOutcome, VerificationCode) {
        let mut hash = [0u8; 32];
        hash[..8].copy_from_slice(&next().to_be_bytes());
        let hash = HandshakeHash::from_bytes(hash);
        let nonce = enroll::draw_nonce(&hash).unwrap();
        let code = VerificationCode::derive(&nonce, &hash).unwrap();
        let declared = Declared {
            hostname: DeclaredText::parse("build-7").unwrap(),
            os: DeclaredText::parse("linux").unwrap(),
            hermes_version: DeclaredText::parse("1.2.3").unwrap(),
        };
        let outcome = enroll::enroll(
            self.pool(),
            &self.home.mark,
            self.org,
            &self.refusals,
            &issued.invite.digest(),
            &Enrollment {
                identity,
                declared: &declared,
                connector_version: &DeclaredText::parse("0.1.0").unwrap(),
                origin,
                nonce: &nonce,
                handshake_hash: &hash,
            },
            self.now(),
        )
        .await
        .unwrap();
        if let EnrollOutcome::Enrolled(accepted) = &outcome {
            assert_eq!(accepted.reply.nonce, nonce);
        }
        (outcome, code)
    }

    pub async fn enroll(
        &self,
        issued: &IssuedInvite,
        identity: &PublicKey,
    ) -> (EnrollOutcome, VerificationCode) {
        self.enroll_from(issued, identity, address(10)).await
    }

    /// A Hermes with a pending request: line, invite and first enrollment.
    pub async fn pending(&self, name: &HermesName) -> Request {
        let issued = self.invite(name).await;
        let identity = identity();
        let (outcome, code) = self.enroll(&issued, &identity).await;
        let EnrollOutcome::Enrolled(accepted) = outcome else {
            panic!("the first enrollment was refused: {outcome:?}");
        };
        Request {
            device: accepted.device,
            identity,
            code,
        }
    }

    /// Approves as the admin command does: it reads the request, with the last use of the
    /// device it would replace, and the answer goes with that last use.
    pub async fn approve(
        &self,
        name: &HermesName,
        code: VerificationCode,
        recognition: Option<Recognition>,
    ) -> ApproveOutcome {
        let shown = approval::request_of(self.pool(), self.org, name)
            .await
            .unwrap()
            .and_then(|view| view.replaces);
        let recognition = recognition
            .zip(shown)
            .map(|(answer, shown)| Recognized { answer, shown });
        self.approve_with(name, Answer { code, recognition }).await
    }

    /// Approves with exactly this answer.
    pub async fn approve_with(&self, name: &HermesName, answer: Answer) -> ApproveOutcome {
        approval::approve(
            self.pool(),
            &self.home.mark,
            self.org,
            &self.admin,
            name,
            answer,
            self.now(),
        )
        .await
        .unwrap()
    }

    /// A Hermes with an approved request, at generation zero.
    pub async fn approved(&self, name: &HermesName, recognition: Option<Recognition>) -> Request {
        let request = self.pending(name).await;
        let outcome = self.approve(name, request.code, recognition).await;
        assert!(
            matches!(outcome, ApproveOutcome::Approved(_)),
            "not approved: {outcome:?}"
        );
        request
    }

    pub async fn connect(&self, device: Uuid) -> ChannelConnection {
        self.connect_from(device, address(20)).await
    }

    pub async fn connect_from(&self, device: Uuid, from: Option<IpAddr>) -> ChannelConnection {
        connection::open(
            self.pool(),
            self.org,
            device,
            from,
            &DeclaredText::parse("0.1.0").unwrap(),
            1,
            self.now(),
        )
        .await
        .unwrap()
    }

    pub async fn issue(
        &self,
        connection: &ChannelConnection,
        presented: Option<&SealedCredential>,
    ) -> SwapOutcome {
        swap::issue(
            self.pool(),
            &self.home.mark,
            self.org,
            &self.key,
            connection,
            presented.map(SealedCredential::expose),
            self.now(),
        )
        .await
        .unwrap()
    }

    /// T1 that must issue.
    pub async fn issued(
        &self,
        connection: &ChannelConnection,
        presented: Option<&SealedCredential>,
    ) -> Box<Issued> {
        match self.issue(connection, presented).await {
            SwapOutcome::Issued(issued) => issued,
            other => panic!("nothing was issued: {other:?}"),
        }
    }

    /// The "saved" a connector that behaves sends for what it received.
    pub fn saved(issued: &Issued) -> Saved {
        Saved {
            generation: issued.receipt.credential().generation,
            digest: issued.receipt.credential().digest,
        }
    }

    pub async fn confirm(&self, connection: &ChannelConnection, issued: &Issued) -> ConfirmOutcome {
        swap::confirm(
            self.pool(),
            &self.home.mark,
            self.org,
            connection,
            &issued.receipt,
            &Self::saved(issued),
            self.now(),
        )
        .await
        .unwrap()
    }

    /// Takes an approved request through its first swap: T1 with nothing presented and T2.
    pub async fn first_swap(&self, request: &Request) -> Connected {
        let connection = self.connect(request.device).await;
        let issued = self.issued(&connection, None).await;
        let confirmed = self.confirm(&connection, &issued).await;
        assert!(
            matches!(confirmed, ConfirmOutcome::Confirmed { .. }),
            "the first swap was not confirmed: {confirmed:?}"
        );
        let issued = *issued;
        Connected {
            device: request.device,
            identity: request.identity,
            connection,
            credential: issued.receipt.credential(),
            sealed: issued.sealed,
        }
    }

    /// A Hermes with an active device, on its first connection.
    pub async fn active(&self, name: &HermesName) -> Connected {
        let request = self.approved(name, None).await;
        self.first_swap(&request).await
    }

    /// One more full swap of an active device, on a new connection: presents the current
    /// credential, saves and confirms the new one.
    pub async fn swap_again(&self, connected: &Connected) -> Connected {
        let connection = self.connect(connected.device).await;
        let issued = self.issued(&connection, Some(&connected.sealed)).await;
        let confirmed = self.confirm(&connection, &issued).await;
        assert!(matches!(confirmed, ConfirmOutcome::Confirmed { .. }));
        let issued = *issued;
        Connected {
            device: connected.device,
            identity: connected.identity,
            connection,
            credential: issued.receipt.credential(),
            sealed: issued.sealed,
        }
    }

    pub async fn device(&self, id: Uuid) -> Device {
        repo::device::by_id(self.pool(), self.org, id)
            .await
            .unwrap()
            .unwrap()
    }

    /// The actions of the audit of this organization about `target`, in order.
    pub async fn actions_about(&self, target: Uuid) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT action FROM audit WHERE organization_id = $1 AND target_id = $2 ORDER BY seq",
        )
        .bind(self.org.id())
        .bind(target)
        .fetch_all(self.pool())
        .await
        .unwrap()
    }

    /// The kinds of the alerts of this organization about `device`, in order.
    pub async fn alerts_about(&self, device: Uuid) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT kind FROM alert WHERE organization_id = $1 AND device_id = $2 ORDER BY at, id",
        )
        .bind(self.org.id())
        .bind(device)
        .fetch_all(self.pool())
        .await
        .unwrap()
    }

    /// How many rows of the audit this organization has.
    pub async fn audit_rows(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM audit WHERE organization_id = $1")
            .bind(self.org.id())
            .fetch_one(self.pool())
            .await
            .unwrap()
    }
}

/// A code that is not `code`.
pub fn another_code(code: VerificationCode) -> VerificationCode {
    VerificationCode::parse(&format!("{:06}", (code.value() + 1) % 1_000_000)).unwrap()
}
