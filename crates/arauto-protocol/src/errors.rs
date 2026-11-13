//! The catalog of errors.
//!
//! Every refusal has a stable code, the same in the Hermes, in the hub and in the audit, and a
//! fixed phrase in English, in ASCII only. The phrase says what happened and who solves it,
//! and never brings a command to run. The hub sends only the code: the phrase lives here,
//! compiled into the connector, so the hub cannot change a text that an LLM reads
//! (`docs/11-conector.md`, section 8).

use core::fmt;
use core::str::FromStr;
use std::borrow::Cow;

use crate::limits::RETRY_AFTER_MAX_S;

bounded_integer!(
    /// In how many seconds a refusal stops applying: from 1 to 86,400.
    ///
    /// It is the only number that goes into an error phrase, and only after this check.
    RetryAfter(u32),
    1,
    RETRY_AFTER_MAX_S,
    "retry_after_s"
);

macro_rules! error_codes {
    ($( $variant:ident = $code:literal, $phrase:literal; )+) => {
        /// A code of the catalog.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum ErrorCode {
            $(
                #[doc = $phrase]
                $variant,
            )+
        }

        impl ErrorCode {
            /// Every code, in the order of the catalog.
            pub const ALL: &'static [ErrorCode] = &[$(ErrorCode::$variant,)+];

            /// The code as text, like `AR-101`.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(ErrorCode::$variant => $code,)+
                }
            }

            /// Reads a code written exactly as [`ErrorCode::as_str`] writes it.
            #[must_use]
            pub fn parse(text: &str) -> Option<Self> {
                match text {
                    $($code => Some(ErrorCode::$variant),)+
                    _ => None,
                }
            }

            const fn fixed_phrase(self) -> &'static str {
                match self {
                    $(ErrorCode::$variant => concat!($code, ": ", $phrase),)+
                }
            }
        }
    };
}

error_codes! {
    Ar101 = "AR-101", "The hub did not accept this invite: it is expired, cancelled, unknown or already used. Nothing changed here. The admin must issue a new invite.";
    Ar102 = "AR-102", "This invite was already used by another device. Nothing was installed here. The admin must issue a new invite and check the alert on the hub.";
    Ar104 = "AR-104", "This connection request is already approved. There is nothing to do.";
    Ar105 = "AR-105", "This folder already holds a device of another hub. It must leave that hub first; if the old hub is gone or changed its key, a person must do the local leave.";
    Ar201 = "AR-201", "No connection to the hub: name not resolved, timeout or connection refused. The operator must check the network and the hub address.";
    Ar202 = "AR-202", "The server at this address is not the hub of the key in the block. The connection was refused. The admin must issue a new block.";
    Ar203 = "AR-203", "The channel to the hub is down. This call was not sent and can be repeated. The connector keeps trying to reconnect.";
    Ar204 = "AR-204", "The hub closed the connection: protocol error or limit exceeded. The connector reconnects by itself.";
    Ar205 = "AR-205", "The hub is stopping or out of resources. The connector reconnects by itself in a moment.";
    Ar206 = "AR-206", "This connection attempt was replaced by a newer one from the same device. The connector tries again by itself.";
    Ar207 = "AR-207", "The call was sent and the channel dropped before the answer: the result is unknown. Check whether it took effect before repeating it.";
    Ar208 = "AR-208", "The hub gave this connection up for dead: another connection of the same device took its place. The connector reconnects right away.";
    Ar301 = "AR-301", "The connection request was denied. Only the admin can issue a new invite.";
    Ar302 = "AR-302", "The connection request expired without approval: nobody confirmed the code in time, or the hub closed the request. The admin must issue a new invite.";
    Ar303 = "AR-303", "The connection request was approved but expired before the connector finished. The admin must reconnect this Hermes.";
    Ar401 = "AR-401", "This connector is outdated. The admin must reconnect this Hermes from the hub.";
    Ar402 = "AR-402", "The hub is older than this connector. Whoever runs the hub must update it.";
    Ar501 = "AR-501", "This device is in quarantine: the hub stopped trusting its credential. The admin must reconnect this Hermes from the hub.";
    Ar502 = "AR-502", "This device was revoked. Only the admin can connect this Hermes again.";
    Ar503 = "AR-503", "This device was replaced by another one. There is nothing to do on this one.";
    Ar504 = "AR-504", "The hub does not recognize this device. Nothing was deleted. The admin must reconnect this Hermes from the hub.";
    Ar505 = "AR-505", "Leaving is pending: the hub did not answer. Nothing was deleted. The admin must revoke this device on the hub.";
    Ar506 = "AR-506", "This device is suspended: the hub came back from a backup. The admin must reconnect this Hermes from the hub.";
    Ar507 = "AR-507", "This device expired for inactivity. The admin must reconnect this Hermes from the hub.";
    Ar601 = "AR-601", "Registering with Hermes failed or was refused. The connection request is still open; repeating the same join retries the registration.";
    Ar602 = "AR-602", "The local Hermes API does not answer or refuses the key. The operator must fix it; until then this Hermes is not woken up.";
    Ar701 = "AR-701", "This Hermes does not have the permission this call needs. Only the admin can grant it.";
    Ar702 = "AR-702", "This number is unavailable: disconnected from WhatsApp or waiting for the admin to confirm it. The admin must fix it.";
    Ar703 = "AR-703", "The WhatsApp engine is unavailable. Whoever runs the hub must fix it.";
    Ar704 = "AR-704", "Send limit reached. Wait before sending again; insisting does not help.";
    Ar705 = "AR-705", "Chat not found: it does not exist or is outside what this Hermes can use.";
    Ar706 = "AR-706", "Message limit between Hermes agents reached. Wait before sending again; insisting does not help.";
    Ar707 = "AR-707", "There is no peer with that name that this Hermes can send to. Only the admin changes that.";
    Ar708 = "AR-708", "Thread not found: it does not exist or belongs to others.";
    Ar709 = "AR-709", "Too large: ask for fewer items or send smaller content.";
    Ar710 = "AR-710", "Too many calls at the same time. Try again in a moment.";
    Ar711 = "AR-711", "Invalid argument. Check the description of the tool and call again.";
    Ar712 = "AR-712", "Message not found: it does not exist, is outside what this Hermes can use, or WhatsApp no longer allows it.";
    Ar713 = "AR-713", "The result of the sending is unknown: the message may have been sent. Read the chat before sending it again.";
}

