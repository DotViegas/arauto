//! Durable replacement of a file.
//!
//! Four steps, in this order: the new content is written to a temporary file in the same
//! folder; that file is forced to disk; it is renamed over the final name; the folder is
//! forced to disk. The final name therefore always holds a whole file, the old one or the
//! new one, and the new one is only called durable when the rename itself is on disk.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use data_encoding::HEXLOWER;

use crate::error::FsError;

/// How many temporary names are drawn before giving up. One is enough unless somebody
/// creates the drawn names on purpose.
const TEMPORARY_NAME_ATTEMPTS: usize = 8;

/// How many random bytes go into a name created next to a target. The name carries them as
/// twice as many lowercase hexadecimal digits.
const RANDOM_SUFFIX_BYTES: usize = 8;

/// What the name of every temporary file ends with.
const TEMPORARY_EXTENSION: &str = ".tmp";

/// Replaces the file at `path` with `contents`, and returns `Ok` only after the new content
/// is safe on disk.
///
/// What an error means depends on how far the call went:
///
/// - Before the rename (every error but [`FsError::NotDurable`]): the previous file, or no
///   file, is still under `path`, never a partial one, and the temporary file is removed.
/// - After the rename ([`FsError::NotDurable`]): the **new** content is under `path`, but
///   the folder could not be forced to disk, so a power loss may bring the previous file
///   back. Do not report the write as done, and decide what is current by reading the
///   file, not by what is in memory.
///
/// A process that dies in the middle leaves the temporary file behind; see
/// [`remove_stale_temporaries`].
///
/// On Unix the new file is created with mode `0600` and the folder is forced to disk after
/// the rename. On Windows the folder is not forced to disk: a power loss right after the
/// call may bring the old file back, and [`FsError::NotDurable`] never happens.
///
/// # Errors
///
/// [`FsError::NotAFilePath`] when `path` has no file name; [`FsError::NotDurable`] as
/// described above; [`FsError::Io`] when the folder does not exist or any earlier step
/// fails.
pub fn write_durable(path: &Path, contents: &[u8]) -> Result<(), FsError> {
    stage(path, Access::Private, |file| file.write_all(contents))?.install(path)
}

/// Removes from `dir` the temporary files that interrupted writes left behind, and returns
/// how many were removed.
///
/// [`write_durable`] and [`replace_executable`](crate::replace_executable) remove their
/// temporary file when they fail, but not when the process dies in the middle, nor, on
/// Windows, while another program holds that file open. What stays is a file named
/// `.{name}.{16 hexadecimal digits}.tmp` with a copy of what was being written: for a
/// credential, a copy of the secret.
///
/// Every regular file of `dir` whose name has that shape is removed. **Subfolders are not
/// visited**: call it once for each folder that is written in, the folders that hold the
/// secrets among them.
///
/// A write in progress owns a file of the same shape, and removing it makes that write
/// fail (the final file is not harmed). So call this only while nothing else writes in the
/// folder. A lock ([`FileLock`](crate::FileLock)) keeps out only the processes that take
/// that same lock: holding the lock of the first copy of a program does not keep out a
/// command that writes in the folder under another lock, or under none. Either the cleanup
/// takes the lock of every writer of the folder, or each writer cleans, under its own
/// lock, the folder that only it writes in.
///
/// # Errors
///
/// [`FsError::Io`] when the folder cannot be listed, and for the first leftover that could
/// not be removed (the others are still tried).
pub fn remove_stale_temporaries(dir: &Path) -> Result<usize, FsError> {
    let list_failed = |cause| FsError::io("list the folder", dir, cause);
    let mut removed: usize = 0;
    let mut first_failure = None;
    for entry in fs::read_dir(dir).map_err(list_failed)? {
        let entry = entry.map_err(list_failed)?;
        // A folder or a link under such a name was not made by this crate.
        if !is_temporary_name(&entry.file_name())
            || !entry.file_type().is_ok_and(|kind| kind.is_file())
        {
            continue;
        }
        let path = entry.path();
        match fs::remove_file(&path) {
            Ok(()) => removed = removed.saturating_add(1),
            // Somebody else removed it first.
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
            Err(cause) => {
                if first_failure.is_none() {
                    first_failure = Some(FsError::io(
                        "remove the leftover temporary file",
                        &path,
                        cause,
                    ));
                }
            }
        }
    }
    match first_failure {
        Some(failure) => Err(failure),
        None => Ok(removed),
    }
}

