//! One request of a command (`join`, `status`, `leave`) to the leader in execution, through the
//! local socket (`docs/14-protocolo-do-canal.md`, section 11).

use arauto_platform::{LocalSocketError, LocalStream};
use serde_json::{Map, Value};
use tokio::io::{AsyncWriteExt, BufReader};

use crate::home::Home;
use crate::local::{self, Lines};
use crate::settings::duration;

/// Why the leader did not answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AskError {
    /// No leader runs on this folder.
    NoLeader,
    /// The leader did not answer, or answered what is not an answer.
    NoAnswer,
}

/// Sends `request` (with `"v"` added) to the leader of `home` and reads one answer within
/// `patience_ms`.
pub(crate) async fn ask(
    home: &Home,
    mut request: Map<String, Value>,
    patience_ms: i64,
) -> Result<Map<String, Value>, AskError> {
    request.insert("v".to_owned(), Value::from(local::VERSION));
    let exchange = async {
        let stream = match LocalStream::connect(home.root()).await {
            Ok(stream) => stream,
            Err(LocalSocketError::NoListener { .. }) => return Err(AskError::NoLeader),
            Err(_) => return Err(AskError::NoAnswer),
        };
        let (reader, mut writer) = tokio::io::split(stream);
        let line = local::line(&Value::Object(request));
        if writer.write_all(&line).await.is_err() || writer.flush().await.is_err() {
            return Err(AskError::NoAnswer);
        }
        let mut lines = Lines::new(BufReader::new(reader), local::ANSWER_LINE_MAX);
        let answer = lines.next().await.map_err(|_| AskError::NoAnswer)?;
        drop(writer);
        let answer = answer.ok_or(AskError::NoAnswer)?;
        match arauto_protocol::message::read_strict(&answer) {
            Ok(Value::Object(answer)) => Ok(answer),
            _ => Err(AskError::NoAnswer),
        }
    };
    tokio::time::timeout(duration(patience_ms), exchange)
        .await
        .map_err(|_| AskError::NoAnswer)?
}

/// A request with only an operation.
pub(crate) fn operation(op: &str) -> Map<String, Value> {
    let mut request = Map::new();
    request.insert("op".to_owned(), Value::from(op));
    request
}
