//! A folder that only its owner can use.

use std::fs::{self, DirBuilder, Metadata};
use std::io;
use std::path::Path;

use crate::durable::settle;
use crate::error::FsError;

/// Makes sure `path` is a folder that only its owner can read, write and enter, and that
/// the owner is the user this process runs as.
///
/// Creates the folder when it does not exist; the parent must exist. On Unix the folder is
/// created with mode `0700` and its entry is forced to disk. A folder that already exists
/// is accepted only when:
///
/// - it is a folder itself, not a link to one: a link is refused whatever it leads to;
/// - it belongs to the user this process runs as. That user is found by creating an empty
///   file in the folder and removing it at once, so the process must be able to write in
///   the folder. This is what stops a process that may write anywhere (the administrator,
///   as in many containers) from keeping its secrets in a folder whose owner can replace
///   them;
/// - it is closed to the group and to others, or can be closed: a folder that is open to
///   them is changed to `0700`.
///
/// Between looking at the folder and closing it, somebody who may write in the folder
/// above could still put a link in its place. Keep the folder inside one that only its
/// owner writes in.
///
/// On Windows the folder keeps the permissions inherited from its parent, and an existing
/// folder is accepted as it is, through a link or not: there the user profile is what
/// keeps other users out.
///
/// The entry is forced to disk only by the call that creates the folder. When that step
/// fails ([`FsError::NotDurable`]) the folder exists, and a later call finds it and returns
/// `Ok` without forcing anything: a caller that needs the guarantee removes the folder and
/// starts again.
///
/// # Errors
///
/// [`FsError::NotADirectory`] when `path` exists and is not a folder; on Unix,
/// [`FsError::IsALink`] when it is a link and [`FsError::NotOwner`] when the folder
/// belongs to another user; [`FsError::NotDurable`] when the folder was created and its
/// entry could not be forced to disk; [`FsError::Io`] when the folder cannot be created,
/// when no file can be created in a folder that exists, or when its mode cannot be
/// corrected.
pub fn create_private_dir(path: &Path) -> Result<(), FsError> {
    match create(path) {
        Ok(()) => settle(path),
        // Whatever the system called the failure, an existing folder is the other valid case.
        Err(cause) => match look_at(path) {
            Ok(existing) if existing.is_dir() => adopt(path, &existing),
            // Only where `look_at` does not follow links.
            Ok(existing) if existing.file_type().is_symlink() => Err(FsError::IsALink {
                path: path.to_path_buf(),
            }),
            Ok(_) => Err(FsError::NotADirectory {
                path: path.to_path_buf(),
            }),
            Err(_) => Err(FsError::io("create the folder", path, cause)),
        },
    }
}

#[cfg(unix)]
fn create(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    DirBuilder::new().mode(0o700).create(path)
}

#[cfg(windows)]
fn create(path: &Path) -> io::Result<()> {
    DirBuilder::new().create(path)
}

/// What is under `path` itself: a link is not followed.
#[cfg(unix)]
fn look_at(path: &Path) -> io::Result<Metadata> {
    // Without what the system would take as "the folder this leads to": a link is
    // followed when its name comes with a trailing separator or a trailing `.`.
    let itself: std::path::PathBuf = path.components().collect();
    fs::symlink_metadata(itself)
}

/// What `path` leads to: on Windows a folder reached through a link is taken as it is.
#[cfg(windows)]
fn look_at(path: &Path) -> io::Result<Metadata> {
    fs::metadata(path)
}

/// Accepts a folder that already exists, if it is, or can be made, private to the user
/// this process runs as.
#[cfg(unix)]
fn adopt(path: &Path, existing: &Metadata) -> Result<(), FsError> {
    let user = crate::durable::user_creating_in(path)?;
    close_to_others(path, existing, user)
}

/// On Windows the inherited permissions stand.
#[cfg(windows)]
fn adopt(_path: &Path, _existing: &Metadata) -> Result<(), FsError> {
    Ok(())
}