/// Whether `name` has the shape that [`create_temporary`] gives: `.{name}.{digits}.tmp`.
fn is_temporary_name(name: &OsStr) -> bool {
    let Some(rest) = name.as_encoded_bytes().strip_prefix(b".") else {
        return false;
    };
    let Some(rest) = rest.strip_suffix(TEMPORARY_EXTENSION.as_bytes()) else {
        return false;
    };
    // The dot and the digits that come after the name of the target.
    let Some(target_len) = rest.len().checked_sub(1 + RANDOM_SUFFIX_BYTES * 2) else {
        return false;
    };
    let (target, drawn) = rest.split_at(target_len);
    !target.is_empty()
        && matches!(
            drawn.split_first(),
            Some((b'.', digits))
                if digits.iter().all(|digit| matches!(digit, b'0'..=b'9' | b'a'..=b'f'))
        )
}

/// Who may use a file created by [`stage`]. Only Unix has a mode to set.
#[derive(Clone, Copy)]
pub(crate) enum Access {
    /// Owner reads and writes (`0600`).
    Private,
    /// Owner reads, writes and runs (`0700`).
    PrivateExecutable,
}

/// A complete temporary file, already forced to disk, waiting to take its final name.
///
/// Dropping it without [`install`](Self::install) removes the temporary file.
pub(crate) struct Staged {
    path: PathBuf,
    installed: bool,
}

impl Staged {
    /// Renames the temporary file over `target` and makes the new name durable.
    pub(crate) fn install(self, target: &Path) -> Result<(), FsError> {
        self.rename_to(target)?;
        settle(target)
    }

    /// Renames the temporary file over `target`. A failure changes nothing under `target`
    /// and removes the temporary file.
    pub(crate) fn rename_to(mut self, target: &Path) -> Result<(), FsError> {
        let rename = || {
            #[cfg(test)]
            probe::step(probe::Step::Rename, target)?;
            fs::rename(&self.path, target)
        };
        rename().map_err(|cause| FsError::io("rename a temporary file to", target, cause))?;
        self.installed = true;
        Ok(())
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.installed {
            // Best effort: the temporary file holds nothing that the final name depends on.
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Creates a temporary file next to `target`, lets `fill` write it and forces it to disk.
pub(crate) fn stage(
    target: &Path,
    access: Access,
    fill: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<Staged, FsError> {
    let name = target.file_name().ok_or_else(|| FsError::NotAFilePath {
        path: target.to_path_buf(),
    })?;
    let (path, mut file) = create_temporary(parent_of(target), name, access)?;
    let staged = Staged {
        path,
        installed: false,
    };
    let written = match fill(&mut file) {
        Ok(()) => sync_file(&file, &staged.path),
        Err(cause) => Err(FsError::io("write", &staged.path, cause)),
    };
    // Closed before the rename, and before the removal that a failure triggers.
    drop(file);
    written.map(|()| staged)
}

/// Forces the content of the temporary file at `path` to disk.
fn sync_file(file: &File, path: &Path) -> Result<(), FsError> {
    let force = || {
        #[cfg(test)]
        probe::step(probe::Step::SyncFile, path)?;
        file.sync_all()
    };
    force().map_err(|cause| FsError::io("force to disk", path, cause))
}

/// Makes durable the entry of `path`, which was just renamed into place or created: forces
/// its folder to disk. A failure here does not undo what is already under `path`, hence an
/// error of its own.
pub(crate) fn settle(path: &Path) -> Result<(), FsError> {
    sync_dir(parent_of(path)).map_err(|cause| FsError::NotDurable {
        path: path.to_path_buf(),
        cause,
    })
}

/// The folder that holds `path`. A bare file name lives in the current folder.
pub(crate) fn parent_of(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// Forces the entries of `dir` to disk, so that a rename or a creation in it survives a
/// power loss.
fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(test)]
    probe::step(probe::Step::SyncDir, dir)?;
    system_sync_dir(dir)
}

#[cfg(unix)]
fn system_sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir).and_then(|folder| folder.sync_all())
}

/// Windows cannot force a folder to disk through the standard library; the weaker guarantee
/// is the documented reason why it is a development platform only.
#[cfg(windows)]
fn system_sync_dir(_dir: &Path) -> io::Result<()> {
    Ok(())
}

/// The steps of a durable write, announced to the test that asked to watch them. A watcher
/// sees the files as they are when a step is about to run, and makes the step fail by
/// returning an error.
#[cfg(test)]
pub(crate) mod probe {
    use std::cell::RefCell;
    use std::io;
    use std::path::Path;

