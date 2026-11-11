//! Replacing a program that may be running.

use std::fs::{self, File};
use std::io;
use std::path::Path;

use crate::durable::{Access, Staged, stage};
use crate::error::FsError;

/// Copies the program at `source` to `destination`, which may be a program in execution.
///
/// The copy is written to a temporary file in the folder of `destination` and forced to
/// disk before it takes the final name, so `destination` never holds a partial program.
/// When both paths name the same file, nothing is done.
///
/// On Unix the temporary file is renamed over `destination`: a process that runs the old
/// program keeps it until it exits. The new file has mode `0700`. The folder is then
/// forced to disk; if that fails, the new program is already in place and the error is
/// [`FsError::NotDurable`].
///
/// On Windows a program in execution cannot be replaced or removed, but it can be renamed:
/// the current `destination` is first renamed to `{name}.old-{random}` and the new program
/// takes its place. Once it is there, the renamed file and the leftovers of earlier calls
/// are removed, as far as they are no longer in use (their names are matched as the system
/// matches names, without regard to the case of ASCII letters). If the new program cannot
/// take the place, the old one is put back; if that fails too, the error is
/// [`FsError::OldProgramNotRestored`], which says where the old program is. The two
/// renames are not one step: a process that dies between them leaves no program under
/// `destination`, and the next call puts one there.
///
/// A process that dies in the middle leaves the temporary file behind; see
/// [`remove_stale_temporaries`](crate::remove_stale_temporaries).
///
/// # Errors
///
/// [`FsError::NotAFilePath`] when `destination` has no file name;
/// [`FsError::IsADirectory`] when it is a folder; [`FsError::NotDurable`] and
/// [`FsError::OldProgramNotRestored`] as described above; [`FsError::Io`] when `source`
/// cannot be read or any other step fails, with the program that was under `destination`
/// still there.
pub fn replace_executable(source: &Path, destination: &Path) -> Result<(), FsError> {
    // Unix would refuse the rename over a folder; Windows would rename the folder away.
    if fs::metadata(destination).is_ok_and(|existing| existing.is_dir()) {
        return Err(FsError::IsADirectory {
            path: destination.to_path_buf(),
        });
    }
    if is_same_file(source, destination) {
        return Ok(());
    }
    let mut program = File::open(source).map_err(|cause| FsError::io("open", source, cause))?;
    let staged = stage(destination, Access::PrivateExecutable, |file| {
        io::copy(&mut program, file).map(drop)
    })?;
    install(staged, destination)
}

/// Whether the two paths resolve to one file. A destination that does not exist yet is
/// never the same file, and neither is a link: a link to the source is replaced by a copy,
/// so that the program does not vanish when the source is removed.
fn is_same_file(source: &Path, destination: &Path) -> bool {
    if fs::symlink_metadata(destination).is_ok_and(|entry| entry.file_type().is_symlink()) {
        return false;
    }
    match (fs::canonicalize(source), fs::canonicalize(destination)) {
        (Ok(source), Ok(destination)) => source == destination,
        _ => false,
    }
}

#[cfg(unix)]
fn install(staged: Staged, destination: &Path) -> Result<(), FsError> {
    staged.install(destination)
}

#[cfg(windows)]
fn install(staged: Staged, destination: &Path) -> Result<(), FsError> {
    windows::install(staged, destination)
}

#[cfg(windows)]
mod windows {
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    use crate::durable::{Staged, parent_of, random_suffix, settle};
    use crate::error::FsError;

    /// What comes between the name of the program and the random part in a leftover.
    const LEFTOVER_MARK: &str = ".old-";

    pub(super) fn install(staged: Staged, destination: &Path) -> Result<(), FsError> {
        let name = destination
            .file_name()
            .ok_or_else(|| FsError::NotAFilePath {
                path: destination.to_path_buf(),
            })?;
        let dir = parent_of(destination);

        let parked = park(dir, name, destination)?;
        if let Err(error) = staged.rename_to(destination) {
            return Err(match parked {
                Some(parked) => put_back(parked, destination, error),
                None => error,
            });
        }
        // Only now, with a program in place: until here a leftover may be the only copy of
        // the old program. The file just renamed away is one of the leftovers.
        remove_leftovers(dir, name);
        settle(destination)
    }