/// Refuses the folder of another user than `user`, which is the user of this process, and
/// then removes every permission of the group and of others from a folder that has any.
/// The owner is checked first: the administrator could change the mode of any folder.
#[cfg(unix)]
fn close_to_others(path: &Path, existing: &Metadata, user: u32) -> Result<(), FsError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let owner = existing.uid();
    if owner != user {
        return Err(FsError::NotOwner {
            path: path.to_path_buf(),
            owner,
            user,
        });
    }
    if existing.permissions().mode() & 0o077 == 0 {
        return Ok(());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|cause| FsError::io("close to other users the folder", path, cause))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::durable::probe::{self, Step};

    #[test]
    fn creates_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("home");

        create_private_dir(&path).unwrap();

        assert!(path.is_dir());
    }

    /// A new folder survives a power loss only when the folder that holds it is forced to
    /// disk after the creation.
    #[test]
    fn entry_of_a_new_folder_is_forced_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("home");
        // Each step, with the path it acts on and whether the new folder already exists.
        let steps = Rc::new(RefCell::new(Vec::new()));
        let watching = probe::watch({
            let (steps, path) = (steps.clone(), path.clone());
            move |step, at| {
                steps
                    .borrow_mut()
                    .push((step, at.to_path_buf(), path.is_dir()));
                Ok(())
            }
        });

        create_private_dir(&path).unwrap();
        drop(watching);

        assert_eq!(
            *steps.borrow(),
            [(Step::SyncDir, dir.path().to_path_buf(), true)]
        );
    }

    #[test]
    fn failure_to_force_the_entry_says_that_the_folder_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("home");
        let _watching = probe::fail_at(Step::SyncDir);

        let error = create_private_dir(&path).unwrap_err();

        assert!(
            matches!(&error, FsError::NotDurable { path: created, .. } if *created == path),
            "{error}"
        );
        assert!(path.is_dir());
    }

    #[test]
    fn existing_folder_is_accepted_and_keeps_its_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("home");
        create_private_dir(&path).unwrap();
        fs::write(path.join("hub.json"), b"{}").unwrap();

        create_private_dir(&path).unwrap();

        assert_eq!(fs::read(path.join("hub.json")).unwrap(), b"{}");
    }

    #[test]
    fn missing_parent_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("home");

        let error = create_private_dir(&path).unwrap_err();

        assert!(matches!(error, FsError::Io { .. }), "{error}");
        assert!(!dir.path().join("missing").exists());
    }

    #[test]
    fn a_file_in_the_way_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("home");
        fs::write(&path, b"not a folder").unwrap();

        let error = create_private_dir(&path).unwrap_err();

        assert!(matches!(error, FsError::NotADirectory { .. }), "{error}");
        assert_eq!(fs::read(&path).unwrap(), b"not a folder");
    }

    #[cfg(unix)]
    mod unix {
        use std::os::unix::fs::PermissionsExt;

        use super::*;

        fn mode_of(path: &Path) -> u32 {
            fs::metadata(path).unwrap().permissions().mode() & 0o7777
        }

        #[test]
        fn new_folder_is_for_the_owner_only() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("home");

            create_private_dir(&path).unwrap();

            assert_eq!(mode_of(&path), 0o700);
        }

        #[test]
        fn folder_open_to_others_is_closed() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("home");
            fs::create_dir(&path).unwrap();
            for open in [0o755, 0o750, 0o707, 0o1777] {
                fs::set_permissions(&path, fs::Permissions::from_mode(open)).unwrap();

                create_private_dir(&path).unwrap();

                assert_eq!(mode_of(&path), 0o700, "started as {open:o}");
            }
        }

        #[test]
        fn folder_already_closed_is_left_as_it_is() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("home");
            fs::create_dir(&path).unwrap();
            // Closed to the others, and still letting its owner create files in it.
            fs::set_permissions(&path, fs::Permissions::from_mode(0o300)).unwrap();

            create_private_dir(&path).unwrap();

            assert_eq!(mode_of(&path), 0o300);
            // Lets the temporary folder be removed.
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }

        #[test]
        fn existing_folder_of_the_user_is_accepted_and_nothing_is_left_in_it() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("home");
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();

            create_private_dir(&path).unwrap();

            assert_eq!(mode_of(&path), 0o700);
            assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
        }

        /// A folder that belongs to somebody else is not private to this process, however
        /// closed it is: its owner can replace what is put in it. It is refused before its
        /// mode is touched, which matters for a process that may change any folder.
        #[test]
        fn folder_of_another_user_is_refused_and_left_as_it_was() {
            use std::os::unix::fs::MetadataExt;

            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("home");
            fs::create_dir(&path).unwrap();
            for mode in [0o700, 0o755] {
                fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
                let existing = fs::symlink_metadata(&path).unwrap();
                let owner = existing.uid();
                // As if this process ran as somebody else: the folder becomes a stranger's.
                let somebody_else = owner.wrapping_add(1);

                let refused = close_to_others(&path, &existing, somebody_else);

                assert!(
                    matches!(
                        &refused,
                        Err(FsError::NotOwner { path: folder, owner: found, user })
                            if *folder == path && *found == owner && *user == somebody_else
                    ),
                    "{refused:?}"
                );
                assert_eq!(
                    mode_of(&path),
                    mode,
                    "the folder of another user was changed"
                );

                close_to_others(&path, &existing, owner).unwrap();
                assert_eq!(mode_of(&path), 0o700);
            }
        }

        /// The user is not taken on trust: a folder in which the process cannot create the
        /// file that tells who it is, is not accepted.
        #[test]
        fn folder_where_nothing_can_be_created_is_not_accepted() {
            use std::os::unix::fs::MetadataExt;

            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("home");
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o500)).unwrap();

            let outcome = create_private_dir(&path);

            // The administrator creates files anywhere: for it the folder is simply its own.
            if fs::metadata(&path).unwrap().uid() == 0 {
                outcome.unwrap();
            } else {
                assert!(matches!(outcome, Err(FsError::Io { .. })), "{outcome:?}");
            }
            assert_eq!(mode_of(&path), 0o500);
            // Lets the temporary folder be removed.
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }

        /// A link is refused whatever it leads to, and what it leads to is not touched:
        /// closing the folder would follow the link.
        #[test]
        fn link_in_the_place_of_the_folder_is_refused() {
            let dir = tempfile::tempdir().unwrap();
            let elsewhere = dir.path().join("elsewhere");
            fs::create_dir(&elsewhere).unwrap();
            fs::set_permissions(&elsewhere, fs::Permissions::from_mode(0o755)).unwrap();
            let to_a_folder = dir.path().join("home");
            let to_nothing = dir.path().join("dangling");
            std::os::unix::fs::symlink(&elsewhere, &to_a_folder).unwrap();
            std::os::unix::fs::symlink(dir.path().join("missing"), &to_nothing).unwrap();

            // The spellings that make the system follow a link are refused as well.
            let spellings = [
                to_a_folder.clone(),
                to_a_folder.join(""),
                to_a_folder.join("."),
                to_nothing.clone(),
            ];
            assert!(spellings[1].as_os_str().as_encoded_bytes().ends_with(b"/"));
            for link in &spellings {
                let refused = create_private_dir(link);

                assert!(
                    matches!(&refused, Err(FsError::IsALink { path }) if path == link),
                    "{}: {refused:?}",
                    link.display()
                );
            }
            assert_eq!(mode_of(&elsewhere), 0o755, "the link was followed");
            assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
            assert!(!dir.path().join("missing").exists());
        }
    }
}
