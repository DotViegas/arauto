//! The inbox of events, on the internal port (`docs/15-mvp.md`, section 6): the address the
//! engine calls, `POST /v1/engine/{engine}/events`. It does not exist on the port of the
//! connectors.
//!
//! In this order, and nothing is written before the last check:
//!
//! 1. the size: a declared length above the ceiling is refused at once, and the body is read
//!    with the ceiling and a deadline;
//! 2. the type of content, `application/json`;
//! 3. the signature: HMAC-SHA-512 of the body with the secret of the events of that engine, in
//!    `X-Webhook-Hmac`, compared in constant time. An engine nobody configured and a wrong
//!    signature get the same answer;
//! 4. the shape: one JSON object, with the identifier, the time, the kind of event and the
//!    session, each in its format, the time within the window, the kind one of those the hub
//!    configured, and the session one of a number of that engine;
//! 5. the row, once: the same event twice is one row (`repo::engine_event::record`). The
//!    status of a session is applied to its number in the same transaction.
//!
//! The answer is a status and no body. The log says why an event was refused with a fixed
//! word, never a value of it.

use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderMap, Response, StatusCode, header};
use http_body_util::{BodyExt, Limited};
use hyper::body::Incoming;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::config::{EVENTS_PATH_PREFIX, EVENTS_PATH_SUFFIX};
use super::types::{EngineChatId, EngineMessageId, EngineStatus, SessionName};
use crate::channel::Hub;
use crate::db::{self, ResolvedOrg};
use crate::repo::engine_event::{self, EventKind, NewEvent, Source};
use crate::repo::number::{self, Observed};

/// The header of the signature of an event.
pub const SIGNATURE_HEADER: &str = "x-webhook-hmac";

/// The header that names the algorithm of the signature.
pub const ALGORITHM_HEADER: &str = "x-webhook-hmac-algorithm";

/// The engine of an address of the inbox, `/v1/engine/{engine}/events`.
pub(crate) fn engine_of_path(path: &str) -> Option<Uuid> {
    let middle = path
        .strip_prefix(EVENTS_PATH_PREFIX)?
        .strip_suffix(EVENTS_PATH_SUFFIX)?;
    // Only the hyphenated form, exactly: one way to write each address.
    let id = Uuid::try_parse(middle).ok()?;
    (id.as_hyphenated().to_string() == middle).then_some(id)
}

fn status(code: StatusCode) -> Response<Body> {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = code;
    response
}

fn refused(code: StatusCode, why: &'static str) -> Response<Body> {
    tracing::warn!(
        why,
        status = code.as_u16(),
        "an event of the engine was refused"
    );
    status(code)
}

#[derive(Deserialize)]
struct Envelope {
    id: String,
    timestamp: i64,
    event: String,
    session: String,
    #[serde(default)]
    me: Option<Value>,
    payload: Value,
}

