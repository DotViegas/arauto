//! The files the hub serves at `GET /v1/download/{file}` (`docs/15-mvp.md`, section 6): the
//! connector of each system, as the image of the hub brings it.
//!
//! The folder named by [`DOWNLOAD_DIR`] is read once, when the hub starts: every regular file
//! whose name fits the narrow format of a download goes into the fixed list, whole, in memory.
//! Nothing a request says ever becomes a path. Only the files whose name starts with
//! `arauto-connector-` are connectors; a file that is too large, a name out of the format and
//! anything that is not a regular file are left out, and said in the log by a fixed
//! word. The SHA-256 of each file is what the block shows: whoever installs checks it before
//! running the program.

use std::fs;
use std::io::{self, Read};
use std::path::Path;

use bytes::Bytes;
use data_encoding::HEXLOWER;
use sha2::{Digest, Sha256};

use crate::channel::Downloads;

/// The variable that names the folder of the downloads. Without it, every download is 404.
pub const DOWNLOAD_DIR: &str = "ARAUTO_DOWNLOAD_DIR";

/// How the name of every file served starts.
pub const CONNECTOR_PREFIX: &str = "arauto-connector-";

/// The largest file served.
pub const FILE_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// The most files served.
pub const FILES_MAX: usize = 16;

/// One file of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// Its name, which is the last part of its address.
    pub name: String,
    /// Its size in bytes.
    pub size: u64,
    /// Its SHA-256, in lowercase hexadecimal.
    pub sha256: String,
}

/// Reads the folder `dir`: the files to serve, and what the block says of each, by name.
///
/// # Errors
/// The folder cannot be listed.
pub fn load(dir: &Path) -> io::Result<(Downloads, Vec<Listed>)> {
    let mut names: Vec<String> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        // Only connectors: a list of digests or a note left in the folder is no program.
        if let Some(name) = entry.file_name().to_str()
            && name.starts_with(CONNECTOR_PREFIX)
        {
            names.push(name.to_owned());
        }
    }
    names.sort();
    let mut downloads = Downloads::empty();
    let mut listed = Vec::new();
    for name in names {
        if listed.len() >= FILES_MAX {
            tracing::warn!("the folder of downloads has more files than the hub serves");
            break;
        }
        let path = dir.join(&name);
        let size = fs::metadata(&path)?.len();
        if size > FILE_MAX_BYTES {
            tracing::warn!("a file of the folder of downloads is too large and is not served");
            continue;
        }
        let mut content = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
        fs::File::open(&path)?
            .take(FILE_MAX_BYTES)
            .read_to_end(&mut content)?;
        let sha256 = HEXLOWER.encode(&Sha256::digest(&content));
        let size = content.len() as u64;
        match downloads.clone().with_file(&name, Bytes::from(content)) {
            Ok(with) => {
                downloads = with;
                listed.push(Listed { name, size, sha256 });
            }
            Err(_) => {
                tracing::warn!("a file of the folder of downloads has a name that is not served");
            }
        }
    }
    Ok((downloads, listed))
}

/// The address of the download `name`, from the address of the channel of the hub
/// (`wss://host/v1/channel` gives `https://host/v1/download/name`).
#[must_use]
pub fn address(channel_url: &str, name: &str) -> Option<String> {
    let (scheme, rest) = if let Some(rest) = channel_url.strip_prefix("wss://") {
        ("https://", rest)
    } else {
        ("http://", channel_url.strip_prefix("ws://")?)
    };
    let host = rest.split('/').next()?;
    Some(format!(
        "{scheme}{host}{}{name}",
        crate::channel::DOWNLOAD_PREFIX
    ))
}

/// The connector a block tells a Hermes to download: the first of the list a Unix shell
/// runs. A program for Windows is downloaded and checked by hand, never by this chain.
#[must_use]
pub fn connector(listed: &[Listed]) -> Option<&Listed> {
    listed.iter().find(|file| !file.name.ends_with(".exe"))
}

/// The address of the instruction page of the handle `handle`, from the address of the
/// channel of the hub (`wss://host/v1/channel` gives `https://host/v1/join/handle`).
#[must_use]
pub fn page_address(channel_url: &str, handle: &str) -> Option<zeroize::Zeroizing<String>> {
    let (scheme, rest) = if let Some(rest) = channel_url.strip_prefix("wss://") {
        ("https://", rest)
    } else {
        ("http://", channel_url.strip_prefix("ws://")?)
    };
    let host = rest.split('/').next()?;
    // The handle is a short-lived secret: the address is built once, at its exact size, in a
    // buffer that is wiped when dropped, so that no copy of the handle is left behind by a
    // growth of the buffer.
    let prefix = crate::join_page::JOIN_PREFIX;
    let mut address = zeroize::Zeroizing::new(String::with_capacity(
        scheme.len() + host.len() + prefix.len() + handle.len(),
    ));
    address.push_str(scheme);
    address.push_str(host);
    address.push_str(prefix);
    address.push_str(handle);
    Some(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_gives_the_files_with_narrow_names_and_their_digest() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("arauto-connector-linux-x86_64"), b"abc").unwrap();
        fs::write(dir.path().join(".hidden"), b"x").unwrap();
        fs::write(dir.path().join("SHA256SUMS"), b"x").unwrap();
        fs::write(dir.path().join("with space"), b"x").unwrap();
        fs::create_dir(dir.path().join("a-folder")).unwrap();
        let (_, listed) = load(dir.path()).unwrap();
        assert_eq!(
            listed,
            [Listed {
                name: "arauto-connector-linux-x86_64".to_owned(),
                size: 3,
                // SHA-256 of "abc".
                sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
                    .to_owned(),
            }]
        );
    }

    #[test]
    fn a_folder_that_is_not_there_is_an_error_and_an_empty_one_serves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(&dir.path().join("missing")).is_err());
        assert!(load(dir.path()).unwrap().1.is_empty());
    }

    #[test]
    fn the_address_of_a_download_follows_the_address_of_the_channel() {
        assert_eq!(
            address("wss://hub.example.org/v1/channel", "arauto-connector").as_deref(),
            Some("https://hub.example.org/v1/download/arauto-connector")
        );
        assert_eq!(
            address("ws://127.0.0.1:8443/v1/channel", "c").as_deref(),
            Some("http://127.0.0.1:8443/v1/download/c")
        );
        assert_eq!(address("https://hub.example.org", "c"), None);
    }
}
