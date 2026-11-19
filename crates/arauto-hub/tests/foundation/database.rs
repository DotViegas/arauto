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

async fn column_exists(pool: &PgPool, table: &str, column: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = $1 AND column_name = $2)",
    )
    .bind(table)
    .bind(column)
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

/// Decision 0049: the switch of the Hermes goes away. A Hermes whose switch was off keeps being
/// silent: its "be woken" is turned off on the access and on the line of a chat. A Hermes whose
/// switch was on is untouched. The two columns are gone.
#[tokio::test]
async fn the_wake_switch_is_folded_into_the_verb_and_its_columns_are_dropped() {
    let db = TestDb::empty().await;
    let before = Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|migration| migration.version < 18)
            .cloned()
            .collect::<Vec<_>>(),
    );
    db::migrate_new_database(&db.pool, &before).await.unwrap();
    assert!(column_exists(&db.pool, "hermes", "can_be_woken").await);

    let organization = uuid::Uuid::new_v4();
    let admin = uuid::Uuid::new_v4();
    let engine = uuid::Uuid::new_v4();
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
    sqlx::query(
        "INSERT INTO engine (id, organization_id, kind, address, events_url, events_secret_enc, \
                             config_version, created_at, changed_at) \
         VALUES ($1, $2, 'waha', 'http://192.0.2.10:3000', 'http://192.0.2.11:9000', \
                 decode('00', 'hex'), 1, 1, 1)",
    )
    .bind(engine)
    .bind(organization)
    .execute(&db.pool)
    .await
    .unwrap();
    let number = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO number (id, organization_id, engine_id, session_name, phone_enc, \
                             phone_digest, alias, state, state_since, created_at) \
         VALUES ($1, $2, $3, 'sessionnamesession1', decode('00', 'hex'), \
                 decode(repeat('01', 32), 'hex'), 'desk', 'connected', 1, 1)",
    )
    .bind(number)
    .bind(organization)
    .bind(engine)
    .execute(&db.pool)
    .await
    .unwrap();
    let chat = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO chat (id, organization_id, number_id, kind, created_at) \
         VALUES ($1, $2, $3, 'contact', 1)",
    )
    .bind(chat)
    .bind(organization)
    .bind(number)
    .execute(&db.pool)
    .await
    .unwrap();
    // Two Hermes with the verb on the access and on a line: A has the switch off, B on.
    let mut accesses = Vec::new();
    for (name, switch) in [("ana", false), ("bia", true)] {
        let hermes = uuid::Uuid::new_v4();
        let access = uuid::Uuid::new_v4();
        sqlx::query(
            "INSERT INTO hermes (id, organization_id, name, can_be_woken, created_at) \
             VALUES ($1, $2, $3, $4, 1)",
        )
        .bind(hermes)
        .bind(organization)
        .bind(name)
        .bind(switch)
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO access (id, organization_id, hermes_id, number_id, scope, can_read, \
                                 can_send, can_modify, can_wake, can_start_chats, created_by, \
                                 created_at, changed_at) \
             VALUES ($1, $2, $3, $4, 'all', TRUE, TRUE, FALSE, TRUE, FALSE, $5, 1, 1)",
        )
        .bind(access)
        .bind(organization)
        .bind(hermes)
        .bind(number)
        .bind(admin)
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO access_chat (organization_id, access_id, number_id, chat_id, can_read, \
                                      can_send, can_modify, can_wake, created_at, changed_at) \
             VALUES ($1, $2, $3, $4, TRUE, TRUE, FALSE, TRUE, 1, 1)",
        )
        .bind(organization)
        .bind(access)
        .bind(number)
        .bind(chat)
        .execute(&db.pool)
        .await
        .unwrap();
        accesses.push((name, access));
    }

    db::migrate_existing_database(&db.pool, &MIGRATOR, BackupConfirmed::i_have_a_backup())
        .await
        .unwrap();

    for (name, access) in accesses {
        let on_access: bool = sqlx::query_scalar("SELECT can_wake FROM access WHERE id = $1")
            .bind(access)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        let on_line: bool =
            sqlx::query_scalar("SELECT can_wake FROM access_chat WHERE access_id = $1")
                .bind(access)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        // Only the verbs of the Hermes that was silent are turned off; the read verb stays.
        assert_eq!(
            (on_access, on_line),
            (name == "bia", name == "bia"),
            "{name}"
        );
        let read: bool = sqlx::query_scalar("SELECT can_read FROM access WHERE id = $1")
            .bind(access)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert!(read, "{name}");
    }
    assert!(!column_exists(&db.pool, "hermes", "can_be_woken").await);
    assert!(!column_exists(&db.pool, "hermes", "wake_reason").await);
}

