//! The build mark: `X.Y.Z+<sha256 of the running executable>`, declared in `hello.connector`
//! (`docs/14-protocolo-do-canal.md`, section 5.1) so that the hub can tell which file runs.

use std::sync::OnceLock;

use sha2::{Digest, Sha256};

static MARK: OnceLock<String> = OnceLock::new();

/// Computes the mark once. Every process calls it at start, before a `join` can replace the
/// file at the fixed path.
pub(crate) fn init() {
    let _ = declared();
}

/// The declared text: the version and the digest of the running executable, or the version
/// alone when the executable cannot be read.
pub(crate) fn declared() -> &'static str {
    MARK.get_or_init(|| mark_of(env!("CARGO_PKG_VERSION"), digest_of_self()))
}

/// The pure formatter.
fn mark_of(version: &str, digest: Option<[u8; 32]>) -> String {
    match digest {
        Some(digest) => {
            let mut text = String::with_capacity(version.len() + 65);
            text.push_str(version);
            text.push('+');
            for byte in digest {
                text.push_str(&format!("{byte:02x}"));
            }
            text
        }
        None => version.to_owned(),
    }
}

/// On Linux `/proc/self/exe` is the running image even after a rename replaced the path.
fn digest_of_self() -> Option<[u8; 32]> {
    #[cfg(target_os = "linux")]
    let path = std::path::PathBuf::from("/proc/self/exe");
    #[cfg(not(target_os = "linux"))]
    let path = std::env::current_exe().ok()?;
    digest_of(&path)
}

fn digest_of(path: &std::path::Path) -> Option<[u8; 32]> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(buffer.get(..read)?);
    }
    Some(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use arauto_protocol::limits::DECLARED_TEXT_MAX_CHARS;
    use arauto_protocol::message::DeclaredText;

    use super::*;

    #[test]
    fn the_declared_text_is_the_number_and_the_digest_of_this_program() {
        let bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(declared(), mark_of(version, Some(digest)));
        assert_eq!(declared().len(), version.len() + 65);
        assert!(declared().len() <= DECLARED_TEXT_MAX_CHARS);
        assert!(DeclaredText::parse(declared()).is_ok());
        let (core, tail) = declared().split_once('+').unwrap();
        assert_eq!(core, version);
        assert_eq!(core.split('.').count(), 3);
        assert!(core.split('.').all(|part| part.parse::<u32>().is_ok()));
        assert_eq!(tail.len(), 64);
        assert!(
            tail.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert_eq!(mark_of("0.1.0", None), "0.1.0");
    }
}
