//! `arauto-hub admin engine ...` and `arauto-hub admin number ...` (`docs/15-mvp.md`,
//! section 1): the engine, its secrets, and the numbers with their pairing.
//!
//! The rules of the other commands hold: every write goes through the repositories, which
//! check the database against the audit mark first (a database that came back from an older
//! copy gets nothing); the actor is the local admin. The key of the engine is read from the
//! terminal, never from an argument, and never shown; the secret of the events is drawn by
//! the hub and never shown either: the hub gives it to the engine itself. The QR and the code
//! of a pairing are shown once on the terminal and never stored.

use std::sync::Arc;
use std::time::Duration;

use arauto_protocol::clock::Clock;
use arauto_protocol::names::NumberAlias;

use super::{AdminError, Session};
use crate::core::number::{NumberState, Trigger};
use crate::engine::config::ApiKey;
use crate::engine::registry::LoadedEngine;
use crate::engine::{
    EngineAddress, EngineError, EngineSettings, EngineStatus, EngineVersion, Engines, Ensured,
    EventsUrl, PairingCode, PairingQr, Phone,
};
use crate::repo;
use crate::repo::alert::{AlertDetails, AlertKind, NewAlert, raise};
use crate::repo::engine::{EngineRow, SetOutcome};
use crate::repo::number::{AddOutcome, ChangeOutcome, Number, Observed};
use crate::vault::Vault;

/// How to pair a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairMethod {
    /// A QR on the terminal, scanned by the telephone.
    Qr,
    /// A code shown on the terminal and typed on the telephone.
    Code,
}

/// What the pairing shows, as it goes.
#[derive(Debug)]
pub enum Shown<'a> {
    /// A QR to scan: a new one each time the engine changes it.
    Qr(&'a PairingQr),
    /// The code to type on the telephone.
    Code(&'a PairingCode),
}

/// How a pairing ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairOutcome {
    /// The expected telephone is connected.
    Connected,
    /// Another telephone paired: the accesses wait for `admin number confirm`.
    Swapped,
    /// Nothing was scanned or typed in time: the number is unpaired again.
    Expired,
    /// The engine reported a failure of the session.
    Failed,
}

/// What `admin number add` did.
#[derive(Debug)]
pub struct Added {
    /// The number, unpaired.
    pub number: Number,
    /// Whether its session was created in the engine now. When the engine did not answer, the
    /// hub creates it at its next check.
    pub engine: Result<Ensured, EngineError>,
}

/// What a change that also goes to the engine did.
#[derive(Debug)]
pub struct Changed {
    /// The number, after the change.
    pub number: Number,
    /// What the engine answered. The change in the hub holds either way.
    pub engine: Result<(), EngineError>,
}

/// What happens to the session of the engine of a removed number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    /// It is stopped and stays in the engine, with its history: there is no archive yet to
    /// import it to (`referencia/waha-fatos.md`: leaving erases the history).
    KeepInEngine,
    /// It leaves WhatsApp and is deleted, and the engine erases its history.
    EraseEngineHistory,
}

fn alias(text: &str) -> Result<NumberAlias, AdminError> {
    NumberAlias::parse(text).map_err(AdminError::InvalidAlias)
}

/// A text given to a command that looks for an existing number: outside the format, it is
/// the alias of no number.
fn known_alias(text: &str) -> Result<NumberAlias, AdminError> {
    NumberAlias::parse(text).map_err(|_| AdminError::NoSuchNumber)
}

fn changed(outcome: ChangeOutcome) -> Result<(Number, NumberState), AdminError> {
    match outcome {
        ChangeOutcome::Changed(number, from) => Ok((number, from)),
        ChangeOutcome::NotFound => Err(AdminError::NoSuchNumber),
        ChangeOutcome::NotNow(state) => Err(AdminError::NumberNotNow(state.as_str())),
        ChangeOutcome::PhoneTaken => Err(AdminError::PhoneTaken),
    }
}