/// The "command" mark of a number goes, with no carry-over: the column is dropped from a database
/// that already has data, and a number that carried the mark stays as it was otherwise.
#[tokio::test]
async fn the_command_mark_of_a_number_is_dropped_without_carry_over() {
    let db = TestDb::empty().await;
    let before = Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|migration| migration.version < 20)
            .cloned()
            .collect::<Vec<_>>(),
    );
    db::migrate_new_database(&db.pool, &before).await.unwrap();
    assert!(column_exists(&db.pool, "number", "command").await);

    let organization = uuid::Uuid::new_v4();
    let engine = uuid::Uuid::new_v4();
    let number = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO organization (id, name, state, key_enc, created_at) \
         VALUES ($1, 'one', 'active', decode('00', 'hex'), 1)",
    )
    .bind(organization)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO engine (id, organization_id, kind, address, events_url, events_secret_enc, \
                             config_version, created_at, changed_at) \
         VALUES ($1, $2, 'waha', 'http://192.0.2.10:3000', 'http://192.0.2.11:9000', \
                 decode('00', 'hex'), 1, 1, 1)",
    )
    .bind(engine)
    .bind(organization)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO number (id, organization_id, engine_id, session_name, phone_enc, \
                             phone_digest, alias, command, state, state_since, created_at) \
         VALUES ($1, $2, $3, 'sessionnamesession1', decode('00', 'hex'), \
                 decode(repeat('01', 32), 'hex'), 'sales', TRUE, 'unpaired', 1, 1)",
    )
    .bind(number)
    .bind(organization)
    .bind(engine)
    .execute(&db.pool)
    .await
    .unwrap();

    db::migrate_existing_database(&db.pool, &MIGRATOR, BackupConfirmed::i_have_a_backup())
        .await
        .unwrap();

    assert!(!column_exists(&db.pool, "number", "command").await);
    let alias: String = sqlx::query_scalar("SELECT alias FROM number WHERE id = $1")
        .bind(number)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(alias, "sales");
}