/// The identifier of an event of the engine: `evt_` and up to 64 letters and digits.
fn event_id_ok(id: &str) -> bool {
    id.strip_prefix("evt_").is_some_and(|rest| {
        !rest.is_empty() && rest.len() <= 64 && rest.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

/// Receives one event.
pub(crate) async fn receive(
    hub: &Hub,
    engine: Uuid,
    headers: &HeaderMap,
    body: Incoming,
) -> Response<Body> {
    let settings = hub.engines().settings();
    let max = settings.event_body_max;
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    if declared.is_some_and(|size| size > max as u64) {
        return refused(StatusCode::PAYLOAD_TOO_LARGE, "too large");
    }
    let is_json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|essence| essence.trim().eq_ignore_ascii_case("application/json"));
    if !is_json {
        return refused(StatusCode::UNSUPPORTED_MEDIA_TYPE, "not json");
    }
    let read = Duration::from_millis(u64::try_from(settings.event_read_ms).unwrap_or(0));
    let body = match tokio::time::timeout(read, Limited::new(body, max).collect()).await {
        Ok(Ok(collected)) => collected.to_bytes(),
        Ok(Err(error)) if error.is::<http_body_util::LengthLimitError>() => {
            return refused(StatusCode::PAYLOAD_TOO_LARGE, "too large");
        }
        Ok(Err(_)) => return refused(StatusCode::BAD_REQUEST, "body not read"),
        Err(_) => return refused(StatusCode::REQUEST_TIMEOUT, "body too slow"),
    };

    let signature = headers
        .get(SIGNATURE_HEADER)
        .and_then(|value| value.to_str().ok());
    let algorithm_ok = headers
        .get(ALGORITHM_HEADER)
        .is_none_or(|value| value.as_bytes().eq_ignore_ascii_case(b"sha512"));
    let Some(signature) = signature.filter(|_| algorithm_ok) else {
        return refused(StatusCode::UNAUTHORIZED, "not signed");
    };
    let gate = match db::organization_of_engine_events(hub.pool(), engine).await {
        Ok(gate) => gate,
        Err(_) => return refused(StatusCode::SERVICE_UNAVAILABLE, "database"),
    };
    let Some((organization, engine)) =
        gate.and_then(|gate| gate.verify(hub.vault(), &body, signature))
    else {
        return refused(StatusCode::UNAUTHORIZED, "signature");
    };

    let Ok(envelope) = serde_json::from_slice::<Envelope>(&body) else {
        return refused(StatusCode::BAD_REQUEST, "malformed");
    };
    if !event_id_ok(&envelope.id) || !envelope.payload.is_object() {
        return refused(StatusCode::BAD_REQUEST, "malformed");
    }
    let now = hub.now_ms();
    if envelope.timestamp.abs_diff(now) > settings.event_window_ms.unsigned_abs() {
        return refused(StatusCode::BAD_REQUEST, "outside the window");
    }
    let Some(kind) = EventKind::parse(&envelope.event) else {
        return refused(StatusCode::BAD_REQUEST, "kind");
    };
    let Ok(session) = SessionName::parse(&envelope.session) else {
        return refused(StatusCode::BAD_REQUEST, "session");
    };
    match store(
        hub,
        organization,
        engine,
        &session,
        kind,
        &envelope,
        &body,
        now,
    )
    .await
    {
        Ok(Stored::Recorded) => status(StatusCode::OK),
        Ok(Stored::NoSuchSession) => refused(StatusCode::BAD_REQUEST, "session"),
        Ok(Stored::Malformed) => refused(StatusCode::BAD_REQUEST, "malformed"),
        Err(()) => refused(StatusCode::SERVICE_UNAVAILABLE, "database"),
    }
}

enum Stored {
    Recorded,
    NoSuchSession,
    Malformed,
}

/// What makes an event one: the message, for a message (the same message from the inbox and
/// from the catch-up is one row); the event of the engine, for the others.
fn event_key(kind: EventKind, envelope: &Envelope) -> Option<Vec<u8>> {
    match kind {
        EventKind::Message => {
            let id = message_id(&envelope.payload)?;
            Some(format!("message:{}", id.as_str()).into_bytes())
        }
        _ => Some(format!("event:{}", envelope.id).into_bytes()),
    }
}

fn message_id(payload: &Value) -> Option<EngineMessageId> {
    let id = match payload.get("id")? {
        Value::String(text) => text.clone(),
        Value::Object(object) => object.get("_serialized")?.as_str()?.to_owned(),
        _ => return None,
    };
    EngineMessageId::parse(&id).ok()
}

#[allow(clippy::too_many_arguments)]
async fn store(
    hub: &Hub,
    organization: ResolvedOrg,
    engine: Uuid,
    session: &SessionName,
    kind: EventKind,
    envelope: &Envelope,
    body: &[u8],
    now: i64,
) -> Result<Stored, ()> {
    let Some(key_text) = event_key(kind, envelope) else {
        return Ok(Stored::Malformed);
    };
    let observed = if kind == EventKind::SessionStatus {
        let Some(status) = envelope
            .payload
            .get("status")
            .and_then(Value::as_str)
            .and_then(EngineStatus::parse)
        else {
            return Ok(Stored::Malformed);
        };
        let phone = envelope
            .me
            .as_ref()
            .and_then(|me| me.get("id"))
            .and_then(Value::as_str)
            .and_then(|id| EngineChatId::parse(id).ok())
            .and_then(|id| id.phone());
        Some(Observed {
            status,
            phone,
            at: Some(envelope.timestamp),
        })
    } else {
        None
    };
    let engine_at = if kind == EventKind::Message {
        envelope
            .payload
            .get("timestamp")
            .and_then(Value::as_i64)
            .and_then(|seconds| seconds.checked_mul(1_000))
            .filter(|at| *at >= 0)
            .unwrap_or(envelope.timestamp)
    } else {
        envelope.timestamp
    };
    let key = hub.organization_key(organization).await.map_err(|_| ())?;
    let mut transaction = hub.pool().begin().await.map_err(|_| ())?;
    let Some(number) = number::by_session(&mut transaction, organization, engine, session)
        .await
        .map_err(|_| ())?
    else {
        return Ok(Stored::NoSuchSession);
    };
    let event = NewEvent {
        engine,
        number: number.id,
        kind,
        key: &key_text,
        body,
        engine_at,
        source: Source::Webhook,
    };
    let recorded = engine_event::record(&mut transaction, organization, &key, &event, now)
        .await
        .map_err(|_| ())?;
    if let Some(row) = recorded {
        if let Some(observed) = &observed {
            let changed = number::observe(
                &mut transaction,
                organization,
                &key,
                number.id,
                observed,
                now,
            )
            .await
            .map_err(|_| ())?;
            if let Some((from, to)) = changed {
                tracing::info!(
                    number = %number.id,
                    from = from.as_str(),
                    to = to.as_str(),
                    "a number changed state"
                );
            }
            engine_event::handled(&mut transaction, organization, row, now)
                .await
                .map_err(|_| ())?;
        } else if kind == EventKind::Message {
            number::advance_messages(&mut transaction, organization, number.id, engine_at)
                .await
                .map_err(|_| ())?;
        }
    }
    transaction.commit().await.map_err(|_| ())?;
    Ok(Stored::Recorded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_of_the_inbox_names_one_engine_one_way() {
        let id = Uuid::from_u128(0xabc);
        let path = format!("/v1/engine/{}/events", id.as_hyphenated());
        assert_eq!(engine_of_path(&path), Some(id));
        for wrong in [
            "/v1/engine//events".to_owned(),
            "/v1/engine/x/events".to_owned(),
            format!("/v1/engine/{}/events/", id.as_hyphenated()),
            format!("/v1/engine/{}/events", id.simple()),
            format!(
                "/v1/engine/{}/events",
                id.as_hyphenated().to_string().to_uppercase()
            ),
            format!("/v1/engine/../{}/events", id.as_hyphenated()),
            format!("/v1/engines/{}/events", id.as_hyphenated()),
        ] {
            assert_eq!(engine_of_path(&wrong), None, "{wrong}");
        }
    }

    #[test]
    fn the_identifier_of_an_event_is_narrow() {
        assert!(event_id_ok("evt_01aryz6s41tsv4rrffq69g5fav"));
        for wrong in [
            "",
            "evt_",
            "01aryz",
            "evt_../x",
            "evt_a b",
            &format!("evt_{}", "a".repeat(65)),
        ] {
            assert!(!event_id_ok(wrong), "{wrong:?}");
        }
    }
}
