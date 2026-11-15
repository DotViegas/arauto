//! The pool, the durability check, the migrations and the resolved organization.

use std::time::{Duration, Instant};

use arauto_hub::db::{
    self, BackupConfirmed, DbError, MIGRATOR, PoolOptions, SchemaState, SystemTask,
};
use arauto_protocol::invite::InviteDigest;
use arauto_protocol::noise::PublicKey;
use sqlx::migrate::{Migration, MigrationType, Migrator};
use sqlx::postgres::PgPool;
use sqlx::{AssertSqlSafe, SqlSafeStr};

use crate::common::{self, Confirmed, TestDb, TestHome};

async fn show(connection: &mut sqlx::PgConnection, setting: &'static str) -> String {
    sqlx::query_scalar(AssertSqlSafe(format!("SHOW {setting}")))
        .fetch_one(connection)
        .await
        .unwrap()
}

async fn table_exists(pool: &PgPool, table: &str) -> bool {
    sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(table)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------- the pool

#[tokio::test]
async fn every_connection_has_the_two_timeouts() {
    let db = TestDb::shared().await;
    // Three at once: three different connections of the pool.
    let mut connections = Vec::new();
    for _ in 0..3 {
        connections.push(db.pool.acquire().await.unwrap());
    }
    for connection in &mut connections {
        assert_eq!(show(connection, "lock_timeout").await, "5s");
        assert_eq!(
            show(connection, "idle_in_transaction_session_timeout").await,
            "30s"
        );
    }
}

#[tokio::test]
async fn the_pool_does_not_grow_and_the_wait_for_a_connection_ends() {
    let db = TestDb::shared().await;
    // The same wait holds for opening each connection, so it is long enough for a busy
    // machine to open the first two: what is measured is only the third, which never comes.
    let patience = Duration::from_secs(2);
    let options = PoolOptions {
        size: 2,
        acquire_timeout: patience,
    };
    let pool = db::connect(&db.url(), options).await.unwrap();
    let _first = pool.acquire().await.unwrap();
    let _second = pool.acquire().await.unwrap();
    let started = Instant::now();
    let third = pool.acquire().await;
    assert!(matches!(third, Err(sqlx::Error::PoolTimedOut)));
    // It waited for its whole time, and then it ended: there is no bound from above that a
    // busy machine could not break, and none is needed to know that the wait is not forever.
    assert!(started.elapsed() >= patience - Duration::from_millis(100));
    assert_eq!(pool.size(), 2);
}

#[tokio::test]
async fn an_address_that_does_not_answer_is_an_error_without_the_address() {
    let url = arauto_hub::config::DatabaseUrl::new(
        "postgres://nobody:s3cr3t-word@127.0.0.1:1/nothing".to_owned(),
    );
    let options = PoolOptions {
        size: 1,
        acquire_timeout: Duration::from_millis(500),
    };
    let error = db::connect(&url, options).await.unwrap_err();
    assert!(matches!(error, DbError::Connect(_)));
    let shown = format!("{error} {error:?}");
    assert!(!shown.contains("s3cr3t-word"), "{shown}");
}

// ---------------------------------------------------------------------------- durability

#[tokio::test]
async fn the_settings_are_read_from_the_server() {
    let db = TestDb::shared().await;
    let found = db::read_durability(&db.pool).await.unwrap();
    // The test server is a real one: it forces its writes.
    assert!(found.fsync && found.full_page_writes, "{found:?}");
    db::check_durability(&db.pool, false).await.unwrap();
}

#[tokio::test]
async fn a_server_with_fsync_off_is_refused_unless_the_test_option_is_given() {
    let db = TestDb::shared().await;
    // The reading of the real server, with one setting turned off on the way.
    let fsync_off = || async {
        let mut found = db::read_durability(&db.pool).await?;
        found.fsync = false;
        Ok(found)
    };
    let refused = db::check_durability_with(fsync_off(), false).await;
    assert!(matches!(
        refused,
        Err(DbError::UnsafeDurability {
            fsync: false,
            full_page_writes: true
        })
    ));
    db::check_durability_with(fsync_off(), true).await.unwrap();
}

#[tokio::test]
async fn the_durable_transaction_waits_for_the_disk_whatever_the_session_says() {
    let db = TestDb::shared().await;
    let options = PoolOptions {
        size: 1,
        ..PoolOptions::default()
    };
    let pool = db::connect(&db.url(), options).await.unwrap();
    {
        let mut connection = pool.acquire().await.unwrap();
        sqlx::query("SET synchronous_commit = off")
            .execute(&mut *connection)
            .await
            .unwrap();
        assert_eq!(show(&mut connection, "synchronous_commit").await, "off");
    }
    // The pool has one connection: the transaction runs on the one that was just changed.
    let mut transaction = db::begin_durable(&pool).await.unwrap();
    assert_eq!(show(&mut transaction, "synchronous_commit").await, "on");
    transaction.commit().await.unwrap();
    // Only for that transaction.
    let mut connection = pool.acquire().await.unwrap();
    assert_eq!(show(&mut connection, "synchronous_commit").await, "off");
}

// ---------------------------------------------------------------------------- identifiers

/// Rule 3 of the database: identifiers are drawn by the hub, and the only sequence is the one
/// of the audit.
#[tokio::test]
async fn the_only_sequence_of_the_schema_is_the_one_of_the_audit() {
    let db = TestDb::shared().await;
    let sequences: Vec<String> = sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_catalog.pg_class c \
         JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = current_schema() AND c.relkind = 'S' \
         ORDER BY 1",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(sequences, ["audit_seq"]);
    let identifiers: Vec<(String, String)> = sqlx::query_as(
        "SELECT table_name::text, data_type::text FROM information_schema.columns \
         WHERE table_schema = current_schema() AND column_name = 'id' \
         ORDER BY 1",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert!(identifiers.len() >= 10, "{identifiers:?}");
    for (table, kind) in &identifiers {
        assert_eq!(kind, "uuid", "{table}.id");
    }
}

// ---------------------------------------------------------------------------- migrations

/// The migrations of the program and one more, as a newer program would have.
fn newer_program() -> Migrator {
    let mut migrations: Vec<Migration> = MIGRATOR.iter().cloned().collect();
    migrations.push(Migration::new(
        9_999,
        "later".into(),
        MigrationType::Simple,
        "CREATE TABLE added_later (id BIGINT PRIMARY KEY)".into_sql_str(),
        false,
    ));
    Migrator::with_migrations(migrations)
}

#[tokio::test]
async fn an_empty_database_gets_every_migration() {
    let db = TestDb::empty().await;
    assert_eq!(
        db::schema_state(&db.pool, &MIGRATOR).await.unwrap(),
        SchemaState::Empty
    );
    assert!(matches!(
        db::require_current_schema(&db.pool, &MIGRATOR).await,
        Err(DbError::SchemaEmpty)
    ));
    assert!(!table_exists(&db.pool, "device").await);

    db::migrate_new_database(&db.pool, &MIGRATOR).await.unwrap();
    assert_eq!(
        db::schema_state(&db.pool, &MIGRATOR).await.unwrap(),
        SchemaState::Current
    );
    db::require_current_schema(&db.pool, &MIGRATOR)
        .await
        .unwrap();
    for table in [
        "installation",
        "organization",
        "hermes",
        "device",
        "invite",
        "audit",
    ] {
        assert!(table_exists(&db.pool, table).await, "{table}");
    }
    // Running `init` again changes nothing.
    db::migrate_new_database(&db.pool, &MIGRATOR).await.unwrap();
    assert_eq!(
        db::schema_state(&db.pool, &MIGRATOR).await.unwrap(),
        SchemaState::Current
    );
}

#[tokio::test]
async fn a_database_with_data_is_only_migrated_with_the_word_about_the_backup() {
    let db = TestDb::own().await;
    let program = newer_program();
    assert_eq!(
        db::schema_state(&db.pool, &program).await.unwrap(),
        SchemaState::Pending {
            pending: vec![9_999]
        }
    );

    // Neither `init` nor `serve` nor `admin` applies it.
    let refused = db::migrate_new_database(&db.pool, &program).await;
    let Err(DbError::MigrationsPending { first, pending }) = refused else {
        panic!("a pending migration was applied without the confirmation: {refused:?}");
    };
    assert_eq!((first, pending), (9_999, vec![9_999]));
    assert!(matches!(
        db::require_current_schema(&db.pool, &program).await,
        Err(DbError::MigrationsPending { .. })
    ));
    assert!(!table_exists(&db.pool, "added_later").await);

    // `migrate --i-have-a-backup` does.
    let applied =
        db::migrate_existing_database(&db.pool, &program, BackupConfirmed::i_have_a_backup())
            .await
            .unwrap();
    assert_eq!(applied, [9_999]);
    assert!(table_exists(&db.pool, "added_later").await);
    db::require_current_schema(&db.pool, &program)
        .await
        .unwrap();
    // And once more is nothing.
    let applied =
        db::migrate_existing_database(&db.pool, &program, BackupConfirmed::i_have_a_backup())
            .await
            .unwrap();
    assert!(applied.is_empty());
}

#[tokio::test]
async fn a_database_newer_than_the_program_is_refused_by_every_command() {
    let db = TestDb::own().await;
    db::migrate_existing_database(
        &db.pool,
        &newer_program(),
        BackupConfirmed::i_have_a_backup(),
    )
    .await
    .unwrap();

    // The program of today meets the database of tomorrow.
    assert_eq!(
        db::schema_state(&db.pool, &MIGRATOR).await.unwrap(),
        SchemaState::Newer { newest: 9_999 }
    );
    assert!(matches!(
        db::require_current_schema(&db.pool, &MIGRATOR).await,
        Err(DbError::SchemaNewer { newest: 9_999 })
    ));
    assert!(matches!(
        db::migrate_new_database(&db.pool, &MIGRATOR).await,
        Err(DbError::SchemaNewer { .. })
    ));
    assert!(matches!(
        db::migrate_existing_database(&db.pool, &MIGRATOR, BackupConfirmed::i_have_a_backup())
            .await,
        Err(DbError::SchemaNewer { .. })
    ));
}

#[tokio::test]
async fn a_database_of_something_else_is_left_alone() {
    let db = TestDb::empty().await;
    sqlx::query("CREATE TABLE somebody_elses (id BIGINT PRIMARY KEY)")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        db::schema_state(&db.pool, &MIGRATOR).await.unwrap(),
        SchemaState::Foreign
    );
    assert!(matches!(
        db::migrate_new_database(&db.pool, &MIGRATOR).await,
        Err(DbError::SchemaForeign)
    ));
    assert!(matches!(
        db::migrate_existing_database(&db.pool, &MIGRATOR, BackupConfirmed::i_have_a_backup())
            .await,
        Err(DbError::SchemaForeign)
    ));
    assert!(!table_exists(&db.pool, "device").await);
    assert!(!table_exists(&db.pool, "_sqlx_migrations").await);
}

#[tokio::test]
async fn a_changed_or_unfinished_migration_is_refused() {
    let db = TestDb::own().await;
    let mut changed: Vec<Migration> = MIGRATOR.iter().cloned().collect();
    let first = changed.remove(0);
    changed.insert(
        0,
        Migration::new(
            first.version,
            first.description.clone(),
            MigrationType::Simple,
            "CREATE TABLE not_what_was_applied (id BIGINT)".into_sql_str(),
            false,
        ),
    );
    let other_build = Migrator::with_migrations(changed);
    assert_eq!(
        db::schema_state(&db.pool, &other_build).await.unwrap(),
        SchemaState::Changed {
            version: first.version
        }
    );
    assert!(matches!(
        db::migrate_existing_database(&db.pool, &other_build, BackupConfirmed::i_have_a_backup())
            .await,
        Err(DbError::MigrationChanged { .. })
    ));
    assert!(!table_exists(&db.pool, "not_what_was_applied").await);

    sqlx::query("UPDATE _sqlx_migrations SET success = FALSE")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        db::schema_state(&db.pool, &MIGRATOR).await.unwrap(),
        SchemaState::Failed {
            version: first.version
        }
    );
    assert!(matches!(
        db::require_current_schema(&db.pool, &MIGRATOR).await,
        Err(DbError::MigrationFailed { .. })
    ));
}