impl Session {
    pub(super) async fn engine(&self, vault: &Vault) -> Result<Arc<LoadedEngine>, AdminError> {
        let engines = Engines::new(EngineSettings::default());
        match engines.of(&self.pool, vault, self.organization).await {
            Ok(engine) => Ok(engine),
            Err(EngineError::Unavailable(crate::engine::Cause::NotConfigured)) => {
                match repo::engine::get(&self.pool, self.organization).await? {
                    None => Err(AdminError::NoEngine),
                    // There is an engine and no key: the command says how to set it.
                    Some(_) => {
                        Err(EngineError::Unavailable(crate::engine::Cause::NotConfigured).into())
                    }
                }
            }
            Err(other) => Err(other.into()),
        }
    }

    // ------------------------------------------------------------------ engine

    /// `admin engine set --url URL --events-url URL`.
    ///
    /// # Errors
    /// [`AdminError::InvalidAddress`]; [`AdminError::Restored`].
    pub async fn engine_set(
        &self,
        vault: &Vault,
        url: &str,
        events_url: &str,
        now: i64,
    ) -> Result<SetOutcome, AdminError> {
        let address = EngineAddress::parse(url).map_err(AdminError::InvalidAddress)?;
        let events = EventsUrl::parse(events_url).map_err(AdminError::InvalidAddress)?;
        Ok(repo::engine::set_address(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            vault,
            &address,
            &events,
            now,
        )
        .await?)
    }

    /// `admin engine set-key`: `typed` is the line read from the terminal.
    ///
    /// # Errors
    /// [`AdminError::InvalidKey`]; [`AdminError::NoEngine`]; [`AdminError::Restored`].
    pub async fn engine_set_key(
        &self,
        vault: &Vault,
        typed: &str,
        now: i64,
    ) -> Result<(), AdminError> {
        let key = ApiKey::parse(typed).map_err(AdminError::InvalidKey)?;
        let set = repo::engine::set_key(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            vault,
            &key,
            now,
        )
        .await?;
        if set {
            Ok(())
        } else {
            Err(AdminError::NoEngine)
        }
    }

    /// `admin engine new-events-secret`.
    ///
    /// # Errors
    /// [`AdminError::NoEngine`]; [`AdminError::Restored`].
    pub async fn engine_new_events_secret(
        &self,
        vault: &Vault,
        now: i64,
    ) -> Result<(), AdminError> {
        let drawn = repo::engine::new_events_secret(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            vault,
            now,
        )
        .await?;
        if drawn {
            Ok(())
        } else {
            Err(AdminError::NoEngine)
        }
    }

    /// `admin engine show`.
    ///
    /// # Errors
    /// What the database returns.
    pub async fn engine_show(&self) -> Result<Option<EngineRow>, AdminError> {
        Ok(repo::engine::get(&self.pool, self.organization).await?)
    }

    /// `admin engine health`.
    ///
    /// # Errors
    /// [`AdminError::NoEngine`]; [`AdminError::Engine`] when the engine is not healthy.
    pub async fn engine_health(&self, vault: &Vault) -> Result<(), AdminError> {
        let engine = self.engine(vault).await?;
        Ok(engine.client.health().await?)
    }

    /// `admin engine version`: asks the engine, records what it said, and alerts when it is
    /// a version the hub was not checked against.
    ///
    /// # Errors
    /// [`AdminError::NoEngine`]; [`AdminError::Engine`].
    pub async fn engine_version(
        &self,
        vault: &Vault,
        now: i64,
    ) -> Result<EngineVersion, AdminError> {
        let engine = self.engine(vault).await?;
        let version = engine.client.version().await?;
        let changed =
            repo::engine::record_version(&self.pool, self.organization, engine.id, &version, now)
                .await?;
        if changed && !version.is_tested() {
            let mut connection = self.pool.acquire().await?;
            raise(
                &mut connection,
                self.organization,
                now,
                &NewAlert {
                    kind: AlertKind::EngineVersionUntested,
                    hermes: None,
                    device: None,
                    details: AlertDetails::new().id("engine", engine.id),
                },
            )
            .await?;
        }
        Ok(version)
    }

