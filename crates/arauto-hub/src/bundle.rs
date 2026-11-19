//! The connectors this hub was built with (decision 0053, proposal, 19/11/2026).
//!
//! The image builds the connectors first and then compiles their build number and their
//! SHA-256 digests into the hub, through two environment variables of the build. At start the
//! hub compares the files it serves with those digests: only when every one matches is the
//! bundle "known", and only then do messages carry version numbers and the panel explain an
//! outdated connector. A development build, another download folder or any difference leaves
//! the bundle unknown. The number is a build number, not a published signed version.

use crate::core::reconnect::CoreVersion;
use crate::download::{self, Listed};

// `option_env!` takes a literal, not a constant, so the names are written once, in these two
// macros: the constants and `compiled` both come from them, and a test ties `build.rs` to them.
macro_rules! version_variable {
    () => {
        "ARAUTO_BUNDLE_CONNECTOR_VERSION"
    };
}
macro_rules! digests_variable {
    () => {
        "ARAUTO_BUNDLE_CONNECTOR_SHA256"
    };
}

/// The variable of the build that holds the build number of the connectors.
pub const VERSION_VARIABLE: &str = version_variable!();

/// The variable of the build that holds `name=sha256` of each connector file, separated by
/// commas.
pub const DIGESTS_VARIABLE: &str = digests_variable!();

/// The connectors the hub was built with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    /// The build number of the connectors.
    pub number: CoreVersion,
    /// The file name and the lowercase hexadecimal SHA-256 of each connector.
    pub files: Vec<(String, String)>,
}

/// Reads the two values of the build. Anything out of the format reads as no bundle: the
/// number is `X.Y.Z`; the digests are one to four `name=hex` separated by commas, each name one
/// of [`download::SYSTEMS`] at most once and each digest exactly 64 of `0-9a-f`.
#[must_use]
pub fn parse(version: Option<&str>, digests: Option<&str>) -> Option<Bundle> {
    let number = CoreVersion::parse_core(version?)?;
    let mut files: Vec<(String, String)> = Vec::new();
    for entry in digests?.split(',') {
        let (name, hex) = entry.split_once('=')?;
        let named = download::SYSTEMS.iter().any(|(file, _)| *file == name);
        let hex_ok = hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if !named || !hex_ok || files.iter().any(|(seen, _)| seen == name) {
            return None;
        }
        files.push((name.to_owned(), hex.to_owned()));
    }
    (!files.is_empty() && files.len() <= download::SYSTEMS.len())
        .then_some(Bundle { number, files })
}

/// The bundle compiled into this hub, when the build gave it.
#[must_use]
pub fn compiled() -> Option<Bundle> {
    parse(
        option_env!(version_variable!()),
        option_env!(digests_variable!()),
    )
}

/// The number of the bundle, only when the connectors served are exactly its files: the same
/// names, the same digests and the same count.
#[must_use]
pub fn known(bundle: Option<&Bundle>, listed: &[Listed]) -> Option<CoreVersion> {
    let bundle = bundle?;
    let mut served: Vec<(&str, &str)> = download::connectors(listed)
        .into_iter()
        .map(|(_, file)| (file.name.as_str(), file.sha256.as_str()))
        .collect();
    let mut built: Vec<(&str, &str)> = bundle
        .files
        .iter()
        .map(|(name, hex)| (name.as_str(), hex.as_str()))
        .collect();
    served.sort_unstable();
    built.sort_unstable();
    (served == built).then_some(bundle.number)
}

/// The number of the bundle for the files in `listed`, with the bundle compiled into this hub.
#[must_use]
pub fn known_for(listed: &[Listed]) -> Option<CoreVersion> {
    known(compiled().as_ref(), listed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINUX: &str = "arauto-connector-linux-x86_64";
    const WINDOWS: &str = "arauto-connector-windows-x86_64.exe";

    fn hex(c: char) -> String {
        c.to_string().repeat(64)
    }

    fn listed(name: &str, c: char) -> Listed {
        Listed {
            name: name.to_owned(),
            size: 1,
            sha256: hex(c),
        }
    }

    #[test]
    fn the_build_script_watches_the_variables_the_hub_reads() {
        let script = include_str!("../build.rs");
        for variable in [VERSION_VARIABLE, DIGESTS_VARIABLE] {
            assert!(script.contains(&format!("cargo:rerun-if-env-changed={variable}")));
        }
    }

    #[test]
    fn a_bundle_is_read_only_in_its_format() {
        let good = format!("{LINUX}={},{WINDOWS}={}", hex('a'), hex('b'));
        assert!(parse(None, Some(&good)).is_none());
        assert!(parse(Some("0.1.1"), None).is_none());
        assert!(parse(Some("0.1"), Some(&good)).is_none());
        let outside = format!("arauto-connector-other={}", hex('a'));
        assert!(parse(Some("0.1.1"), Some(&outside)).is_none());
        let repeated = format!("{LINUX}={},{LINUX}={}", hex('a'), hex('b'));
        assert!(parse(Some("0.1.1"), Some(&repeated)).is_none());
        let short = format!("{LINUX}={}", "a".repeat(63));
        assert!(parse(Some("0.1.1"), Some(&short)).is_none());
        let upper = format!("{LINUX}={}", "A".repeat(64));
        assert!(parse(Some("0.1.1"), Some(&upper)).is_none());
        let five = format!(
            "{LINUX}={0},{WINDOWS}={0},arauto-connector-macos-aarch64={0},\
             arauto-connector-macos-x86_64={0},{LINUX}={0}",
            hex('a')
        );
        assert!(parse(Some("0.1.1"), Some(&five)).is_none());
        assert!(parse(Some("0.1.1"), Some("")).is_none());
        let bundle = parse(Some("0.1.1"), Some(&good)).unwrap();
        assert_eq!(bundle.number.to_string(), "0.1.1");
        assert_eq!(
            bundle.files,
            vec![(LINUX.to_owned(), hex('a')), (WINDOWS.to_owned(), hex('b'))]
        );
    }

    #[test]
    fn the_bundle_is_known_only_when_the_served_files_are_its_files() {
        let text = format!("{LINUX}={},{WINDOWS}={}", hex('a'), hex('b'));
        let bundle = parse(Some("0.1.1"), Some(&text));
        let same = [listed(LINUX, 'a'), listed(WINDOWS, 'b')];
        assert_eq!(
            known(bundle.as_ref(), &same).map(|n| n.to_string()),
            Some("0.1.1".to_owned())
        );
        let different = [listed(LINUX, 'a'), listed(WINDOWS, 'c')];
        assert!(known(bundle.as_ref(), &different).is_none());
        assert!(known(bundle.as_ref(), &[listed(LINUX, 'a')]).is_none());
        let extra = [
            listed(LINUX, 'a'),
            listed(WINDOWS, 'b'),
            listed("arauto-connector-macos-aarch64", 'd'),
        ];
        assert!(known(bundle.as_ref(), &extra).is_none());
        assert!(known(None, &same).is_none());
    }
}