    /// One step, announced with the path it is about to act on.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum Step {
        /// Forcing the temporary file to disk; the path is the temporary file.
        SyncFile,
        /// Renaming the temporary file; the path is the final name.
        Rename,
        /// Forcing the folder to disk; the path is the folder.
        SyncDir,
    }

    type Watcher = Box<dyn FnMut(Step, &Path) -> io::Result<()>>;

    thread_local! {
        /// Per thread, so that a test only watches the writes it makes itself.
        static WATCHER: RefCell<Option<Watcher>> = const { RefCell::new(None) };
    }

    /// Keeps a watcher in place; dropping it ends the watching.
    #[must_use = "the watching ends when this value is dropped"]
    pub(crate) struct Watching;

    impl Drop for Watching {
        fn drop(&mut self) {
            WATCHER.with(|watcher| watcher.borrow_mut().take());
        }
    }

    /// Watches the steps of the writes made by this thread from now on.
    pub(crate) fn watch(watcher: impl FnMut(Step, &Path) -> io::Result<()> + 'static) -> Watching {
        WATCHER.with(|slot| *slot.borrow_mut() = Some(Box::new(watcher)));
        Watching
    }

    /// Makes `step` fail, and no other, in the writes made by this thread from now on.
    pub(crate) fn fail_at(step: Step) -> Watching {
        watch(move |announced, _| {
            if announced == step {
                Err(io::Error::other("the test made this step fail"))
            } else {
                Ok(())
            }
        })
    }

    pub(super) fn step(step: Step, path: &Path) -> io::Result<()> {
        WATCHER.with(|slot| match slot.borrow_mut().as_mut() {
            Some(watcher) => watcher(step, path),
            None => Ok(()),
        })
    }
}

/// A random lowercase suffix for names created next to a target, from the system generator.
pub(crate) fn random_suffix() -> Result<String, FsError> {
    let mut bytes = [0u8; RANDOM_SUFFIX_BYTES];
    getrandom::fill(&mut bytes).map_err(FsError::Random)?;
    Ok(HEXLOWER.encode(&bytes))
}

/// Creates `.{name}.{random}.tmp` in `dir`.
fn create_temporary(dir: &Path, name: &OsStr, access: Access) -> Result<(PathBuf, File), FsError> {
    create_temporary_drawing(dir, name, access, random_suffix)
}

/// Creates `.{name}.{suffix}.tmp` in `dir`, with the suffixes that `draw` gives. A name
/// that already exists is never opened, so a planted file or link is not written through:
/// another name is drawn instead.
fn create_temporary_drawing(
    dir: &Path,
    name: &OsStr,
    access: Access,
    mut draw: impl FnMut() -> Result<String, FsError>,
) -> Result<(PathBuf, File), FsError> {
    for _ in 0..TEMPORARY_NAME_ATTEMPTS {
        let mut temporary = OsString::from(".");
        temporary.push(name);
        temporary.push(format!(".{}{TEMPORARY_EXTENSION}", draw()?));
        let path = dir.join(temporary);

        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        restrict(&mut options, access);
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => {}
            Err(cause) => return Err(FsError::io("create", &path, cause)),
        }
    }
    Err(FsError::NoTemporaryName {
        dir: dir.to_path_buf(),
    })
}

#[cfg(unix)]
fn restrict(options: &mut OpenOptions, access: Access) {
    use std::os::unix::fs::OpenOptionsExt;

    options.mode(match access {
        Access::Private => 0o600,
        Access::PrivateExecutable => 0o700,
    });
}

/// On Windows a new file takes the permissions of its folder.
#[cfg(windows)]
fn restrict(_options: &mut OpenOptions, _access: Access) {}