impl ErrorCode {
    /// The fixed phrase of the code, starting with the code itself.
    ///
    /// `retry_after` only matters to the two limits, AR-704 and AR-706, whose phrase then says
    /// when the limit comes back. No other value ever enters a phrase.
    #[must_use]
    pub fn phrase(self, retry_after: Option<RetryAfter>) -> Cow<'static, str> {
        match (self, retry_after) {
            (ErrorCode::Ar704, Some(wait)) => Cow::Owned(format!(
                "AR-704: Send limit reached. It comes back in {wait} second(s); insisting before \
                 that does not help."
            )),
            (ErrorCode::Ar706, Some(wait)) => Cow::Owned(format!(
                "AR-706: Message limit between Hermes agents reached. It comes back in {wait} \
                 second(s); insisting before that does not help."
            )),
            _ => Cow::Borrowed(self.fixed_phrase()),
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The text is not a code of the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("not a code of the error catalog")]
pub struct UnknownErrorCode;

impl FromStr for ErrorCode {
    type Err = UnknownErrorCode;

    fn from_str(text: &str) -> Result<Self, UnknownErrorCode> {
        Self::parse(text).ok_or(UnknownErrorCode)
    }
}

impl serde::Serialize for ErrorCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for ErrorCode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::serde_util::text(deserializer, "error code", Self::parse)
    }
}

/// Why the connector could not reach the hub. It picks the phrase of AR-201.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectFailure {
    /// The name of the hub did not resolve.
    NameNotResolved,
    /// The attempt ran out of time.
    TimedOut,
    /// The connection was refused.
    Refused,
}

