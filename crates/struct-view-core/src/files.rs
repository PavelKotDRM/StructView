//! File writes shared by graphical and command-line operations.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Reject replacing an input file, including aliases through symbolic links.
///
/// A destination that does not exist is allowed. Resolution errors are returned
/// rather than treated as evidence that the paths differ.
pub fn ensure_distinct_paths(input: &Path, output: &Path) -> io::Result<()> {
    match fs::metadata(output) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    }
    if input.canonicalize()? == output.canonicalize()? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Imported graph files are read-only and cannot be overwritten",
        ));
    }
    Ok(())
}

/// Replace a UTF-8 file only after its complete contents have been written.
///
/// Existing symbolic links are followed, even when their target does not yet
/// exist, and file permissions are retained.
/// A failed write leaves the destination unchanged.
pub fn write_text_atomic(path: &Path, content: &str) -> io::Result<()> {
    write_bytes_atomic(path, content.as_bytes())
}

/// Atomically create a new file, refusing to replace any existing path.
///
/// Unlike a separate existence check followed by a write, publication remains
/// non-overwriting if another writer creates the destination in the meantime.
pub fn create_bytes_atomic(path: &Path, content: &[u8]) -> io::Result<()> {
    persist_bytes_atomic(path, content, None, false)
}

/// Replace a file atomically, preserving permissions and following symbolic links.
pub fn write_bytes_atomic(path: &Path, content: &[u8]) -> io::Result<()> {
    let mut target = path.to_path_buf();
    // Resolve the final path component, including dangling links whose target
    // is to be created. Parent-directory links are followed by the OS.
    let mut resolved = false;
    for _ in 0..40 {
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let link = fs::read_link(&target)?;
                target = if link.is_absolute() {
                    link
                } else {
                    target
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .unwrap_or_else(|| Path::new("."))
                        .join(link)
                };
            }
            Ok(_) => {
                resolved = true;
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                resolved = true;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    if !resolved {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Too many symbolic links",
        ));
    }
    let permissions = match fs::metadata(&target) {
        Ok(metadata) => {
            if metadata.permissions().readonly() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "The file is read-only",
                ));
            }
            Some(metadata.permissions())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    persist_bytes_atomic(&target, content, permissions, true)
}

fn persist_bytes_atomic(
    target: &Path,
    content: &[u8],
    permissions: Option<fs::Permissions>,
    overwrite: bool,
) -> io::Result<()> {
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(content)?;
    if let Some(permissions) = permissions {
        temporary.as_file().set_permissions(permissions)?;
    }
    temporary.as_file().sync_all()?;
    if overwrite {
        temporary.persist(target)
    } else {
        temporary.persist_noclobber(target)
    }
    .map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_paths_rejects_the_input_but_allows_other_destinations() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("graph.xml");
        let output = directory.path().join("graph.json");
        fs::write(&input, "original").unwrap();
        assert!(ensure_distinct_paths(&input, &input).is_err());
        assert!(ensure_distinct_paths(&input, &output).is_ok());
        fs::write(&output, "other").unwrap();
        assert!(ensure_distinct_paths(&input, &output).is_ok());
        assert!(ensure_distinct_paths(&directory.path().join("missing"), &output).is_err());
        assert_eq!(fs::read_to_string(input).unwrap(), "original");
    }

    #[cfg(unix)]
    #[test]
    fn distinct_paths_rejects_symbolic_link_aliases() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("graph.xml");
        let alias = directory.path().join("graph.json");
        fs::write(&input, "original").unwrap();
        std::os::unix::fs::symlink(&input, &alias).unwrap();
        assert!(ensure_distinct_paths(&input, &alias).is_err());
    }

    #[test]
    fn binary_atomic_write_preserves_all_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("image.png");
        let bytes = [0, 255, 128, b'\n', 0];
        write_bytes_atomic(&path, &bytes).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        write_bytes_atomic(&path, &[1, 2]).unwrap();
        assert_eq!(fs::read(&path).unwrap(), [1, 2]);
    }

    #[test]
    fn atomic_creation_never_replaces_an_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document.json");
        create_bytes_atomic(&path, b"original").unwrap();
        assert_eq!(
            create_bytes_atomic(&path, b"replacement")
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists,
        );
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn concurrent_atomic_creators_publish_exactly_one_complete_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document.json");
        let barrier = std::sync::Barrier::new(2);
        let results = std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                barrier.wait();
                create_bytes_atomic(&path, b"first")
            });
            let second = scope.spawn(|| {
                barrier.wait();
                create_bytes_atomic(&path, b"second")
            });
            [first.join().unwrap(), second.join().unwrap()]
        });
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .find_map(|result| result.as_ref().err())
                .unwrap()
                .kind(),
            io::ErrorKind::AlreadyExists,
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            if results[0].is_ok() {
                b"first".as_slice()
            } else {
                b"second".as_slice()
            },
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_creation_does_not_follow_or_replace_dangling_links() {
        let directory = tempfile::tempdir().unwrap();
        let link = directory.path().join("link.json");
        let target = directory.path().join("target.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(create_bytes_atomic(&link, b"replacement").is_err());
        assert!(!target.exists());
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn creates_and_replaces_files_without_leaving_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document.json");
        write_text_atomic(&path, "first").unwrap();
        write_text_atomic(&path, "second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_writes_preserve_the_destination_and_remove_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document.json");
        fs::write(&path, "original").unwrap();
        let original_permissions = fs::metadata(&path).unwrap().permissions();
        let mut readonly = original_permissions.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let result = write_text_atomic(&path, "replacement");
        fs::set_permissions(&path, original_permissions).unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        assert!(write_text_atomic(directory.path(), "replacement").is_err());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn follows_existing_symbolic_links() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.json");
        let link = directory.path().join("link.json");
        fs::write(&target, "original").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        write_text_atomic(&link, "replacement").unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "replacement");
    }

    #[cfg(unix)]
    #[test]
    fn creates_the_target_of_a_dangling_symbolic_link() {
        let directory = tempfile::tempdir_in(".").unwrap();
        let target = directory.path().join("target.json");
        let link = directory.path().join("link.json");
        std::os::unix::fs::symlink("target.json", &link).unwrap();

        write_text_atomic(&link, "created").unwrap();

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "created");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    }
}