    /// Returns the old program to its place after the new one failed with `cause` to take
    /// it. Gives the error for the caller: `cause` itself when the old program is back, and
    /// otherwise an error that also says where the old program is.
    fn put_back(parked: PathBuf, destination: &Path, cause: FsError) -> FsError {
        match fs::rename(&parked, destination) {
            Ok(()) => cause,
            Err(put_back) => FsError::OldProgramNotRestored {
                destination: destination.to_path_buf(),
                parked,
                cause: Box::new(cause),
                put_back,
            },
        }
    }

    /// Renames the current program out of the way. `None` when there was no program.
    fn park(dir: &Path, name: &OsStr, destination: &Path) -> Result<Option<PathBuf>, FsError> {
        let mut leftover = leftover_prefix(name);
        leftover.push(random_suffix()?);
        let parked = dir.join(leftover);
        match fs::rename(destination, &parked) {
            Ok(()) => Ok(Some(parked)),
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(cause) => Err(FsError::io("rename out of the way", destination, cause)),
        }
    }

    fn leftover_prefix(name: &OsStr) -> OsString {
        let mut prefix = name.to_os_string();
        prefix.push(LEFTOVER_MARK);
        prefix
    }

    /// Removes what replacements could not remove. Never fails: a leftover that is still in
    /// use (the old program, while it runs) stays for the next call.
    fn remove_leftovers(dir: &Path, name: &OsStr) {
        let prefix = leftover_prefix(name);
        let prefix = prefix.as_encoded_bytes();
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            // The system finds a file whatever the case its name is written in, so the
            // leftovers of `Program.exe` are also those of `program.exe`. Only ASCII
            // letters are folded: that never takes the file of another program for a
            // leftover, and misses one only when other letters differ in case.
            let is_leftover = entry
                .file_name()
                .as_encoded_bytes()
                .get(..prefix.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(prefix));
            if is_leftover {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::io::{BufRead, BufReader, Read};
    use std::process::{Command, Stdio};
    use std::rc::Rc;

    use super::*;
    use crate::durable::probe::{self, Step};
    use crate::test_support::{CHILD_READY, ChildGuard, child_role};

    /// Role of the copy of this test program that keeps running while it is replaced.
    const ROLE_RUN_UNTIL_STDIN_CLOSES: &str = "run-until-stdin-closes";

    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn destination_that_did_not_exist_gets_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let destination = dir.path().join("bin").join("arauto-connector");
        fs::create_dir(dir.path().join("bin")).unwrap();
        fs::write(&source, b"program v1").unwrap();

        replace_executable(&source, &destination).unwrap();

        assert_eq!(fs::read(&destination).unwrap(), b"program v1");
        assert_eq!(names_in(&dir.path().join("bin")), ["arauto-connector"]);
        assert_eq!(
            fs::read(&source).unwrap(),
            b"program v1",
            "the source stays"
        );
    }

    #[test]
    fn destination_that_existed_gets_the_new_program() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let destination = dir.path().join("arauto-connector");
        fs::write(&destination, b"a longer program, version 1").unwrap();
        fs::write(&source, b"program v2").unwrap();

        replace_executable(&source, &destination).unwrap();

        assert_eq!(fs::read(&destination).unwrap(), b"program v2");
        assert_eq!(names_in(dir.path()), ["arauto-connector", "download"]);
    }

    #[test]
    fn same_file_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("arauto-connector");
        fs::write(&program, b"program v1").unwrap();
        let other_spelling = dir.path().join(".").join("arauto-connector");
        #[cfg(unix)]
        let file_before = std::os::unix::fs::MetadataExt::ino(&fs::metadata(&program).unwrap());

        replace_executable(&program, &other_spelling).unwrap();