/// Who gives a Hermes orders on WhatsApp (migration 0021): the tables come to a database that
/// has data, empty, and the schema itself refuses a record that is not one.
#[tokio::test]
async fn the_record_of_who_gives_a_hermes_orders_comes_empty_and_the_schema_keeps_its_rules() {
    let db = TestDb::empty().await;
    let before = Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|migration| migration.version < 21)
            .cloned()
            .collect::<Vec<_>>(),
    );
    db::migrate_new_database(&db.pool, &before).await.unwrap();
    assert!(!table_exists(&db.pool, "hermes_whatsapp_phone").await);
    db::migrate_existing_database(&db.pool, &MIGRATOR, BackupConfirmed::i_have_a_backup())
        .await
        .unwrap();
    for table in ["hermes_whatsapp_phone", "hermes_whatsapp_phone_digest"] {
        assert!(table_exists(&db.pool, table).await, "{table}");
        let rows: i64 = sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(rows, 0, "{table} starts empty");
    }

    let home = TestHome::new();
    let organization = common::organization(&db.pool, &home).await;
    let first = common::hermes(&db.pool, organization).await;
    let second = common::hermes(&db.pool, organization).await;
    let insert = |hermes: uuid::Uuid, role: &'static str, mode: Option<&'static str>| {
        let pool = db.pool.clone();
        async move {
            let id = uuid::Uuid::new_v4();
            sqlx::query(
                "INSERT INTO hermes_whatsapp_phone \
                     (id, organization_id, hermes_id, role, whatsapp_mode, phone_enc, created_at) \
                 VALUES ($1, $2, $3, $4, $5, decode('00', 'hex'), 1)",
            )
            .bind(id)
            .bind(organization.id())
            .bind(hermes)
            .bind(role)
            .bind(mode)
            .execute(&pool)
            .await
            .map(|_| id)
        }
    };
    let digest = |phone: uuid::Uuid, role: &'static str, byte: &'static str| {
        let pool = db.pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO hermes_whatsapp_phone_digest (organization_id, phone_id, role, digest) \
                 VALUES ($1, $2, $3, decode(repeat($4, 32), 'hex'))",
            )
            .bind(organization.id())
            .bind(phone)
            .bind(role)
            .bind(byte)
            .execute(&pool)
            .await
        }
    };
    // The mode belongs to the bridge's row, and only to it.
    assert!(insert(first, "takes_orders", None).await.is_err());
    assert!(insert(first, "gives_orders", Some("bot")).await.is_err());
    assert!(insert(first, "gives_orders", None).await.is_ok());
    assert!(insert(first, "takes_orders", Some("loud")).await.is_err());
    let takes = insert(first, "takes_orders", Some("self_chat"))
        .await
        .unwrap();
    // One bridge per Hermes.
    assert!(insert(first, "takes_orders", Some("bot")).await.is_err());
    // One Hermes per bridge telephone: the same digest in two Hermes is refused, for the
    // bridge only.
    digest(takes, "takes_orders", "0a").await.unwrap();
    let other = insert(second, "takes_orders", Some("bot")).await.unwrap();
    assert!(digest(other, "takes_orders", "0a").await.is_err());
    let giver = insert(second, "gives_orders", None).await.unwrap();
    assert!(digest(giver, "gives_orders", "0a").await.is_ok());
    // A digest is 32 bytes.
    let short = sqlx::query(
        "INSERT INTO hermes_whatsapp_phone_digest (organization_id, phone_id, role, digest) \
         VALUES ($1, $2, 'gives_orders', decode('01', 'hex'))",
    )
    .bind(organization.id())
    .bind(giver)
    .execute(&db.pool)
    .await;
    assert!(short.is_err());
    // The digests go with their telephone.
    sqlx::query("DELETE FROM hermes_whatsapp_phone WHERE id = $1")
        .bind(takes)
        .execute(&db.pool)
        .await
        .unwrap();
    let left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM hermes_whatsapp_phone_digest WHERE phone_id = $1")
            .bind(takes)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(left, 0);
}

/// Orders from outside WhatsApp (migration 0023): the table comes to a database that has data,
/// empty, with one row per Hermes and only for a Hermes that exists.
#[tokio::test]
async fn the_answer_of_orders_from_outside_whatsapp_comes_empty_and_keeps_one_row_per_hermes() {
    let db = TestDb::empty().await;
    let before = Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|migration| migration.version < 23)
            .cloned()
            .collect::<Vec<_>>(),
    );
    db::migrate_new_database(&db.pool, &before).await.unwrap();
    assert!(!table_exists(&db.pool, "hermes_orders_outside_whatsapp").await);
    db::migrate_existing_database(&db.pool, &MIGRATOR, BackupConfirmed::i_have_a_backup())
        .await
        .unwrap();
    assert!(table_exists(&db.pool, "hermes_orders_outside_whatsapp").await);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM hermes_orders_outside_whatsapp")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(rows, 0, "the table starts empty");

    let home = TestHome::new();
    let organization = common::organization(&db.pool, &home).await;
    let hermes = common::hermes(&db.pool, organization).await;
    let insert = |hermes: uuid::Uuid| {
        let pool = db.pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO hermes_orders_outside_whatsapp \
                     (organization_id, hermes_id, created_at) VALUES ($1, $2, 1)",
            )
            .bind(organization.id())
            .bind(hermes)
            .execute(&pool)
            .await
        }
    };
    assert!(insert(hermes).await.is_ok());
    assert!(insert(hermes).await.is_err(), "one row per Hermes");
    assert!(
        insert(uuid::Uuid::new_v4()).await.is_err(),
        "only for a Hermes that exists"
    );
}

