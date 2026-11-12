//! The HTTP/1.1 client of the hub towards the engine, over `hyper` (`docs/15-mvp.md`,
//! section 4, with the client of the library the hub already serves with).
//!
//! The answer of the engine is hostile input. Every call has a deadline for the connection
//! and one for the whole call; the size of the body is checked against the ceiling of the call
//! before it is read, and it is read with that ceiling; the type of content is checked; and
//! nothing of the answer is put in an error. One connection per call: the engine sits on the
//! private network, and a connection that is never shared cannot hold another number.

use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE, HOST, HeaderValue};
use hyper::{Method, Request, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

use super::config::{ApiKey, EngineAddress};
use super::error::{Cause, EngineError};

/// The header of the key of the API of the engine.
pub const API_KEY_HEADER: &str = "x-api-key";

/// Ceiling of an ordinary answer in JSON.
pub const JSON_ANSWER_MAX: usize = 4 * 1024 * 1024;

/// What an answer is expected to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Expect {
    /// JSON, up to this many bytes.
    Json(usize),
    /// Any type, up to this many bytes (the content of a file).
    Bytes(usize),
}

/// One call.
#[derive(Debug)]
pub(crate) struct Call {
    pub(crate) method: Method,
    /// The path, already put together from checked segments.
    pub(crate) path: String,
    /// The query, each value encoded here.
    pub(crate) query: Vec<(&'static str, String)>,
    /// A body in JSON.
    pub(crate) body: Option<Vec<u8>>,
    pub(crate) expect: Expect,
}

impl Call {
    pub(crate) fn new(method: Method, path: String, expect: Expect) -> Self {
        Self {
            method,
            path,
            query: Vec::new(),
            body: None,
            expect,
        }
    }

    pub(crate) fn query(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.query.push((key, value.into()));
        self
    }

    pub(crate) fn json(mut self, body: &serde_json::Value) -> Self {
        self.body = Some(body.to_string().into_bytes());
        self
    }

    fn target(&self) -> String {
        let mut target = self.path.clone();
        for (index, (key, value)) in self.query.iter().enumerate() {
            target.push(if index == 0 { '?' } else { '&' });
            target.push_str(key);
            target.push('=');
            target.push_str(&encode_query(value));
        }
        target
    }
}

/// Percent-encodes everything but the unreserved characters.
pub(crate) fn encode_query(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// What came back.
#[derive(Debug)]
pub(crate) struct Answer {
    pub(crate) status: StatusCode,
    pub(crate) body: Bytes,
}

/// The deadlines of the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadlines {
    /// To open the connection.
    pub connect: Duration,
    /// For the whole call, from the connection to the last byte of the answer.
    pub call: Duration,
}

impl Default for Deadlines {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(3),
            call: Duration::from_secs(15),
        }
    }
}

/// The client of one engine.
#[derive(Debug, Clone)]
pub(crate) struct HttpClient {
    address: EngineAddress,
    key: Option<ApiKey>,
    deadlines: Deadlines,
}

impl HttpClient {
    pub(crate) fn new(address: EngineAddress, key: Option<ApiKey>, deadlines: Deadlines) -> Self {
        Self {
            address,
            key,
            deadlines,
        }
    }

    /// Sends `call` and returns the status and the body, within the deadlines and the ceiling.
    /// A 5xx is an error here; every other status goes back to the adapter.
    pub(crate) async fn send(&self, call: Call) -> Result<Answer, EngineError> {
        match tokio::time::timeout(self.deadlines.call, self.exchange(call)).await {
            Ok(answer) => answer,
            Err(_) => Err(EngineError::Unavailable(Cause::Timeout)),
        }
    }

    async fn exchange(&self, call: Call) -> Result<Answer, EngineError> {
        let http = self.address.http();
        let connect = TcpStream::connect((http.host(), http.port()));
        let stream = match tokio::time::timeout(self.deadlines.connect, connect).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(_)) => return Err(EngineError::Unavailable(Cause::Connect)),
            Err(_) => return Err(EngineError::Unavailable(Cause::Timeout)),
        };
        let _ = stream.set_nodelay(true);
        let (mut sender, connection) =
            hyper::client::conn::http1::handshake::<_, Full<Bytes>>(TokioIo::new(stream))
                .await
                .map_err(|_| EngineError::Unavailable(Cause::Connect))?;
        // The connection is driven beside the call and ends with it: dropping the guard
        // aborts it, whatever way the call ends.
        let driver = AbortOnDrop(tokio::spawn(async move {
            let _ = connection.await;
        }));

        let mut request = Request::builder()
            .method(call.method.clone())
            .uri(call.target())
            .header(HOST, http.authority());
        request = match call.expect {
            Expect::Json(_) => request.header(ACCEPT, "application/json"),
            Expect::Bytes(_) => request.header(ACCEPT, "*/*"),
        };
        if let Some(key) = &self.key {
            let mut value =
                HeaderValue::from_str(key.expose()).map_err(|_| EngineError::Refused)?;
            value.set_sensitive(true);
            request = request.header(API_KEY_HEADER, value);
        }
        let body = match call.body {
            Some(body) => {
                request = request.header(CONTENT_TYPE, "application/json");
                Full::new(Bytes::from(body))
            }
            None => Full::new(Bytes::new()),
        };
        let request = request
            .body(body)
            .map_err(|_| EngineError::Invalid("request"))?;
        let response = sender
            .send_request(request)
            .await
            .map_err(|_| EngineError::Unavailable(Cause::Connect))?;

        let status = response.status();
        if status.is_server_error() {
            return Err(EngineError::Unavailable(Cause::Status(status.as_u16())));
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(EngineError::Unavailable(Cause::KeyRefused));
        }
        let ceiling = match call.expect {
            Expect::Json(max) | Expect::Bytes(max) => max,
        };
        // Before reading: a declared size above the ceiling is refused at once.
        let declared = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());
        if declared.is_some_and(|size| size > ceiling as u64) {
            return Err(EngineError::Unavailable(Cause::TooLarge));
        }
        let is_json = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|essence| essence.trim().eq_ignore_ascii_case("application/json"))
            });
        let body = Limited::new(response.into_body(), ceiling)
            .collect()
            .await
            .map_err(|error| {
                if error.is::<http_body_util::LengthLimitError>() {
                    EngineError::Unavailable(Cause::TooLarge)
                } else {
                    EngineError::Unavailable(Cause::Connect)
                }
            })?
            .to_bytes();
        drop(driver);
        // A JSON answer of success says it is JSON; an error may come with any type, and
        // its body is not read by the adapter.
        if matches!(call.expect, Expect::Json(_))
            && status.is_success()
            && !body.is_empty()
            && !is_json
        {
            return Err(EngineError::Unavailable(Cause::ContentType));
        }
        Ok(Answer { status, body })
    }
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_value_is_encoded_whole() {
        assert_eq!(encode_query("5511@c.us"), "5511%40c.us");
        assert_eq!(encode_query("a&b=c/../d"), "a%26b%3Dc%2F..%2Fd");
        assert_eq!(encode_query("%2e"), "%252e");
        let call = Call::new(Method::GET, "/api/x".to_owned(), Expect::Json(10))
            .query("limit", "5")
            .query("contactId", "1@c.us");
        assert_eq!(call.target(), "/api/x?limit=5&contactId=1%40c.us");
    }
}