        assert_eq!(fs::read(&program).unwrap(), b"program v1");
        assert_eq!(names_in(dir.path()), ["arauto-connector"]);
        // Not even rewritten with the same bytes: it is still the very same file.
        #[cfg(unix)]
        assert_eq!(
            std::os::unix::fs::MetadataExt::ino(&fs::metadata(&program).unwrap()),
            file_before
        );
    }

    #[test]
    fn missing_source_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("arauto-connector");
        fs::write(&destination, b"program v1").unwrap();

        let error = replace_executable(&dir.path().join("missing"), &destination).unwrap_err();

        assert!(matches!(error, FsError::Io { .. }), "{error}");
        assert_eq!(fs::read(&destination).unwrap(), b"program v1");
        assert_eq!(names_in(dir.path()), ["arauto-connector"]);
    }

    #[test]
    fn folder_under_the_destination_name_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let destination = dir.path().join("arauto-connector");
        fs::write(&source, b"program v1").unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("inside"), b"kept").unwrap();

        let error = replace_executable(&source, &destination).unwrap_err();

        assert!(matches!(error, FsError::IsADirectory { .. }), "{error}");
        assert_eq!(fs::read(destination.join("inside")).unwrap(), b"kept");
        assert_eq!(names_in(dir.path()), ["arauto-connector", "download"]);
    }

    #[test]
    fn missing_destination_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        fs::write(&source, b"program v1").unwrap();

        let error =
            replace_executable(&source, &dir.path().join("missing").join("program")).unwrap_err();

        assert!(matches!(error, FsError::Io { .. }), "{error}");
        assert_eq!(names_in(dir.path()), ["download"]);
    }

    /// The same order as any durable write: the whole program is forced to disk under
    /// another name, then it takes the place, then the folder is forced to disk.
    #[test]
    fn program_is_forced_before_it_takes_the_place_and_the_folder_after() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let destination = dir.path().join("bin").join("arauto-connector");
        fs::create_dir(dir.path().join("bin")).unwrap();
        fs::write(&source, b"program v2").unwrap();
        fs::write(&destination, b"program v1").unwrap();
        // Each step, with the path it acts on and what that path holds as the step starts.
        let steps = Rc::new(RefCell::new(Vec::new()));
        let watching = probe::watch({
            let (steps, destination) = (steps.clone(), destination.clone());
            move |step, at| {
                let holds = match step {
                    Step::SyncFile => fs::read(at).ok(),
                    // On Windows the old program is out of the way by now.
                    Step::Rename => None,
                    Step::SyncDir => fs::read(&destination).ok(),
                };
                steps.borrow_mut().push((step, at.to_path_buf(), holds));
                Ok(())
            }
        });

        replace_executable(&source, &destination).unwrap();
        drop(watching);

        let steps = steps.borrow();
        let [forcing_program, taking_the_place, forcing_folder] = steps.as_slice() else {
            panic!("three steps were expected: {steps:?}");
        };
        assert_eq!(forcing_program.0, Step::SyncFile);
        assert_ne!(forcing_program.1, destination);
        assert_eq!(forcing_program.2.as_deref(), Some(&b"program v2"[..]));
        assert_eq!(
            (taking_the_place.0, &taking_the_place.1),
            (Step::Rename, &destination)
        );
        assert_eq!(
            *forcing_folder,
            (
                Step::SyncDir,
                dir.path().join("bin"),
                Some(b"program v2".to_vec())
            )
        );
    }

    /// A new program that cannot take the place leaves the old one where it was: on
    /// Windows, where the old one is renamed away first, it is put back.
    #[test]
    fn old_program_is_in_place_when_the_new_one_cannot_take_it() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let destination = dir.path().join("arauto-connector");
        fs::write(&source, b"program v2").unwrap();
        fs::write(&destination, b"program v1").unwrap();
        let _watching = probe::fail_at(Step::Rename);

        let error = replace_executable(&source, &destination).unwrap_err();

        assert!(matches!(error, FsError::Io { .. }), "{error}");
        assert_eq!(fs::read(&destination).unwrap(), b"program v1");
        assert_eq!(names_in(dir.path()), ["arauto-connector", "download"]);
    }

    #[test]
    fn failure_to_force_the_folder_says_that_the_new_program_is_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let destination = dir.path().join("arauto-connector");
        fs::write(&source, b"program v2").unwrap();
        fs::write(&destination, b"program v1").unwrap();
        let _watching = probe::fail_at(Step::SyncDir);

        let error = replace_executable(&source, &destination).unwrap_err();

        assert!(
            matches!(&error, FsError::NotDurable { path, .. } if *path == destination),
            "{error}"
        );
        assert_eq!(fs::read(&destination).unwrap(), b"program v2");
        assert_eq!(names_in(dir.path()), ["arauto-connector", "download"]);
    }

    /// When even the way back fails, the caller learns where the old program is, and that
    /// file is not removed before a new program is in place.
    #[cfg(windows)]
    #[test]
    fn old_program_that_cannot_be_put_back_is_reported_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let destination = dir.path().join("arauto-connector.exe");
        fs::write(&source, b"program v2").unwrap();
        fs::write(&destination, b"program v1").unwrap();
        // The new program fails to take the place, and by then something that cannot be
        // replaced has the name: the old program has nowhere to go back to.
        let watching = probe::watch({
            let destination = destination.clone();
            move |step, _| {
                if step != Step::Rename {
                    return Ok(());
                }
                fs::create_dir(&destination)?;
                fs::write(destination.join("inside"), b"x")?;
                Err(io::Error::other("the test made this step fail"))
            }
        });

        let error = replace_executable(&source, &destination).unwrap_err();
        drop(watching);

        let FsError::OldProgramNotRestored {
            destination: emptied,
            parked,
            ..
        } = &error
        else {
            panic!("the error does not say where the old program is: {error}");
        };
        assert_eq!(*emptied, destination);
        assert_eq!(parked.parent(), Some(dir.path()));
        assert_eq!(fs::read(parked).unwrap(), b"program v1");
        assert!(
            error.to_string().contains(&parked.display().to_string()),
            "{error}"
        );

        // A replacement that fails again does not remove the only copy of the old program.
        fs::remove_dir_all(&destination).unwrap();
        let failing = probe::fail_at(Step::Rename);
        replace_executable(&source, &destination).unwrap_err();
        drop(failing);
        assert_eq!(fs::read(parked).unwrap(), b"program v1");
        assert!(!destination.exists());

        // One that succeeds does: a program is in place again.
        replace_executable(&source, &destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"program v2");
        assert_eq!(names_in(dir.path()), ["arauto-connector.exe", "download"]);
    }

    /// The reason this function exists: the destination is a program in execution.
    #[test]
    fn program_in_execution_is_replaced() {
        if child_role().as_deref() == Some(ROLE_RUN_UNTIL_STDIN_CLOSES) {
            println!("{CHILD_READY}");
            let mut rest = Vec::new();
            let _ = std::io::stdin().read_to_end(&mut rest);
            return;
        }

        // Next to the test program, where programs are sure to be allowed to run.
        let this_program = std::env::current_exe().unwrap();
        let dir = tempfile::tempdir_in(this_program.parent().unwrap()).unwrap();
        let name = format!("running-program{}", std::env::consts::EXE_SUFFIX);
        let destination = dir.path().join(&name);
        fs::copy(&this_program, &destination).unwrap();
        let source = dir.path().join("download");
        fs::write(&source, b"program v2").unwrap();

        let mut running = ChildGuard::spawn(
            Command::new(&destination)
                .args([
                    "executable::tests::program_in_execution_is_replaced",
                    "--exact",
                    "--nocapture",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null()),
            ROLE_RUN_UNTIL_STDIN_CLOSES,
        );
        let mut output = BufReader::new(running.stdout());
        let mut line = String::new();
        while line.trim_end() != CHILD_READY {
            line.clear();
            assert_ne!(
                output.read_line(&mut line).unwrap(),
                0,
                "the copy never started"
            );
        }

        // A program copied onto itself is left as it is: on Windows nothing is renamed
        // away, which would leave a file behind for as long as the program runs.
        replace_executable(&destination, &destination).unwrap();
        assert_eq!(names_in(dir.path()), ["download".to_string(), name.clone()]);

        replace_executable(&source, &destination).unwrap();

        assert_eq!(fs::read(&destination).unwrap(), b"program v2");
        assert!(running.is_running(), "the old program keeps running");
        // Windows keeps the file of a running program: it stays under the other name.
        #[cfg(windows)]
        assert_eq!(
            names_in(dir.path())
                .iter()
                .filter(|entry| entry.starts_with(&format!("{name}.old-")))
                .count(),
            1,
            "{:?}",
            names_in(dir.path())
        );
        #[cfg(unix)]
        assert_eq!(names_in(dir.path()), ["download".to_string(), name.clone()]);

        running.close_stdin();
        assert!(running.wait().success(), "the old program ends by itself");

        // With the old program gone, a later replacement leaves nothing behind. The system
        // may hold the file of a process for an instant after it ends, hence the retries.
        fs::write(&source, b"program v3").unwrap();
        let expected = ["download".to_string(), name];
        for _ in 0..100 {
            replace_executable(&source, &destination).unwrap();
            if names_in(dir.path()) == expected {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        assert_eq!(fs::read(&destination).unwrap(), b"program v3");
        assert_eq!(names_in(dir.path()), expected);
    }

    #[cfg(windows)]
    #[test]
    fn leftovers_of_earlier_replacements_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let destination = dir.path().join("arauto-connector.exe");
        fs::write(&source, b"program v2").unwrap();
        fs::write(&destination, b"program v1").unwrap();
        fs::write(
            dir.path().join("arauto-connector.exe.old-0011223344556677"),
            b"v0",
        )
        .unwrap();
        // Left when the same program was reached through a name written in another case.
        fs::write(
            dir.path().join("Arauto-Connector.EXE.old-8899aabbccddeeff"),
            b"v0",
        )
        .unwrap();
        // Same beginning, other program: not a leftover of this destination.
        fs::write(dir.path().join("arauto-connector.exe-notes.old-1"), b"keep").unwrap();
        fs::write(dir.path().join("other.exe.old-0011223344556677"), b"keep").unwrap();

        replace_executable(&source, &destination).unwrap();

        assert_eq!(fs::read(&destination).unwrap(), b"program v2");
        assert_eq!(
            names_in(dir.path()),
            [
                "arauto-connector.exe",
                "arauto-connector.exe-notes.old-1",
                "download",
                "other.exe.old-0011223344556677",
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn new_program_can_be_run_by_the_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let destination = dir.path().join("arauto-connector");
        fs::write(&source, b"#!/bin/sh\n").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();

        replace_executable(&source, &destination).unwrap();

        let mode = fs::metadata(&destination).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, 0o700);
    }

    #[cfg(unix)]
    #[test]
    fn link_to_the_source_becomes_a_copy_of_it() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let destination = dir.path().join("arauto-connector");
        fs::write(&source, b"program v1").unwrap();
        std::os::unix::fs::symlink(&source, &destination).unwrap();

        replace_executable(&source, &destination).unwrap();
        fs::remove_file(&source).unwrap();

        assert!(fs::symlink_metadata(&destination).unwrap().is_file());
        assert_eq!(fs::read(&destination).unwrap(), b"program v1");
    }

    #[cfg(unix)]
    #[test]
    fn source_that_is_a_link_to_the_destination_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("arauto-connector");
        let source = dir.path().join("link-to-it");
        fs::write(&destination, b"program v1").unwrap();
        std::os::unix::fs::symlink(&destination, &source).unwrap();

        replace_executable(&source, &destination).unwrap();

        assert_eq!(fs::read(&destination).unwrap(), b"program v1");
        assert_eq!(names_in(dir.path()), ["arauto-connector", "link-to-it"]);
    }
}
