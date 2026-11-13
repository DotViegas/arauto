//! A PostgreSQL role of its own, for a test that replaces the password of the role the hub
//! connects as: the role of the test server is shared by every test and is never touched.
//!
//! The test server trusts the connections of this machine, so a wrong password is not refused
//! there. What a password opens is checked against the verifier PostgreSQL stores for the
//! role, SCRAM-SHA-256 (RFC 5802 and 7677), recomputed here from the password.

use arauto_hub::config::DatabaseUrl;
use data_encoding::BASE64;
use hkdf::hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use sqlx::AssertSqlSafe;
use sqlx::postgres::PgPool;

/// A role with `LOGIN`, created by a test.
pub struct Role {
    /// Its name: `arauto_rot_` and 32 random hexadecimal digits.
    pub name: String,
}

/// Creates a role that logs in with `password`, with every right on the tables and the
/// sequences of the database `admin` is connected to: what the hub needs there.
pub async fn create(admin: &PgPool, password: &str) -> Role {
    let name = format!("arauto_rot_{}", uuid::Uuid::new_v4().simple());
    let created = sqlx::raw_sql(AssertSqlSafe(format!(
        "CREATE ROLE \"{name}\" LOGIN PASSWORD '{password}'"
    )))
    .execute(admin)
    .await;
    if let Err(error) = created {
        panic!(
            "the role of the test server cannot create roles ({error}); the replacement of the \
             password is then only covered by the tests without a database"
        );
    }
    sqlx::raw_sql(AssertSqlSafe(format!(
        "GRANT USAGE ON SCHEMA public TO \"{name}\"; \
         GRANT ALL ON ALL TABLES IN SCHEMA public TO \"{name}\"; \
         GRANT ALL ON ALL SEQUENCES IN SCHEMA public TO \"{name}\""
    )))
    .execute(admin)
    .await
    .unwrap();
    Role { name }
}

impl Role {
    /// The address `of`, with this role as the user and `password`, when given, as its
    /// password.
    pub fn url(&self, of: &DatabaseUrl, password: Option<&str>) -> DatabaseUrl {
        let text = of.expose();
        let (scheme, rest) = text.split_once("://").unwrap();
        let authority_end = rest.find('/').unwrap_or(rest.len());
        let after_user = match rest[..authority_end].rfind('@') {
            Some(at) => &rest[at + 1..],
            None => rest,
        };
        let password = password
            .map(|password| format!(":{password}"))
            .unwrap_or_default();
        DatabaseUrl::new(format!("{scheme}://{}{password}@{after_user}", self.name))
    }

    /// The verifier PostgreSQL keeps for this role.
    pub async fn verifier(&self, admin: &PgPool) -> String {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT rolpassword FROM pg_authid WHERE rolname = $1",
        )
        .bind(&self.name)
        .fetch_one(admin)
        .await
        .unwrap()
        .expect("the role has a password")
    }

    /// Removes the role, and its rights in the database `admin` is connected to.
    pub async fn drop(self, admin: &PgPool) {
        sqlx::raw_sql(AssertSqlSafe(format!(
            "DROP OWNED BY \"{0}\"; DROP ROLE \"{0}\"",
            self.name
        )))
        .execute(admin)
        .await
        .unwrap();
    }
}

fn hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).unwrap();
    mac.update(message);
    mac.finalize().into_bytes().into()
}

/// PBKDF2-HMAC-SHA-256 with one block of output: the `Hi` of RFC 5802.
fn salted(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut first = salt.to_vec();
    first.extend_from_slice(&1u32.to_be_bytes());
    let mut block = hmac(password, &first);
    let mut out = block;
    for _ in 1..iterations {
        block = hmac(password, &block);
        for (out, byte) in out.iter_mut().zip(block) {
            *out ^= byte;
        }
    }
    out
}

/// Whether `password` is the one of a SCRAM-SHA-256 verifier
/// (`SCRAM-SHA-256$iterations:salt$StoredKey:ServerKey`).
pub fn opens(verifier: &str, password: &str) -> bool {
    let rest = verifier
        .strip_prefix("SCRAM-SHA-256$")
        .unwrap_or_else(|| panic!("the test server keeps SCRAM-SHA-256 verifiers"));
    let (iterations_and_salt, keys) = rest.split_once('$').unwrap();
    let (iterations, salt) = iterations_and_salt.split_once(':').unwrap();
    let (stored_key, server_key) = keys.split_once(':').unwrap();
    let salt = BASE64.decode(salt.as_bytes()).unwrap();
    let salted = salted(password.as_bytes(), &salt, iterations.parse().unwrap());
    let client_key = hmac(&salted, b"Client Key");
    let stored: [u8; 32] = Sha256::digest(client_key).into();
    let server = hmac(&salted, b"Server Key");
    BASE64.encode(&stored) == stored_key && BASE64.encode(&server) == server_key
}

#[test]
fn the_verifier_is_recomputed_as_postgresql_computes_it() {
    // RFC 7677, section 3: the user "user" with the password "pencil", 4096 iterations.
    let salt = BASE64.decode(b"W22ZaJ0SNY7soEsUEjb6gQ==").unwrap();
    let salted = salted(b"pencil", &salt, 4096);
    let client_key = hmac(&salted, b"Client Key");
    let stored: [u8; 32] = Sha256::digest(client_key).into();
    let server = hmac(&salted, b"Server Key");
    let verifier = format!(
        "SCRAM-SHA-256$4096:W22ZaJ0SNY7soEsUEjb6gQ==${}:{}",
        BASE64.encode(&stored),
        BASE64.encode(&server)
    );
    assert!(opens(&verifier, "pencil"));
    assert!(!opens(&verifier, "pencil "));
    // The ServerSignature of the exchange of the RFC comes out of this ServerKey.
    let auth_message = "n=user,r=rOprNGfwEbeRWgbNEkqO,\
                        r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,\
                        s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096,\
                        c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0";
    assert_eq!(
        BASE64.encode(&hmac(&server, auth_message.as_bytes())),
        "6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4="
    );
}
