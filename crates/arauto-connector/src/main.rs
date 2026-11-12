//! `arauto-connector`: `join`, `mcp`, `status` and `leave` (`docs/11-conector.md`, section 1).
#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::time::Duration;

/// How long the end of the program waits for work on the threads of blocking calls: a write
/// to disk under way ends, but a read of the input, which cannot be cut, is not waited for.
const BLOCKING_PATIENCE: Duration = Duration::from_millis(150);

fn main() -> ExitCode {
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        eprintln!("{}", arauto_connector::catalog::Local::Internal.phrase());
        return ExitCode::from(arauto_connector::cli::Exit::Failure.code());
    };
    let arguments: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let hermes = std::env::var_os("ARAUTO_HERMES_BIN");
    let api_url = std::env::var_os(arauto_connector::wake::URL_VARIABLE);
    let api_key = std::env::var_os(arauto_connector::wake::KEY_VARIABLE);
    let exit = runtime.block_on(arauto_connector::cli::run_waking(
        arguments, hermes, api_url, api_key,
    ));
    runtime.shutdown_timeout(BLOCKING_PATIENCE);
    ExitCode::from(exit.code())
}