#[tokio::test]
async fn no_column_of_the_schema_has_a_default_that_reads_the_clock() {
    let db = TestDb::shared().await;
    let defaults: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT table_name::text, column_name::text, column_default::text \
         FROM information_schema.columns \
         WHERE table_schema = current_schema() AND column_default IS NOT NULL \
           AND table_name <> '_sqlx_migrations'",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert!(!defaults.is_empty());
    for (table, column, default) in defaults {
        let lowered = default.to_ascii_lowercase();
        for clock in ["now()", "current_", "clock_timestamp", "localtime"] {
            assert!(
                !lowered.contains(clock),
                "{table}.{column} defaults to {default}"
            );
        }
    }
    let time_types: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name <> '_sqlx_migrations' \
           AND (data_type LIKE 'timestamp%' OR data_type LIKE 'time %' \
                OR data_type IN ('date', 'interval'))",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(time_types, 0);
}

// ---------------------------------------------------------------------------- the organization

#[tokio::test]
async fn an_organization_is_found_by_the_identity_of_a_device_and_by_the_digest_of_an_invite() {
    let db = TestDb::shared().await;
    let home = TestHome::new();
    let ours = common::organization(&db.pool, &home).await;
    let theirs = common::organization(&db.pool, &home).await;
    assert_ne!(ours.id(), theirs.id());

    let our_hermes = common::hermes(&db.pool, ours).await;
    let their_hermes = common::hermes(&db.pool, theirs).await;
    let (_, our_identity) =
        common::device(&db.pool, ours, our_hermes, "active", Confirmed::Yes).await;
    let (_, their_identity) =
        common::device(&db.pool, theirs, their_hermes, "revoked", Confirmed::Yes).await;
    let (_, our_digest) = common::invite(&db.pool, ours, our_hermes, "issued", None).await;

    let by_device = |identity: [u8; 32]| {
        let pool = db.pool.clone();
        async move {
            db::organization_of_device(&pool, &PublicKey::from_bytes(identity))
                .await
                .unwrap()
        }
    };
    assert_eq!(by_device(our_identity).await, Some(ours));
    // In any state: a closed device is still recognized.
    assert_eq!(by_device(their_identity).await, Some(theirs));
    assert_eq!(by_device([0xee; 32]).await, None);

    let by_invite = |digest: [u8; 32]| {
        let pool = db.pool.clone();
        async move {
            db::organization_of_invite(&pool, &InviteDigest::from_bytes(digest))
                .await
                .unwrap()
        }
    };
    assert_eq!(by_invite(our_digest).await, Some(ours));
    assert_eq!(by_invite([0xee; 32]).await, None);
    // The identity of a device is not the digest of an invite.
    assert_eq!(by_invite(our_identity).await, None);

    let all = db::organizations_for_system_task(&db.pool, SystemTask::Deadlines)
        .await
        .unwrap();
    assert!(all.contains(&ours) && all.contains(&theirs));
}