/// The reason of an invite and the program that enrolled a device (migration 0024): the
/// columns come empty to a database that has data, and keep their format.
#[tokio::test]
async fn the_reason_of_an_invite_comes_empty_and_keeps_its_format() {
    let db = TestDb::empty().await;
    let before = Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|migration| migration.version < 24)
            .cloned()
            .collect::<Vec<_>>(),
    );
    db::migrate_new_database(&db.pool, &before).await.unwrap();
    assert!(!column_exists(&db.pool, "invite", "reason").await);
    assert!(!column_exists(&db.pool, "device", "enrolled_connector").await);
    let home = TestHome::new();
    let organization = common::organization(&db.pool, &home).await;
    let hermes = common::hermes(&db.pool, organization).await;
    let (device, _) =
        common::device(&db.pool, organization, hermes, "active", Confirmed::Yes).await;
    let (invite, _) = common::invite(&db.pool, organization, hermes, "used", Some(device)).await;
    db::migrate_existing_database(&db.pool, &MIGRATOR, BackupConfirmed::i_have_a_backup())
        .await
        .unwrap();
    for column in ["reason", "version_clause", "version_from", "version_to"] {
        assert!(column_exists(&db.pool, "invite", column).await, "{column}");
        let filled: i64 = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT count(*) FROM invite WHERE {column} IS NOT NULL"
        )))
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(filled, 0, "{column} starts empty");
    }
    let (connector, proto): (Option<String>, Option<i32>) =
        sqlx::query_as("SELECT enrolled_connector, enrolled_proto FROM device WHERE id = $1")
            .bind(device)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!((connector, proto), (None, None));

    // The format is kept by the database.
    let set = |sql: &'static str, text: Option<&'static str>| {
        let pool = db.pool.clone();
        async move {
            sqlx::query(sql)
                .bind(invite)
                .bind(text)
                .execute(&pool)
                .await
        }
    };
    assert!(
        set(
            "UPDATE invite SET reason = $2 WHERE id = $1",
            Some("request_approved")
        )
        .await
        .is_ok()
    );
    assert!(
        set("UPDATE invite SET reason = $2 WHERE id = $1", Some("pick"))
            .await
            .is_err(),
        "an unknown reason"
    );
    assert!(
        set(
            "UPDATE invite SET version_clause = $2 WHERE id = $1",
            Some("newer")
        )
        .await
        .is_err(),
        "an unknown clause"
    );
    assert!(
        set(
            "UPDATE invite SET version_from = $2, version_to = $2, version_clause = 'numbers' \
             WHERE id = $1",
            Some("1.2")
        )
        .await
        .is_err(),
        "a version is three numbers"
    );
    assert!(
        set(
            "UPDATE invite SET version_from = $2, version_to = $2 WHERE id = $1",
            Some("1.2.3")
        )
        .await
        .is_err(),
        "versions only come with the clause numbers"
    );
    assert!(
        set(
            "UPDATE invite SET version_clause = 'numbers' WHERE id = $1",
            None
        )
        .await
        .is_err(),
        "the clause numbers comes with both versions"
    );
    assert!(
        set(
            "UPDATE invite SET version_from = $2, version_to = '1.2.4', \
             version_clause = 'numbers' WHERE id = $1",
            Some("1.2.3")
        )
        .await
        .is_ok()
    );
    let long = "x".repeat(129);
    assert!(
        sqlx::query("UPDATE device SET enrolled_connector = $2 WHERE id = $1")
            .bind(device)
            .bind(&long)
            .execute(&db.pool)
            .await
            .is_err(),
        "the declared text is at most 128 characters"
    );
    assert!(
        sqlx::query("UPDATE device SET enrolled_proto = 0 WHERE id = $1")
            .bind(device)
            .execute(&db.pool)
            .await
            .is_err()
    );
}
