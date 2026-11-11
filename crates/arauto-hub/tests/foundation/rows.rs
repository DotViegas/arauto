//! The row types against the real schema.

use arauto_hub::core::judge::{self, Decision, Mode, Presented};
use arauto_hub::core::model::{
    Device, DeviceState, Hermes, HermesMode, Invite, InviteState, Slot, StateReason,
};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::ChannelState;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::common::{self, Confirmed, NOW, TestDb, TestHome};

async fn read_device(pool: &PgPool, id: Uuid) -> Result<Device, sqlx::Error> {
    sqlx::query_as("SELECT * FROM device WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
}

#[tokio::test]
async fn a_hermes_row_reads_back() {
    let db = TestDb::shared().await;
    let home = TestHome::new();
    let organization = common::organization(&db.pool, &home).await;
    let id = common::hermes(&db.pool, organization).await;
    let hermes: Hermes = sqlx::query_as("SELECT * FROM hermes WHERE id = $1")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(hermes.id, id);
    assert_eq!(hermes.organization_id, organization.id());
    assert!(hermes.name.as_str().starts_with('h'));
    assert_eq!(hermes.mode, HermesMode::Fast);
    assert!(!hermes.can_be_woken);
    assert_eq!(hermes.wake_reason, None);
    assert_eq!((hermes.created_at, hermes.deleted_at), (NOW, None));
}

#[tokio::test]
async fn a_device_row_reads_back_in_every_state() {
    let db = TestDb::shared().await;
    let home = TestHome::new();
    let organization = common::organization(&db.pool, &home).await;
    let cases = [
        (
            "pending",
            Confirmed::Never,
            DeviceState::Pending,
            Some(Slot::Request),
        ),
        (
            "approved",
            Confirmed::Never,
            DeviceState::Approved,
            Some(Slot::Request),
        ),
        (
            "active",
            Confirmed::Yes,
            DeviceState::Active,
            Some(Slot::InUse),
        ),
        (
            "quarantined",
            Confirmed::Yes,
            DeviceState::Quarantined,
            Some(Slot::InUse),
        ),
        (
            "quarantined",
            Confirmed::Never,
            DeviceState::Quarantined,
            Some(Slot::Request),
        ),
        (
            "suspended",
            Confirmed::Yes,
            DeviceState::Suspended,
            Some(Slot::InUse),
        ),
        ("denied", Confirmed::Never, DeviceState::Denied, None),
        ("expired", Confirmed::Never, DeviceState::Expired, None),
        ("replaced", Confirmed::Yes, DeviceState::Replaced, None),
        ("revoked", Confirmed::Yes, DeviceState::Revoked, None),
    ];
    for (text, confirmed, state, slot) in cases {
        let hermes = common::hermes(&db.pool, organization).await;
        let (id, identity) = common::device(&db.pool, organization, hermes, text, confirmed).await;
        let device = read_device(&db.pool, id).await.unwrap();
        assert_eq!(device.id, id);
        assert_eq!(device.hermes_id, hermes);
        assert_eq!(device.organization_id, organization.id());
        assert_eq!(device.public_key.as_bytes(), &identity);
        assert_eq!((device.state, device.slot), (state, slot), "{text}");
        assert_eq!(device.state_reason, None);
        assert!(!device.marked_by_restoration);
        match confirmed {
            Confirmed::Never => {
                assert_eq!(device.current, None);
                assert_eq!(device.max_generation, 0);
                assert_eq!(device.first_confirmed_at, None);
            }
            Confirmed::Yes => {
                let current = device.current.unwrap();
                assert_eq!(current.generation.get(), 3);
                assert_eq!(current.digest.as_bytes(), &[0x33; 32]);
                assert_eq!(device.max_generation, 3);
                assert_eq!(device.first_confirmed_at, Some(NOW));
            }
        }
        assert_eq!(device.pending, None);
        assert_eq!(
            device.verification_code.map(|code| code.to_string()),
            (text == "pending").then(|| "123-456".to_owned())
        );
        assert_eq!(device.snapshot().state, state);
    }
}

#[tokio::test]
async fn the_swap_rule_reads_the_credentials_of_the_row() {
    let db = TestDb::shared().await;
    let home = TestHome::new();
    let organization = common::organization(&db.pool, &home).await;
    let hermes = common::hermes(&db.pool, organization).await;
    let (id, _) = common::device(&db.pool, organization, hermes, "active", Confirmed::Yes).await;
    sqlx::query(
        "UPDATE device SET pending_generation = 4, pending_digest = $2, max_generation = 6, \
                           declared = $3, connector_version = '0.1.0', proto = 1, \
                           last_contact_at = $4, last_address = '192.0.2.5' \
         WHERE id = $1",
    )
    .bind(id)
    .bind(vec![0x44u8; 32])
    .bind(sqlx::types::Json(serde_json::json!({
        "hostname": "box-7",
        "os": "linux\u{202e}",
        "hermes_version": 12,
        "unexpected": "ignored",
    })))
    .bind(NOW + 9)
    .execute(&db.pool)
    .await
    .unwrap();
    let device = read_device(&db.pool, id).await.unwrap();
    let snapshot = device.snapshot();
    let current = snapshot.current.unwrap();
    let pending = snapshot.pending.unwrap();
    assert_eq!((current.generation.get(), pending.generation.get()), (3, 4));
    assert_eq!(snapshot.max_generation, 6);

    assert_eq!(
        judge::judge(&snapshot, &Presented::Opened(current), Mode::Swap),
        Decision::Accept {
            promote_pending: false
        }
    );
    assert_eq!(
        judge::judge(&snapshot, &Presented::Opened(pending), Mode::Swap),
        Decision::Accept {
            promote_pending: true
        }
    );
    assert_eq!(judge::next_generation(&snapshot).unwrap().get(), 7);

    // What the device declared is not trusted: a character that reorders text is dropped,
    // and a value that is not a text reads as nothing.
    assert_eq!(device.declared.hostname.as_str(), "box-7");
    assert_eq!(device.declared.os.as_str(), "linux");
    assert_eq!(device.declared.hermes_version.as_str(), "");
    assert_eq!(device.connector_version.as_deref(), Some("0.1.0"));
    assert_eq!(device.proto, Some(1));
    assert_eq!(device.last_contact_at, Some(NOW + 9));
    assert_eq!(device.last_address.as_deref(), Some("192.0.2.5"));
}

#[tokio::test]
async fn the_state_and_the_reason_give_the_code() {
    let db = TestDb::shared().await;
    let home = TestHome::new();
    let organization = common::organization(&db.pool, &home).await;
    let cases = [
        (
            "expired",
            Confirmed::Never,
            "pending_timeout",
            ErrorCode::Ar302,
        ),
        (
            "expired",
            Confirmed::Never,
            "approved_timeout",
            ErrorCode::Ar303,
        ),
        ("expired", Confirmed::Yes, "inactivity", ErrorCode::Ar507),
        (
            "denied",
            Confirmed::Never,
            "code_attempts",
            ErrorCode::Ar301,
        ),
        ("quarantined", Confirmed::Yes, "reuse", ErrorCode::Ar501),
        ("revoked", Confirmed::Yes, "admin", ErrorCode::Ar502),
        ("replaced", Confirmed::Yes, "reconnected", ErrorCode::Ar503),
        ("suspended", Confirmed::Yes, "restored", ErrorCode::Ar506),
    ];
    for (state, confirmed, reason, code) in cases {
        let hermes = common::hermes(&db.pool, organization).await;
        let (id, _) = common::device(&db.pool, organization, hermes, state, confirmed).await;
        sqlx::query("UPDATE device SET state_reason = $2 WHERE id = $1")
            .bind(id)
            .bind(reason)
            .execute(&db.pool)
            .await
            .unwrap();
        let device = read_device(&db.pool, id).await.unwrap();
        assert_eq!(device.state_reason, StateReason::parse(reason));
        assert_eq!(device.ended_code(), Some(code), "{state} {reason}");
        assert_eq!(device.channel_state(), ChannelState::Ended(code));
    }
}

#[tokio::test]
async fn a_row_this_program_does_not_understand_is_an_error() {
    let db = TestDb::shared().await;
    let home = TestHome::new();
    let organization = common::organization(&db.pool, &home).await;
    let hermes = common::hermes(&db.pool, organization).await;
    let (id, _) = common::device(&db.pool, organization, hermes, "active", Confirmed::Yes).await;
    sqlx::query("UPDATE device SET state_reason = 'a reason of tomorrow' WHERE id = $1")
        .bind(id)
        .execute(&db.pool)
        .await
        .unwrap();
    let error = read_device(&db.pool, id).await.unwrap_err();
    assert!(
        matches!(&error, sqlx::Error::ColumnDecode { index, .. } if index == "state_reason"),
        "{error}"
    );
}

#[tokio::test]
async fn an_invite_row_reads_back() {
    let db = TestDb::shared().await;
    let home = TestHome::new();
    let organization = common::organization(&db.pool, &home).await;
    let hermes = common::hermes(&db.pool, organization).await;
    let (pending, _) =
        common::device(&db.pool, organization, hermes, "pending", Confirmed::Never).await;
    let read = |id: Uuid| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_as::<_, Invite>("SELECT * FROM invite WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };

    let (used, digest) =
        common::invite(&db.pool, organization, hermes, "used", Some(pending)).await;
    let invite = read(used).await;
    assert_eq!(invite.state, InviteState::Used);
    assert_eq!(invite.digest.unwrap().as_bytes(), &digest);
    assert_eq!(invite.device_id, Some(pending));
    assert_eq!(invite.used_at, Some(NOW));
    assert_eq!((invite.created_at, invite.expires_at), (NOW, NOW + 900_000));
    assert_eq!(invite.hermes_id, hermes);

    // An expired invite has no digest any more.
    let (expired, _) = common::invite(&db.pool, organization, hermes, "expired", None).await;
    let invite = read(expired).await;
    assert_eq!(invite.state, InviteState::Expired);
    assert!(invite.digest.is_none());
    assert_eq!(invite.device_id, None);
}