#[tokio::test]
async fn the_key_of_an_organization_comes_back_from_the_database() {
    let db = TestDb::shared().await;
    let home = TestHome::new();
    let organization = common::organization(&db.pool, &home).await;
    let key_enc: Vec<u8> = sqlx::query_scalar("SELECT key_enc FROM organization WHERE id = $1")
        .bind(organization.id())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let key = home
        .vault
        .open_organization_key(organization.id(), &key_enc)
        .unwrap();
    assert_eq!(key.organization_id(), organization.id());
    // Another installation, with another master key, cannot open it.
    let elsewhere = TestHome::new();
    assert!(
        elsewhere
            .vault
            .open_organization_key(organization.id(), &key_enc)
            .is_err()
    );
}

/// R15 and R1b, on a database that already has data: the codes that exist become command codes
/// with the deadline they have (the ones that are dead stay dead), a cancelled one says when it
/// was cancelled, and an organization that has a passkey, removed or not, has done its first
/// access.
#[tokio::test]
async fn the_codes_and_the_passkeys_that_exist_are_carried_to_the_first_access_migration() {
    let db = TestDb::empty().await;
    let before = Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|migration| migration.version < 15)
            .cloned()
            .collect::<Vec<_>>(),
    );
    db::migrate_new_database(&db.pool, &before).await.unwrap();

    let with_a_passkey = uuid::Uuid::new_v4();
    let without = uuid::Uuid::new_v4();
    let admin = uuid::Uuid::new_v4();
    for (id, name) in [(with_a_passkey, "one"), (without, "two")] {
        sqlx::query(
            "INSERT INTO organization (id, name, state, key_enc, created_at) \
             VALUES ($1, $2, 'active', decode('00', 'hex'), 1)",
        )
        .bind(id)
        .bind(name)
        .execute(&db.pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO admin (id, organization_id, name, kind, created_at) \
         VALUES ($1, $2, 'a', 'local', 1)",
    )
    .bind(admin)
    .bind(with_a_passkey)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO panel_passkey (id, organization_id, admin_id, credential_id, user_handle, \
                                    static_state, dynamic_state, state, created_at, removed_at) \
         VALUES (gen_random_uuid(), $1, $2, decode('01', 'hex'), decode('02', 'hex'), \
                 decode('03', 'hex'), decode('04', 'hex'), 'removed', 777, 800)",
    )
    .bind(with_a_passkey)
    .bind(admin)
    .execute(&db.pool)
    .await
    .unwrap();
    let digest = vec![7u8; 32];
    for (state, expires_at, digest, used_at) in [
        ("used", 1_000_i64, None, Some(900_i64)),
        ("cancelled", 2_000, None, None),
        ("issued", 3_000, Some(digest), None),
    ] {
        sqlx::query(
            "INSERT INTO panel_entry (id, organization_id, digest, state, created_by, \
                                      created_at, expires_at, used_at) \
             VALUES (gen_random_uuid(), $1, $2, $3, $4, 1, $5, $6)",
        )
        .bind(with_a_passkey)
        .bind(digest)
        .bind(state)
        .bind(admin)
        .bind(expires_at)
        .bind(used_at)
        .execute(&db.pool)
        .await
        .unwrap();
    }

    db::migrate_existing_database(&db.pool, &MIGRATOR, BackupConfirmed::i_have_a_backup())
        .await
        .unwrap();

    type Entry = (String, String, Option<i64>, Option<i64>, Option<String>);
    let entries: Vec<Entry> = sqlx::query_as(
        "SELECT state, kind, expires_at, cancelled_at, cancelled_by FROM panel_entry \
         ORDER BY expires_at",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    let command = |state: &str, expires_at: i64, cancelled: Option<i64>| {
        (
            state.to_owned(),
            "command".to_owned(),
            Some(expires_at),
            cancelled,
            cancelled.map(|_| "command".to_owned()),
        )
    };
    assert_eq!(
        entries,
        [
            command("used", 1_000, None),
            command("cancelled", 2_000, Some(2_000)),
            command("issued", 3_000, None),
        ]
    );
    let marks: Vec<(uuid::Uuid, Option<i64>)> =
        sqlx::query_as("SELECT id, panel_first_passkey_at FROM organization")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    for (id, mark) in marks {
        assert_eq!(mark, (id == with_a_passkey).then_some(777), "{id}");
    }
}