impl ConnectFailure {
    /// Every cause.
    pub const ALL: &'static [ConnectFailure] = &[
        ConnectFailure::NameNotResolved,
        ConnectFailure::TimedOut,
        ConnectFailure::Refused,
    ];

    /// The fixed phrase of AR-201 for this cause.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            ConnectFailure::NameNotResolved => {
                "AR-201: No connection to the hub: its name was not resolved. The operator must \
                 check the network and the hub address."
            }
            ConnectFailure::TimedOut => {
                "AR-201: No connection to the hub: the attempt timed out. The operator must check \
                 the network and whether the hub is running."
            }
            ConnectFailure::Refused => {
                "AR-201: No connection to the hub: the connection was refused. The operator must \
                 check the hub address and whether the hub is running."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The catalog of `docs/11-conector.md`, section 8, written a second time.
    const CATALOG: [&str; 39] = [
        "AR-101", "AR-102", "AR-104", "AR-105", "AR-201", "AR-202", "AR-203", "AR-204", "AR-205",
        "AR-206", "AR-207", "AR-208", "AR-301", "AR-302", "AR-303", "AR-401", "AR-402", "AR-501",
        "AR-502", "AR-503", "AR-504", "AR-505", "AR-506", "AR-507", "AR-601", "AR-602", "AR-701",
        "AR-702", "AR-703", "AR-704", "AR-705", "AR-706", "AR-707", "AR-708", "AR-709", "AR-710",
        "AR-711", "AR-712", "AR-713",
    ];

    fn every_phrase() -> Vec<(ErrorCode, String)> {
        let waits = [
            None,
            RetryAfter::new(1),
            RetryAfter::new(90),
            RetryAfter::new(86_400),
        ];
        let mut phrases = Vec::new();
        for code in ErrorCode::ALL {
            for wait in waits {
                phrases.push((*code, code.phrase(wait).into_owned()));
            }
        }
        for cause in ConnectFailure::ALL {
            phrases.push((ErrorCode::Ar201, cause.phrase().to_owned()));
        }
        phrases
    }

    #[test]
    fn the_enum_has_every_code_of_the_catalog_and_no_other() {
        let codes: Vec<&str> = ErrorCode::ALL.iter().map(|code| code.as_str()).collect();
        assert_eq!(codes, CATALOG);
    }

    #[test]
    fn codes_are_unique() {
        let texts: HashSet<&str> = ErrorCode::ALL.iter().map(|code| code.as_str()).collect();
        assert_eq!(texts.len(), ErrorCode::ALL.len());
        let variants: HashSet<ErrorCode> = ErrorCode::ALL.iter().copied().collect();
        assert_eq!(variants.len(), ErrorCode::ALL.len());
    }

    #[test]
    fn codes_go_to_text_and_back() {
        for code in ErrorCode::ALL {
            let text = code.as_str();
            assert_eq!(ErrorCode::parse(text), Some(*code));
            assert_eq!(text.parse::<ErrorCode>(), Ok(*code));
            assert_eq!(code.to_string(), text);
            let json = serde_json::to_string(code).unwrap();
            assert_eq!(json, format!("\"{text}\""));
            assert_eq!(serde_json::from_str::<ErrorCode>(&json).unwrap(), *code);
        }
    }

    #[test]
    fn the_code_format_is_strict() {
        for text in [
            "",
            "AR-103",
            "AR-000",
            "AR-999",
            "ar-101",
            "Ar-101",
            "AR101",
            "AR-101 ",
            " AR-101",
            "AR-1011",
            "AR-10",
            "AR_101",
            "101",
            "AR-101\n",
            "AR-\u{ff11}01",
        ] {
            assert_eq!(ErrorCode::parse(text), None, "{text:?}");
            assert_eq!(text.parse::<ErrorCode>(), Err(UnknownErrorCode), "{text:?}");
        }
        for json in ["\"AR-103\"", "\"ar-101\"", "101", "null", "[\"AR-101\"]"] {
            assert!(serde_json::from_str::<ErrorCode>(json).is_err(), "{json}");
        }
    }

    #[test]
    fn every_phrase_is_plain_ascii_and_starts_with_its_code() {
        for (code, phrase) in every_phrase() {
            assert!(phrase.is_ascii(), "{phrase}");
            assert!(
                phrase.bytes().all(|b| b == b' ' || b.is_ascii_graphic()),
                "control character or line break in: {phrase}"
            );
            let prefix = format!("{code}: ");
            assert!(phrase.starts_with(&prefix), "{phrase}");
            let sentence = &phrase[prefix.len()..];
            assert!(
                sentence.len() >= 20,
                "too short to say what happened: {phrase}"
            );
            assert!(
                sentence.len() <= 200,
                "too long for a short and literal text: {phrase}"
            );
            assert!(sentence.ends_with('.'), "{phrase}");
            assert!(!sentence.contains("  "), "{phrase}");
        }
    }

    #[test]
    fn phrases_are_unique() {
        let phrases: HashSet<String> = ErrorCode::ALL
            .iter()
            .map(|code| code.phrase(None).into_owned())
            .collect();
        assert_eq!(phrases.len(), ErrorCode::ALL.len());
    }

    /// A phrase tells who solves the problem; it never hands the LLM something to execute.
    #[test]
    fn no_phrase_brings_a_command() {
        let forbidden = [
            "`",
            "$",
            "--",
            "sudo",
            "curl",
            "wget",
            "arauto-connector",
            "arauto-hub",
            "~/",
            "/v1/",
            ".arauto",
            "http",
            "|",
            "&&",
            "<",
            ">",
            "\\",
            "/",
        ];
        for (_, phrase) in every_phrase() {
            for piece in forbidden {
                assert!(!phrase.contains(piece), "{piece:?} in: {phrase}");
            }
        }
    }

    #[test]
    fn retry_after_limits() {
        assert_eq!(RetryAfter::new(0), None);
        assert_eq!(RetryAfter::new(1).map(RetryAfter::get), Some(1));
        assert_eq!(RetryAfter::new(86_400).map(RetryAfter::get), Some(86_400));
        assert_eq!(RetryAfter::new(86_401), None);
        assert_eq!(RetryAfter::new(u32::MAX), None);
        assert_eq!(RetryAfter::MIN.get(), 1);
        assert_eq!(RetryAfter::MAX.get(), 86_400);

        assert_eq!(serde_json::from_str::<RetryAfter>("1").unwrap().get(), 1);
        assert_eq!(
            serde_json::from_str::<RetryAfter>("86400").unwrap().get(),
            86_400
        );
        for json in [
            "0",
            "86401",
            "-1",
            "1.0",
            "1.5",
            "\"60\"",
            "null",
            "4294967296",
            "1e2",
            "true",
        ] {
            assert!(serde_json::from_str::<RetryAfter>(json).is_err(), "{json}");
        }
        assert_eq!(
            serde_json::to_string(&RetryAfter::new(60).unwrap()).unwrap(),
            "60"
        );
    }

    #[test]
    fn the_phrase_of_ar706_says_when_the_limit_comes_back() {
        assert_eq!(
            ErrorCode::Ar706.phrase(RetryAfter::new(1)),
            "AR-706: Message limit between Hermes agents reached. It comes back in 1 second(s); \
             insisting before that does not help."
        );
        assert_eq!(
            ErrorCode::Ar706.phrase(RetryAfter::new(86_400)),
            "AR-706: Message limit between Hermes agents reached. It comes back in 86400 \
             second(s); insisting before that does not help."
        );
        assert_eq!(
            ErrorCode::Ar706.phrase(None),
            "AR-706: Message limit between Hermes agents reached. Wait before sending again; \
             insisting does not help."
        );
    }

    #[test]
    fn the_phrase_of_ar704_says_when_the_send_limit_comes_back() {
        assert_eq!(
            ErrorCode::Ar704.phrase(RetryAfter::new(37)),
            "AR-704: Send limit reached. It comes back in 37 second(s); insisting before that \
             does not help."
        );
        assert_eq!(
            ErrorCode::Ar704.phrase(None),
            "AR-704: Send limit reached. Wait before sending again; insisting does not help."
        );
    }

    #[test]
    fn only_the_two_limits_use_the_wait() {
        let wait = RetryAfter::new(1_234);
        for code in ErrorCode::ALL {
            if matches!(*code, ErrorCode::Ar704 | ErrorCode::Ar706) {
                assert!(code.phrase(wait).contains("1234"));
                continue;
            }
            assert_eq!(code.phrase(wait), code.phrase(None), "{code}");
            assert!(!code.phrase(wait).contains("1234"), "{code}");
            assert!(matches!(code.phrase(wait), Cow::Borrowed(_)), "{code}");
        }
    }

    #[test]
    fn connect_failures_have_their_own_phrase_of_ar201() {
        let phrases: HashSet<&str> = ConnectFailure::ALL.iter().map(|c| c.phrase()).collect();
        assert_eq!(phrases.len(), 3);
        assert!(
            ConnectFailure::NameNotResolved
                .phrase()
                .contains("name was not resolved")
        );
        assert!(ConnectFailure::TimedOut.phrase().contains("timed out"));
        assert!(ConnectFailure::Refused.phrase().contains("refused"));
    }
}
