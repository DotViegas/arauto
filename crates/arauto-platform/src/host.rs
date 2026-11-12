//! The name of this machine, as a device declares it.

/// The longest name kept, in bytes: a host name has at most 253.
const NAME_MAX: usize = 255;

/// The name of this machine, for a device to declare: the first line of `/etc/hostname` on
/// Unix, and nothing on Windows, a platform of development only. Nobody trusts it: the hub
/// shows it as declared. Empty when it cannot be read; the file is never read past
/// [`NAME_MAX`] bytes.
#[must_use]
pub fn host_name() -> String {
    first_line(&imp::name_bytes())
}

/// The first line of `bytes`, without the spaces around it, cut at [`NAME_MAX`] bytes.
fn first_line(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes.get(..NAME_MAX).unwrap_or(bytes));
    text.lines().next().unwrap_or_default().trim().to_owned()
}

#[cfg(unix)]
mod imp {
    use std::io::Read;

    use super::NAME_MAX;

    pub(super) fn name_bytes() -> Vec<u8> {
        let Ok(file) = std::fs::File::open("/etc/hostname") else {
            return Vec::new();
        };
        let mut bytes = Vec::with_capacity(NAME_MAX);
        let limit = u64::try_from(NAME_MAX).unwrap_or(u64::MAX);
        match file.take(limit).read_to_end(&mut bytes) {
            Ok(_) => bytes,
            Err(_) => Vec::new(),
        }
    }
}

#[cfg(windows)]
mod imp {
    /// Nothing: on a platform of development, the name is not worth reading the environment.
    pub(super) fn name_bytes() -> Vec<u8> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_line_is_kept_and_never_more_than_the_ceiling() {
        assert_eq!(first_line(b"  machine-1 \nsecond line\n"), "machine-1");
        assert_eq!(first_line(b""), "");
        assert_eq!(first_line(&[b'a'; 4096]).len(), NAME_MAX);
        assert_eq!(first_line(b"bad \xff byte"), "bad \u{fffd} byte");
    }

    #[test]
    fn the_name_of_this_machine_is_one_short_line() {
        let name = host_name();
        assert!(name.len() <= NAME_MAX * 3, "{name:?}");
        assert!(!name.contains('\n'), "{name:?}");
        #[cfg(windows)]
        assert!(name.is_empty());
    }
}