/// The two tables of the org chart arrive in a database that has data: they are empty, every
/// live Hermes is "not placed yet", the links are as they were, and the version of the chart is
/// stable.
#[tokio::test]
async fn the_chart_tables_come_to_a_database_with_data() {
    use arauto_hub::repo::org;

    let db = TestDb::empty().await;
    let before = Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|migration| migration.version < 17)
            .cloned()
            .collect::<Vec<_>>(),
    );
    db::migrate_new_database(&db.pool, &before).await.unwrap();
    assert!(!table_exists(&db.pool, "org_level").await);

    let organization = uuid::Uuid::new_v4();
    let admin = uuid::Uuid::new_v4();
    let (one, two) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    sqlx::query(
        "INSERT INTO organization (id, name, state, key_enc, created_at) \
         VALUES ($1, 'one', 'active', decode('00', 'hex'), 1)",
    )
    .bind(organization)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO admin (id, organization_id, name, kind, created_at) \
         VALUES ($1, $2, 'a', 'local', 1)",
    )
    .bind(admin)
    .bind(organization)
    .execute(&db.pool)
    .await
    .unwrap();
    for (id, name) in [(one, "ana"), (two, "bia")] {
        sqlx::query(
            "INSERT INTO hermes (id, organization_id, name, created_at) VALUES ($1, $2, $3, 1)",
        )
        .bind(id)
        .bind(organization)
        .bind(name)
        .execute(&db.pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO peer_link (organization_id, from_hermes_id, to_hermes_id, wake, created_by, \
                                created_at, changed_at) VALUES ($1, $2, $3, TRUE, $4, 1, 1)",
    )
    .bind(organization)
    .bind(one)
    .bind(two)
    .bind(admin)
    .execute(&db.pool)
    .await
    .unwrap();

    db::migrate_existing_database(&db.pool, &MIGRATOR, BackupConfirmed::i_have_a_backup())
        .await
        .unwrap();

    for table in ["org_level", "org_member"] {
        assert!(table_exists(&db.pool, table).await, "{table}");
        let rows: i64 = sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(rows, 0, "{table}");
    }
    let resolved = db::organizations_for_system_task(&db.pool, SystemTask::Deadlines)
        .await
        .unwrap()
        .into_iter()
        .find(|found| found.id() == organization)
        .unwrap();
    let chart = org::chart(&db.pool, resolved).await.unwrap();
    assert!(chart.levels.is_empty());
    // The members come in the order of their ids, which are drawn: compare without the order.
    let mut names: Vec<&str> = chart.members.iter().map(|m| m.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["ana", "bia"]);
    assert!(chart.members.iter().all(|member| member.place.is_none()));
    assert_eq!(chart.links.len(), 1);
    assert!(chart.links[0].wake);
    assert_eq!(
        org::chart(&db.pool, resolved).await.unwrap().version(),
        chart.version()
    );
}