    // ------------------------------------------------------------------ numbers

    /// `admin number add ALIAS PHONE [--command]`: the number, unpaired, and its session in the
    /// engine, created already configured.
    ///
    /// # Errors
    /// [`AdminError::InvalidAlias`], [`AdminError::InvalidPhone`], [`AdminError::NoEngine`],
    /// [`AdminError::AliasTaken`], [`AdminError::PhoneTaken`]; [`AdminError::Restored`].
    pub async fn number_add(
        &self,
        vault: &Vault,
        alias_text: &str,
        phone: &str,
        command: bool,
        now: i64,
    ) -> Result<Added, AdminError> {
        let alias = alias(alias_text)?;
        let phone = Phone::parse(phone).map_err(|_| AdminError::InvalidPhone)?;
        let key = repo::device::organization_key(&self.pool, self.organization, vault).await?;
        let added = repo::number::add(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &key,
            &alias,
            &phone,
            command,
            now,
        )
        .await?;
        let number = match added {
            AddOutcome::Added(number) => number,
            AddOutcome::NoEngine => return Err(AdminError::NoEngine),
            AddOutcome::AliasTaken => return Err(AdminError::AliasTaken),
            AddOutcome::PhoneTaken => return Err(AdminError::PhoneTaken),
        };
        let engine = match self.engine(vault).await {
            Ok(engine) => self.ensure(&engine, &number, now).await,
            Err(AdminError::Engine(error)) => Err(error),
            Err(other) => return Err(other),
        };
        Ok(Added { number, engine })
    }

    async fn ensure(
        &self,
        engine: &LoadedEngine,
        number: &Number,
        now: i64,
    ) -> Result<Ensured, EngineError> {
        let config = engine.session_config();
        let digest = engine.client.config_digest(&config);
        let applied_before = number.config_digest.as_deref() == Some(digest.as_slice());
        let ensured = engine
            .client
            .ensure_session(&number.session, &config, applied_before)
            .await?;
        if ensured != Ensured::Unchanged || !applied_before {
            repo::number::config_applied(&self.pool, self.organization, number.id, &digest, now)
                .await
                .map_err(|_| EngineError::Unavailable(crate::engine::Cause::Connect))?;
        }
        Ok(ensured)
    }

    /// `admin number list`.
    ///
    /// # Errors
    /// What the database returns.
    pub async fn number_list(&self, include_removed: bool) -> Result<Vec<Number>, AdminError> {
        Ok(repo::number::list(&self.pool, self.organization, include_removed).await?)
    }

    /// The telephone of a number, for the screen of `number list`.
    ///
    /// # Errors
    /// What the vault or the database return.
    pub async fn number_phone(&self, vault: &Vault, number: &Number) -> Result<Phone, AdminError> {
        let key = repo::device::organization_key(&self.pool, self.organization, vault).await?;
        Ok(number.phone(&key)?)
    }

