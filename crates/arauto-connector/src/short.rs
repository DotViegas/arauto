//! The short connections: the enrollment of `join` and the request to leave of `leave`
//! (`docs/14-protocolo-do-canal.md`, sections 5.2 and 5.4; `docs/04-seguranca-conexao.md`, C3,
//! C4 and C10).
//!
//! Each one proves the identity in the handshake, carries one request and closes. Neither
//! presents a credential, neither becomes a session, and neither counts as a double connection.

use arauto_protocol::code::VerificationCode;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::invite::Invite;
use arauto_protocol::message::{
    ConfirmWithin, Declared, DeclaredText, Enroll, EnrollmentNumber, Hello, HelloOk, LeftStatus,
    Message, ProtoRange, Purpose,
};
use arauto_protocol::noise::{PublicKey, StaticKeypair};

use crate::address::HubAddress;
use crate::attempt::version;
use crate::catalog::{Local, Said};
use crate::link::{self, Link, OpenError, RecvError};
use crate::settings::Settings;

/// How an enrollment ended.
#[derive(Debug)]
pub(crate) enum Enrolled {
    /// The hub accepted it.
    Accepted {
        /// The code the Hermes says and the admin types.
        code: VerificationCode,
        /// In how many seconds it has to be typed.
        confirm_within: ConfirmWithin,
        /// Which enrollment of the request this is.
        enrollment: EnrollmentNumber,
    },
    /// The hub refused it with this code.
    Refused(ErrorCode),
    /// It did not reach the hub, or broke on the way. `sent` says whether `enroll` left: the
    /// hub may then have recorded the request.
    Failed {
        /// What happened.
        said: Said,
        /// Whether `enroll` was sent.
        sent: bool,
    },
}

fn failed(error: OpenError) -> Said {
    match error {
        OpenError::Unreachable(cause) => Said::Unreachable(cause),
        OpenError::Busy => Said::code(ErrorCode::Ar205),
        OpenError::NotTheHub => Said::code(ErrorCode::Ar202),
        OpenError::NoTls => Said::Local(Local::NoTls),
        OpenError::Random => Said::Local(Local::Random),
    }
}

/// What a short connection says when the hub closed it, or did not answer.
fn ended(recv: RecvError) -> Said {
    match recv {
        RecvError::Ended(end) => end.closing.map_or(
            Said::Unreachable(arauto_protocol::errors::ConnectFailure::Refused),
            Said::code,
        ),
        RecvError::TimedOut => Said::Unreachable(arauto_protocol::errors::ConnectFailure::TimedOut),
    }
}

/// Sends `hello` and reads `hello_ok`.
async fn hello(link: &mut Link, purpose: Purpose, settings: &Settings) -> Result<(), RecvError> {
    let offered = ProtoRange::CURRENT;
    let hello = Message::Hello(Hello {
        proto: offered,
        connector: version(),
        purpose,
    });
    if link.send(&hello).await.is_err() {
        return Err(RecvError::Ended(link.handle().ended().await));
    }
    match link
        .recv_within(settings.deadlines.wait_hello_ok_ms)
        .await?
    {
        Message::HelloOk(answer) if answer.fits(offered) => {
            // A connector too old is still served when it asks to leave.
            if answer == HelloOk::Outdated && purpose == Purpose::Enroll {
                link.close();
                return Err(RecvError::Ended(link.handle().ended().await));
            }
            link.start_heartbeat();
            Ok(())
        }
        _ => {
            link.close();
            Err(RecvError::Ended(link.handle().ended().await))
        }
    }
}

/// What the device says about itself: the name of the machine and its system. Not verified,
/// and not trusted by the hub; nothing here comes from the environment.
pub(crate) fn declared() -> Declared {
    Declared {
        hostname: DeclaredText::lossy(&arauto_platform::host_name()),
        os: DeclaredText::lossy(&format!(
            "{} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )),
        hermes_version: DeclaredText::default(),
    }
}

/// The enrollment: a short connection that presents the invite (C3).
pub(crate) async fn enroll(
    address: &HubAddress,
    hub_key: &PublicKey,
    identity: &StaticKeypair,
    invite: Invite,
    settings: &Settings,
) -> Enrolled {
    let mut link = match link::open(address, identity, hub_key, settings).await {
        Ok(link) => link,
        Err(error) => {
            return Enrolled::Failed {
                said: failed(error),
                sent: false,
            };
        }
    };
    if let Err(error) = hello(&mut link, Purpose::Enroll, settings).await {
        return refused_or_failed(error, false);
    }
    let enroll = Message::Enroll(Enroll {
        invite,
        declared: declared(),
    });
    let sent = link.send(&enroll).await;
    drop(enroll);
    if sent.is_err() {
        return Enrolled::Failed {
            said: Said::Unreachable(arauto_protocol::errors::ConnectFailure::Refused),
            sent: true,
        };
    }
    let answer = link.recv_within(settings.deadlines.wait_enrolled_ms).await;
    link.close();
    match answer {
        Ok(Message::Enrolled(enrolled)) => {
            match VerificationCode::derive(&enrolled.nonce, link.handshake_hash()) {
                Ok(code) => Enrolled::Accepted {
                    code,
                    confirm_within: enrolled.confirm_within_s,
                    enrollment: enrolled.enrollment,
                },
                // The hub draws another nonce before it sends one that gives no code.
                Err(_) => Enrolled::Failed {
                    said: Said::code(ErrorCode::Ar204),
                    sent: true,
                },
            }
        }
        Ok(_) => Enrolled::Failed {
            said: Said::code(ErrorCode::Ar204),
            sent: true,
        },
        Err(error) => refused_or_failed(error, true),
    }
}

fn refused_or_failed(error: RecvError, sent: bool) -> Enrolled {
    match error {
        RecvError::Ended(end) => match end.closing {
            Some(code) => Enrolled::Refused(code),
            None => Enrolled::Failed {
                said: ended(error),
                sent,
            },
        },
        RecvError::TimedOut => Enrolled::Failed {
            said: ended(error),
            sent,
        },
    }
}

/// The request to leave over a short connection, one per identity (C10). `Err` when the hub
/// did not answer, or is not the hub of the key: then nothing may be deleted (AR-505).
pub(crate) async fn leave(
    address: &HubAddress,
    hub_key: &PublicKey,
    identity: &StaticKeypair,
    settings: &Settings,
) -> Result<LeftStatus, Said> {
    let mut link = link::open(address, identity, hub_key, settings)
        .await
        .map_err(failed)?;
    hello(&mut link, Purpose::Leave, settings)
        .await
        .map_err(ended)?;
    if link.send(&Message::Leave).await.is_err() {
        return Err(Said::Unreachable(
            arauto_protocol::errors::ConnectFailure::Refused,
        ));
    }
    let answer = link.recv_within(settings.deadlines.wait_left_ms).await;
    link.close();
    match answer {
        Ok(Message::Left(left)) => Ok(left.status),
        Ok(_) => Err(Said::code(ErrorCode::Ar204)),
        Err(error) => Err(ended(error)),
    }
}