/// The database refuses what the chart does not allow, whatever the program wrote.
#[tokio::test]
async fn the_database_refuses_what_the_chart_does_not_allow() {
    let db = TestDb::shared().await;
    let home = TestHome::new();
    let ours = common::organization(&db.pool, &home).await;
    let theirs = common::organization(&db.pool, &home).await;
    let level = |organization: uuid::Uuid, position: i32, name: &'static str| {
        let pool = db.pool.clone();
        async move {
            let id = uuid::Uuid::new_v4();
            sqlx::query(
                "INSERT INTO org_level (id, organization_id, position, name, created_at, changed_at) \
                 VALUES ($1, $2, $3, $4, 1, 1)",
            )
            .bind(id)
            .bind(organization)
            .bind(position)
            .bind(name)
            .execute(&pool)
            .await
            .map(|_| id)
        }
    };
    // A reserved name, a position out of range, a name that breaks the format.
    for (position, name) in [
        (0, "system"),
        (0, "hermes"),
        (12, "board"),
        (0, "Board"),
        (-1, "x"),
    ] {
        assert!(
            level(ours.id(), position, name).await.is_err(),
            "{position} {name}"
        );
    }
    let board = level(ours.id(), 0, "board").await.unwrap();
    // The same name or the same position twice in one organization: refused at the commit, here
    // at once because each statement is its own transaction.
    assert!(level(ours.id(), 1, "board").await.is_err());
    assert!(level(ours.id(), 0, "floor").await.is_err());
    // Another organization may have the same name and the same position.
    let other = level(theirs.id(), 0, "board").await.unwrap();

    let hermes = common::hermes(&db.pool, ours).await;
    let member = |hermes: uuid::Uuid,
                  organization: uuid::Uuid,
                  level: uuid::Uuid,
                  x: i32,
                  role: Option<&'static str>| {
        let pool = db.pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO org_member (hermes_id, organization_id, level_id, map_x, role, changed_at) \
                 VALUES ($1, $2, $3, $4, $5, 1)",
            )
            .bind(hermes)
            .bind(organization)
            .bind(level)
            .bind(x)
            .bind(role)
            .execute(&pool)
            .await
        }
    };
    // A level of another organization, a place out of range, a role with a control character, a
    // direction character or a separator, or too long.
    assert!(member(hermes, ours.id(), other, 10, None).await.is_err());
    assert!(member(hermes, ours.id(), board, 4001, None).await.is_err());
    assert!(member(hermes, ours.id(), board, -1, None).await.is_err());
    for role in [
        "a\u{7}b",
        "a\nb",
        "a\u{202e}b",
        "a\u{2028}b",
        "a\u{e0041}b",
        "",
    ] {
        assert!(
            member(hermes, ours.id(), board, 10, Some(role))
                .await
                .is_err(),
            "{role:?}"
        );
    }
    let long = "x".repeat(281);
    let long: &'static str = Box::leak(long.into_boxed_str());
    assert!(
        member(hermes, ours.id(), board, 10, Some(long))
            .await
            .is_err()
    );
    // A Hermes of another organization.
    let strange = common::hermes(&db.pool, theirs).await;
    assert!(member(strange, ours.id(), board, 10, None).await.is_err());
    // What is allowed goes in, and only once for one Hermes.
    assert!(
        member(hermes, ours.id(), board, 10, Some("keeps the books"))
            .await
            .is_ok()
    );
    assert!(member(hermes, ours.id(), board, 20, None).await.is_err());
}