    /// `admin number pair ALIAS [--code]`: puts the number in pairing, makes sure its session
    /// exists configured, starts it, and shows each QR (or the code) through `show` until the
    /// telephone connects or the pairing expires.
    ///
    /// # Errors
    /// [`AdminError::NoSuchNumber`], [`AdminError::NumberNotNow`], [`AdminError::NoEngine`],
    /// [`AdminError::Engine`]; [`AdminError::Restored`].
    pub async fn number_pair(
        &self,
        vault: &Vault,
        alias_text: &str,
        method: PairMethod,
        settings: &EngineSettings,
        clock: &dyn Clock,
        mut show: impl FnMut(Shown<'_>),
    ) -> Result<PairOutcome, AdminError> {
        let alias = known_alias(alias_text)?;
        let engine = self.engine(vault).await?;
        let now = clock.now_ms();
        let until = now.saturating_add(settings.pairing_ms);
        let (number, _) = changed(
            repo::number::change(
                &self.pool,
                &self.mark,
                self.organization,
                &self.admin,
                &alias,
                Trigger::Pair,
                Some(until),
                now,
            )
            .await?,
        )?;
        let key = repo::device::organization_key(&self.pool, self.organization, vault).await?;
        let phone = number.phone(&key)?;
        self.ensure(&engine, &number, now).await?;
        engine.client.start(&number.session).await?;
        let mut last_qr: Option<String> = None;
        let mut code_shown = false;
        while clock.now_ms() <= until {
            let info = engine.client.session(&number.session).await?;
            match info.as_ref().map(|info| info.status) {
                Some(EngineStatus::Working) => {
                    let me = info.and_then(|info| info.me).and_then(|me| me.phone);
                    let observed = Observed {
                        status: EngineStatus::Working,
                        phone: me,
                        at: None,
                    };
                    let mut transaction = self.pool.begin().await?;
                    let change = repo::number::observe(
                        &mut transaction,
                        self.organization,
                        &key,
                        number.id,
                        &observed,
                        clock.now_ms(),
                    )
                    .await?;
                    transaction.commit().await?;
                    // What happened is what the row says now, whoever wrote it: this loop,
                    // an event of the engine or the supervision. An engine that works and
                    // does not name its telephone yet is asked again.
                    let state = match change {
                        Some((_, to)) => to,
                        None => repo::number::by_alias(&self.pool, self.organization, &alias)
                            .await?
                            .map_or(NumberState::Pairing, |number| number.state),
                    };
                    match state {
                        NumberState::Connected => return Ok(PairOutcome::Connected),
                        NumberState::NumberSwapped => return Ok(PairOutcome::Swapped),
                        NumberState::Unpaired => return Ok(PairOutcome::Expired),
                        NumberState::Failed => return Ok(PairOutcome::Failed),
                        _ => {}
                    }
                }
                Some(EngineStatus::Failed) => {
                    let _ = engine.client.stop(&number.session).await;
                    repo::number::expire_pairings(
                        &self.pool,
                        self.organization,
                        until.saturating_add(1),
                    )
                    .await?;
                    return Ok(PairOutcome::Failed);
                }
                Some(EngineStatus::ScanQrCode) => match method {
                    PairMethod::Qr => {
                        let qr = engine.client.pairing_qr(&number.session).await?;
                        if last_qr.as_deref() != Some(qr.expose()) {
                            show(Shown::Qr(&qr));
                            last_qr = Some(qr.expose().to_owned());
                        }
                    }
                    PairMethod::Code if !code_shown => {
                        let code = engine.client.pairing_code(&number.session, &phone).await?;
                        show(Shown::Code(&code));
                        code_shown = true;
                    }
                    PairMethod::Code => {}
                },
                _ => {}
            }
            tokio::time::sleep(Duration::from_millis(
                u64::try_from(settings.pairing_poll_ms).unwrap_or(0),
            ))
            .await;
        }
        let _ = engine.client.stop(&number.session).await;
        repo::number::expire_pairings(&self.pool, self.organization, until.saturating_add(1))
            .await?;
        Ok(PairOutcome::Expired)
    }

    async fn number_change(
        &self,
        vault: &Vault,
        alias_text: &str,
        trigger: Trigger,
        now: i64,
    ) -> Result<(Number, Arc<LoadedEngine>), AdminError> {
        let alias = known_alias(alias_text)?;
        let engine = self.engine(vault).await?;
        let (number, _) = changed(
            repo::number::change(
                &self.pool,
                &self.mark,
                self.organization,
                &self.admin,
                &alias,
                trigger,
                None,
                now,
            )
            .await?,
        )?;
        Ok((number, engine))
    }

    /// `admin number disconnect ALIAS`: stops its session, without leaving WhatsApp.
    ///
    /// # Errors
    /// [`AdminError::NoSuchNumber`], [`AdminError::NumberNotNow`]; [`AdminError::Restored`].
    pub async fn number_disconnect(
        &self,
        vault: &Vault,
        alias_text: &str,
        now: i64,
    ) -> Result<Changed, AdminError> {
        let (number, engine) = self
            .number_change(vault, alias_text, Trigger::Disconnect, now)
            .await?;
        let engine = engine.client.stop(&number.session).await;
        Ok(Changed { number, engine })
    }

    /// `admin number restart ALIAS`: a failed session of the engine starts again.
    ///
    /// # Errors
    /// [`AdminError::NoSuchNumber`], [`AdminError::NumberNotNow`]; [`AdminError::Restored`].
    pub async fn number_restart(
        &self,
        vault: &Vault,
        alias_text: &str,
        now: i64,
    ) -> Result<Changed, AdminError> {
        let (number, engine) = self
            .number_change(vault, alias_text, Trigger::Restart, now)
            .await?;
        let engine = engine.client.restart(&number.session).await;
        Ok(Changed { number, engine })
    }

    /// `admin number confirm ALIAS`: the new telephone of a swapped number is accepted.
    ///
    /// # Errors
    /// [`AdminError::NoSuchNumber`], [`AdminError::NumberNotNow`]; [`AdminError::Restored`].
    pub async fn number_confirm(&self, alias_text: &str, now: i64) -> Result<Number, AdminError> {
        let alias = known_alias(alias_text)?;
        let (number, _) = changed(
            repo::number::change(
                &self.pool,
                &self.mark,
                self.organization,
                &self.admin,
                &alias,
                Trigger::Confirm,
                None,
                now,
            )
            .await?,
        )?;
        Ok(number)
    }

    /// `admin number unpair ALIAS --erase-engine-history`: the pairing of a swapped number is
    /// undone. Leaving WhatsApp erases the history the engine keeps: the command asks for it
    /// in so many words.
    ///
    /// # Errors
    /// [`AdminError::NoSuchNumber`], [`AdminError::NumberNotNow`]; [`AdminError::Restored`].
    pub async fn number_unpair(
        &self,
        vault: &Vault,
        alias_text: &str,
        now: i64,
    ) -> Result<Changed, AdminError> {
        let (number, engine) = self
            .number_change(vault, alias_text, Trigger::Unpair, now)
            .await?;
        let engine = engine.client.log_out_and_delete(&number.session).await;
        Ok(Changed { number, engine })
    }

    /// `admin number remove ALIAS --keep-in-engine | --erase-engine-history`: every access to
    /// it ends now; its session is stopped and kept, or leaves WhatsApp and is deleted.
    ///
    /// # Errors
    /// [`AdminError::NoSuchNumber`]; [`AdminError::Restored`].
    pub async fn number_remove(
        &self,
        vault: &Vault,
        alias_text: &str,
        removal: Removal,
        now: i64,
    ) -> Result<Changed, AdminError> {
        let alias = known_alias(alias_text)?;
        let (number, _) = changed(
            repo::number::change(
                &self.pool,
                &self.mark,
                self.organization,
                &self.admin,
                &alias,
                Trigger::Remove,
                None,
                now,
            )
            .await?,
        )?;
        let engine = match self.engine(vault).await {
            Ok(engine) => match removal {
                Removal::KeepInEngine => engine.client.stop(&number.session).await,
                Removal::EraseEngineHistory => {
                    engine.client.log_out_and_delete(&number.session).await
                }
            },
            Err(AdminError::Engine(error)) => Err(error),
            Err(AdminError::NoEngine) => Ok(()),
            Err(other) => return Err(other),
        };
        Ok(Changed { number, engine })
    }
}