/// What the probe file of [`user_creating_in`] is called, before the random part.
#[cfg(unix)]
const USER_PROBE_NAME: &str = "user-probe";

/// The user that owns what this process creates in `dir`: the user the process runs as.
///
/// The standard library cannot tell the user of the process, and the system gives every
/// new file to that user. So an empty file is created in `dir` and removed at once. It has
/// the shape of a temporary file: if it ever stays, [`remove_stale_temporaries`] takes it.
#[cfg(unix)]
pub(crate) fn user_creating_in(dir: &Path) -> Result<u32, FsError> {
    use std::os::unix::fs::MetadataExt;

    let (path, probe) = create_temporary(dir, OsStr::new(USER_PROBE_NAME), Access::Private)?;
    // Read from the open file, not from its name: the name may be given to another file.
    let user = probe.metadata().map(|created| created.uid());
    drop(probe);
    let _ = fs::remove_file(&path);
    user.map_err(|cause| FsError::io("inspect", &path, cause))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::io::Read;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::probe::Step;
    use super::*;

    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn text_of(path: &Path) -> String {
        String::from_utf8(fs::read(path).unwrap()).unwrap()
    }

    /// The order that makes "saved" true (04, C7, rule 5): the whole new file is forced to
    /// disk while the final name still holds the old one; then the rename; then the folder.
    #[test]
    fn file_is_forced_before_the_rename_and_the_folder_after() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential.seal");
        write_durable(&path, b"previous").unwrap();
        // Each step, with the path it acts on and what the final name holds as it starts.
        let steps = Rc::new(RefCell::new(Vec::new()));
        let forced_file = Rc::new(RefCell::new(None));
        let watching = probe::watch({
            let (steps, forced_file, path) = (steps.clone(), forced_file.clone(), path.clone());
            move |step, at| {
                if step == Step::SyncFile {
                    *forced_file.borrow_mut() = Some(text_of(at));
                }
                steps
                    .borrow_mut()
                    .push((step, at.to_path_buf(), text_of(&path)));
                Ok(())
            }
        });

        write_durable(&path, b"the new content").unwrap();
        drop(watching);

        let steps = steps.borrow();
        let order: Vec<Step> = steps.iter().map(|(step, _, _)| *step).collect();
        assert_eq!(order, [Step::SyncFile, Step::Rename, Step::SyncDir]);
        let [forcing_file, renaming, forcing_folder] = steps.as_slice() else {
            panic!("three steps were expected: {steps:?}");
        };
        assert_eq!(
            forced_file.borrow().as_deref(),
            Some("the new content"),
            "the file forced to disk is the whole new file"
        );
        assert_ne!(
            forcing_file.1, path,
            "the file is forced under another name"
        );
        assert_eq!(forcing_file.2, "previous");
        assert_eq!((&renaming.1, renaming.2.as_str()), (&path, "previous"));
        assert_eq!(forcing_folder.1, dir.path());
        assert_eq!(forcing_folder.2, "the new content");
    }

    /// No test can watch a disk being flushed, but a file that cannot be flushed tells
    /// whether the system was asked to: the step is a real call, not just a place in the
    /// order.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn forcing_the_file_to_disk_asks_the_system() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential.seal");
        write_durable(&path, b"previous").unwrap();

        let error = stage(&path, Access::Private, |file| {
            *file = file_that_cannot_be_flushed(elsewhere.path())?;
            Ok(())
        })
        .err()
        .expect("a file that could not be flushed was taken for a file on disk");

        assert!(
            matches!(
                &error,
                FsError::Io {
                    action: "force to disk",
                    ..
                }
            ),
            "{error}"
        );
        assert_eq!(fs::read(&path).unwrap(), b"previous");
        assert_eq!(names_in(dir.path()), ["credential.seal"]);
    }

    /// Windows flushes only through a handle that may write.
    #[cfg(windows)]
    fn file_that_cannot_be_flushed(scratch: &Path) -> io::Result<File> {
        let path = scratch.join("opened-for-reading");
        fs::write(&path, b"")?;
        File::open(path)
    }

    /// Linux has nothing to force to disk behind a device like this one, and says so.
    #[cfg(target_os = "linux")]
    fn file_that_cannot_be_flushed(_scratch: &Path) -> io::Result<File> {
        File::open("/dev/null")
    }

    #[test]
    fn failure_to_force_the_file_keeps_the_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential.seal");
        let never_written = dir.path().join("identity.key");
        write_durable(&path, b"previous").unwrap();
        let _watching = probe::fail_at(Step::SyncFile);

        let error = write_durable(&path, b"new").unwrap_err();
        let first_write = write_durable(&never_written, b"new");

        assert!(
            matches!(
                &error,
                FsError::Io {
                    action: "force to disk",
                    ..
                }
            ),
            "{error}"
        );
        assert!(matches!(first_write, Err(FsError::Io { .. })));
        assert_eq!(fs::read(&path).unwrap(), b"previous");
        assert_eq!(names_in(dir.path()), ["credential.seal"]);
    }

    /// The contract of the one failure that comes after the rename: the caller is told
    /// that the new content is in place, not that nothing changed.
    #[test]
    fn failure_to_force_the_folder_says_that_the_new_file_is_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential.seal");
        write_durable(&path, b"previous").unwrap();
        let _watching = probe::fail_at(Step::SyncDir);

        let error = write_durable(&path, b"new").unwrap_err();

        assert!(
            matches!(&error, FsError::NotDurable { path: in_place, .. } if *in_place == path),
            "{error}"
        );
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(names_in(dir.path()), ["credential.seal"]);
    }

    /// The content is never rewritten in place: whoever opened the file before the
    /// replacement still has the old file, whole.
    #[test]
    fn reader_that_came_before_the_replacement_keeps_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential.seal");
        write_durable(&path, b"previous").unwrap();
        let mut opened_before = File::open(&path).unwrap();

        write_durable(&path, b"new and longer content").unwrap();

        let mut seen = Vec::new();
        opened_before.read_to_end(&mut seen).unwrap();
        assert_eq!(seen, b"previous");
        assert_eq!(fs::read(&path).unwrap(), b"new and longer content");
    }

    /// The same rule while it happens: whoever reads the final name during replacements
    /// gets the whole old file or the whole new one.
    #[test]
    fn readers_never_see_a_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential.seal");
        let small = vec![b'a'; 1024];
        let large = vec![b'b'; 256 * 1024];
        write_durable(&path, &small).unwrap();
        let stop = AtomicBool::new(false);

        let (writes, whole, partial) = std::thread::scope(|scope| {
            let readers: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        let (mut whole, mut partial) = (0u64, 0u64);
                        while !stop.load(Ordering::Relaxed) {
                            // A read that fails saw no file, which is not a partial file.
                            match fs::read(&path) {
                                Ok(bytes) if bytes == small || bytes == large => whole += 1,
                                Ok(_) => partial += 1,
                                Err(_) => {}
                            }
                        }
                        (whole, partial)
                    })
                })
                .collect();
            let writes = (0..300).try_for_each(|round| {
                write_durable(&path, if round % 2 == 0 { &large } else { &small })
            });
            // Before any assertion: a reader that is never told to stop would hang the test.
            stop.store(true, Ordering::Relaxed);
            let (mut whole, mut partial) = (0u64, 0u64);
            for reader in readers {
                let (reader_whole, reader_partial) = reader.join().unwrap();
                whole += reader_whole;
                partial += reader_partial;
            }
            (writes, whole, partial)
        });

        writes.unwrap();
        assert_eq!(partial, 0, "partial reads, next to {whole} whole ones");
        assert!(whole > 0, "the readers never read the file");
    }

    #[test]
    fn temporary_files_left_behind_are_removed_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        write_durable(&dir.path().join("credential.seal"), b"kept").unwrap();
        let leftovers = [
            ".credential.seal.0123456789abcdef.tmp",
            ".arauto-connector.exe.fedcba9876543210.tmp",
        ];
        // Close to the shape, and not it: these belong to somebody else.
        let not_leftovers = [
            "credential.seal.1123456789abcdef.tmp",
            ".credential.seal.2123456789ABCDEF.tmp",
            ".credential.seal.3123456789abcde.tmp",
            ".credential.seal.4123456789abcdef0.tmp",
            ".credential.seal.5123456789abcdeg.tmp",
            ".credential.seal.6123456789abcdef.tmp.bak",
            ".credential.seal.a123456789abcdef",
            ".credential.seal-7123456789abcdef.tmp",
            "..8123456789abcdef.tmp",
            ".9123456789abcdef.tmp",
            ".tmp",
            "notes.tmp",
        ];
        for name in leftovers.iter().chain(&not_leftovers) {
            fs::write(dir.path().join(name), b"left").unwrap();
        }
        let folder = ".folder.0123456789abcdef.tmp";
        fs::create_dir(dir.path().join(folder)).unwrap();

        let removed = remove_stale_temporaries(dir.path()).unwrap();

        let mut expected: Vec<String> = not_leftovers
            .iter()
            .chain(&[folder, "credential.seal"])
            .map(|name| String::from(*name))
            .collect();
        expected.sort();
        assert_eq!(removed, leftovers.len());
        assert_eq!(names_in(dir.path()), expected);
        assert_eq!(
            fs::read(dir.path().join("credential.seal")).unwrap(),
            b"kept"
        );
        assert_eq!(remove_stale_temporaries(dir.path()).unwrap(), 0);
    }

    /// What the cleanup takes for a leftover is what an interrupted write really leaves.
    #[test]
    fn file_of_a_write_that_never_finished_is_removed_by_the_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential.seal");
        write_durable(&path, b"previous").unwrap();
        // As when the process dies between writing the temporary file and renaming it.
        std::mem::forget(stage(&path, Access::Private, |file| file.write_all(b"new")).unwrap());
        assert_eq!(names_in(dir.path()).len(), 2);

        assert_eq!(remove_stale_temporaries(dir.path()).unwrap(), 1);

        assert_eq!(names_in(dir.path()), ["credential.seal"]);
        assert_eq!(fs::read(&path).unwrap(), b"previous");
    }

    #[test]
    fn cleanup_of_a_missing_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();

        let outcome = remove_stale_temporaries(&dir.path().join("missing"));

        assert!(matches!(outcome, Err(FsError::Io { .. })), "{outcome:?}");
    }

    #[cfg(windows)]
    #[test]
    fn leftover_that_cannot_be_removed_is_reported_and_the_others_still_go() {
        use std::os::windows::fs::OpenOptionsExt;

        /// `FILE_SHARE_READ | FILE_SHARE_WRITE`: everything but removal.
        const SHARE_ALL_BUT_DELETE: u32 = 0x1 | 0x2;

        let dir = tempfile::tempdir().unwrap();
        let held = dir
            .path()
            .join(".arauto-connector.exe.0123456789abcdef.tmp");
        for name in [
            ".arauto-connector.exe.0023456789abcdef.tmp",
            ".arauto-connector.exe.0123456789abcdef.tmp",
            ".arauto-connector.exe.0223456789abcdef.tmp",
        ] {
            fs::write(dir.path().join(name), b"left").unwrap();
        }
        let holder = OpenOptions::new()
            .read(true)
            .share_mode(SHARE_ALL_BUT_DELETE)
            .open(&held)
            .unwrap();

        let error = remove_stale_temporaries(dir.path()).unwrap_err();

        assert!(
            matches!(&error, FsError::Io { path, .. } if *path == held),
            "{error}"
        );
        assert_eq!(
            names_in(dir.path()),
            [".arauto-connector.exe.0123456789abcdef.tmp"]
        );
        drop(holder);
        assert_eq!(remove_stale_temporaries(dir.path()).unwrap(), 1);
    }

    #[test]
    fn creates_the_file_with_the_new_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential.seal");

        write_durable(&path, b"first").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"first");
        assert_eq!(names_in(dir.path()), ["credential.seal"]);
    }

    #[test]
    fn replaces_what_was_there_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential.seal");
        fs::write(&path, b"a much longer previous content").unwrap();

        write_durable(&path, b"new").unwrap();
        write_durable(&path, b"").unwrap();
        write_durable(&path, b"newest").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"newest");
        assert_eq!(names_in(dir.path()), ["credential.seal"]);
    }

    #[test]
    fn missing_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("credential.seal");

        let error = write_durable(&path, b"x").unwrap_err();

        assert!(matches!(error, FsError::Io { .. }), "{error}");
        assert!(!dir.path().join("missing").exists());
    }

    #[test]
    fn path_without_a_file_name_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("..");

        let error = write_durable(&path, b"x").unwrap_err();

        assert!(matches!(error, FsError::NotAFilePath { .. }), "{error}");
    }

    /// The rule behind "saved": the final name never holds a partial file.
    #[test]
    fn failure_in_the_middle_keeps_the_previous_file_whole() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential.seal");
        write_durable(&path, b"previous").unwrap();

        let outcome = stage(&path, Access::Private, |file| {
            file.write_all(b"half of the new cont")?;
            Err(io::Error::other("disk went away"))
        });

        assert!(matches!(outcome, Err(FsError::Io { .. })));
        assert_eq!(fs::read(&path).unwrap(), b"previous");
        assert_eq!(names_in(dir.path()), ["credential.seal"]);
    }

    #[test]
    fn staged_file_that_is_not_installed_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential.seal");

        let staged = stage(&path, Access::Private, |file| file.write_all(b"new")).unwrap();
        assert_eq!(names_in(dir.path()).len(), 1, "the temporary file exists");
        assert!(!path.exists(), "the final name is not taken before install");
        drop(staged);

        assert!(names_in(dir.path()).is_empty());
    }

    #[test]
    fn failed_rename_removes_the_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        // A folder with something inside cannot be replaced by a file on any system.
        let path = dir.path().join("taken");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("inside"), b"x").unwrap();

        let error = write_durable(&path, b"new").unwrap_err();

        assert!(matches!(error, FsError::Io { .. }), "{error}");
        assert_eq!(names_in(dir.path()), ["taken"]);
        assert_eq!(fs::read(path.join("inside")).unwrap(), b"x");
    }

    #[test]
    fn temporary_names_do_not_repeat() {
        let first = random_suffix().unwrap();
        let second = random_suffix().unwrap();

        assert_eq!(first.len(), 16);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    /// A temporary name that somebody already took is never written through.
    #[test]
    fn file_planted_under_a_drawn_name_is_not_written_through() {
        let dir = tempfile::tempdir().unwrap();
        let planted = dir.path().join(".credential.seal.planted.tmp");
        fs::write(&planted, b"planted").unwrap();
        let mut suffixes = ["planted", "free"].into_iter();

        let (path, mut file) = create_temporary_drawing(
            dir.path(),
            OsStr::new("credential.seal"),
            Access::Private,
            || Ok(suffixes.next().unwrap().to_string()),
        )
        .unwrap();
        file.write_all(b"new").unwrap();

        assert_eq!(path, dir.path().join(".credential.seal.free.tmp"));
        assert_eq!(fs::read(&planted).unwrap(), b"planted");
    }

    #[test]
    fn names_that_are_always_taken_end_in_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let planted = dir.path().join(".credential.seal.planted.tmp");
        fs::write(&planted, b"planted").unwrap();
        let mut draws = 0;

        let outcome = create_temporary_drawing(
            dir.path(),
            OsStr::new("credential.seal"),
            Access::Private,
            || {
                draws += 1;
                Ok("planted".to_string())
            },
        );

        assert!(matches!(outcome, Err(FsError::NoTemporaryName { .. })));
        assert_eq!(draws, TEMPORARY_NAME_ATTEMPTS);
        assert_eq!(fs::read(&planted).unwrap(), b"planted");
    }

    #[test]
    fn bare_file_name_lives_in_the_current_folder() {
        assert_eq!(parent_of(Path::new("credential.seal")), Path::new("."));
        assert_eq!(parent_of(Path::new("a/credential.seal")), Path::new("a"));
    }

    #[cfg(unix)]
    mod unix {
        use std::os::unix::fs::PermissionsExt;

        use super::*;

        fn mode_of(path: &Path) -> u32 {
            fs::metadata(path).unwrap().permissions().mode() & 0o7777
        }

        #[test]
        fn new_file_is_readable_by_the_owner_only() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("identity.key");

            write_durable(&path, b"secret").unwrap();

            assert_eq!(mode_of(&path), 0o600);
        }

        #[test]
        fn replacing_a_file_that_others_could_read_closes_it() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("identity.key");
            fs::write(&path, b"old").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

            write_durable(&path, b"secret").unwrap();

            assert_eq!(mode_of(&path), 0o600);
            assert_eq!(fs::read(&path).unwrap(), b"secret");
        }

        #[test]
        fn staged_executable_can_be_run_by_the_owner_only() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("program");

            stage(&path, Access::PrivateExecutable, |file| {
                file.write_all(b"#!")
            })
            .unwrap()
            .install(&path)
            .unwrap();

            assert_eq!(mode_of(&path), 0o700);
        }

        #[test]
        fn a_link_planted_under_the_final_name_is_replaced_not_followed() {
            let dir = tempfile::tempdir().unwrap();
            let elsewhere = dir.path().join("elsewhere");
            fs::write(&elsewhere, b"untouched").unwrap();
            let path = dir.path().join("credential.seal");
            std::os::unix::fs::symlink(&elsewhere, &path).unwrap();

            write_durable(&path, b"new").unwrap();

            assert_eq!(fs::read(&elsewhere).unwrap(), b"untouched");
            assert!(fs::symlink_metadata(&path).unwrap().is_file());
            assert_eq!(fs::read(&path).unwrap(), b"new");
        }

        #[test]
        fn link_planted_under_a_drawn_name_is_not_followed() {
            let dir = tempfile::tempdir().unwrap();
            let elsewhere = dir.path().join("elsewhere");
            fs::write(&elsewhere, b"untouched").unwrap();
            let nowhere = dir.path().join("nowhere");
            let planted = |suffix: &str| dir.path().join(format!(".credential.seal.{suffix}.tmp"));
            std::os::unix::fs::symlink(&elsewhere, planted("to-a-file")).unwrap();
            std::os::unix::fs::symlink(&nowhere, planted("to-nothing")).unwrap();
            let mut suffixes = ["to-a-file", "to-nothing", "free"].into_iter();

            let (path, mut file) = create_temporary_drawing(
                dir.path(),
                OsStr::new("credential.seal"),
                Access::Private,
                || Ok(suffixes.next().unwrap().to_string()),
            )
            .unwrap();
            file.write_all(b"new").unwrap();

            assert_eq!(path, planted("free"));
            assert_eq!(fs::read(&elsewhere).unwrap(), b"untouched");
            assert!(!nowhere.exists(), "the dangling link was followed");
        }

        /// The owner checks of the folder and of the local socket rest on this: what the
        /// process creates belongs to the user it runs as, and looking leaves nothing.
        #[test]
        fn user_of_the_process_is_the_owner_of_what_it_creates() {
            use std::os::unix::fs::MetadataExt;

            let dir = tempfile::tempdir().unwrap();
            let own_file = dir.path().join("own");
            fs::write(&own_file, b"").unwrap();
            let expected = fs::metadata(&own_file).unwrap().uid();
            fs::remove_file(&own_file).unwrap();

            let user = user_creating_in(dir.path()).unwrap();

            assert_eq!(user, expected);
            assert_eq!(fs::metadata(dir.path()).unwrap().uid(), user);
            assert!(names_in(dir.path()).is_empty(), "the probe file stayed");
        }

        #[test]
        fn user_of_the_process_is_not_guessed_when_no_file_can_be_created() {
            let dir = tempfile::tempdir().unwrap();

            let outcome = user_creating_in(&dir.path().join("missing"));

            assert!(matches!(outcome, Err(FsError::Io { .. })), "{outcome:?}");
        }

        /// A probe file that stays, because the process died right after creating it, is
        /// a leftover like any other.
        #[test]
        fn probe_file_that_stays_is_removed_by_the_cleanup() {
            let dir = tempfile::tempdir().unwrap();
            let (probe, _file) =
                create_temporary(dir.path(), OsStr::new(USER_PROBE_NAME), Access::Private).unwrap();
            assert!(probe.exists());

            assert_eq!(remove_stale_temporaries(dir.path()).unwrap(), 1);

            assert!(names_in(dir.path()).is_empty());
        }

        /// Forcing the folder to disk is a real call too: it fails for a folder that is
        /// not there and, on Linux, for something that has nothing to force to disk.
        #[test]
        fn folder_can_be_forced_to_disk() {
            let dir = tempfile::tempdir().unwrap();

            sync_dir(dir.path()).unwrap();
            assert!(sync_dir(&dir.path().join("missing")).is_err());
            #[cfg(target_os = "linux")]
            assert!(sync_dir(Path::new("/dev/null")).is_err());
        }
    }
}
